//! Ingress: Live Speech-to-Speech WebSocket endpoint (`GET /v1/live`).
//!
//! Authenticates via Bearer / x-api-key / `?key=` query param, checks monthly
//! voice_minutes quota, then bidirectionally proxies to Google's Gemini
//! Multimodal Live API (BidiGenerateContent).
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

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message as TungsteniteMessage;

use crate::api::ingress::authenticate;
use crate::services::usage::UsageRow;
use crate::state::AppState;

// ─── constants ───────────────────────────────────────────────────────────────

const GOOGLE_LIVE_WS_BASE: &str =
    "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1alpha.GenerativeService.BidiGenerateContent";

/// Default model for the Live endpoint.
/// Regular flash/pro models are NOT supported on BidiGenerateContent.
const DEFAULT_LIVE_MODEL: &str = "gemini-2.0-flash-live-001";

/// Max consecutive upstream 429s before closing the session.
const MAX_RATE_LIMIT_RETRIES: u32 = 3;

/// Default retry delay when Google does not send a `retryDelay` field.
const DEFAULT_RETRY_DELAY_SECS: u64 = 60;

/// TTL for entries in `LiveSessionStore` (2 minutes).
const LAST_REPLY_TTL_SECS: u64 = 120;

/// Gemini Live audio-input billing rate (USD per minute).
const LIVE_AUDIO_COST_PER_MIN_USD: f64 = 0.35;

// ─── LiveSessionStore ────────────────────────────────────────────────────────

struct LastReplyEntry {
    text: String,
    stored_at: SystemTime,
}

/// Short-lived in-memory store for voice-to-text fallback replies, keyed by
/// session id. Entries expire after `LAST_REPLY_TTL_SECS`.
#[derive(Default)]
pub struct LiveSessionStore {
    inner: Mutex<HashMap<String, LastReplyEntry>>,
}

impl LiveSessionStore {
    /// Stores the last model text for `session_id`, evicting expired entries.
    pub fn store(&self, session_id: &str, text: String) {
        let mut map = self.inner.lock().expect("LiveSessionStore lock poisoned");
        // Opportunistic TTL eviction.
        map.retain(|_, v| {
            v.stored_at
                .elapsed()
                .map(|e| e.as_secs() < LAST_REPLY_TTL_SECS)
                .unwrap_or(false)
        });
        map.insert(
            session_id.to_owned(),
            LastReplyEntry {
                text,
                stored_at: SystemTime::now(),
            },
        );
    }

    /// Takes (removes and returns) the stored reply, returning `None` if absent
    /// or expired.
    pub fn take(&self, session_id: &str) -> Option<String> {
        let mut map = self.inner.lock().expect("LiveSessionStore lock poisoned");
        let entry = map.remove(session_id)?;
        if entry
            .stored_at
            .elapsed()
            .map(|e| e.as_secs())
            .unwrap_or(u64::MAX)
            >= LAST_REPLY_TTL_SECS
        {
            return None; // expired
        }
        Some(entry.text)
    }
}

// ─── helpers ─────────────────────────────────────────────────────────────────

/// Generates a simple session id from the current nanosecond timestamp.
/// Not cryptographically random but collision-free enough for short-lived
/// in-process keys.
fn new_session_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("live-{nanos:032x}")
}

/// Extracts text from an upstream `BidiGenerateContentServerContent` frame.
///
/// Google's shape:
/// ```json
/// { "serverContent": { "modelTurn": { "parts": [{"text": "..."}] } } }
/// ```
fn extract_model_text(raw: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let parts = v.pointer("/serverContent/modelTurn/parts")?.as_array()?;
    let text: String = parts
        .iter()
        .filter_map(|p| p.get("text")?.as_str())
        .collect();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

/// Returns the retry-after delay in seconds when the upstream frame is a 429
/// error envelope, otherwise `None`.
fn extract_rate_limit_secs(raw: &str) -> Option<u64> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let code = v.pointer("/error/code")?.as_u64()?;
    if code != 429 {
        return None;
    }
    // Try to parse `retryDelay` from the error details array.
    let retry = v
        .pointer("/error/details")
        .and_then(|d| d.as_array())
        .and_then(|arr| {
            arr.iter().find_map(|item| {
                item.get("retryDelay")
                    .and_then(|s| s.as_str())
                    // Google sends e.g. "60s"
                    .and_then(|s| s.trim_end_matches('s').parse::<u64>().ok())
            })
        })
        .unwrap_or(DEFAULT_RETRY_DELAY_SECS);
    Some(retry)
}

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

// ─── GET /v1/live/last-reply ─────────────────────────────────────────────────

/// Returns the last model text reply stored for a session that ended early.
/// Used by the desktop app for voice→text fallback.
pub async fn last_reply_handler(
    State(state): State<AppState>,
    mut headers: HeaderMap,
    Query(query): Query<LastReplyQuery>,
) -> Response {
    // Mirror the same key injection as the live handler.
    if !headers.contains_key("authorization") && !headers.contains_key("x-api-key") {
        if let Some(ref key) = query.key {
            if let Ok(val) = axum::http::HeaderValue::from_str(&format!("Bearer {key}")) {
                headers.insert(axum::http::header::AUTHORIZATION, val);
            }
        }
    }
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
        .unwrap_or_else(|| DEFAULT_LIVE_MODEL.to_owned());

    // Allow API key via ?key= query param (WS clients can't set custom headers).
    if !headers.contains_key("authorization") && !headers.contains_key("x-api-key") {
        if let Some(ref key) = query.key {
            if let Ok(val) = axum::http::HeaderValue::from_str(&format!("Bearer {key}")) {
                headers.insert(axum::http::header::AUTHORIZATION, val);
            }
        }
    }

    let principal = match authenticate(&state, &headers).await {
        Ok(p) => p,
        Err(err) => {
            state.usage.record(UsageRow {
                key_id: None,
                user_id: None,
                plan_id: None,
                model: model.clone(),
                provider: Some("gemini-live".to_owned()),
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cost_usd: 0.0,
                status: err.status as i32,
                latency_ms: Some(start.elapsed().as_millis() as i64),
                error_code: Some(err.code.as_str().to_owned()),
            });
            return (
                StatusCode::from_u16(err.status).unwrap_or(StatusCode::UNAUTHORIZED),
                axum::Json(serde_json::json!({
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
            state.usage.record(UsageRow {
                key_id: Some(principal.key_id.clone()),
                user_id: Some(principal.user_id.clone()),
                plan_id: Some(principal.plan.clone()),
                model: model.clone(),
                provider: Some("gemini-live".to_owned()),
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cost_usd: 0.0,
                status: StatusCode::FORBIDDEN.as_u16() as i32,
                latency_ms: Some(start.elapsed().as_millis() as i64),
                error_code: Some("voice_not_available".to_owned()),
            });
            return (
                StatusCode::FORBIDDEN,
                axum::Json(serde_json::json!({
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
            state.usage.record(UsageRow {
                key_id: Some(principal.key_id.clone()),
                user_id: Some(principal.user_id.clone()),
                plan_id: Some(principal.plan.clone()),
                model: model.clone(),
                provider: Some("gemini-live".to_owned()),
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cost_usd: 0.0,
                status: StatusCode::TOO_MANY_REQUESTS.as_u16() as i32,
                latency_ms: Some(start.elapsed().as_millis() as i64),
                error_code: Some("voice_minutes_exhausted".to_owned()),
            });
            return (
                StatusCode::TOO_MANY_REQUESTS,
                axum::Json(serde_json::json!({
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
            state.usage.record(UsageRow {
                key_id: Some(principal.key_id.clone()),
                user_id: Some(principal.user_id.clone()),
                plan_id: Some(principal.plan.clone()),
                model: model.clone(),
                provider: Some("gemini-live".to_owned()),
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cost_usd: 0.0,
                status: StatusCode::INTERNAL_SERVER_ERROR.as_u16() as i32,
                latency_ms: Some(start.elapsed().as_millis() as i64),
                error_code: Some("quota_check_failed".to_owned()),
            });
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(serde_json::json!({
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
        handle_socket(
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

// ─── WebSocket handler ───────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
async fn handle_socket(
    client_ws: WebSocket,
    state: AppState,
    key_id: String,
    user_id: String,
    plan_id: String,
    model: String,
    remaining_minutes: i64,
    gemini_key: Option<String>,
    session_id: String,
) {
    let start = Instant::now();
    // Cap session duration to the user's remaining plan minutes (minimum 1 minute).
    let max_duration = Duration::from_secs((remaining_minutes.max(1) as u64) * 60);

    let Some(api_key) = gemini_key else {
        tracing::error!("GEMINI_API_KEY is not configured on the router");
        let (mut sender, _) = client_ws.split();
        let _ = sender
            .send(Message::Text(
                serde_json::json!({
                    "error": {
                        "code": "SERVICE_UNAVAILABLE",
                        "message": "Gemini Live provider is unconfigured on router (missing GEMINI_API_KEY)"
                    }
                })
                .to_string()
                .into(),
            ))
            .await;
        let _ = sender.close().await;
        state.usage.record(UsageRow {
            key_id: Some(key_id),
            user_id: Some(user_id),
            plan_id: Some(plan_id),
            model,
            provider: Some("gemini-live".to_owned()),
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cost_usd: 0.0,
            status: 503,
            latency_ms: Some(start.elapsed().as_millis() as i64),
            error_code: Some("provider_unconfigured".to_owned()),
        });
        return;
    };

    let google_url = format!("{GOOGLE_LIVE_WS_BASE}?key={api_key}");
    let upstream = match connect_async(&google_url).await {
        Ok((stream, _)) => stream,
        Err(err) => {
            tracing::error!(%err, "failed to connect to upstream Google Live WebSocket");
            let (mut sender, _) = client_ws.split();
            let _ = sender
                .send(Message::Text(
                    serde_json::json!({
                        "error": {
                            "code": "UPSTREAM_ERROR",
                            "message": format!("Failed to connect to Google Live WebSocket: {err}")
                        }
                    })
                    .to_string()
                    .into(),
                ))
                .await;
            let _ = sender.close().await;
            state.usage.record(UsageRow {
                key_id: Some(key_id),
                user_id: Some(user_id),
                plan_id: Some(plan_id),
                model,
                provider: Some("gemini-live".to_owned()),
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cost_usd: 0.0,
                status: 502,
                latency_ms: Some(start.elapsed().as_millis() as i64),
                error_code: Some("upstream_error".to_owned()),
            });
            return;
        }
    };

    let (mut client_sender, mut client_receiver) = client_ws.split();
    let (mut upstream_sender, mut upstream_receiver) = upstream.split();

    let deadline = tokio::time::sleep(max_duration);
    tokio::pin!(deadline);

    let mut quota_exhausted = false;
    // Tracks the most recent accumulated model text for voice→text fallback.
    let mut last_model_text = String::new();
    // Set once the client sends at least one audio chunk.
    let mut has_sent_audio = false;
    // Consecutive upstream 429 counter.
    let mut rate_limit_retries: u32 = 0;

    loop {
        tokio::select! {
            // ── quota deadline ───────────────────────────────────────────────
            _ = &mut deadline => {
                tracing::info!(%user_id, "live call exceeded remaining plan minutes");
                quota_exhausted = true;
                let _ = client_sender
                    .send(Message::Text(
                        serde_json::json!({
                            "error": {
                                "code": "voice_minutes_exhausted",
                                "status": 429,
                                "message": "Monthly voice minutes quota exhausted."
                            }
                        })
                        .to_string()
                        .into(),
                    ))
                    .await;
                let _ = client_sender.close().await;
                let _ = upstream_sender.close().await;
                break;
            }

            // ── client → upstream ────────────────────────────────────────────
            from_client = client_receiver.next() => {
                match from_client {
                    Some(Ok(msg)) => {
                        // Detect audio input so we know to offer a text fallback later.
                        if let Message::Text(ref txt) = msg {
                            if txt.contains("realtimeInput") && txt.contains("audio") {
                                has_sent_audio = true;
                            }
                        }
                        let tung_msg = match msg {
                            Message::Text(txt) => TungsteniteMessage::Text(txt.as_str().into()),
                            Message::Binary(bin) => TungsteniteMessage::Binary(bin.into()),
                            Message::Ping(p) => TungsteniteMessage::Ping(p.into()),
                            Message::Pong(p) => TungsteniteMessage::Pong(p.into()),
                            Message::Close(c) => {
                                let tung_close = c.map(|frame| {
                                    tokio_tungstenite::tungstenite::protocol::frame::CloseFrame {
                                        code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::from(frame.code),
                                        reason: frame.reason.as_str().into(),
                                    }
                                });
                                TungsteniteMessage::Close(tung_close)
                            }
                        };
                        if let Err(e) = upstream_sender.send(tung_msg).await {
                            tracing::warn!(%e, "error forwarding client message to upstream");
                            break;
                        }
                    }
                    Some(Err(err)) => {
                        tracing::debug!(%err, "client WebSocket read error");
                        break;
                    }
                    None => break, // client closed
                }
            }

            // ── upstream → client ────────────────────────────────────────────
            from_upstream = upstream_receiver.next() => {
                match from_upstream {
                    Some(Ok(TungsteniteMessage::Text(ref txt))) => {
                        let raw = txt.to_string();

                        // ── rate-limit detection ─────────────────────────────
                        if let Some(retry_secs) = extract_rate_limit_secs(&raw) {
                            rate_limit_retries += 1;
                            tracing::warn!(
                                %user_id,
                                retries = rate_limit_retries,
                                retry_secs,
                                "Google Live 429 rate-limit received"
                            );
                            if rate_limit_retries >= MAX_RATE_LIMIT_RETRIES {
                                let _ = client_sender
                                    .send(Message::Text(
                                        serde_json::json!({
                                            "type": "rate_limit_exceeded",
                                            "message": "Google Live token rate limit hit too many times; session closed."
                                        })
                                        .to_string()
                                        .into(),
                                    ))
                                    .await;
                                let _ = client_sender.close().await;
                                let _ = upstream_sender.close().await;
                                break;
                            }
                            // Notify client of the pause and sleep.
                            let _ = client_sender
                                .send(Message::Text(
                                    serde_json::json!({
                                        "type": "rate_limited",
                                        "retry_after": retry_secs,
                                        "message": format!("Rate limited by Google; resuming in {retry_secs}s")
                                    })
                                    .to_string()
                                    .into(),
                                ))
                                .await;
                            tokio::time::sleep(Duration::from_secs(retry_secs.min(60))).await;
                            continue;
                        }

                        // ── accumulate model text for fallback ───────────────
                        if let Some(text) = extract_model_text(&raw) {
                            last_model_text = text;
                            rate_limit_retries = 0; // successful reply resets retry counter
                        }

                        // Forward to client.
                        if let Err(e) = client_sender
                            .send(Message::Text(raw.into()))
                            .await
                        {
                            tracing::warn!(%e, "error forwarding upstream message to client");
                            break;
                        }
                    }
                    Some(Ok(msg)) => {
                        // Binary / Ping / Pong / Close frames — convert and forward.
                        let axum_msg = match msg {
                            TungsteniteMessage::Binary(bin) => Message::Binary(bin.into()),
                            TungsteniteMessage::Ping(p) => Message::Ping(p.into()),
                            TungsteniteMessage::Pong(p) => Message::Pong(p.into()),
                            TungsteniteMessage::Close(c) => {
                                let axum_close = c.map(|frame| axum::extract::ws::CloseFrame {
                                    code: frame.code.into(),
                                    reason: frame.reason.to_string().into(),
                                });
                                Message::Close(axum_close)
                            }
                            TungsteniteMessage::Frame(_) => continue,
                            TungsteniteMessage::Text(_) => unreachable!(),
                        };
                        if let Err(e) = client_sender.send(axum_msg).await {
                            tracing::warn!(%e, "error forwarding upstream message to client");
                            break;
                        }
                    }
                    Some(Err(err)) => {
                        tracing::warn!(%err, "Google Live upstream read error");
                        break;
                    }
                    None => break, // upstream closed
                }
            }
        }
    }

    // ── voice→text fallback ──────────────────────────────────────────────────
    // If the client sent audio and we have partial model text, store it so the
    // desktop app can retrieve it via GET /v1/live/last-reply.
    if has_sent_audio && !last_model_text.is_empty() {
        state.live_sessions.store(&session_id, last_model_text);
        tracing::debug!(%session_id, "stored last-reply for voice→text fallback");
    }

    // ── usage recording ──────────────────────────────────────────────────────
    let elapsed_ms = start.elapsed().as_millis() as i64;
    // Estimate cost: audio input rate × elapsed minutes.
    let elapsed_minutes = elapsed_ms as f64 / 60_000.0;
    let cost_usd = elapsed_minutes * LIVE_AUDIO_COST_PER_MIN_USD;

    let (status, error_code) = if quota_exhausted {
        (429, Some("voice_minutes_exhausted".to_owned()))
    } else {
        (200, None)
    };

    state.usage.record(UsageRow {
        key_id: Some(key_id),
        user_id: Some(user_id),
        plan_id: Some(plan_id),
        model,
        provider: Some("gemini-live".to_owned()),
        input_tokens: 0,
        output_tokens: 0,
        cache_read_tokens: 0,
        cost_usd,
        status,
        latency_ms: Some(elapsed_ms),
        error_code,
    });
}
