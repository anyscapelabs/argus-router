//! Ingress: Live Speech-to-Speech WebSocket endpoint (`GET /v1/live`).
//!
//! Authenticates via Bearer / x-api-key / `?key=` query param, checks monthly
//! voice_minutes quota, then bidirectionally proxies to Google's Gemini
//! Multimodal Live API (BidiGenerateContent).
//!
//! # Supported Live models
//! The allowlist and billing rates live in `catalog/live_models.json`
//! (embedded at build time; see [`models`]). Requests with an unknown
//! `?model=` are rejected with 400; omitting `?model=` selects the registry's
//! default entry. Live models never appear in the chat model catalogs.
//!
//! # Rate-limit handling (65 k TPM)
//! Google Live enforces 65,000 tokens-per-minute. When an upstream text frame
//! contains a 429 error envelope, the proxy:
//!   1. Pauses forwarding for `Retry-After` seconds (capped at 60 s).
//!   2. Sends the client a `{"type":"rate_limited","retry_after":<n>}` notice.
//!   3. Resumes after the backoff. After `MAX_RATE_LIMIT_RETRIES` consecutive
//!      429s the session is closed gracefully.
//!
//! # Voice→text fallback
//! When the client disconnects after sending audio (but before receiving a
//! complete reply), the last partial model text is stored in `LiveSessionStore`
//! for two minutes. The desktop app polls `GET /v1/live/last-reply?session_id=`
//! and injects the reply as a normal chat message.

mod frames;
mod models;
mod session;
mod store;
mod usage;

pub use store::LiveSessionStore;

use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use serde::Deserialize;
use std::time::Instant;

use crate::api::ingress::authenticate;
use crate::state::AppState;

use frames::new_session_id;
use models::{all_model_ids, default_live_model, find_live_model};
use usage::{LiveUsage, record};

// ─── query structs ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct LiveQuery {
    pub key: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LastReplyQuery {
    pub session_id: String,
    pub key: Option<String>,
}

// ─── helpers ─────────────────────────────────────────────────────────────────

/// Allows the API key via `?key=` query param (WS clients can't set custom
/// headers): injects it as an `Authorization: Bearer` header when the request
/// carries no auth header of its own.
fn inject_query_key(headers: &mut HeaderMap, key: &Option<String>) {
    if headers.contains_key("authorization") || headers.contains_key("x-api-key") {
        return;
    }
    let Some(key) = key else { return };
    if let Ok(val) = axum::http::HeaderValue::from_str(&format!("Bearer {key}")) {
        headers.insert(axum::http::header::AUTHORIZATION, val);
    }
}

// ─── GET /v1/live/last-reply ─────────────────────────────────────────────────

/// Returns the last model text reply stored for a session that ended early.
/// Used by the desktop app for voice→text fallback.
pub async fn last_reply_handler(
    State(state): State<AppState>,
    mut headers: HeaderMap,
    Query(query): Query<LastReplyQuery>,
) -> Response {
    // Mirror the same key injection as the live handler.
    inject_query_key(&mut headers, &query.key);
    if authenticate(&state, &headers).await.is_err() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": {"code": "unauthorized", "message": "invalid API key"}
            })),
        )
            .into_response();
    }
    match state.live_sessions.take(&query.session_id) {
        Some(text) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "session_id": query.session_id,
                "text": text,
            })),
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": {
                    "code": "not_found",
                    "message": "no pending reply for this session (may have expired or never existed)"
                }
            })),
        )
            .into_response(),
    }
}

// ─── GET /v1/live (WebSocket upgrade) ───────────────────────────────────────

/// Upgrades the HTTP connection to a WebSocket after authenticating the caller
/// and verifying their monthly voice-minutes quota.
pub async fn live_websocket_handler(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
    mut headers: HeaderMap,
    Query(query): Query<LiveQuery>,
) -> Response {
    let start = Instant::now();
    let model = query
        .model
        .clone()
        .unwrap_or_else(|| default_live_model().id.clone());

    // Reject unknown live models instead of blindly proxying them upstream:
    // keeps cost accounting correct and the upstream allowlist tight.
    if find_live_model(&model).is_none() {
        let allowed = all_model_ids();
        record(
            &state,
            LiveUsage {
                key_id: None,
                user_id: None,
                plan_id: None,
                model: &model,
                status: StatusCode::BAD_REQUEST.as_u16() as i32,
                latency_ms: start.elapsed().as_millis() as i64,
                error_code: Some("unknown_live_model"),
                cost_usd: 0.0,
            },
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": {
                    "code": "INVALID_ARGUMENT",
                    "message": format!(
                        "unknown live model `{model}`; supported models: {}",
                        allowed.join(", ")
                    ),
                    "supported_models": allowed,
                }
            })),
        )
            .into_response();
    }

    // Allow API key via ?key= query param (WS clients can't set custom headers).
    inject_query_key(&mut headers, &query.key);

    let principal = match authenticate(&state, &headers).await {
        Ok(p) => p,
        Err(err) => {
            record(
                &state,
                LiveUsage {
                    key_id: None,
                    user_id: None,
                    plan_id: None,
                    model: &model,
                    status: err.status as i32,
                    latency_ms: start.elapsed().as_millis() as i64,
                    error_code: Some(err.code.as_str()),
                    cost_usd: 0.0,
                },
            );
            return (
                StatusCode::from_u16(err.status).unwrap_or(StatusCode::UNAUTHORIZED),
                Json(serde_json::json!({
                    "error": {"code": err.code.as_str(), "message": err.message}
                })),
            )
                .into_response();
        }
    };

    // Check voice quota atomically (advisory lock). DB errors bubble up as
    // 500s rather than being silently treated as quota exhaustion.
    let (used_minutes, allowed_minutes) = match state
        .app_db
        .check_voice_quota(&principal.user_id, &principal.plan)
        .await
    {
        Ok(crate::services::app_db::QuotaCheck::NotAvailable) => {
            record(
                &state,
                LiveUsage {
                    key_id: Some(&principal.key_id),
                    user_id: Some(&principal.user_id),
                    plan_id: Some(&principal.plan),
                    model: &model,
                    status: StatusCode::FORBIDDEN.as_u16() as i32,
                    latency_ms: start.elapsed().as_millis() as i64,
                    error_code: Some("voice_not_available"),
                    cost_usd: 0.0,
                },
            );
            return (
                StatusCode::FORBIDDEN,
                Json(serde_json::json!({
                    "error": {
                        "code": "FORBIDDEN",
                        "message": format!(
                            "live voice is not available on your plan `{}`",
                            principal.plan
                        ),
                    }
                })),
            )
                .into_response();
        }
        Ok(crate::services::app_db::QuotaCheck::Exceeded { used, quota }) => {
            record(
                &state,
                LiveUsage {
                    key_id: Some(&principal.key_id),
                    user_id: Some(&principal.user_id),
                    plan_id: Some(&principal.plan),
                    model: &model,
                    status: StatusCode::TOO_MANY_REQUESTS.as_u16() as i32,
                    latency_ms: start.elapsed().as_millis() as i64,
                    error_code: Some("voice_minutes_exhausted"),
                    cost_usd: 0.0,
                },
            );
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(serde_json::json!({
                    "error": {
                        "code": "RESOURCE_EXHAUSTED",
                        "message": "Monthly voice minutes quota exhausted for this plan.",
                        "voice_minutes_limit": quota,
                        "voice_minutes_used": used,
                    }
                })),
            )
                .into_response();
        }
        Ok(crate::services::app_db::QuotaCheck::Allowed { used, quota }) => (used, quota),
        Err(err) => {
            tracing::error!(%err, user_id = %principal.user_id, "voice quota check failed");
            record(
                &state,
                LiveUsage {
                    key_id: Some(&principal.key_id),
                    user_id: Some(&principal.user_id),
                    plan_id: Some(&principal.plan),
                    model: &model,
                    status: StatusCode::INTERNAL_SERVER_ERROR.as_u16() as i32,
                    latency_ms: start.elapsed().as_millis() as i64,
                    error_code: Some("quota_check_failed"),
                    cost_usd: 0.0,
                },
            );
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": {
                        "code": "UPSTREAM_ERROR",
                        "message": "the voice quota lookup failed",
                    }
                })),
            )
                .into_response();
        }
    };

    let remaining_minutes = (allowed_minutes - used_minutes).max(0);
    let gemini_key = state.settings.gemini_api_key.clone();
    let session_id = new_session_id();

    ws.on_upgrade(move |socket| async move {
        session::handle_socket(
            socket,
            state,
            principal.key_id,
            principal.user_id,
            principal.plan,
            model,
            remaining_minutes,
            gemini_key,
            session_id,
        )
        .await;
    })
}
