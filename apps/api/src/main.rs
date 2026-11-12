use api::api as gateway;
use api::{auth, services};

use std::sync::Arc;

use axum::Router;
use better_auth::BetterAuth;
use better_auth::integrations::axum::AxumIntegration;
use tower_http::cors::{AllowHeaders, AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;

use api::auth::AppAuthSchema;
use api::config::Settings;
use api::services::cache::Cache;
use api::state::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tower_http=debug".into()),
        )
        .init();

    let settings = Settings::from_env();
    let database = auth::connect_database(&settings).await?;
    auth::run_migrations(&database).await?;
    let app_db =
        services::app_db::AppDb::from_pool(database.get_postgres_connection_pool().clone()).await?;
    let usage = services::usage::UsageRecorder::new(app_db.pool());
    let seeded = app_db
        .load_catalog(&services::catalog::seed_entries())
        .await?;
    let catalog = services::catalog::CatalogCache::new(seeded.clone());
    tracing::info!(entries = catalog.snapshot().len(), "model catalog loaded");

    let billing_catalog = Arc::new(services::dodo::BillingCatalog::load()?);
    let dodo = match (
        settings.dodo_api_key.clone(),
        settings.dodo_webhook_key.as_ref(),
    ) {
        (Some(key), Some(_)) => Some(Arc::new(services::dodo::DodoClient::new(
            key,
            settings.dodo_test_mode,
        ))),
        (Some(_), None) => {
            tracing::warn!("Dodo Payments API key is set without a webhook key; checkout disabled");
            None
        }
        _ => None,
    };

    let plans = services::plans::expand_tiers(services::plans::seed_entries(), &seeded);
    if let Err(error) = app_db.seed_plans(&plans).await {
        tracing::warn!(%error, "plan seeding failed");
    }
    if let Err(error) = app_db.seed_plan_allowlists(&plans).await {
        tracing::warn!(%error, "plan allowlist seeding failed");
    }

    let config = auth::auth_config(&settings);
    let store = auth::make_store(config, database.clone());
    let (auth, api_keys) = auth::build_auth(&settings, store).await?;
    let cache = Cache::new(
        settings.redis_url.as_deref(),
        settings.redis_token.as_deref(),
    );
    auth::init_profile_cache(cache.clone());

    tracing::info!(
        base = %settings.base_url,
        "Anyscape Gateway API listening on 0.0.0.0:8080"
    );

    let search_router = Arc::new(services::search::SearchRouter::new());
    let live_sessions = Arc::new(gateway::ingress::live::LiveSessionStore::default());
    let rate_window = Arc::new(services::rate::RateWindow::default());

    let state = AppState {
        settings: settings.clone(),
        auth: auth.clone(),
        api_keys,
        app_db,
        catalog,
        billing_catalog,
        dodo,
        cache,
        usage,
        search_router,
        rate_window,
        live_sessions,
        db: database,
    };

    let maintenance_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60 * 60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if let Err(error) = maintenance_state
                .app_db
                .reclaim_expired_generation_reservations()
                .await
            {
                tracing::error!(%error, "expired generation reservation cleanup failed");
            }
            gateway::v1::billing::reconcile_subscriptions(&maintenance_state).await;
        }
    });

    let app = router(auth)
        .layer(TraceLayer::new_for_http().make_span_with(api::observe::redacted_span))
        .layer(cors(&settings))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn router(auth: Arc<BetterAuth<AppAuthSchema>>) -> Router<AppState> {
    Router::new()
        .nest("/api/v1/auth", auth.axum_router_with_state::<AppState>())
        .nest("/api/v1", gateway::v1::router())
        .route(
            "/v1/chat/completions",
            axum::routing::post(gateway::ingress::openai_chat_completions),
        )
        .route(
            "/v1/models",
            axum::routing::get(gateway::ingress::openai_models),
        )
        .route(
            "/v1/search",
            axum::routing::post(gateway::ingress::search_endpoint),
        )
        .route(
            "/v1beta/models/{model_action}",
            axum::routing::post(gateway::ingress::gemini_models_action),
        )
        .route(
            "/v1beta/models",
            axum::routing::get(gateway::ingress::gemini_models),
        )
        .route(
            "/v1/live",
            axum::routing::get(gateway::ingress::live::live_websocket_handler),
        )
        .route(
            "/v1/live/last-reply",
            axum::routing::get(gateway::ingress::live::last_reply_handler),
        )
}

fn cors(settings: &Settings) -> CorsLayer {
    let mut origins: Vec<axum::http::HeaderValue> = Vec::new();
    if let Ok(v) = settings.dashboard_url.parse() {
        origins.push(v);
    }
    for extra in &settings.extra_allowed_origins {
        if let Ok(v) = extra.parse() {
            origins.push(v);
        }
    }
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::PATCH,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
        ])
        .allow_headers(AllowHeaders::mirror_request())
        .allow_credentials(true)
        .allow_private_network(true)
}
