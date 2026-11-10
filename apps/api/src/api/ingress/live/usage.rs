//! Usage-row recording for the Live endpoint. Every `usage_logs` row written
//! by the Live handlers funnels through [`record`] so the provider tag and
//! zero-token shape live in exactly one place.

use crate::services::usage::UsageRow;
use crate::state::AppState;

/// Identity + outcome metadata for a single Live usage row.
pub(super) struct LiveUsage<'a> {
    pub key_id: Option<&'a str>,
    pub user_id: Option<&'a str>,
    pub plan_id: Option<&'a str>,
    pub model: &'a str,
    pub status: i32,
    pub latency_ms: i64,
    pub error_code: Option<&'a str>,
    pub cost_usd: f64,
}

/// Records a `usage_logs` row tagged with the `gemini-live` provider.
pub(super) fn record(state: &AppState, info: LiveUsage<'_>) {
    state.usage.record(UsageRow {
        key_id: info.key_id.map(str::to_owned),
        user_id: info.user_id.map(str::to_owned),
        plan_id: info.plan_id.map(str::to_owned),
        model: info.model.to_owned(),
        provider: Some("gemini-live".to_owned()),
        input_tokens: 0,
        output_tokens: 0,
        cache_read_tokens: 0,
        cost_usd: info.cost_usd,
        status: info.status,
        latency_ms: Some(info.latency_ms),
        error_code: info.error_code.map(str::to_owned),
    });
}
