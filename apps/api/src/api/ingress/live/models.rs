use serde::Deserialize;
use std::sync::OnceLock;

const LIVE_MODELS_JSON: &str = include_str!("../../../../catalog/live_models.json");

#[derive(Debug, Deserialize)]
pub(super) struct LiveModelSpec {
    pub(super) id: String,
    #[allow(dead_code)]
    name: String,
    #[serde(default)]
    pub(super) default: bool,
    #[allow(dead_code)]
    deprecated: bool,
    audio_in_per_min: f64,
    audio_out_per_min: f64,
}

impl LiveModelSpec {
    pub(super) fn blended_cost_per_min(&self) -> f64 {
        self.audio_in_per_min + self.audio_out_per_min
    }
}

fn live_models() -> &'static [LiveModelSpec] {
    static MODELS: OnceLock<Vec<LiveModelSpec>> = OnceLock::new();
    MODELS.get_or_init(|| {
        serde_json::from_str(LIVE_MODELS_JSON).expect("catalog/live_models.json is valid JSON")
    })
}

pub(super) fn default_live_model() -> &'static LiveModelSpec {
    live_models()
        .iter()
        .find(|m| m.default)
        .unwrap_or_else(|| &live_models()[0])
}

pub(super) fn find_live_model(id: &str) -> Option<&'static LiveModelSpec> {
    live_models().iter().find(|m| m.id == id)
}

pub(super) fn all_model_ids() -> Vec<&'static str> {
    live_models().iter().map(|m| m.id.as_str()).collect()
}
