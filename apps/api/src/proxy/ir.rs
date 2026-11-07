//! Canonical internal representation (CIR) for chat proxying.
//!
//! Every ingress format (OpenAI compat, Gemini compat) parses into this
//! form; the upstream adapter family is rebuilt from it. Tools and tool-call
//! history only ever exist here in normalized form, so a client can switch
//! models at any point of a conversation without tool-calling context being
//! mangled or lost.
//!
//! The IR mirrors the desktop app's surface one-for-one; members the router
//! has not yet wired through are kept for that parity rather than trimmed.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Per-origin conversion quirks, mirroring the desktop app's `Quirks` struct
/// so a future direct-origin family (native Anthropic/Gemini) ports into the
/// same pipeline without a redesign.
#[derive(Clone, Copy, Debug)]
pub struct Quirks {
    /// Chat-completions path appended to the upstream base URL.
    pub chat_path: &'static str,
    /// Field upstream expects the output cap in.
    pub max_tokens_field: &'static str,
    /// Whether `stream_options.include_usage` is safe to request.
    pub stream_options: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
}

/// One message turn: a role and its normalized blocks.
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
    /// Vision input, already base64-decoded inline (`mime`, `data_b64`).
    Image {
        mime: String,
        data_b64: String,
    },
    /// Model reasoning surfaced upstream (Anthropic thinking; OpenAI
    /// reasoning_content for reasoning flavors). Round-tripped so a
    /// mid-conversation switch keeps reasoning context where supported,
    /// silently dropped upstream where not.
    Thinking {
        text: String,
    },
    /// A call the assistant emitted. `input_json` is a serialized JSON value
    /// (object); kept as a string so round-trips never re-parse or re-flatten
    /// large tool payloads needlessly.
    ToolUse {
        id: String,
        name: String,
        input_json: String,
    },
    /// A result fed back to the model, `is_error` marks failed tool calls.
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
    /// Parameters JSON schema (serialized object), normalized from the
    /// ingress dialect.
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
    /// Already clamped to the entry's `max_output`.
    pub max_tokens: Option<u64>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub stop: Vec<String>,
}

impl Params {
    pub fn clamped(params: Option<OpenAiParams>, entry_max_output: i64) -> Params {
        let max_tokens = params
            .as_ref()
            .and_then(|p| p.max_tokens.or(p.max_completion_tokens))
            .map(|tokens| tokens.min(entry_max_output.max(1) as u64));
        Params {
            max_tokens,
            temperature: params.as_ref().and_then(|p| p.temperature),
            top_p: params.as_ref().and_then(|p| p.top_p),
            stop: params
                .as_ref()
                .and_then(|p| p.stop.clone())
                .unwrap_or_default(),
        }
    }
}

/// Mid-conversation model switches are common, so every field stays
/// optional/loosely typed — no upstream-specific fields escape here.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct OpenAiParams {
    pub max_tokens: Option<u64>,
    pub max_completion_tokens: Option<u64>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub stop: Option<Vec<String>>,
}

/// A normalized model call.
#[derive(Clone, Debug)]
pub struct Chat {
    /// Concatenated system turns.
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

/// Streamed / collected model output, normalized across adapters.
///
/// Ingress re-serialization sits on top: a stream of these events (SSE
/// endpoint) or a finished `Turn` (non-streaming) maps back into the caller's
/// chosen wire dialect.
#[derive(Clone, Debug)]
pub enum Event {
    /// Model-visible output text.
    TextDelta(String),
    /// Reasoning tokens (Anthropic thinking / OpenAI reasoning_content).
    ThinkingDelta(String),
    /// First fragment of one tool call. `index` is the order of the call
    /// within one assistant turn — not upstream's raw array index.
    ToolUseStart {
        index: usize,
        id: String,
        name: String,
    },
    /// A chunk of the serialized call arguments JSON.
    ToolUseDelta {
        index: usize,
        value_json: String,
    },
    /// Call arguments are complete for this index.
    ToolUseEnd {
        index: usize,
    },
    Usage {
        input_tokens: i64,
        output_tokens: i64,
        cache_read_tokens: i64,
    },
}

/// A finished model turn (non-streaming path), again normalized.
#[derive(Clone, Debug, Serialize)]
pub struct Turn {
    pub text: String,
    pub thinking: String,
    #[serde(skip)]
    pub tool_use: Vec<(String, String, String)>,
    pub usage: Usage,
}

impl Default for Turn {
    fn default() -> Self {
        Self {
            text: String::new(),
            thinking: String::new(),
            tool_use: Vec::new(),
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

/// Terminal error of a stream (ingress turns these into dialect-shaped
/// error envelopes).
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
    /// HTTP status for the ingress response.
    pub status: u16,
    pub message: String,
    /// Human provider name for attribution ("OpenAI", "Anthropic", ...).
    pub provider: Option<String>,
    /// Upstream request id (`x-request-id`-style) so support can trace.
    pub request_id: Option<String>,
    /// Retry hint in seconds, from `Retry-After` upstream.
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

/// Tool args are accumulated as strings but must be presented as JSON when
/// re-serialized; a parse failure on an upstream's malformed argument string
/// degrades to a plain JSON string (call integrity beats pretty shape).
pub fn parse_json_lenient(raw: &str) -> serde_json::Value {
    serde_json::from_str(raw).unwrap_or_else(|_| serde_json::Value::String(raw.to_owned()))
}
