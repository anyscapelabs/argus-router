# Anyscape Gateway

Anyscape Gateway is the model-gateway service behind the Argus Agent desktop
app: one API that authenticates service-account keys, enforces per-plan
quotas, and proxies chat / search / voice traffic across model providers
(upstream origin: OpenRouter; direct: Google Gemini). It ships as a Rust
(Axum + Postgres) API plus a Next.js console.

## Layout

```
apps/
  api/         Rust gateway (Axum, better-auth, sqlx/Postgres)
  dashboard/   Next.js console (React 19, Tailwind 4, TanStack Query)
packages/      shared UI + eslint/tsconfig packages
test/          cross-package Rust integration tests (see test/README.md)
```

## Quickstart

Requirements: Bun 1.3+, Rust stable, Postgres (Neon works well), Node 24+.

```sh
# 1. configure the API
cp apps/api/.env.example apps/api/.env   # fill in BETTER_AUTH_SECRET, DATABASE_URL, DASHBOARD_URL ...

# 2. configure the dashboard
cp apps/dashboard/.env.example apps/dashboard/.env   # NEXT_PUBLIC_API_URL=http://localhost:8080

# 3. run both (API on :8080, console on :3000)
bun run dev
```

The database schema migrates itself on boot (better-auth entities + app
tables); plan seeds from `apps/api/plans/plans.json` are refreshed on every
start. `ADMIN_USER_IDS` / `ADMIN_EMAILS` control who may call the shared
catalog/plan-refresh endpoints — with neither set, those endpoints are
closed to everyone.

## Test

```sh
bun test                              # cargo test for apps/api
cargo test --manifest-path apps/api/Cargo.toml
bun run check-types                   # tsc across workspaces
bun run lint                          # eslint
```

## Deploy

`apps/api` is a single binary (`cargo build --release`) that listens on
:8080 — put it behind TLS terminator of your choice. `apps/dashboard`
builds statically-ish with `bun run build` and runs with `bun start`;
required env vars are the ones in the example files.

## Notes

- The desktop app is "Argus Agent"; this service is "Anyscape Gateway".
  Internal identifiers (key prefix `argus_pk_`, Redis namespace `argus:`,
  env var names such as `BETTER_AUTH_*`, provider id `argus-router`) are
  stable — the rename was presentation-only.
- Cross-tenant safety invariants: per-user model preferences narrow the
  plan allowlist (never widen), service keys cannot act as browser
  sessions, and email verification is required before any product access.
