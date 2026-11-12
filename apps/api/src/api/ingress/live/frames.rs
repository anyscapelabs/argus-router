use std::time::SystemTime;

pub(super) const DEFAULT_RETRY_DELAY_SECS: u64 = 60;

pub(super) fn new_session_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("live-{nanos:032x}")
}

pub(super) fn extract_model_text(raw: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let parts = v.pointer("/serverContent/modelTurn/parts")?.as_array()?;
    let text: String = parts
        .iter()
        .filter_map(|p| p.get("text")?.as_str())
        .collect();
    if text.is_empty() { None } else { Some(text) }
}

pub(super) fn extract_rate_limit_secs(raw: &str) -> Option<u64> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let code = v.pointer("/error/code")?.as_u64()?;
    if code != 429 {
        return None;
    }
    let retry = v
        .pointer("/error/details")
        .and_then(|d| d.as_array())
        .and_then(|arr| {
            arr.iter().find_map(|item| {
                item.get("retryDelay")
                    .and_then(|s| s.as_str())
                    .and_then(|s| s.trim_end_matches('s').parse::<u64>().ok())
            })
        })
        .unwrap_or(DEFAULT_RETRY_DELAY_SECS);
    Some(retry)
}

pub(super) fn extract_error_code(raw: &str) -> Option<i64> {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()?
        .pointer("/error/code")?
        .as_i64()
}
