mod hooks;
mod schema;
mod verification;

pub use hooks::init_profile_cache;

use std::sync::Arc;

use better_auth::plugins::oauth::{OAuthPlugin, OAuthProvider};
use better_auth::plugins::{
    ApiKeyPlugin, EmailPasswordPlugin, EmailVerificationPlugin, OrganizationPlugin,
    SessionManagementPlugin,
};
use better_auth::seaorm::sea_orm::ConnectionTrait;
use better_auth::seaorm::{Database, DatabaseConnection, SeaOrmStore};
use better_auth::{AuthConfig, BetterAuth};

pub use schema::AppAuthSchema;

use crate::config::Settings;
use crate::services::workspace::DEFAULT_WORKSPACE_ROLE;

pub fn auth_config(settings: &Settings) -> AuthConfig {
    AuthConfig::new(settings.secret.clone())
        .app_name("Argus Agent")
        .base_url(settings.base_url.clone())
        .base_path("/api/v1/auth")
        .trusted_origin(settings.dashboard_url.clone())
        .password_min_length(8)
        .disabled_path("/organization/create")
}

pub async fn connect_database(
    settings: &Settings,
) -> Result<DatabaseConnection, Box<dyn std::error::Error>> {
    let mut options =
        better_auth::seaorm::sea_orm::ConnectOptions::new(settings.database_url.clone());
    options
        .max_connections(settings.db_max_connections)
        .min_connections(1)
        .connect_timeout(std::time::Duration::from_secs(30))
        .acquire_timeout(std::time::Duration::from_secs(30))
        .idle_timeout(std::time::Duration::from_secs(8 * 60))
        .max_lifetime(std::time::Duration::from_secs(30 * 60))
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

pub async fn run_migrations(
    database: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    let engine = better_auth::seaorm::sea_orm::Schema::new(database.get_database_backend());
    macro_rules! migrate_from {
        ($entity:ident) => {{
            let statement = engine
                .create_table_from_entity(schema::$entity::Entity)
                .if_not_exists()
                .to_owned();
            let _ = database.execute(&statement).await?;
        }};
    }
    migrate_from!(user);
    migrate_from!(session);
    migrate_from!(account);
    migrate_from!(verification);
    migrate_from_api_keys(&engine, database).await?;
    migrate_from_organization(&engine, database).await?;
    ensure_workspace_columns(database).await?;
    ensure_auth_indexes(database).await;
    Ok(())
}

async fn ensure_auth_indexes(database: &DatabaseConnection) {
    for statement in [
        "CREATE INDEX IF NOT EXISTS sessions_token_idx ON sessions (token)",
        "CREATE INDEX IF NOT EXISTS sessions_user_idx ON sessions (user_id)",
        "CREATE INDEX IF NOT EXISTS accounts_user_provider_idx ON accounts (user_id, provider_id)",
        "CREATE INDEX IF NOT EXISTS verifications_identifier_idx ON verifications (identifier)",
        "CREATE INDEX IF NOT EXISTS member_user_idx ON member (user_id)",
        "CREATE INDEX IF NOT EXISTS api_keys_reference_idx ON api_keys (reference_id)",
        "CREATE UNIQUE INDEX IF NOT EXISTS users_email_key ON users (lower(email))",
        "CREATE UNIQUE INDEX IF NOT EXISTS sessions_token_key ON sessions (token)",
        "CREATE UNIQUE INDEX IF NOT EXISTS api_keys_key_key ON api_keys (key)",
    ] {
        if let Err(error) = database.execute_unprepared(statement).await {
            tracing::warn!(%error, %statement, "auth index creation skipped");
        }
    }
}

async fn migrate_from_organization(
    engine: &better_auth::seaorm::sea_orm::Schema,
    database: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    use better_auth_seaorm::store::entities::{invitation, member, organization};

    macro_rules! migrate_bundled {
        ($module:ident) => {{
            let statement = engine
                .create_table_from_entity($module::Entity)
                .if_not_exists()
                .to_owned();
            let _ = database.execute(&statement).await?;
        }};
    }
    migrate_bundled!(organization);
    migrate_bundled!(member);
    migrate_bundled!(invitation);
    Ok(())
}

async fn ensure_workspace_columns(
    database: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    for statement in [
        "ALTER TABLE IF EXISTS sessions ADD COLUMN IF NOT EXISTS active_organization_id TEXT",
        "ALTER TABLE IF EXISTS sessions ADD COLUMN IF NOT EXISTS active_team_id TEXT",
        "CREATE UNIQUE INDEX IF NOT EXISTS organization_slug_key ON organization (slug)",
    ] {
        database.execute_unprepared(statement).await?;
    }
    Ok(())
}

async fn migrate_from_api_keys(
    engine: &better_auth::seaorm::sea_orm::Schema,
    database: &DatabaseConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    use better_auth_seaorm::store::entities::api_key;

    let statement = engine
        .create_table_from_entity(api_key::Entity)
        .if_not_exists()
        .to_owned();
    let _ = database.execute(&statement).await?;
    Ok(())
}

pub async fn build_auth(
    settings: &Settings,
    store: SeaOrmStore<AppAuthSchema>,
) -> Result<(Arc<BetterAuth<AppAuthSchema>>, Arc<ApiKeyPlugin>), better_auth::AuthError> {
    let sender = verification::ResendVerificationSender::new(settings, ::reqwest::Client::new());
    let config = auth_config(settings);

    let api_keys = ApiKeyPlugin::builder()
        .prefix("argus_pk_".to_owned())
        .enable_session_for_api_keys(false)
        .rate_limit(better_auth::plugins::api_key::RateLimitDefaults {
            enabled: true,
            time_window: 86_400_000.0,
            max_requests: 1000.0,
        })
        .build();

    let mut oauth = OAuthPlugin::new();
    if let (Some(id), Some(secret)) = (&settings.google_client_id, &settings.google_client_secret) {
        oauth = oauth.add_provider("google", OAuthProvider::google(id, secret));
        tracing::info!("OAuth provider enabled: google");
    }
    if let (Some(id), Some(secret)) = (&settings.github_client_id, &settings.github_client_secret) {
        oauth = oauth.add_provider("github", OAuthProvider::github(id, secret));
        tracing::info!("OAuth provider enabled: github");
    }

    let auth = BetterAuth::<AppAuthSchema>::new(config)
        .store(store)
        .rate_limit(better_auth::middleware::RateLimitConfig {
            enabled: Some(true),
            window: Some(60.0),
            max_requests: Some(20.0),
            ..Default::default()
        })
        .plugin(
            EmailPasswordPlugin::new()
                .enable_signup(true)
                .require_email_verification(true),
        )
        .plugin(
            EmailVerificationPlugin::new()
                .send_on_sign_in(true)
                .custom_send_verification_email(Arc::new(sender)),
        )
        .plugin(oauth)
        .plugin(SessionManagementPlugin::new())
        .plugin(OrganizationPlugin::new().creator_role(DEFAULT_WORKSPACE_ROLE))
        .plugin(api_keys.clone())
        .build()
        .await?;

    Ok((Arc::new(auth), Arc::new(api_keys)))
}

pub fn make_store(
    config: better_auth::AuthConfig,
    database: DatabaseConnection,
) -> SeaOrmStore<AppAuthSchema> {
    SeaOrmStore::<AppAuthSchema>::new(config, database).hook(hooks::InitialAvatarHook)
}
