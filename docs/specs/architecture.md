# gh-inbox Architecture

## Overview

`gh-inbox` is a `gh` CLI extension. Running `gh inbox` starts a local Rust HTTP server, opens the browser to `localhost`, and serves a Svelte SPA that communicates with the server over a JSON REST API. The backend calls the GitHub REST API directly using a token obtained at startup — no `gh` process is spawned per request.

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
6. When the browser tab is closed or the user sends SIGINT, the server shuts down.

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

### GitHub data — REST API

At startup, the binary acquires a GitHub token and uses it for all subsequent calls to the GitHub REST API via `reqwest`.

Key endpoints used:
- `GET /notifications` — inbox notifications
- `GET /repos/:owner/:repo/pulls/:number` — PR metadata
- `GET /repos/:owner/:repo/pulls/:number/reviews` — reviews
- `GET /repos/:owner/:repo/pulls/:number/comments` — inline comments
- `GET /repos/:owner/:repo/commits/:ref/check-runs` — CI status

Responses are parsed from JSON into typed Rust structs via `serde_json`. The token is kept in memory only and never exposed to the frontend.

### Local state — SQLite

SQLite stores state that has no equivalent in GitHub's API:

| Table | Purpose |
|---|---|
| `notifications` | Cached notification list with read/unread/archived status |
| `pull_requests` | Cached PR metadata (title, author, CI status, etc.) |
| `last_fetched_at` | Timestamp per resource for cache invalidation |

The database file lives in the OS user data directory (e.g., `~/.local/share/gh-inbox/db.sqlite` on Linux, `~/Library/Application Support/gh-inbox/db.sqlite` on macOS).

There is no remote database and no sync — state is local only.

## API surface

### Transport model

- **REST + JSON** for all client-initiated operations (queries and mutations). JSON on localhost has negligible overhead.
- **Server-Sent Events (SSE)** for server-push. The client never needs to stream data to the server, so WebSockets are overkill. SSE is unidirectional (server → client), maps directly to the `EventSource` browser API, and is trivial to implement in `axum`.

```
Frontend                           Rust server
   │── GET /api/notifications ────▶│  initial load
   │── POST /api/.../archive ─────▶│  mutations
   │◀── GET /api/events (SSE) ─────│  new notifications, CI updates, sync status
```

### REST endpoints

| Method | Path | Description |
|---|---|---|
| `GET` | `/api/notifications` | List inbox notifications (unread + unarchived) |
| `POST` | `/api/notifications/:id/read` | Mark a notification as read |
| `POST` | `/api/notifications/:id/archive` | Archive a notification |
| `GET` | `/api/pull-requests/:id` | PR detail: metadata, comments, commits, CI |
| `GET` | `/api/pull-requests/:id/threads` | Review threads grouped by conversation |

### SSE endpoint

| Path | Description |
|---|---|
| `GET /api/events` | Persistent stream of server-push events |

Event types pushed by the server:
- `notifications:updated` — new or changed notifications after a background sync
- `pr:ci_updated` — CI status changed for a tracked PR
- `sync:status` — background sync started / completed / errored

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

- `gh` is used only once at startup to retrieve the auth token — never per request.
- All GitHub data comes from the GitHub REST API via `reqwest`, never from the frontend.
- No `.unwrap()` in API handlers — use typed errors with proper HTTP status codes.
- Frontend never holds auth tokens.
- Keep handlers thin. Business logic lives in `github/` and `db/` modules.
- SQLite is the only persistence mechanism — no external services.
