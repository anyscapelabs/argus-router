use axum::{Json as AxumJson, Router, routing::get};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(health))
}

async fn health() -> AxumJson<serde_json::Value> {
    AxumJson(serde_json::json!({ "ok": true }))
}
