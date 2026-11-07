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
}

impl Settings {
    pub fn from_env() -> Self {
        let secret = env::var("BETTER_AUTH_SECRET")
            .expect("BETTER_AUTH_SECRET must be set (at least 32 chars)");
        Self {
            secret,
            base_url: env::var("BETTER_AUTH_URL")
                .expect("BETTER_AUTH_URL must be set — see .env.example"),
            dashboard_url: env::var("DASHBOARD_URL")
                .expect("DASHBOARD_URL must be set — see .env.example"),
            database_url: env::var("DATABASE_URL")
                .expect("DATABASE_URL must be set (e.g. postgresql://...)"),
            redis_url: env::var("UPSTASH_REDIS_REST_URL").ok().filter(|v| !v.is_empty()),
            redis_token: env::var("UPSTASH_REDIS_REST_TOKEN").ok().filter(|v| !v.is_empty()),
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
        }
    }
}
