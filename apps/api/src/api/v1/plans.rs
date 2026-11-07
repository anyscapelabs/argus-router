use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json as AxumJson, Router};
use better_auth::integrations::axum::CurrentSession;

use crate::auth::AppAuthSchema;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(list_plans))
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