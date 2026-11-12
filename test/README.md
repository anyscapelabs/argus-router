# test/

Cross-package tests that belong to the repository rather than to a single
package. Rust integration tests live in `test/api/` and compile against the
gateway's library target (`apps/api/src/lib.rs`); they run with `cargo test`
from `apps/api` (wired up in `apps/api/Cargo.toml` via `[[test]]` entries) or
through the root `npm test` / `bun test` script.

Unit tests stay colocated with the code they cover (`src/**` files contain
`#[cfg(test)]` modules) — same-package white-box tests cannot be moved.
Go tests in the sibling repository follow the same convention (white-box Go
unit tests cannot be relocated; `go test ./...` covers them).

## Layout

- `api/` — Rust integration tests for the gateway (catalog/plan consistency,
  quota parsing, rate window, secret redaction).
