mod hooks;
mod schema;
mod verification;

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

/// Single source of truth for auth configuration; nested under the versioned
/// API so links, callbacks and cookies agree with the router tree.
pub fn auth_config(settings: &Settings) -> AuthConfig {
    AuthConfig::new(settings.secret.clone())
        .app_name("Argus Agent")
        .base_url(settings.base_url.clone())
        .base_path("/api/v1/auth")
        .trusted_origin(settings.dashboard_url.clone())
        .password_min_length(8)
}

pub async fn connect_database(
    settings: &Settings,
) -> Result<DatabaseConnection, Box<dyn std::error::Error>> {
    // One shared sqlx pool for the whole process. Neon (pgbouncer) caps the
    // total connections per compute, so keep the pool small; the acquisition
    // timeout covers cold-start latency on a pooled connection.
    let mut options = better_auth::seaorm::sea_orm::ConnectOptions::new(settings.database_url.clone());
    options
        .max_connections(5)
        .min_connections(1)
        .connect_timeout(std::time::Duration::from_secs(30))
        .acquire_timeout(std::time::Duration::from_secs(30))
        .idle_timeout(std::time::Duration::from_secs(8 * 60))
        .max_lifetime(std::time::Duration::from_secs(30 * 60))
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

pub async fn run_migrations(database: &DatabaseConnection) -> Result<(), Box<dyn std::error::Error>> {
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
    // The ApiKeyPlugin persists keys in its own canonical table.
    migrate_from_api_keys(&engine, database).await?;
    // Workspaces are better-auth organizations; see `services::workspace`.
    migrate_from_organization(&engine, database).await?;
    ensure_workspace_columns(database).await?;
    Ok(())
}

/// Workspace tables, created from the organization plugin's own bundled
/// entities so the store's column mapping always agrees with what it queries.
/// Teams stay disabled and dynamic roles are off, so `team`/`team_member`/
/// `organization_role` are deliberately not created.
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

/// Postgres-only, idempotent DDL for databases created before workspaces
/// existed (the deployment is Neon Postgres, matching `AppDb`).
///
/// `create_table_from_entity` skips a table that is already there, so the two
/// organization columns the plugin needs on `sessions` are added in place, and
/// the slug uniqueness the bundled entity does not declare gets its index —
/// that index is what makes two concurrent first reads of the same account's
/// default workspace safe.
async fn ensure_workspace_columns(database: &DatabaseConnection) -> Result<(), Box<dyn std::error::Error>> {
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

/// Builds the better-auth instance: email/password, signed-up verification via
/// Resend, sessions, and the optional social providers enabled through the
/// environment.
pub async fn build_auth(
    settings: &Settings,
    store: SeaOrmStore<AppAuthSchema>,
) -> Result<(Arc<BetterAuth<AppAuthSchema>>, Arc<ApiKeyPlugin>), better_auth::AuthError> {
    let sender = verification::ResendVerificationSender::new(settings, ::reqwest::Client::new());
    let config = auth_config(settings);

    // Agent/Desktop long-lived keys (`x-api-key` sessions) for the Argus app.
    // The same instance is registered in the auth stack and shared through
    // AppState for machine-facing domain routes.
    let api_keys = ApiKeyPlugin::builder()
        .prefix("argus_pk_".to_owned())
        .enable_session_for_api_keys(true)
        .rate_limit(better_auth::plugins::api_key::RateLimitDefaults {
            enabled: true,
            time_window: 86_400_000.0, // 24 hours
            max_requests: 1000.0,
        })
        .build();

    let mut oauth = OAuthPlugin::new();
    if let (Some(id), Some(secret)) =
        (&settings.google_client_id, &settings.google_client_secret)
    {
        oauth = oauth.add_provider("google", OAuthProvider::google(id, secret));
        tracing::info!("OAuth provider enabled: google");
    }
    if let (Some(id), Some(secret)) =
        (&settings.github_client_id, &settings.github_client_secret)
    {
        oauth = oauth.add_provider("github", OAuthProvider::github(id, secret));
        tracing::info!("OAuth provider enabled: github");
    }

    let auth = BetterAuth::<AppAuthSchema>::new(config)
        .store(store)
        .plugin(EmailPasswordPlugin::new().enable_signup(true))
        .plugin(EmailVerificationPlugin::new().custom_send_verification_email(Arc::new(sender)))
        .plugin(oauth)
        .plugin(SessionManagementPlugin::new())
        // Workspaces: every account gets one "Default" organization, created
        // lazily by `services::workspace` on first read — never seeded at boot.
        // Registered ahead of the API key plugin so keys can resolve an
        // organization actor once a workspace is shared.
        .plugin(OrganizationPlugin::new().creator_role(DEFAULT_WORKSPACE_ROLE))
        .plugin(api_keys.clone())
        .build()
        .await?;

    Ok((Arc::new(auth), Arc::new(api_keys)))
}

/// Store assembly applying the SeaORM-side lifecycle hooks (initial avatars).
pub fn make_store(config: better_auth::AuthConfig, database: DatabaseConnection) -> SeaOrmStore<AppAuthSchema> {
    SeaOrmStore::<AppAuthSchema>::new(config, database).hook(hooks::InitialAvatarHook)
}
