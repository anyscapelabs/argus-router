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

fn user_id(session: &CurrentSession<AppAuthSchema>) -> String {
    session.user.id.typed().ok().cloned().unwrap_or_default()
}

async fn complete(
    State(state): State<AppState>,
    session: CurrentSession<AppAuthSchema>,
) -> AxumJson<OnboardingStatus> {
    if let Err(error) = state
        .app_db
        .complete_onboarding(&user_id(&session))
        .await
    {
        tracing::error!(%error, "onboarding completion failed");
    }
    AxumJson(OnboardingStatus { completed: true })
}
