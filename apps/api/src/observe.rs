pub const REDACTED_QUERY_PARAMS: &[&str] = &["key", "api_key", "token", "access_token"];

pub fn redact_uri(uri: &axum::http::Uri) -> String {
    let path = uri.path();
    let Some(query) = uri.query() else {
        return path.to_owned();
    };
    let mut redacted = false;
    let cleaned: Vec<String> = query
        .split('&')
        .map(|pair| match pair.split_once('=') {
            Some((name, _)) if REDACTED_QUERY_PARAMS.contains(&name) => {
                redacted = true;
                format!("{name}=[redacted]")
            }
            _ => pair.to_owned(),
        })
        .collect();
    if !redacted {
        return path.to_owned();
    }
    format!("{path}?{}", cleaned.join("&"))
}

pub fn redacted_span<B>(request: &axum::http::Request<B>) -> tracing::Span {
    tracing::debug_span!(
        "request",
        method = %request.method(),
        uri = %redact_uri(request.uri()),
        version = ?request.version(),
    )
}
