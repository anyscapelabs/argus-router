use crate::services::usage::UsageRow;
use crate::state::AppState;

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

pub(super) fn record(state: &AppState, info: LiveUsage<'_>) {
    state.usage.record(row(info));
}

pub(super) async fn record_with_reservation(
    state: &AppState,
    info: LiveUsage<'_>,
    reservation_id: Option<&str>,
) -> Result<(), sqlx::Error> {
    let row = row(info);
    if let Some(reservation_id) = reservation_id {
        crate::api::ingress::settle_generation_usage(&state.usage, reservation_id, row).await
    } else {
        state.usage.record(row);
        Ok(())
    }
}

fn row(info: LiveUsage<'_>) -> UsageRow {
    UsageRow {
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
    }
}
