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
    Router::new().route("/", get(list_usage))
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
