use std::sync::Arc;
use std::time::Duration;

use reqwest::{Client as HttpClient, Url};

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

    pub async fn delete(&self, key: &str) {
        let Some(inner) = self.client.as_ref() else {
            return;
        };
        let _: Option<usize> = run(inner, &["DEL", key]).await;
    }
}

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
