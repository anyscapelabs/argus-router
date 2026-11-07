//! Upstream origin: OpenRouter. Every provider and model lives behind it
//! (one server-held key, `OPENROUTER_API_KEY`); per-provider keys were
//! never agreed and are intentionally absent. Adapter family choice and
//! quirks mirror the desktop app's `Provider`/`Router` conversion style.

use crate::proxy::Quirks;
use crate::services::catalog::Compat;

#[derive(Clone, Copy, Debug)]
pub enum Auth {
    Bearer,
}

#[derive(Clone, Debug)]
pub struct Provider {
    pub id: &'static str,
    pub name: &'static str,
    pub compat: Compat,
    /// Base URL, trailing slash trimmed; the adapter appends the chat path
    /// from its quirks (SSRF-safe by construction: no client input enters
    /// the URL).
    pub base_url: &'static str,
    /// Bearer is the only upstream auth style today; kept in the struct so
    /// a future direct-origin family slots in without a redesign.
    #[allow(dead_code)]
    pub auth: Auth,
    /// Environment variable holding the origin's key.
    pub key_env: &'static str,
    pub extra_headers: &'static [(&'static str, &'static str)],
    pub quirks: Quirks,
}

pub const OPENROUTER: Provider = Provider {
    id: "openrouter",
    name: "OpenRouter",
    compat: Compat::Openai,
    base_url: "https://openrouter.ai/api/v1",
    auth: Auth::Bearer,
    key_env: "OPENROUTER_API_KEY",
    extra_headers: &[],
    quirks: Quirks {
        chat_path: "/chat/completions",
        // OpenRouter accepts plain `max_tokens` for every model it fronts.
        max_tokens_field: "max_tokens",
        // OpenRouter relays usage on streamed finals on its own; asking
        // for `stream_options.include_usage` breaks some fronted models.
        stream_options: false,
    },
};

pub fn lookup(id: &str) -> Option<&'static Provider> {
    (id == OPENROUTER.id).then_some(&OPENROUTER)
}

/// Resolves the origin key from the process environment (`None` when the
/// origin is unconfigured — surfaced as 503, not a client bug).
pub fn key_of(provider: &Provider) -> Option<String> {
    let value = std::env::var(provider.key_env).unwrap_or_default();
    let value = value.trim().to_owned();
    (!value.is_empty()).then_some(value)
}
