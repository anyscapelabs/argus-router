//! Ingress, Gemini dialect (`POST /v1beta/models/{model}:generateContent`
//! and `:streamGenerateContent?alt=sse`).
//!
//! Parses the request body into IR, re-emits normalized events as
//! `GenerateContentResponse` SSE frames, and maps errors to the Gemini
//! error envelope. Because Gemini identifies tool results by function name
//! rather than call id, parsing assigns synthetic call ids and keeps an
//! id↔name map inside each conversation — which is what keeps tool-call
//! context coherent when the same conversation later switches to an
//! OpenAI-compat (or Anthropic) model.

use serde_json::{json, Map, Value};
use std::collections::HashMap;

use crate::proxy::ir::{
    Block, Chat, ErrCode, Errx, Event, Message, Params, Role, ToolChoice, ToolDef, Turn,
};

/// Splits the `{model}:{action}` path segment; action decides streaming.
pub fn split_path(raw: &str) -> Result<(String, bool), Errx> {
    let invalid = |message: &str| Errx::new(ErrCode::NotFound, 404, message.to_owned());
    let (model, action) = raw
        .rsplit_once(':')
        .ok_or_else(|| invalid("path must be models/<model>:generateContent"))?;
    let streaming = match action {
        "generateContent" => false,
        "streamGenerateContent" => true,
        other => {
            return Err(invalid(&format!("unknown action `{other}`")));
        }
    };
    let model = model.trim().to_owned();
    if model.is_empty() || model.contains('/') {
        return Err(invalid("the model path segment is malformed"));
    }
    Ok((model, streaming))
}

pub fn parse(payload: &Value) -> Result<Chat, Errx> {
    let invalid = |message: &str| Errx::new(ErrCode::InvalidRequest, 400, message.to_owned());
    let contents = payload
        .get("contents")
        .and_then(|found| found.as_array())
        .ok_or_else(|| invalid("`contents` must be an array"))?;

    let mut chat = Chat {
        system: Vec::new(),
        messages: Vec::new(),
        tools: Vec::new(),
        tool_choice: ToolChoice::Auto,
        params: Params::clamped(None, i64::MAX),
    };
    // Mirrors the upstream adapter: Gemini never carries call ids, so the
    // proxy mints stable ids and tracks function names until the response
    // arrives.
    let mut pending: HashMap<String, String> = HashMap::new(); // name → latest id
    let mut call_counter = 0usize;

    for entry in contents {
        let role = match entry.get("role").and_then(|found| found.as_str()) {
            Some("user") => Role::User,
            Some("model") | Some("assistant") => Role::Assistant,
            _ => continue,
        };
        let mut blocks = Vec::new();
        for part in entry
            .get("parts")
            .and_then(|found| found.as_array())
            .map(|parts| parts.as_slice())
            .unwrap_or(&[])
        {
            if let Some(text) = part.get("text").and_then(|found| found.as_str()) {
                if part.get("thought").and_then(|found| found.as_bool()) == Some(true) {
                    blocks.push(Block::Thinking {
                        text: text.to_owned(),
                    });
                } else {
                    blocks.push(Block::Text {
                        text: text.to_owned(),
                    });
                }
                continue;
            }
            if let Some(data) = part.get("inlineData") {
                let mime = data
                    .get("mimeType")
                    .and_then(|found| found.as_str())
                    .unwrap_or("image/png")
                    .to_owned();
                let encoded = data
                    .get("data")
                    .and_then(|found| found.as_str())
                    .unwrap_or("")
                    .to_owned();
                if !encoded.is_empty() {
                    blocks.push(Block::Image {
                        mime,
                        data_b64: encoded,
                    });
                }
                continue;
            }
            if let Some(call) = part.get("functionCall") {
                let name = call
                    .get("name")
                    .and_then(|found| found.as_str())
                    .unwrap_or("")
                    .to_owned();
                if name.is_empty() {
                    return Err(invalid("functionCall parts need a name"));
                }
                call_counter += 1;
                let id = format!("call_{}_{name}", call_counter);
                pending.insert(name.clone(), id.clone());
                let args = call.get("args").cloned().unwrap_or_else(|| json!({}));
                if !args.is_object() {
                    return Err(invalid("functionCall args must be an object"));
                }
                blocks.push(Block::ToolUse {
                    id,
                    name,
                    input_json: args.to_string(),
                });
                continue;
            }
            if let Some(response) = part.get("functionResponse") {
                let name = response
                    .get("name")
                    .and_then(|found| found.as_str())
                    .unwrap_or("");
                let id = pending
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| name.to_owned());
                let output = response
                    .get("response")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let content = match output.get("output") {
                    Some(Value::String(text)) => text.clone(),
                    _ => output.to_string(),
                };
                blocks.push(Block::ToolResult {
                    id,
                    content,
                    is_error: output.get("isError").and_then(|found| found.as_bool()) == Some(true),
                });
                continue;
            }
            // Unknown parts (video metadata etc.) are skipped rather than
            // failing the whole conversation.
        }
        if !blocks.is_empty() {
            chat.messages.push(Message { role, blocks });
        }
    }

    if let Some(system) = payload.get("systemInstruction") {
        for part in system
            .get("parts")
            .and_then(|found| found.as_array())
            .map(|parts| parts.as_slice())
            .unwrap_or(&[])
        {
            if let Some(text) = part.get("text").and_then(|found| found.as_str()) {
                chat.system.push(text.to_owned());
            }
        }
    }

    if let Some(tools) = payload.get("tools").and_then(|found| found.as_array()) {
        for tool_group in tools {
            for tool in tool_group
                .get("functionDeclarations")
                .and_then(|found| found.as_array())
                .map(|found| found.as_slice())
                .unwrap_or(&[])
            {
                let name = tool
                    .get("name")
                    .and_then(|found| found.as_str())
                    .unwrap_or("");
                if name.is_empty() {
                    return Err(invalid("functionDeclarations entries need a name"));
                }
                chat.tools.push(ToolDef {
                    name: name.to_owned(),
                    description: tool
                        .get("description")
                        .and_then(|found| found.as_str())
                        .map(str::to_owned),
                    schema_json: tool
                        .get("parameters")
                        .cloned()
                        .unwrap_or_else(|| json!({"type": "object"}))
                        .to_string(),
                });
            }
        }
    }
    if let Some(config) = payload.pointer("/toolConfig/functionCallingConfig") {
        chat.tool_choice = match config.get("mode").and_then(|found| found.as_str()) {
            Some("NONE") => ToolChoice::None,
            Some("ANY") => match config
                .get("allowedFunctionNames")
                .and_then(|found| found.as_array())
                .and_then(|names| names.first())
                .and_then(|name| name.as_str())
            {
                Some(single) => ToolChoice::named(single.to_owned()),
                None => ToolChoice::Required,
            },
            _ => ToolChoice::Auto,
        };
    }

    if let Some(config) = payload.get("generationConfig") {
        let stop = config
            .get("stopSequences")
            .and_then(|found| found.as_array())
            .map(|items| {
                items
                    .iter()
                    .filter_map(|found| found.as_str().map(str::to_owned))
                    .collect::<Vec<_>>()
            });
        let access = |field: &str| config.get(field).and_then(|found| found.as_u64());
        let float = |field: &str| config.get(field).and_then(|found| found.as_f64());
        let params = Params::clamped(
            Some(crate::proxy::ir::OpenAiParams {
                max_tokens: access("maxOutputTokens"),
                max_completion_tokens: None,
                temperature: float("temperature"),
                top_p: float("topP"),
                stop,
            }),
            // Real clamp happens in `clamp_params` after model resolution.
            i64::MAX,
        );
        chat.params = params;
    }

    Ok(chat)
}

pub fn clamp_params(chat: &mut Chat, max_output: i64) {
    chat.params.max_tokens = chat
        .params
        .max_tokens
        .map(|tokens| tokens.min(max_output.max(1) as u64));
}

pub struct StreamState {
    pub model: String,
    pub saw_tools: bool,
}

impl StreamState {
    pub fn new(model: String) -> Self {
        Self {
            model,
            saw_tools: false,
        }
    }
}

/// One streamed `GenerateContentResponse` frame per event.
pub fn chunk(state: &mut StreamState, event: &Event) -> Option<Value> {
    let mut base = Map::new();
    let mut candidate = Map::new();
    match event {
        Event::TextDelta(text) => {
            candidate.insert(
                "content".to_owned(),
                json!({"role": "model", "parts": [{"text": text}]}),
            );
        }
        Event::ThinkingDelta(text) => {
            candidate.insert(
                "content".to_owned(),
                json!({"role": "model", "parts": [{"text": text, "thought": true}]}),
            );
        }
        Event::ToolUseStart {
            index: _,
            id: _,
            name,
        } => {
            state.saw_tools = true;
            candidate.insert(
                "content".to_owned(),
                json!({"role": "model", "parts": [{"functionCall": {"name": name}}]}),
            );
        }
        // Gemini delivers function calls complete; the upstream adapter
        // already folded args into the start frame, so deltas render as
        // nothing and the end frame finalizes without a part.
        Event::ToolUseDelta { .. } => return None,
        Event::ToolUseEnd { .. } => return None,
        Event::Usage {
            input_tokens,
            output_tokens,
            cache_read_tokens,
        } => {
            base.insert(
                "usageMetadata".to_owned(),
                json!({
                    "promptTokenCount": input_tokens,
                    "candidatesTokenCount": output_tokens - *cache_read_tokens.max(&0),
                    "cachedContentTokenCount": cache_read_tokens,
                    "totalTokenCount": input_tokens + output_tokens,
                }),
            );
        }
    }
    if !candidate.is_empty() {
        base.insert(
            "candidates".to_owned(),
            json!([{"index": 0, "content": Value::Object(candidate)}]),
        );
    }
    if base.is_empty() {
        return None;
    }
    base.insert("modelVersion".to_owned(), json!(state.model));
    Some(Value::Object(base))
}

/// The wrapped-up final frame for streaming (mirrors Gemini's finish chunk).
pub fn final_chunk(state: &StreamState, usage: Option<&Turn>) -> Value {
    let mut frame = json!({
        "modelVersion": state.model,
        "candidates": [{"index": 0, "finishReason": "STOP", "content": {"role": "model", "parts": []}}],
    });
    if let Some(turn) = usage {
        frame["usageMetadata"] = json!({
            "promptTokenCount": turn.usage.input_tokens,
            "candidatesTokenCount": turn.usage.output_tokens,
            "totalTokenCount": turn.usage.input_tokens + turn.usage.output_tokens,
        });
    }
    frame
}

/// A complete (non-streaming) `GenerateContentResponse`.
pub fn completion(state: &StreamState, turn: &Turn) -> Value {
    let mut parts = Vec::<Value>::new();
    if !turn.thinking.is_empty() {
        parts.push(json!({"text": turn.thinking, "thought": true}));
    }
    if !turn.text.is_empty() {
        parts.push(json!({"text": turn.text}));
    }
    for (index, (id, name, arguments)) in turn.tool_use.iter().enumerate() {
        let _ = (id, index);
        parts.push(json!({
            "functionCall": {
                "name": name,
                "args": crate::proxy::parse_json_lenient(arguments),
            },
        }));
    }
    let finish = if turn.tool_use.is_empty() {
        "STOP"
    } else {
        "STOP"
    };
    json!({
        "modelVersion": state.model,
        "candidates": [{
            "index": 0,
            "finishReason": finish,
            "content": {"role": "model", "parts": parts},
        }],
        "usageMetadata": {
            "promptTokenCount": turn.usage.input_tokens,
            "candidatesTokenCount": turn.usage.output_tokens,
            "cachedContentTokenCount": turn.usage.cache_read_tokens,
            "totalTokenCount": turn.usage.input_tokens + turn.usage.output_tokens,
        },
    })
}

/// Gemini error envelope (`error.code` carries the HTTP status).
pub fn error_body(item: &Errx) -> Value {
    json!({
        "error": {
            "code": item.status,
            "message": item.message,
            "status": error_status(item.code),
        }
    })
}

fn error_status(code: ErrCode) -> &'static str {
    match code {
        ErrCode::InvalidRequest => "INVALID_ARGUMENT",
        ErrCode::Unauthorized => "UNAUTHENTICATED",
        ErrCode::Forbidden => "PERMISSION_DENIED",
        ErrCode::NotFound => "NOT_FOUND",
        ErrCode::RateLimited => "RESOURCE_EXHAUSTED",
        ErrCode::Overloaded | ErrCode::Upstream | ErrCode::Network => "UNAVAILABLE",
        ErrCode::ProviderUnconfigured => "UNAVAILABLE",
        ErrCode::Timeout => "DEADLINE_EXCEEDED",
    }
}

/// `GET /v1beta/models` shape for key-holders.
pub fn models_list(entries: Vec<Value>) -> Value {
    json!({"models": entries})
}

/// HTTP response carrying the Gemini error envelope.
pub fn error_response(item: Errx) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use axum::Json;
    let status = StatusCode::from_u16(item.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Json(error_body(&item))).into_response()
}
