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

Note: `DELETE /notifications/threads/{thread_id}` is the official "mark as done" endpoint. If the token lacks the required scope and returns 403, treat it as a no-op — do not surface a sync error to the user.

The notification `id` stored in SQLite (sourced directly from GitHub's `id` field on the notification object) is the GitHub thread ID. No additional lookup is needed — it can be passed directly to the API endpoints.

---

## Backend

### New helpers in `src/github/mod.rs`

The existing `github_request` helper builds GET requests. Add two analogous private helpers that return `RequestBuilder` (same return type as `github_request`) — the caller is responsible for chaining `.send().await?.error_for_status()?`:

- `github_patch(client, token, url) -> RequestBuilder` — sets `PATCH` method plus the same four headers (`Authorization`, `Accept`, `User-Agent`, `X-GitHub-Api-Version`) as `github_request`.
- `github_delete(client, token, url) -> RequestBuilder` — same but for `DELETE`.

### New GitHub functions (`src/github/notifications.rs`)

Two new async functions:

- `mark_thread_read(token, client, base_url, thread_id) -> Result<(), reqwest::Error>`
  Uses `github_patch` on `/notifications/threads/{thread_id}`. Inspects the response status before error conversion: 404 and 403 are treated as `Ok(())` (no error). Any other non-2xx is returned as `Err`.

- `mark_thread_done(token, client, base_url, thread_id) -> Result<(), reqwest::Error>`
  Uses `github_delete` on `/notifications/threads/{thread_id}`. Same 404/403 no-op logic as above.

Both functions must not use `error_for_status()` directly; instead they must check `response.status()` first and short-circuit for 403/404.

### Action handlers (`src/api/inbox/read.rs`, `src/api/inbox/archive.rs`)

Each handler:
1. Updates local SQLite (existing behavior — unchanged).
2. Spawns a `tokio::spawn` fire-and-forget task calling the appropriate GitHub function. The `JoinHandle` is dropped (not awaited). If the task panics, Tokio's default panic hook logs it.
3. On `Err` from the GitHub function inside the spawned task, broadcasts `SyncEvent::GithubSyncError(GithubSyncErrorData { notification_id, message })` on the existing `AppState` broadcast channel.

The handler returns immediately after spawning — it does not await the GitHub call. The HTTP response to the frontend is unaffected by GitHub API success or failure.

### New `SyncEvent` variant (`src/models/sync_event.rs`)

```rust
GithubSyncError(GithubSyncErrorData)
```

Where `GithubSyncErrorData` is a `#[derive(Serialize)]` struct:

```rust
pub struct GithubSyncErrorData {
    pub notification_id: String,
    pub message: String,
}
```

Follows the existing pattern of `NewNotificationsData`, `SyncStatusData`, `PrTeamsUpdatedData`. Serialises to SSE event type `github:sync_error`. This is distinct from `sync:status` (which covers background sync-loop failures) — `github:sync_error` is only for user-action push failures.

### SSE handler (`src/api/events.rs`)

Add a `GithubSyncError` match arm that emits the SSE event with the notification ID and message. No other changes.

### No changes to the sync loop

`src/github/sync.rs` is not modified.

---

## Frontend

### SSE (`frontend/src/lib/sse.svelte.ts`)

Export a new registration function following the existing pattern:

```ts
export function onGithubSyncError(
  callback: (notificationId: string, message: string) => void
): () => void
```

Adds a listener for the `github:sync_error` event type and returns an unsubscribe function.

### App (`frontend/src/lib/App.svelte`)

In `onMount`, alongside the existing SSE subscriptions, register `onGithubSyncError`. On receipt, call the existing `showError('Failed to sync with GitHub')` from `toast.svelte.ts`. The toast infrastructure (auto-dismiss, styling, rendering) is already in place — no new UI components needed.

---

## Error handling

- The spawned task catches `Err` from `reqwest` (network failures, non-2xx from GitHub excluding 403/404).
- A failed GitHub API call does not roll back the local SQLite change — local state is authoritative.
- 404 from GitHub (thread no longer exists): no-op success.
- 403 from GitHub (insufficient token scope): no-op success.
- Any other non-2xx response: emits `GithubSyncError` via SSE.
- Task panics are logged by Tokio's default panic hook (JoinHandle is dropped).

---

## Testing

### Backend
- Unit tests for `mark_thread_read` and `mark_thread_done`: mock server returning 205, assert correct HTTP method, path, and required headers.
- Unit test for 404 no-op: mock server returns 404, assert `Ok(())` is returned.
- Unit test for 403 no-op: mock server returns 403, assert `Ok(())` is returned.
- Integration test for `POST /api/inbox/:id/read`: mock GitHub endpoint returning a 500 error; subscribe to `state.tx` broadcast channel before the request; assert local DB updated and `GithubSyncError` event received (use `tokio::time::timeout` of 500ms to avoid flakiness).
- Integration test for `POST /api/inbox/:id/archive`: same pattern.

### Frontend
- Test: `onGithubSyncError` registration function fires callback when `github:sync_error` SSE event is received.
- Test: error banner renders in Topbar when sync error state is set.

---

## Summary of files changed

| File | Change |
|---|---|
| `src/github/mod.rs` | Add `github_patch`, `github_delete` helpers |
| `src/github/notifications.rs` | Add `mark_thread_read`, `mark_thread_done` |
| `src/models/sync_event.rs` | Add `GithubSyncError` variant |
| `src/api/inbox/read.rs` | Spawn GitHub call after local update |
| `src/api/inbox/archive.rs` | Spawn GitHub call after local update |
| `src/api/events.rs` | Add `GithubSyncError` SSE match arm |
| `frontend/src/lib/sse.svelte.ts` | Add `onGithubSyncError` |
| `frontend/src/lib/App.svelte` | Register `onGithubSyncError`, call `showError()` on receipt |
