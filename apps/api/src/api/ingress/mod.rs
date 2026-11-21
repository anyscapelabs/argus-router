pub mod live;
pub mod openai;

use axum::http::HeaderMap;
use better_auth::plugins::api_key::VerifyApiKey;

use std::time::{Duration, Instant};

use crate::error::{ErrCode, Errx};
use crate::services::app_db::{GenerationReservation, GenerationReservationOutcome, QuotaCheck};
use crate::services::catalog::{AUTO_MODEL_ID, ModelCatalogEntry, Tier};
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
) -> Result<(), Errx> {
    let db_error = |what: &str| Errx::new(ErrCode::Upstream, 500, format!("{what} failed"));
    let quota = state
        .app_db
        .quota_of(plan)
        .await
        .map_err(|_| db_error("the plan quota lookup"))?;

    if let QuotaCheck::Exceeded { used, quota } = state
        .app_db
        .reserve_daily_request(user_id, plan)
        .await
        .map_err(|_| db_error("the daily request reservation"))?
    {
        return Err(limit_error(state, LimitWindow::Daily, quota, used).await);
    }
    let (monthly, weekly) = state
        .app_db
        .tokens_used(user_id)
        .await
        .map_err(|_| db_error("the token usage lookup"))?;
    if quota.monthly_tokens > 0 && monthly >= quota.monthly_tokens {
        return Err(limit_error(state, LimitWindow::Monthly, quota.monthly_tokens, monthly).await);
    }
    if quota.weekly_tokens > 0 && weekly >= quota.weekly_tokens {
        return Err(limit_error(state, LimitWindow::Weekly, quota.weekly_tokens, weekly).await);
    }
    Ok(())
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
                    "{} Limit has reached. Try again later.",
                    window_label(window),
                ),
            );
        }
    };
    Errx::new(
        window.code(),
        429,
        limit_message(window, quota, used, reset),
    )
    .with_retry_after(reset_seconds_left(reset))
}

fn window_label(window: LimitWindow) -> &'static str {
    match window {
        LimitWindow::Daily => "Daily",
        LimitWindow::Weekly => "Weekly",
        LimitWindow::Monthly => "Monthly",
    }
}

fn limit_message(
    window: LimitWindow,
    _quota: i64,
    _used: i64,
    reset: chrono::DateTime<chrono::Utc>,
) -> String {
    let countdown = humanize_duration(reset_seconds_left(reset));
    format!(
        "{} Limit has reached. Try again in {countdown}",
        window_label(window)
    )
}

fn reset_seconds_left(reset: chrono::DateTime<chrono::Utc>) -> u64 {
    let now = chrono::Utc::now();
    reset.signed_duration_since(now).num_seconds().max(0) as u64
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

async fn reserve_gateway_generation(
    state: &AppState,
    user_id: &str,
    plan: &str,
    payload: &Value,
    body_len: usize,
    entry: &ModelCatalogEntry,
) -> Result<GenerationReservation, Errx> {
    // This bound is intentionally format-agnostic: Rust does not parse chat
    // messages once the LLM gateway owns protocol compatibility.
    let prompt_tokens = i64::try_from(body_len.div_ceil(3))
        .unwrap_or(i64::MAX)
        .max(1);
    let context_room = entry.context.saturating_sub(prompt_tokens);
    if context_room <= 0 {
        return Err(Errx::new(
            ErrCode::InvalidRequest,
            400,
            "the request is too large for the model context window",
        ));
    }
    let desired_output = ["max_completion_tokens", "max_tokens"]
        .iter()
        .find_map(|name| payload.get(name).and_then(Value::as_u64))
        .unwrap_or(entry.max_output.max(1) as u64)
        .min(entry.max_output.max(1) as u64)
        .min(context_room as u64)
        .max(1);
    let outcome = state
        .app_db
        .reserve_generation(
            user_id,
            plan,
            Some(entry.tier.as_str()),
            desired_output,
            prompt_tokens,
        )
        .await
        .map_err(|_| Errx::new(ErrCode::Upstream, 500, "generation reservation failed"))?;
    match outcome {
        GenerationReservationOutcome::Reserved(reservation) => Ok(reservation),
        GenerationReservationOutcome::MonthlyExhausted => {
            Err(limit_error(state, LimitWindow::Monthly, 0, 0).await)
        }
        GenerationReservationOutcome::WeeklyExhausted => {
            Err(limit_error(state, LimitWindow::Weekly, 0, 0).await)
        }
    }
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
        .get_or_init(|| {
            reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(10))
                .build()
                .expect("static client configuration")
        })
        .clone()
}

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json, Response};
use futures_util::StreamExt;
use serde_json::Value;

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

// ─── LLM Gateway forwarding ───────────────────────────────────────────────────
//
// When LLM_GATEWAY_URL + LLM_GATEWAY_KEY are set the Rust gateway acts as a
// pure auth/quota/billing layer: it validates the principal, checks quotas,
// makes a billing reservation, then forwards the RAW request body to LiteLLM.
// LiteLLM owns ALL LLM concerns: routing, provider selection, retries,
// fallbacks, OpenAI-compat translation, OpenRouter API calls.

#[derive(Clone, Copy, Debug, Default)]
struct UsageSnapshot {
    input_tokens: i64,
    output_tokens: i64,
    cache_read_tokens: i64,
    cost_usd: f64,
}

fn usage_from_response(value: &Value) -> UsageSnapshot {
    let Some(usage) = value.get("usage").filter(|found| found.is_object()) else {
        return UsageSnapshot::default();
    };
    let Some(input_tokens) = usage.get("prompt_tokens").and_then(Value::as_i64) else {
        return UsageSnapshot::default();
    };
    UsageSnapshot {
        input_tokens,
        output_tokens: usage
            .get("completion_tokens")
            .and_then(Value::as_i64)
            .unwrap_or(0),
        cache_read_tokens: usage
            .pointer("/prompt_tokens_details/cached_tokens")
            .and_then(Value::as_i64)
            .or_else(|| usage.get("cached_tokens").and_then(Value::as_i64))
            .unwrap_or(0),
        cost_usd: usage
            .get("cost")
            .and_then(Value::as_f64)
            .or_else(|| usage.get("response_cost").and_then(Value::as_f64))
            .unwrap_or(0.0),
    }
}

fn usage_from_sse_line(line: &[u8]) -> Option<UsageSnapshot> {
    let text = std::str::from_utf8(line).ok()?;
    let data = text.trim().strip_prefix("data:")?.trim();
    if data == "[DONE]" {
        return None;
    }
    let value: Value = serde_json::from_str(data).ok()?;
    let found = usage_from_response(&value);
    (found.input_tokens > 0 || found.output_tokens > 0).then_some(found)
}

#[allow(clippy::too_many_arguments)]
async fn forward_to_llm_gateway(
    client: &reqwest::Client,
    gw_url: &str,
    gw_key: &str,
    body: axum::body::Bytes,
    streaming: bool,
    state: AppState,
    principal: Principal,
    model: String,
    entry: ModelCatalogEntry,
    reservation: GenerationReservation,
    start: Instant,
) -> Response {
    use axum::body::Body;
    use axum::http::header;

    let url = format!("{gw_url}/v1/chat/completions");
    let req = client
        .post(&url)
        .bearer_auth(gw_key)
        .header(header::CONTENT_TYPE, "application/json")
        // Forward user/plan metadata so LiteLLM can log them
        .header("x-argus-user-id", &principal.user_id)
        .header("x-argus-plan", &principal.plan)
        .header("x-argus-allowed-models", principal.allowlist.join(","))
        .body(body);

    let upstream = match req.send().await {
        Ok(r) => r,
        Err(err) => {
            tracing::error!(%err, "LLM gateway connection failed");
            let item = Errx::new(ErrCode::Upstream, 502, "LLM gateway is unreachable");
            let row = UsageRow {
                key_id: Some(principal.key_id.clone()),
                user_id: Some(principal.user_id.clone()),
                plan_id: Some(principal.plan.clone()),
                model: model.clone(),
                requested_model: Some(model.clone()),
                provider: Some("llm-gateway".to_owned()),
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cost_usd: 0.0,
                status: 502,
                latency_ms: Some(start.elapsed().as_millis() as i64),
                error_code: Some("gateway_unreachable".to_owned()),
            };
            let _ = settle_generation_usage(&state.usage, &reservation.id, row).await;
            return openai::error_response(item);
        }
    };

    let gw_status = upstream.status();

    let deployed_model = [
        "x-litellm-model-id",
        "x-litellm-model-name",
        "x-litellm-model-group",
    ]
    .iter()
    .find_map(|name| upstream.headers().get(*name).and_then(|v| v.to_str().ok()));
    let billing_entry = if model == AUTO_MODEL_ID {
        deployed_model
            .and_then(|name| {
                state.catalog.find_by_upstream(name).or_else(|| {
                    state
                        .catalog
                        .find_by_deployed_id(name.strip_suffix("-auto").unwrap_or(name))
                })
            })
            .unwrap_or_else(|| entry.clone())
    } else {
        entry.clone()
    };
    let billable = billing_entry.tier != Tier::Lite;

    // Token usage and cost come from the response body (JSON or final SSE
    // usage chunk). The cost header is only a non-streaming fallback.
    let gw_cost: f64 = upstream
        .headers()
        .get("x-litellm-response-cost")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0);

    if !gw_status.is_success() {
        // Gateway returned an error — relay it, settle billing as 0 cost.
        let body_bytes = upstream.bytes().await.unwrap_or_default();
        let status_u16 = gw_status.as_u16();
        let err_code = if status_u16 == 429 {
            "rate_limited"
        } else {
            "upstream_error"
        };
        let row = UsageRow {
            key_id: Some(principal.key_id.clone()),
            user_id: Some(principal.user_id.clone()),
            plan_id: Some(principal.plan.clone()),
            model: model.clone(),
            requested_model: Some(model.clone()),
            provider: Some("llm-gateway".to_owned()),
            input_tokens: 0,
            output_tokens: 0,
            cache_read_tokens: 0,
            cost_usd: 0.0,
            status: status_u16 as i32,
            latency_ms: Some(start.elapsed().as_millis() as i64),
            error_code: Some(err_code.to_owned()),
        };
        let _ = settle_generation_usage(&state.usage, &reservation.id, row).await;
        // Forward the gateway error body as-is so the client gets a proper OpenAI error shape.
        return (
            axum::http::StatusCode::from_u16(status_u16)
                .unwrap_or(axum::http::StatusCode::BAD_GATEWAY),
            [(header::CONTENT_TYPE, "application/json")],
            body_bytes,
        )
            .into_response();
    }

    // ── Success path: forward body bytes unchanged, settle from usage ────────
    let key_id = principal.key_id.clone();
    let user_id = principal.user_id.clone();
    let plan = principal.plan.clone();
    let usage_rec = state.usage.clone();
    let reservation_id = reservation.id.clone();

    if streaming {
        let byte_stream = upstream.bytes_stream();
        let (usage_tx, usage_rx) = tokio::sync::oneshot::channel::<UsageSnapshot>();
        let billing_entry = billing_entry.clone();

        tokio::spawn(async move {
            let reported = usage_rx.await.unwrap_or_default();
            let catalog_cost = cost_usd(
                &billing_entry,
                reported.input_tokens,
                reported.output_tokens,
                reported.cache_read_tokens,
            );
            let cost = if !billable {
                0.0
            } else if catalog_cost > 0.0 {
                catalog_cost
            } else if reported.cost_usd > 0.0 {
                reported.cost_usd
            } else {
                gw_cost
            };
            let row = UsageRow {
                key_id: Some(key_id),
                user_id: Some(user_id),
                plan_id: Some(plan),
                model: billing_entry.id.clone(),
                requested_model: Some(model.clone()),
                provider: Some("llm-gateway".to_owned()),
                input_tokens: reported.input_tokens,
                output_tokens: reported.output_tokens,
                cache_read_tokens: reported.cache_read_tokens,
                cost_usd: cost,
                status: 200,
                latency_ms: Some(start.elapsed().as_millis() as i64),
                error_code: None,
            };
            if let Err(err) = settle_generation_usage(&usage_rec, &reservation_id, row).await {
                tracing::error!(%err, reservation_id, "generation settlement failed (streaming gateway)");
            }
        });

        let wrapped = async_stream::stream! {
            let mut inner = byte_stream;
            let mut buffer: Vec<u8> = Vec::new();
            let mut reported = UsageSnapshot::default();
            while let Some(item) = inner.next().await {
                match item {
                    Ok(chunk) => {
                        buffer.extend_from_slice(&chunk);
                        while let Some(end) = buffer.iter().position(|byte| *byte == b'\n') {
                            let line: Vec<u8> = buffer.drain(..=end).collect();
                            if let Some(found) = usage_from_sse_line(&line) {
                                reported = found;
                            }
                        }
                        yield Ok::<_, reqwest::Error>(chunk);
                    }
                    Err(error) => yield Err(error),
                }
            }
            if let Some(found) = usage_from_sse_line(&buffer) {
                reported = found;
            }
            let _ = usage_tx.send(reported);
        };

        (
            axum::http::StatusCode::OK,
            [
                (header::CONTENT_TYPE, "text/event-stream"),
                (header::CACHE_CONTROL, "no-cache"),
                (
                    axum::http::header::HeaderName::from_static("x-accel-buffering"),
                    "no",
                ),
            ],
            Body::from_stream(wrapped),
        )
            .into_response()
    } else {
        // Non-streaming: buffer full response body, settle billing, forward.
        let body_bytes = match upstream.bytes().await {
            Ok(b) => b,
            Err(err) => {
                tracing::error!(%err, "failed to read LLM gateway response body");
                let row = UsageRow {
                    key_id: Some(key_id),
                    user_id: Some(user_id),
                    plan_id: Some(plan),
                    model: billing_entry.id.clone(),
                    requested_model: Some(model.clone()),
                    provider: Some("llm-gateway".to_owned()),
                    input_tokens: 0,
                    output_tokens: 0,
                    cache_read_tokens: 0,
                    cost_usd: 0.0,
                    status: 502,
                    latency_ms: Some(start.elapsed().as_millis() as i64),
                    error_code: Some("gateway_body_read_failed".to_owned()),
                };
                let _ = settle_generation_usage(&usage_rec, &reservation_id, row).await;
                return openai::error_response(Errx::new(
                    ErrCode::Upstream,
                    502,
                    "failed to read LLM gateway response",
                ));
            }
        };

        let reported = serde_json::from_slice::<Value>(&body_bytes)
            .map(|value| usage_from_response(&value))
            .unwrap_or_default();
        let catalog_cost = cost_usd(
            &billing_entry,
            reported.input_tokens,
            reported.output_tokens,
            reported.cache_read_tokens,
        );
        let cost = if !billable {
            0.0
        } else if catalog_cost > 0.0 {
            catalog_cost
        } else if reported.cost_usd > 0.0 {
            reported.cost_usd
        } else {
            gw_cost
        };
        let row = UsageRow {
            key_id: Some(key_id),
            user_id: Some(user_id),
            plan_id: Some(plan),
            model: billing_entry.id.clone(),
            requested_model: Some(model.clone()),
            provider: Some("llm-gateway".to_owned()),
            input_tokens: reported.input_tokens,
            output_tokens: reported.output_tokens,
            cache_read_tokens: reported.cache_read_tokens,
            cost_usd: cost,
            status: 200,
            latency_ms: Some(start.elapsed().as_millis() as i64),
            error_code: None,
        };
        if let Err(err) = settle_generation_usage(&usage_rec, &reservation_id, row).await {
            tracing::error!(%err, reservation_id, "generation settlement failed (non-streaming gateway)");
        }

        (
            axum::http::StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json")],
            body_bytes,
        )
            .into_response()
    }
}

pub async fn openai_chat_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Response {
    let start = Instant::now();

    // ── 1. Auth ────────────────────────────────────────────────────────────
    let principal = match authenticate(&state, &headers).await {
        Ok(p) => p,
        Err(item) => {
            state.usage.record(UsageRow {
                key_id: None,
                user_id: None,
                plan_id: None,
                model: "unknown".to_owned(),
                requested_model: None,
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

    // ── 2. Parse model from raw JSON (no IR) ───────────────────────────────
    let payload: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return openai::error_response(Errx::new(
                ErrCode::InvalidRequest,
                400,
                format!("the request body is not valid JSON: {e}"),
            ));
        }
    };
    let model_id = payload_model(&payload);

    // ── 3. Quota check ─────────────────────────────────────────────────────
    if let Err(item) = enforce_plan_quotas(&state, &principal.user_id, &principal.plan).await {
            state.usage.record(UsageRow {
                key_id: Some(principal.key_id.clone()),
                user_id: Some(principal.user_id.clone()),
                plan_id: Some(principal.plan.clone()),
                model: model_id.clone(),
                requested_model: Some(model_id.clone()),
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

    // ── 4. Validate the requested model without resolving or rewriting it ───
    let entry = match state.catalog.find(&model_id) {
        Some(entry) => entry,
        None => {
            let item = Errx::new(
                ErrCode::NotFound,
                404,
                format!("no model named `{model_id}` is published"),
            );
            state.usage.record(UsageRow {
                key_id: Some(principal.key_id.clone()),
                user_id: Some(principal.user_id.clone()),
                plan_id: Some(principal.plan.clone()),
                model: model_id.clone(),
                requested_model: Some(model_id.clone()),
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
    let model = entry.id().to_owned();
    if !principal.allowlist.iter().any(|allowed| allowed == &model) {
        let item = Errx::new(
            ErrCode::Forbidden,
            403,
            format!("model `{model}` is not enabled for your plan"),
        );
        state.usage.record(UsageRow {
            key_id: Some(principal.key_id.clone()),
            user_id: Some(principal.user_id.clone()),
            plan_id: Some(principal.plan.clone()),
            model: model.clone(),
            requested_model: Some(model.clone()),
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

    // ── 5. Require the internal router ──────────────────────────────────────
    let (gw_url, gw_key) = match (
        state.settings.llm_gateway_url.clone(),
        state.settings.llm_gateway_key.clone(),
    ) {
        (Some(url), Some(key)) => (url, key),
        _ => {
            let item = Errx::new(
                ErrCode::ProviderUnconfigured,
                503,
                "the internal LLM gateway is not configured",
            );
            state.usage.record(UsageRow {
                key_id: Some(principal.key_id.clone()),
                user_id: Some(principal.user_id.clone()),
                plan_id: Some(principal.plan.clone()),
                model: model.clone(),
                requested_model: Some(model.clone()),
                provider: Some("llm-gateway".to_owned()),
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

    // ── 6. Billing reservation ─────────────────────────────────────────────
    let reservation = match reserve_gateway_generation(
        &state,
        &principal.user_id,
        &principal.plan,
        &payload,
        body.len(),
        &entry,
    )
    .await
    {
        Ok(r) => r,
        Err(item) => {
            state.usage.record(UsageRow {
                key_id: Some(principal.key_id.clone()),
                user_id: Some(principal.user_id.clone()),
                plan_id: Some(principal.plan.clone()),
                model: model.clone(),
                requested_model: Some(model.clone()),
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
            requested_model: Some(model.clone()),
            provider: Some("llm-gateway".to_owned()),
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

    // ── 7. Forward the original request bytes unchanged ────────────────────
    let streaming = payload
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    return forward_to_llm_gateway(
        &shared_client(),
        &gw_url,
        &gw_key,
        body,
        streaming,
        state,
        principal,
        model,
        entry,
        reservation,
        start,
    )
    .await;
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
                requested_model: None,
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

    if let Err(item) = enforce_plan_quotas(&state, &principal.user_id, &principal.plan).await {
        state.usage.record(UsageRow {
            key_id: Some(principal.key_id.clone()),
            user_id: Some(principal.user_id.clone()),
            plan_id: Some(principal.plan.clone()),
            model: "web-search".to_owned(),
            requested_model: None,
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
                requested_model: None,
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
                requested_model: None,
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
                requested_model: None,
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
        Ok((search_res, cost)) => {
            let latency = start.elapsed().as_millis() as i64;
            let row = UsageRow {
                key_id: Some(principal.key_id.clone()),
                user_id: Some(principal.user_id.clone()),
                plan_id: Some(principal.plan.clone()),
                model: "web-search".to_owned(),
                requested_model: None,
                provider: Some(search_res.provider.clone()),
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cost_usd: cost,
                status: 200,
                latency_ms: Some(latency),
                error_code: None,
            };
            state.usage.record(row);
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
                requested_model: None,
                provider: None,
                input_tokens: 0,
                output_tokens: 0,
                cache_read_tokens: 0,
                cost_usd: 0.0,
                status: err.status as i32,
                latency_ms: Some(latency),
                error_code: Some(err.code.as_str().to_owned()),
            };
            state.usage.record(row);
            openai::error_response(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{usage_from_response, usage_from_sse_line};

    #[test]
    fn reads_usage_and_cost_from_json_response() {
        let snapshot = usage_from_response(&json!({
            "usage": {
                "prompt_tokens": 12,
                "completion_tokens": 7,
                "prompt_tokens_details": {"cached_tokens": 3},
                "cost": 0.00042
            }
        }));
        assert_eq!(snapshot.input_tokens, 12);
        assert_eq!(snapshot.output_tokens, 7);
        assert_eq!(snapshot.cache_read_tokens, 3);
        assert_eq!(snapshot.cost_usd, 0.00042);
    }

    #[test]
    fn reads_usage_and_cost_from_final_sse_chunk() {
        let line = b"data: {\"choices\":[],\"usage\":{\"prompt_tokens\":26,\"completion_tokens\":48,\"cost\":0.0008778}}\n";
        let snapshot = usage_from_sse_line(line).expect("usage chunk is parsed");
        assert_eq!(snapshot.input_tokens, 26);
        assert_eq!(snapshot.output_tokens, 48);
        assert_eq!(snapshot.cost_usd, 0.0008778);
    }

    #[test]
    fn ignores_chunks_without_usage() {
        let line = b"data: {\"choices\":[{\"delta\":{\"content\":\"hi\"}}],\"usage\":null}\n";
        assert!(usage_from_sse_line(line).is_none());
        assert!(usage_from_sse_line(b"data: [DONE]\n").is_none());
    }
}
