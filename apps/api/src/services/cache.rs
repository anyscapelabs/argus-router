//! Upstash Redis cache client.
//!
//! Speaks the Upstash REST API directly (`POST <url>` with `["COMMAND",
//! args...]` and an `Authorization: Bearer <token>` header) via the shared
//! `reqwest` client — no extra dependency. Every operation is best-effort:
//! cache failures log a warning and fall back to the origin, never failing
//! the request. Keys use the `argus:<domain>:<id>` namespace.

use std::sync::Arc;
use std::time::Duration;

use reqwest::{Client as HttpClient, Url};

/// Cache backend — `Some(client)` when Upstash credentials are configured,
/// `None` in local dev without them (all operations become no-ops).
#[derive(Clone, Default)]
pub struct Cache {
    client: Option<Arc<Inner>>,
}

struct Inner {
    http: HttpClient,
    url: Url,
    token: String,
}

impl Cache {
    pub fn new(url: Option<&str>, token: Option<&str>) -> Self {
        let client = match (url, token) {
            (Some(url), Some(token)) if !url.is_empty() && !token.is_empty() => {
                match Url::parse(url) {
                    Ok(url) => {
                        let http = HttpClient::builder()
                            .timeout(Duration::from_secs(2))
                            .build();
                        match http {
                            Ok(http) => Some(Arc::new(Inner {
                                http,
                                url,
                                token: token.to_string(),
                            })),
                            Err(error) => {
                                tracing::warn!(%error, "invalid Upstash HTTP client; caching disabled");
                                None
                            }
                        }
                    }
                    Err(error) => {
                        tracing::warn!(%error, "invalid Upstash Redis URL; caching disabled");
                        None
                    }
                }
            }
            _ => None,
        };
        if client.is_some() {
            tracing::info!("Upstash Redis cache enabled");
        } else {
            tracing::warn!("Upstash Redis not configured; caching disabled");
        }
        Self { client }
    }

    /// Fetch a JSON value from the cache. `None` on miss or error.
    pub async fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        let inner = self.client.as_ref()?;
        let raw = run::<Option<String>>(inner, &["GET", key]).await?;
        let Some(raw) = raw else { return None };
        match serde_json::from_str::<T>(&raw) {
            Ok(value) => Some(value),
            Err(error) => {
                tracing::warn!(%error, key, "cache value failed to deserialize");
                None
            }
        }
    }

    /// Store a JSON value with a TTL. Best-effort — errors are logged, never raised.
    pub async fn set(&self, key: &str, value: &impl serde::Serialize, ttl_secs: u64) {
        let Some(inner) = self.client.as_ref() else {
            return;
        };
        let Ok(json) = serde_json::to_string(value) else {
            return;
        };
        let ttl = ttl_secs.to_string();
        let _: Option<String> = match run(inner, &["SET", key, &json, "EX", &ttl]).await {
            Some(result) => Some(result),
            None => None,
        };
    }

    /// Remove a key. Best-effort.
    pub async fn delete(&self, key: &str) {
        let Some(inner) = self.client.as_ref() else {
            return;
        };
        let _: Option<usize> = run(inner, &["DEL", key]).await;
    }
}

/// Run one Upstash REST command. `None` on any error (logged by the caller
/// context — deliberately quiet here so `get`/`set` stay one-liners).
async fn run<T: serde::de::DeserializeOwned>(inner: &Inner, args: &[&str]) -> Option<T> {
    let body = serde_json::to_value(args).ok()?;
    let response = inner
        .http
        .post(inner.url.clone())
        .bearer_auth(&inner.token)
        .json(&body)
        .send()
        .await
        .ok()?;
    let value: serde_json::Value = response.json().await.ok()?;
    match value.get("result") {
        Some(result) => serde_json::from_value(result.clone()).ok(),
        None => None,
    }
}
