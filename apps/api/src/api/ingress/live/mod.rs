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

fn inject_query_key(headers: &mut HeaderMap, key: &Option<String>) {
    if headers.contains_key("authorization") || headers.contains_key("x-api-key") {
        return;
    }
    let Some(key) = key else { return };
    if let Ok(val) = axum::http::HeaderValue::from_str(&format!("Bearer {key}")) {
        headers.insert(axum::http::header::AUTHORIZATION, val);
    }
}

pub async fn last_reply_handler(
    State(state): State<AppState>,
    mut headers: HeaderMap,
    Query(query): Query<LastReplyQuery>,
) -> Response {
    inject_query_key(&mut headers, &query.key);
    let principal = match authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": {"code": "unauthorized", "message": "An error has occurred. Try again later."}
                })),
            )
                .into_response();
        }
    };
    match state.live_sessions.take(&query.session_id, &principal.user_id) {
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
                    "message": "An error has occurred. Try again later."
                }
            })),
        )
            .into_response(),
    }
}

pub async fn live_websocket_handler(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
    mut headers: HeaderMap,
    Query(query): Query<LiveQuery>,
) -> Response {
    let ws = ws.max_message_size(1 << 20).max_frame_size(256 << 10);
    let start = Instant::now();
    let model = query
        .model
        .clone()
        .unwrap_or_else(|| default_live_model().id.clone());

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
                    "message": "An error has occurred. Try again later.",
                    "supported_models": allowed,
                }
            })),
        )
            .into_response();
    }

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
                    "error": {"code": err.code.as_str(), "message": err.public_message()}
                })),
            )
                .into_response();
        }
    };

    let credits_paid =
        match crate::api::ingress::enforce_plan_quotas(&state, &principal.user_id, &principal.plan)
            .await
        {
            Ok(credits_paid) => credits_paid,
            Err(err) => {
                record(
                    &state,
                    LiveUsage {
                        key_id: Some(&principal.key_id),
                        user_id: Some(&principal.user_id),
                        plan_id: Some(&principal.plan),
                        model: &model,
                        status: err.status as i32,
                        latency_ms: start.elapsed().as_millis() as i64,
                        error_code: Some(err.code.as_str()),
                        cost_usd: 0.0,
                    },
                );
                return (
                    StatusCode::from_u16(err.status).unwrap_or(StatusCode::TOO_MANY_REQUESTS),
                    Json(serde_json::json!({
                        "error": {"code": err.code.as_str(), "message": err.public_message()}
                    })),
                )
                    .into_response();
            }
        };

    let session_id = new_session_id();
    let (used_minutes, allowed_minutes) = match state
        .app_db
        .reserve_voice_session(&principal.user_id, &principal.plan, &session_id)
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
                        "message": "An error has occurred. Try again later.",
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
                        "message": "An error has occurred. Try again later.",
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
                        "message": "An error has occurred. Try again later.",
                    }
                })),
            )
                .into_response();
        }
    };

    let remaining_minutes = (allowed_minutes - used_minutes).max(0);
    let credit_reservation_id = if credits_paid {
        let cost_per_minute = find_live_model(&model)
            .map(models::LiveModelSpec::blended_cost_per_min)
            .unwrap_or(0.0);
        let max_session_micros = (remaining_minutes.max(1) as f64 * cost_per_minute * 1_000_000.0)
            .ceil()
            .clamp(1.0, i64::MAX as f64) as i64;
        let balance = match state.app_db.credit_balance(&principal.user_id).await {
            Ok(balance) => balance,
            Err(err) => {
                tracing::error!(%err, user_id = %principal.user_id, "credit balance lookup failed");
                let _ = state.app_db.release_voice_reservation(&session_id).await;
                record(
                    &state,
                    LiveUsage {
                        key_id: Some(&principal.key_id),
                        user_id: Some(&principal.user_id),
                        plan_id: Some(&principal.plan),
                        model: &model,
                        status: StatusCode::INTERNAL_SERVER_ERROR.as_u16() as i32,
                        latency_ms: start.elapsed().as_millis() as i64,
                        error_code: Some("credit_balance_failed"),
                        cost_usd: 0.0,
                    },
                );
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": {
                            "code": "UPSTREAM_ERROR",
                            "message": "An error has occurred. Try again later.",
                        }
                    })),
                )
                    .into_response();
            }
        };
        let held_micros = max_session_micros.min(balance);
        if held_micros <= 0 {
            let _ = state.app_db.release_voice_reservation(&session_id).await;
            record(
                &state,
                LiveUsage {
                    key_id: Some(&principal.key_id),
                    user_id: Some(&principal.user_id),
                    plan_id: Some(&principal.plan),
                    model: &model,
                    status: StatusCode::PAYMENT_REQUIRED.as_u16() as i32,
                    latency_ms: start.elapsed().as_millis() as i64,
                    error_code: Some("credits_exhausted"),
                    cost_usd: 0.0,
                },
            );
            return (
                StatusCode::PAYMENT_REQUIRED,
                Json(serde_json::json!({
                    "error": {
                        "code": "credits_exhausted",
                        "message": "An error has occurred. Try again later.",
                    }
                })),
            )
                .into_response();
        }
        match super::reserve_credit_operation(&state, &principal.user_id, held_micros).await {
            Ok(reservation) => Some(reservation.id),
            Err(item) => {
                let _ = state.app_db.release_voice_reservation(&session_id).await;
                record(
                    &state,
                    LiveUsage {
                        key_id: Some(&principal.key_id),
                        user_id: Some(&principal.user_id),
                        plan_id: Some(&principal.plan),
                        model: &model,
                        status: item.status as i32,
                        latency_ms: start.elapsed().as_millis() as i64,
                        error_code: Some(item.code.as_str()),
                        cost_usd: 0.0,
                    },
                );
                return (
                    StatusCode::from_u16(item.status).unwrap_or(StatusCode::PAYMENT_REQUIRED),
                    Json(serde_json::json!({
                        "error": {"code": item.code.as_str(), "message": item.public_message()}
                    })),
                )
                    .into_response();
            }
        }
    } else {
        None
    };
    let gemini_key = state.settings.gemini_api_key.clone();

    ws.on_upgrade(move |socket| async move {
        session::handle_socket(
            socket,
            state.clone(),
            principal.key_id,
            principal.user_id,
            principal.plan,
            model,
            remaining_minutes,
            gemini_key,
            &session_id,
            credit_reservation_id,
        )
        .await;
        let _ = state.app_db.release_voice_reservation(&session_id).await;
    })
}
