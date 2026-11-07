//! Upstream adapters in the desktop app's conversion style: a resolved
//! `Target` plus per-origin `Quirks`, dispatched per adapter family. Only
//! the OpenAI-compatible family is wired upstream (OpenRouter fronts every
//! provider); new direct-origin families plug into the same `Adapter` shape.

use std::collections::{HashMap, HashSet};

use futures_util::Stream;
use serde_json::{json, Value};

use crate::proxy::ir::{
    Block, Chat, ErrCode, Errx, Event, Quirks, Role, ToolChoice, Turn, Usage,
};
use crate::proxy::sse::Framer;
use crate::services::catalog::Compat;
use crate::services::providers::Provider;

/// Boxed event streams produced by adapters (`async_stream` yields
/// unnameable types, same trade-off the desktop's `iter.Seq` carries).
pub type EventStream = std::pin::Pin<Box<dyn Stream<Item = Result<Event, Errx>> + Send>>;

/// Extra knobs an adapter family may tune per call (desktop's
/// `Router.TargetOptions`).
#[derive(Clone, Debug, Default)]
pub struct TargetOptions {
    pub quirks: Option<Quirks>,
}

/// A fully resolved upstream call target (desktop's `Protocol.Target`).
#[derive(Clone, Debug)]
pub struct Target {
    /// Kept for the desktop parity surface (family dispatch happens at
    /// call time; only the OpenAI-compat family is wired today).
    #[allow(dead_code)]
    pub family: Compat,
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    pub extra_headers: &'static [(&'static str, &'static str)],
    pub quirks: Quirks,
    pub provider_routing: Option<serde_json::Value>,
}

/// Desktop's `Router.TargetFor`: validates a model id + origin pair into a
/// concrete target. The URL only ever derives from the registry — no client
/// input reaches it.
pub fn target_for(
    provider: &'static Provider,
    model: &str,
    api_key: &str,
    opts: TargetOptions,
) -> Result<Target, Errx> {
    if model.is_empty() {
        return Err(Errx::new(ErrCode::InvalidRequest, 400, "a model id is required"));
    }
    let base = provider.base_url.trim_end_matches('/');
    if base.is_empty() {
        return Err(Errx::new(
            ErrCode::Upstream,
            503,
            "the provider has no base URL",
        ));
    }
    Ok(Target {
        family: provider.compat,
        base_url: base.to_owned(),
        model: model.to_owned(),
        api_key: api_key.to_owned(),
        extra_headers: provider.extra_headers,
        quirks: opts.quirks.unwrap_or(provider.quirks),
        provider_routing: None,
    })
}

/// Adapter-wide cap so a malformed upstream error body cannot flood memory.
const ERROR_BODY_CAP: usize = 1024 * 1024;

/// The shared reqwest client: redirects forbidden (a redirect could leak
/// the origin key), tight connect timeout, no total timeout — long
/// generations must not die mid-stream.
pub fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("static client configuration")
}

fn apply_auth(request: reqwest::RequestBuilder, target: &Target) -> reqwest::RequestBuilder {
    let mut keyed = request.bearer_auth(&target.api_key);
    for (name, value) in target.extra_headers {
        keyed = keyed.header(*name, *value);
    }
    keyed
}

/// Builds one upstream chat call. The mirror of the desktop's
/// `ad.BuildRequest`.
fn build_request(target: &Target, chat: &Chat, streaming: bool) -> Value {
    let mut payload = json!({
        "model": target.model,
        "messages": build_messages(chat),
        "stream": streaming,
    });
    if let Some(max_tokens) = chat.params.max_tokens {
        payload[target.quirks.max_tokens_field] = json!(max_tokens);
    }
    if let Some(temperature) = chat.params.temperature {
        payload["temperature"] = json!(temperature);
    }
    if let Some(top_p) = chat.params.top_p {
        payload["top_p"] = json!(top_p);
    }
    if !chat.params.stop.is_empty() {
        payload["stop"] = json!(chat.params.stop);
    }
    let provider_routing = target
        .provider_routing
        .clone()
        .unwrap_or_else(|| json!({"data_collection": "deny", "sort": "throughput", "allow_fallbacks": true}));
    payload["provider"] = provider_routing;
    if chat.has_tools() {
        payload["tools"] = json!(chat
            .tools
            .iter()
            .map(|tool| {
                json!({
                    "type": "function",
                    "function": {
                        "name": tool.name,
                        "description": tool.description.clone().unwrap_or_default(),
                        "parameters": crate::proxy::parse_json_lenient(&tool.schema_json),
                    },
                })
            })
            .collect::<Vec<_>>());
        payload["tool_choice"] = json!(match &chat.tool_choice {
            ToolChoice::Auto => json!("auto"),
            ToolChoice::None => json!("none"),
            ToolChoice::Required => json!("required"),
            ToolChoice::Named(name) => json!({"type": "function", "function": {"name": name}}),
            ToolChoice::NamedOwned(name) => {
                json!({"type": "function", "function": {"name": name}})
            }
        });
    }
    payload
}

fn build_messages(chat: &Chat) -> Vec<Value> {
    let mut out = Vec::new();
    for system in &chat.system {
        out.push(json!({"role": "system", "content": system}));
    }
    for message in &chat.messages {
        match message.role {
            Role::User => {
                let mut content = Vec::<Value>::new();
                for block in &message.blocks {
                    match block {
                        Block::Text { text } => {
                            content.push(json!({"type": "text", "text": text}));
                        }
                        Block::Image { mime, data_b64 } => {
                            content.push(json!({
                                "type": "image_url",
                                "image_url": {
                                    "url": format!("data:{mime};base64,{data_b64}"),
                                },
                            }));
                        }
                        // Tool results become their own `tool` messages;
                        // anything else in a user turn stays in the content
                        // array.
                        Block::ToolResult { id, content, .. } => {
                            out.push(json!({
                                "role": "tool",
                                "tool_call_id": id,
                                "content": content,
                            }));
                        }
                        Block::Thinking { .. } | Block::ToolUse { .. } => {}
                    }
                }
                if !content.is_empty() {
                    let text_only = content
                        .iter()
                        .all(|part| part.get("type").and_then(|t| t.as_str()) == Some("text"));
                    let content_value = if text_only && content.len() == 1 {
                        content[0]["text"].clone()
                    } else {
                        Value::Array(content)
                    };
                    out.push(json!({"role": "user", "content": content_value}));
                }
            }
            Role::Assistant => {
                let mut text = String::new();
                let mut tool_calls = Vec::<Value>::new();
                for block in &message.blocks {
                    match block {
                        Block::Text { text: fragment } => text.push_str(fragment),
                        Block::ToolUse {
                            id,
                            name,
                            input_json,
                        } => tool_calls.push(json!({
                            "id": id,
                            "type": "function",
                            "function": {"name": name, "arguments": input_json},
                        })),
                        Block::Thinking { .. } | Block::Image { .. }
                        | Block::ToolResult { .. } => {}
                    }
                }
                let mut entry = json!({"role": "assistant"});
                if text.is_empty() && tool_calls.is_empty() {
                    // Neither text nor calls: a dropped no-op turn keeps the
                    // upstream from ever seeing a malformed empty message.
                    continue;
                }
                entry["content"] = json!(text);
                if !tool_calls.is_empty() {
                    entry["tool_calls"] = Value::Array(tool_calls);
                }
                out.push(entry);
            }
        }
    }
    out
}

/// Sends the request and maps non-2xx responses to normalized errors —
/// the mirror of the desktop's `ad.MapError` plus `NetworkError`.
async fn send_chat(
    client: &reqwest::Client,
    target: &Target,
    body: Value,
) -> Result<reqwest::Response, Errx> {
    let url = format!("{}{}", target.base_url, target.quirks.chat_path);
    let request = apply_auth(client.post(url), target);
    let response = request
        .json(&body)
        .send()
        .await
        .map_err(|error| network_errx(&error))?;
    if !response.status().is_success() {
        return Err(errx_from_response(target, response).await);
    }
    Ok(response)
}

fn network_errx(error: &reqwest::Error) -> Errx {
    if error.is_timeout() {
        Errx::new(ErrCode::Timeout, 504, "the provider did not respond in time")
    } else {
        Errx::new(ErrCode::Network, 502, "reaching the provider failed")
    }
}

async fn errx_from_response(target: &Target, response: reqwest::Response) -> Errx {
    let status = response.status().as_u16();
    let request_id = ["x-request-id", "request-id", "openai-request-id"]
        .iter()
        .find_map(|header| {
            response
                .headers()
                .get(*header)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
        });
    let retry_after = response
        .headers()
        .get("retry-after")
        .and_then(|value| value.to_str().ok().and_then(|raw| raw.trim().parse::<u64>().ok()));
    let bytes = response.bytes().await.unwrap_or_default();
    let message = extract_error_message(&bytes);
    let mut errx = Errx::from_upstream_status(status, message).with_provider(target.model.as_str());
    if let Some(request_id) = request_id {
        errx.request_id = Some(request_id);
    }
    if let Some(retry_after) = retry_after {
        errx.retry_after = Some(retry_after);
    }
    errx
}

fn extract_error_message(bytes: &[u8]) -> String {
    let head = &bytes[..bytes.len().min(ERROR_BODY_CAP)];
    let text = String::from_utf8_lossy(head);
    if let Ok(value) = serde_json::from_str::<Value>(&text) {
        let message =
            value
                .pointer("/error/message")
                .and_then(|found| found.as_str())
                .or_else(|| value.get("message").and_then(|found| found.as_str()));
        if let Some(message) = message {
            return message.to_owned();
        }
    }
    let trimmed = text.trim();
    if trimmed.is_empty() {
        "the upstream provider request failed".to_owned()
    } else {
        trimmed.chars().take(300).collect()
    }
}

/// Streams normalized events out of an upstream SSE response — the mirror
/// of the desktop's `ad.ParseStream`. Tool-call deltas arrive as a
/// raw-indexed array of fragments; the adapter assigns stable canonical
/// indexes so every ingress dialect sees an identical shape regardless of
/// which short session host spilled them.
fn encode_stream(
    mut response: reqwest::Response,
    quirks: Quirks,
) -> impl Stream<Item = Result<Event, Errx>> {
    async_stream::stream! {
        let _ = quirks;
        let mut framer = Framer::default();
        let mut next_canonical = 0usize;
        let mut raw_map: HashMap<u64, usize> = HashMap::new();
        let mut id_map: HashMap<String, usize> = HashMap::new();
        let mut open: HashSet<usize> = HashSet::new();
        let mut usage: Option<Usage> = None;

        'outer: loop {
            let chunk = match response.chunk().await {
                Ok(Some(bytes)) => bytes,
                Ok(None) => break 'outer,
                Err(_) => {
                    yield Err(Errx::new(
                        ErrCode::Network,
                        502,
                        "the provider stream dropped mid-generation",
                    ));
                    return;
                }
            };
            for data in framer.push(&chunk) {
                if data == "[DONE]" {
                    break 'outer;
                }
                let payload: Value = match serde_json::from_str(&data) {
                    Ok(found) => found,
                    Err(_) => continue,
                };
                if let Some(bad) = payload.get("error") {
                    let message = bad
                        .get("message")
                        .and_then(|found| found.as_str())
                        .unwrap_or("the provider reported a mid-stream error")
                        .to_owned();
                    yield Err(Errx::new(ErrCode::Upstream, 502, message));
                    return;
                }
                if let Some(found) = payload.get("usage").filter(|found| !found.is_null()) {
                    if let Some(found) = usage_of(found) {
                        usage = Some(found);
                    }
                }
                let choice = payload
                    .pointer("/choices/0")
                    .filter(|found| !found.is_null())
                    .cloned()
                    .unwrap_or_else(|| json!(null));
                let delta = choice.get("delta").cloned().unwrap_or_else(|| json!(null));

                if let Some(fragment) = delta.get("reasoning_content").and_then(|v| v.as_str()) {
                    if !fragment.is_empty() {
                        yield Ok(Event::ThinkingDelta(fragment.to_owned()));
                    }
                }
                if let Some(fragment) = delta.get("content").and_then(|v| v.as_str()) {
                    if !fragment.is_empty() {
                        yield Ok(Event::TextDelta(fragment.to_owned()));
                    }
                }
                if let Some(calls) = delta.get("tool_calls").and_then(|v| v.as_array()) {
                    for call in calls {
                        // OpenAI-compat providers carry a raw array index;
                        // a few omit it, in which case calls are keyed by
                        // their `id` (or simply assigned in order).
                        let canonical = if let Some(raw_index) =
                            call.get("index").and_then(|v| v.as_u64())
                        {
                            *raw_map
                                .entry(raw_index)
                                .or_insert_with(|| new_canonical(&mut next_canonical))
                        } else if let Some(id) = call.get("id").and_then(|v| v.as_str()) {
                            *id_map
                                .entry(id.to_owned())
                                .or_insert_with(|| new_canonical(&mut next_canonical))
                        } else {
                            new_canonical(&mut next_canonical)
                        };
                        if !open.contains(&canonical) {
                            let id = call
                                .get("id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("call_0")
                                .to_owned();
                            let name = call
                                .pointer("/function/name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_owned();
                            open.insert(canonical);
                            yield Ok(Event::ToolUseStart {
                                index: canonical,
                                id,
                                name,
                            });
                        }
                        if let Some(fragment) =
                            call.pointer("/function/arguments").and_then(|v| v.as_str())
                        {
                            if !fragment.is_empty() {
                                yield Ok(Event::ToolUseDelta {
                                    index: canonical,
                                    value_json: fragment.to_owned(),
                                });
                            }
                        }
                    }
                }
            }
        }

        // Close any call still streaming argument fragments.
        for index in open.drain() {
            yield Ok(Event::ToolUseEnd { index });
        }
        if let Some(usage) = usage {
            yield Ok(Event::Usage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                cache_read_tokens: usage.cache_read_tokens,
            });
        }
    }
}

fn new_canonical(next: &mut usize) -> usize {
    let canonical = *next;
    *next += 1;
    canonical
}

fn usage_of(value: &Value) -> Option<Usage> {
    let input_tokens = value.get("prompt_tokens")?.as_i64()?;
    let output_tokens = value
        .get("completion_tokens")
        .and_then(|found| found.as_i64())
        .unwrap_or(0);
    let cache_read_tokens = value
        .pointer("/prompt_tokens_details/cached_tokens")
        .and_then(|found| found.as_i64())
        .unwrap_or(0);
    Some(Usage {
        input_tokens,
        output_tokens,
        cache_read_tokens,
    })
}

fn decode_nonstream(payload: &Value) -> Turn {
    let mut turn = Turn::default();
    if let Some(usage) = payload.get("usage").filter(|found| !found.is_null()) {
        if let Some(usage) = usage_of(usage) {
            turn.usage = usage;
        }
    }
    let message = payload.pointer("/choices/0/message");
    if let Some(text) = message
        .and_then(|found| found.get("content"))
        .and_then(|found| found.as_str())
    {
        turn.text = text.to_owned();
    }
    if let Some(thinking) = message
        .and_then(|found| found.get("reasoning_content"))
        .and_then(|found| found.as_str())
    {
        turn.thinking = thinking.to_owned();
    }
    if let Some(calls) = message
        .and_then(|found| found.get("tool_calls"))
        .and_then(|found| found.as_array())
    {
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
            turn.tool_use.push((id, name, arguments));
        }
    }
    turn
}

/// Streaming chat through a resolved target. The desktop's `Router.Run`
/// mirror.
pub async fn chat_stream(
    client: &reqwest::Client,
    target: &Target,
    chat: Chat,
) -> Result<EventStream, Errx> {
    let mut body = build_request(target, &chat, true);
    if target.quirks.stream_options {
        if let Some(object) = body.as_object_mut() {
            object.insert("stream_options".to_owned(), json!({"include_usage": true}));
        }
    }
    let response = send_chat(client, target, body).await?;
    Ok(Box::pin(encode_stream(response, target.quirks)))
}

/// Non-streaming chat through a resolved target.
pub async fn chat_once(
    client: &reqwest::Client,
    target: &Target,
    chat: Chat,
) -> Result<Turn, Errx> {
    let body = build_request(target, &chat, false);
    let response = send_chat(client, target, body).await?;
    let bytes = response
        .bytes()
        .await
        .map_err(|_| Errx::new(ErrCode::Network, 502, "reaching the provider failed"))?;
    let payload: Value = serde_json::from_slice(&bytes).map_err(|_| {
        Errx::new(
            ErrCode::Upstream,
            502,
            "the model returned an unreadable response",
        )
    })?;
    Ok(decode_nonstream(&payload))
}
