use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanLimits {
    pub monthly_tokens: String,
    pub weekly_tokens: String,
    pub daily_requests: String,
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

}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TierLimits {
    pub monthly_tokens: String,
    pub weekly_tokens: String,
}

impl TierLimits {
    pub fn monthly_tokens_count(&self) -> i64 {
        PlanLimits::count(&self.monthly_tokens)
    }

    pub fn weekly_tokens_count(&self) -> i64 {
        PlanLimits::count(&self.weekly_tokens)
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
    pub compute_budget_usd: f64,
    #[serde(default)]
    pub allowance_usd: f64,
    #[serde(default)]
    pub weekly_fraction: f64,
    #[serde(default)]
    pub groups: Vec<String>,
    #[serde(default)]
    pub group_caps: HashMap<String, f64>,
    #[serde(default)]
    pub limits: Option<PlanLimits>,
    #[serde(default)]
    pub tier_limits: Option<HashMap<String, TierLimits>>,
    pub tiers: Vec<String>,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub model_names: Vec<String>,
    #[serde(default)]
    pub model_count: String,
}

pub fn expand_groups(
    mut plans: Vec<PlanCatalogEntry>,
    catalog: &[crate::services::catalog::ModelCatalogEntry],
) -> Vec<PlanCatalogEntry> {
    let auto = crate::services::catalog::AUTO_MODEL_ID;
    for plan in &mut plans {
        let selected_groups = if plan.groups.is_empty() {
            &plan.tiers
        } else {
            &plan.groups
        };
        if plan.models.is_empty() {
            plan.models = catalog
                .iter()
                .filter(|entry| {
                    !entry.is_blocked()
                        && selected_groups
                            .iter()
                            .any(|group| group == entry.group.as_str())
                })
                .map(|entry| entry.id.clone())
                .collect();
        }
        plan.models.retain(|model| {
            if model == auto {
                return true;
            }
            catalog
                .iter()
                .find(|entry| &entry.id == model)
                .is_some_and(|entry| !entry.is_blocked())
        });
        if catalog.iter().any(|entry| entry.id == auto) && !plan.models.iter().any(|m| m == auto) {
            plan.models.push(auto.to_owned());
        }
        plan.model_names = catalog
            .iter()
            .filter(|entry| plan.models.iter().any(|m| m == &entry.id))
            .map(|entry| entry.name.clone())
            .collect();
        plan.model_count = format!("{} models", plan.models.len());
    }
    plans
}

pub fn expand_tiers(
    plans: Vec<PlanCatalogEntry>,
    catalog: &[crate::services::catalog::ModelCatalogEntry],
) -> Vec<PlanCatalogEntry> {
    expand_groups(plans, catalog)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_plans_parse_with_dollar_allowances() {
        let plans = seed_entries();
        assert!(plans.iter().any(|plan| plan.id == "max"));
        assert!(plans.iter().any(|plan| plan.id == "max-plus"));
        let starter = plans
            .iter()
            .find(|plan| plan.id == "starter")
            .expect("starter plan");
        assert_eq!(starter.price, "5");
        assert_eq!(starter.searches_per_month, 60);
        assert_eq!(starter.voice_minutes, 10);
        assert_eq!(starter.compute_budget_usd, 3.0);
        assert_eq!(starter.allowance_usd, 2.53);
        assert_eq!(starter.weekly_fraction, 0.4);
        assert_eq!(starter.groups, vec!["lite", "standard"]);
    }

    #[test]
    fn aggregate_limits_equal_group_sums() {
        for plan in seed_entries() {
            let Some(tier_limits) = &plan.tier_limits else {
                continue;
            };
            let limits = plan.limits.as_ref().expect("aggregate limits");
            let tier_monthly: i64 = tier_limits.values().map(|t| t.monthly_tokens_count()).sum();
            let tier_weekly: i64 = tier_limits.values().map(|t| t.weekly_tokens_count()).sum();
            assert_eq!(
                limits.monthly_tokens_count(),
                tier_monthly,
                "plan {} monthly mismatch",
                plan.id
            );
            assert_eq!(
                limits.weekly_tokens_count(),
                tier_weekly,
                "plan {} weekly mismatch",
                plan.id
            );
        }
    }
}
