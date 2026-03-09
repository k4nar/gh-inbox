# gh-inbox Roadmap

## How to use this file

This is the shared source of truth for what to build next.

**At the start of every session:**
1. Check off any tasks that are already done in the codebase.
2. Pick up at the first incomplete task in the current milestone.

**Rules:**
- A milestone is done only when its "Done when" condition is met — not before.
- Check off tasks as they are completed, not in advance.
- Do not start M(n+1) before M(n) is fully done.
- Each milestone should leave the project in a runnable state.
- Keep tasks atomic: one clear action, one clear output.

---

## M0 — Rust binary that serves a page

Goal: `cargo run` starts a server, opens the browser, and shows something. Backend tooling is in place.

- [x] Init `Cargo.toml` with deps: `axum`, `tokio`, `open`, `serde`, `serde_json`
- [x] `src/main.rs`: bind to a random available localhost port, start axum server, open browser
- [x] `src/server.rs`: axum router with a single `GET /` route returning `"gh-inbox works"`
- [x] Add `tower` and `http-body-util` as dev-dependencies (axum integration test helpers)
- [x] Integration test for `GET /`: assert 200
- [x] Integration test for unknown routes: assert 404
- [x] Ensure `cargo fmt`, `cargo clippy` pass with no warnings
- [x] Confirm: `cargo run` opens browser showing the text; `cargo test` passes

**Done when:** `cargo run` opens the browser, page loads, and `cargo test` passes with both route tests green.

---

## M1 — Frontend scaffold with dev proxy

Goal: The Svelte app is set up with a Vite dev server proxying `/api/*` to the Rust backend. Hot reload works. Frontend test infra is in place.

- [x] Init frontend: `npm create vite frontend -- --template svelte` (Svelte 5)
- [x] Add `bits-ui` dependency
- [x] Configure Vite dev server to proxy `/api/*` to the Rust backend port
- [x] Build the static layout: topbar, sidebar (Inbox / Archived / Repos / Teams), PR list area — styled to match `docs/mockups/inbox.html`
- [x] Add `vitest` and `@testing-library/svelte` as dev-dependencies; configure Vitest in `vite.config.ts` with jsdom environment
- [x] Add `npm test` script to `frontend/package.json`
- [x] Test: sidebar renders the Inbox and Archived nav items
- [x] Test: `reasonLabel(reason)` utility maps `review_requested`, `mention`, `assign` to the correct label strings
- [x] Confirm: `npm test` passes; Vite dev server shows the styled inbox shell with hot reload

**Done when:** `npm run dev` shows the styled inbox shell with hot reload, and `npm test` passes.

---

## M2 — SQLite data layer

Goal: The database layer is set up with migrations and tested. The schema matches the architecture spec.

- [x] Add `sqlx` with SQLite feature to `Cargo.toml`
- [x] `src/db/mod.rs`: init SQLite database (create file in OS data dir if missing), run migrations
- [x] Migration: create `notifications` table (id, pr_id, reason, unread, archived, updated_at)
- [x] Migration: create `pull_requests` table (id, title, repo, author, url, ci_status, last_viewed_at)
- [x] Migration: create `comments` table (id, pr_id, thread_id, author, body, created_at)
- [x] Migration: create `last_fetched_at` table (resource, fetched_at)
- [x] `src/db/queries.rs`: basic CRUD — insert/upsert notification, query inbox (unarchived), query archived
- [x] Validate data model against architecture spec: confirm all fields from the API surface can be served from these tables
- [x] Unit tests for queries: insert + query round-trip, archive/unarchive, upsert idempotency
- [x] Wire DB init into `main.rs` startup (after token acquisition, before server start)
- [x] Confirm: `cargo test` passes; server starts and creates the DB file

**Done when:** `cargo test` passes with query tests green, and the server creates a valid SQLite DB on startup.

---

## M3 — GitHub API client + first API endpoint

Goal: The backend acquires a token, fetches notifications from GitHub, caches them in SQLite, and serves `GET /api/inbox`. Tested end-to-end.

- [x] At startup, run `gh auth token` (one subprocess) and store the token in an `Arc<str>` shared across handlers
- [x] `src/models/notification.rs`: typed `Notification` struct (id, title, repository, reason, updated_at, url)
- [x] `src/github/mod.rs`: `parse_notifications(json: &str) -> Result<Vec<Notification>>` (pure) + `fetch_notifications(token, client)` that calls `GET https://api.github.com/notifications`
- [x] `src/api/inbox.rs`: thin handler for `GET /api/inbox` — calls `fetch_notifications`, caches to SQLite, returns JSON
- [x] Wire the route in `src/server.rs`; add typed error type; no `.unwrap()` in handlers
- [x] Unit tests for `parse_notifications`: valid input, missing fields, empty array, malformed JSON
- [x] Unit tests for error variants: confirm each maps to the expected HTTP status code
- [x] Integration test for `GET /api/inbox`: mock the GitHub API response, assert correct JSON shape and 200
- [x] Confirm: `curl localhost:<port>/api/inbox` returns real data and `cargo test` passes

**Done when:** The endpoint returns real data from the GitHub API, data is cached in SQLite, and all tests pass.

---

## M4 — Inbox UI with real data

Goal: The frontend fetches and renders real notifications from the API. Key UI components are tested.

- [x] Frontend: `GET /api/inbox` on load, store results in Svelte 5 state (`$state`)
- [x] Render each notification as a PR row: repo, author, title/PR number, reason label, updated_at
- [x] Reason label maps `review_requested` / `mention` / `assign` to the pill styles from the mockup
- [x] Show unread dot for unread notifications (use `unread` field from gh response)
- [x] Empty state when inbox is empty
- [x] Test: PR row renders repo, title, reason pill, and unread dot correctly
- [x] Test: empty state renders when inbox is empty
- [x] Confirm: running the app shows real PRs from your GitHub notifications; `npm test` passes

**Done when:** Real notifications appear in the inbox UI and all frontend tests pass.

---

## M5 — Embed frontend in binary

Goal: The binary serves the compiled Svelte app. Single-binary distribution works.

- [x] Add `include_dir` crate to `Cargo.toml`
- [x] Add a `build.rs` that runs `npm run build` inside `frontend/` so the dist is ready before `rustc` compiles
- [x] `src/server.rs`: use `include_dir!` to embed `frontend/dist/` at compile time; serve embedded files from `/` with correct MIME types and SPA fallback to `index.html`
- [x] Keep the existing dev-mode behaviour (`cfg!(debug_assertions)` → Vite dev server); only serve embedded assets in release mode
- [x] Integration test: build the app with `--release`, assert `GET /` returns HTML containing the Svelte mount point (not the old plain-text response)
- [x] Confirm: `cargo build --release && ./target/release/gh-inbox` serves the full app with no external files

**Done when:** `cargo build --release` produces a single binary that serves the Svelte app, and `cargo test` still passes (dev-mode tests unchanged).