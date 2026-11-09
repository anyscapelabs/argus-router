use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResultItem {
    pub title: String,
    pub url: String,
    pub snippet: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResponse {
    pub query: String,
    pub provider: String,
    pub results: Vec<SearchResultItem>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    #[serde(default = "default_max_results")]
    pub max_results: usize,
}

impl SearchRequest {
    pub fn clamped_max_results(&self) -> usize {
        self.max_results.clamp(1, 20)
    }
}

fn default_max_results() -> usize {
    5
}
