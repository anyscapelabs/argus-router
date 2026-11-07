use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PlanCatalogEntry {
    pub id: String,
    pub name: String,
    pub price: String,
    pub models: Vec<String>,
}

const EMBEDDED: &str = include_str!("../../plans/plans.json");

pub fn seed_entries() -> Vec<PlanCatalogEntry> {
    serde_json::from_str(EMBEDDED).unwrap_or_default()
}