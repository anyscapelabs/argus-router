use std::time::{Duration, Instant};

use axum::extract::ws::Message;
use axum::extract::ws::WebSocket;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message as TungsteniteMessage;

use crate::state::AppState;

use super::frames::{extract_error_code, extract_model_text, extract_rate_limit_secs};
use super::models::{LiveModelSpec, find_live_model};
use super::usage::{LiveUsage, record_with_reservation};

const GOOGLE_LIVE_WS_BASE: &str = "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1alpha.GenerativeService.BidiGenerateContent";

const MAX_RATE_LIMIT_RETRIES: u32 = 3;

fn wire_model(id: &str) -> String {
    if id.starts_with("models/") {
        id.to_owned()
    } else {
        format!("models/{id}")
    }
}

fn inject_default_model(text: &str, model: &str) -> String {
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(text) else {
        return text.to_owned();
    };
    if value.get("setup").is_none() {
        return text.to_owned();
    }
    if let Some(setup) = value.get_mut("setup").and_then(|s| s.as_object_mut()) {
        setup.insert(
            "model".to_owned(),
            serde_json::Value::String(model.to_owned()),
        );
    }
    value.to_string()
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn handle_socket(
    client_ws: WebSocket,
    state: AppState,
    key_id: String,
    user_id: String,
    plan_id: String,
    model: String,
    remaining_minutes: i64,
    gemini_key: Option<String>,
    session_id: &str,
    credit_reservation_id: Option<String>,
) {
    let start = Instant::now();
    let max_duration = Duration::from_secs((remaining_minutes.max(1) as u64) * 60);

    if let Some(reservation_id) = credit_reservation_id.as_deref() {
        if let Err(error) = state
            .app_db
            .mark_generation_dispatched(reservation_id)
            .await
        {
            tracing::error!(%error, reservation_id, "live credit reservation dispatch failed");
            let (mut sender, _) = client_ws.split();
            let _ = sender
                .send(Message::Text(
                    serde_json::json!({
                        "error": {
                            "code": "UPSTREAM_ERROR",
                            "message": "An error has occurred. Try again later."
                        }
                    })
                    .to_string()
                    .into(),
                ))
                .await;
            let _ = sender.close().await;
            let _ = record_with_reservation(
                &state,
                LiveUsage {
                    key_id: Some(&key_id),
                    user_id: Some(&user_id),
                    plan_id: Some(&plan_id),
                    model: &model,
                    status: 500,
                    latency_ms: start.elapsed().as_millis() as i64,
                    error_code: Some("credit_dispatch_failed"),
                    cost_usd: 0.0,
                },
                credit_reservation_id.as_deref(),
            )
            .await;
            return;
        }
    }

    let Some(api_key) = gemini_key else {
        tracing::error!("GEMINI_API_KEY is not configured on the router");
        let (mut sender, _) = client_ws.split();
        let _ = sender
            .send(Message::Text(
                serde_json::json!({
                    "error": {
                        "code": "SERVICE_UNAVAILABLE",
                        "message": "An error has occurred. Try again later."
                    }
                })
                .to_string()
                .into(),
            ))
            .await;
        let _ = sender.close().await;
        let _ = record_with_reservation(
            &state,
            LiveUsage {
                key_id: Some(&key_id),
                user_id: Some(&user_id),
                plan_id: Some(&plan_id),
                model: &model,
                status: 503,
                latency_ms: start.elapsed().as_millis() as i64,
                error_code: Some("provider_unconfigured"),
                cost_usd: 0.0,
            },
            credit_reservation_id.as_deref(),
        )
        .await;
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
                            "message": "An error has occurred. Try again later."
                        }
                    })
                    .to_string()
                    .into(),
                ))
                .await;
            let _ = sender.close().await;
            let _ = record_with_reservation(
                &state,
                LiveUsage {
                    key_id: Some(&key_id),
                    user_id: Some(&user_id),
                    plan_id: Some(&plan_id),
                    model: &model,
                    status: 502,
                    latency_ms: start.elapsed().as_millis() as i64,
                    error_code: Some("upstream_error"),
                    cost_usd: 0.0,
                },
                credit_reservation_id.as_deref(),
            )
            .await;
            return;
        }
    };

    let (mut client_sender, mut client_receiver) = client_ws.split();
    let (mut upstream_sender, mut upstream_receiver) = upstream.split();

    let deadline = tokio::time::sleep(max_duration);
    tokio::pin!(deadline);

    let mut quota_exhausted = false;
    let mut last_model_text = String::new();
    let mut has_sent_audio = false;
    let mut logged_first_upstream_frame = false;
    let mut rate_limit_retries: u32 = 0;

    loop {
        tokio::select! {
            _ = &mut deadline => {
                tracing::info!(%user_id, "live call exceeded remaining plan minutes");
                quota_exhausted = true;
                let _ = client_sender
                    .send(Message::Text(
                        serde_json::json!({
                            "error": {
                                "code": "voice_minutes_exhausted",
                                "status": 429,
                                "message": "An error has occurred. Try again later."
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

            from_client = client_receiver.next() => {
                match from_client {
                    Some(Ok(msg)) => {
                        if let Message::Text(txt) = &msg {
                            if txt.contains("\"setup\"") {
                                tracing::info!(
                                    %user_id,
                                    setup_model = %wire_model(&model),
                                    "client setup forwarded upstream"
                                );
                            } else if txt.contains("realtimeInput") && !has_sent_audio {
                                has_sent_audio = true;
                                tracing::info!(%user_id, "client audio is flowing upstream");
                            }
                        }
                        let tung_msg = match msg {
                            Message::Text(txt) => {
                                let text = inject_default_model(txt.as_str(), &wire_model(&model));
                                TungsteniteMessage::Text(text.into())
                            }
                            Message::Binary(bin) => TungsteniteMessage::Binary(bin),
                            Message::Ping(p) => TungsteniteMessage::Ping(p),
                            Message::Pong(p) => TungsteniteMessage::Pong(p),
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
                    None => break,
                }
            }

            from_upstream = upstream_receiver.next() => {
                match from_upstream {
                    Some(Ok(TungsteniteMessage::Text(ref txt))) => {
                        let raw = txt.to_string();

                        if !logged_first_upstream_frame {
                            logged_first_upstream_frame = true;
                            let snippet: String = raw.chars().take(200).collect();
                            tracing::info!(
                                %user_id,
                                bytes = raw.len(),
                                snippet = %snippet,
                                "first frame received from Google Live"
                            );
                        }

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
                                            "message": "An error has occurred. Try again later."
                                        })
                                        .to_string()
                                        .into(),
                                    ))
                                    .await;
                                let _ = client_sender.close().await;
                                let _ = upstream_sender.close().await;
                                break;
                            }
                            let _ = client_sender
                                .send(Message::Text(
                                    serde_json::json!({
                                        "type": "rate_limited",
                                        "retry_after": retry_secs,
                                        "message": "An error has occurred. Try again later.".to_string()
                                    })
                                    .to_string()
                                    .into(),
                                ))
                                .await;
                            tokio::time::sleep(Duration::from_secs(retry_secs.min(60))).await;
                            continue;
                        }

                        if let Some(code) = extract_error_code(&raw) {
                            tracing::warn!(%user_id, code, "Google Live returned an upstream error");
                            let _ = client_sender
                                .send(Message::Text(
                                    serde_json::json!({
                                        "error": {
                                            "code": "UPSTREAM_ERROR",
                                            "message": "An error has occurred. Try again later."
                                        }
                                    })
                                    .to_string()
                                    .into(),
                                ))
                                .await;
                            break;
                        }

                        if let Some(text) = extract_model_text(&raw) {
                            last_model_text = text;
                            rate_limit_retries = 0;
                        }

                        if let Err(e) = client_sender
                            .send(Message::Text(raw.into()))
                            .await
                        {
                            tracing::warn!(%e, "error forwarding upstream message to client");
                            break;
                        }
                    }
                    Some(Ok(msg)) => {
                        if matches!(msg, TungsteniteMessage::Binary(_)) && !logged_first_upstream_frame {
                            logged_first_upstream_frame = true;
                            tracing::info!(%user_id, "first frame received from Google Live is BINARY");
                        }
                        let axum_msg = match msg {
                            TungsteniteMessage::Binary(bin) => Message::Binary(bin),
                            TungsteniteMessage::Ping(p) => Message::Ping(p),
                            TungsteniteMessage::Pong(p) => Message::Pong(p),
                            TungsteniteMessage::Close(c) => {
                                tracing::info!(
                                    %user_id,
                                    code = c.as_ref().map(|f| u16::from(f.code)),
                                    reason = c.as_ref().map(|f| f.reason.to_string()),
                                    "Google Live closed the upstream connection"
                                );
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
                    None => {
                        tracing::info!(%user_id, "Google Live upstream closed without a close frame");
                        break;
                    }
                }
            }
        }
    }

    if has_sent_audio && !last_model_text.is_empty() {
        state
            .live_sessions
            .store(session_id, &user_id, last_model_text);
        tracing::debug!(%session_id, "stored last-reply for voice→text fallback");
    }

    record_final_usage(
        &state,
        &key_id,
        &user_id,
        &plan_id,
        &model,
        start,
        quota_exhausted,
        credit_reservation_id.as_deref(),
    )
    .await;
}

async fn record_final_usage(
    state: &AppState,
    key_id: &str,
    user_id: &str,
    plan_id: &str,
    model: &str,
    start: Instant,
    quota_exhausted: bool,
    credit_reservation_id: Option<&str>,
) {
    let elapsed_ms = start.elapsed().as_millis() as i64;
    let cost_per_min = find_live_model(model)
        .map(LiveModelSpec::blended_cost_per_min)
        .unwrap_or(0.0);
    let elapsed_minutes = elapsed_ms as f64 / 60_000.0;
    let (status, error_code) = if quota_exhausted {
        (429, Some("voice_minutes_exhausted"))
    } else {
        (200, None)
    };

    if let Err(error) = record_with_reservation(
        state,
        LiveUsage {
            key_id: Some(key_id),
            user_id: Some(user_id),
            plan_id: Some(plan_id),
            model,
            status,
            latency_ms: elapsed_ms,
            error_code,
            cost_usd: elapsed_minutes * cost_per_min,
        },
        credit_reservation_id,
    )
    .await
    {
        tracing::error!(%error, ?credit_reservation_id, "live usage settlement failed");
    }
}
