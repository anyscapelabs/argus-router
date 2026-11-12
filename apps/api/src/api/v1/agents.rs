use axum::{Json as AxumJson, Router, routing::get};
use better_auth::integrations::axum::CurrentSession;

use crate::auth::AppAuthSchema;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(list_agents))
}

async fn list_agents(session: CurrentSession<AppAuthSchema>) -> AxumJson<serde_json::Value> {
    AxumJson(serde_json::json!({
        "owner": session.user.id,
        "agents": [],
    }))
}
