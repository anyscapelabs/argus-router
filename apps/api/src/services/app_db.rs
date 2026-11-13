use serde::Serialize;
use sqlx::PgPool;

mod billing;
mod catalog;
mod generation;
mod migrate;
mod models;
mod onboarding;
mod plans;
mod quota;
mod settings;

pub use billing::{SubscriptionRow, SubscriptionView};

#[derive(Clone)]
pub struct AppDb {
    pool: PgPool,
}

impl AppDb {
    pub fn pool(&self) -> PgPool {
        self.pool.clone()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PlanQuota {
    pub monthly_tokens: i64,
    pub weekly_tokens: i64,
    pub daily_requests: i64,
}

#[derive(Clone, Debug)]
pub struct GenerationReservation {
    pub id: String,
    pub max_output_tokens: u64,
    pub reserved_tokens: i64,
    pub held_micros: i64,
}

#[derive(Clone, Debug)]
pub enum GenerationReservationOutcome {
    Reserved(GenerationReservation),
    MonthlyExhausted,
    CreditsExhausted,
}

#[derive(Clone, Debug)]
pub enum CheckoutClaim {
    Claimed,
    Existing(String),
    InProgress,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanView {
    pub id: String,
    pub name: String,
    pub price: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tagline: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<crate::services::plans::PlanLimits>,
    #[serde(default)]
    pub searches_per_month: i64,
    #[serde(default)]
    pub voice_minutes: i64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub model_names: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model_count: String,
    pub models: Vec<String>,
}

pub const FREE_PLAN_ID: &str = "free";

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserSettings {
    pub allow_free_training: bool,
    pub max_key_lifetime_days: i64,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            allow_free_training: true,
            max_key_lifetime_days: 0,
        }
    }
}

#[derive(Clone, Debug)]
pub enum QuotaCheck {
    Allowed { used: i64, quota: i64 },
    NotAvailable,
    Exceeded { used: i64, quota: i64 },
}
