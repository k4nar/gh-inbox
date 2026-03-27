# Notification Sync Improvements — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the single-page, unconditional notification fetch with a two-mode sync that uses `since=` for 30s incremental cycles and fetches all pages for first-run or post-gap full syncs, while removing the frontend-triggered bootstrap.

**Architecture:** `sync_notifications` checks `last_fetched_at`; if None or >2h old it calls `fetch_all_notifications` (follows Link pagination) then reconciles by archiving anything GitHub no longer returns; otherwise it calls `fetch_notifications_since` with the ISO timestamp. The `GET /api/inbox` bootstrap block is deleted — the background loop owns the first sync.

**Tech Stack:** Rust, axum, sqlx (SQLite), reqwest, tokio

---

## File Map

| File | Change |
|---|---|
| `src/db/queries/notifications.rs` | Add `archive_if_not_in` |
| `src/github/mod.rs` | Add `get_url`, `base_url` to `GithubClient`; update `pub use` |
| `src/github/notifications.rs` | Add `parse_next_link`, `fetch_notifications_page`, `fetch_all_notifications`, `fetch_notifications_since`; remove `fetch_notifications` |
| `src/github/sync.rs` | Rewrite `sync_notifications`; add helpers; change loop interval; update test `make_state` |
| `src/api/inbox/get.rs` | Remove bootstrap block and its imports |
| `src/server.rs` | Remove `bootstrap_done` from `AppState` and construction |
| `tests/routes.rs` | Update `get_api_inbox_returns_notifications`; add `get_inbox_returns_empty_without_prior_sync` |

---

## Task 1: Add `archive_if_not_in` to DB queries

**Files:**
- Modify: `src/db/queries/notifications.rs`

- [ ] **Step 1: Write the failing tests**

Add inside the `#[cfg(test)]` block in `src/db/queries/notifications.rs`:

```rust
#[tokio::test]
async fn archive_if_not_in_archives_missing() {
    let pool = test_pool().await;
    upsert_notification(&pool, &sample("n1")).await.unwrap();
    upsert_notification(&pool, &sample("n2")).await.unwrap();
    upsert_notification(&pool, &sample("n3")).await.unwrap();

    let count = archive_if_not_in(&pool, &["n1", "n2"]).await.unwrap();
    assert_eq!(count, 1);

    let archived = query_archived(&pool).await.unwrap();
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].id, "n3");
}

#[tokio::test]
async fn archive_if_not_in_leaves_present_unchanged() {
    let pool = test_pool().await;
    upsert_notification(&pool, &sample("n1")).await.unwrap();
    upsert_notification(&pool, &sample("n2")).await.unwrap();

    let count = archive_if_not_in(&pool, &["n1", "n2"]).await.unwrap();
    assert_eq!(count, 0);

    assert_eq!(query_inbox(&pool).await.unwrap().len(), 2);
}

#[tokio::test]
async fn archive_if_not_in_returns_zero_for_empty_list() {
    let pool = test_pool().await;
    upsert_notification(&pool, &sample("n1")).await.unwrap();

    let count = archive_if_not_in(&pool, &[]).await.unwrap();
    assert_eq!(count, 0);
    assert_eq!(query_inbox(&pool).await.unwrap().len(), 1);
}

#[tokio::test]
async fn archive_if_not_in_skips_already_archived() {
    let pool = test_pool().await;
    upsert_notification(&pool, &sample("n1")).await.unwrap();
    archive_notification(&pool, "n1").await.unwrap();

    // n1 is not in the list, but it's already archived — rows_affected should be 0
    let count = archive_if_not_in(&pool, &[]).await.unwrap();
    assert_eq!(count, 0); // empty list guard fires before SQL
    let count2 = archive_if_not_in(&pool, &["n99"]).await.unwrap();
    assert_eq!(count2, 0); // n1 already archived, nothing new to archive
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cargo test -p gh-inbox notifications::tests::archive_if_not_in 2>&1 | tail -20
```

Expected: compile error — `archive_if_not_in` not found.

- [ ] **Step 3: Implement `archive_if_not_in`**

Add before the `#[cfg(test)]` block in `src/db/queries/notifications.rs`:

```rust
/// Archive all non-archived notifications whose id is NOT in `ids`.
/// Returns the number of rows affected.
/// If `ids` is empty, returns 0 immediately without touching the DB.
pub async fn archive_if_not_in(pool: &SqlitePool, ids: &[&str]) -> sqlx::Result<u64> {
    if ids.is_empty() {
        return Ok(0);
    }
    let placeholders = (0..ids.len()).map(|_| "?").collect::<Vec<_>>().join(", ");
    let sql = format!(
        "UPDATE notifications SET archived = 1 \
         WHERE archived = 0 AND id NOT IN ({placeholders})"
    );
    let mut query = sqlx::query(&sql);
    for id in ids {
        query = query.bind(*id);
    }
    let result = query.execute(pool).await?;
    Ok(result.rows_affected())
}
```

Also add it to the `pub use` in `src/db/queries/mod.rs`:

```rust
pub use notifications::{
    NotificationRow, archive_if_not_in, archive_notification, mark_read, query_archived,
    query_inbox, unarchive_notification, upsert_notification,
};
```

- [ ] **Step 4: Run tests to confirm they pass**

```bash
cargo test -p gh-inbox notifications::tests::archive_if_not_in 2>&1 | tail -20
```

Expected: 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/db/queries/notifications.rs src/db/queries/mod.rs
git commit -m "feat: add archive_if_not_in query for full-sync reconciliation"
```

---

## Task 2: Add `get_url` and `base_url` to `GithubClient`

**Files:**
- Modify: `src/github/mod.rs`

- [ ] **Step 1: Add the two methods**

In `src/github/mod.rs`, inside `impl GithubClient`, add after the existing `async fn delete` method:

```rust
/// Fetch a fully-qualified URL with the standard GitHub auth headers.
/// Used for following pagination `Link: rel="next"` URLs.
pub(crate) async fn get_url(&self, url: &str) -> Result<Response, reqwest::Error> {
    let builder = self.client
        .request(Method::GET, url)
        .header("Authorization", format!("Bearer {}", self.token))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "gh-inbox")
        .header("X-GitHub-Api-Version", "2026-03-10");
    self.execute(builder, "GET", url).await
}

/// The base URL this client sends requests to (e.g. `https://api.github.com`).
/// Needed by callers that build full pagination URLs.
pub fn base_url(&self) -> &str {
    &self.base_url
}
```

- [ ] **Step 2: Confirm it compiles**

```bash
cargo build 2>&1 | grep -E "^error"
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add src/github/mod.rs
git commit -m "feat: add get_url and base_url to GithubClient for pagination"
```

---

## Task 3: Add `parse_next_link` and `fetch_notifications_page`

**Files:**
- Modify: `src/github/notifications.rs`

- [ ] **Step 1: Write the failing tests**

Add a new `#[cfg(test)]` block at the bottom of `src/github/notifications.rs` (after the existing test modules):

```rust
#[cfg(test)]
mod page_tests {
    use std::sync::Arc;

    use axum::Router;
    use axum::routing::get;
    use tokio::net::TcpListener;

    use super::{fetch_notifications_page, parse_next_link};
    use crate::github::GithubClient;

    const ONE_NOTIFICATION: &str = r#"[{
        "id": "1",
        "reason": "review_requested",
        "unread": true,
        "updated_at": "2025-01-01T00:00:00Z",
        "subject": {
            "title": "Fix bug",
            "url": "https://api.github.com/repos/owner/repo/pulls/42",
            "type": "PullRequest"
        },
        "repository": { "full_name": "owner/repo" }
    }]"#;

    // --- parse_next_link unit tests ---

    #[test]
    fn parses_next_from_multi_rel_header() {
        let link = r#"<https://api.github.com/notifications?page=2>; rel="next", <https://api.github.com/notifications?page=5>; rel="last""#;
        assert_eq!(
            parse_next_link(link),
            Some("https://api.github.com/notifications?page=2".to_string())
        );
    }

    #[test]
    fn returns_none_when_no_next_rel() {
        let link = r#"<https://api.github.com/notifications?page=5>; rel="last""#;
        assert_eq!(parse_next_link(link), None);
    }

    #[test]
    fn returns_none_for_empty_string() {
        assert_eq!(parse_next_link(""), None);
    }

    // --- fetch_notifications_page integration tests ---

    #[tokio::test]
    async fn returns_notifications_and_next_url_from_link_header() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://{addr}");
        let next_url = format!("{base}/notifications?page=2&per_page=50");
        let next_url_clone = next_url.clone();

        let app = Router::new().route(
            "/notifications",
            get(move || {
                let next = next_url_clone.clone();
                async move {
                    axum::http::Response::builder()
                        .header("content-type", "application/json")
                        .header("link", format!(r#"<{next}>; rel="next""#))
                        .body(axum::body::Body::from(ONE_NOTIFICATION))
                        .unwrap()
                }
            }),
        );
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let github = GithubClient::new(Arc::from("fake-token"), base.clone());
        let url = format!("{base}/notifications?per_page=50");
        let (notifs, got_next) = fetch_notifications_page(&github, &url).await.unwrap();

        assert_eq!(notifs.len(), 1);
        assert_eq!(notifs[0].id, "1");
        assert_eq!(got_next, Some(next_url));
    }

    #[tokio::test]
    async fn returns_none_next_when_no_link_header() {
        let app = Router::new().route(
            "/notifications",
            get(|| async {
                axum::http::Response::builder()
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(ONE_NOTIFICATION))
                    .unwrap()
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://{addr}");
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let github = GithubClient::new(Arc::from("fake-token"), base.clone());
        let url = format!("{base}/notifications?per_page=50");
        let (notifs, next) = fetch_notifications_page(&github, &url).await.unwrap();

        assert_eq!(notifs.len(), 1);
        assert!(next.is_none());
    }
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cargo test -p gh-inbox page_tests 2>&1 | tail -20
```

Expected: compile error — `fetch_notifications_page` and `parse_next_link` not found.

- [ ] **Step 3: Implement `parse_next_link` and `fetch_notifications_page`**

Add before `pub async fn fetch_notifications` in `src/github/notifications.rs`:

```rust
/// Parse the URL with `rel="next"` from an HTTP `Link` header value.
/// Returns `None` if no next-page link is present.
fn parse_next_link(link: &str) -> Option<String> {
    for part in link.split(',') {
        let mut url: Option<String> = None;
        let mut is_next = false;
        for segment in part.trim().split(';') {
            let segment = segment.trim();
            if segment.starts_with('<') && segment.ends_with('>') {
                url = Some(segment[1..segment.len() - 1].to_string());
            } else if segment == r#"rel="next""# {
                is_next = true;
            }
        }
        if is_next {
            return url;
        }
    }
    None
}

/// Fetch a single page of notifications from a fully-qualified URL.
/// Returns the deserialized notifications and the next-page URL (from the
/// `Link: rel="next"` response header), if any.
pub(crate) async fn fetch_notifications_page(
    github: &super::GithubClient,
    url: &str,
) -> Result<(Vec<crate::models::Notification>, Option<String>), reqwest::Error> {
    let response = github.get_url(url).await?.error_for_status()?;
    let next = response
        .headers()
        .get("link")
        .and_then(|v| v.to_str().ok())
        .and_then(parse_next_link);
    let notifications: Vec<crate::models::Notification> = response.json().await?;
    Ok((notifications, next))
}
```

- [ ] **Step 4: Run tests to confirm they pass**

```bash
cargo test -p gh-inbox page_tests 2>&1 | tail -20
```

Expected: 5 tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/github/notifications.rs
git commit -m "feat: add fetch_notifications_page with Link header pagination support"
```

---

## Task 4: Add `fetch_all_notifications`

**Files:**
- Modify: `src/github/notifications.rs`

- [ ] **Step 1: Write the failing test**

Add inside `mod page_tests` in `src/github/notifications.rs`:

```rust
use std::sync::atomic::{AtomicUsize, Ordering};

const PAGE_1: &str = r#"[{
    "id": "1",
    "reason": "review_requested",
    "unread": true,
    "updated_at": "2025-01-01T00:00:00Z",
    "subject": { "title": "PR 1", "url": null, "type": "PullRequest" },
    "repository": { "full_name": "owner/repo" }
}]"#;

const PAGE_2: &str = r#"[{
    "id": "2",
    "reason": "mention",
    "unread": false,
    "updated_at": "2025-01-02T00:00:00Z",
    "subject": { "title": "PR 2", "url": null, "type": "PullRequest" },
    "repository": { "full_name": "owner/repo" }
}]"#;

#[tokio::test]
async fn fetch_all_follows_link_headers_across_pages() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base = format!("http://{addr}");
    let base_clone = base.clone();
    let call_count = Arc::new(AtomicUsize::new(0));
    let call_count_clone = call_count.clone();

    let app = Router::new().route(
        "/notifications",
        get(move || {
            let count = call_count_clone.fetch_add(1, Ordering::SeqCst);
            let base = base_clone.clone();
            async move {
                if count == 0 {
                    let next = format!("{base}/notifications?page=2&per_page=50");
                    axum::http::Response::builder()
                        .header("content-type", "application/json")
                        .header("link", format!(r#"<{next}>; rel="next""#))
                        .body(axum::body::Body::from(PAGE_1))
                        .unwrap()
                } else {
                    axum::http::Response::builder()
                        .header("content-type", "application/json")
                        .body(axum::body::Body::from(PAGE_2))
                        .unwrap()
                }
            }
        }),
    );
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let github = GithubClient::new(Arc::from("fake-token"), base);
    let notifs = super::fetch_all_notifications(&github).await.unwrap();

    assert_eq!(notifs.len(), 2);
    assert_eq!(notifs[0].id, "1");
    assert_eq!(notifs[1].id, "2");
}
```

- [ ] **Step 2: Run the test to confirm it fails**

```bash
cargo test -p gh-inbox page_tests::fetch_all_follows 2>&1 | tail -10
```

Expected: compile error — `fetch_all_notifications` not found.

- [ ] **Step 3: Implement `fetch_all_notifications`**

Add after `fetch_notifications_page` in `src/github/notifications.rs`:

```rust
/// Fetch ALL notifications from GitHub, following pagination links until exhausted.
/// Used for full syncs (first run or after a >2h gap).
pub async fn fetch_all_notifications(
    github: &super::GithubClient,
) -> Result<Vec<crate::models::Notification>, reqwest::Error> {
    let mut url = format!(
        "{}/notifications?all=true&per_page=50",
        github.base_url()
    );
    let mut all = Vec::new();
    loop {
        let (page, next) = fetch_notifications_page(github, &url).await?;
        all.extend(page);
        match next {
            Some(next_url) => url = next_url,
            None => break,
        }
    }
    Ok(all)
}
```

- [ ] **Step 4: Run the test to confirm it passes**

```bash
cargo test -p gh-inbox page_tests::fetch_all_follows 2>&1 | tail -10
```

Expected: 1 test passes.

- [ ] **Step 5: Commit**

```bash
git add src/github/notifications.rs
git commit -m "feat: add fetch_all_notifications with multi-page pagination"
```

---

## Task 5: Add `fetch_notifications_since`; update exports

**Files:**
- Modify: `src/github/notifications.rs`
- Modify: `src/github/mod.rs`

- [ ] **Step 1: Write the failing test**

Add inside `mod page_tests` in `src/github/notifications.rs`:

```rust
use std::sync::Mutex;
use axum::extract::Request;

#[tokio::test]
async fn fetch_since_includes_since_param_in_url() {
    let captured_uri = Arc::new(Mutex::new(String::new()));
    let captured_clone = captured_uri.clone();

    let app = Router::new().route(
        "/notifications",
        get(move |req: Request| {
            let captured = captured_clone.clone();
            async move {
                *captured.lock().unwrap() = req.uri().to_string();
                axum::http::Response::builder()
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from("[]"))
                    .unwrap()
            }
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base = format!("http://{addr}");
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let github = GithubClient::new(Arc::from("fake-token"), base);
    super::fetch_notifications_since(&github, "2025-06-01T00:00:00Z")
        .await
        .unwrap();

    let uri = captured_uri.lock().unwrap().clone();
    assert!(
        uri.contains("since="),
        "URI should contain since= param, got: {uri}"
    );
    assert!(
        uri.contains("2025-06-01"),
        "URI should contain the since date, got: {uri}"
    );
}
```

- [ ] **Step 2: Run the test to confirm it fails**

```bash
cargo test -p gh-inbox page_tests::fetch_since_includes 2>&1 | tail -10
```

Expected: compile error — `fetch_notifications_since` not found.

- [ ] **Step 3: Implement `fetch_notifications_since`**

Add after `fetch_all_notifications` in `src/github/notifications.rs`:

```rust
/// Fetch notifications updated after `since_iso` (ISO 8601 UTC string), following pagination.
/// Used for incremental syncs.
pub async fn fetch_notifications_since(
    github: &super::GithubClient,
    since_iso: &str,
) -> Result<Vec<crate::models::Notification>, reqwest::Error> {
    let mut url = format!(
        "{}/notifications?all=true&since={since_iso}&per_page=50",
        github.base_url()
    );
    let mut all = Vec::new();
    loop {
        let (page, next) = fetch_notifications_page(github, &url).await?;
        all.extend(page);
        match next {
            Some(next_url) => url = next_url,
            None => break,
        }
    }
    Ok(all)
}
```

- [ ] **Step 4: Update `pub use` in `src/github/mod.rs`**

Replace the existing notifications re-export:

```rust
// Before:
pub use notifications::{fetch_notifications, mark_thread_done, mark_thread_read};

// After:
pub use notifications::{
    fetch_all_notifications, fetch_notifications_since, mark_thread_done, mark_thread_read,
};
```

- [ ] **Step 5: Run the test to confirm it passes**

```bash
cargo test -p gh-inbox page_tests::fetch_since 2>&1 | tail -10
```

Expected: 1 test passes.

- [ ] **Step 6: Commit**

```bash
git add src/github/notifications.rs src/github/mod.rs
git commit -m "feat: add fetch_notifications_since for incremental sync; update exports"
```

---

## Task 6: Rewrite `sync_notifications` with two-mode logic; remove old `fetch_notifications`

**Files:**
- Modify: `src/github/sync.rs`
- Modify: `src/github/notifications.rs` (remove `fetch_notifications`)

- [ ] **Step 1: Write the failing tests**

Replace the existing `#[cfg(test)] mod tests` in `src/github/sync.rs` entirely with the block below. The existing tests (`inserts_notification_and_returns_changed_count`, `idempotent_when_data_unchanged`, `pr_id_extracted_from_subject_url`, `null_subject_url_gives_null_pr_id`) are preserved; new tests are added at the end.

```rust
#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicBool;

    use axum::Router;
    use axum::extract::Request;
    use axum::routing::get;
    use tokio::net::TcpListener;
    use tokio::sync::broadcast;

    use super::*;
    use crate::db::queries;
    use crate::github::GithubClient;

    async fn make_state(base_url: String) -> AppState {
        let pool = crate::db::init_with_path(":memory:").await;
        let (tx, _rx) = broadcast::channel(8);
        AppState {
            pool,
            github: GithubClient::new(Arc::from("fake-token"), base_url),
            tx,
            viewport_prs: Arc::new(tokio::sync::RwLock::new(std::collections::HashSet::new())),
            session_token: Arc::from("test-session-token"),
        }
    }

    // ── simple mock (no URL capture) ──────────────────────────────────────

    async fn start_mock(response: &'static str) -> String {
        let app = Router::new().route(
            "/notifications",
            get(move || async move { ([("content-type", "application/json")], response) }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}")
    }

    // ── URL-capturing mock ────────────────────────────────────────────────

    async fn start_mock_capturing(
        response: &'static str,
    ) -> (String, Arc<Mutex<Option<String>>>) {
        let captured = Arc::new(Mutex::new(None::<String>));
        let captured_clone = captured.clone();
        let app = Router::new().route(
            "/notifications",
            get(move |req: Request| {
                let cap = captured_clone.clone();
                async move {
                    *cap.lock().unwrap() = Some(req.uri().to_string());
                    axum::http::Response::builder()
                        .header("content-type", "application/json")
                        .body(axum::body::Body::from(response))
                        .unwrap()
                }
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (format!("http://{addr}"), captured)
    }

    // ── notification fixtures ─────────────────────────────────────────────

    const ONE_NOTIFICATION: &str = r#"[{
        "id": "1",
        "reason": "review_requested",
        "unread": true,
        "updated_at": "2025-01-01T00:00:00Z",
        "subject": {
            "title": "Fix bug",
            "url": "https://api.github.com/repos/owner/repo/pulls/42",
            "type": "PullRequest"
        },
        "repository": { "full_name": "owner/repo" }
    }]"#;

    const NULL_URL_NOTIFICATION: &str = r#"[{
        "id": "2",
        "reason": "mention",
        "unread": false,
        "updated_at": "2025-01-02T00:00:00Z",
        "subject": {
            "title": "Release note",
            "url": null,
            "type": "Release"
        },
        "repository": { "full_name": "owner/repo" }
    }]"#;

    const NOTIFICATION_ID_99: &str = r#"[{
        "id": "99",
        "reason": "mention",
        "unread": true,
        "updated_at": "2025-02-01T00:00:00Z",
        "subject": { "title": "Other PR", "url": null, "type": "PullRequest" },
        "repository": { "full_name": "owner/repo" }
    }]"#;

    // ── preserved tests ───────────────────────────────────────────────────

    #[tokio::test]
    async fn inserts_notification_and_returns_changed_count() {
        let state = make_state(start_mock(ONE_NOTIFICATION).await).await;

        let changed = sync_notifications(&state).await.unwrap();
        assert_eq!(changed.len(), 1);

        let inbox = queries::query_inbox(&state.pool).await.unwrap();
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].id, "1");
        assert_eq!(inbox[0].reason, "review_requested");
    }

    #[tokio::test]
    async fn idempotent_when_data_unchanged() {
        let state = make_state(start_mock(ONE_NOTIFICATION).await).await;

        assert_eq!(sync_notifications(&state).await.unwrap().len(), 1);
        // Second call: incremental (recent last_fetched_at), same data → 0 changed
        assert_eq!(sync_notifications(&state).await.unwrap().len(), 0);

        assert_eq!(queries::query_inbox(&state.pool).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn pr_id_extracted_from_subject_url() {
        let state = make_state(start_mock(ONE_NOTIFICATION).await).await;
        sync_notifications(&state).await.unwrap();

        let inbox = queries::query_inbox(&state.pool).await.unwrap();
        assert_eq!(inbox[0].pr_id, Some(42));
    }

    #[tokio::test]
    async fn null_subject_url_gives_null_pr_id() {
        let state = make_state(start_mock(NULL_URL_NOTIFICATION).await).await;
        sync_notifications(&state).await.unwrap();

        let inbox = queries::query_inbox(&state.pool).await.unwrap();
        assert_eq!(inbox[0].pr_id, None);
    }

    // ── new two-mode tests ────────────────────────────────────────────────

    #[tokio::test]
    async fn full_sync_when_never_fetched_does_not_include_since() {
        let (base, captured) = start_mock_capturing(ONE_NOTIFICATION).await;
        let state = make_state(base).await;
        // No last_fetched_at set → full sync

        sync_notifications(&state).await.unwrap();

        let uri = captured.lock().unwrap().clone().unwrap();
        assert!(
            !uri.contains("since="),
            "Full sync should not send since=, got: {uri}"
        );
        assert!(uri.contains("all=true"), "Should use all=true, got: {uri}");
    }

    #[tokio::test]
    async fn full_sync_when_last_fetch_over_2h_does_not_include_since() {
        let (base, captured) = start_mock_capturing(ONE_NOTIFICATION).await;
        let state = make_state(base).await;

        let three_hours_ago = now_epoch() - 3 * 3600;
        sqlx::query(
            "INSERT INTO last_fetched_at (resource, fetched_at) VALUES ('notifications', ?)",
        )
        .bind(three_hours_ago)
        .execute(&state.pool)
        .await
        .unwrap();

        sync_notifications(&state).await.unwrap();

        let uri = captured.lock().unwrap().clone().unwrap();
        assert!(
            !uri.contains("since="),
            "Stale full sync should not send since=, got: {uri}"
        );
    }

    #[tokio::test]
    async fn incremental_sync_when_recently_fetched_includes_since() {
        let (base, captured) = start_mock_capturing(ONE_NOTIFICATION).await;
        let state = make_state(base).await;

        let one_min_ago = now_epoch() - 60;
        sqlx::query(
            "INSERT INTO last_fetched_at (resource, fetched_at) VALUES ('notifications', ?)",
        )
        .bind(one_min_ago)
        .execute(&state.pool)
        .await
        .unwrap();

        sync_notifications(&state).await.unwrap();

        let uri = captured.lock().unwrap().clone().unwrap();
        assert!(
            uri.contains("since="),
            "Incremental sync should send since=, got: {uri}"
        );
    }

    #[tokio::test]
    async fn incremental_sync_since_value_matches_last_fetched_at() {
        let (base, captured) = start_mock_capturing("[]").await;
        let state = make_state(base).await;

        // 1735689600 = 2025-01-01T00:00:00Z
        let epoch: i64 = 1735689600;
        sqlx::query(
            "INSERT INTO last_fetched_at (resource, fetched_at) VALUES ('notifications', ?)",
        )
        .bind(epoch)
        .execute(&state.pool)
        .await
        .unwrap();

        sync_notifications(&state).await.unwrap();

        let uri = captured.lock().unwrap().clone().unwrap();
        assert!(
            uri.contains("since=2025-01-01"),
            "URI should contain since=2025-01-01, got: {uri}"
        );
    }

    #[tokio::test]
    async fn full_sync_archives_notifications_missing_from_github() {
        // id="1" is in the mock response; id="99" is not → should be archived
        let state = make_state(start_mock(ONE_NOTIFICATION).await).await;

        // Pre-insert two notifications
        queries::upsert_notification(
            &state.pool,
            &queries::NotificationRow {
                id: "1".to_string(),
                pr_id: Some(42),
                title: "Fix bug".to_string(),
                repository: "owner/repo".to_string(),
                reason: "review_requested".to_string(),
                unread: true,
                archived: false,
                updated_at: "2025-01-01T00:00:00Z".to_string(),
            },
        )
        .await
        .unwrap();
        queries::upsert_notification(
            &state.pool,
            &queries::NotificationRow {
                id: "99".to_string(),
                pr_id: None,
                title: "Gone".to_string(),
                repository: "owner/repo".to_string(),
                reason: "mention".to_string(),
                unread: true,
                archived: false,
                updated_at: "2025-01-01T00:00:00Z".to_string(),
            },
        )
        .await
        .unwrap();

        // Full sync (no last_fetched_at) — GitHub returns only id="1"
        sync_notifications(&state).await.unwrap();

        let archived = queries::query_archived(&state.pool).await.unwrap();
        assert_eq!(archived.len(), 1, "id=99 should be archived");
        assert_eq!(archived[0].id, "99");

        let inbox = queries::query_inbox(&state.pool).await.unwrap();
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].id, "1");
    }
}
```

- [ ] **Step 2: Run tests to confirm they fail**

```bash
cargo test -p gh-inbox sync::tests 2>&1 | tail -20
```

Expected: compile error or test failures due to missing `now_epoch`, `epoch_to_iso` helpers and unchanged `sync_notifications`.

- [ ] **Step 3: Add helpers and rewrite `sync_notifications`**

Replace the entire `sync_notifications` function and add helpers above it in `src/github/sync.rs`. Also remove the `use crate::github::fetch_notifications` (or `super::fetch_notifications`) import if present (it will be replaced by the new calls below).

Add these helpers right after the `impl From<sqlx::Error> for SyncError` block:

```rust
const FULL_SYNC_THRESHOLD_SECS: i64 = 2 * 60 * 60; // 2 hours

fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before UNIX epoch")
        .as_secs() as i64
}

/// Convert a Unix epoch (seconds) to an ISO 8601 UTC string suitable for
/// the GitHub API `since` parameter.  Avoids adding a time-crate dependency.
fn epoch_to_iso(epoch: i64) -> String {
    let mut rem = epoch as u64;
    let ss = rem % 60;
    rem /= 60;
    let mm = rem % 60;
    rem /= 60;
    let hh = rem % 24;
    rem /= 24;
    let (y, mo, d) = days_since_epoch_to_ymd(rem as u32);
    format!("{y:04}-{mo:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

fn days_since_epoch_to_ymd(mut days: u32) -> (u32, u32, u32) {
    let mut year = 1970u32;
    loop {
        let diy = if is_leap_year(year) { 366 } else { 365 };
        if days < diy {
            break;
        }
        days -= diy;
        year += 1;
    }
    const MONTH_DAYS: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1u32;
    for (i, &base) in MONTH_DAYS.iter().enumerate() {
        let dim = if i == 1 && is_leap_year(year) { 29 } else { base };
        if days < dim {
            break;
        }
        days -= dim;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap_year(y: u32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}
```

Then replace the `sync_notifications` function:

```rust
/// Fetch notifications from GitHub and upsert into the database.
///
/// **Full sync** (first run or last fetch >2h ago): fetches all pages and
/// archives any local notification that GitHub no longer returns.
///
/// **Incremental sync** (recent last fetch): fetches only notifications
/// changed since the last fetch using the `since=` parameter.
///
/// Returns the notifications that changed (upserted or reconciliation-archived).
pub async fn sync_notifications(state: &AppState) -> Result<Vec<ChangedNotification>, SyncError> {
    let last_fetched = queries::get_last_fetched_epoch(&state.pool, "notifications").await?;
    let now = now_epoch();

    let is_full_sync = last_fetched
        .map(|t| now - t > FULL_SYNC_THRESHOLD_SECS)
        .unwrap_or(true);

    let notifications = if is_full_sync {
        super::fetch_all_notifications(&state.github).await?
    } else {
        let since_iso = epoch_to_iso(last_fetched.unwrap());
        super::fetch_notifications_since(&state.github, &since_iso).await?
    };

    let mut changed = Vec::new();
    let mut returned_ids: Vec<String> = Vec::new();

    for notif in &notifications {
        let pr_id = notif
            .subject
            .url
            .as_deref()
            .and_then(|url| url.rsplit('/').next())
            .and_then(|s| s.parse::<i64>().ok());

        let row = queries::NotificationRow {
            id: notif.id.clone(),
            pr_id,
            title: notif.subject.title.clone(),
            repository: notif.repository.full_name.clone(),
            reason: notif.reason.clone(),
            unread: notif.unread,
            archived: false,
            updated_at: notif.updated_at.clone(),
        };

        let rows_affected = queries::upsert_notification(&state.pool, &row).await?;
        if rows_affected > 0 {
            changed.push(ChangedNotification {
                repository: notif.repository.full_name.clone(),
                pr_id,
            });
        }
        returned_ids.push(notif.id.clone());
    }

    // Full sync reconciliation: archive notifications GitHub no longer returns.
    // Guard: skip if returned_ids is empty to avoid archiving everything on an
    // unexpected empty response.
    if is_full_sync && !returned_ids.is_empty() {
        let id_refs: Vec<&str> = returned_ids.iter().map(|s| s.as_str()).collect();
        let archived_count = queries::archive_if_not_in(&state.pool, &id_refs).await?;
        // Push dummy entries so run_sync_loop fires SSE events for reconciled items.
        for _ in 0..archived_count {
            changed.push(ChangedNotification {
                repository: String::new(),
                pr_id: None,
            });
        }
    }

    queries::set_last_fetched_now(&state.pool, "notifications").await?;

    Ok(changed)
}
```

- [ ] **Step 4: Remove `fetch_notifications` from `src/github/notifications.rs`**

Delete the `fetch_notifications` function entirely (the `pub async fn fetch_notifications` that calls `/notifications?all=true`).

- [ ] **Step 5: Run tests to confirm they pass**

```bash
cargo test -p gh-inbox sync::tests 2>&1 | tail -30
```

Expected: all 9 tests pass (4 preserved + 5 new).

- [ ] **Step 6: Confirm full test suite passes**

```bash
cargo test 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/github/sync.rs src/github/notifications.rs src/github/mod.rs
git commit -m "feat: two-mode sync — incremental with since=, full with pagination and reconciliation"
```

---

## Task 7: Change default sync interval from 60s to 30s

**Files:**
- Modify: `src/github/sync.rs`

- [ ] **Step 1: Update the default interval**

In `run_sync_loop` in `src/github/sync.rs`, change:

```rust
// Before:
.unwrap_or(60);

// After:
.unwrap_or(30);
```

- [ ] **Step 2: Confirm it compiles**

```bash
cargo build 2>&1 | grep -E "^error"
```

Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add src/github/sync.rs
git commit -m "chore: reduce default sync interval from 60s to 30s"
```

---

## Task 8: Remove the frontend-triggered bootstrap

**Files:**
- Modify: `src/api/inbox/get.rs`
- Modify: `src/server.rs`
- Modify: `src/github/sync.rs` (test helper `make_state`)
- Modify: `tests/routes.rs`

- [ ] **Step 1: Add the `get_inbox_returns_empty_without_prior_sync` integration test**

Add to `tests/routes.rs`:

```rust
#[tokio::test]
async fn get_inbox_returns_empty_without_prior_sync() {
    // After bootstrap removal, GET /api/inbox reads from DB only.
    // An empty DB returns an empty paginated response — no GitHub call needed.
    let pool = gh_inbox::db::init_with_path(":memory:").await;
    let (app, _state) = gh_inbox::app(pool, Arc::from("fake-token"));

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/inbox")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let items = parse_inbox_items(&body);
    assert!(items.is_empty());
}
```

- [ ] **Step 2: Run the new test to confirm it currently fails (bootstrap makes it non-empty OR passes for wrong reason)**

```bash
cargo test -p gh-inbox get_inbox_returns_empty_without_prior_sync 2>&1 | tail -10
```

This test passes vacuously today (the bootstrap calls GitHub which returns an error on `fake-token`), but the intent is that after the change the test passes cleanly without any GitHub call.

- [ ] **Step 3: Update `get_api_inbox_returns_notifications` in `tests/routes.rs`**

The test currently relies on the bootstrap triggering a sync. Replace it with a version that pre-populates the DB directly:

```rust
#[tokio::test]
async fn get_api_inbox_returns_notifications() {
    let pool = gh_inbox::db::init_with_path(":memory:").await;

    // Pre-populate DB directly — the endpoint reads from DB, not GitHub.
    gh_inbox::db::queries::upsert_notification(
        &pool,
        &gh_inbox::db::queries::NotificationRow {
            id: "123".to_string(),
            pr_id: Some(42),
            title: "Add feature X".to_string(),
            repository: "owner/repo".to_string(),
            reason: "review_requested".to_string(),
            unread: true,
            archived: false,
            updated_at: "2025-06-01T10:00:00Z".to_string(),
        },
    )
    .await
    .unwrap();

    let (app, _state) = gh_inbox::app(pool, Arc::from("fake-token"));

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/inbox")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let notifications = parse_inbox_items(&body);

    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0]["id"], "123");
    assert_eq!(notifications[0]["reason"], "review_requested");
    assert_eq!(notifications[0]["pr_id"], 42);
    assert!(notifications[0]["unread"].as_bool().unwrap());
}
```

- [ ] **Step 4: Remove the bootstrap block from `src/api/inbox/get.rs`**

Delete these lines from `get_inbox`:

```rust
// Remove this import at the top of the file:
use std::sync::atomic::Ordering;
use crate::github::sync::sync_notifications;

// Remove this block inside get_inbox:
// Bootstrap on first request
if state
    .bootstrap_done
    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
    .is_ok()
{
    let has_fetched = queries::get_last_fetched_epoch(&state.pool, "notifications")
        .await
        .map_err(AppError::Database)?
        .is_some();
    if !has_fetched {
        sync_notifications(&state).await?;
    }
}
```

The resulting `get_inbox` function starts directly with:
```rust
let page = query.page.unwrap_or(1).max(1);
```

- [ ] **Step 5: Remove `bootstrap_done` from `AppState` in `src/server.rs`**

Remove from `AppState` struct:
```rust
pub bootstrap_done: Arc<AtomicBool>,
```

Remove from the state construction in `app_with_base_url`:
```rust
bootstrap_done: Arc::new(AtomicBool::new(false)),
```

Remove the now-unused import at the top of `src/server.rs`:
```rust
use std::sync::atomic::AtomicBool;
```

- [ ] **Step 6: Update `make_state` in `src/github/sync.rs`**

Remove `bootstrap_done` from the `AppState` construction in the test helper:

```rust
// Before:
AppState {
    pool,
    github: GithubClient::new(Arc::from("fake-token"), base_url),
    tx,
    bootstrap_done: Arc::new(AtomicBool::new(false)),
    viewport_prs: Arc::new(tokio::sync::RwLock::new(std::collections::HashSet::new())),
    session_token: Arc::from("test-session-token"),
}

// After:
AppState {
    pool,
    github: GithubClient::new(Arc::from("fake-token"), base_url),
    tx,
    viewport_prs: Arc::new(tokio::sync::RwLock::new(std::collections::HashSet::new())),
    session_token: Arc::from("test-session-token"),
}
```

Also remove the unused import at the top of the test module:
```rust
use std::sync::atomic::AtomicBool;
```

- [ ] **Step 7: Run the full test suite**

```bash
cargo test 2>&1 | tail -30
```

Expected: all tests pass, including the two new/updated integration tests.

- [ ] **Step 8: Commit**

```bash
git add src/api/inbox/get.rs src/server.rs src/github/sync.rs tests/routes.rs
git commit -m "refactor: remove frontend-triggered bootstrap; sync loop owns first sync"
```

---

## Final verification

- [ ] **Run `cargo fmt` and `cargo clippy`**

```bash
cargo fmt && cargo clippy -- -D warnings 2>&1 | tail -20
```

Expected: no warnings or errors.

- [ ] **Run full test suite one last time**

```bash
cargo test 2>&1 | grep -E "test result|FAILED"
```

Expected: `test result: ok`.
