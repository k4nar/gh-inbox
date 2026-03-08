# gh-inbox

## Product
Extension for `gh` to manage Github notifications like an inbox.

See @docs/specs/product.md for full details.

## Stack
- Backend: Rust HTTP server (axum + tokio) + SQLite
- Frontend: Svelte 5 + Bits UI
- Data: GitHub REST API via `reqwest` (token from `gh auth token` at startup)
- No external DB — local state only (SQLite via sqlx)

## Architecture
See @docs/specs/architecture.md for full details.

Keep things simple.

## Non-negotiables
- Never call GitHub API from the frontend — all GitHub data flows through the Rust backend
- `gh` CLI is used only once at startup (`gh auth token`) — never per request
- Route handlers are thin adapters; business logic stays in Rust modules
- All async handlers must propagate typed errors (no `.unwrap()` in handlers)
- Frontend never holds auth tokens

## Dev workflow
- `cargo run` to start the server (opens browser automatically)
- `cargo test` before any PR
- Commit on every working checkpoint

## On new sessions
Read @docs/specs/roadmap.md and check off completed items before starting work.
