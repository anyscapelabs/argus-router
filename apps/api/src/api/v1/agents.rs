use axum::{Json as AxumJson, Router, routing::get};
use better_auth::integrations::axum::CurrentSession;

use crate::auth::AppAuthSchema;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(list_agents))
}

/// Agents owned by the signed-in operator (`GET /api/v1/agents`). Placeholder
/// for the agents domain — returns the owner id with an empty list until the
/// agent routing model lands.
async fn list_agents(session: CurrentSession<AppAuthSchema>) -> AxumJson<serde_json::Value> {
    AxumJson(serde_json::json!({
        "owner": session.user.id,
        "agents": [],
    }))
}
