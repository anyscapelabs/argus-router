//! Ingress, OpenAI dialect (`POST /v1/chat/completions`).
//!
//! Parses requests into IR, re-emits normalized events as
//! `chat.completion.chunk` SSE frames, and maps errors to the OpenAI error
//! envelope. Assistant tool-call history survives intact across model
//! switches because extraction/assembly both go through the canonical IR.

use serde_json::{Map, Value, json};

use crate::api::ingress::new_id;
use crate::proxy::ir::{
    Block, Chat, ErrCode, Errx, Event, Message, OpenAiParams, Params, Role, ToolChoice, ToolDef,
    Turn,
};

/// Parses the request body; returns the requested model id plus the
/// normalized turn. Unknown roles are ignored so future clients don't break.
pub fn parse(payload: &Value) -> Result<(String, Chat), Errx> {
    let invalid = |message: &str| Errx::new(ErrCode::InvalidRequest, 400, message.to_owned());
    let model = payload
        .get("model")
        .and_then(|found| found.as_str())
        .filter(|found| !found.is_empty())
        .ok_or_else(|| invalid("`model` is required"))?
        .to_owned();
    let messages = payload
        .get("messages")
        .and_then(|found| found.as_array())
        .ok_or_else(|| invalid("`messages` must be an array"))?;

    let mut chat = Chat {
        system: Vec::new(),
        messages: Vec::new(),
        tools: Vec::new(),
        tool_choice: ToolChoice::Auto,
        // Leave the client's cap untouched here; the endpoint clamps it to
        // `entry.max_output` via `clamp_params` once the model is resolved.
        // (Passing 0 here used to squash every cap down to 1 token.)
        params: Params::clamped(params_of(payload), i64::MAX),
    };

    for message in messages {
        let role = message
            .get("role")
            .and_then(|found| found.as_str())
            .unwrap_or("");
        let content = message.get("content");
        match role {
            "system" | "developer" => {
                if let Some(text) = text_of(content) {
                    chat.system.push(text);
                }
            }
            "user" => {
                let mut blocks = Vec::new();
                match content {
                    Some(Value::Array(parts)) => {
                        for part in parts {
                            let kind = part
                                .get("type")
                                .and_then(|found| found.as_str())
                                .unwrap_or("");
                            if kind == "text" || part.get("text").is_some() {
                                if let Some(text) =
                                    part.get("text").and_then(|found| found.as_str())
                                {
                                    blocks.push(Block::Text {
                                        text: text.to_owned(),
                                    });
                                }
                            } else if kind == "image_url" {
                                let url = part
                                    .pointer("/image_url/url")
                                    .and_then(|found| found.as_str())
                                    .unwrap_or("");
                                if let Some(image) = data_uri_image(url) {
                                    blocks.push(Block::Image {
                                        mime: image.0,
                                        data_b64: image.1,
                                    });
                                } else {
                                    return Err(invalid(
                                        "only inline base64 image data URLs are supported",
                                    ));
                                }
                            }
                        }
                    }
                    other => {
                        if let Some(text) = string_of(other) {
                            blocks.push(Block::Text { text });
                        }
                    }
                }
                if !blocks.is_empty() {
                    chat.messages.push(Message {
                        role: Role::User,
                        blocks,
                    });
                }
            }
            "assistant" => {
                let mut blocks = Vec::new();
                if let Some(text) = string_of(content) {
                    if !text.is_empty() {
                        blocks.push(Block::Text { text });
                    }
                }
                if let Some(calls) = message.get("tool_calls").and_then(|found| found.as_array()) {
                    for call in calls {
                        let id = call
                            .get("id")
                            .and_then(|found| found.as_str())
                            .unwrap_or("call_0")
                            .to_owned();
                        let name = call
                            .pointer("/function/name")
                            .and_then(|found| found.as_str())
                            .unwrap_or("")
                            .to_owned();
                        let arguments = call
                            .pointer("/function/arguments")
                            .and_then(|found| found.as_str())
                            .unwrap_or("{}")
                            .to_owned();
                        blocks.push(Block::ToolUse {
                            id,
                            name,
                            input_json: arguments,
                        });
                    }
                }
                if !blocks.is_empty() {
                    chat.messages.push(Message {
                        role: Role::Assistant,
                        blocks,
                    });
                }
            }
            "tool" => {
                let id = message
                    .get("tool_call_id")
                    .and_then(|found| found.as_str())
                    .unwrap_or("")
                    .to_owned();
                let content = string_of(content).unwrap_or_default();
                chat.messages.push(Message {
                    role: Role::User,
                    blocks: vec![Block::ToolResult {
                        id,
                        content,
                        is_error: false,
                    }],
                });
            }
            "function" | _ => {}
        }
    }

    if let Some(tools) = payload.get("tools").and_then(|found| found.as_array()) {
        for tool in tools {
            let name = tool
                .pointer("/function/name")
                .and_then(|found| found.as_str())
                .unwrap_or("");
            if name.is_empty() {
                return Err(invalid("every tool needs a function name"));
            }
            let schema = tool
                .pointer("/function/parameters")
                .cloned()
                .unwrap_or_else(|| json!({"type": "object"}));
            if !schema.is_object() {
                return Err(invalid("tool parameter schemas must be objects"));
            }
            chat.tools.push(ToolDef {
                name: name.to_owned(),
                description: tool
                    .pointer("/function/description")
                    .and_then(|found| found.as_str())
                    .map(str::to_owned),
                schema_json: schema.to_string(),
            });
        }
    }
    if let Some(choice) = payload.get("tool_choice") {
        if let Some(choice) = tool_choice(choice) {
            chat.tool_choice = choice;
        }
    }
    Ok((model, chat))
}

fn params_of(payload: &Value) -> Option<OpenAiParams> {
    if !payload.is_object() {
        return None;
    }
    let stop = match payload.get("stop") {
        Some(Value::Array(items)) => Some(
            items
                .iter()
                .filter_map(|found| found.as_str().map(str::to_owned))
                .collect::<Vec<_>>(),
        ),
        Some(Value::String(text)) if !text.is_empty() => Some(vec![text.clone()]),
        _ => None,
    };
    let access = |field: &str| payload.get(field).and_then(|found| found.as_u64());
    let float = |field: &str| payload.get(field).and_then(|found| found.as_f64());
    if access("max_tokens").is_none()
        && access("max_completion_tokens").is_none()
        && float("temperature").is_none()
        && float("top_p").is_none()
        && stop.is_none()
    {
        return None;
    }
    Some(OpenAiParams {
        max_tokens: access("max_tokens"),
        max_completion_tokens: access("max_completion_tokens"),
        temperature: float("temperature"),
        top_p: float("top_p"),
        stop,
    })
}

fn tool_choice(choice: &Value) -> Option<ToolChoice> {
    match choice {
        Value::String(raw) if !raw.is_empty() => match raw.as_str() {
            "none" => Some(ToolChoice::None),
            "required" => Some(ToolChoice::Required),
            _ => Some(ToolChoice::Auto),
        },
        Value::Object(_) => {
            let name = choice
                .pointer("/function/name")
                .and_then(|found| found.as_str())?;
            Some(ToolChoice::named(name.to_owned()))
        }
        _ => None,
    }
}

/// Joins text parts and leaves non-text parts out (those are handled in the
/// content loops above).
fn text_of(content: Option<&Value>) -> Option<String> {
    match content? {
        Value::String(text) => Some(text.clone()),
        Value::Array(parts) => Some(
            parts
                .iter()
                .filter_map(|part| part.get("text").and_then(|found| found.as_str()))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        _ => None,
    }
}

fn string_of(content: Option<&Value>) -> Option<String> {
    match content? {
        Value::String(text) => Some(text.clone()),
        Value::Array(parts) => Some(
            parts
                .iter()
                .filter_map(|part| part.get("text").and_then(|found| found.as_str()))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        Value::Null => None,
        _ => None,
    }
}

fn data_uri_image(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("data:")?;
    let (mime, data) = rest.split_once(";base64,")?;
    let mime = if mime.is_empty() {
        "image/png".to_owned()
    } else {
        mime.to_owned()
    };
    Some((mime, data.to_owned()))
}

/// Re-clamps generation parameters against the resolved entry.
pub fn clamp_params(chat: &mut Chat, max_output: i64) {
    chat.params.max_tokens = chat
        .params
        .max_tokens
        .map(|tokens| tokens.min(max_output.max(1) as u64));
}

pub struct StreamState {
    pub id: String,
    pub created: i64,
    pub model: String,
    pub saw_tools: bool,
}

impl StreamState {
    pub fn new(model: String) -> Self {
        Self {
            id: new_id("chatcmpl"),
            created: chrono::Utc::now().timestamp(),
            model,
            saw_tools: false,
        }
    }
}

/// The TCP/SSE data payload for one event (`chat.completion.chunk` without
/// the `data:` wrapper — axum's Sse adds that).
pub fn chunk(state: &mut StreamState, event: &Event) -> Option<Value> {
    let mut base = Map::new();
    base.insert("id".to_owned(), json!(state.id));
    base.insert("object".to_owned(), json!("chat.completion.chunk"));
    base.insert("created".to_owned(), json!(state.created));
    base.insert("model".to_owned(), json!(state.model));
    match event {
        Event::TextDelta(text) => {
            base.insert(
                "choices".to_owned(),
                json!([{"index": 0, "delta": {"content": text}, "finish_reason": Value::Null}]),
            );
        }
        Event::ThinkingDelta(text) => {
            base.insert(
                "choices".to_owned(),
                json!([{"index": 0, "delta": {"reasoning_content": text}, "finish_reason": Value::Null}]),
            );
        }
        Event::ToolUseStart { index, id, name } => {
            state.saw_tools = true;
            base.insert(
                "choices".to_owned(),
                json!([{"index": 0, "delta": {"tool_calls": [{
                    "index": index, "id": id, "type": "function",
                    "function": {"name": name, "arguments": ""},
                }]}, "finish_reason": Value::Null}]),
            );
        }
        Event::ToolUseDelta { index, value_json } => {
            base.insert(
                "choices".to_owned(),
                json!([{"index": 0, "delta": {"tool_calls": [{
                    "index": index, "function": {"arguments": value_json},
                }]}, "finish_reason": Value::Null}]),
            );
        }
        // OpenAI-compat has no per-call end marker; the finish chunk seals
        // the turn.
        Event::ToolUseEnd { .. } => return None,
        Event::Usage {
            input_tokens,
            output_tokens,
            cache_read_tokens,
        } => {
            base.insert("choices".to_owned(), json!([]));
            base.insert(
                "usage".to_owned(),
                json!({
                    "prompt_tokens": input_tokens,
                    "completion_tokens": output_tokens,
                    "cached_tokens": cache_read_tokens,
                    "total_tokens": input_tokens + output_tokens,
                }),
            );
        }
    }
    Some(Value::Object(base))
}

/// The final chunk: finish_reason sealed, then the protocol terminator.
pub fn final_chunk(state: &StreamState) -> Value {
    json!({
        "id": state.id,
        "object": "chat.completion.chunk",
        "created": state.created,
        "model": state.model,
        "choices": [{"index": 0, "delta": {}, "finish_reason": crate::api::ingress::openai::final_reason(state)}],
    })
}

fn final_reason(state: &StreamState) -> &'static str {
    if state.saw_tools {
        "tool_calls"
    } else {
        "stop"
    }
}

/// A complete (non-streaming) `chat.completion` response.
pub fn completion(state: &StreamState, turn: &Turn) -> Value {
    let mut message = json!({"role": "assistant"});
    if turn.text.is_empty() {
        message["content"] = Value::Null;
    } else {
        message["content"] = json!(turn.text);
    }
    if !turn.tool_use.is_empty() {
        message["tool_calls"] =
            json!(turn.tool_use.iter().enumerate().map(|(index, (id, name, arguments))| {
            json!({
                "id": if id.is_empty() { format!("call_{index}_{name}") } else { id.clone() },
                "type": "function",
                "function": {"name": name, "arguments": arguments},
            })
        }).collect::<Vec<_>>());
    }
    json!({
        "id": state.id,
        "object": "chat.completion",
        "created": state.created,
        "model": state.model,
        "choices": [{
            "index": 0,
            "message": message,
            "finish_reason": if turn.tool_use.is_empty() { "stop" } else { "tool_calls" },
        }],
        "usage": {
            "prompt_tokens": turn.usage.input_tokens,
            "completion_tokens": turn.usage.output_tokens,
            "cached_tokens": turn.usage.cache_read_tokens,
            "total_tokens": turn.usage.input_tokens + turn.usage.output_tokens,
        },
    })
}

/// OpenAI error envelope.
pub fn error_body(item: &Errx) -> Value {
    json!({
        "error": {
            "message": item.message,
            "type": error_type(item.code),
            "code": item.code.as_str(),
            "param": Value::Null,
        }
    })
}

fn error_type(code: ErrCode) -> &'static str {
    match code {
        ErrCode::InvalidRequest => "invalid_request_error",
        ErrCode::Unauthorized => "authentication_error",
        ErrCode::Forbidden => "permission_error",
        ErrCode::NotFound => "invalid_request_error",
        ErrCode::RateLimited => "rate_limit_error",
        ErrCode::Overloaded => "overloaded_error",
        ErrCode::Upstream | ErrCode::Network | ErrCode::Timeout => "api_error",
        ErrCode::ProviderUnconfigured => "api_error",
    }
}

/// `GET /v1/models` shape for key-holders.
pub fn models_list(entries: Vec<Value>) -> Value {
    json!({
        "object": "list",
        "data": entries,
    })
}

/// HTTP response carrying the OpenAI error envelope.
pub fn error_response(item: Errx) -> axum::response::Response {
    use axum::Json;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    let status = StatusCode::from_u16(item.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (status, Json(error_body(&item))).into_response()
}
