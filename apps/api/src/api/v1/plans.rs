use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json as AxumJson, Router};
use better_auth::integrations::axum::CurrentSession;

use crate::auth::AppAuthSchema;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_plans))
        .route("/refresh", post(refresh_plans))
}

async fn list_plans(
    State(state): State<AppState>,
    _session: CurrentSession<AppAuthSchema>,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    let plans = state.app_db.list_plans().await.map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(serde_json::json!({"error": {"code": "PLAN_LIST", "message": error.to_string()}})),
        )
    })?;
    Ok(AxumJson(serde_json::json!({"plans": plans})))
}

async fn refresh_plans(
    State(state): State<AppState>,
    _session: CurrentSession<AppAuthSchema>,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    let plans = crate::services::plans::seed_entries();
    if let Err(error) = state.app_db.seed_plans(&plans).await {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(serde_json::json!({"error": {"code": "PLAN_REFRESH", "message": error.to_string()}})),
        ));
    }
    if let Err(error) = state.app_db.seed_plan_allowlists(&plans).await {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(serde_json::json!({"error": {"code": "PLAN_REFRESH", "message": error.to_string()}})),
        ));
    }
    Ok(AxumJson(serde_json::json!({"refreshed": true, "count": plans.len()})))
}