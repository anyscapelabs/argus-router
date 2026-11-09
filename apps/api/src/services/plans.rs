use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanLimits {
    pub monthly_tokens: String,
    pub weekly_tokens: String,
    pub daily_requests: String,
    pub rate_per_min: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanCatalogEntry {
    pub id: String,
    pub name: String,
    pub price: String,
    #[serde(default)]
    pub searches_per_month: i64,
    #[serde(default)]
    pub voice_minutes: i64,
    pub models: Vec<String>,
    #[serde(default)]
    pub model_names: Vec<String>,
    #[serde(default)]
    pub model_count: String,
}

const EMBEDDED: &str = include_str!("../../plans/plans.json");

pub fn seed_entries() -> Vec<PlanCatalogEntry> {
    if let Ok(path) = std::env::var("PLANS_PATH") {
        if !path.trim().is_empty() {
            if let Ok(body) = std::fs::read_to_string(path.trim()) {
                if let Ok(entries) = serde_json::from_str(&body) {
                    return entries;
                }
            }
        }
    }
    serde_json::from_str(EMBEDDED).unwrap_or_default()
}