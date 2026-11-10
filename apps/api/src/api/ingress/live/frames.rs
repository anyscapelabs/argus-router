//! Upstream frame parsing helpers for the Live proxy: session ids, model-text
//! extraction, and 429 rate-limit envelope detection.

use std::time::SystemTime;

/// Default retry delay when Google does not send a `retryDelay` field.
pub(super) const DEFAULT_RETRY_DELAY_SECS: u64 = 60;

/// Generates a simple session id from the current nanosecond timestamp.
/// Not cryptographically random but collision-free enough for short-lived
/// in-process keys.
pub(super) fn new_session_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("live-{nanos:032x}")
}

/// Extracts text from an upstream `BidiGenerateContentServerContent` frame.
///
/// Google's shape:
/// ```json
/// { "serverContent": { "modelTurn": { "parts": [{"text": "..."}] } } }
/// ```
pub(super) fn extract_model_text(raw: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let parts = v.pointer("/serverContent/modelTurn/parts")?.as_array()?;
    let text: String = parts
        .iter()
        .filter_map(|p| p.get("text")?.as_str())
        .collect();
    if text.is_empty() { None } else { Some(text) }
}

/// Returns the retry-after delay in seconds when the upstream frame is a 429
/// error envelope, otherwise `None`.
pub(super) fn extract_rate_limit_secs(raw: &str) -> Option<u64> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let code = v.pointer("/error/code")?.as_u64()?;
    if code != 429 {
        return None;
    }
    // Try to parse `retryDelay` from the error details array.
    let retry = v
        .pointer("/error/details")
        .and_then(|d| d.as_array())
        .and_then(|arr| {
            arr.iter().find_map(|item| {
                item.get("retryDelay")
                    .and_then(|s| s.as_str())
                    // Google sends e.g. "60s"
                    .and_then(|s| s.trim_end_matches('s').parse::<u64>().ok())
            })
        })
        .unwrap_or(DEFAULT_RETRY_DELAY_SECS);
    Some(retry)
}
