use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json as AxumJson, Router};
use better_auth::integrations::axum::CurrentSession;
use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::auth::AppAuthSchema;
use crate::state::AppState;

use sqlx::Row;

#[derive(Deserialize, Default)]
struct UsageQuery {
    #[serde(alias = "keyId")]
    key_id: Option<String>,
    since: Option<String>,
    limit: Option<i64>,
}

fn user_id(session: &CurrentSession<AppAuthSchema>) -> String {
    session
        .user
        .id
        .typed()
        .ok()
        .cloned()
        .unwrap_or_default()
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_usage))
        .route("/summary", get(usage_summary))
}

async fn list_usage(
    State(state): State<AppState>,
    session: CurrentSession<AppAuthSchema>,
    Query(q): Query<UsageQuery>,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    let user = user_id(&session);
    if user.is_empty() {
        return Err((
            StatusCode::UNAUTHORIZED,
            AxumJson(serde_json::json!({"error": {"code": "SESSION", "message": "The session carries no user identifier."}})),
        ));
    }

    let limit = q.limit.unwrap_or(500).clamp(1, 1000);
    let since: Option<DateTime<Utc>> = match q.since.as_deref() {
        Some(raw) => Some(
            DateTime::parse_from_rfc3339(raw)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        AxumJson(serde_json::json!({"error": {"code": "USAGE_SINCE", "message": format!("Invalid since: {e}")}})),
                    )
                })?,
        ),
        None => None,
    };

    let pool = state.app_db.pool();
    let rows = match (q.key_id.as_deref(), since) {
        (Some(key_id), Some(since)) => {
            sqlx::query(
                "SELECT id, created_at, key_id, user_id, plan_id, model, provider, input_tokens, output_tokens, cache_read_tokens, cost_usd, status, latency_ms, error_code FROM usage_logs WHERE user_id = $1 AND key_id = $2 AND created_at >= $3 ORDER BY created_at DESC LIMIT $4",
            )
            .bind(&user)
            .bind(key_id)
            .bind(since)
            .bind(limit)
            .fetch_all(&pool)
            .await
        }
        (Some(key_id), None) => {
            sqlx::query(
                "SELECT id, created_at, key_id, user_id, plan_id, model, provider, input_tokens, output_tokens, cache_read_tokens, cost_usd, status, latency_ms, error_code FROM usage_logs WHERE user_id = $1 AND key_id = $2 ORDER BY created_at DESC LIMIT $3",
            )
            .bind(&user)
            .bind(key_id)
            .bind(limit)
            .fetch_all(&pool)
            .await
        }
        (None, Some(since)) => {
            sqlx::query(
                "SELECT id, created_at, key_id, user_id, plan_id, model, provider, input_tokens, output_tokens, cache_read_tokens, cost_usd, status, latency_ms, error_code FROM usage_logs WHERE user_id = $1 AND created_at >= $2 ORDER BY created_at DESC LIMIT $3",
            )
            .bind(&user)
            .bind(since)
            .bind(limit)
            .fetch_all(&pool)
            .await
        }
        (None, None) => {
            sqlx::query(
                "SELECT id, created_at, key_id, user_id, plan_id, model, provider, input_tokens, output_tokens, cache_read_tokens, cost_usd, status, latency_ms, error_code FROM usage_logs WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2",
            )
            .bind(&user)
            .bind(limit)
            .fetch_all(&pool)
            .await
        }
    }
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(serde_json::json!({"error": {"code": "USAGE_LIST", "message": e.to_string()}})),
        )
    })?;

    let mut out = Vec::new();
    for row in rows {
        out.push(serde_json::json!({
            "id": row.try_get::<i64, _>("id").ok(),
            "createdAt": row.try_get::<chrono::DateTime<chrono::Utc>, _>("created_at").map(|v| v.to_rfc3339()).ok(),
            "keyId": row.try_get::<Option<String>, _>("key_id").ok().flatten(),
            "userId": row.try_get::<Option<String>, _>("user_id").ok().flatten(),
            "planId": row.try_get::<Option<String>, _>("plan_id").ok().flatten(),
            "model": row.try_get::<String, _>("model").ok(),
            "provider": row.try_get::<Option<String>, _>("provider").ok().flatten(),
            "inputTokens": row.try_get::<i64, _>("input_tokens").ok(),
            "outputTokens": row.try_get::<i64, _>("output_tokens").ok(),
            "cacheReadTokens": row.try_get::<i64, _>("cache_read_tokens").ok(),
            "costUsd": row.try_get::<f64, _>("cost_usd").ok(),
            "status": row.try_get::<i32, _>("status").ok(),
            "latencyMs": row.try_get::<Option<i64>, _>("latency_ms").ok().flatten(),
            "errorCode": row.try_get::<Option<String>, _>("error_code").ok().flatten(),
        }));
    }
    Ok(AxumJson(serde_json::json!({"rows": out})))
}


#[derive(Deserialize, Default)]
struct SummaryQuery {
    #[serde(alias = "keyId")]
    key_id: Option<String>,
    since: Option<String>,
    /// `hour` or `day`; anything else defaults to `day`.
    bucket: Option<String>,
    /// IANA zone name (e.g. `Africa/Harare`) so chart buckets line up with
    /// the viewer's local hours/days. Unknown names fall back to UTC.
    tz: Option<String>,
}

type ApiErr = (StatusCode, AxumJson<serde_json::Value>);

fn api_err(status: StatusCode, code: &str, message: String) -> ApiErr {
    (
        status,
        AxumJson(serde_json::json!({"error": {"code": code, "message": message}})),
    )
}

/// `GET /api/v1/usage/summary` — aggregates computed in SQL over *every* row
/// in range. The list endpoint is capped (max 1000 rows) so totals summed
/// from it undercount busy accounts; the dashboard uses this for totals,
/// per-model / per-key breakdowns and chart series.
async fn usage_summary(
    State(state): State<AppState>,
    session: CurrentSession<AppAuthSchema>,
    Query(q): Query<SummaryQuery>,
) -> Result<AxumJson<serde_json::Value>, ApiErr> {
    let user = user_id(&session);
    if user.is_empty() {
        return Err(api_err(
            StatusCode::UNAUTHORIZED,
            "SESSION",
            "The session carries no user identifier.".to_owned(),
        ));
    }
    let since: Option<DateTime<Utc>> = match q.since.as_deref() {
        Some(raw) => Some(
            DateTime::parse_from_rfc3339(raw)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| {
                    api_err(StatusCode::BAD_REQUEST, "USAGE_SINCE", format!("Invalid since: {e}"))
                })?,
        ),
        None => None,
    };
    let unit = if q.bucket.as_deref() == Some("hour") { "hour" } else { "day" };
    let pool = state.app_db.pool();
    let db_err = |e: sqlx::Error| api_err(StatusCode::INTERNAL_SERVER_ERROR, "USAGE_SUMMARY", e.to_string());

    // Only ever hand Postgres a zone name it knows.
    let tz = match q.tz.as_deref().filter(|name| !name.is_empty()) {
        Some(name) => {
            let known = sqlx::query("SELECT 1 FROM pg_timezone_names WHERE name = $1 LIMIT 1")
                .bind(name)
                .fetch_optional(&pool)
                .await
                .map_err(db_err)?
                .is_some();
            if known { name.to_owned() } else { "UTC".to_owned() }
        }
        None => "UTC".to_owned(),
    };

    // SQL is assembled at compile time (`concat!`), never at runtime: every
    // user-controlled value below is a bound parameter.
    macro_rules! filter {
        () => {
            "user_id = $1 AND ($2::text IS NULL OR key_id = $2) AND ($3::timestamptz IS NULL OR created_at >= $3)"
        };
    }
    macro_rules! agg {
        () => {
            "COUNT(*)::bigint AS requests, (COUNT(*) FILTER (WHERE status >= 400))::bigint AS errors, COALESCE(SUM(input_tokens), 0)::bigint AS input_tokens, COALESCE(SUM(output_tokens), 0)::bigint AS output_tokens, COALESCE(SUM(cache_read_tokens), 0)::bigint AS cache_read_tokens, COALESCE(SUM(cost_usd), 0)::float8 AS cost_usd"
        };
    }

    let agg_json = |row: &sqlx::postgres::PgRow| {
        serde_json::json!({
            "requests": row.try_get::<i64, _>("requests").unwrap_or(0),
            "errors": row.try_get::<i64, _>("errors").unwrap_or(0),
            "inputTokens": row.try_get::<i64, _>("input_tokens").unwrap_or(0),
            "outputTokens": row.try_get::<i64, _>("output_tokens").unwrap_or(0),
            "cacheReadTokens": row.try_get::<i64, _>("cache_read_tokens").unwrap_or(0),
            "costUsd": row.try_get::<f64, _>("cost_usd").unwrap_or(0.0),
        })
    };

    let key_id = q.key_id.as_deref();

    let totals_row = sqlx::query(concat!("SELECT ", agg!(), " FROM usage_logs WHERE ", filter!()))
        .bind(&user)
        .bind(key_id)
        .bind(since)
        .fetch_one(&pool)
        .await
        .map_err(db_err)?;

    let by_model_rows = sqlx::query(concat!(
        "SELECT model, ", agg!(), " FROM usage_logs WHERE ", filter!(),
        " GROUP BY model ORDER BY SUM(cost_usd) DESC, COUNT(*) DESC"
    ))
    .bind(&user)
    .bind(key_id)
    .bind(since)
    .fetch_all(&pool)
    .await
    .map_err(db_err)?;

    let by_key_rows = sqlx::query(concat!(
        "SELECT key_id, ", agg!(), " FROM usage_logs WHERE ", filter!(),
        " GROUP BY key_id ORDER BY SUM(cost_usd) DESC, COUNT(*) DESC"
    ))
    .bind(&user)
    .bind(key_id)
    .bind(since)
    .fetch_all(&pool)
    .await
    .map_err(db_err)?;

    // `date_trunc` over the viewer-local wall clock, rendered as a plain
    // local timestamp string the client can key its buckets on.
    let series_rows = sqlx::query(concat!(
        "SELECT to_char(date_trunc($4::text, created_at AT TIME ZONE $5::text), 'YYYY-MM-DD\"T\"HH24:MI') AS bucket, ",
        "COUNT(*)::bigint AS requests, ",
        "COALESCE(SUM(input_tokens + output_tokens), 0)::bigint AS tokens, ",
        "COALESCE(SUM(cost_usd), 0)::float8 AS cost_usd ",
        "FROM usage_logs WHERE ", filter!(), " GROUP BY 1 ORDER BY 1"
    ))
    .bind(&user)
    .bind(key_id)
    .bind(since)
    .bind(unit)
    .bind(&tz)
    .fetch_all(&pool)
    .await
    .map_err(db_err)?;

    let by_model: Vec<_> = by_model_rows
        .iter()
        .map(|row| {
            let mut v = agg_json(row);
            v["model"] = serde_json::json!(row.try_get::<String, _>("model").unwrap_or_default());
            v
        })
        .collect();
    let by_key: Vec<_> = by_key_rows
        .iter()
        .map(|row| {
            let mut v = agg_json(row);
            v["keyId"] = serde_json::json!(row.try_get::<Option<String>, _>("key_id").ok().flatten());
            v
        })
        .collect();
    let series: Vec<_> = series_rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "bucket": row.try_get::<String, _>("bucket").unwrap_or_default(),
                "requests": row.try_get::<i64, _>("requests").unwrap_or(0),
                "tokens": row.try_get::<i64, _>("tokens").unwrap_or(0),
                "costUsd": row.try_get::<f64, _>("cost_usd").unwrap_or(0.0),
            })
        })
        .collect();

    Ok(AxumJson(serde_json::json!({
        "totals": agg_json(&totals_row),
        "byModel": by_model,
        "byKey": by_key,
        "series": series,
        "bucket": unit,
        "tz": tz,
    })))
}
