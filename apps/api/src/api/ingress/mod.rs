pub mod gemini;
pub mod live;
pub mod openai;

use axum::http::HeaderMap;
use better_auth::plugins::api_key::VerifyApiKey;

use std::time::{Duration, Instant};

use crate::proxy::ErrCode;
use crate::proxy::Errx;
use crate::proxy::ir::{Block, Chat, Event};
use crate::services::app_db::{GenerationReservation, GenerationReservationOutcome, QuotaCheck};
use crate::services::catalog::ModelCatalogEntry;
use crate::services::usage::UsageRow;
use crate::state::AppState;

pub struct Principal {
    pub key_id: String,
    pub user_id: String,
    pub plan: String,
    pub allowlist: Vec<String>,
}

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

async fn authenticate_identity(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(String, String, String), Errx> {
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
    Ok((key_id, user_id, plan))
}

async fn authenticate(state: &AppState, headers: &HeaderMap) -> Result<Principal, Errx> {
    let (key_id, user_id, plan) = authenticate_identity(state, headers).await?;
    let allowlist = state
        .app_db
        .effective_allowlist_of(&user_id, &plan)
        .await
        .map_err(|_| Errx::new(ErrCode::Upstream, 500, "the allowlist lookup failed"))?;
    Ok(Principal {
        key_id,
        user_id,
        plan,
        allowlist,
    })
}

async fn authenticate_search(state: &AppState, headers: &HeaderMap) -> Result<Principal, Errx> {
    let (key_id, user_id, plan) = authenticate_identity(state, headers).await?;
    Ok(Principal {
        key_id,
        user_id,
        plan,
        allowlist: Vec::new(),
    })
}

pub(crate) async fn enforce_plan_quotas(
    state: &AppState,
    user_id: &str,
    plan: &str,
) -> Result<bool, Errx> {
    let db_error = |what: &str| Errx::new(ErrCode::Upstream, 500, format!("{what} failed"));
    let quota = state
        .app_db
        .quota_of(plan)
        .await
        .map_err(|_| db_error("the plan quota lookup"))?;

    let (used, limit) = state.rate_window.hit(user_id, quota.rate_per_min);
    if limit > 0 && used > limit {
        return Err(Errx::new(
            ErrCode::RateLimited,
            429,
            format!("plan limit is {limit} requests per minute"),
        )
        .with_retry_after(60));
    }

    let daily_exceeded = if let QuotaCheck::Exceeded { .. } = state
        .app_db
        .reserve_daily_request(user_id, plan)
        .await
        .map_err(|_| db_error("the daily request reservation"))?
    {
        true
    } else {
        false
    };
    let (monthly, weekly) = state
        .app_db
        .tokens_used(user_id)
        .await
        .map_err(|_| db_error("the token usage lookup"))?;
    if quota.monthly_tokens > 0 && monthly >= quota.monthly_tokens {
        return Err(limit_error(
            state,
            LimitWindow::Monthly,
            quota.monthly_tokens,
            monthly,
        )
        .await);
    }
    let weekly_exceeded = quota.weekly_tokens > 0 && weekly >= quota.weekly_tokens;
    if daily_exceeded || weekly_exceeded {
        let balance = state
            .app_db
            .credit_balance(user_id)
            .await
            .map_err(|_| db_error("the credit balance lookup"))?;
        if balance <= 0 {
            if weekly_exceeded && !daily_exceeded {
                return Err(limit_error(
                    state,
                    LimitWindow::Weekly,
                    quota.weekly_tokens,
                    weekly,
                )
                .await);
            }
            return Err(limit_error(
                state,
                LimitWindow::Daily,
                quota.daily_requests,
                quota.daily_requests,
            )
            .await);
        }
        return Ok(true);
    }
    Ok(false)
}

#[derive(Clone, Copy)]
enum LimitWindow {
    Daily,
    Weekly,
    Monthly,
}

impl LimitWindow {
    fn code(self) -> ErrCode {
        match self {
            LimitWindow::Daily => ErrCode::DailyLimitReached,
            LimitWindow::Weekly => ErrCode::WeeklyLimitReached,
            LimitWindow::Monthly => ErrCode::MonthlyLimitReached,
        }
    }
}

async fn limit_error(state: &AppState, window: LimitWindow, quota: i64, used: i64) -> Errx {
    let sql: &str = match window {
        LimitWindow::Daily => {
            "SELECT EXTRACT(EPOCH FROM (date_trunc('day', CURRENT_TIMESTAMP) + INTERVAL '1 day'))::bigint"
        }
        LimitWindow::Weekly => {
            "SELECT EXTRACT(EPOCH FROM (date_trunc('week', CURRENT_TIMESTAMP) + INTERVAL '1 week'))::bigint"
        }
        LimitWindow::Monthly => {
            "SELECT EXTRACT(EPOCH FROM (date_trunc('month', CURRENT_TIMESTAMP) + INTERVAL '1 month'))::bigint"
        }
    };
    let reset_epoch: Result<i64, _> = sqlx::query_scalar(sql)
        .fetch_one(&state.app_db.pool())
        .await;
    let reset = match reset_epoch {
        Ok(epoch) => chrono::DateTime::from_timestamp(epoch, 0)
            .unwrap_or_else(|| chrono::Utc::now() + chrono::Duration::hours(24)),
        Err(_) => {
            return Errx::new(
                window.code(),
                429,
                format!(
                    "{} Limit Reached ({used}/{quota}) — it resets {}",
                    window_label(window),
                    window_reset_hint(window),
                ),
            );
        }
    };
    Errx::new(window.code(), 429, limit_message(window, quota, used, reset))
        .with_retry_after(reset_seconds_left(reset))
}

fn window_label(window: LimitWindow) -> &'static str {
    match window {
        LimitWindow::Daily => "Daily",
        LimitWindow::Weekly => "Weekly",
        LimitWindow::Monthly => "Monthly",
    }
}

fn window_reset_hint(window: LimitWindow) -> &'static str {
    match window {
        LimitWindow::Daily => "at 00:00 UTC",
        LimitWindow::Weekly => "on Monday, 00:00 UTC",
        LimitWindow::Monthly => "on the 1st, 00:00 UTC",
    }
}

fn limit_message(
    window: LimitWindow,
    quota: i64,
    used: i64,
    reset: chrono::DateTime<chrono::Utc>,
) -> String {
    let reset_text = match window {
        LimitWindow::Daily => format!("Resets at {}", reset.format("%H:%M UTC")),
        _ => format!("Resets on {}", reset.format("%a, %d %b %Y at %H:%M UTC")),
    };
    let countdown = humanize_duration(reset_seconds_left(reset));
    let counter = if quota > 0 { format!(" ({used}/{quota})") } else { String::new() };
    format!(
        "{} Limit Reached — {reset_text} (in {countdown}){counter}",
        window_label(window)
    )
}

fn reset_seconds_left(reset: chrono::DateTime<chrono::Utc>) -> u64 {
    let now = chrono::Utc::now();
    reset
        .signed_duration_since(now)
        .num_seconds()
        .max(0) as u64
}

fn humanize_duration(total_seconds: u64) -> String {
    let days = total_seconds / 86_400;
    let hours = (total_seconds % 86_400) / 3_600;
    let minutes = (total_seconds % 3_600) / 60;
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

async fn reserve_generation(
    state: &AppState,
    user_id: &str,
    plan: &str,
    force_credits: bool,
    chat: &mut Chat,
    entry: &ModelCatalogEntry,
) -> Result<GenerationReservation, Errx> {
    let prompt_tokens = prompt_token_bound(chat, entry.context);
    let context_room = entry.context.saturating_sub(prompt_tokens);
    if context_room <= 0 {
        return Err(Errx::new(
            ErrCode::InvalidRequest,
            400,
            "the request is too large for the model context window",
        ));
    }
    let desired_output = chat
        .params
        .max_tokens
        .unwrap_or(entry.max_output.max(1) as u64)
        .min(context_room as u64)
        .max(1);
    let input_price = entry.pricing.input.parse::<f64>().unwrap_or(0.0);
    let output_price = entry.pricing.output.parse::<f64>().unwrap_or(0.0);
    let outcome = state
        .app_db
        .reserve_generation(
            user_id,
            plan,
            force_credits,
            desired_output,
            prompt_tokens,
            input_price,
            output_price,
        )
        .await
        .map_err(|_| Errx::new(ErrCode::Upstream, 500, "generation reservation failed"))?;
    match outcome {
        GenerationReservationOutcome::Reserved(reservation) => {
            chat.params.max_tokens = Some(reservation.max_output_tokens);
            Ok(reservation)
        }
        GenerationReservationOutcome::MonthlyExhausted => {
            Err(limit_error(state, LimitWindow::Monthly, 0, 0).await)
        }
        GenerationReservationOutcome::CreditsExhausted => Err(Errx::new(
            ErrCode::CreditsExhausted,
            402,
            "included usage is exhausted and no prepaid credits remain",
        )),
    }
}

async fn reserve_credit_operation(
    state: &AppState,
    user_id: &str,
    amount_micros: i64,
) -> Result<GenerationReservation, Errx> {
    let outcome = state
        .app_db
        .reserve_credit_operation(user_id, amount_micros)
        .await
        .map_err(|_| Errx::new(ErrCode::Upstream, 500, "credit reservation failed"))?;
    let reservation = match outcome {
        GenerationReservationOutcome::Reserved(reservation) => reservation,
        GenerationReservationOutcome::CreditsExhausted
        | GenerationReservationOutcome::MonthlyExhausted => {
            return Err(Errx::new(
                ErrCode::CreditsExhausted,
                402,
                "no prepaid credits remain for this request",
            ));
        }
    };
    Ok(reservation)
}

fn prompt_token_bound(chat: &Chat, context: i64) -> i64 {
    let mut bound = 256u64;
    for text in &chat.system {
        bound = bound.saturating_add(text.len() as u64);
    }
    for message in &chat.messages {
        bound = bound.saturating_add(16);
        for block in &message.blocks {
            let size = match block {
                Block::Text { text } | Block::Thinking { text } => text.len() as u64,
                Block::Image { .. } => 65_536,
                Block::ToolUse {
                    id,
                    name,
                    input_json,
                } => (id.len() + name.len() + input_json.len()) as u64,
                Block::ToolResult { id, content, .. } => (id.len() + content.len()) as u64,
            };
            bound = bound.saturating_add(size);
        }
    }
    for tool in &chat.tools {
        bound = bound
            .saturating_add(tool.name.len() as u64)
            .saturating_add(tool.description.as_deref().map(str::len).unwrap_or(0) as u64)
            .saturating_add(tool.schema_json.len() as u64)
            .saturating_add(32);
    }
    bound.min(context.max(1) as u64) as i64
}

async fn settle_generation_usage(
    usage: &crate::services::usage::UsageRecorder,
    reservation_id: &str,
    row: UsageRow,
) -> Result<(), sqlx::Error> {
    let mut last_error = None;
    for delay_ms in [0, 50, 200] {
        if delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }
        match usage.settle_generation(reservation_id, row.clone()).await {
            Ok(()) => return Ok(()),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or(sqlx::Error::RowNotFound))
}

fn shared_client() -> reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT
        .get_or_init(crate::proxy::adapters::build_client)
        .clone()
}

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
        .map(sanitize_model)
        .unwrap_or_else(|| "unknown".to_owned())
}

fn sanitize_model(model: &str) -> String {
    let cleaned: String = model.chars().filter(|c| !c.is_control()).collect();
    cleaned.chars().take(256).collect()
}

fn cost_usd(entry: &ModelCatalogEntry, input: i64, output: i64, cache_read: i64) -> f64 {
    let parse = |s: &str| s.parse::<f64>().unwrap_or(0.0);
    let cache_read = cache_read.clamp(0, input.max(0));
    let fresh_input = input.max(0) - cache_read;
    (fresh_input as f64 * parse(&entry.pricing.input)
        + output as f64 * parse(&entry.pricing.output)
        + cache_read as f64
            * parse(
                entry
                    .pricing
                    .cache_read
                    .as_deref()
                    .unwrap_or(&entry.pricing.input),
            ))
        / 1_000_000.0
}

pub async fn openai_chat_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let start = Instant::now();

    let principal = match authenticate(&state, &headers).await {
        Ok(principal) => principal,
        Err(item) => {
            state.usage.record(UsageRow {
                key_id: None,
                user_id: None,
                plan_id: None,
                model: "unknown".to_owned(),
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

    let payload: Value = match serde_json::from_slice(&body) {
        Ok(payload) => payload,
        Err(error) => {
            let item = Errx::new(
                ErrCode::InvalidRequest,
                400,
                format!("the request body is not valid JSON: {error}"),
            );
            return openai::error_response(item);
        }
    };
    let model_for_log = payload_model(&payload);

    let credits_paid = match enforce_plan_quotas(&state, &principal.user_id, &principal.plan).await
    {
        Ok(credits_paid) => credits_paid,
        Err(item) => {
            state.usage.record(UsageRow {
                key_id: Some(principal.key_id.clone()),
                user_id: Some(principal.user_id.clone()),
                plan_id: Some(principal.plan.clone()),
                model: model_for_log.clone(),
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
    let reservation = match reserve_generation(
        &state,
        &principal.user_id,
        &principal.plan,
        credits_paid,
        &mut chat,
        &resolved.entry,
    )
    .await
    {
        Ok(reservation) => reservation,
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
            return openai::error_response(item);
        }
    };

    let streaming = payload
        .get("stream")
        .and_then(|found| found.as_bool())
        .unwrap_or(false);
    let turn_state = openai::StreamState::new(model.clone());
    let client = shared_client();
    if let Err(error) = state
        .app_db
        .mark_generation_dispatched(&reservation.id)
        .await
    {
        tracing::error!(%error, reservation_id = reservation.id, "generation dispatch failed");
        let row = UsageRow {
            key_id: Some(principal.key_id.clone()),
            user_id: Some(principal.user_id.clone()),
            plan_id: Some(principal.plan.clone()),
            model: model.clone(),
            provider: Some(resolved.provider.id.to_owned()),
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cost_usd: 0.0,
            status: 500,
            latency_ms: Some(start.elapsed().as_millis() as i64),
            error_code: Some("generation_dispatch_failed".to_owned()),
        };
        let _ = settle_generation_usage(&state.usage, &reservation.id, row).await;
        return openai::error_response(Errx::new(
            ErrCode::Upstream,
            500,
            "generation dispatch failed",
        ));
    }

    if streaming {
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Errx>>(64);
        let key_id = principal.key_id.clone();
        let user_id = principal.user_id.clone();
        let plan = principal.plan.clone();
        let model_rec = model.clone();
        let provider_rec = resolved.provider.id.to_owned();
        let entry = resolved.entry.clone();
        let usage_rec = state.usage.clone();
        let reservation_id = reservation.id;
        let reserved_tokens = reservation.reserved_tokens;
        let held_micros = reservation.held_micros;
        let chat_for_task = chat;
        let resolved_for_task = resolved;
        tokio::spawn(async move {
            let mut input_tokens = 0i64;
            let mut output_tokens = 0i64;
            let mut cache_read_tokens = 0i64;
            let mut status = 200i32;
            let mut error_code: Option<String> = None;
            let mut usage_reported = false;
            let mut generated_output = false;
            let mut conservative_settlement = false;
            match service::stream_chat(&client, &resolved_for_task, chat_for_task).await {
                Ok(mut stream) => {
                    let mut client_gone = false;
                    loop {
                        let next = if client_gone {
                            match tokio::time::timeout(Duration::from_secs(120), stream.next())
                                .await
                            {
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
                                usage_reported = true;
                                input_tokens = *i;
                                output_tokens = *o;
                                cache_read_tokens = *c;
                            }
                            Err(err) => {
                                status = err.status as i32;
                                error_code = Some(err.code.as_str().to_owned());
                            }
                            Ok(
                                Event::TextDelta(_)
                                | Event::ThinkingDelta(_)
                                | Event::ToolUseStart { .. }
                                | Event::ToolUseDelta { .. },
                            ) => generated_output = true,
                            _ => {}
                        }
                        if !client_gone && tx.send(item).await.is_err() {
                            client_gone = true;
                            error_code.get_or_insert_with(|| "client_disconnected".to_owned());
                        }
                    }
                }
                Err(err) => {
                    status = err.status as i32;
                    error_code = Some(err.code.as_str().to_owned());
                    let _ = tx.send(Err(err)).await;
                }
            }
            if !usage_reported && (status == 200 || generated_output) {
                conservative_settlement = true;
                tracing::warn!(model = %model_rec, "streamed response finished without usage; settling the full reservation");
                input_tokens = reserved_tokens;
                output_tokens = 0;
                cache_read_tokens = 0;
                status = 502;
                error_code = Some("upstream_usage_missing".to_owned());
                let _ = tx
                    .send(Err(Errx::new(
                        ErrCode::Upstream,
                        502,
                        "the upstream provider omitted usage data",
                    )))
                    .await;
            }
            let latency = start.elapsed().as_millis() as i64;
            let cost = if conservative_settlement && held_micros > 0 {
                held_micros as f64 / 1_000_000.0
            } else {
                cost_usd(&entry, input_tokens, output_tokens, cache_read_tokens)
            };
            let row = UsageRow {
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
            };
            if let Err(error) = settle_generation_usage(&usage_rec, &reservation_id, row).await {
                tracing::error!(%error, reservation_id, "generation settlement failed");
                let _ = tx
                    .send(Err(Errx::new(
                        ErrCode::Upstream,
                        500,
                        "generation settlement failed",
                    )))
                    .await;
            }
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
                let missing_usage = !turn.usage_reported;
                let (input_tokens, output_tokens, cache_read_tokens, cost, status, error_code) =
                    if missing_usage {
                        (
                            reservation.reserved_tokens,
                            0,
                            0,
                            reservation.held_micros as f64 / 1_000_000.0,
                            502,
                            Some("upstream_usage_missing".to_owned()),
                        )
                    } else {
                        (
                            turn.usage.input_tokens,
                            turn.usage.output_tokens,
                            turn.usage.cache_read_tokens,
                            cost_usd(
                                &resolved.entry,
                                turn.usage.input_tokens,
                                turn.usage.output_tokens,
                                turn.usage.cache_read_tokens,
                            ),
                            200,
                            None,
                        )
                    };
                let row = UsageRow {
                    key_id: Some(principal.key_id.clone()),
                    user_id: Some(principal.user_id.clone()),
                    plan_id: Some(principal.plan.clone()),
                    model: model.clone(),
                    provider: Some(resolved.provider.id.to_owned()),
                    input_tokens,
                    output_tokens,
                    cache_read_tokens,
                    cost_usd: cost,
                    status,
                    latency_ms: Some(latency),
                    error_code,
                };
                if let Err(error) =
                    settle_generation_usage(&state.usage, &reservation.id, row).await
                {
                    tracing::error!(%error, reservation_id = reservation.id, "generation settlement failed");
                    return openai::error_response(Errx::new(
                        ErrCode::Upstream,
                        500,
                        "generation settlement failed",
                    ));
                }
                if missing_usage {
                    return openai::error_response(Errx::new(
                        ErrCode::Upstream,
                        502,
                        "the upstream provider omitted usage data",
                    ));
                }
                (StatusCode::OK, Json(openai::completion(&turn_state, &turn))).into_response()
            }
            Err(item) => {
                let row = UsageRow {
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
                };
                if let Err(error) =
                    settle_generation_usage(&state.usage, &reservation.id, row).await
                {
                    tracing::error!(%error, reservation_id = reservation.id, "generation settlement failed");
                }
                openai::error_response(item)
            }
        }
    }
}

pub async fn gemini_models_action(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(model_action): Path<String>,
    body: axum::body::Bytes,
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
    let payload: Value = match serde_json::from_slice(&body) {
        Ok(payload) => payload,
        Err(error) => {
            let item = Errx::new(
                ErrCode::InvalidRequest,
                400,
                format!("the request body is not valid JSON: {error}"),
            );
            return gemini::error_response(item);
        }
    };
    let credits_paid = match enforce_plan_quotas(&state, &principal.user_id, &principal.plan).await
    {
        Ok(credits_paid) => credits_paid,
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
    let reservation = match reserve_generation(
        &state,
        &principal.user_id,
        &principal.plan,
        credits_paid,
        &mut chat,
        &resolved.entry,
    )
    .await
    {
        Ok(reservation) => reservation,
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
            return gemini::error_response(item);
        }
    };

    let turn_state = gemini::StreamState::new(model.clone());
    let client = shared_client();
    if let Err(error) = state
        .app_db
        .mark_generation_dispatched(&reservation.id)
        .await
    {
        tracing::error!(%error, reservation_id = reservation.id, "generation dispatch failed");
        let row = UsageRow {
            key_id: Some(principal.key_id.clone()),
            user_id: Some(principal.user_id.clone()),
            plan_id: Some(principal.plan.clone()),
            model: model.clone(),
            provider: Some(resolved.provider.id.to_owned()),
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cost_usd: 0.0,
            status: 500,
            latency_ms: Some(start.elapsed().as_millis() as i64),
            error_code: Some("generation_dispatch_failed".to_owned()),
        };
        let _ = settle_generation_usage(&state.usage, &reservation.id, row).await;
        return gemini::error_response(Errx::new(
            ErrCode::Upstream,
            500,
            "generation dispatch failed",
        ));
    }

    if streaming {
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Errx>>(64);
        let key_id = principal.key_id.clone();
        let user_id = principal.user_id.clone();
        let plan = principal.plan.clone();
        let model_rec = model.clone();
        let provider_rec = resolved.provider.id.to_owned();
        let entry = resolved.entry.clone();
        let usage_rec = state.usage.clone();
        let reservation_id = reservation.id;
        let reserved_tokens = reservation.reserved_tokens;
        let held_micros = reservation.held_micros;
        let chat_for_task = chat;
        let resolved_for_task = resolved;
        tokio::spawn(async move {
            let mut input_tokens = 0i64;
            let mut output_tokens = 0i64;
            let mut cache_read_tokens = 0i64;
            let mut status = 200i32;
            let mut error_code: Option<String> = None;
            let mut usage_reported = false;
            let mut generated_output = false;
            let mut conservative_settlement = false;
            match service::stream_chat(&client, &resolved_for_task, chat_for_task).await {
                Ok(mut stream) => {
                    let mut client_gone = false;
                    loop {
                        let next = if client_gone {
                            match tokio::time::timeout(Duration::from_secs(120), stream.next())
                                .await
                            {
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
                                usage_reported = true;
                                input_tokens = *i;
                                output_tokens = *o;
                                cache_read_tokens = *c;
                            }
                            Err(err) => {
                                status = err.status as i32;
                                error_code = Some(err.code.as_str().to_owned());
                            }
                            Ok(
                                Event::TextDelta(_)
                                | Event::ThinkingDelta(_)
                                | Event::ToolUseStart { .. }
                                | Event::ToolUseDelta { .. },
                            ) => generated_output = true,
                            _ => {}
                        }
                        if !client_gone && tx.send(item).await.is_err() {
                            client_gone = true;
                            error_code.get_or_insert_with(|| "client_disconnected".to_owned());
                        }
                    }
                }
                Err(err) => {
                    status = err.status as i32;
                    error_code = Some(err.code.as_str().to_owned());
                    let _ = tx.send(Err(err)).await;
                }
            }
            if !usage_reported && (status == 200 || generated_output) {
                conservative_settlement = true;
                tracing::warn!(model = %model_rec, "streamed response finished without usage; settling the full reservation");
                input_tokens = reserved_tokens;
                output_tokens = 0;
                cache_read_tokens = 0;
                status = 502;
                error_code = Some("upstream_usage_missing".to_owned());
                let _ = tx
                    .send(Err(Errx::new(
                        ErrCode::Upstream,
                        502,
                        "the upstream provider omitted usage data",
                    )))
                    .await;
            }
            let latency = start.elapsed().as_millis() as i64;
            let cost = if conservative_settlement && held_micros > 0 {
                held_micros as f64 / 1_000_000.0
            } else {
                cost_usd(&entry, input_tokens, output_tokens, cache_read_tokens)
            };
            let row = UsageRow {
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
            };
            if let Err(error) = settle_generation_usage(&usage_rec, &reservation_id, row).await {
                tracing::error!(%error, reservation_id, "generation settlement failed");
                let _ = tx
                    .send(Err(Errx::new(
                        ErrCode::Upstream,
                        500,
                        "generation settlement failed",
                    )))
                    .await;
            }
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
                let missing_usage = !turn.usage_reported;
                let (input_tokens, output_tokens, cache_read_tokens, cost, status, error_code) =
                    if missing_usage {
                        (
                            reservation.reserved_tokens,
                            0,
                            0,
                            reservation.held_micros as f64 / 1_000_000.0,
                            502,
                            Some("upstream_usage_missing".to_owned()),
                        )
                    } else {
                        (
                            turn.usage.input_tokens,
                            turn.usage.output_tokens,
                            turn.usage.cache_read_tokens,
                            cost_usd(
                                &resolved.entry,
                                turn.usage.input_tokens,
                                turn.usage.output_tokens,
                                turn.usage.cache_read_tokens,
                            ),
                            200,
                            None,
                        )
                    };
                let row = UsageRow {
                    key_id: Some(principal.key_id.clone()),
                    user_id: Some(principal.user_id.clone()),
                    plan_id: Some(principal.plan.clone()),
                    model: model.clone(),
                    provider: Some(resolved.provider.id.to_owned()),
                    input_tokens,
                    output_tokens,
                    cache_read_tokens,
                    cost_usd: cost,
                    status,
                    latency_ms: Some(latency),
                    error_code,
                };
                if let Err(error) =
                    settle_generation_usage(&state.usage, &reservation.id, row).await
                {
                    tracing::error!(%error, reservation_id = reservation.id, "generation settlement failed");
                    return gemini::error_response(Errx::new(
                        ErrCode::Upstream,
                        500,
                        "generation settlement failed",
                    ));
                }
                if missing_usage {
                    return gemini::error_response(Errx::new(
                        ErrCode::Upstream,
                        502,
                        "the upstream provider omitted usage data",
                    ));
                }
                (StatusCode::OK, Json(gemini::completion(&turn_state, &turn))).into_response()
            }
            Err(item) => {
                let row = UsageRow {
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
                };
                if let Err(error) =
                    settle_generation_usage(&state.usage, &reservation.id, row).await
                {
                    tracing::error!(%error, reservation_id = reservation.id, "generation settlement failed");
                }
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

pub async fn search_endpoint(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let start = Instant::now();
    let principal = match authenticate_search(&state, &headers).await {
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
    let payload: crate::services::search::SearchRequest = match serde_json::from_slice(&body) {
        Ok(payload) => payload,
        Err(error) => {
            let item = Errx::new(
                ErrCode::InvalidRequest,
                400,
                format!("the request body is not valid JSON: {error}"),
            );
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

    let credits_paid = match enforce_plan_quotas(&state, &principal.user_id, &principal.plan).await
    {
        Ok(credits_paid) => credits_paid,
        Err(item) => {
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
    };

    match state
        .app_db
        .reserve_search_quota(&principal.user_id, &principal.plan)
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

    let credit_reservation = if credits_paid {
        let max_cost_micros = (state.search_router.max_cost_per_query(&payload.query) * 1_000_000.0)
            .ceil()
            .clamp(1.0, i64::MAX as f64) as i64;
        match reserve_credit_operation(&state, &principal.user_id, max_cost_micros).await {
            Ok(reservation) => {
                if state
                    .app_db
                    .mark_generation_dispatched(&reservation.id)
                    .await
                    .is_err()
                {
                    let _ = state.app_db.release_search_quota(&principal.user_id).await;
                    return openai::error_response(Errx::new(
                        ErrCode::Upstream,
                        500,
                        "search credit reservation failed",
                    ));
                }
                Some(reservation)
            }
            Err(item) => {
                let _ = state.app_db.release_search_quota(&principal.user_id).await;
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
    } else {
        None
    };

    let client = shared_client();
    let max_results = payload.clamped_max_results();
    match state
        .search_router
        .search(&client, &payload.query, max_results)
        .await
    {
        Ok((search_res, cost)) => {
            let latency = start.elapsed().as_millis() as i64;
            let row = UsageRow {
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
            };
            if let Some(reservation) = &credit_reservation {
                if let Err(error) =
                    settle_generation_usage(&state.usage, &reservation.id, row).await
                {
                    tracing::error!(%error, reservation_id = reservation.id, "search credit settlement failed");
                    return openai::error_response(Errx::new(
                        ErrCode::Upstream,
                        500,
                        "search credit settlement failed",
                    ));
                }
            } else {
                state.usage.record(row);
            }
            (StatusCode::OK, Json(search_res)).into_response()
        }
        Err(err) => {
            let latency = start.elapsed().as_millis() as i64;
            if let Err(release_error) = state.app_db.release_search_quota(&principal.user_id).await
            {
                tracing::error!(
                    error = %release_error,
                    user_id = %principal.user_id,
                    "failed to release unsuccessful search quota reservation"
                );
            }
            let row = UsageRow {
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
            };
            if let Some(reservation) = &credit_reservation {
                if let Err(error) =
                    settle_generation_usage(&state.usage, &reservation.id, row).await
                {
                    tracing::error!(%error, reservation_id = reservation.id, "search credit settlement failed");
                }
            } else {
                state.usage.record(row);
            }
            openai::error_response(err)
        }
    }
}
