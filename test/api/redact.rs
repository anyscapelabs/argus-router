use std::str::FromStr;

use api::observe::redact_uri;
use axum::http::Uri;

#[test]
fn redacts_sensitive_params() {
    let uri =
        Uri::from_str("https://gateway.test/v1/live?model=live&key=SUPERSECRET&page=1").unwrap();
    let cleaned = redact_uri(&uri);
    assert!(
        !cleaned.contains("SUPERSECRET"),
        "key must not leak: {cleaned}"
    );
    assert!(cleaned.contains("key=[redacted]"));
    assert!(
        cleaned.contains("model=live"),
        "non-secret params stay: {cleaned}"
    );
    assert!(cleaned.contains("page=1"));
}

#[test]
fn leaves_plain_paths_untouched() {
    let uri = Uri::from_str("https://gateway.test/v1/models").unwrap();
    assert_eq!(redact_uri(&uri), "/v1/models");
}

#[test]
fn redacts_all_known_secret_names() {
    for name in ["key", "api_key", "token", "access_token"] {
        let uri = Uri::from_str(&format!("https://gateway.test/path?{name}=abc")).unwrap();
        assert_eq!(redact_uri(&uri), format!("/path?{name}=[redacted]"));
    }
}
