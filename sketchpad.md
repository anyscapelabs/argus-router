# Argus Router Pricing Sketchpad

Working notes for pricing and limits. Stages 1–2 (catalog/groups/prices and plan limits) are implemented; dollar metering, router changes, and dashboard changes remain pending.

## What changed from the earlier model

- Dollar metering. Usage is enforced as an internal dollar allowance per plan and shown to users as percentages. The earlier per-group token quotas are gone. Every request is charged at its real cost, so adding models cannot silently burn money.
- Realistic cost model. Planning uses the mix seen in the logs: about 98% input, 2% output, and about half of input read from cache. This replaces the earlier 85/15 no-cache worst case and roughly doubles or triples usable tokens.
- Bedrock caching is counted. Bedrock models are assumed to get cache reads at 25% of input until real cache prices are synced.
- Groups come from price, not hand-picking. `google/gemini-4-carbon` is back in Max.
- No paying users yet, so no subscriber migration. New plans go live with new Dodo products.
- Dashboard shows weekly and monthly usage only, as percentages. No daily meter.

## Plans at a glance

| Field | Free | Starter | Max | Max Plus |
|---|---:|---:|---:|---:|
| Price per month | `$0` | `$5` | `$10` | `$15` |
| Compute budget, worst case internal | `$0.50` | `$3.00` | `$8.00` | `$12.00` |
| Margin before payment fees | n/a | `$2.00` | `$2.00` | `$3.00` |
| Chat allowance, internal | `$0.48` | `$2.53` | `$6.83` | `$9.97` |
| Web searches per month | 5 | 60 | 150 | 250 |
| Voice minutes per month | 0 | 10 | 25 | 45 |
| Model groups | lite | lite + standard | lite + standard + premium | all four |
| Models available | 8 | 19 | 24 | 29 |
| Auto router | lite only | yes | yes | yes |
| Weekly limit | 40% of monthly | 40% | 40% | 40% |
| Monthly limit | 100% | 100% | 100% | 100% |
| Concurrent requests, proposed | 2 | 3 | 5 | 8 |

The compute budget is what a user costs us at 100% use of everything: chat, search, and voice. The chat allowance is the compute budget minus the worst-case cost of the plan's searches at `$0.004` each and voice minutes at `$0.023` each. Search and voice have monthly limits only.

## Cost model

- Observed mix from AWS RDS `usage_logs`, 458 requests: average 7,853 input and 180 output tokens per request, about 50% of input read from cache. Output is about 2.2% of all tokens.
- Effective cost per 1M tokens:
  `0.978 × (0.5 × input + 0.5 × cache_read) + 0.022 × output`, at 50% cache hits.
- Strong caching at 80% cuts cost further, roughly 35% on average for models with cheap cache reads.
- Sample size is small, but dollar metering makes this safe. If real traffic is more output-heavy, users simply get fewer tokens and we never exceed the allowance.
- Bedrock cache-read prices are an assumption. Bedrock prompt caching exists, but the catalog has no cache-read prices for Bedrock models. This model assumes cache reads cost 25% of input on every Bedrock model, conservative for Claude and possibly wrong for others.
- Cache-read prices are capped at the input price until stale entries are fixed.

## Model groups

Groups are assigned by effective cost per 1M tokens at the reference mix above, not by hand:

- Lite: under `$0.10`
- Standard: `$0.10` to `$0.70`
- Premium: `$0.70` to `$1.40`
- Ultra: above `$1.40`

A small overrides file can pin a group or block a model. `z-ai/glm-5.3` is pinned to standard until its cache price is refreshed.

| Group | Model | Provider | Effective $/1M |
|---|---|---|---:|
| Lite | `z-ai/glm-5.3-flash` | OpenRouter | 0.036 |
| Lite | `bedrock/nova-lite` | Bedrock | 0.042 |
| Lite | `minimax/minimax-m3.1-flash` | OpenRouter | 0.051 |
| Lite | `bedrock/glm-4.7-flash` | Bedrock | 0.052 |
| Lite | `anthropic/claude-haiku-5.5` | OpenRouter | 0.065 |
| Lite | `openai/gpt-6-luna` | OpenRouter | 0.065 |
| Lite | `deepseek/deepseek-v4.1-flash-1017` | OpenRouter | 0.088 |
| Lite | `qwen/qwen3.8-flash` | OpenRouter | 0.092 |
| Standard | `z-ai/glm-5.3` pinned | OpenRouter | 0.100 |
| Standard | `minimax/minimax-m3` | OpenRouter | 0.203 |
| Standard | `bedrock/nova-2-lite` | Bedrock | 0.239 |
| Standard | `bedrock/glm-4.7` | Bedrock | 0.416 |
| Standard | `moonshotai/kimi-k3.1-pro` | OpenRouter | 0.419 |
| Standard | `bedrock/deepseek-v3.2` | Bedrock | 0.420 |
| Standard | `bedrock/kimi-k2.5` | Bedrock | 0.434 |
| Standard | `deepseek/deepseek-v4-pro` | OpenRouter | 0.495 |
| Standard | `google/gemini-3.8-flash` | OpenRouter | 0.532 |
| Standard | `bedrock/nova-pro` | Bedrock | 0.560 |
| Standard | `bedrock/glm-5` | Bedrock | 0.683 |
| Premium | `bedrock/claude-haiku-4.5` | Bedrock | 0.723 |
| Premium | `x-ai/grok-4.7` | OpenRouter | 1.161 |
| Premium | `anthropic/claude-sonnet-5.5` | OpenRouter | 1.250 |
| Premium | `google/gemini-4-carbon` | OpenRouter | 1.250 |
| Premium | `openai/gpt-6.1-sol` | OpenRouter | 1.250 |
| Ultra | `bedrock/kimi-k3` | Bedrock | 2.169 |
| Ultra | `anthropic/claude-opus-5.5` | OpenRouter | 2.599 |
| Ultra | `bedrock/kimi-k3.1` | Bedrock | 2.892 |
| Ultra | `bedrock/claude-opus-4.5` | Bedrock | 3.615 |
| Ultra | `anthropic/claude-fable-5.2` | OpenRouter | 5.197 |

Blocked models:

- `bedrock/minimax-m2.1` and `bedrock/minimax-m2.5` — hung in earlier testing.
- `qwen/qwen3.8-max` — not live on OpenRouter. Nearest live ID is `qwen/qwen3.8-max-0902` at `$2 / $6`.

Access by plan:

| Plan | Groups | Models |
|---|---|---:|
| Free | lite | 8 |
| Starter | lite + standard | 19 |
| Max | lite + standard + premium, including `google/gemini-4-carbon` | 24 |
| Max Plus | lite + standard + premium + ultra | 29 |

`argus/auto` is available on every plan and resolves server-side to that plan's models. Logs show the model as `Auto`.

## Weekly and monthly limits

- Weekly limit: 40% of the monthly allowance. Resets Monday 00:00 UTC.
- Monthly limit: 100% of the allowance. For subscribers it follows the billing cycle, not the calendar month. Free users use the UTC calendar month.
- No daily limit and no daily request cap.
- Search and voice: monthly counts only, no weekly or daily limits.
- When a limit is hit, the API returns 429 with the reset time. Users see only percentages.

| Field | Free | Starter | Max | Max Plus |
|---|---:|---:|---:|---:|
| Monthly chat allowance, internal | `$0.48` | `$2.53` | `$6.83` | `$9.97` |
| Weekly limit, internal | `$0.19` | `$1.01` | `$2.73` | `$3.99` |
| About tokens per month, default Auto routing | 8.6M | 34.1M | 79.6M | 107.4M |
| About tokens per week | 3.4M | 13.6M | 31.8M | 43.0M |
| Search / month | 5 | 60 | 150 | 250 |
| Voice minutes / month | 0 | 10 | 25 | 45 |

Token estimates assume the 8 lite models weighted toward the cheapest, average `$0.056/M`, plus 10% of requests escalated to standard, average `$0.31/M`, at 50% cache hits. Strong caching adds roughly a third. These are estimates, not promises. The dollar meter is the limit.

### Group usage caps

Caps are maximum shares of the monthly chat allowance, not allocations. Lite is always the largest share and the fallback. Weekly caps are 40% of each monthly cap. When a tier cap is exhausted, Auto skips that tier and degrades downward to Lite.

| Plan | Lite | Standard | Premium | Ultra | Premium + ultra |
|---|---:|---:|---:|---:|---:|
| Free | 100% | — | — | — | — |
| Starter | 70% | 30% | — | — | — |
| Max | 60% | 25% | 15% | — | 15% |
| Max Plus | 55% | 25% | 15% | 5% | 20% |

Dollar caps:

| Plan | Lite | Standard | Premium | Ultra |
|---|---:|---:|---:|---:|
| Free | $0.48 | — | — | — |
| Starter | $1.77 | $0.76 | — | — |
| Max | $4.10 | $1.71 | $1.02 | — |
| Max Plus | $5.48 | $2.49 | $1.50 | $0.50 |

Average token caps, using Lite `$0.056/M`, Standard `$0.31/M`, Premium `$1.13/M`, Ultra `$3.29/M`:

| Plan | Lite | Standard | Premium | Ultra |
|---|---:|---:|---:|---:|
| Free | 8.57M | — | — | — |
| Starter | 31.62M | 2.45M | — | — |
| Max | 73.18M | 5.51M | 0.91M | — |
| Max Plus | 97.92M | 8.04M | 1.33M | 0.15M |

What the same chat allowance buys on one model:

| Model | Max `$6.83` | Max Plus `$9.97` |
|---|---:|---:|
| `z-ai/glm-5.3-flash` | 190M tokens | 277M tokens |
| `anthropic/claude-sonnet-5.5` | 5.5M tokens | 8.0M tokens |
| `anthropic/claude-opus-5.5` | 2.6M tokens | 3.8M tokens |
| `anthropic/claude-fable-5.2` | 1.3M tokens | 1.9M tokens |

The 200M-token numbers other products advertise are only reachable on the cheapest cached models. Showing percentages avoids promising a token count.

## Search and voice pricing

Both are monthly only and stay outside the chat meter.

| Search provider | Cost per query | First in chain for |
|---|---:|---|
| Exa | `$0.001` | never |
| Brave | `$0.003` | simple queries |
| Tavily | `$0.004` | complex queries |

Budgets use `$0.004` per query.

Voice uses Gemini Live: audio input `$0.005/min` plus audio output `$0.018/min`, so `$0.023/min` worst case. Minutes are counted as `CEIL(session ms / 60000)` per session.

## Verified price notes

USD per 1M tokens:

| Model | Input | Output |
|---|---:|---:|
| `deepseek/deepseek-v4-pro` | 0.90 | 1.50 |
| `google/gemini-3.8-flash` | 0.825 | 4.125 |
| `moonshotai/kimi-k3.1-pro` | 0.253 | 8.80 |
| `minimax/minimax-m3.1-flash` | 0.075 | 0.30 |
| `qwen/qwen3.8-flash` | 0.15 | 0.47 |
| `z-ai/glm-5.3` | 0.0699 | 1.40 |
| `z-ai/glm-5.3-flash` | 0.0399 | 0.0784 |

Bedrock global profiles verified: `nova-2-lite`, `claude-haiku-4.5`, `claude-opus-4.5`, `kimi-k3`, and `kimi-k3.1`. No global profile for Nova Lite, Nova Pro, Kimi K2.5, GLM 4.7, GLM 4.7 Flash, GLM 5, MiniMax M2.x, or DeepSeek V3.2.
