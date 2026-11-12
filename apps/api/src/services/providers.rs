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
    pub base_url: &'static str,
    #[allow(dead_code)]
    pub auth: Auth,
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
        max_tokens_field: "max_tokens",
        stream_options: true,
    },
};

pub fn lookup(id: &str) -> Option<&'static Provider> {
    (id == OPENROUTER.id).then_some(&OPENROUTER)
}

pub fn key_of(provider: &Provider) -> Option<String> {
    let value = std::env::var(provider.key_env).unwrap_or_default();
    let value = value.trim().to_owned();
    (!value.is_empty()).then_some(value)
}
