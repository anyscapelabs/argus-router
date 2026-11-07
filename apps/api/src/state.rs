use std::sync::Arc;

use axum::extract::FromRef;
use better_auth::BetterAuth;
use better_auth::plugins::ApiKeyPlugin;

use crate::auth::AppAuthSchema;
use crate::config::Settings;
use crate::services::app_db::AppDb;
use crate::services::cache::Cache;
use crate::services::catalog::CatalogCache;
use crate::services::usage::UsageRecorder;

/// Shared application state, resolved per-route via `FromRef`.
#[derive(Clone)]
pub struct AppState {
    pub settings: Settings,
    pub auth: Arc<BetterAuth<AppAuthSchema>>,
    /// Same ApiKeyPlugin instance registered in the auth stack — used by
    /// machine-facing domain routes to verify `x-api-key` credentials.
    pub api_keys: Arc<ApiKeyPlugin>,
    /// App-owned SQLx pool (plans, entitlements, allowlists).
    pub app_db: AppDb,
    /// Dynamic model catalog (model_catalog table), live-cache with refresh.
    pub catalog: CatalogCache,
    /// Upstash Redis cache for hot reads (profile, ...); no-ops unconfigured.
    pub cache: Cache,
    /// Batched usage recorder (never stores prompt content).
    pub usage: UsageRecorder,
    pub db: better_auth::seaorm::DatabaseConnection,
}

impl FromRef<AppState> for Arc<BetterAuth<AppAuthSchema>> {
    fn from_ref(state: &AppState) -> Self {
        state.auth.clone()
    }
}

impl FromRef<AppState> for Cache {
    fn from_ref(state: &AppState) -> Self {
        state.cache.clone()
    }
}
