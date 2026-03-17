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

---

## M6 — PR Detail View

Goal: Clicking a PR in the inbox opens a detail panel showing metadata, CI status, and comments grouped by thread. New comments since last view are highlighted.

- [x] Migration 005: add columns to `pull_requests` — `body TEXT DEFAULT ''`, `state TEXT DEFAULT 'open'`, `head_sha TEXT DEFAULT ''`, `additions INTEGER DEFAULT 0`, `deletions INTEGER DEFAULT 0`, `changed_files INTEGER DEFAULT 0`
- [x] Migration 006: add columns to `comments` — `comment_type TEXT DEFAULT 'issue_comment'`, `path TEXT`, `position INTEGER`, `in_reply_to_id INTEGER`
- [x] `src/models/pull_request.rs`: typed structs for GitHub API responses — `GithubPullRequest`, `GithubIssueComment`, `GithubReviewComment`, `GithubCheckRun`, `GithubCheckRunList`
- [x] `src/github/mod.rs`: add `fetch_pull_request()`, `fetch_issue_comments()`, `fetch_review_comments()`, `fetch_check_runs()` — each calls the corresponding GitHub REST endpoint
- [x] Unit tests for each parse function (valid input, empty array, missing optional fields)
- [x] `src/db/queries.rs`: update `PullRequestRow` with new fields; update `upsert_pull_request()`; add `get_pull_request(pool, repo, number)`
- [x] `src/db/queries.rs`: add `CommentRow` struct with all fields; add `upsert_comment()`, `query_comments_for_pr(pool, pr_id)`
- [x] `src/db/queries.rs`: add `update_last_viewed_at(pool, pr_id)` — sets timestamp when user opens a PR
- [x] Unit tests for comment queries (insert + query round-trip, thread grouping)
- [x] `src/api/pull_requests.rs`: handler for `GET /api/pull-requests/:owner/:repo/:number` — fetches from GitHub (with 30s throttle), caches in SQLite, updates `last_viewed_at`, returns JSON (PR metadata + comments + check runs)
- [x] `src/api/pull_requests.rs`: handler for `GET /api/pull-requests/:owner/:repo/:number/threads` — queries comments from SQLite, groups by `thread_id`, returns JSON array of thread objects
- [x] Wire routes in `src/server.rs`
- [x] Integration tests: mock GitHub endpoints, assert correct JSON shape and threading
- [x] `frontend/src/lib/PrDetail.svelte`: detail panel — PR title, author, body, CI status badges, threaded comments
- [x] `frontend/src/lib/CommentThread.svelte`: renders a thread (file path header for inline comments, comment bodies with author/date)
- [x] `frontend/src/App.svelte`: add `selectedPr` state; clicking a row shows `PrDetail` in a right panel
- [x] Highlight "new" comments where `comment.created_at > pr.last_viewed_at`
- [x] Frontend tests for PrDetail and CommentThread rendering
- [x] Confirm: clicking a PR in the inbox shows the detail panel with real data; `cargo test` and `npm test` pass

**Done when:** Clicking a PR in the inbox opens a detail panel showing metadata, CI status, and comments grouped by thread. New comments are highlighted. `cargo test` and `npm test` pass.

---

## M7 — Inbox Actions

Goal: Users can mark notifications as read, archive PRs, view archived PRs, and unarchive. The inbox becomes a workflow tool with inbox-zero flow.

- [x] `src/api/inbox.rs`: add `post_mark_read` handler for `POST /api/inbox/:id/read` — calls existing `queries::mark_read`, returns 204
- [x] `src/api/inbox.rs`: add `post_archive` handler for `POST /api/inbox/:id/archive` — calls existing `queries::archive_notification`, returns 204
- [x] `src/api/inbox.rs`: add `post_unarchive` handler for `POST /api/inbox/:id/unarchive` — calls existing `queries::unarchive_notification`, returns 204
- [x] `src/api/inbox.rs`: add `?status=archived` query param to `get_inbox` — calls `query_archived` when set
- [x] `src/api/error.rs`: add `NotFound` variant (404) for missing notification IDs
- [x] Wire routes in `src/server.rs`
- [x] Integration tests: archive → verify gone from inbox, visible in archived; unarchive → back in inbox; mark read; 404 for nonexistent ID
- [x] Frontend: shared state for `currentView` (inbox/archived) and `notifications` so Sidebar and PrList can share state
- [x] `Sidebar.svelte`: clicking Inbox/Archived sets `currentView`; active style follows it
- [x] `PrList.svelte`: refetch from `/api/inbox?status={currentView}` when view changes; archive/unarchive buttons on rows; clicking a PR marks it as read (optimistic UI)
- [x] Empty state differs by view: "All caught up!" vs "No archived notifications."
- [x] Frontend tests: archive removes from list, view switching works, mark-read removes unread dot
- [x] Confirm: full archive/read/unarchive flow works end-to-end; `cargo test` and `npm test` pass

**Done when:** Full archive/read/unarchive flow works end-to-end with optimistic UI. `cargo test` and `npm test` pass.

---

## M8 — SSE + Background Sync

Goal: The server syncs notifications in the background and pushes updates to the frontend via SSE. The inbox updates in real-time without page refresh.

- [x] Add `tokio::sync::broadcast` channel to `AppState`
- [x] `src/models/sync_event.rs`: `SyncEvent` enum — `NewNotifications { ids: Vec<String> }`, `SyncStatus { status: String }`
- [x] `src/github/sync.rs`: `run_sync_loop(state, tx)` — async loop (60s default, configurable via env var), fetches notifications, upserts, sends events on changes
- [x] `src/api/events.rs`: handler for `GET /api/events` — returns `Sse<impl Stream>` subscribing to broadcast receiver
- [x] Wire route and spawn sync loop in `main.rs`
- [x] Integration tests: SSE events received after sync; no event when nothing changed
- [x] `frontend/src/lib/sse.js`: `EventSource` utility with `onNewNotifications` / `onSyncStatus` callbacks
- [x] Frontend: refetch list on `notifications:new` event; show sync status in Topbar (spinning/idle/error)
- [x] Frontend tests: SSE event parsing, list refresh on new notifications
- [x] Confirm: new GitHub notifications appear without page refresh; `cargo test` and `npm test` pass

**Done when:** New GitHub notifications appear in the inbox without page refresh. Sync status indicator works. All tests pass.

---

## M9 — PR Detail View Improvements

Goal: The PR detail panel gains a direct link to GitHub, shows commits with "new" highlighting, and surfaces failed/pending CI checks first while collapsing passing ones.

- [x] `src/models/pull_request.rs`: add `GithubCommit`, `GithubCommitDetail`, `GithubCommitAuthor` structs
- [x] `src/github/mod.rs`: add `fetch_commits()` and `parse_commits()` + unit tests
- [x] `migrations/007_create_commits.sql`: create `commits` table (sha, pr_id, message, author, committed_at)
- [x] `src/db/queries.rs`: add `CommitRow`, `upsert_commit()`, `query_commits_for_pr()` + unit tests
- [x] `src/api/pull_requests.rs`: fetch and cache commits in `get_pr()`, add `commits` to `PrDetailResponse`
- [x] Integration test: assert commits appear in PR detail response
- [x] `PrDetail.svelte`: add GitHub link icon in detail header (links to `detail.pull_request.url`, `target="_blank"`)
- [x] `PrDetail.svelte`: add commits section — SHA, message, author, date; "new" badge for commits after `last_viewed_at`
- [x] `PrDetail.svelte`: group CI checks — failed/pending shown first, passing behind collapsible toggle, "All checks passed" summary
- [x] Frontend tests: GitHub link, commit rendering with new-commit highlighting, CI check grouping
- [x] Confirm: `cargo test` and `npm test` pass

**Done when:** PR detail shows a GitHub link, commits with "new" highlighting, and CI checks grouped by status. All tests pass.

---

## M10 — PR List Enrichment

Goal: Each PR row in the inbox shows the author avatar, PR status (open/draft/merged/closed), activity since last visit (new commits and comments), and the user's teams that are requested reviewers.

- [x] Migration 009: add `draft`, `merged_at`, `teams` columns to `pull_requests`
- [x] Migration 010: create `user_teams` table
- [x] `src/models/pull_request.rs`: add `draft`, `merged_at` to `GithubPullRequest`
- [x] `src/models/sync_event.rs`: add `PrTeamsUpdated` variant and `PrTeamsUpdatedData` struct
- [x] `src/db/queries/pull_requests.rs`: add `InboxItem`, `query_inbox_enriched`, `query_archived_enriched`, `set_teams_fetching`, `update_teams`; update `upsert_pull_request` to include `draft`/`merged_at`, exclude `teams`
- [x] `src/db/queries/user_teams.rs`: `get_all_user_teams`, `replace_user_teams`
- [x] `src/github/teams.rs`: `fetch_user_teams`, `fetch_requested_reviewer_teams`
- [x] `src/api/events.rs`: add `pr:teams_updated` match arm
- [x] `src/api/inbox/get.rs`: return `Vec<InboxItem>`, spawn async team fetch with concurrency guard
- [x] `frontend/src/lib/types.ts`: add `InboxItem` type
- [x] `frontend/src/lib/sse.svelte.ts`: add `onPrTeamsUpdated` callback and listener
- [x] `frontend/src/lib/PrList.svelte`: redesign row layout
- [x] All backend and frontend tests pass

**Done when:** The inbox list shows avatar, status badge, activity sentence, and team badges for each PR. Team badges update in real-time via SSE. `cargo test` and `npm test` pass.

---

## M11 — PR Detail Redesign

Goal: Compact PR detail panel with status bar, since-last-visit timeline, collapsed threads with previews, and clickable comments linking to GitHub.

- [x] Migration 012: add `html_url TEXT` column to `comments` table
- [x] `src/models/pull_request.rs`: add `html_url` to `GithubIssueComment` and `GithubReviewComment`
- [x] `src/db/queries/comments.rs`: add `html_url: Option<String>` to `CommentRow`, update upsert/query
- [x] `src/api/pull_requests/fetch.rs`: pass `html_url` when constructing `CommentRow`
- [x] `src/api/pull_requests/get.rs`: add `previous_viewed_at` to `PrDetailResponse`; read old `last_viewed_at` before updating
- [x] `frontend/src/lib/types.ts`: add `html_url` to `Comment`, `draft`/`merged_at` to `PullRequest`, `previous_viewed_at` to `PrDetailResponse`
- [x] `frontend/src/lib/CommentThread.svelte`: rewrite — collapsed preview with avatars, expand on click, clickable comment links
- [x] `frontend/src/lib/PrDetail.svelte`: rewrite — status bar (state pill, author avatar, diff stats, CI tooltip), since-last-visit timeline

**Done when:** PR detail panel shows compact status bar, timeline divided by "Since your last visit" / "Earlier", collapsed threads with previews, and comments link to GitHub. `cargo test` and `npm test` pass.