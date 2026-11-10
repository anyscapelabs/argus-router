//! Embedded registry of supported Gemini Live models
//! (`catalog/live_models.json`).
//!
//! Edit that file to add/remove models or adjust billing rates — the ids are
//! embedded at build time, so changing the JSON requires a rebuild but no code
//! changes. Live models are deliberately NOT part of `catalog/models.json`, so
//! they never show up in the `/v1/models` or `/v1beta/models` chat catalogs —
//! they are only reachable through the Live WebSocket endpoint.

use serde::Deserialize;
use std::sync::OnceLock;

const LIVE_MODELS_JSON: &str = include_str!("../../../../catalog/live_models.json");

/// A supported Gemini Live model from `live_models.json`.
#[derive(Debug, Deserialize)]
pub(super) struct LiveModelSpec {
    pub(super) id: String,
    #[allow(dead_code)]
    name: String,
    #[serde(default)]
    pub(super) default: bool,
    #[allow(dead_code)]
    deprecated: bool,
    /// Audio-input billing rate (USD per minute of session).
    audio_in_per_min: f64,
    /// Audio-output billing rate (USD per minute of session).
    audio_out_per_min: f64,
}

impl LiveModelSpec {
    /// Blended full-duplex cost estimate (input + output audio) per minute.
    pub(super) fn blended_cost_per_min(&self) -> f64 {
        self.audio_in_per_min + self.audio_out_per_min
    }
}

/// Parses the embedded Live model registry exactly once.
fn live_models() -> &'static [LiveModelSpec] {
    static MODELS: OnceLock<Vec<LiveModelSpec>> = OnceLock::new();
    MODELS.get_or_init(|| {
        serde_json::from_str(LIVE_MODELS_JSON).expect("catalog/live_models.json is valid JSON")
    })
}

/// The registry entry flagged as `default` (falls back to the first entry).
pub(super) fn default_live_model() -> &'static LiveModelSpec {
    live_models()
        .iter()
        .find(|m| m.default)
        .unwrap_or_else(|| &live_models()[0])
}

/// Looks up a Live model spec by id (e.g. `gemini-3.8-live`).
pub(super) fn find_live_model(id: &str) -> Option<&'static LiveModelSpec> {
    live_models().iter().find(|m| m.id == id)
}

/// All registered model ids, for error messages.
pub(super) fn all_model_ids() -> Vec<&'static str> {
    live_models().iter().map(|m| m.id.as_str()).collect()
}
