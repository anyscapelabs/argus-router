use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json as AxumJson, Router};
use better_auth::integrations::axum::CurrentSession;
use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::auth::AppAuthSchema;
use crate::state::AppState;

use sqlx::{QueryBuilder, Row};

#[derive(Deserialize, Default)]
struct UsageQuery {
    #[serde(alias = "keyId")]
    key_id: Option<String>,
    #[serde(rename = "type")]
    log_type: Option<String>,
    since: Option<String>,
    until: Option<String>,
    limit: Option<i64>,
}

fn user_id(session: &CurrentSession<AppAuthSchema>) -> String {
    session.user.id.typed().ok().cloned().unwrap_or_default()
}

fn display_model(model: Option<String>) -> Option<String> {
    model.map(|model| {
        if model == crate::services::catalog::AUTO_MODEL_ID {
            "Auto".to_owned()
        } else {
            model
        }
    })
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
            AxumJson(
                serde_json::json!({"error": {"code": "SESSION", "message": "The session carries no user identifier."}}),
            ),
        ));
    }

    let limit = q.limit.unwrap_or(500).clamp(1, 1000);
    let since: Option<DateTime<Utc>> = match q.since.as_deref() {
        Some(raw) => Some(
            DateTime::parse_from_rfc3339(raw)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_e| {
                    (
                        StatusCode::BAD_REQUEST,
                        AxumJson(serde_json::json!({"error": {"code": "USAGE_SINCE", "message": "An error has occurred. Try again later.".to_owned()}})),
                    )
                })?,
        ),
        None => None,
    };
    let until: Option<DateTime<Utc>> = match q.until.as_deref() {
        Some(raw) => Some(
            DateTime::parse_from_rfc3339(raw)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_e| {
                    (
                        StatusCode::BAD_REQUEST,
                        AxumJson(serde_json::json!({"error": {"code": "USAGE_UNTIL", "message": "An error has occurred. Try again later.".to_owned()}})),
                    )
                })?,
        ),
        None => None,
    };

    let pool = state.app_db.pool();
    let mut qb = QueryBuilder::new(
        "SELECT id, created_at, key_id, user_id, plan_id, model, requested_model, provider, input_tokens, output_tokens, cache_read_tokens, status, latency_ms, error_code, log_type, details FROM usage_logs WHERE user_id = ",
    );
    qb.push_bind(&user);
    if let Some(key_id) = q.key_id.as_deref() {
        qb.push(" AND key_id = ").push_bind(key_id);
    }
    if let Some(log_type) = q.log_type.as_deref() {
        qb.push(" AND log_type = ").push_bind(log_type);
    }
    if let Some(since) = since {
        qb.push(" AND created_at >= ").push_bind(since);
    }
    if let Some(until) = until {
        qb.push(" AND created_at <= ").push_bind(until);
    }
    qb.push(" ORDER BY created_at DESC LIMIT ").push_bind(limit);
    let rows = qb.build().fetch_all(&pool).await.map_err(|error| {
        tracing::error!(%error, "usage list query failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(serde_json::json!({"error": {"code": "USAGE_LIST", "message": "An error has occurred. Try again later."}})),
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
            "model": display_model(
                row.try_get::<Option<String>, _>("requested_model").ok().flatten()
                    .or_else(|| row.try_get::<String, _>("model").ok()),
            ),
            "provider": row.try_get::<Option<String>, _>("provider").ok().flatten(),
            "inputTokens": row.try_get::<i64, _>("input_tokens").ok(),
            "outputTokens": row.try_get::<i64, _>("output_tokens").ok(),
            "cacheReadTokens": row.try_get::<i64, _>("cache_read_tokens").ok(),
            "status": row.try_get::<i32, _>("status").ok(),
            "latencyMs": row.try_get::<Option<i64>, _>("latency_ms").ok().flatten(),
            "errorCode": row.try_get::<Option<String>, _>("error_code").ok().flatten(),
            "logType": row.try_get::<String, _>("log_type").unwrap_or_else(|_| "chat".to_owned()),
            "details": row.try_get::<String, _>("details")
                .ok()
                .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
                .unwrap_or_else(|| serde_json::json!({})),
        }));
    }
    Ok(AxumJson(serde_json::json!({"rows": out})))
}

#[derive(Deserialize, Default)]
struct SummaryQuery {
    #[serde(alias = "keyId")]
    key_id: Option<String>,
    since: Option<String>,
    bucket: Option<String>,
    tz: Option<String>,
}

type ApiErr = (StatusCode, AxumJson<serde_json::Value>);

fn api_err(status: StatusCode, code: &str, message: String) -> ApiErr {
    (
        status,
        AxumJson(serde_json::json!({"error": {"code": code, "message": message}})),
    )
}

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
                .map_err(|_e| {
                    api_err(
                        StatusCode::BAD_REQUEST,
                        "USAGE_SINCE",
                        "An error has occurred. Try again later.".to_owned(),
                    )
                })?,
        ),
        None => None,
    };
    let since = since.map(|s| s.max(Utc::now() - chrono::Duration::days(400)));
    let unit = if q.bucket.as_deref() == Some("hour") {
        "hour"
    } else {
        "day"
    };
    let pool = state.app_db.pool();
    let db_err = |error: sqlx::Error| {
        tracing::error!(%error, "usage summary database operation failed");
        api_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            "USAGE_SUMMARY",
            "An error has occurred. Try again later.".to_owned(),
        )
    };

    let tz = match q.tz.as_deref().filter(|name| !name.is_empty()) {
        Some(name) => {
            let known = sqlx::query("SELECT 1 FROM pg_timezone_names WHERE name = $1 LIMIT 1")
                .bind(name)
                .fetch_optional(&pool)
                .await
                .map_err(db_err)?
                .is_some();
            if known {
                name.to_owned()
            } else {
                "UTC".to_owned()
            }
        }
        None => "UTC".to_owned(),
    };

    macro_rules! filter {
        () => {
            "user_id = $1 AND ($2::text IS NULL OR key_id = $2) AND ($3::timestamptz IS NULL OR created_at >= $3)"
        };
    }
    macro_rules! agg {
        () => {
            "COUNT(*)::bigint AS requests, (COUNT(*) FILTER (WHERE status >= 400))::bigint AS errors, COALESCE(SUM(input_tokens), 0)::bigint AS input_tokens, COALESCE(SUM(output_tokens), 0)::bigint AS output_tokens, COALESCE(SUM(cache_read_tokens), 0)::bigint AS cache_read_tokens"
        };
    }

    let agg_json = |row: &sqlx::postgres::PgRow| {
        serde_json::json!({
            "requests": row.try_get::<i64, _>("requests").unwrap_or(0),
            "errors": row.try_get::<i64, _>("errors").unwrap_or(0),
            "inputTokens": row.try_get::<i64, _>("input_tokens").unwrap_or(0),
            "outputTokens": row.try_get::<i64, _>("output_tokens").unwrap_or(0),
            "cacheReadTokens": row.try_get::<i64, _>("cache_read_tokens").unwrap_or(0),
        })
    };

    let key_id = q.key_id.as_deref();

    let totals_row = sqlx::query(concat!(
        "SELECT ",
        agg!(),
        " FROM usage_logs WHERE ",
        filter!()
    ))
    .bind(&user)
    .bind(key_id)
    .bind(since)
    .fetch_one(&pool)
    .await
    .map_err(db_err)?;

    let by_model_rows = sqlx::query(concat!(
        "SELECT COALESCE(NULLIF(requested_model, ''), model) AS model, ",
        agg!(),
        " FROM usage_logs WHERE ",
        filter!(),
        " GROUP BY 1 ORDER BY SUM(input_tokens + output_tokens) DESC, COUNT(*) DESC LIMIT 50"
    ))
    .bind(&user)
    .bind(key_id)
    .bind(since)
    .fetch_all(&pool)
    .await
    .map_err(db_err)?;

    let by_key_rows = sqlx::query(concat!(
        "SELECT key_id, ",
        agg!(),
        " FROM usage_logs WHERE ",
        filter!(),
        " GROUP BY key_id ORDER BY SUM(input_tokens + output_tokens) DESC, COUNT(*) DESC LIMIT 50"
    ))
    .bind(&user)
    .bind(key_id)
    .bind(since)
    .fetch_all(&pool)
    .await
    .map_err(db_err)?;

    let series_rows = sqlx::query(concat!(
        "SELECT to_char(date_trunc($4::text, created_at AT TIME ZONE $5::text), 'YYYY-MM-DD\"T\"HH24:MI') AS bucket, ",
        "COUNT(*)::bigint AS requests, ",
        "COALESCE(SUM(input_tokens + output_tokens), 0)::bigint AS tokens ",
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
            v["model"] = serde_json::json!(display_model(
                row.try_get::<String, _>("model").ok()
            ));
            v
        })
        .collect();
    let by_key: Vec<_> = by_key_rows
        .iter()
        .map(|row| {
            let mut v = agg_json(row);
            v["keyId"] =
                serde_json::json!(row.try_get::<Option<String>, _>("key_id").ok().flatten());
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
            })
        })
        .collect();

    let by_model_series_rows = sqlx::query(concat!(
        "SELECT to_char(date_trunc($4::text, created_at AT TIME ZONE $5::text), 'YYYY-MM-DD\"T\"HH24:MI') AS bucket, ",
        "COALESCE(NULLIF(requested_model, ''), model) AS model, ",
        "COUNT(*)::bigint AS requests, ",
        "COALESCE(SUM(input_tokens + output_tokens), 0)::bigint AS tokens ",
        "FROM usage_logs WHERE ", filter!(), " GROUP BY 1, 2 ORDER BY 1, 2"
    ))
    .bind(&user)
    .bind(key_id)
    .bind(since)
    .bind(unit)
    .bind(&tz)
    .fetch_all(&pool)
    .await
    .map_err(db_err)?;

    let by_model_series: Vec<_> = by_model_series_rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "bucket": row.try_get::<String, _>("bucket").unwrap_or_default(),
                "model": display_model(row.try_get::<String, _>("model").ok()),
                "requests": row.try_get::<i64, _>("requests").unwrap_or(0),
                "tokens": row.try_get::<i64, _>("tokens").unwrap_or(0),
            })
        })
        .collect();

    let period = state.app_db.usage_period(&user).await.map_err(db_err)?;
    let period_row = sqlx::query(
        "SELECT
           COALESCE(SUM(input_tokens + output_tokens) FILTER (WHERE created_at >= $3), 0)::bigint AS weekly,
           COALESCE(SUM(input_tokens + output_tokens) FILTER (WHERE created_at >= $4), 0)::bigint AS monthly
         FROM usage_logs
         WHERE user_id = $1 AND ($2::text IS NULL OR key_id = $2)",
    )
    .bind(&user)
    .bind(key_id)
    .bind(period.weekly_start)
    .bind(period.monthly_start)
    .fetch_one(&pool)
    .await
    .map_err(db_err)?;
    let weekly_tokens = period_row.try_get::<i64, _>("weekly").unwrap_or(0);
    let monthly_tokens = period_row.try_get::<i64, _>("monthly").unwrap_or(0);

    let plan_id = state.app_db.plan_of(&user).await.map_err(db_err)?;
    let (voice_used, voice_quota) = state
        .app_db
        .voice_usage(&user, &plan_id)
        .await
        .map_err(db_err)?;
    let (search_used, search_quota) = state
        .app_db
        .search_usage(&user, &plan_id)
        .await
        .map_err(db_err)?;

    Ok(AxumJson(serde_json::json!({
        "weeklyResetAt": period.weekly_reset.to_rfc3339(),
        "monthlyResetAt": period.monthly_reset.to_rfc3339(),
        "periods": {
            "weekly": {
                "startAt": period.weekly_start.to_rfc3339(),
                "resetAt": period.weekly_reset.to_rfc3339(),
                "tokens": weekly_tokens,
            },
            "monthly": {
                "startAt": period.monthly_start.to_rfc3339(),
                "resetAt": period.monthly_reset.to_rfc3339(),
                "tokens": monthly_tokens,
            },
        },
        "totals": agg_json(&totals_row),
        "byModel": by_model,
        "byKey": by_key,
        "series": series,
        "byModelSeries": by_model_series,
        "bucket": unit,
        "tz": tz,
        "voiceMinutesUsed": voice_used,
        "voiceMinutesQuota": voice_quota,
        "webSearchUsed": search_used,
        "webSearchQuota": search_quota,
    })))
}
