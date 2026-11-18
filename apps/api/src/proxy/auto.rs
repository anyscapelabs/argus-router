use crate::proxy::ir::{Block, Chat, Role};
use crate::proxy::{ErrCode, Errx};
use crate::services::catalog::{AUTO_MODEL_ID, ModelCatalogEntry, Tier};

const SMALL_PROMPT_TOKENS: i64 = 8_000;
const LARGE_PROMPT_TOKENS: i64 = 64_000;
const LARGE_OUTPUT_TOKENS: u64 = 16_000;
const COMPLEX_MESSAGES: usize = 8;
const MANY_TOOLS: usize = 12;

#[derive(Clone, Copy, Debug)]
pub struct RequestSignals {
    pub needs_vision: bool,
    pub needs_tools: bool,
    pub active_tool_loop: bool,
    pub needs_reasoning: bool,
    pub tool_count: usize,
    pub prompt_tokens: i64,
    pub max_output_tokens: Option<u64>,
    pub messages: usize,
}

impl RequestSignals {
    pub fn from_chat(chat: &Chat) -> Self {
        let active_tool_loop = chat.messages.iter().any(|message| {
            message
                .blocks
                .iter()
                .any(|block| matches!(block, Block::ToolUse { .. } | Block::ToolResult { .. }))
        });
        let latest_user_text = chat
            .messages
            .iter()
            .rev()
            .find(|message| message.role == Role::User)
            .map(|message| {
                message
                    .blocks
                    .iter()
                    .filter_map(|block| match block {
                        Block::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        RequestSignals {
            needs_vision: chat.messages.iter().any(|message| {
                message
                    .blocks
                    .iter()
                    .any(|block| matches!(block, Block::Image { .. }))
            }),
            needs_tools: chat.has_tools(),
            active_tool_loop,
            needs_reasoning: active_tool_loop || looks_complex_prompt(&latest_user_text),
            tool_count: chat.tools.len(),
            prompt_tokens: chat.prompt_token_bound(),
            max_output_tokens: chat
                .params
                .max_tokens
                .filter(|tokens| *tokens != i64::MAX as u64),
            messages: chat.messages.len(),
        }
    }
}

fn looks_complex_prompt(prompt: &str) -> bool {
    let trimmed = prompt.trim();
    if trimmed.len() >= 600 || trimmed.contains("```") {
        return true;
    }
    let lower = trimmed.to_ascii_lowercase();
    [
        "debug",
        "refactor",
        "architect",
        "implement",
        "analyze",
        "analyse",
        "compare",
        "design",
        "investigate",
        "root cause",
        "sql",
        "patch",
        "diff",
    ]
    .iter()
    .any(|keyword| lower.contains(keyword))
}

pub fn select(
    candidates: &[ModelCatalogEntry],
    allowlist: &[String],
    signals: &RequestSignals,
) -> Result<ModelCatalogEntry, Errx> {
    let enabled: Vec<&ModelCatalogEntry> = candidates
        .iter()
        .filter(|entry| entry.id != AUTO_MODEL_ID)
        .filter(|entry| allowlist.iter().any(|allowed| allowed == entry.id()))
        .collect();
    if enabled.is_empty() {
        return Err(Errx::new(
            ErrCode::Forbidden,
            403,
            "no models are enabled for your plan, so the auto router has nothing to route to",
        ));
    }
    let fitting: Vec<&ModelCatalogEntry> = enabled
        .iter()
        .copied()
        .filter(|entry| !signals.needs_vision || entry.capabilities.vision)
        .filter(|entry| !signals.needs_tools || entry.capabilities.tools)
        .filter(|entry| signals.prompt_tokens <= entry.context)
        .filter(|entry| {
            signals
                .max_output_tokens
                .is_none_or(|tokens| tokens as i64 <= entry.max_output)
        })
        .collect();
    if fitting.is_empty() {
        let reason = if signals.needs_vision && !enabled.iter().any(|entry| entry.capabilities.vision)
        {
            "none of your enabled models support image input"
        } else if signals.needs_tools && !enabled.iter().any(|entry| entry.capabilities.tools) {
            "none of your enabled models support tool use"
        } else {
            "this request is too large for every enabled model"
        };
        return Err(Errx::new(ErrCode::InvalidRequest, 400, reason));
    }
    let target = target_tier(signals);
    let mut pool = tier_pool(&fitting, target);
    if pool.is_empty() {
        pool = fitting;
    }
    pool.sort_by(|a, b| {
        fit_score(b, signals)
            .cmp(&fit_score(a, signals))
            .then_with(|| {
                price(a)
                    .partial_cmp(&price(b))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(pool[0].clone())
}

fn fit_score(entry: &ModelCatalogEntry, signals: &RequestSignals) -> i64 {
    let mut score = 0i64;
    if signals.needs_reasoning && entry.capabilities.reasoning {
        score += 40;
    }
    if signals.active_tool_loop && entry.capabilities.tools {
        score += 20;
    }
    if entry.context >= signals.prompt_tokens.saturating_mul(2) {
        score += 10;
    }
    let has_fallbacks = entry
        .provider_routing
        .as_ref()
        .and_then(|routing| routing.get("allow_fallbacks"))
        .and_then(|flag| flag.as_bool())
        .unwrap_or(true);
    if has_fallbacks {
        score += 5;
    }
    score
}

fn tier_pool<'a>(
    fitting: &[&'a ModelCatalogEntry],
    target: Tier,
) -> Vec<&'a ModelCatalogEntry> {
    let mut tier = Some(target);
    while let Some(current) = tier {
        let pool: Vec<&'a ModelCatalogEntry> = fitting
            .iter()
            .copied()
            .filter(|entry| entry.tier == current)
            .collect();
        if !pool.is_empty() {
            return pool;
        }
        tier = current.below();
    }
    let mut above = target.above();
    while let Some(current) = above {
        let pool: Vec<&'a ModelCatalogEntry> = fitting
            .iter()
            .copied()
            .filter(|entry| entry.tier == current)
            .collect();
        if !pool.is_empty() {
            return pool;
        }
        above = current.above();
    }
    fitting.to_vec()
}

fn target_tier(signals: &RequestSignals) -> Tier {
    let mut score = 0usize;
    if signals.needs_tools {
        score += 1;
    }
    if signals.active_tool_loop {
        score += 1;
    }
    if signals.needs_reasoning {
        score += 1;
    }
    if signals.tool_count >= MANY_TOOLS {
        score += 1;
    }
    if signals.prompt_tokens > LARGE_PROMPT_TOKENS {
        score += 2;
    } else if signals.prompt_tokens > SMALL_PROMPT_TOKENS {
        score += 1;
    }
    if signals
        .max_output_tokens
        .is_some_and(|tokens| tokens >= LARGE_OUTPUT_TOKENS)
    {
        score += 1;
    }
    if signals.messages > COMPLEX_MESSAGES {
        score += 1;
    }
    match score {
        0 => Tier::Lite,
        1 | 2 => Tier::Standard,
        3 | 4 => Tier::Premium,
        _ => Tier::Ultra,
    }
}

fn price(entry: &ModelCatalogEntry) -> f64 {
    let parse = |value: &str| value.trim().parse::<f64>().unwrap_or(f64::INFINITY);
    parse(&entry.pricing.input) + parse(&entry.pricing.output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::catalog::{Capabilities, Compat, Pricing};

    fn entry(id: &str, tier: Tier, input: &str, output: &str) -> ModelCatalogEntry {
        entry_with(id, tier, input, output, true, true, 1_048_576, 65_536)
    }

    #[allow(clippy::too_many_arguments)]
    fn entry_with(
        id: &str,
        tier: Tier,
        input: &str,
        output: &str,
        vision: bool,
        tools: bool,
        context: i64,
        max_output: i64,
    ) -> ModelCatalogEntry {
        ModelCatalogEntry {
            id: id.to_owned(),
            name: id.to_owned(),
            compat: Compat::Openai,
            provider: "openrouter".to_owned(),
            upstream_model: id.to_owned(),
            provider_routing: None,
            context,
            max_output,
            capabilities: Capabilities {
                tools,
                vision,
                reasoning: false,
            },
            pricing: Pricing {
                input: input.to_owned(),
                output: output.to_owned(),
                cache_read: None,
                cache_write: None,
            },
            free: false,
            tier,
        }
    }

    fn signals() -> RequestSignals {
        RequestSignals {
            needs_vision: false,
            needs_tools: false,
            active_tool_loop: false,
            needs_reasoning: false,
            tool_count: 0,
            prompt_tokens: 1_000,
            max_output_tokens: None,
            messages: 1,
        }
    }

    fn allowlist(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| id.to_string()).collect()
    }

    #[test]
    fn picks_cheapest_within_target_tier() {
        let catalog = vec![
            entry("a/cheap", Tier::Lite, "0.10", "0.50"),
            entry("b/pricier", Tier::Lite, "0.20", "0.60"),
            entry("c/standard", Tier::Standard, "2.00", "10.00"),
        ];
        let picked = select(&catalog, &allowlist(&["a/cheap", "b/pricier", "c/standard"]), &signals())
            .expect("selects a model");
        assert_eq!(picked.id, "a/cheap");
    }

    #[test]
    fn complex_request_routes_to_premium() {
        let catalog = vec![
            entry("a/cheap", Tier::Lite, "0.10", "0.50"),
            entry("c/premium", Tier::Premium, "2.00", "10.00"),
        ];
        let mut complex = signals();
        complex.needs_tools = true;
        complex.prompt_tokens = 200_000;
        complex.messages = 20;
        let picked = select(&catalog, &allowlist(&["a/cheap", "c/premium"]), &complex)
            .expect("selects a model");
        assert_eq!(picked.id, "c/premium");
    }

    #[test]
    fn extreme_request_routes_to_ultra() {
        let catalog = vec![
            entry("a/cheap", Tier::Lite, "0.10", "0.50"),
            entry("c/premium", Tier::Premium, "2.00", "10.00"),
            entry("e/ultra", Tier::Ultra, "4.00", "20.00"),
        ];
        let mut extreme = signals();
        extreme.needs_tools = true;
        extreme.prompt_tokens = 500_000;
        extreme.max_output_tokens = Some(32_000);
        extreme.messages = 20;
        let picked = select(&catalog, &allowlist(&["a/cheap", "c/premium", "e/ultra"]), &extreme)
            .expect("selects a model");
        assert_eq!(picked.id, "e/ultra");
    }

    #[test]
    fn simple_request_never_uses_a_higher_tier() {
        let catalog = vec![entry("c/standard", Tier::Standard, "2.00", "10.00")];
        let picked = select(&catalog, &allowlist(&["c/standard"]), &signals())
            .expect("relaxes to the only fitting model");
        assert_eq!(picked.id, "c/standard");
    }

    #[test]
    fn vision_requirement_filters_models() {
        let catalog = vec![
            entry_with("a/text-only", Tier::Lite, "0.10", "0.50", false, true, 1_048_576, 65_536),
            entry_with("b/vision", Tier::Lite, "0.20", "0.60", true, true, 1_048_576, 65_536),
        ];
        let mut image_request = signals();
        image_request.needs_vision = true;
        let picked = select(&catalog, &allowlist(&["a/text-only", "b/vision"]), &image_request)
            .expect("selects a model");
        assert_eq!(picked.id, "b/vision");
    }

    #[test]
    fn tools_requirement_filters_models() {
        let catalog = vec![
            entry_with("a/no-tools", Tier::Lite, "0.10", "0.50", true, false, 1_048_576, 65_536),
            entry_with("b/tools", Tier::Lite, "0.20", "0.60", true, true, 1_048_576, 65_536),
        ];
        let mut tool_request = signals();
        tool_request.needs_tools = true;
        let picked = select(&catalog, &allowlist(&["a/no-tools", "b/tools"]), &tool_request)
            .expect("selects a model");
        assert_eq!(picked.id, "b/tools");
    }

    #[test]
    fn prompt_must_fit_context() {
        let catalog = vec![
            entry_with("a/tiny", Tier::Lite, "0.10", "0.50", true, true, 100_000, 65_536),
            entry_with("b/huge", Tier::Lite, "0.20", "0.60", true, true, 1_048_576, 65_536),
        ];
        let mut big = signals();
        big.prompt_tokens = 300_000;
        let picked = select(&catalog, &allowlist(&["a/tiny", "b/huge"]), &big)
            .expect("selects a model");
        assert_eq!(picked.id, "b/huge");
    }

    #[test]
    fn output_request_must_fit_max_output() {
        let catalog = vec![
            entry_with("a/small-cap", Tier::Lite, "0.10", "0.50", true, true, 1_048_576, 8_192),
            entry_with("b/big-cap", Tier::Lite, "0.20", "0.60", true, true, 1_048_576, 65_536),
        ];
        let mut long = signals();
        long.max_output_tokens = Some(16_000);
        let picked = select(&catalog, &allowlist(&["a/small-cap", "b/big-cap"]), &long)
            .expect("selects a model");
        assert_eq!(picked.id, "b/big-cap");
    }

    #[test]
    fn lite_plan_pool_routes_among_lite_models() {
        let catalog = vec![
            entry("vendor/a", Tier::Lite, "0.10", "0.50"),
            entry("vendor/b", Tier::Lite, "0.14", "0.28"),
        ];
        let picked = select(&catalog, &allowlist(&["vendor/a", "vendor/b"]), &signals())
            .expect("selects a model");
        assert_eq!(picked.tier, Tier::Lite);
    }

    #[test]
    fn ignores_models_outside_the_plan_allowlist() {
        let catalog = vec![
            entry("a/cheap", Tier::Lite, "0.10", "0.50"),
            entry("c/premium", Tier::Premium, "2.00", "10.00"),
        ];
        let mut complex = signals();
        complex.needs_tools = true;
        complex.prompt_tokens = 200_000;
        complex.messages = 20;
        let picked = select(&catalog, &allowlist(&["a/cheap"]), &complex)
            .expect("relaxes within the allowlist");
        assert_eq!(picked.id, "a/cheap");
    }

    #[test]
    fn excludes_the_auto_alias_from_candidates() {
        let catalog = vec![
            entry(AUTO_MODEL_ID, Tier::Lite, "0", "0"),
            entry("a/cheap", Tier::Lite, "0.10", "0.50"),
        ];
        let picked = select(&catalog, &allowlist(&[AUTO_MODEL_ID, "a/cheap"]), &signals())
            .expect("selects a model");
        assert_eq!(picked.id, "a/cheap");
    }

    #[test]
    fn empty_allowlist_is_forbidden() {
        let catalog = vec![entry("a/cheap", Tier::Lite, "0.10", "0.50")];
        let error = select(&catalog, &[], &signals()).expect_err("empty allowlist errors");
        assert_eq!(error.status, 403);
    }

    #[test]
    fn unmet_vision_need_names_the_cause() {
        let catalog = vec![entry_with(
            "a/text-only",
            Tier::Lite,
            "0.10",
            "0.50",
            false,
            true,
            1_048_576,
            65_536,
        )];
        let mut image_request = signals();
        image_request.needs_vision = true;
        let error = select(&catalog, &allowlist(&["a/text-only"]), &image_request)
            .expect_err("no vision-capable model");
        assert_eq!(error.status, 400);
        assert!(error.message.contains("image"));
    }

    #[test]
    fn oversized_request_names_the_cause() {
        let catalog = vec![entry_with(
            "a/tiny",
            Tier::Lite,
            "0.10",
            "0.50",
            true,
            true,
            1_000,
            65_536,
        )];
        let mut big = signals();
        big.prompt_tokens = 100_000;
        let error = select(&catalog, &allowlist(&["a/tiny"]), &big)
            .expect_err("prompt exceeds every model");
        assert_eq!(error.status, 400);
        assert!(error.message.contains("large"));
    }

    #[test]
    fn ties_break_on_id_for_determinism() {
        let catalog = vec![
            entry("b/twin", Tier::Lite, "0.10", "0.50"),
            entry("a/twin", Tier::Lite, "0.10", "0.50"),
        ];
        let picked = select(&catalog, &allowlist(&["b/twin", "a/twin"]), &signals())
            .expect("selects a model");
        assert_eq!(picked.id, "a/twin");
    }

    #[test]
    fn unparsable_pricing_loses_to_parsable() {
        let catalog = vec![
            entry("a/broken", Tier::Lite, "n/a", "n/a"),
            entry("b/known", Tier::Lite, "0.10", "0.50"),
        ];
        let picked = select(&catalog, &allowlist(&["a/broken", "b/known"]), &signals())
            .expect("selects a model");
        assert_eq!(picked.id, "b/known");
    }

    #[test]
    fn signals_read_images_tools_and_bound_from_chat() {
        use crate::proxy::ir::{Message, Params, Role, ToolDef};
        let chat = Chat {
            system: vec!["sys".to_owned()],
            messages: vec![
                Message {
                    role: Role::User,
                    blocks: vec![Block::Text {
                        text: "hello".to_owned(),
                    }],
                },
                Message {
                    role: Role::User,
                    blocks: vec![Block::Image {
                        mime: "image/png".to_owned(),
                        data_b64: "AAAA".to_owned(),
                    }],
                },
            ],
            tools: vec![ToolDef {
                name: "search".to_owned(),
                description: None,
                schema_json: "{}".to_owned(),
            }],
            tool_choice: crate::proxy::ir::ToolChoice::Auto,
            params: Params {
                max_tokens: Some(1_000),
                temperature: None,
                top_p: None,
                stop: Vec::new(),
            },
        };
        let signals = RequestSignals::from_chat(&chat);
        assert!(signals.needs_vision);
        assert!(signals.needs_tools);
        assert_eq!(signals.max_output_tokens, Some(1_000));
        assert_eq!(signals.messages, 2);
        assert!(signals.prompt_tokens > 0);
    }
}
