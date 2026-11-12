use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

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
    Free,
    Small,
    Medium,
}

impl Tier {
    pub fn as_str(self) -> &'static str {
        match self {
            Tier::Free => "free",
            Tier::Small => "small",
            Tier::Medium => "medium",
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
}

impl ModelCatalogEntry {
    pub fn view(&self, enabled: bool) -> ModelView {
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
}

const EMBEDDED: &str = include_str!("../../catalog/models.json");

pub fn seed_entries() -> Vec<ModelCatalogEntry> {
    let body = if let Ok(path) = std::env::var("CATALOG_PATH") {
        if !path.trim().is_empty() {
            std::fs::read_to_string(path.trim()).unwrap_or_else(|_| EMBEDDED.to_owned())
        } else {
            EMBEDDED.to_owned()
        }
    } else {
        EMBEDDED.to_owned()
    };
    let mut entries = from_json(&body);
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    entries
}

fn from_json(body: &str) -> Vec<ModelCatalogEntry> {
    serde_json::from_str(body).unwrap_or_default()
}
