# PR List Enrichment — Design Spec

**Date:** 2026-03-14
**Milestone:** M10

## Goal

Enrich the PR inbox list so users can assess each PR at a glance without opening it:
- Author avatar
- PR status (open / draft / merged / closed)
- Activity summary since last visit (new commits, new comments grouped by author)
- Teams the user belongs to that are requested reviewers on the PR

---

## Row Layout

Option C was selected: large avatar anchors the left, activity as a prose sentence, teams and status as inline badges.

### States

| Condition | Activity line |
|---|---|
| `last_viewed_at IS NULL` | `✦ New pull request` (white, `#e6edf3`, font-weight 500) |
| `last_viewed_at` set, no new activity | `No new activity since your last visit` (muted italic, `#6e7681`) |
| New commits and/or comments | e.g. `alice pushed 2 commits · alice and bob left 3 comments` (actor names in white) |

### Status badges

| State | Condition | Display |
|---|---|---|
| Merged | `merged_at IS NOT NULL` | `⎇ Merged` (purple) |
| Closed | `state = 'closed' AND merged_at IS NULL` | `✕ Closed` (red) |
| Draft | `state = 'open' AND draft = 1` | `◌ Draft` (grey) |
| Open | `state = 'open' AND draft = 0` | `● Open` (green) |

Priority order: merged → closed → draft → open. A PR with `state = 'closed'` can never be draft.

### Teams

- Shown as `@acme/platform` badges (amber) in the top meta line
- While `teams IS NULL` in the DB: shimmer placeholder badge shown
- If fetched and none matched (`teams = '[]'`): no badge shown
- Multiple matched teams: multiple badges shown

### Avatars

Loaded directly from `https://github.com/{login}.png?size=64` (public CDN, not GitHub API). Browser caches them. Fallback: if the image fails to load, show the first character of the `author` login as initials on a neutral background (`#21262d`). Implemented with an `onerror` handler on the `<img>` element that swaps it for an initials `<div>`. For rows with `author = null` (non-PR notifications), no avatar is shown — the avatar slot is left empty.

---

## Migration Numbering

`008_create_check_runs.sql` already exists. New migrations are numbered 009 and 010.

---

## Data Model Changes

### Migration 009 — extend `pull_requests`

```sql
ALTER TABLE pull_requests ADD COLUMN draft     BOOLEAN NOT NULL DEFAULT 0;
ALTER TABLE pull_requests ADD COLUMN merged_at TEXT;
ALTER TABLE pull_requests ADD COLUMN teams     TEXT;
-- NULL   = team fetch not yet attempted
-- '[]'   = fetched, no user-owned teams are requested reviewers
-- '[...]' = JSON array of matched team slugs e.g. '["acme/platform"]'
```

`teams` must not be overwritten during regular PR upsert (re-sync should preserve async-fetched team data). `draft` and `merged_at` are updated on each lazy detail fetch and must be included in `DO UPDATE SET`. The updated upsert clause:

```sql
ON CONFLICT(id) DO UPDATE SET
    title         = excluded.title,
    repo          = excluded.repo,
    author        = excluded.author,
    url           = excluded.url,
    ci_status     = excluded.ci_status,
    body          = excluded.body,
    state         = excluded.state,
    head_sha      = excluded.head_sha,
    additions     = excluded.additions,
    deletions     = excluded.deletions,
    changed_files = excluded.changed_files,
    draft         = excluded.draft,
    merged_at     = excluded.merged_at
    -- teams is intentionally excluded: preserved across re-syncs
```

### Migration 010 — create `user_teams`

```sql
CREATE TABLE user_teams (
    slug TEXT PRIMARY KEY  -- full slug e.g. "acme/platform"
);
```

Populated once, then refreshed using the existing `last_fetched_at` table (key: `'user_teams'`, TTL: 24 hours). On each team fetch attempt, check `last_fetched_at` for `'user_teams'` — if older than 24h or missing, re-fetch and replace all rows.

---

## API Response

`GET /api/inbox` (both `?status=inbox` and `?status=archived`) returns `Vec<InboxItem>` instead of `Vec<NotificationRow>`.

```rust
pub struct InboxItem {
    // from notifications
    pub id: String,
    pub pr_id: Option<i64>,
    pub title: String,
    pub repository: String,
    pub reason: String,
    pub unread: bool,
    pub archived: bool,
    pub updated_at: String,
    // joined from pull_requests (None when notification has no linked PR row)
    pub author: Option<String>,
    pub pr_status: Option<String>,          // "open" | "draft" | "merged" | "closed" | null
    // activity since last_viewed_at (None = first visit, i.e. last_viewed_at IS NULL)
    pub new_commits: Option<i64>,
    pub new_comments: Option<Vec<CommentAuthorCount>>,
    // teams (None = fetch not yet attempted; Some([]) = fetched, none matched)
    pub teams: Option<Vec<String>>,
}

pub struct CommentAuthorCount {
    pub author: String,
    pub count: i64,
}
```

`new_commits = None` and `new_comments = None` both signal "first visit" — the frontend shows `✦ New pull request`. `Some(0)` / `Some([])` means "visited before, nothing new."

Non-PR notifications (`pr_id IS NULL`) will have `author`, `pr_status`, `new_commits`, `new_comments`, and `teams` all as `None`. The frontend renders them without avatar, status badge, activity line, or team badges.

Note: `pull_requests.id` is the GitHub PR number (not an auto-increment key). `pr_id` in `InboxItem` is therefore the display number used to render `#1234` in the title line. This is confirmed by `002_create_pull_requests.sql` (`id INTEGER PRIMARY KEY`) and the existing upsert which binds `GithubPullRequest.number`.

### SQL query (inbox)

```sql
SELECT
    n.id, n.pr_id, n.title, n.repository, n.reason,
    n.unread, n.archived, n.updated_at,
    pr.author,
    CASE
        WHEN pr.merged_at IS NOT NULL          THEN 'merged'
        WHEN pr.state = 'closed'               THEN 'closed'
        WHEN pr.draft = 1                      THEN 'draft'
        WHEN pr.id IS NOT NULL                 THEN 'open'
        ELSE NULL
    END as pr_status,
    CASE WHEN pr.last_viewed_at IS NULL THEN NULL
         ELSE (SELECT COUNT(*) FROM commits c
               WHERE c.pr_id = pr.id AND c.committed_at > pr.last_viewed_at)
    END as new_commits,
    CASE WHEN pr.last_viewed_at IS NULL THEN NULL
         ELSE COALESCE((
             SELECT json_group_array(json_object('author', author, 'count', cnt))
             FROM (SELECT author, COUNT(*) as cnt FROM comments
                   WHERE pr_id = pr.id AND created_at > pr.last_viewed_at
                   GROUP BY author
                   ORDER BY cnt DESC, author ASC)
         ), '[]')
    END as new_comments_json,
    pr.teams as teams_json
FROM notifications n
LEFT JOIN pull_requests pr ON pr.id = n.pr_id AND pr.repo = n.repository
WHERE n.archived = 0
ORDER BY n.updated_at DESC
```

`new_comments_json` and `teams_json` come back from sqlx as `Option<String>` and are deserialized in a post-processing step before building `InboxItem`. All datetime columns (`committed_at`, `last_viewed_at`, `created_at`) are stored as UTC ISO 8601 strings in `YYYY-MM-DDTHH:MM:SSZ` format (e.g. `"2025-01-01T00:00:00Z"`), confirmed by existing test fixtures in `commits.rs`. String comparison is therefore correct and consistent.

**Deserialization mapping:**

```rust
// new_comments_json: NULL means first visit (CASE WHEN returns NULL)
// '[]' means visited + no new comments
// '[{"author":"alice","count":2}]' means activity
let new_comments: Option<Vec<CommentAuthorCount>> = match row.new_comments_json.as_deref() {
    None => None,
    Some(json) => Some(serde_json::from_str(json)?),
};

// teams_json: NULL and 'fetching' both mean loading (shimmer)
// '[]' means fetched + no matching teams
// '[\"acme/platform\"]' means matched teams
let teams: Option<Vec<String>> = match row.teams_json.as_deref() {
    None | Some("fetching") => None,
    Some(json) => Some(serde_json::from_str(json)?),
};
```

The query is the same for `?status=archived` (replace `WHERE n.archived = 0` with `WHERE n.archived = 1`).

---

## PR Status Fields — Sync Strategy

`draft` and `merged_at` are fetched from the GitHub PR detail endpoint (`GET /repos/{owner}/{repo}/pulls/{number}`). They are **not** fetched proactively in the background sync loop (to avoid hitting rate limits for large inboxes). They are populated lazily when the user opens a PR detail — the existing `get_pr` handler in `src/api/pull_requests/get.rs` already calls `fetch_pull_request` and upserts the result. Migration 009 adds the new columns; the `upsert_pull_request` query is updated to include them.

This means `draft` and `merged_at` may remain at default values (`0` / `NULL`) until the PR is first opened. The PR list will show `● Open` for such rows — acceptable for a first visit.

---

## Async Team Fetching

### Concurrency guard

After the inbox query returns, the handler collects all `InboxItem`s where `teams == None` on the Rust side (which covers both `NULL` and `'fetching'` from the DB — both deserialize to `None`). For those, it issues a single atomic UPDATE before spawning any task:

```sql
UPDATE pull_requests SET teams = 'fetching'
WHERE id IN (?, ?, ...) AND teams IS NULL
```

The `AND teams IS NULL` clause is key: if a concurrent request already set `teams = 'fetching'` for a given PR, this UPDATE is a no-op for that row and `rows_affected` for that id will be 0. The handler only spawns a background task for PRs where the UPDATE actually changed a row (i.e. rows that transitioned from `NULL` to `'fetching'`). This prevents duplicate fetches under concurrent inbox loads.

The `InboxItem` serialization layer maps both `NULL` and `'fetching'` to `teams: None` (→ shimmer in the frontend).

### Background task logic

1. **Check `user_teams` freshness.** Query `last_fetched_at` for key `'user_teams'`. If missing or older than 24h:
   - Call `GET /user/teams` on GitHub
   - In a single transaction: delete all rows from `user_teams`, re-insert the new set, and update `last_fetched_at` for `'user_teams'`
   - This transaction also acts as a write lock: if two background tasks race on `user_teams` refresh, SQLite's serialized writes ensure one transaction commits fully before the other begins. The second transaction's DELETE+INSERT simply overwrites with the same data — idempotent and safe.
2. **For each PR with `teams = 'fetching'`** (the ones this task owns): call `GET /repos/{owner}/{repo}/pulls/{number}/requested_reviewers`.
3. Intersect the returned team slugs with `user_teams`.
4. Store the result: `UPDATE pull_requests SET teams = ? WHERE id = ?` with the JSON array (`'[]'` if no match).
5. Push SSE event for each updated PR.

### New GitHub module: `src/github/teams.rs`

```rust
pub async fn fetch_user_teams(client: &Client, token: &str) -> Result<Vec<String>>;
// calls GET /user/teams, returns full slugs: "{org}/{slug}"

pub async fn fetch_requested_reviewer_teams(
    client: &Client, token: &str,
    owner: &str, repo: &str, pr_number: i64,
) -> Result<Vec<String>>;
// calls GET /repos/{owner}/{repo}/pulls/{number}/requested_reviewers
// returns team slugs as "{org}/{slug}"
```

### New SSE event variant

```rust
// src/models/sync_event.rs
pub enum SyncEvent {
    // ... existing variants ...
    PrTeamsUpdated(PrTeamsUpdatedData),
}

#[derive(Debug, Clone, Serialize)]
pub struct PrTeamsUpdatedData {
    pub pr_id: i64,
    pub teams: Vec<String>,
}
```

Match arm in `src/api/events.rs` (consistent with existing pattern):
```rust
SyncEvent::PrTeamsUpdated(data) => Event::default()
    .event("pr:teams_updated")
    .json_data(data)
    .unwrap_or_default(),
```

---

## Frontend Changes

### `frontend/src/lib/types.ts`

Add `InboxItem` type. Keep existing `Notification` type if referenced elsewhere.

```typescript
export interface InboxItem {
    id: string;
    pr_id: number | null;
    title: string;
    repository: string;
    reason: string;
    unread: boolean;
    archived: boolean;
    updated_at: string;
    author: string | null;
    pr_status: "open" | "draft" | "merged" | "closed" | null;
    new_commits: number | null;       // null = first visit
    new_comments: { author: string; count: number }[] | null;  // null = first visit
    teams: string[] | null;           // null = loading (shimmer)
}
```

### `frontend/src/lib/PrList.svelte`

Redesign each row:
- **Unread dot** (8px, blue/transparent)
- **Avatar** (32px circle): `<img src="https://github.com/{author}.png?size=64" onerror="...">`. On error, swap to an initials `<div>` showing the first character of `author` (uppercased). If `author` is null, render an empty 32px placeholder to preserve alignment.
- **Top meta line**: `{repo}` · status badge · team badge(s) (or shimmer if `teams === null`)
- **Title line**: `#{pr_id}` + title
- **Activity line** (omitted if `author === null`):
  - `new_commits === null`: `✦ New pull request`
  - all zero activity: `No new activity since your last visit` (muted italic)
  - otherwise: prose sentence built from `new_commits` and `new_comments`
- **Right column**: reason pill · date · archive/unarchive button (on hover)

Activity sentence format:
```
{actors} pushed {n} commit(s) [· {actors} left {n} comment(s)]
```
Where actors = comma-separated author names joined with "and" for the last. Example: `alice pushed 2 commits · alice and bob left 3 comments`.

### `frontend/src/lib/sse.svelte.ts` (or equivalent SSE handler)

Handle `pr:teams_updated` event:
```typescript
source.addEventListener("pr:teams_updated", (e) => {
    const { pr_id, teams } = JSON.parse(e.data);
    const item = notifications.find(n => n.pr_id === pr_id);
    if (item) item.teams = teams;
    notifications = [...notifications];  // trigger reactivity
});
```

---

## New DB Queries Module: `src/db/queries/user_teams.rs`

```rust
pub async fn get_all_user_teams(pool: &SqlitePool) -> sqlx::Result<Vec<String>>;
pub async fn replace_user_teams(pool: &SqlitePool, slugs: &[String]) -> sqlx::Result<()>;
// DELETE all + INSERT new, wrapped in a transaction
```

`src/db/queries/mod.rs` must re-export from this module.

---

## Files Affected

| File | Change |
|---|---|
| `migrations/009_extend_pull_requests_status.sql` | New — add `draft`, `merged_at`, `teams` |
| `migrations/010_create_user_teams.sql` | New |
| `src/models/pull_request.rs` | Add `draft: bool`, `merged_at: Option<String>` to `GithubPullRequest` |
| `src/models/sync_event.rs` | Add `PrTeamsUpdated(PrTeamsUpdatedData)` variant + `PrTeamsUpdatedData` struct |
| `src/db/queries/pull_requests.rs` | Add `draft`, `merged_at`, `teams` to `PullRequestRow`; update upsert (exclude `teams` from DO UPDATE); add `query_inbox_enriched`, `query_archived_enriched`, `set_teams_fetching`, `update_teams` |
| `src/db/queries/user_teams.rs` | New — `get_all_user_teams`, `replace_user_teams` |
| `src/db/queries/mod.rs` | Re-export `user_teams` module |
| `src/github/teams.rs` | New — `fetch_user_teams`, `fetch_requested_reviewer_teams` |
| `src/github/mod.rs` | Expose teams module |
| `src/api/inbox/get.rs` | Return `Vec<InboxItem>`, set `teams = 'fetching'` + spawn team fetch task |
| `src/api/events.rs` | Add `PrTeamsUpdated` match arm |
| `frontend/src/lib/types.ts` | Add `InboxItem` type |
| `frontend/src/lib/PrList.svelte` | Redesign row layout |
| `frontend/src/lib/sse.svelte.ts` | Handle `pr:teams_updated` |

---

## Testing

**Backend:**
- Unit tests for `fetch_user_teams` and `fetch_requested_reviewer_teams` parsers (valid input, empty teams, missing fields)
- Unit tests for `query_inbox_enriched`: activity counts, pr_status derivation (all 4 states), NULL semantics for first-visit vs visited, teams NULL/`[]`/value
- Unit test: `upsert_pull_request` does not overwrite an existing non-null `teams` value
- Unit test: `replace_user_teams` is atomic (old rows replaced, not appended)
- Integration test: inbox response shape includes `author`, `pr_status`, `new_commits`, `new_comments`, `teams`
- Integration test: team fetch task writes correct value to `pull_requests.teams`, pushes SSE `pr:teams_updated`
- Integration test: concurrent inbox calls do not double-fetch teams (concurrency guard)

**Frontend:**
- Row renders avatar with initials fallback when `author` is non-null
- Row renders empty placeholder when `author` is null
- Status badge renders for all 4 states
- Activity line: `✦ New pull request` when `new_commits === null`
- Activity line: muted text when `new_commits === 0` and `new_comments === []`
- Activity line: prose sentence for non-zero activity
- Shimmer shown when `teams === null`, real badges shown after SSE event
- No team badges when `teams === []`
