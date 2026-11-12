# Anyscape Gateway API (`apps/api`)

Rust (Axum) gateway: service-account key auth, per-plan quota enforcement
and model/search/voice proxying. One process, one Postgres database.

## Routes

| Route | Purpose |
| --- | --- |
| `/api/v1/auth/*` | better-auth (email/password, social OAuth, verification) |
| `/v1/chat/completions` | OpenAI-compatible chat proxy (desktop `openai_compat` adapter) |
| `/v1/models` | key-eligible catalog (plan-effective allowlist) |
| `/v1/search` | web search router |
| `/v1beta/models/{model}:{action}` | Gemini-compatible chat proxy |
| `/v1/live` | Gemini Live speech-to-speech WebSocket |
| `/api/v1/account/entitlements` | identity + plan + effective allowlist for paired desktop keys |
| `/api/v1/{models,plans,usage,workspace,profile,agents}` | console dashboard API |

## Auth model

- **Console sessions** — short-lived browser sessions via better-auth.
  Sign-up sends a verification email; unverified accounts cannot sign in,
  create keys, or use the product.
- **Service keys** (`argus_pk_...`) — long-lived, created from the dashboard
  (Settings → Service Accounts). Keys authenticate the *ingress* and
  entitlement routes only; they can never act as a browser session, and
  they never appear in logs (query-string secrets are redacted from the
  request span).
- **Effective allowlist** — a user's plan defines the maximum set of
  callable models; personal preferences only narrow it. Plans refresh on
  boot from `plans/plans.json`; admins (`ADMIN_USER_IDS`/`ADMIN_EMAILS`) may
  re-seed the shared catalog/plans via `/refresh` endpoints.

## Quotas (enforced per request)

- Per-minute RPM window (plan `ratePerMin`)
- Daily request count (plan `dailyRequests`, atomic conditional upsert)
- Rolling monthly/weekly token totals from completed `usage_logs` rows
- Monthly web searches (plan `searchesPerMonth`, atomic counter) and voice
  minutes (plan `voiceMinutes`) with in-flight session reservations

## Environment

See [`.env.example`](./.env.example). Highlights: `DATABASE_URL`,
`BETTER_AUTH_SECRET` (≥32 chars), `DASHBOARD_URL`, `OPENROUTER_API_KEY`
(upstream origin), `GEMINI_API_KEY` (voice), `UPSTASH_REDIS_REST_URL/TOKEN`
(optional cache), `API_DB_MAX_CONNECTIONS` (default 10), plus optional
`EXA/BRAVE/TAVILY` search keys.

## Test

```sh
cargo test            # unit + integration (test/ tree, see ../../../test/README.md)
```

Migrations are automatic on boot. Usage analytics live in `usage_logs` —
keep it indexed by user/created_at (already created).
