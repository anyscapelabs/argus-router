//! Ingress: Live Speech-to-Speech WebSocket endpoint (`GET /v1/live`).
//! Authenticates via Bearer or x-api-key or ?key query param, checks monthly
//! voice_minutes quota against usage_logs, and bidirectionally proxies to Google's
//! Gemini Multimodal Live API using extended thinking models.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::time::Instant;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message as TungsteniteMessage;

use crate::api::ingress::authenticate;
use crate::services::usage::UsageRow;
use crate::state::AppState;

const GOOGLE_LIVE_WS_BASE: &str =
    "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1alpha.GenerativeService.BidiGenerateContent";

#[derive(Debug, Deserialize)]
pub struct LiveQuery {
    pub key: Option<String>,
    pub model: Option<String>,
}

/// Upgrades HTTP connection to WebSocket after checking Bearer token / API key
/// and remaining monthly voice minutes quota.
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
        .unwrap_or_else(|| "gemini-3.8-flash".to_owned());

    // If API key is provided via query parameter ?key=..., inject it into headers
    if !headers.contains_key("authorization") && !headers.contains_key("x-api-key") {
        if let Some(ref key) = query.key {
            if let Ok(val) = axum::http::HeaderValue::from_str(&format!("Bearer {}", key)) {
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
                    "error": {
                        "code": err.code.as_str(),
                        "message": err.message,
                    }
                })),
            )
                .into_response();
        }
    };

    // Check voice_minutes quota
    let allowed_minutes = match state.app_db.voice_minutes_of(&principal.plan).await {
        Ok(mins) => mins,
        Err(err) => {
            tracing::error!(%err, "failed to get plan voice_minutes");
            0
        }
    };

    let used_minutes = match state
        .app_db
        .voice_minutes_used_this_month(&principal.user_id)
        .await
    {
        Ok(mins) => mins,
        Err(err) => {
            tracing::error!(%err, "failed to get used voice_minutes");
            0
        }
    };

    if used_minutes >= allowed_minutes {
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
                    "voice_minutes_limit": allowed_minutes,
                    "voice_minutes_used": used_minutes,
                }
            })),
        )
            .into_response();
    }

    let remaining_minutes = allowed_minutes - used_minutes;
    let gemini_key = state.settings.gemini_api_key.clone();

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
        )
        .await;
    })
}

async fn handle_socket(
    client_ws: WebSocket,
    state: AppState,
    key_id: String,
    user_id: String,
    plan_id: String,
    model: String,
    remaining_minutes: i64,
    gemini_key: Option<String>,
) {
    let start = Instant::now();
    let max_duration = std::time::Duration::from_secs((remaining_minutes.max(1) * 60) as u64);

    let Some(api_key) = gemini_key else {
        tracing::error!("GEMINI_API_KEY is not configured on the router");
        let (mut client_sender, _) = client_ws.split();
        let error_msg = serde_json::json!({
            "error": {
                "code": "SERVICE_UNAVAILABLE",
                "message": "Gemini Live provider is unconfigured on router (missing GEMINI_API_KEY)"
            }
        });
        let _ = client_sender
            .send(Message::Text(error_msg.to_string().into()))
            .await;
        let _ = client_sender.close().await;

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

    // Target Google Live upstream URL
    let google_url = format!("{}?key={}", GOOGLE_LIVE_WS_BASE, api_key);

    let upstream = match connect_async(&google_url).await {
        Ok((stream, _resp)) => stream,
        Err(err) => {
            tracing::error!(%err, "failed to connect to upstream Google Live WebSocket");
            let (mut client_sender, _) = client_ws.split();
            let error_msg = serde_json::json!({
                "error": {
                    "code": "UPSTREAM_ERROR",
                    "message": format!("Failed to connect to Google Live WebSocket: {}", err)
                }
            });
            let _ = client_sender
                .send(Message::Text(error_msg.to_string().into()))
                .await;
            let _ = client_sender.close().await;

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

    loop {
        tokio::select! {
            _ = &mut deadline => {
                tracing::info!(%user_id, "live call exceeded remaining plan minutes");
                quota_exhausted = true;
                let exhaustion_msg = serde_json::json!({
                    "error": {
                        "code": "voice_minutes_exhausted",
                        "status": 429,
                        "message": "Monthly voice minutes quota exhausted."
                    }
                });
                let _ = client_sender.send(Message::Text(exhaustion_msg.to_string().into())).await;
                let _ = client_sender.close().await;
                let _ = upstream_sender.close().await;
                break;
            }

            from_client = client_receiver.next() => {
                match from_client {
                    Some(Ok(msg)) => {
                        let tung_msg = match msg {
                            Message::Text(txt) => TungsteniteMessage::Text(txt.as_str().into()),
                            Message::Binary(bin) => TungsteniteMessage::Binary(bin.into()),
                            Message::Ping(p) => TungsteniteMessage::Ping(p.into()),
                            Message::Pong(p) => TungsteniteMessage::Pong(p.into()),
                            Message::Close(c) => {
                                let tung_close = c.map(|frame| tokio_tungstenite::tungstenite::protocol::frame::CloseFrame {
                                    code: tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode::from(frame.code),
                                    reason: frame.reason.as_str().into(),
                                });
                                TungsteniteMessage::Close(tung_close)
                            }
                        };
                        if let Err(e) = upstream_sender.send(tung_msg).await {
                            tracing::warn!(%e, "error forwarding to upstream Google Live");
                            break;
                        }
                    }
                    Some(Err(err)) => {
                        tracing::debug!(%err, "client WebSocket read error");
                        break;
                    }
                    None => {
                        // Client closed connection
                        break;
                    }
                }
            }

            from_upstream = upstream_receiver.next() => {
                match from_upstream {
                    Some(Ok(msg)) => {
                        let axum_msg = match msg {
                            TungsteniteMessage::Text(txt) => Message::Text(txt.to_string().into()),
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
                        };
                        if let Err(e) = client_sender.send(axum_msg).await {
                            tracing::warn!(%e, "error forwarding to client");
                            break;
                        }
                    }
                    Some(Err(err)) => {
                        tracing::warn!(%err, "Google Live upstream read error");
                        break;
                    }
                    None => {
                        // Upstream closed connection
                        break;
                    }
                }
            }
        }
    }

    let elapsed_ms = start.elapsed().as_millis() as i64;
    let status = if quota_exhausted { 429 } else { 200 };
    let error_code = if quota_exhausted {
        Some("voice_minutes_exhausted".to_owned())
    } else {
        None
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
        cost_usd: 0.0,
        status,
        latency_ms: Some(elapsed_ms),
        error_code,
    });
}
