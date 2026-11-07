use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

/// Adapter family of the upstream endpoint a model is served through.
/// The router fronts everything via OpenRouter's OpenAI-compatible API:
/// the direct Gemini protocol broke tool calling in the desktop app, so
/// the Gemini ingress dialect stays but converts through the IR. Further
/// direct-origin families (native Anthropic/Gemini) can slot in here.
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Capabilities {
    pub tools: bool,
    pub vision: bool,
    pub reasoning: bool,
}

/// Per-1M-token prices in USD, displayed in the dashboard models list.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Pricing {
    pub input: String,
    pub output: String,
    pub cache_read: Option<String>,
    pub cache_write: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCatalogEntry {
    pub id: String,
    pub name: String,
    pub compat: Compat,
    /// Server-held provider id, resolved through the provider registry.
    pub provider: String,
    pub upstream_model: String,
    pub context: i64,
    pub max_output: i64,
    pub capabilities: Capabilities,
    pub pricing: Pricing,
    #[serde(default)]
    pub free: bool,
}

/// Serialized shape returned by the models API and consumed by the dashboard.
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
    /// Published/allowlisted for the viewer (console: plan allowlist;
    /// agent key: its plan's allowlist).
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
            enabled,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

/// Live catalog cache — loaded from Postgres at boot (`model_catalog`
/// table) and refreshed from it on demand. Cloned into application state.
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

/// Seed list. The live catalog lives in the `model_catalog` table (dynamic,
/// editable without redeploying); this file seeds it on boot and can
/// temporarily override through `CATALOG_PATH` for local experiments.
const EMBEDDED: &str = include_str!("../../catalog/models.json");

pub fn seed_entries() -> Vec<ModelCatalogEntry> {
    let mut entries = from_json(EMBEDDED);
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    entries
}

fn from_json(body: &str) -> Vec<ModelCatalogEntry> {
    serde_json::from_str(body).unwrap_or_default()
}
