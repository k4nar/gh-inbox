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

Goal: `cargo run` starts a server, opens the browser, and shows something.

- [ ] Init `Cargo.toml` with deps: `axum`, `tokio`, `open`, `serde`, `serde_json`
- [ ] `src/main.rs`: bind to a random available localhost port, start axum server, open browser
- [ ] `src/server.rs`: axum router with a single `GET /` route returning `"gh-inbox works"`
- [ ] Confirm: `cargo run` opens browser showing the text

**Done when:** `cargo run` opens the browser and the page loads.

---

## M1 — Backend tests

Goal: Test infrastructure is in place and the server scaffold is covered. Establishes the pattern for all future tests.

- [ ] Add `tower` and `http-body-util` as dev-dependencies (axum integration test helpers)
- [ ] Integration test for `GET /`: assert 200
- [ ] Integration test for unknown routes: assert 404
- [ ] Confirm: `cargo test` passes with no warnings

**Done when:** `cargo test` passes with at least the two route tests green.

---

## M2 — Frontend scaffold embedded in binary

Goal: The binary serves the compiled Svelte app. The UI matches the mockup shell (no real data yet).

- [ ] Init frontend: `npm create vite frontend -- --template svelte` (Svelte 5)
- [ ] Add `bits-ui` dependency
- [ ] Build the static layout: topbar, sidebar (Inbox / Archived / Repos / Teams), PR list area — styled to match `docs/mockups/inbox.html`
- [ ] Add `include_dir` crate; embed `frontend/dist/` in binary at build time
- [ ] `src/server.rs`: serve embedded static assets from `/` (fallback to `index.html` for SPA)
- [ ] Add build script or `Makefile` target that runs `npm run build` then `cargo build`
- [ ] Confirm: `cargo run` serves the Svelte app

**Done when:** `cargo run` shows the styled inbox shell in the browser (hardcoded/empty state is fine).

---

## M3 — Frontend tests

Goal: Frontend test infrastructure is in place and the scaffold components are covered. Establishes the pattern for all future component tests.

- [ ] Add `vitest` and `@testing-library/svelte` as dev-dependencies; configure Vitest in `vite.config.ts` with jsdom environment
- [ ] Add `npm test` script to `frontend/package.json`
- [ ] Test: sidebar renders the Inbox and Archived nav items
- [ ] Test: `reasonLabel(reason)` utility maps `review_requested`, `mention`, `assign` to the correct label strings
- [ ] Confirm: `npm test` passes with no warnings

**Done when:** `npm test` passes with at least the component and utility tests green.

---

## M4 — GitHub API client + first API endpoint

Goal: The backend acquires a token via `gh auth token`, calls the GitHub REST API directly, and exposes the inbox over HTTP. Logic is unit-tested in isolation.

- [ ] At startup, run `gh auth token` (one subprocess) and store the token in an `Arc<str>` shared across handlers
- [ ] `src/models/inbox.rs`: typed `InboxItem` struct (id, title, repository, reason, updated_at, url)
- [ ] `src/github/mod.rs`: `parse_inbox(json: &str) -> Result<Vec<InboxItem>>` (pure) + `fetch_inbox(token, client)` that calls `GET https://api.github.com/notifications`
- [ ] `src/api/inbox.rs`: thin handler for `GET /api/inbox` — calls `fetch_inbox`, returns JSON
- [ ] Wire the route in `src/server.rs`; add typed error type; no `.unwrap()` in handlers
- [ ] Unit tests for `parse_inbox`: valid input, missing fields, empty array, malformed JSON
- [ ] Unit tests for error variants: confirm each maps to the expected HTTP status code
- [ ] Integration test for `GET /api/inbox`: mock the GitHub API response, assert correct JSON shape and 200
- [ ] Confirm: `curl localhost:<port>/api/inbox` returns real data and `cargo test` passes

**Done when:** The endpoint returns real data from the GitHub API and all new tests pass.

---

## M5 — Inbox UI with real data

Goal: The frontend fetches and renders real notifications from the API.

- [ ] Frontend: `GET /api/notifications` on load, store results in Svelte 5 state (`$state`)
- [ ] Render each notification as a PR row: repo, author, title/PR number, reason label, updated_at
- [ ] Reason label maps `review_requested` / `mention` / `assign` to the pill styles from the mockup
- [ ] Show unread dot for unread notifications (use `unread` field from gh response)
- [ ] Empty state when inbox is empty
- [ ] Confirm: running the app shows real PRs from your GitHub notifications

**Done when:** Real notifications appear in the inbox UI.