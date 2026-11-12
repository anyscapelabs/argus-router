#![allow(dead_code)]

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug)]
pub struct Quirks {
    pub chat_path: &'static str,
    pub max_tokens_field: &'static str,
    pub stream_options: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Clone, Debug)]
pub struct Message {
    pub role: Role,
    pub blocks: Vec<Block>,
}

impl Message {
    pub fn assistant_empty() -> Message {
        Message {
            role: Role::Assistant,
            blocks: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Block {
    Text {
        text: String,
    },
    Image {
        mime: String,
        data_b64: String,
    },
    Thinking {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input_json: String,
    },
    ToolResult {
        id: String,
        content: String,
        is_error: bool,
    },
}

#[derive(Clone, Debug)]
pub struct ToolDef {
    pub name: String,
    pub description: Option<String>,
    pub schema_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolChoice {
    Auto,
    None,
    Required,
    Named(&'static str),
    NamedOwned(String),
}

impl ToolChoice {
    pub fn named(name: String) -> ToolChoice {
        ToolChoice::NamedOwned(name)
    }
}

#[derive(Clone, Debug)]
pub struct Params {
    pub max_tokens: Option<u64>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub stop: Vec<String>,
}

impl Params {
    pub fn clamped(params: Option<OpenAiParams>, entry_max_output: i64) -> Params {
        let catalog_limit = entry_max_output.max(1) as u64;
        let max_tokens = params
            .as_ref()
            .and_then(|p| p.max_tokens.or(p.max_completion_tokens))
            .unwrap_or(catalog_limit)
            .clamp(1, catalog_limit);
        Params {
            max_tokens: Some(max_tokens),
            temperature: params.as_ref().and_then(|p| p.temperature),
            top_p: params.as_ref().and_then(|p| p.top_p),
            stop: params
                .as_ref()
                .and_then(|p| p.stop.clone())
                .unwrap_or_default(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct OpenAiParams {
    pub max_tokens: Option<u64>,
    pub max_completion_tokens: Option<u64>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub stop: Option<Vec<String>>,
}

#[derive(Clone, Debug)]
pub struct Chat {
    pub system: Vec<String>,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDef>,
    pub tool_choice: ToolChoice,
    pub params: Params,
}

impl Chat {
    pub fn has_tools(&self) -> bool {
        !self.tools.is_empty()
    }

    pub fn append_system(&mut self, text: String) {
        self.system.push(text);
    }

    pub fn push_message(&mut self, message: Message) {
        self.messages.push(message);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FinishReason {
    #[default]
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
}

#[derive(Clone, Debug)]
pub enum Event {
    TextDelta(String),
    ThinkingDelta(String),
    ToolUseStart {
        index: usize,
        id: String,
        name: String,
    },
    ToolUseDelta {
        index: usize,
        value_json: String,
    },
    ToolUseEnd {
        index: usize,
    },
    Finish(FinishReason),
    Usage {
        input_tokens: i64,
        output_tokens: i64,
        cache_read_tokens: i64,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct Turn {
    pub text: String,
    pub thinking: String,
    #[serde(skip)]
    pub tool_use: Vec<(String, String, String)>,
    #[serde(skip)]
    pub finish_reason: FinishReason,
    #[serde(skip)]
    pub usage_reported: bool,
    pub usage: Usage,
}

impl Default for Turn {
    fn default() -> Self {
        Self {
            text: String::new(),
            thinking: String::new(),
            tool_use: Vec::new(),
            finish_reason: FinishReason::Stop,
            usage_reported: false,
            usage: Usage::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct Usage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
}

impl Usage {
    pub fn add_output(&mut self, tokens: i64) {
        self.output_tokens += tokens;
    }
}

#[derive(Clone, Debug)]
pub enum StreamEnd {
    Done(Usage),
    Errored(crate::proxy::Errx),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrCode {
    InvalidRequest,
    Unauthorized,
    Forbidden,
    NotFound,
    RateLimited,
    CreditsExhausted,
    Overloaded,
    Upstream,
    Network,
    ProviderUnconfigured,
    Timeout,
}

impl ErrCode {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrCode::InvalidRequest => "invalid_request_error",
            ErrCode::Unauthorized => "unauthorized",
            ErrCode::Forbidden => "forbidden",
            ErrCode::NotFound => "not_found",
            ErrCode::RateLimited => "rate_limit_error",
            ErrCode::CreditsExhausted => "credits_exhausted",
            ErrCode::Overloaded => "overloaded_error",
            ErrCode::Upstream => "upstream_error",
            ErrCode::Network => "network_error",
            ErrCode::ProviderUnconfigured => "provider_unconfigured",
            ErrCode::Timeout => "timeout_error",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Errx {
    pub code: ErrCode,
    pub status: u16,
    pub message: String,
    pub provider: Option<String>,
    pub request_id: Option<String>,
    pub retry_after: Option<u64>,
}

impl Errx {
    pub fn new(code: ErrCode, status: u16, message: impl Into<String>) -> Self {
        Self {
            code,
            status,
            message: message.into(),
            provider: None,
            request_id: None,
            retry_after: None,
        }
    }

    pub fn with_provider(mut self, provider: &str) -> Self {
        self.provider = Some(provider.to_owned());
        self
    }

    pub fn with_retry_after(mut self, seconds: u64) -> Self {
        self.retry_after = Some(seconds);
        self
    }

    pub fn from_upstream_status(status: u16, message: impl Into<String>) -> Self {
        let code = match status {
            400 | 404 | 422 => ErrCode::InvalidRequest,
            401 => ErrCode::Unauthorized,
            403 => ErrCode::Forbidden,
            408 => ErrCode::Timeout,
            429 => ErrCode::RateLimited,
            409 => ErrCode::Upstream,
            500 | 502 | 503 | 504 => ErrCode::Upstream,
            _ => ErrCode::Upstream,
        };
        Self::new(code, status, message)
    }
}

pub fn parse_json_lenient(raw: &str) -> serde_json::Value {
    serde_json::from_str(raw).unwrap_or_else(|_| serde_json::Value::String(raw.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::{OpenAiParams, Params};

    #[test]
    fn materializes_catalog_output_limit() {
        assert_eq!(Params::clamped(None, 4096).max_tokens, Some(4096));
    }

    #[test]
    fn clamps_requested_output_limit() {
        let params = OpenAiParams {
            max_tokens: Some(8192),
            ..OpenAiParams::default()
        };
        assert_eq!(Params::clamped(Some(params), 4096).max_tokens, Some(4096));
    }
}
