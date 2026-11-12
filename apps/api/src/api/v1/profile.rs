use axum::{Json as AxumJson, Router, extract::State, routing::get};
use better_auth::integrations::axum::CurrentSession;
use serde::{Deserialize, Serialize};

use crate::auth::AppAuthSchema;
use crate::services::cache::Cache;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(me))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub image: Option<String>,
    pub email_verified: bool,
}

async fn me(
    State(cache): State<Cache>,
    session: CurrentSession<AppAuthSchema>,
) -> AxumJson<Profile> {
    let user = &session.user;
    let id = user.id.typed().map(|s| s.clone()).unwrap_or_default();
    let key = format!("argus:profile:{id}");
    if let Some(cached) = cache.get::<Profile>(&key).await {
        return AxumJson(cached);
    }
    let profile = Profile {
        id: id.clone(),
        email: user.email.typed().ok().cloned().unwrap_or(None),
        name: user.name.typed().ok().cloned().unwrap_or(None),
        image: user.image.typed().ok().cloned().unwrap_or(None),
        email_verified: user.email_verified.typed().ok().copied().unwrap_or(false),
    };
    cache.set(&key, &profile, 60).await;
    AxumJson(profile)
}
