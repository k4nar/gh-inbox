# gh-inbox

## Product
Extension for `gh` to manage Github notifications like an inbox.

See @docs/specs/product.md for full details.

## Stack
- Backend: Rust HTTP server (axum + tokio) + SQLite
- Frontend: Svelte 5 + Bits UI
- Data: gh CLI as the data layer (shell out from Rust, no direct GitHub API)
- No external DB — local state only (SQLite via sqlx)

## Architecture
See @docs/specs/architecture.md for full details.

Keep things simple.

## Non-negotiables
- Never call GitHub API directly — always proxy through `gh` CLI
- Route handlers are thin adapters; business logic stays in Rust modules
- All async handlers must propagate typed errors (no `.unwrap()` in handlers)
- Frontend never holds auth tokens

## Dev workflow
- `cargo run` to start the server (opens browser automatically)
- `cargo test` before any PR
- Commit on every working checkpoint

## On new sessions
Read @docs/specs/roadmap.md and check off completed items before starting work.
