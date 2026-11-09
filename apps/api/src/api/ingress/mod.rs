//! Model-proxy ingress: the two public dialects (OpenAI-compatible and
//! Gemini-compatible) the desktop agent points its protocol adapters at.
//! Requests are authenticated with a service-account API key, validated
//! against the key's plan allowlist, normalized to IR, dispatched upstream
//! by echoing back the caller's dialect from the event stream adapter.

pub mod gemini;
pub mod live;
pub mod openai;

use axum::http::HeaderMap;
use better_auth::plugins::api_key::VerifyApiKey;

use std::time::{Duration, Instant};

use crate::proxy::ErrCode;
use crate::proxy::Errx;
use crate::proxy::ir::Event;
use crate::services::catalog::ModelCatalogEntry;
use crate::services::usage::UsageRow;
use crate::state::AppState;

/// A paired service account: the key's user and its plan's model allowlist.
pub struct Principal {
    pub key_id: String,
    pub user_id: String,
    pub plan: String,
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
    let user_id =
        view.reference_id.typed().ok().cloned().ok_or_else(|| {
            Errx::new(ErrCode::Upstream, 500, "the API key lacks a user reference")
        })?;
    let key_id = view
        .id
        .typed()
        .cloned()
        .map_err(|_| Errx::new(ErrCode::Upstream, 500, "the API key lacks an id"))?;
    let plan = state
        .app_db
        .plan_of(&user_id)
        .await
        .map_err(|_| Errx::new(ErrCode::Upstream, 500, "the plan lookup failed"))?;
    let allowlist = state
        .app_db
        .allowlist_of(&plan)
        .await
        .map_err(|_| Errx::new(ErrCode::Upstream, 500, "the allowlist lookup failed"))?;
    Ok(Principal {
        key_id,
        user_id,
        plan,
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

use axum::Json as AxumJson;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event as SseFrame, KeepAlive, Sse};
use axum::response::{IntoResponse, Json, Response};
use futures_util::{Stream, StreamExt};
use serde_json::Value;

use crate::proxy::adapters::EventStream;
use crate::proxy::service;

fn payload_model(payload: &Value) -> String {
    payload
        .get("model")
        .and_then(|found| found.as_str())
        .filter(|found| !found.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| "unknown".to_owned())
}

fn cost_usd(entry: &ModelCatalogEntry, input: i64, output: i64, cache_read: i64) -> f64 {
    let parse = |s: &str| s.parse::<f64>().unwrap_or(0.0);
    // Upstream `prompt_tokens` already *includes* the cached tokens, so the
    // cached portion is billed at the cache-read rate only; charging the full
    // input at the input rate as well double-counts it.
    let cache_read = cache_read.clamp(0, input.max(0));
    let fresh_input = input.max(0) - cache_read;
    (fresh_input as f64 * parse(&entry.pricing.input)
        + output as f64 * parse(&entry.pricing.output)
        + cache_read as f64 * parse(entry.pricing.cache_read.as_deref().unwrap_or(&entry.pricing.input)))
        / 1_000_000.0
}

/// `POST /v1/chat/completions` — OpenAI-compatible ingress.
pub async fn openai_chat_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumJson(payload): AxumJson<Value>,
) -> Response {
    let start = Instant::now();
    let model_for_log = payload_model(&payload);

    let principal = match authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(item) => {
            state.usage.record(UsageRow {
                key_id: None,
                user_id: None,
                plan_id: None,
                model: model_for_log,
                provider: None,
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cost_usd: 0.0,
                status: item.status as i32,
                latency_ms: Some(start.elapsed().as_millis() as i64),
                error_code: Some(item.code.as_str().to_owned()),
            });
            return openai::error_response(item);
        }
    };

    let (model, mut chat) = match openai::parse(&payload) {
        Ok(found) => found,
        Err(item) => {
            state.usage.record(UsageRow {
                key_id: Some(principal.key_id.clone()),
                user_id: Some(principal.user_id.clone()),
                plan_id: Some(principal.plan.clone()),
                model: model_for_log,
                provider: None,
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cost_usd: 0.0,
                status: item.status as i32,
                latency_ms: Some(start.elapsed().as_millis() as i64),
                error_code: Some(item.code.as_str().to_owned()),
            });
            return openai::error_response(item);
        }
    };

    let resolved = match service::resolve(&state.catalog, &model) {
        Ok(resolved) => resolved,
        Err(item) => {
            state.usage.record(UsageRow {
                key_id: Some(principal.key_id.clone()),
                user_id: Some(principal.user_id.clone()),
                plan_id: Some(principal.plan.clone()),
                model: model.clone(),
                provider: None,
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cost_usd: 0.0,
                status: item.status as i32,
                latency_ms: Some(start.elapsed().as_millis() as i64),
                error_code: Some(item.code.as_str().to_owned()),
            });
            return openai::error_response(item);
        }
    };

    if let Err(item) = service::guard_entitlement(&resolved, &principal.allowlist) {
        state.usage.record(UsageRow {
            key_id: Some(principal.key_id.clone()),
            user_id: Some(principal.user_id.clone()),
            plan_id: Some(principal.plan.clone()),
            model: model.clone(),
            provider: Some(resolved.provider.id.to_owned()),
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cost_usd: 0.0,
            status: item.status as i32,
            latency_ms: Some(start.elapsed().as_millis() as i64),
            error_code: Some(item.code.as_str().to_owned()),
        });
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
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Errx>>(64);
        let key_id = principal.key_id.clone();
        let user_id = principal.user_id.clone();
        let plan = principal.plan.clone();
        let model_rec = model.clone();
        let provider_rec = resolved.provider.id.to_owned();
        let entry = resolved.entry.clone();
        let usage_rec = state.usage.clone();
        let chat_for_task = chat;
        let resolved_for_task = resolved;
        tokio::spawn(async move {
            let mut input_tokens = 0i64;
            let mut output_tokens = 0i64;
            let mut cache_read_tokens = 0i64;
            let mut status = 200i32;
            let mut error_code: Option<String> = None;
            match service::stream_chat(&client, &resolved_for_task, chat_for_task).await {
                Ok(mut stream) => {
                    let mut client_gone = false;
                    loop {
                        // Once the client is gone we keep draining the upstream
                        // purely to capture the final usage chunk (the provider
                        // bills the whole generation either way), bounded so an
                        // abandoned request cannot hang forever.
                        let next = if client_gone {
                            match tokio::time::timeout(Duration::from_secs(120), stream.next()).await {
                                Ok(next) => next,
                                Err(_) => break,
                            }
                        } else {
                            stream.next().await
                        };
                        let Some(item) = next else { break };
                        match &item {
                            Ok(Event::Usage {
                                input_tokens: i,
                                output_tokens: o,
                                cache_read_tokens: c,
                            }) => {
                                input_tokens = *i;
                                output_tokens = *o;
                                cache_read_tokens = *c;
                            }
                            Err(err) => {
                                status = err.status as i32;
                                error_code = Some(err.code.as_str().to_owned());
                            }
                            _ => {}
                        }
                        if !client_gone && tx.send(item).await.is_err() {
                            client_gone = true;
                            error_code.get_or_insert_with(|| "client_disconnected".to_owned());
                        }
                    }
                    if status == 200 && input_tokens == 0 && output_tokens == 0 {
                        tracing::warn!(model = %model_rec, "streamed response finished without a usage chunk; recording zero tokens");
                    }
                }
                Err(err) => {
                    status = err.status as i32;
                    error_code = Some(err.code.as_str().to_owned());
                    let _ = tx.send(Err(err)).await;
                }
            }
            let latency = start.elapsed().as_millis() as i64;
            let cost = cost_usd(&entry, input_tokens, output_tokens, cache_read_tokens);
            usage_rec.record(UsageRow {
                key_id: Some(key_id),
                user_id: Some(user_id),
                plan_id: Some(plan),
                model: model_rec,
                provider: Some(provider_rec),
                input_tokens,
                output_tokens,
                cache_read_tokens,
                cost_usd: cost,
                status,
                latency_ms: Some(latency),
                error_code,
            });
        });
        let events = async_stream::stream! {
            let mut rx = rx;
            while let Some(item) = rx.recv().await {
                yield item;
            }
        };
        sse_turn(Box::pin(events), turn_state).into_response()
    } else {
        match service::finish_chat(&client, &resolved, chat).await {
            Ok(turn) => {
                let latency = start.elapsed().as_millis() as i64;
                let cost = cost_usd(
                    &resolved.entry,
                    turn.usage.input_tokens,
                    turn.usage.output_tokens,
                    turn.usage.cache_read_tokens,
                );
                state.usage.record(UsageRow {
                    key_id: Some(principal.key_id.clone()),
                    user_id: Some(principal.user_id.clone()),
                    plan_id: Some(principal.plan.clone()),
                    model: model.clone(),
                    provider: Some(resolved.provider.id.to_owned()),
                    input_tokens: turn.usage.input_tokens,
                    output_tokens: turn.usage.output_tokens,
                    cache_read_tokens: turn.usage.cache_read_tokens,
                    cost_usd: cost,
                    status: 200,
                    latency_ms: Some(latency),
                    error_code: None,
                });
                (StatusCode::OK, Json(openai::completion(&turn_state, &turn))).into_response()
            }
            Err(item) => {
                state.usage.record(UsageRow {
                    key_id: Some(principal.key_id.clone()),
                    user_id: Some(principal.user_id.clone()),
                    plan_id: Some(principal.plan.clone()),
                    model: model.clone(),
                    provider: Some(resolved.provider.id.to_owned()),
                    input_tokens: 0,
                    output_tokens: 0,
                    cache_read_tokens: 0,
                    cost_usd: 0.0,
                    status: item.status as i32,
                    latency_ms: Some(start.elapsed().as_millis() as i64),
                    error_code: Some(item.code.as_str().to_owned()),
                });
                openai::error_response(item)
            }
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
    let start = Instant::now();
    let (model, streaming) = match gemini::split_path(&model_action) {
        Ok(found) => found,
        Err(item) => return gemini::error_response(item),
    };
    let principal = match authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(item) => {
            state.usage.record(UsageRow {
                key_id: None,
                user_id: None,
                plan_id: None,
                model: model.clone(),
                provider: None,
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cost_usd: 0.0,
                status: item.status as i32,
                latency_ms: Some(start.elapsed().as_millis() as i64),
                error_code: Some(item.code.as_str().to_owned()),
            });
            return gemini::error_response(item);
        }
    };
    let mut chat = match gemini::parse(&payload) {
        Ok(chat) => chat,
        Err(item) => {
            state.usage.record(UsageRow {
                key_id: Some(principal.key_id.clone()),
                user_id: Some(principal.user_id.clone()),
                plan_id: Some(principal.plan.clone()),
                model: model.clone(),
                provider: None,
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cost_usd: 0.0,
                status: item.status as i32,
                latency_ms: Some(start.elapsed().as_millis() as i64),
                error_code: Some(item.code.as_str().to_owned()),
            });
            return gemini::error_response(item);
        }
    };
    let resolved = match service::resolve(&state.catalog, &model) {
        Ok(resolved) => resolved,
        Err(item) => {
            state.usage.record(UsageRow {
                key_id: Some(principal.key_id.clone()),
                user_id: Some(principal.user_id.clone()),
                plan_id: Some(principal.plan.clone()),
                model: model.clone(),
                provider: None,
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cost_usd: 0.0,
                status: item.status as i32,
                latency_ms: Some(start.elapsed().as_millis() as i64),
                error_code: Some(item.code.as_str().to_owned()),
            });
            return gemini::error_response(item);
        }
    };
    if let Err(item) = service::guard_entitlement(&resolved, &principal.allowlist) {
        state.usage.record(UsageRow {
            key_id: Some(principal.key_id.clone()),
            user_id: Some(principal.user_id.clone()),
            plan_id: Some(principal.plan.clone()),
            model: model.clone(),
            provider: Some(resolved.provider.id.to_owned()),
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cost_usd: 0.0,
            status: item.status as i32,
            latency_ms: Some(start.elapsed().as_millis() as i64),
            error_code: Some(item.code.as_str().to_owned()),
        });
        return gemini::error_response(item);
    }
    gemini::clamp_params(&mut chat, resolved.entry.max_output);

    let turn_state = gemini::StreamState::new(model.clone());
    let client = shared_client();

    if streaming {
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Errx>>(64);
        let key_id = principal.key_id.clone();
        let user_id = principal.user_id.clone();
        let plan = principal.plan.clone();
        let model_rec = model.clone();
        let provider_rec = resolved.provider.id.to_owned();
        let entry = resolved.entry.clone();
        let usage_rec = state.usage.clone();
        let chat_for_task = chat;
        let resolved_for_task = resolved;
        tokio::spawn(async move {
            let mut input_tokens = 0i64;
            let mut output_tokens = 0i64;
            let mut cache_read_tokens = 0i64;
            let mut status = 200i32;
            let mut error_code: Option<String> = None;
            match service::stream_chat(&client, &resolved_for_task, chat_for_task).await {
                Ok(mut stream) => {
                    let mut client_gone = false;
                    loop {
                        // Once the client is gone we keep draining the upstream
                        // purely to capture the final usage chunk (the provider
                        // bills the whole generation either way), bounded so an
                        // abandoned request cannot hang forever.
                        let next = if client_gone {
                            match tokio::time::timeout(Duration::from_secs(120), stream.next()).await {
                                Ok(next) => next,
                                Err(_) => break,
                            }
                        } else {
                            stream.next().await
                        };
                        let Some(item) = next else { break };
                        match &item {
                            Ok(Event::Usage {
                                input_tokens: i,
                                output_tokens: o,
                                cache_read_tokens: c,
                            }) => {
                                input_tokens = *i;
                                output_tokens = *o;
                                cache_read_tokens = *c;
                            }
                            Err(err) => {
                                status = err.status as i32;
                                error_code = Some(err.code.as_str().to_owned());
                            }
                            _ => {}
                        }
                        if !client_gone && tx.send(item).await.is_err() {
                            client_gone = true;
                            error_code.get_or_insert_with(|| "client_disconnected".to_owned());
                        }
                    }
                    if status == 200 && input_tokens == 0 && output_tokens == 0 {
                        tracing::warn!(model = %model_rec, "streamed response finished without a usage chunk; recording zero tokens");
                    }
                }
                Err(err) => {
                    status = err.status as i32;
                    error_code = Some(err.code.as_str().to_owned());
                    let _ = tx.send(Err(err)).await;
                }
            }
            let latency = start.elapsed().as_millis() as i64;
            let cost = cost_usd(&entry, input_tokens, output_tokens, cache_read_tokens);
            usage_rec.record(UsageRow {
                key_id: Some(key_id),
                user_id: Some(user_id),
                plan_id: Some(plan),
                model: model_rec,
                provider: Some(provider_rec),
                input_tokens,
                output_tokens,
                cache_read_tokens,
                cost_usd: cost,
                status,
                latency_ms: Some(latency),
                error_code,
            });
        });
        let events = async_stream::stream! {
            let mut rx = rx;
            while let Some(item) = rx.recv().await {
                yield item;
            }
        };
        gemini_sse(Box::pin(events), turn_state).into_response()
    } else {
        match service::finish_chat(&client, &resolved, chat).await {
            Ok(turn) => {
                let latency = start.elapsed().as_millis() as i64;
                let cost = cost_usd(
                    &resolved.entry,
                    turn.usage.input_tokens,
                    turn.usage.output_tokens,
                    turn.usage.cache_read_tokens,
                );
                state.usage.record(UsageRow {
                    key_id: Some(principal.key_id.clone()),
                    user_id: Some(principal.user_id.clone()),
                    plan_id: Some(principal.plan.clone()),
                    model: model.clone(),
                    provider: Some(resolved.provider.id.to_owned()),
                    input_tokens: turn.usage.input_tokens,
                    output_tokens: turn.usage.output_tokens,
                    cache_read_tokens: turn.usage.cache_read_tokens,
                    cost_usd: cost,
                    status: 200,
                    latency_ms: Some(latency),
                    error_code: None,
                });
                (StatusCode::OK, Json(gemini::completion(&turn_state, &turn))).into_response()
            }
            Err(item) => {
                state.usage.record(UsageRow {
                    key_id: Some(principal.key_id.clone()),
                    user_id: Some(principal.user_id.clone()),
                    plan_id: Some(principal.plan.clone()),
                    model: model.clone(),
                    provider: Some(resolved.provider.id.to_owned()),
                    input_tokens: 0,
                    output_tokens: 0,
                    cache_read_tokens: 0,
                    cost_usd: 0.0,
                    status: item.status as i32,
                    latency_ms: Some(start.elapsed().as_millis() as i64),
                    error_code: Some(item.code.as_str().to_owned()),
                });
                gemini::error_response(item)
            }
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
        .filter(|entry| {
            principal
                .allowlist
                .iter()
                .any(|allowed| allowed == entry.id())
        })
        .map(|entry| {
            serde_json::json!({
                "id": entry.id,
                "object": "model",
                "created": 0,
                "owned_by": entry.provider,
                "display_name": entry.name,
                "name": entry.name,
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
                .filter(|entry| {
                    principal
                        .allowlist
                        .iter()
                        .any(|allowed| allowed == entry.id())
                })
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

/// Web search router ingress (`POST /v1/search` and `POST /api/v1/search`).
/// Authenticated via the same service-account API key.
/// Enforces monthly quotas per plan and records billing/usage.
pub async fn search_endpoint(
    State(state): State<AppState>,
    headers: HeaderMap,
    AxumJson(payload): AxumJson<crate::services::search::SearchRequest>,
) -> Response {
    let start = Instant::now();
    let principal = match authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(item) => {
            state.usage.record(UsageRow {
                key_id: None,
                user_id: None,
                plan_id: None,
                model: "web-search".to_owned(),
                provider: None,
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cost_usd: 0.0,
                status: item.status as i32,
                latency_ms: Some(start.elapsed().as_millis() as i64),
                error_code: Some(item.code.as_str().to_owned()),
            });
            return openai::error_response(item);
        }
    };

    if payload.query.trim().is_empty() {
        let item = Errx::new(
            ErrCode::InvalidRequest,
            400,
            "query parameter cannot be empty",
        );
        return openai::error_response(item);
    }

    // Verify and reserve against the monthly search quota atomically (advisory lock).
    // DB/read failures are surfaced as upstream errors, not as quota/plan decisions.
    match state
        .app_db
        .check_search_quota(&principal.user_id, &principal.plan)
        .await
    {
        Ok(crate::services::app_db::QuotaCheck::NotAvailable) => {
            let item = Errx::new(
                ErrCode::Forbidden,
                403,
                format!(
                    "web search is not available on your plan `{}`",
                    principal.plan
                ),
            );
            state.usage.record(UsageRow {
                key_id: Some(principal.key_id.clone()),
                user_id: Some(principal.user_id.clone()),
                plan_id: Some(principal.plan.clone()),
                model: "web-search".to_owned(),
                provider: None,
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cost_usd: 0.0,
                status: item.status as i32,
                latency_ms: Some(start.elapsed().as_millis() as i64),
                error_code: Some(item.code.as_str().to_owned()),
            });
            return openai::error_response(item);
        }
        Ok(crate::services::app_db::QuotaCheck::Exceeded { used, quota }) => {
            let item = Errx::new(
                ErrCode::RateLimited,
                429,
                format!(
                    "monthly web search quota exceeded for your plan `{}` ({}/{} used this month)",
                    principal.plan, used, quota
                ),
            );
            state.usage.record(UsageRow {
                key_id: Some(principal.key_id.clone()),
                user_id: Some(principal.user_id.clone()),
                plan_id: Some(principal.plan.clone()),
                model: "web-search".to_owned(),
                provider: None,
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cost_usd: 0.0,
                status: item.status as i32,
                latency_ms: Some(start.elapsed().as_millis() as i64),
                error_code: Some(item.code.as_str().to_owned()),
            });
            return openai::error_response(item);
        }
        Ok(crate::services::app_db::QuotaCheck::Allowed { .. }) => {}
        Err(err) => {
            tracing::error!(error = %err, user_id = %principal.user_id, "search quota check failed");
            let item = Errx::new(ErrCode::Upstream, 500, "the search quota lookup failed");
            state.usage.record(UsageRow {
                key_id: Some(principal.key_id.clone()),
                user_id: Some(principal.user_id.clone()),
                plan_id: Some(principal.plan.clone()),
                model: "web-search".to_owned(),
                provider: None,
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cost_usd: 0.0,
                status: item.status as i32,
                latency_ms: Some(start.elapsed().as_millis() as i64),
                error_code: Some(item.code.as_str().to_owned()),
            });
            return openai::error_response(item);
        }
    }

    let client = shared_client();
    let max_results = payload.clamped_max_results();
    match state
        .search_router
        .search(&client, &payload.query, max_results)
        .await
    {
        Ok(search_res) => {
            let latency = start.elapsed().as_millis() as i64;
            // Estimated cost per search call (~$0.003)
            let cost = 0.003;
            state.usage.record(UsageRow {
                key_id: Some(principal.key_id.clone()),
                user_id: Some(principal.user_id.clone()),
                plan_id: Some(principal.plan.clone()),
                model: "web-search".to_owned(),
                provider: Some(search_res.provider.clone()),
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cost_usd: cost,
                status: 200,
                latency_ms: Some(latency),
                error_code: None,
            });
            (StatusCode::OK, Json(search_res)).into_response()
        }
        Err(err) => {
            let latency = start.elapsed().as_millis() as i64;
            state.usage.record(UsageRow {
                key_id: Some(principal.key_id.clone()),
                user_id: Some(principal.user_id.clone()),
                plan_id: Some(principal.plan.clone()),
                model: "web-search".to_owned(),
                provider: None,
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cost_usd: 0.0,
                status: err.status as i32,
                latency_ms: Some(latency),
                error_code: Some(err.code.as_str().to_owned()),
            });
            openai::error_response(err)
        }
    }
}
