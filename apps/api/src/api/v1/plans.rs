use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json as AxumJson, Router};
use better_auth::integrations::axum::CurrentSession;

use crate::auth::AppAuthSchema;
use crate::state::AppState;

fn user_id(session: &CurrentSession<AppAuthSchema>) -> String {
    session.user.id.typed().ok().cloned().unwrap_or_default()
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_plans))
        .route("/refresh", post(refresh_plans))
}

async fn list_plans(
    State(state): State<AppState>,
    _session: CurrentSession<AppAuthSchema>,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    let plans = state.app_db.list_plans().await.map_err(|_error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(
                serde_json::json!({"error": {"code": "PLAN_LIST", "message": "An error has occurred. Try again later."}}),
            ),
        )
    })?;
    Ok(AxumJson(serde_json::json!({"plans": plans})))
}

async fn refresh_plans(
    State(state): State<AppState>,
    session: CurrentSession<AppAuthSchema>,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    let admin_email = session.user.email.typed().ok().cloned().flatten();
    let is_admin = state
        .settings
        .is_admin(&user_id(&session), admin_email.as_deref());
    if !is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            AxumJson(
                serde_json::json!({"error": {"code": "ADMIN_ONLY", "message": "this action requires an operator account"}}),
            ),
        ));
    }
    let plans = crate::services::plans::seed_entries();
    if let Err(_error) = state.app_db.seed_plans(&plans).await {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(
                serde_json::json!({"error": {"code": "PLAN_REFRESH", "message": "An error has occurred. Try again later."}}),
            ),
        ));
    }
    if let Err(_error) = state.app_db.seed_plan_allowlists(&plans).await {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(
                serde_json::json!({"error": {"code": "PLAN_REFRESH", "message": "An error has occurred. Try again later."}}),
            ),
        ));
    }
    Ok(AxumJson(
        serde_json::json!({"refreshed": true, "count": plans.len()}),
    ))
}
