use axum::{Router, routing::post};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/", post(crate::api::ingress::search_endpoint))
}
