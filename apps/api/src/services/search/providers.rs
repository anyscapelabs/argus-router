use async_trait::async_trait;
use serde_json::Value;

use super::types::{SearchResponse, SearchResultItem};
use crate::proxy::ir::{ErrCode, Errx};

fn read_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|k| k.trim().to_owned())
        .filter(|k| !k.is_empty())
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    let mut iter = s.char_indices();
    match iter.nth(max_chars) {
        Some((byte_idx, _)) => format!("{}...", &s[..byte_idx]),
        None => s.to_owned(),
    }
}

#[async_trait]
pub trait SearchProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn is_configured(&self) -> bool;
    fn cost_per_query(&self) -> f64;
    async fn search(
        &self,
        client: &reqwest::Client,
        query: &str,
        max_results: usize,
    ) -> Result<SearchResponse, Errx>;
}

pub struct BraveSearchProvider {
    api_key: Option<String>,
    base_url: String,
}

impl BraveSearchProvider {
    pub fn from_env() -> Self {
        let api_key = read_env("BRAVE_SEARCH_API_KEY").or_else(|| read_env("BRAVE_API_KEY"));
        Self {
            api_key,
            base_url: "https://api.search.brave.com".to_owned(),
        }
    }

    #[cfg(test)]
    pub fn disabled() -> Self {
        Self {
            api_key: None,
            base_url: "https://api.search.brave.com".to_owned(),
        }
    }

    #[cfg(test)]
    fn configured_for(base_url: String) -> Self {
        Self {
            api_key: Some("test-key".to_owned()),
            base_url,
        }
    }
}

#[async_trait]
impl SearchProvider for BraveSearchProvider {
    fn id(&self) -> &'static str {
        "brave"
    }

    fn is_configured(&self) -> bool {
        self.api_key.is_some()
    }
    fn cost_per_query(&self) -> f64 {
        0.003
    }

    async fn search(
        &self,
        client: &reqwest::Client,
        query: &str,
        max_results: usize,
    ) -> Result<SearchResponse, Errx> {
        let key = self.api_key.as_deref().ok_or_else(|| {
            Errx::new(
                ErrCode::ProviderUnconfigured,
                503,
                "Brave search API key not configured",
            )
        })?;
        let count = max_results.clamp(1, 20);
        let url = format!(
            "{}/res/v1/web/search?q={}&count={}&result_filter=web&text_decorations=false",
            self.base_url.trim_end_matches('/'),
            urlencoding::encode(query),
            count
        );

        let res = client
            .get(&url)
            .header("Accept", "application/json")
            .header("X-Subscription-Token", key)
            .send()
            .await
            .map_err(|e| {
                Errx::new(
                    ErrCode::Network,
                    502,
                    format!("Brave search network error: {e}"),
                )
            })?;

        if !res.status().is_success() {
            let status = res.status().as_u16();
            let body = res.text().await.unwrap_or_default();
            return Err(Errx::new(
                if status == 429 {
                    ErrCode::RateLimited
                } else {
                    ErrCode::Upstream
                },
                status,
                format!("Brave search error ({status}): {body}"),
            ));
        }

        let content_type = res
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("unknown")
            .to_owned();
        let content_encoding = res
            .headers()
            .get(reqwest::header::CONTENT_ENCODING)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("identity")
            .to_owned();
        let bytes = res.bytes().await.map_err(|e| {
            Errx::new(
                ErrCode::Network,
                502,
                format!("Failed to read Brave response: {e}"),
            )
        })?;
        let body: Value = serde_json::from_slice(&bytes).map_err(|e| {
            Errx::new(
                ErrCode::Upstream,
                502,
                format!(
                    "Failed to parse Brave response: {e} (content-type={content_type}, content-encoding={content_encoding}, bytes={})",
                    bytes.len()
                ),
            )
        })?;

        let mut results = Vec::new();
        if let Some(items) = body.pointer("/web/results").and_then(|v| v.as_array()) {
            for item in items {
                let title = item
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_owned();
                let url = item
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_owned();
                let snippet = item
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_owned();
                let content = item
                    .pointer("/extra_snippets")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|s| s.as_str())
                            .collect::<Vec<_>>()
                            .join(" ")
                    });
                results.push(SearchResultItem {
                    title,
                    url,
                    snippet,
                    content,
                });
            }
        }

        Ok(SearchResponse {
            query: query.to_owned(),
            provider: self.id().to_owned(),
            results,
        })
    }
}

pub struct TavilySearchProvider {
    api_key: Option<String>,
}

impl TavilySearchProvider {
    pub fn from_env() -> Self {
        Self {
            api_key: read_env("TAVILY_API_KEY"),
        }
    }

    #[cfg(test)]
    pub fn disabled() -> Self {
        Self { api_key: None }
    }
}

#[async_trait]
impl SearchProvider for TavilySearchProvider {
    fn id(&self) -> &'static str {
        "tavily"
    }

    fn is_configured(&self) -> bool {
        self.api_key.is_some()
    }
    fn cost_per_query(&self) -> f64 {
        0.004
    }

    async fn search(
        &self,
        client: &reqwest::Client,
        query: &str,
        max_results: usize,
    ) -> Result<SearchResponse, Errx> {
        let key = self.api_key.as_deref().ok_or_else(|| {
            Errx::new(
                ErrCode::ProviderUnconfigured,
                503,
                "Tavily search API key not configured",
            )
        })?;
        let payload = serde_json::json!({
            "api_key": key,
            "query": query,
            "max_results": max_results.clamp(1, 20),
            "search_depth": "basic",
            "include_answer": false,
            "include_raw_content": false,
        });

        let res = client
            .post("https://api.tavily.com/search")
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                Errx::new(
                    ErrCode::Network,
                    502,
                    format!("Tavily search network error: {e}"),
                )
            })?;

        if !res.status().is_success() {
            let status = res.status().as_u16();
            let body = res.text().await.unwrap_or_default();
            return Err(Errx::new(
                if status == 429 {
                    ErrCode::RateLimited
                } else {
                    ErrCode::Upstream
                },
                status,
                format!("Tavily search error ({status}): {body}"),
            ));
        }

        let body: Value = res.json().await.map_err(|e| {
            Errx::new(
                ErrCode::Upstream,
                502,
                format!("Failed to parse Tavily response: {e}"),
            )
        })?;

        let mut results = Vec::new();
        if let Some(items) = body.get("results").and_then(|v| v.as_array()) {
            for item in items {
                let title = item
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_owned();
                let url = item
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_owned();
                let snippet = item
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_owned();
                let raw_content = item
                    .get("raw_content")
                    .and_then(|v| v.as_str())
                    .map(str::to_owned);
                results.push(SearchResultItem {
                    title,
                    url,
                    snippet,
                    content: raw_content,
                });
            }
        }

        Ok(SearchResponse {
            query: query.to_owned(),
            provider: self.id().to_owned(),
            results,
        })
    }
}

pub struct ExaSearchProvider {
    api_key: Option<String>,
}

impl ExaSearchProvider {
    pub fn from_env() -> Self {
        Self {
            api_key: read_env("EXA_API_KEY"),
        }
    }

    #[cfg(test)]
    pub fn disabled() -> Self {
        Self { api_key: None }
    }
}

#[async_trait]
impl SearchProvider for ExaSearchProvider {
    fn id(&self) -> &'static str {
        "exa"
    }

    fn is_configured(&self) -> bool {
        self.api_key.is_some()
    }
    fn cost_per_query(&self) -> f64 {
        0.001
    }

    async fn search(
        &self,
        client: &reqwest::Client,
        query: &str,
        max_results: usize,
    ) -> Result<SearchResponse, Errx> {
        let key = self.api_key.as_deref().ok_or_else(|| {
            Errx::new(
                ErrCode::ProviderUnconfigured,
                503,
                "Exa search API key not configured",
            )
        })?;
        let payload = serde_json::json!({
            "query": query,
            "numResults": max_results.clamp(1, 20),
            "useAutoprompt": true,
            "type": "neural",
            "contents": {
                "text": true
            }
        });

        let res = client
            .post("https://api.exa.ai/search")
            .header("x-api-key", key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| {
                Errx::new(
                    ErrCode::Network,
                    502,
                    format!("Exa search network error: {e}"),
                )
            })?;

        if !res.status().is_success() {
            let status = res.status().as_u16();
            let body = res.text().await.unwrap_or_default();
            return Err(Errx::new(
                if status == 429 {
                    ErrCode::RateLimited
                } else {
                    ErrCode::Upstream
                },
                status,
                format!("Exa search error ({status}): {body}"),
            ));
        }

        let body: Value = res.json().await.map_err(|e| {
            Errx::new(
                ErrCode::Upstream,
                502,
                format!("Failed to parse Exa response: {e}"),
            )
        })?;

        let mut results = Vec::new();
        if let Some(items) = body.get("results").and_then(|v| v.as_array()) {
            for item in items {
                let title = item
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_owned();
                let url = item
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_owned();
                let snippet = item
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                let truncated_snippet = truncate_chars(snippet, 300);
                results.push(SearchResultItem {
                    title,
                    url,
                    snippet: truncated_snippet,
                    content: if snippet.is_empty() {
                        None
                    } else {
                        Some(snippet.to_owned())
                    },
                });
            }
        }

        Ok(SearchResponse {
            query: query.to_owned(),
            provider: self.id().to_owned(),
            results,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use axum::Router;
    use axum::body::Body;
    use axum::http::{Response, header};
    use axum::routing::get;
    use flate2::Compression;
    use flate2::write::GzEncoder;

    use super::{BraveSearchProvider, SearchProvider, truncate_chars};

    #[test]
    fn truncate_chars_is_utf8_safe() {
        let input = "é".repeat(400);
        let truncated = truncate_chars(&input, 300);
        assert!(truncated.ends_with("..."));
        assert_eq!(truncated.chars().count(), 303);
        assert!(truncated.is_char_boundary(truncated.len()));

        let short = "héllo";
        assert_eq!(truncate_chars(short, 300), short);
    }

    #[tokio::test]
    async fn brave_decodes_gzip_response() {
        let json = br#"{"web":{"results":[{"title":"Argus","url":"https://example.com","description":"result"}]}}"#;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(json).expect("compress fixture");
        let compressed = encoder.finish().expect("finish fixture");
        let app = Router::new().route(
            "/res/v1/web/search",
            get(move || {
                let body = compressed.clone();
                async move {
                    Response::builder()
                        .header(header::CONTENT_TYPE, "application/json")
                        .header(header::CONTENT_ENCODING, "gzip")
                        .body(Body::from(body))
                        .expect("gzip response")
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let address = listener.local_addr().expect("test server address");
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve fixture");
        });

        let provider = BraveSearchProvider::configured_for(format!("http://{address}"));
        let response = provider
            .search(&reqwest::Client::new(), "argus", 5)
            .await
            .expect("decode gzip search result");
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].title, "Argus");
    }
}
