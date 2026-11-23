use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json as AxumJson, Router, extract::State};
use better_auth::integrations::axum::CurrentSession;
use serde::Serialize;

use crate::auth::AppAuthSchema;
use crate::state::AppState;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnboardingStatus {
    pub completed: bool,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/", post(complete))
}

type ApiFailure = (StatusCode, AxumJson<serde_json::Value>);

fn failure(status: StatusCode, code: &str, message: &str) -> ApiFailure {
    (
        status,
        AxumJson(serde_json::json!({"error": {"code": code, "message": message}})),
    )
}

fn user_id(session: &CurrentSession<AppAuthSchema>) -> Result<String, ApiFailure> {
    session
        .user
        .id
        .typed()
        .ok()
        .cloned()
        .filter(|user_id| !user_id.is_empty())
        .ok_or_else(|| {
            failure(
                StatusCode::UNAUTHORIZED,
                "SESSION",
                "The session carries no user identifier.",
            )
        })
}

async fn complete(
    State(state): State<AppState>,
    session: CurrentSession<AppAuthSchema>,
) -> Result<AxumJson<OnboardingStatus>, ApiFailure> {
    let user_id = user_id(&session)?;
    state
        .app_db
        .complete_onboarding(&user_id)
        .await
        .map_err(|error| {
            tracing::error!(%error, %user_id, "onboarding completion failed");
            failure(
                StatusCode::INTERNAL_SERVER_ERROR,
                "ONBOARDING",
                "An error has occurred. Try again later.",
            )
        })?;
    Ok(AxumJson(OnboardingStatus { completed: true }))
}
