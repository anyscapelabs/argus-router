use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PlanCatalogEntry {
    pub id: String,
    pub name: String,
    pub price: String,
    #[serde(default)]
    pub searches_per_month: i64,
    pub models: Vec<String>,
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