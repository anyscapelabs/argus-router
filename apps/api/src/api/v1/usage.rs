use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json as AxumJson, Router};
use better_auth::integrations::axum::CurrentSession;
use serde::Deserialize;

use crate::auth::AppAuthSchema;
use crate::state::AppState;

use sqlx::Row;

#[derive(Deserialize, Default)]
struct UsageQuery {
    key_id: Option<String>,
    limit: Option<i64>,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(list_usage))
}

async fn list_usage(
    State(state): State<AppState>,
    _session: CurrentSession<AppAuthSchema>,
    Query(q): Query<UsageQuery>,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    let limit = q.limit.unwrap_or(100).clamp(1, 1000);
    let pool = state.app_db.pool();
    let rows = if let Some(key_id) = q.key_id {
        sqlx::query(
            "SELECT id, created_at, key_id, user_id, plan_id, model, provider, input_tokens, output_tokens, cache_read_tokens, cost_usd, status, latency_ms, error_code FROM usage_logs WHERE key_id = $1 ORDER BY created_at DESC LIMIT $2",
        )
        .bind(key_id)
        .bind(limit)
        .fetch_all(&pool)
        .await
    } else {
        sqlx::query(
            "SELECT id, created_at, key_id, user_id, plan_id, model, provider, input_tokens, output_tokens, cache_read_tokens, cost_usd, status, latency_ms, error_code FROM usage_logs ORDER BY created_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&pool)
        .await
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