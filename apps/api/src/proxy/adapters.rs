use std::collections::{HashMap, HashSet};

use futures_util::Stream;
use serde_json::{Value, json};

use crate::proxy::ir::{
    Block, Chat, ErrCode, Errx, Event, FinishReason, Quirks, Role, ToolChoice, Turn, Usage,
};
use crate::proxy::sse::Framer;
use crate::services::catalog::Compat;
use crate::services::providers::Provider;

pub type EventStream = std::pin::Pin<Box<dyn Stream<Item = Result<Event, Errx>> + Send>>;

#[derive(Clone, Debug, Default)]
pub struct TargetOptions {
    pub quirks: Option<Quirks>,
}

#[derive(Clone, Debug)]
pub struct Target {
    #[allow(dead_code)]
    pub family: Compat,
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    pub extra_headers: Vec<(String, String)>,
    pub quirks: Quirks,
    pub provider_routing: Option<serde_json::Value>,
}

pub fn target_for(
    provider: &'static Provider,
    model: &str,
    api_key: &str,
    opts: TargetOptions,
) -> Result<Target, Errx> {
    if model.is_empty() {
        return Err(Errx::new(
            ErrCode::InvalidRequest,
            400,
            "a model id is required",
        ));
    }
    let base = provider.base_url.trim_end_matches('/');
    if base.is_empty() {
        return Err(Errx::new(
            ErrCode::Upstream,
            503,
            "the provider has no base URL",
        ));
    }
    let mut extra_headers: Vec<(String, String)> = provider
        .extra_headers
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();
    if provider.id == "openrouter" {
        let title = std::env::var("OPENROUTER_APP_TITLE")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "Argus Agent".to_owned());
        extra_headers.push(("X-OpenRouter-Title".to_owned(), title.clone()));
        extra_headers.push(("X-Title".to_owned(), title));

        let referer = std::env::var("OPENROUTER_APP_URL")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .or_else(|| {
                std::env::var("DASHBOARD_URL")
                    .ok()
                    .map(|value| value.trim().to_owned())
                    .filter(|value| !value.is_empty())
            });
        if let Some(referer) = referer {
            extra_headers.push(("HTTP-Referer".to_owned(), referer.clone()));
            extra_headers.push(("Referer".to_owned(), referer));
        }
    }
    Ok(Target {
        family: provider.compat,
        base_url: base.to_owned(),
        model: model.to_owned(),
        api_key: api_key.to_owned(),
        extra_headers,
        quirks: opts.quirks.unwrap_or(provider.quirks),
        provider_routing: None,
    })
}

const MAX_UPSTREAM_BYTES: usize = 32 * 1024 * 1024;

async fn capped_bytes(response: reqwest::Response) -> Result<Vec<u8>, Errx> {
    if let Some(len) = response.content_length() {
        if len > MAX_UPSTREAM_BYTES as u64 {
            return Err(Errx::new(
                ErrCode::Upstream,
                502,
                "the provider sent an oversized response",
            ));
        }
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|_| {
            Errx::new(
                ErrCode::Network,
                502,
                "reading the provider response failed",
            )
        })?
        .to_vec();
    if bytes.len() > MAX_UPSTREAM_BYTES {
        return Err(Errx::new(
            ErrCode::Upstream,
            502,
            "the provider sent an oversized response",
        ));
    }
    Ok(bytes)
}

pub fn build_client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("static client configuration")
}

fn apply_auth(request: reqwest::RequestBuilder, target: &Target) -> reqwest::RequestBuilder {
    let mut keyed = request.bearer_auth(&target.api_key);
    for (name, value) in &target.extra_headers {
        keyed = keyed.header(name.as_str(), value.as_str());
    }
    keyed
}

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
    let provider_routing = target.provider_routing.clone().unwrap_or_else(
        || json!({"data_collection": "deny", "sort": "throughput", "allow_fallbacks": true}),
    );
    payload["provider"] = provider_routing;
    if chat.has_tools() {
        payload["tools"] = json!(
            chat.tools
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
                .collect::<Vec<_>>()
        );
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
                        Block::Thinking { .. } | Block::Image { .. } | Block::ToolResult { .. } => {
                        }
                    }
                }
                let mut entry = json!({"role": "assistant"});
                if text.is_empty() && tool_calls.is_empty() {
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
        Errx::new(
            ErrCode::Timeout,
            504,
            "the provider did not respond in time",
        )
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
    let retry_after = response.headers().get("retry-after").and_then(|value| {
        value
            .to_str()
            .ok()
            .and_then(|raw| raw.trim().parse::<u64>().ok())
    });
    drop(response);
    tracing::warn!(status, request_id = ?request_id, model = %target.model, "upstream provider rejected a request");
    let (code, client_status, message) = match status {
        400 | 404 | 422 => (
            ErrCode::InvalidRequest,
            status,
            "the upstream provider rejected the request",
        ),
        408 => (
            ErrCode::Timeout,
            504,
            "the upstream provider did not respond in time",
        ),
        429 => (
            ErrCode::RateLimited,
            429,
            "the upstream provider is temporarily rate limited",
        ),
        _ => (
            ErrCode::Upstream,
            502,
            "the upstream provider request failed",
        ),
    };
    let mut errx = Errx::new(code, client_status, message).with_provider(target.model.as_str());
    if let Some(request_id) = request_id {
        errx.request_id = Some(request_id);
    }
    if let Some(retry_after) = retry_after {
        errx.retry_after = Some(retry_after);
    }
    errx
}

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
        let mut finish_reason: Option<FinishReason> = None;

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
                    let code = bad.get("code").and_then(Value::as_i64);
                    tracing::warn!(?code, "upstream provider reported a mid-stream error");
                    yield Err(Errx::new(
                        ErrCode::Upstream,
                        502,
                        "the upstream provider stream failed",
                    ));
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
                if let Some(found) = choice.get("finish_reason").and_then(Value::as_str) {
                    finish_reason = Some(parse_finish_reason(found));
                }
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

        for index in open.drain() {
            yield Ok(Event::ToolUseEnd { index });
        }
        yield Ok(Event::Finish(finish_reason.unwrap_or(FinishReason::Stop)));
        if let Some(usage) = usage {
            yield Ok(Event::Usage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                cache_read_tokens: usage.cache_read_tokens,
            });
        }
    }
}

fn parse_finish_reason(value: &str) -> FinishReason {
    match value {
        "length" => FinishReason::Length,
        "tool_calls" | "function_call" => FinishReason::ToolCalls,
        "content_filter" => FinishReason::ContentFilter,
        _ => FinishReason::Stop,
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
    if let Some(reason) = payload
        .pointer("/choices/0/finish_reason")
        .and_then(Value::as_str)
    {
        turn.finish_reason = parse_finish_reason(reason);
    }
    if let Some(usage) = payload.get("usage").filter(|found| !found.is_null()) {
        if let Some(usage) = usage_of(usage) {
            turn.usage = usage;
            turn.usage_reported = true;
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

pub async fn chat_once(
    client: &reqwest::Client,
    target: &Target,
    chat: Chat,
) -> Result<Turn, Errx> {
    let body = build_request(target, &chat, false);
    let response = send_chat(client, target, body).await?;
    let bytes = capped_bytes(response).await?;
    let payload: Value = serde_json::from_slice(&bytes).map_err(|_| {
        Errx::new(
            ErrCode::Upstream,
            502,
            "the model returned an unreadable response",
        )
    })?;
    if payload.get("error").is_some()
        || payload
            .get("choices")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty)
    {
        tracing::warn!("upstream provider returned an invalid success envelope");
        return Err(Errx::new(
            ErrCode::Upstream,
            502,
            "the upstream provider returned an invalid response",
        ));
    }
    Ok(decode_nonstream(&payload))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{decode_nonstream, parse_finish_reason};
    use crate::proxy::ir::FinishReason;

    #[test]
    fn keeps_nonstream_length_finish_reason() {
        let turn = decode_nonstream(&json!({
            "choices": [{
                "finish_reason": "length",
                "message": {"content": "partial"}
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5}
        }));
        assert_eq!(turn.finish_reason, FinishReason::Length);
        assert_eq!(turn.text, "partial");
    }

    #[test]
    fn maps_stream_length_finish_reason() {
        assert_eq!(parse_finish_reason("length"), FinishReason::Length);
    }
}
