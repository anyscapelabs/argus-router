//! Model-proxy ingress: the two public dialects (OpenAI-compatible and
//! Gemini-compatible) the desktop agent points its protocol adapters at.
//! Requests are authenticated with a service-account API key, validated
//! against the key's plan allowlist, normalized to IR, dispatched upstream
//! by echoing back the caller's dialect from the event stream adapter.

pub mod gemini;
pub mod openai;

use axum::http::HeaderMap;
use better_auth::plugins::api_key::VerifyApiKey;

use crate::proxy::ErrCode;
use crate::proxy::Errx;
use crate::state::AppState;

/// A paired service account: the key's user and its plan's model allowlist.
pub struct Principal {
    /// The key's owning user; surfaced for future per-user usage tracking.
    #[allow(dead_code)]
    pub user_id: String,
    pub allowlist: Vec<String>,
}

/// Bearer token (OpenAI style) or `x-api-key` (Gemini style / desktop).
fn presented_key(headers: &HeaderMap) -> Option<String> {
    let bearer = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if let Some(key) = bearer {
        return Some(key.to_owned());
    }
    headers
        .get("x-api-key")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// Verifies the presented key through the same ApiKeyPlugin the desktop's
/// domain routes use (quota + rate limiting included), then resolves the
/// plan/model allowlist for the owning user.
async fn authenticate(state: &AppState, headers: &HeaderMap) -> Result<Principal, Errx> {
    let key = presented_key(headers).ok_or_else(|| {
        Errx::new(
            ErrCode::Unauthorized,
            401,
            "a service-account API key is required (Authorization: Bearer or x-api-key)",
        )
    })?;
    let view = state
        .api_keys
        .verify_api_key(
            &VerifyApiKey {
                key: &key,
                config_id: None,
                permissions: None,
            },
            state.auth.context(),
        )
        .await
        .map_err(|_| {
            Errx::new(
                ErrCode::Unauthorized,
                401,
                "the API key is invalid or was revoked",
            )
        })?;
    let user_id = view
        .reference_id
        .typed()
        .ok()
        .cloned()
        .ok_or_else(|| {
            Errx::new(
                ErrCode::Upstream,
                500,
                "the API key lacks a user reference",
            )
        })?;
    let plan = state.app_db.plan_of(&user_id).await.map_err(|_| {
        Errx::new(ErrCode::Upstream, 500, "the plan lookup failed")
    })?;
    let allowlist = state.app_db.allowlist_of(&plan).await.map_err(|_| {
        Errx::new(ErrCode::Upstream, 500, "the allowlist lookup failed")
    })?;
    Ok(Principal {
        user_id,
        allowlist,
    })
}

fn shared_client() -> reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT
        .get_or_init(crate::proxy::adapters::build_client)
        .clone()
}

/// Collision-free-enough ids for streamed turns.
pub(crate) fn new_id(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_nanos())
        .unwrap_or(0);
    format!("{}-{:x}", prefix, nanos)
}

use std::pin::Pin;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event as SseFrame, KeepAlive, Sse};
use axum::response::{Json, IntoResponse, Response};
use axum::Json as AxumJson;
use futures_util::{Stream, StreamExt};
use serde_json::Value;

use crate::proxy::adapters::EventStream;
use crate::proxy::service;

/// `POST /v1/chat/completions` — OpenAI-compatible ingress.
pub async fn openai_chat_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumJson(payload): AxumJson<Value>,
) -> Response {
    let principal = match authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(item) => return openai::error_response(item),
    };
    let (model, mut chat) = match openai::parse(&payload) {
        Ok(found) => found,
        Err(item) => return openai::error_response(item),
    };
    let resolved = match service::resolve(&state.catalog, &model) {
        Ok(resolved) => resolved,
        Err(item) => return openai::error_response(item),
    };
    if let Err(item) = service::guard_entitlement(&resolved, &principal.allowlist) {
        return openai::error_response(item);
    }
    openai::clamp_params(&mut chat, resolved.entry.max_output);

    let streaming = payload
        .get("stream")
        .and_then(|found| found.as_bool())
        .unwrap_or(false);
    let turn_state = openai::StreamState::new(model.clone());
    let client = shared_client();

    if streaming {
        let events = match service::stream_chat(&client, &resolved, chat).await {
            Ok(events) => events,
            Err(item) => return openai::error_response(item),
        };
        sse_turn(events, turn_state).into_response()
    } else {
        match service::finish_chat(&client, &resolved, chat).await {
            Ok(turn) => (StatusCode::OK, Json(openai::completion(&turn_state, &turn)))
                .into_response(),
            Err(item) => openai::error_response(item),
        }
    }
}

/// `POST /v1beta/models/{model}:{action}` — Gemini-compatible ingress.
pub async fn gemini_models_action(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(model_action): Path<String>,
    AxumJson(payload): AxumJson<Value>,
) -> Response {
    let (model, streaming) = match gemini::split_path(&model_action) {
        Ok(found) => found,
        Err(item) => return gemini::error_response(item),
    };
    let principal = match authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(item) => return gemini::error_response(item),
    };
    let mut chat = match gemini::parse(&payload) {
        Ok(chat) => chat,
        Err(item) => return gemini::error_response(item),
    };
    let resolved = match service::resolve(&state.catalog, &model) {
        Ok(resolved) => resolved,
        Err(item) => return gemini::error_response(item),
    };
    if let Err(item) = service::guard_entitlement(&resolved, &principal.allowlist) {
        return gemini::error_response(item);
    }
    gemini::clamp_params(&mut chat, resolved.entry.max_output);

    let turn_state = gemini::StreamState::new(model.clone());
    let client = shared_client();

    if streaming {
        let events = match service::stream_chat(&client, &resolved, chat).await {
            Ok(events) => events,
            Err(item) => return gemini::error_response(item),
        };
        gemini_sse(events, turn_state).into_response()
    } else {
        match service::finish_chat(&client, &resolved, chat).await {
            Ok(turn) => {
                (StatusCode::OK, Json(gemini::completion(&turn_state, &turn))).into_response()
            }
            Err(item) => gemini::error_response(item),
        }
    }
}

fn sse_turn(events: EventStream, turn_state: openai::StreamState) -> Response {
    let frames = async_stream::stream! {
        let mut events = events;
        let mut turn_state = turn_state;
        while let Some(item) = events.next().await {
            match item {
                Ok(event) => {
                    if let Some(frame) = openai::chunk(&mut turn_state, &event) {
                        yield Ok(SseFrame::default().data(frame.to_string()));
                    }
                }
                Err(item) => {
                    yield Ok(SseFrame::default().data(openai::error_body(&item).to_string()));
                    return;
                }
            }
        }
        yield Ok(SseFrame::default().data(openai::final_chunk(&turn_state).to_string()));
        yield Ok(SseFrame::default().data("[DONE]"));
    };
    let frames: Pin<Box<dyn Stream<Item = Result<SseFrame, std::convert::Infallible>> + Send>> =
        Box::pin(frames);
    Sse::new(frames)
        .keep_alive(KeepAlive::default())
        .into_response()
}

fn gemini_sse(events: EventStream, turn_state: gemini::StreamState) -> Response {
    let frames = async_stream::stream! {
        let mut events = events;
        let mut turn_state = turn_state;
        while let Some(item) = events.next().await {
            match item {
                Ok(event) => {
                    if let Some(frame) = gemini::chunk(&mut turn_state, &event) {
                        yield Ok(SseFrame::default().data(frame.to_string()));
                    }
                }
                Err(item) => {
                    yield Ok(SseFrame::default().data(gemini::error_body(&item).to_string()));
                    return;
                }
            }
        }
        yield Ok(SseFrame::default().data(gemini::final_chunk(&turn_state, None).to_string()));
    };
    let frames: Pin<Box<dyn Stream<Item = Result<SseFrame, std::convert::Infallible>> + Send>> =
        Box::pin(frames);
    Sse::new(frames)
        .keep_alive(KeepAlive::default())
        .into_response()
}

/// `GET /v1/models` — OpenAI-style catalog listing for key-holders, filtered
/// to the key's plan allowlist.
pub async fn openai_models(State(state): State<AppState>, headers: HeaderMap) -> Response {
    match openai_models_view(&state, &headers).await {
        Ok(body) => Json(body).into_response(),
        Err(item) => openai::error_response(item),
    }
}

async fn openai_models_view(state: &AppState, headers: &HeaderMap) -> Result<Value, Errx> {
    let principal = authenticate(state, headers).await?;
    let entries: Vec<Value> = state
        .catalog
        .snapshot()
        .into_iter()
        .filter(|entry| principal.allowlist.iter().any(|allowed| allowed == entry.id()))
        .map(|entry| {
            serde_json::json!({
                "id": entry.id,
                "object": "model",
                "created": 0,
                "owned_by": entry.provider,
            })
        })
        .collect();
    Ok(openai::models_list(entries))
}

/// `GET /v1beta/models` — Gemini-style catalog listing for key-holders.
pub async fn gemini_models(State(state): State<AppState>, headers: HeaderMap) -> Response {
    match authenticate(&state, &headers).await {
        Ok(principal) => {
            let entries: Vec<Value> = state
                .catalog
                .snapshot()
                .into_iter()
                .filter(|entry| principal.allowlist.iter().any(|allowed| allowed == entry.id()))
                .map(|entry| {
                    serde_json::json!({
                        "name": format!("models/{}", entry.id),
                        "displayName": entry.name,
                        "ownedBy": entry.provider,
                    })
                })
                .collect();
            Json(gemini::models_list(entries)).into_response()
        }
        Err(item) => gemini::error_response(item),
    }
}

