//! Search router: analyzes query complexity and routes with automatic fallback.

use super::providers::{
    BraveSearchProvider, ExaSearchProvider, SearchProvider, TavilySearchProvider,
};
use super::types::SearchResponse;
use crate::proxy::ir::{ErrCode, Errx};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueryComplexity {
    Simple,
    Complex,
}

/// Evaluates query complexity based on length, comparative keywords,
/// analytical terminology, research questions, and structured requirements.
pub fn evaluate_complexity(query: &str) -> QueryComplexity {
    let lower = query.to_lowercase();
    let word_count = lower.split_whitespace().count();

    // Long, descriptive research prompts are usually complex
    if word_count > 12 {
        return QueryComplexity::Complex;
    }

    let complex_keywords = [
        "compare",
        "vs",
        "versus",
        "difference between",
        "how does",
        "why does",
        "explain",
        "in-depth",
        "analysis",
        "analyze",
        "pros and cons",
        "tradeoffs",
        "architecture",
        "benchmark",
        "documentation",
        "research",
        "latest developments",
        "tutorial",
    ];

    for kw in &complex_keywords {
        if lower.contains(kw) {
            return QueryComplexity::Complex;
        }
    }

    QueryComplexity::Simple
}

pub struct SearchRouter {
    brave: BraveSearchProvider,
    tavily: TavilySearchProvider,
    exa: ExaSearchProvider,
}

impl Default for SearchRouter {
    fn default() -> Self {
        Self::from_env()
    }
}

impl SearchRouter {
    pub fn from_env() -> Self {
        Self {
            brave: BraveSearchProvider::from_env(),
            tavily: TavilySearchProvider::from_env(),
            exa: ExaSearchProvider::from_env(),
        }
    }

    pub fn new() -> Self {
        Self::from_env()
    }

    #[cfg(test)]
    pub fn unconfigured() -> Self {
        Self {
            brave: BraveSearchProvider::disabled(),
            tavily: TavilySearchProvider::disabled(),
            exa: ExaSearchProvider::disabled(),
        }
    }

    /// Determines the provider priority chain for the query.
    /// Complex queries prefer Tavily / Exa for semantic quality and clean extraction,
    /// falling back to Brave.
    /// Simple queries prefer Brave for speed and cost-effectiveness,
    /// falling back to Tavily and then Exa.
    pub fn provider_chain(&self, query: &str) -> Vec<&(dyn SearchProvider + 'static)> {
        let complexity = evaluate_complexity(query);
        match complexity {
            QueryComplexity::Complex => {
                vec![&self.tavily, &self.exa, &self.brave]
            }
            QueryComplexity::Simple => {
                vec![&self.brave, &self.tavily, &self.exa]
            }
        }
    }

    /// Dispatches search to the best available provider with automatic fallback on failure.
    pub async fn search(
        &self,
        client: &reqwest::Client,
        query: &str,
        max_results: usize,
    ) -> Result<SearchResponse, Errx> {
        let chain = self.provider_chain(query);
        let mut last_err = None;
        let mut attempted_any = false;

        for provider in chain {
            if !provider.is_configured() {
                continue;
            }
            attempted_any = true;
            match provider.search(client, query, max_results).await {
                Ok(response) => return Ok(response),
                Err(err) => {
                    tracing::warn!(
                        provider = provider.id(),
                        error = %err.message,
                        status = err.status,
                        "Search provider attempt failed, attempting fallback"
                    );
                    last_err = Some(err);
                }
            }
        }

        if !attempted_any {
            return Err(Errx::new(
                ErrCode::ProviderUnconfigured,
                503,
                "no web search providers (Brave, Tavily, or Exa) are configured on this router",
            ));
        }

        Err(last_err.unwrap_or_else(|| {
            Errx::new(
                ErrCode::Upstream,
                502,
                "all search provider attempts failed",
            )
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complexity_simple() {
        assert_eq!(
            evaluate_complexity("weather in Paris"),
            QueryComplexity::Simple
        );
        assert_eq!(
            evaluate_complexity("apple stock price"),
            QueryComplexity::Simple
        );
        assert_eq!(
            evaluate_complexity("rust 1.85 release date"),
            QueryComplexity::Simple
        );
    }

    #[test]
    fn test_complexity_complex() {
        assert_eq!(
            evaluate_complexity("compare PostgreSQL vs MySQL performance in high concurrency"),
            QueryComplexity::Complex
        );
        assert_eq!(
            evaluate_complexity("how does transformer attention mechanism work in detail"),
            QueryComplexity::Complex
        );
        assert_eq!(
            evaluate_complexity(
                "in-depth analysis of rust async runtime architectures and benchmarks"
            ),
            QueryComplexity::Complex
        );
    }

    #[test]
    fn test_provider_chain_ordering() {
        let router = SearchRouter::new();
        let simple_chain = router.provider_chain("apple stock price");
        assert_eq!(simple_chain[0].id(), "brave");
        assert_eq!(simple_chain[1].id(), "tavily");
        assert_eq!(simple_chain[2].id(), "exa");

        let complex_chain = router.provider_chain("compare rust vs go concurrency models");
        assert_eq!(complex_chain[0].id(), "tavily");
        assert_eq!(complex_chain[1].id(), "exa");
        assert_eq!(complex_chain[2].id(), "brave");
    }

    #[tokio::test]
    async fn test_search_unconfigured_error() {
        // Use an explicit unconfigured router so the test is deterministic
        // regardless of ambient BRAVE/TAVILY/EXA env vars.
        let router = SearchRouter::unconfigured();
        let client = reqwest::Client::new();
        let res = router.search(&client, "test query", 5).await;
        assert!(res.is_err());
        let err = res.err().unwrap();
        assert_eq!(err.status, 503);
    }
}
