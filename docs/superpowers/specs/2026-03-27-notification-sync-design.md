# Notification Sync Improvements — Design

Date: 2026-03-27

## Problem

The current sync implementation has three issues:

1. **Always fetches the first page only.** Every 60s sync re-fetches the same page of notifications from GitHub with no `since` filter, pulling data that hasn't changed.
2. **Never catches up after a gap.** If the service is stopped for hours and restarted, the sync still fetches only the first page, missing notifications that were marked read or archived directly on GitHub.
3. **The frontend triggers the first sync.** `GET /api/inbox` calls `sync_notifications` on the first request if the DB is empty. The backend loop should own the full sync lifecycle.

## Solution: Two-mode sync (Option A)

`sync_notifications` decides its mode based on `last_fetched_at`:

| Condition | Mode | Behavior |
|---|---|---|
| `last_fetched_at` is None | Full | Fetch all pages, reconcile |
| `now − last_fetched_at > 2h` | Full | Fetch all pages, reconcile |
| Otherwise | Incremental | Fetch only what changed since last fetch |

The 2h threshold is hardcoded. The sync interval changes from 60s to 30s.

---

## GitHub fetch layer (`src/github/notifications.rs`)

### `fetch_notifications_page(github, url) -> Result<(Vec<Notification>, Option<String>)>`

Low-level primitive. Fetches one page from the given URL and returns:
- The deserialized notifications
- The next-page URL, parsed from the `Link: <url>; rel="next"` response header (None if no next page)

### `fetch_all_notifications(github) -> Result<Vec<Notification>>`

Full-sync fetch. Starts at `/notifications?all=true&per_page=50`, follows `Link` next-page headers until exhausted. Returns the complete list.

### `fetch_notifications_since(github, since_iso: &str) -> Result<Vec<Notification>>`

Incremental fetch. Calls `/notifications?all=true&since=<since_iso>&per_page=50`, follows pagination. The `since` value is the previous `last_fetched_at` converted from epoch seconds to ISO 8601.

---

## Sync logic (`src/github/sync.rs`)

### Mode decision

```
last_fetched_epoch = get_last_fetched_epoch(pool, "notifications")
now_epoch = unix_now()

mode = if last_fetched_epoch.is_none() || (now_epoch - last_fetched_epoch) > 7200 {
    Full
} else {
    Incremental { since_iso: epoch_to_iso(last_fetched_epoch) }
}
```

### Full sync

1. Call `fetch_all_notifications`.
2. Upsert each notification as before (existing logic).
3. Collect the set of returned IDs.
4. If `ids` is non-empty, call `archive_if_not_in(pool, &ids)` — auto-archives local notifications absent from GitHub's response. If `ids` is empty (GitHub returned nothing), skip reconciliation to avoid nuking the entire inbox on an empty response.
5. Update `last_fetched_at`.
6. Return changed count (upserts + archived).

### Incremental sync

1. Call `fetch_notifications_since(since_iso)`.
2. Upsert each notification.
3. Update `last_fetched_at`.
4. Return changed count.

### `run_sync_loop` changes

- Default interval: **30s** (was 60s; env var `GH_INBOX_SYNC_INTERVAL` still overrides)
- No other changes — the loop already runs immediately at startup before the first sleep.

---

## DB layer (`src/db/queries/notifications.rs`)

### New: `archive_if_not_in(pool, ids: &[&str]) -> sqlx::Result<u64>`

Archives all non-archived notifications whose `id` is not in `ids`:

```sql
UPDATE notifications
SET archived = 1
WHERE archived = 0
  AND id NOT IN (/* bound list */)
```

Returns rows affected. Used by full sync to auto-archive notifications that disappeared from GitHub.

No schema migration needed.

---

## Bootstrap removal (`src/api/inbox/get.rs` + `src/server.rs`)

The bootstrap block in `get_inbox` is deleted:

```rust
// REMOVED:
if state.bootstrap_done.compare_exchange(...).is_ok() {
    if !has_fetched { sync_notifications(&state).await?; }
}
```

`bootstrap_done: Arc<AtomicBool>` is removed from `AppState`. The sync loop owns the first sync — it runs before the first sleep, so data is available within one sync cycle of startup.

`GET /api/inbox` returns an empty list if called before the first sync completes. The frontend already handles this (empty state renders, then fills when the SSE `notifications:new` event fires).

---

## Testing

### `src/github/notifications.rs`
- `fetch_all_notifications_follows_link_header` — mock returns two pages; assert both are fetched
- `fetch_notifications_since_sends_since_param` — assert `since=` appears in the request URL

### `src/db/queries/notifications.rs`
- `archive_if_not_in_archives_missing` — insert three notifications, call with two IDs, assert third is archived
- `archive_if_not_in_leaves_present_unchanged` — assert notifications in the list stay unarchived

### `src/github/sync.rs`
- `full_sync_when_never_fetched` — no `last_fetched_at`; assert all-pages URL used
- `full_sync_when_last_fetch_over_2h` — `last_fetched_at` = now − 3h; assert all-pages URL
- `incremental_sync_when_recent` — `last_fetched_at` = now − 1min; assert `since=` param in URL
- `full_sync_auto_archives_missing` — GitHub returns 1 of 2 known notifications; assert the other is archived
- `incremental_sync_passes_correct_since` — assert ISO 8601 value matches stored epoch

### `tests/routes.rs` (integration)
- `get_inbox_returns_empty_before_first_sync` — assert `GET /api/inbox` returns `{"items":[],...}` without triggering a GitHub call
