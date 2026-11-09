pub mod account;
pub mod agents;
pub mod health;
pub mod models;
pub mod plans;
pub mod profile;
pub mod search;
pub mod usage;
pub mod workspace;

use axum::Router;

use crate::state::AppState;

/// Versioned, domain-partitioned API surface. Every handler sits under
/// `/api/v1/<domain>/...`; breaking changes get routed through a future
/// `/api/v2` instance instead of in-place rewrites.
pub fn router() -> Router<AppState> {
    Router::new()
        .nest("/account", account::router())
        .nest("/agents", agents::router())
        .nest("/health", health::router())
        .nest("/models", models::router())
        .nest("/plans", plans::router())
        .nest("/profile", profile::router())
        .nest("/search", search::router())
        .nest("/usage", usage::router())
        .nest("/workspace", workspace::router())
}
