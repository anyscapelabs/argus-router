# Anyscape Gateway Console (`apps/dashboard`)

Next.js console for Anyscape Gateway: account, plans, models, usage and
service-account keys for the Argus Agent desktop app.

## Develop

```sh
cp .env.example .env          # NEXT_PUBLIC_API_URL=http://localhost:8080
bun dev                       # http://localhost:3000
```

Requires the API (`apps/api`) to be running; auth calls go to
`${NEXT_PUBLIC_API_URL}/api/v1/auth/**`. The desktop pairs against the same
API.

## Scripts

```sh
bun run build         # production build
bun start             # serve the build
bun run lint          # eslint (0 errors)
bun run check-types   # tsc
```

## Flows

- **Sign up / sign in** via email (verification required — unverified
  accounts cannot use the product) or Google/GitHub OAuth.
- **Models** — toggle per-user model availability; the effective allowlist
  is your plan's set narrowed by your toggles.
- **Usage** — totals, per-model/per-key breakdowns, logs.
- **Settings → Service Accounts** — create `argus_pk_` keys, paste one into
  the desktop app's pairing screen. Key shown once at creation.

## Env

| Var | Purpose |
| --- | --- |
| `NEXT_PUBLIC_API_URL` | API origin (required) |
| `NEXT_PUBLIC_ARGUS_DOWNLOAD_URL` | optional: public URL for desktop builds; when unset the overview guide offers the pairing-key flow instead of dead download buttons |
