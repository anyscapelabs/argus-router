//! The bidirectional Live proxy session: connects upstream to Google's
//! BidiGenerateContent WebSocket, forwards frames both ways, enforces the
//! plan-minute deadline, handles upstream 429 backoff, and records usage.

use std::time::{Duration, Instant};

use axum::extract::ws::Message;
use axum::extract::ws::WebSocket;
use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message as TungsteniteMessage;

use crate::state::AppState;

use super::frames::{extract_model_text, extract_rate_limit_secs};
use super::models::{LiveModelSpec, find_live_model};
use super::usage::{LiveUsage, record};

const GOOGLE_LIVE_WS_BASE: &str = "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1alpha.GenerativeService.BidiGenerateContent";

/// Max consecutive upstream 429s before closing the session.
const MAX_RATE_LIMIT_RETRIES: u32 = 3;

/// Google's Live endpoint names models in their wire form (`models/<id>`);
/// the catalog stores bare ids (`gemini-3.8-live`). The prefix is added here
/// for the setup frame only — billing and quotas keep the catalog id.
fn wire_model(id: &str) -> String {
    if id.starts_with("models/") {
        id.to_owned()
    } else {
        format!("models/{id}")
    }
}

/// Injects the router's resolved model id into a client `setup` frame. The
/// desktop opens `/v1/live` and sends a setup without naming a model; the
/// router is the only place that knows the configured model, so it fills the
/// gap before forwarding upstream. Non-setup frames pass through unchanged.
fn inject_default_model(text: &str, model: &str) -> String {
    let Ok(mut value) = serde_json::from_str::<serde_json::Value>(text) else {
        return text.to_owned();
    };
    if value.get("setup").is_none() {
        return text.to_owned();
    }
    if let Some(setup) = value.get_mut("setup").and_then(|s| s.as_object_mut()) {
        setup.insert("model".to_owned(), serde_json::Value::String(model.to_owned()));
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
        record(
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
        );
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
            record(
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
            );
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
    // First-frame diagnostics: the client either never sends audio (mic blocked
    // in the webview) or Google never answers — each gets one log line.
    let mut logged_first_upstream_frame = false;
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
                            // The desktop sends a setup frame without naming a
                            // model; the router resolves the configured model
                            // (the `?model` param, defaulting to the JSON entry
                            // flagged `default`) and injects it in Google's wire
                            // form, so the client never has to know a model id.
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
                    None => break, // client closed
                }
            }

            // ── upstream → client ────────────────────────────────────────────
            from_upstream = upstream_receiver.next() => {
                match from_upstream {
                    Some(Ok(TungsteniteMessage::Text(ref txt))) => {
                        let raw = txt.to_string();

                        // One line for the first thing Google ever sends back:
                        // if this never appears, the silence is upstream, not
                        // in the client. The snippet names the frame type
                        // (setupComplete, an error, audio…).
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
                        if matches!(msg, TungsteniteMessage::Binary(_)) && !logged_first_upstream_frame {
                            logged_first_upstream_frame = true;
                            tracing::info!(%user_id, "first frame received from Google Live is BINARY");
                        }
                        let axum_msg = match msg {
                            TungsteniteMessage::Binary(bin) => Message::Binary(bin),
                            TungsteniteMessage::Ping(p) => Message::Ping(p),
                            TungsteniteMessage::Pong(p) => Message::Pong(p),
                            TungsteniteMessage::Close(c) => {
                                // Google's own words for why it hung up — the
                                // only place a bad model id or key shows itself.
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
                        // No close frame — the socket just died (network, auth
                        // teardown). Worth a line: this is the arm a rejected
                        // session usually ends in.
                        tracing::info!(%user_id, "Google Live upstream closed without a close frame");
                        break;
                    } // upstream closed
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

    record_final_usage(
        &state,
        &key_id,
        &user_id,
        &plan_id,
        &model,
        start,
        quota_exhausted,
    );
}

/// Records the terminal usage row for a completed session, estimating cost
/// from the model's blended per-minute rate in `live_models.json`.
fn record_final_usage(
    state: &AppState,
    key_id: &str,
    user_id: &str,
    plan_id: &str,
    model: &str,
    start: Instant,
    quota_exhausted: bool,
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

    record(
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
    );
}
