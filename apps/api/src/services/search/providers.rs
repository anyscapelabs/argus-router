//! Web search provider adapters for Brave Search, Tavily AI, and Exa AI.

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
    /// Estimated cost in USD per query for this provider.
    fn cost_per_query(&self) -> f64;
    async fn search(
        &self,
        client: &reqwest::Client,
        query: &str,
        max_results: usize,
    ) -> Result<SearchResponse, Errx>;
}

/// Brave Search API provider.
/// https://api.search.brave.com/res/v1/web/search
pub struct BraveSearchProvider {
    api_key: Option<String>,
}

impl BraveSearchProvider {
    pub fn from_env() -> Self {
        let api_key = read_env("BRAVE_SEARCH_API_KEY").or_else(|| read_env("BRAVE_API_KEY"));
        Self { api_key }
    }

    #[cfg(test)]
    pub fn disabled() -> Self {
        Self { api_key: None }
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
        // Brave Data for Search: $3.00 per 1000 queries = $0.003 each.
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
            "https://api.search.brave.com/res/v1/web/search?q={}&count={}",
            urlencoding::encode(query),
            count
        );

        let res = client
            .get(&url)
            .header("Accept", "application/json")
            .header("Accept-Encoding", "gzip")
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

        let body: Value = res.json().await.map_err(|e| {
            Errx::new(
                ErrCode::Upstream,
                502,
                format!("Failed to parse Brave response: {e}"),
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

/// Tavily Search API provider.
/// https://api.tavily.com/search
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
        // Tavily Starter: $4.00 per 1000 queries = $0.004 each.
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

/// Exa AI search provider.
/// https://api.exa.ai/search
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
        // Exa: $1.00 per 1000 queries = $0.001 each.
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
    use super::truncate_chars;

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
}
