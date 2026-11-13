use std::sync::Arc;

use axum::extract::FromRef;
use better_auth::BetterAuth;
use better_auth::plugins::ApiKeyPlugin;

use crate::api::ingress::live::LiveSessionStore;
use crate::auth::AppAuthSchema;
use crate::config::Settings;
use crate::services::app_db::AppDb;
use crate::services::cache::Cache;
use crate::services::catalog::CatalogCache;
use crate::services::dodo::{BillingCatalog, DodoClient};
use crate::services::search::SearchRouter;
use crate::services::usage::UsageRecorder;

#[derive(Clone)]
pub struct AppState {
    pub settings: Settings,
    pub auth: Arc<BetterAuth<AppAuthSchema>>,
    pub api_keys: Arc<ApiKeyPlugin>,
    pub app_db: AppDb,
    pub catalog: CatalogCache,
    pub billing_catalog: Arc<BillingCatalog>,
    pub dodo: Option<Arc<DodoClient>>,
    pub cache: Cache,
    pub usage: UsageRecorder,
    pub search_router: Arc<SearchRouter>,
    pub live_sessions: Arc<LiveSessionStore>,
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
