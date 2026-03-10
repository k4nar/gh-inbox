# M7 — Inbox Actions Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Users can mark notifications as read, archive/unarchive PRs, view archived PRs, and switch between inbox/archived views with optimistic UI.

**Architecture:** Backend adds 3 thin POST handlers calling existing DB queries, plus a query param on GET /api/inbox. Frontend lifts notification state to App.svelte, makes Sidebar reactive, and adds action buttons with optimistic updates to PrList.

**Tech Stack:** Rust/axum (backend handlers), Svelte 5 runes (frontend state), vitest + testing-library (frontend tests), axum integration tests (backend tests)

---

## Chunk 1: Backend

### Task 1: Add NotFound error variant

**Files:**
- Modify: `src/api/error.rs`

- [ ] **Step 1: Add NotFound variant to AppError and update IntoResponse**

In `src/api/error.rs`, add a `NotFound` variant to `AppError`:

```rust
pub enum AppError {
    GitHub(reqwest::Error),
    Database(sqlx::Error),
    NotFound(String),
}
```

Update the `IntoResponse` impl match arm:

```rust
AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
```

- [ ] **Step 2: Add unit test for NotFound variant**

```rust
#[test]
fn not_found_error_maps_to_404() {
    let app_err = AppError::NotFound("notification not found".to_string());
    let response = app_err.into_response();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test`
Expected: All tests pass including the new one.

- [ ] **Step 4: Commit**

```bash
git add src/api/error.rs
git commit -m "Add NotFound error variant (M7)"
```

---

### Task 2: Add POST handlers and query param to inbox API

**Files:**
- Modify: `src/api/inbox.rs`
- Modify: `src/db/queries.rs` (add `notification_exists` helper)

- [ ] **Step 1: Add notification_exists query helper**

In `src/db/queries.rs`:

```rust
pub async fn notification_exists(pool: &SqlitePool, id: &str) -> sqlx::Result<bool> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT 1 FROM notifications WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    Ok(row.is_some())
}
```

- [ ] **Step 2: Add InboxQuery struct and update get_inbox**

In `src/api/inbox.rs`, add a query param struct and modify `get_inbox`:

```rust
use axum::extract::Query;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct InboxQuery {
    pub status: Option<String>,
}
```

Update `get_inbox` signature to accept `Query<InboxQuery>` and dispatch to `query_archived` when `status=archived`.

- [ ] **Step 3: Add post_mark_read handler**

```rust
pub async fn post_mark_read(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    if !queries::notification_exists(&state.pool, &id).await? {
        return Err(AppError::NotFound(format!("notification {id} not found")));
    }
    queries::mark_read(&state.pool, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
```

- [ ] **Step 4: Add post_archive handler**

Same pattern as post_mark_read but calls `queries::archive_notification`.

- [ ] **Step 5: Add post_unarchive handler**

Same pattern but calls `queries::unarchive_notification`.

- [ ] **Step 6: Run tests**

Run: `cargo test`
Expected: All existing tests pass (new handlers not wired yet).

- [ ] **Step 7: Commit**

```bash
git add src/api/inbox.rs src/db/queries.rs
git commit -m "Add inbox action handlers: mark read, archive, unarchive (M7)"
```

---

### Task 3: Wire routes in server.rs

**Files:**
- Modify: `src/server.rs`

- [ ] **Step 1: Add POST routes**

```rust
use axum::routing::{get, post};

// Add to router:
.route("/api/inbox/{id}/read", post(api::inbox::post_mark_read))
.route("/api/inbox/{id}/archive", post(api::inbox::post_archive))
.route("/api/inbox/{id}/unarchive", post(api::inbox::post_unarchive))
```

- [ ] **Step 2: Run tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 3: Commit**

```bash
git add src/server.rs
git commit -m "Wire inbox action routes (M7)"
```

---

### Task 4: Backend integration tests

**Files:**
- Modify: `tests/routes.rs`

- [ ] **Step 1: Add test for archive flow**

Test: insert notification via GET /api/inbox (mock), POST archive, verify GET /api/inbox returns empty, GET /api/inbox?status=archived returns it.

- [ ] **Step 2: Add test for unarchive flow**

Test: archive then unarchive, verify it's back in inbox.

- [ ] **Step 3: Add test for mark read**

Test: POST mark read, verify GET /api/inbox shows unread=false.

- [ ] **Step 4: Add test for 404 on nonexistent ID**

Test: POST /api/inbox/nonexistent/archive returns 404.

- [ ] **Step 5: Run tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 6: Commit**

```bash
git add tests/routes.rs
git commit -m "Add integration tests for inbox actions (M7)"
```

---

## Chunk 2: Frontend

### Task 5: Lift state to App.svelte and make Sidebar reactive

**Files:**
- Modify: `frontend/src/App.svelte`
- Modify: `frontend/src/lib/Sidebar.svelte`
- Modify: `frontend/src/lib/Sidebar.test.js`

- [ ] **Step 1: Add currentView state to App.svelte**

Add `let currentView = $state("inbox");` and pass it to Sidebar and PrList as props. Pass an `onViewChange` callback to Sidebar.

- [ ] **Step 2: Make Sidebar accept props and be reactive**

Accept `currentView` and `onViewChange` props. Replace static `active` class with dynamic binding based on `currentView`. Replace `<a>` with `<button>` elements that call `onViewChange`.

- [ ] **Step 3: Update Sidebar tests**

Test: clicking Inbox/Archived calls onViewChange with correct value. Test: active class follows currentView prop.

- [ ] **Step 4: Run frontend tests**

Run: `cd frontend && npm test`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add frontend/src/App.svelte frontend/src/lib/Sidebar.svelte frontend/src/lib/Sidebar.test.js
git commit -m "Lift view state to App, make Sidebar reactive (M7)"
```

---

### Task 6: Update PrList with actions and view switching

**Files:**
- Modify: `frontend/src/lib/PrList.svelte`
- Modify: `frontend/src/lib/PrList.test.js`

- [ ] **Step 1: Accept currentView prop and refetch on change**

Replace `onMount` fetch with `$effect` that fetches from `/api/inbox?status={currentView}` whenever `currentView` changes.

- [ ] **Step 2: Update header title and empty state per view**

Header title: "Inbox" or "Archived" based on `currentView`. Empty states: "All caught up!" for inbox, "No archived notifications." for archived.

- [ ] **Step 3: Add archive/unarchive action buttons on rows**

Show archive button in inbox view, unarchive button in archived view. Buttons call POST endpoints and optimistically remove item from list.

- [ ] **Step 4: Mark as read on click (optimistic UI)**

When clicking a PR row, also POST to `/api/inbox/{id}/read` and optimistically set `unread = false`.

- [ ] **Step 5: Update PrList tests**

Test: archive removes from list. Test: view switching changes empty state text. Test: mark-read removes unread dot. Test: fetch URL includes `?status=` parameter.

- [ ] **Step 6: Run all tests**

Run: `cd frontend && npm test`
Expected: All tests pass.

- [ ] **Step 7: Commit**

```bash
git add frontend/src/lib/PrList.svelte frontend/src/lib/PrList.test.js
git commit -m "Add inbox actions and view switching to PrList (M7)"
```

---

### Task 7: End-to-end verification

- [ ] **Step 1: Run all backend tests**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 2: Run all frontend tests**

Run: `cd frontend && npm test`
Expected: All pass.

- [ ] **Step 3: Manual smoke test**

Run: `cargo run` — verify archive/read/unarchive flow works end-to-end with real GitHub data.
