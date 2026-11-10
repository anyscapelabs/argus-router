mod api;
mod auth;
mod config;
mod proxy;
mod services;
mod state;

use std::sync::Arc;

use axum::Router;
use better_auth::BetterAuth;
use better_auth::integrations::axum::AxumIntegration;
use tower_http::cors::{AllowHeaders, AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::auth::AppAuthSchema;
use crate::config::Settings;
use crate::services::cache::Cache;
use crate::state::AppState;

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
    let app_db = services::app_db::AppDb::connect(&settings).await?;
    let usage = services::usage::UsageRecorder::new(app_db.pool());
    // The dynamic catalog: seeded from the embedded JSON into the
    // `model_catalog` table, then loaded from Postgres into the live cache
    // (`POST /api/v1/models/refresh` re-seeds it from our catalog without a
    // redeploy). OpenRouter is used only as the upstream origin for model
    // calls, never as the catalog source.
    let seeded = app_db
        .load_catalog(&services::catalog::seed_entries())
        .await?;
    let catalog = services::catalog::CatalogCache::new(seeded);
    tracing::info!(entries = catalog.snapshot().len(), "model catalog loaded");

    let plans = services::plans::seed_entries();
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

    tracing::info!(
        base = %settings.base_url,
        "Argus router API listening on 0.0.0.0:8080"
    );

    let search_router = Arc::new(services::search::SearchRouter::new());
    let live_sessions = Arc::new(crate::api::ingress::live::LiveSessionStore::default());

    let state = AppState {
        settings: settings.clone(),
        auth: auth.clone(),
        api_keys,
        app_db,
        catalog,
        cache,
        usage,
        search_router,
        live_sessions,
        db: database,
    };

    let app = router(auth)
        .layer(
            // Same default span tower-http builds, except secret query params
            // are redacted before they reach the logs: WebSocket clients pass
            // their API key as `?key=`, and an unredacted URI would write every
            // caller's key to the log in plaintext.
            TraceLayer::new_for_http().make_span_with(redacted_span),
        )
        .layer(cors(&settings))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    axum::serve(listener, app).await?;
    Ok(())
}

/// Query parameters whose values must never reach the logs.
const REDACTED_QUERY_PARAMS: &[&str] = &["key", "api_key", "token", "access_token"];

/// The request URI with secret query-param values replaced by `[redacted]`.
/// WebSocket clients pass their API key as `?key=`, and the default tower-http
/// span would otherwise log the full URI — every caller's key, in plaintext.
fn redact_uri(uri: &axum::http::Uri) -> String {
    let path = uri.path();
    let Some(query) = uri.query() else {
        return path.to_owned();
    };
    let mut redacted = false;
    let cleaned: Vec<String> = query
        .split('&')
        .map(|pair| match pair.split_once('=') {
            Some((name, _)) if REDACTED_QUERY_PARAMS.contains(&name) => {
                redacted = true;
                format!("{name}=[redacted]")
            }
            _ => pair.to_owned(),
        })
        .collect();
    if !redacted {
        return path.to_owned();
    }
    format!("{path}?{}", cleaned.join("&"))
}

/// The default tower-http request span (method, uri, version) with the URI
/// passed through `redact_uri` first.
fn redacted_span<B>(request: &axum::http::Request<B>) -> tracing::Span {
    tracing::debug_span!(
        "request",
        method = %request.method(),
        uri = %redact_uri(request.uri()),
        version = ?request.version(),
    )
}

/// Domain/version tree: `/api/v1/<domain>` plus the better-auth surface
/// nested under `/api/v1/auth` (its base path is mirrored in `AuthConfig` so
/// emails, callbacks and cookies agree), and the model-proxy ingress
/// dialects mounted at their canonical SDK paths.
fn router(auth: Arc<BetterAuth<AppAuthSchema>>) -> Router<AppState> {    Router::new()
        .nest("/api/v1/auth", auth.axum_router_with_state::<AppState>())
        .nest("/api/v1", api::v1::router())
        // OpenAI-compatible ingress (the desktop agent's `openai_compat`
        // adapter points here with its service-account key).
        .route(
            "/v1/chat/completions",
            axum::routing::post(api::ingress::openai_chat_completions),
        )
        .route(
            "/v1/models",
            axum::routing::get(api::ingress::openai_models),
        )
        // Web search router ingress for AI agents & models.
        .route(
            "/v1/search",
            axum::routing::post(api::ingress::search_endpoint),
        )
        // Gemini-compatible ingress (`gemini` adapter points here).
        .route(
            "/v1beta/models/{model_action}",
            axum::routing::post(api::ingress::gemini_models_action),
        )
        .route(
            "/v1beta/models",
            axum::routing::get(api::ingress::gemini_models),
        )
        // Gemini Live speech-to-speech WebSocket ingress
        .route(
            "/v1/live",
            axum::routing::get(api::ingress::live::live_websocket_handler),
        )
        // Returns the last model reply for a session that ended early.
        // Desktop app polls this for voice→text fallback.
        .route(
            "/v1/live/last-reply",
            axum::routing::get(api::ingress::live::last_reply_handler),
        )
}

fn cors(settings: &Settings) -> CorsLayer {
    // Build the allowed-origins list: the dashboard URL is always included;
    // extra origins (e.g. tauri://localhost) come from the ALLOWED_ORIGINS env var.
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
