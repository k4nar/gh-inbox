# gh-inbox Architecture

## Overview

`gh-inbox` is a `gh` CLI extension. Running `gh inbox` starts a local Rust HTTP server, opens the browser to `localhost`, and serves a Svelte SPA that communicates with the server over a JSON REST API. The backend calls the GitHub REST API directly using a token obtained at startup.

```
gh inbox
    │
    ▼
Rust binary (HTTP server)
    ├── serves        → Svelte SPA (compiled static assets)
    ├── exposes       → JSON REST API  (/api/*)
    ├── reads/writes  → SQLite (local state)
    └── calls         → GitHub REST API (token from `gh auth token`)
```

## Distribution

The extension is installed as a standard `gh` extension:

```
gh extension install <user>/gh-inbox
```

The binary is a single self-contained Rust executable. Compiled Svelte assets are embedded in the binary at build time (via `include_dir!` or equivalent), so no separate file installation is needed.

## Runtime flow

1. User runs `gh inbox`.
2. The binary binds to a random available localhost port and starts the HTTP server.
3. The browser opens automatically to `http://localhost:<port>`.
4. The Svelte frontend loads and begins fetching data from `/api/*`.
5. The server calls the GitHub REST API directly (using the token obtained at startup), persists local state to SQLite, and returns JSON responses.
6. The server shuts down when the user sends SIGINT (Ctrl-C).

## Backend (Rust)

**Responsibilities:**
- HTTP server (serve static assets + JSON API)
- Obtain a GitHub token once at startup and call the GitHub REST API directly
- Read/write local state to SQLite
- Open the browser on startup

**Key boundaries:**
- No `gh` subprocess is spawned per request — token is acquired once at startup.
- Business logic lives in Rust modules, not in route handlers. Handlers are thin adapters.
- All errors are typed and propagated — no `.unwrap()` in handlers.
- The token is held in memory only — never written to disk, never sent to the frontend.

**Suggested crates:**
- `axum` — HTTP server and routing
- `tokio` — async runtime
- `reqwest` — async HTTP client for GitHub REST API calls
- `sqlx` — async SQLite access with compile-time query checking
- `serde` / `serde_json` — serialization
- `open` — cross-platform browser launch
- `include_dir` — embed frontend assets in binary

## Frontend (Svelte 5)

**Responsibilities:**
- Render the inbox UI
- Fetch data from the local REST API (`/api/*`)
- Manage UI-only state (selected PR, filters, panel open/closed)

**Stack:**
- Svelte 5 (runes-based reactivity)
- Bits UI — headless accessible components
- Pure CSS — no Tailwind; follows GitHub's design system with room for theming
- Vite — build tooling; output is embedded into the Rust binary at build time

**Key boundaries:**
- The frontend holds no auth tokens and makes no GitHub API calls.
- All data access goes through `/api/*` on localhost.

## Data layer

### GitHub data — REST + GraphQL

At startup, the binary acquires a GitHub token and uses it for all subsequent GitHub API calls via `reqwest`.

Key endpoints used:
- `GET /notifications?all=true` (REST, paginated) — notification feed for the sync loop
- `PATCH /notifications/threads/:id` — push local "read" to GitHub
- `DELETE /notifications/threads/:id` — push local "archive" to GitHub (marks the thread done)
- `POST /graphql` — full PR detail in one query (metadata, comments, review threads, commits, check runs + commit statuses, reviews, requested reviewer teams), replacing 6+ REST calls per PR
- `GET /user/teams` — the user's teams, for codeowner-team filtering

Responses are parsed from JSON into typed Rust structs via `serde_json`. The token is kept in memory only and never exposed to the frontend.

### GitHub notification semantics (hard-won)

GitHub distinguishes **read** from **done**, but the public API only exposes half of it:

- `GET /notifications?all=true` returns every thread updated in roughly the last **7 weeks** — including threads the user marked **Done** on github.com. Done-state is invisible to the REST API, and the GraphQL field the official mobile apps use (`notificationThreads`) is not in the public schema. The only "done" signal available is a thread **aging out** of the feed window.
- Consequences for the sync design:
  - **Reconciliation is by absence**: a full sync archives local inbox rows the feed no longer returns (`archive_stale`, keyed on `synced_at`). This catches threads marked done elsewhere — but only once they leave the ~7-week window, or immediately for threads done *before* the app first saw them… which leads to:
  - **Cold-start policy**: a thread the DB has never tracked that arrives already *read* is inserted as **archived** (it was handled outside gh-inbox; the API can't say whether it was also marked done). Threads already tracked are never auto-archived by being read — read ≠ done.
  - **Revival**: new activity flips a thread unread on GitHub, and the sync upsert moves an archived row back to the inbox.
- Local state is protected from sync races by two columns on `notifications`:
  - `locally_unarchived` — set on unarchive; exempts the row from reconciliation (GitHub never returns a done thread again, so it would otherwise look stale forever). Cleared when GitHub returns the thread (new activity) or the user re-archives.
  - `local_write_epoch` — set on read/archive/unarchive; the sync upsert ignores its snapshot's unread/archived flags when the local write postdates the sync's start time.

### Local state — SQLite

SQLite stores state that has no equivalent in GitHub's API:

| Table | Purpose |
|---|---|
| `notifications` | Cached notification list with read/unread/archived status + sync-race guards (`locally_unarchived`, `local_write_epoch`, `synced_at`) |
| `pull_requests` | Cached PR metadata (title, author, CI status, `last_viewed_at`, teams, labels) |
| `comments` | Cached issue + review comments for thread grouping |
| `commits` | Cached PR commits (drives "new commits since last visit") |
| `check_runs` | Cached check runs and commit statuses for the PR's head commit |
| `reviews` | Cached PR reviews (approved / changes requested / dismissed) |
| `user_teams` | The user's team slugs, for codeowner-team filtering |
| `user_preferences` | Key/value UI preferences (theme) |
| `last_fetched_at` | Timestamp per resource: sync cursor and PR fetch throttle |

PR numbers are only unique per repository, so `pull_requests` is keyed by the
composite `(repo, id)` and every child table carries `repo` alongside `pr_id`.
Child rows are wiped and re-inserted from each GitHub snapshot inside one
transaction, so the cache always mirrors the latest fetch.

The database file lives in the OS user data directory (e.g., `~/.local/share/gh-inbox/db.sqlite` on Linux, `~/Library/Application Support/gh-inbox/db.sqlite` on macOS).

There is no remote database and no sync — state is local only.

## API surface

### Transport model

- **REST + JSON** for all client-initiated operations (queries and mutations). JSON on localhost has negligible overhead.
- **Server-Sent Events (SSE)** for server-push. The client never needs to stream data to the server, so WebSockets are overkill. SSE is unidirectional (server → client), maps directly to the `EventSource` browser API, and is trivial to implement in `axum`.

```
Frontend                           Rust server
   │── GET /api/inbox ────────────▶│  initial load
   │── POST /api/.../archive ─────▶│  mutations
   │◀── GET /api/events (SSE) ─────│  new notifications, PR info updates, sync status
```

All `/api/*` routes except `/api/events` require an `x-session-token` header;
the token is generated per run and injected into `index.html`, so other local
processes and browser tabs cannot call the API. `/api/events` is exempt because
`EventSource` cannot set headers.

### REST endpoints

| Method | Path | Description |
|---|---|---|
| `GET` | `/api/inbox` | Paginated notification list. Query params: `?status=inbox\|archived`, `?page=`, `?per_page=`, `?repo=`, `?team=`, `?author=`, `?state_include=`, `?state_exclude=` |
| `GET` | `/api/inbox/options` | Sidebar filter options with counts (repos, teams, authors) |
| `POST` | `/api/inbox/prefetch` | Declare the visible viewport rows; spawns background PR fetches whose results arrive via SSE |
| `POST` | `/api/inbox/:id/read` | Mark a notification read (local + pushed to GitHub) |
| `POST` | `/api/inbox/:id/archive` | Archive a notification (local + marks the thread done on GitHub) |
| `POST` | `/api/inbox/:id/unarchive` | Move a notification back to the inbox (local only — GitHub has no "un-done") |
| `GET` | `/api/pull-requests/:owner/:repo/:number` | PR detail: metadata, threads, commits, check runs, reviews, labels; advances `last_viewed_at` |
| `POST` | `/api/sync` | Trigger an immediate full sync (202; progress arrives via SSE) |
| `GET` | `/api/preferences` | Read UI preferences (theme) |
| `PATCH` | `/api/preferences` | Update UI preferences |

### SSE endpoint

| Path | Description |
|---|---|
| `GET /api/events` | Persistent stream of server-push events |

Event types pushed by the server:
- `notifications:new` — the inbox changed after a sync; the client refetches the list. Also sent to a client that lagged behind the event stream, as a refetch hint.
- `pr:info_updated` — fresh PR data (author, status, CI, activity counts, teams) for one row, produced by prefetch, viewport auto-fetch, or opening the PR
- `sync:status` — background sync started / completed / errored
- `github:sync_error` — a fire-and-forget GitHub write (read/done) failed

This surface will grow as features are added. Keep handlers thin — business logic belongs in Rust modules.

### Data model

All the info fetched from Github should be cached in the SQLite DB.

## Project structure (target)

```
gh-inbox/
├── src/                   # Rust source
│   ├── main.rs            # Entry point: start server, open browser
│   ├── server.rs          # Axum router setup
│   ├── api/               # Route handlers (thin adapters)
│   ├── github/            # GitHub REST API client
│   ├── db/                # SQLite schema, queries, migrations
│   └── models/            # Shared types
├── frontend/              # Svelte 5 app
│   ├── src/
│   │   ├── lib/           # Shared components and stores
│   │   └── routes/        # Page-level components
│   └── vite.config.ts
├── Cargo.toml
└── package.json           # Frontend build only
```

## Non-negotiables

- All GitHub data comes from the GitHub REST API via `reqwest`, never from the frontend.
- No `.unwrap()` in API handlers — use typed errors with proper HTTP status codes.
- Frontend never holds auth tokens.
- Keep handlers thin. Business logic lives in `github/` and `db/` modules.
