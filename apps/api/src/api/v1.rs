pub mod account;
pub mod agents;
pub mod billing;
pub mod health;
pub mod models;
pub mod plans;
pub mod profile;
pub mod search;
pub mod settings;
pub mod usage;
pub mod workspace;

use axum::Router;

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .nest("/account", account::router())
        .nest("/agents", agents::router())
        .nest("/billing", billing::router())
        .nest("/health", health::router())
        .nest("/models", models::router())
        .nest("/plans", plans::router())
        .nest("/profile", profile::router())
        .nest("/search", search::router())
        .nest("/settings", settings::router())
        .nest("/usage", usage::router())
        .nest("/workspace", workspace::router())
}
