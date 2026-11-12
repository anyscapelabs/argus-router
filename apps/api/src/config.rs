use std::collections::HashSet;
use std::env;

#[derive(Clone)]
pub struct Settings {
    pub secret: String,
    pub base_url: String,
    pub dashboard_url: String,
    pub database_url: String,
    pub redis_url: Option<String>,
    pub redis_token: Option<String>,
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
    pub github_client_id: Option<String>,
    pub github_client_secret: Option<String>,
    pub resend_api_key: Option<String>,
    pub resend_from: String,
    pub gemini_api_key: Option<String>,
    pub dodo_api_key: Option<String>,
    pub dodo_webhook_key: Option<String>,
    pub dodo_test_mode: bool,
    pub dodo_return_url: Option<String>,
    pub extra_allowed_origins: Vec<String>,
    admin_user_ids: HashSet<String>,
    pub db_max_connections: u32,
    admin_emails: HashSet<String>,
}

impl Settings {
    pub fn from_env() -> Self {
        let secret = env::var("BETTER_AUTH_SECRET")
            .expect("BETTER_AUTH_SECRET must be set (at least 32 chars)");
        let extra_allowed_origins = env::var("ALLOWED_ORIGINS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
        let parse_set = |name: &str| -> HashSet<String> {
            env::var(name)
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(|v| v.to_lowercase())
                .collect()
        };
        let db_max_connections = env::var("API_DB_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .filter(|n| *n > 0 && *n <= 100)
            .unwrap_or(10);
        Self {
            secret,
            base_url: env::var("BETTER_AUTH_URL")
                .expect("BETTER_AUTH_URL must be set — see .env.example"),
            dashboard_url: env::var("DASHBOARD_URL")
                .expect("DASHBOARD_URL must be set — see .env.example"),
            database_url: env::var("DATABASE_URL")
                .expect("DATABASE_URL must be set (e.g. postgresql://...)"),
            redis_url: env::var("UPSTASH_REDIS_REST_URL")
                .ok()
                .filter(|v| !v.is_empty()),
            redis_token: env::var("UPSTASH_REDIS_REST_TOKEN")
                .ok()
                .filter(|v| !v.is_empty()),
            google_client_id: env::var("GOOGLE_CLIENT_ID").ok().filter(|v| !v.is_empty()),
            google_client_secret: env::var("GOOGLE_CLIENT_SECRET")
                .ok()
                .filter(|v| !v.is_empty()),
            github_client_id: env::var("GITHUB_CLIENT_ID").ok().filter(|v| !v.is_empty()),
            github_client_secret: env::var("GITHUB_CLIENT_SECRET")
                .ok()
                .filter(|v| !v.is_empty()),
            resend_api_key: env::var("RESEND_API_KEY").ok().filter(|v| !v.is_empty()),
            resend_from: env::var("RESEND_FROM")
                .unwrap_or_else(|_| "Argus Agent <onboarding@resend.dev>".into()),
            gemini_api_key: env::var("GEMINI_API_KEY").ok().filter(|v| !v.is_empty()),
            dodo_api_key: env::var("DODO_PAYMENTS_API_KEY")
                .ok()
                .filter(|v| !v.is_empty()),
            dodo_webhook_key: env::var("DODO_PAYMENTS_WEBHOOK_KEY")
                .ok()
                .filter(|v| !v.is_empty()),
            dodo_test_mode: env::var("DODO_PAYMENTS_ENVIRONMENT").as_deref() != Ok("live"),
            dodo_return_url: env::var("DODO_PAYMENTS_RETURN_URL")
                .ok()
                .filter(|v| !v.is_empty()),
            extra_allowed_origins,
            admin_user_ids: parse_set("ADMIN_USER_IDS"),
            admin_emails: parse_set("ADMIN_EMAILS"),
            db_max_connections,
        }
    }

    pub fn is_admin(&self, user_id: &str, email: Option<&str>) -> bool {
        self.admin_user_ids.contains(user_id)
            || email
                .map(|e| self.admin_emails.contains(&e.to_lowercase()))
                .unwrap_or(false)
    }
}
