# M12 — GitHub Notification State Sync

**Date:** 2026-03-17

## Goal

When a user marks a notification as read or archives it in gh-inbox, the corresponding state change is pushed to GitHub in the background. Failures are surfaced to the user via a transient error indicator.

---

## GitHub API mapping

| gh-inbox action | GitHub API call |
|---|---|
| Mark as read | `PATCH /notifications/threads/{thread_id}` |
| Archive | `DELETE /notifications/threads/{thread_id}` (mark as done) |
| Unarchive | No-op — local-only |

---

## Backend

### New GitHub functions (`src/github/notifications.rs`)

Two new async functions:

- `mark_thread_read(token, client, base_url, thread_id) -> Result<(), reqwest::Error>`
  Calls `PATCH /notifications/threads/{thread_id}`.

- `mark_thread_done(token, client, base_url, thread_id) -> Result<(), reqwest::Error>`
  Calls `DELETE /notifications/threads/{thread_id}`.

Both are pure HTTP calls with no local side effects.

### Action handlers (`src/api/inbox/read.rs`, `src/api/inbox/archive.rs`)

Each handler:
1. Updates local SQLite (existing behavior — unchanged).
2. Spawns a `tokio::spawn` fire-and-forget task calling the appropriate GitHub function.
3. On error in the spawned task, broadcasts a `SyncEvent::GithubSyncError { notification_id: String, message: String }` on the existing `AppState` broadcast channel.

The handler returns immediately after spawning — it does not await the GitHub call. The HTTP response to the frontend is unaffected by GitHub API success or failure.

### New `SyncEvent` variant (`src/models/sync_event.rs`)

```rust
GithubSyncError { notification_id: String, message: String }
```

Serialises to SSE event type `github:sync_error`.

### SSE handler (`src/api/events.rs`)

Add a `GithubSyncError` match arm that emits the SSE event with the notification ID and message. No other changes.

### No changes to the sync loop

`src/github/sync.rs` is not modified.

---

## Frontend

### SSE (`frontend/src/lib/sse.svelte.ts`)

Add `onGithubSyncError(notificationId: string, message: string)` callback and a listener for the `github:sync_error` event type.

### Topbar (`frontend/src/lib/Topbar.svelte`)

On `onGithubSyncError`, display a transient error banner/toast alongside the existing sync status indicator. The message reads: "Failed to sync with GitHub" (detail available in console for debugging). Dismisses after a few seconds or on user click.

---

## Error handling

- The spawned task catches errors from `reqwest` (network failures, 4xx/5xx from GitHub).
- A failed GitHub API call does not roll back the local SQLite change — local state is authoritative.
- 404 from GitHub (thread no longer exists) should be treated as a no-op success (not an error worth surfacing).

---

## Testing

### Backend
- Unit tests for `mark_thread_read` and `mark_thread_done`: mock server returning 205 (read) and 205 (done), assert correct HTTP method and path.
- Unit test for 404 no-op: mock server returns 404, assert no error is propagated.
- Integration test for `POST /api/inbox/:id/read`: mock GitHub endpoint, assert local DB updated and SSE event emitted on GitHub failure.
- Integration test for `POST /api/inbox/:id/archive`: same pattern.

### Frontend
- Test: `onGithubSyncError` callback fires when `github:sync_error` SSE event is received.
- Test: error banner renders in Topbar when sync error state is set.

---

## Summary of files changed

| File | Change |
|---|---|
| `src/github/notifications.rs` | Add `mark_thread_read`, `mark_thread_done` |
| `src/models/sync_event.rs` | Add `GithubSyncError` variant |
| `src/api/inbox/read.rs` | Spawn GitHub call after local update |
| `src/api/inbox/archive.rs` | Spawn GitHub call after local update |
| `src/api/events.rs` | Add `GithubSyncError` SSE match arm |
| `frontend/src/lib/sse.svelte.ts` | Add `onGithubSyncError` |
| `frontend/src/lib/Topbar.svelte` | Show transient error on sync failure |
