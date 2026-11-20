use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

pub const AUTO_MODEL_ID: &str = "argus/auto";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Compat {
    Openai,
}

impl Compat {
    pub fn as_str(self) -> &'static str {
        match self {
            Compat::Openai => "openai",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Lite,
    Standard,
    Premium,
    Ultra,
}

impl Tier {
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Lite => "lite",
            Tier::Standard => "standard",
            Tier::Premium => "premium",
            Tier::Ultra => "ultra",
        }
    }

    pub fn below(self) -> Option<Tier> {
        match self {
            Tier::Ultra => Some(Tier::Premium),
            Tier::Premium => Some(Tier::Standard),
            Tier::Standard => Some(Tier::Lite),
            Tier::Lite => None,
        }
    }

    pub fn above(self) -> Option<Tier> {
        match self {
            Tier::Lite => Some(Tier::Standard),
            Tier::Standard => Some(Tier::Premium),
            Tier::Premium => Some(Tier::Ultra),
            Tier::Ultra => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Capabilities {
    pub tools: bool,
    pub vision: bool,
    pub reasoning: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Pricing {
    pub input: String,
    pub output: String,
    pub cache_read: Option<String>,
    pub cache_write: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModelCatalogEntry {
    pub id: String,
    pub name: String,
    pub compat: Compat,
    pub provider: String,
    pub upstream_model: String,
    #[serde(default)]
    pub provider_routing: Option<serde_json::Value>,
    pub context: i64,
    pub max_output: i64,
    pub capabilities: Capabilities,
    pub pricing: Pricing,
    #[serde(default)]
    pub free: bool,
    #[serde(default)]
    pub auto: bool,
    #[serde(default)]
    pub auto_priority: u32,
    pub tier: Tier,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelView {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub compat: Compat,
    pub context: i64,
    pub max_output: i64,
    pub capabilities: Capabilities,
    pub pricing: Pricing,
    pub free: bool,
    pub tier: Tier,
    pub enabled: bool,
    pub in_plan: bool,
}

impl ModelCatalogEntry {
    pub fn view(&self, enabled: bool, in_plan: bool) -> ModelView {
        ModelView {
            id: self.id.clone(),
            name: self.name.clone(),
            provider: self.provider.clone(),
            compat: self.compat,
            context: self.context,
            max_output: self.max_output,
            capabilities: self.capabilities.clone(),
            pricing: self.pricing.clone(),
            free: self.free,
            tier: self.tier,
            enabled,
            in_plan,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Clone)]
pub struct CatalogCache {
    inner: Arc<RwLock<Vec<ModelCatalogEntry>>>,
}

impl CatalogCache {
    pub fn new(entries: Vec<ModelCatalogEntry>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(entries)),
        }
    }

    pub fn replace(&self, entries: Vec<ModelCatalogEntry>) {
        *self.inner.write().expect("catalog lock") = entries;
    }

    pub fn snapshot(&self) -> Vec<ModelCatalogEntry> {
        self.inner.read().expect("catalog lock").clone()
    }

    pub fn find(&self, id: &str) -> Option<ModelCatalogEntry> {
        self.inner
            .read()
            .expect("catalog lock")
            .iter()
            .find(|entry| entry.id == id)
            .cloned()
    }

    pub fn find_by_upstream(&self, deployed_model: &str) -> Option<ModelCatalogEntry> {
        let normalized = deployed_model
            .strip_prefix("openrouter/")
            .unwrap_or(deployed_model);
        self.inner
            .read()
            .expect("catalog lock")
            .iter()
            .find(|entry| {
                entry.upstream_model == deployed_model || entry.upstream_model == normalized
            })
            .cloned()
    }
}

const EMBEDDED: &str = include_str!("../../catalog/models.json");

pub fn seed_entries() -> Vec<ModelCatalogEntry> {
    let mut entries = load_override().unwrap_or_else(embedded_catalog);
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    entries
}

fn embedded_catalog() -> Vec<ModelCatalogEntry> {
    parse_catalog(EMBEDDED, "embedded")
}

fn load_override() -> Option<Vec<ModelCatalogEntry>> {
    let path = std::env::var("CATALOG_PATH")
        .ok()
        .map(|path| path.trim().to_owned())
        .filter(|path| !path.is_empty())?;
    let body = match std::fs::read_to_string(&path) {
        Ok(body) => body,
        Err(error) => {
            tracing::warn!(%error, path, "catalog override unreadable — using embedded catalog");
            return None;
        }
    };
    let entries = parse_catalog(&body, &path);
    if entries.is_empty() {
        tracing::warn!(
            path,
            "catalog override has no usable models — using embedded catalog"
        );
        return None;
    }
    Some(entries)
}

fn parse_catalog(body: &str, source: &str) -> Vec<ModelCatalogEntry> {
    match serde_json::from_str(body) {
        Ok(entries) => entries,
        Err(error) => {
            tracing::warn!(%error, source, "catalog json rejected");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_parses() {
        let entries = embedded_catalog();
        assert!(!entries.is_empty());
        assert!(entries.iter().all(|entry| !entry.id.is_empty()));
    }

    #[test]
    fn seed_entries_are_sorted_by_id() {
        let entries = seed_entries();
        assert!(entries.windows(2).all(|pair| pair[0].id <= pair[1].id));
    }
}
