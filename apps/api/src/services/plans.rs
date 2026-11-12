use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanLimits {
    pub monthly_tokens: String,
    pub weekly_tokens: String,
    pub daily_requests: String,
    pub rate_per_min: String,
}

impl PlanLimits {
    fn count(display: &str) -> i64 {
        let token = display
            .trim()
            .split(|c: char| c.is_whitespace() || c == '/' || c == '_')
            .find(|t| !t.is_empty())
            .unwrap_or("")
            .replace(',', "");
        let (digits, suffix) = token.split_at(
            token
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(token.len()),
        );
        let base: i64 = digits.parse().unwrap_or(0);
        let scale = match suffix.to_ascii_uppercase().as_str() {
            "K" => 1_000,
            "M" => 1_000_000,
            "B" | "G" => 1_000_000_000,
            _ => 1,
        };
        base.saturating_mul(scale)
    }

    pub fn monthly_tokens_count(&self) -> i64 {
        Self::count(&self.monthly_tokens)
    }

    pub fn weekly_tokens_count(&self) -> i64 {
        Self::count(&self.weekly_tokens)
    }

    pub fn daily_requests_count(&self) -> i64 {
        Self::count(&self.daily_requests)
    }

    pub fn rate_per_min_count(&self) -> i64 {
        Self::count(&self.rate_per_min)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanCatalogEntry {
    pub id: String,
    pub name: String,
    pub price: String,
    #[serde(default)]
    pub tagline: String,
    #[serde(default)]
    pub searches_per_month: i64,
    #[serde(default)]
    pub voice_minutes: i64,
    #[serde(default)]
    pub limits: Option<PlanLimits>,
    pub tiers: Vec<String>,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub model_names: Vec<String>,
    #[serde(default)]
    pub model_count: String,
}

pub fn expand_tiers(
    mut plans: Vec<PlanCatalogEntry>,
    catalog: &[crate::services::catalog::ModelCatalogEntry],
) -> Vec<PlanCatalogEntry> {
    for plan in &mut plans {
        plan.models = catalog
            .iter()
            .filter(|entry| plan.tiers.iter().any(|t| t == entry.tier.as_str()))
            .map(|entry| entry.id.clone())
            .collect();
        plan.model_names = catalog
            .iter()
            .filter(|entry| plan.models.iter().any(|m| m == &entry.id))
            .map(|entry| entry.name.clone())
            .collect();
        plan.model_count = format!("{} models", plan.models.len());
    }
    plans
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
