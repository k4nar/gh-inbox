# ETag + Light Prefetch Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace time-based PR prefetch throttling with ETag conditional requests and a single-endpoint prefetch, so inbox rows update without consuming rate limit when nothing changed.

**Architecture:** Add an `etag` column to `last_fetched_at`. Introduce a `ConditionalResponse<T>` type in the GitHub layer with a new `fetch_pull_request_conditional` that sends `If-None-Match` and handles 304. Replace the 5-call `fetch_and_cache_pr` in the prefetch path with a new single-call `fetch_and_cache_pr_meta` that uses ETags. The PR detail view (`fetch_and_cache_pr`) is unchanged.

**Tech Stack:** Rust, sqlx (SQLite), reqwest, axum, tokio

---

## Chunk 1: DB + GitHub layer

### Task 1: Migration — add `etag` column to `last_fetched_at`

**Files:**
- Create: `migrations/011_add_etag_to_last_fetched_at.sql`

- [ ] **Write migration**

```sql
ALTER TABLE last_fetched_at ADD COLUMN etag TEXT;
```

- [ ] **Run `cargo test`** to confirm migration applies cleanly on startup

```bash
cargo test 2>&1 | tail -5
```
Expected: all pass.

- [ ] **Commit**

```bash
git add migrations/011_add_etag_to_last_fetched_at.sql
git commit -m "feat: add etag column to last_fetched_at"
```

---

### Task 2: DB layer — ETag read/write

**Files:**
- Modify: `src/db/queries/fetches.rs`

- [ ] **Write failing tests** (add inside the existing `#[cfg(test)]` block)

```rust
#[tokio::test]
async fn get_etag_returns_none_when_not_set() {
    let pool = test_pool().await;
    set_last_fetched_now(&pool, "res").await.unwrap();
    let etag = get_etag(&pool, "res").await.unwrap();
    assert!(etag.is_none());
}

#[tokio::test]
async fn set_and_get_etag_round_trip() {
    let pool = test_pool().await;
    set_fetch_state(&pool, "res", Some("\"abc123\"")).await.unwrap();
    let etag = get_etag(&pool, "res").await.unwrap();
    assert_eq!(etag.as_deref(), Some("\"abc123\""));
}

#[tokio::test]
async fn set_fetch_state_without_etag_clears_previous_etag() {
    let pool = test_pool().await;
    set_fetch_state(&pool, "res", Some("\"old\"")).await.unwrap();
    set_fetch_state(&pool, "res", None).await.unwrap();
    let etag = get_etag(&pool, "res").await.unwrap();
    assert!(etag.is_none());
}
```

- [ ] **Run tests to verify they fail**

```bash
cargo test -p gh-inbox get_etag set_fetch_state 2>&1 | grep -E "FAILED|error"
```
Expected: compile error (functions not defined yet).

- [ ] **Add `get_etag` and `set_fetch_state` functions**

Add after the existing `set_last_fetched_now` function:

```rust
/// Get the stored ETag for a resource, if any.
pub async fn get_etag(pool: &SqlitePool, resource: &str) -> sqlx::Result<Option<String>> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT etag FROM last_fetched_at WHERE resource = ?")
            .bind(resource)
            .fetch_optional(pool)
            .await?;
    Ok(row.and_then(|r| r.0))
}

/// Upsert the fetch timestamp and ETag for a resource.
pub async fn set_fetch_state(
    pool: &SqlitePool,
    resource: &str,
    etag: Option<&str>,
) -> sqlx::Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before UNIX epoch")
        .as_secs() as i64;
    sqlx::query(
        "INSERT INTO last_fetched_at (resource, fetched_at, etag)
         VALUES (?, ?, ?)
         ON CONFLICT(resource) DO UPDATE SET fetched_at = excluded.fetched_at, etag = excluded.etag",
    )
    .bind(resource)
    .bind(now)
    .bind(etag)
    .execute(pool)
    .await?;
    Ok(())
}
```

- [ ] **Run tests to verify they pass**

```bash
cargo test get_etag set_fetch_state 2>&1 | grep -E "ok|FAILED"
```
Expected: 3 tests pass.

- [ ] **Export the new functions from `src/db/queries/mod.rs`**

Change line 12:
```rust
// Before:
pub use fetches::{get_last_fetched_epoch, set_last_fetched_now};
// After:
pub use fetches::{get_etag, get_last_fetched_epoch, set_fetch_state, set_last_fetched_now};
```

- [ ] **Run full test suite**

```bash
cargo test 2>&1 | tail -5
```
Expected: all tests pass.

- [ ] **Commit**

```bash
git add src/db/queries/fetches.rs src/db/queries/mod.rs
git commit -m "feat: add get_etag and set_fetch_state to DB layer"
```

---

### Task 3: GitHub layer — conditional PR metadata fetch

**Files:**
- Modify: `src/github/mod.rs`
- Modify: `src/github/pull_requests.rs`

- [ ] **Write failing test** (add to existing `#[cfg(test)]` block in `pull_requests.rs`)

The `fetch_pull_request_conditional` function can't be easily unit-tested with live HTTP, but we can test the `ConditionalResponse` type and the 304 branch via a mock. Since the project doesn't have an HTTP mock harness, add a unit test for the parsing logic that feeds a known JSON string into the `Modified` branch:

```rust
#[test]
fn conditional_response_modified_holds_data_and_etag() {
    // Simulate what fetch_pull_request_conditional returns on 200.
    // The function itself is integration-tested via the prefetch path.
    // Note: VALID_PR does not include "draft" — add it before using here.
    let json = r#"{
        "number": 42, "title": "Fix bug in parser",
        "body": "This fixes the parser bug.", "state": "open",
        "user": { "login": "alice" }, "html_url": "https://github.com/owner/repo/pull/42",
        "head": { "sha": "abc123" }, "additions": 10, "deletions": 3, "changed_files": 2,
        "draft": false
    }"#;
    let pr: GithubPullRequest = serde_json::from_str(json).unwrap();
    let resp: ConditionalResponse<GithubPullRequest> = ConditionalResponse::Modified {
        data: pr,
        etag: Some("\"abc\"".to_string()),
    };
    match resp {
        ConditionalResponse::Modified { data, etag } => {
            assert_eq!(data.number, 42);
            assert_eq!(etag.as_deref(), Some("\"abc\""));
        }
        ConditionalResponse::NotModified => panic!("expected Modified"),
    }
}
```

- [ ] **Run test to verify it fails**

```bash
cargo test conditional_response 2>&1 | grep -E "FAILED|error"
```
Expected: compile error (`ConditionalResponse` not defined).

- [ ] **Add `ConditionalResponse` to `src/github/mod.rs`**

Also add `use super::ConditionalResponse;` inside the existing `#[cfg(test)] mod tests { ... }` block in `pull_requests.rs` so the test compiles.

Add after the `github_request` function in `mod.rs`:

```rust
/// Result of a conditional HTTP request using `If-None-Match`.
pub enum ConditionalResponse<T> {
    /// Server returned 304 — resource unchanged, use cached data.
    NotModified,
    /// Server returned 200 — fresh data and (optionally) a new ETag.
    Modified { data: T, etag: Option<String> },
}
```

- [ ] **Add `fetch_pull_request_conditional` to `src/github/pull_requests.rs`**

Add after the existing `fetch_pull_request` function. Update the top-level `use` to include `ConditionalResponse`:

```rust
use super::ConditionalResponse;

pub async fn fetch_pull_request_conditional(
    token: &str,
    client: &reqwest::Client,
    base_url: &str,
    owner: &str,
    repo: &str,
    number: i64,
    etag: Option<&str>,
) -> Result<ConditionalResponse<GithubPullRequest>, reqwest::Error> {
    let url = format!("{base_url}/repos/{owner}/{repo}/pulls/{number}");
    let mut builder = super::github_request(client, token, &url);
    if let Some(tag) = etag {
        builder = builder.header("If-None-Match", tag);
    }
    let response = builder.send().await?;
    if response.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(ConditionalResponse::NotModified);
    }
    let response = response.error_for_status()?;
    let etag = response
        .headers()
        .get("etag")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let data = response.json::<GithubPullRequest>().await?;
    Ok(ConditionalResponse::Modified { data, etag })
}
```

Export from `src/github/mod.rs`:

```rust
pub use pull_requests::{
    fetch_issue_comments, fetch_pull_request, fetch_pull_request_conditional, fetch_review_comments,
};
```

- [ ] **Run tests to verify they pass**

```bash
cargo test conditional_response 2>&1 | grep -E "ok|FAILED"
cargo test 2>&1 | tail -5
```
Expected: all pass.

- [ ] **Commit**

```bash
git add src/github/mod.rs src/github/pull_requests.rs
git commit -m "feat: add ConditionalResponse and fetch_pull_request_conditional"
```

---

## Chunk 2: Prefetch path

### Task 4: New `fetch_and_cache_pr_meta` — light single-call fetch

**Files:**
- Modify: `src/api/pull_requests/fetch.rs`

The `fetch_and_cache_pr_meta` function requires live HTTP so it isn't unit-tested directly. `derive_pr_status` (which it calls) already has implicit coverage — skip adding redundant tests here and proceed to implementation.

- [ ] **Add `fetch_and_cache_pr_meta`** to `fetch.rs`, after the existing `fetch_and_cache_pr` function

Add the required import at the top of the file:
```rust
use crate::github::ConditionalResponse;
```

Then add the function:

```rust
/// Fetch only PR metadata (1 GitHub API call) for inbox row enrichment.
/// Uses ETag conditional requests — 304 responses cost no rate limit quota.
/// No time-based throttle; ETags handle freshness naturally.
pub async fn fetch_and_cache_pr_meta(
    pool: &SqlitePool,
    client: &reqwest::Client,
    token: &str,
    base_url: &str,
    owner: &str,
    repo: &str,
    number: i64,
) -> Result<Option<PrFetchResult>, AppError> {
    let full_repo = format!("{owner}/{repo}");
    let etag_key = format!("pr-meta:{full_repo}#{number}");

    let stored_etag = queries::get_etag(pool, &etag_key).await?;

    let response = github::fetch_pull_request_conditional(
        token,
        client,
        base_url,
        owner,
        repo,
        number,
        stored_etag.as_deref(),
    )
    .await?;

    match response {
        ConditionalResponse::NotModified => {
            // Nothing changed — read current data from DB cache.
            match queries::get_pull_request(pool, &full_repo, number).await? {
                Some(pr) => Ok(Some(PrFetchResult {
                    author: pr.author.clone(),
                    pr_status: derive_pr_status_from_row(&pr),
                })),
                None => Ok(None),
            }
        }
        ConditionalResponse::Modified { data, etag } => {
            let author = data.user.login.clone();
            let pr_status =
                derive_pr_status(data.merged_at.as_deref(), &data.state, data.draft);

            let pr_row = PullRequestRow {
                id: data.number,
                title: data.title,
                repo: full_repo.clone(),
                author: author.clone(),
                url: data.html_url,
                ci_status: None,
                last_viewed_at: None,
                body: data.body.unwrap_or_default(),
                state: data.state,
                head_sha: data.head.sha,
                additions: data.additions.unwrap_or(0),
                deletions: data.deletions.unwrap_or(0),
                changed_files: data.changed_files.unwrap_or(0),
                draft: data.draft,
                merged_at: data.merged_at,
                teams: None,
            };
            queries::upsert_pull_request(pool, &pr_row).await?;
            queries::set_fetch_state(pool, &etag_key, etag.as_deref()).await?;

            Ok(Some(PrFetchResult { author, pr_status }))
        }
    }
}
```

`src/api/pull_requests/mod.rs` declares `pub(crate) mod fetch;` — no export change needed. The function is accessible as `crate::api::pull_requests::fetch::fetch_and_cache_pr_meta`.

- [ ] **Run `cargo check`** to catch compile errors before running tests:

```bash
cargo check 2>&1 | grep -E "error|warning" | head -20
```

- [ ] **Run full test suite**

```bash
cargo test 2>&1 | tail -5
```
Expected: all pass.

- [ ] **Commit**

```bash
git add src/api/pull_requests/fetch.rs
git commit -m "feat: add fetch_and_cache_pr_meta with ETag support"
```

---

### Task 5: Wire `fetch_and_cache_pr_meta` into the prefetch handler

**Files:**
- Modify: `src/api/inbox/prefetch.rs`

The `fetch_one` function currently calls `fetch_and_cache_pr` (5 API calls). Replace it with `fetch_and_cache_pr_meta` (1 API call, ETag-aware). The throttled-fallback branch is no longer needed since `fetch_and_cache_pr_meta` always returns a result (either from GitHub or from the DB cache via 304).

- [ ] **Update `fetch_one` in `src/api/inbox/prefetch.rs`**

Replace the import at the top:
```rust
// Before:
use crate::api::pull_requests::fetch::{derive_pr_status_from_row, fetch_and_cache_pr};
// After:
use crate::api::pull_requests::fetch::fetch_and_cache_pr_meta;
```

Replace the body of `fetch_one` from after `let (owner, repo_name) = ...` to the `tx.send` call:

```rust
    // fetch_and_cache_pr_meta uses ETags — no time-based throttle needed.
    let fetch_result = fetch_and_cache_pr_meta(
        pool,
        client,
        token,
        base_url,
        owner,
        repo_name,
        item.pr_number,
    )
    .await
    .map_err(|e| format!("{e:?}"))?;

    let (author, pr_status) = match fetch_result {
        Some(r) => (r.author, r.pr_status),
        None => return Ok(()), // PR not in DB and 304 received — nothing to broadcast.
    };
```

The rest of the function (activity counts, `tx.send`, team fetch) stays identical.

- [ ] **Run `cargo check`**

```bash
cargo check 2>&1 | grep "error" | head -10
```

- [ ] **Run full test suite**

```bash
cargo test 2>&1 | tail -5
```
Expected: all pass.

- [ ] **Commit**

```bash
git add src/api/inbox/prefetch.rs
git commit -m "perf: use ETag light-fetch (1 call) for inbox prefetch instead of 5-call full fetch"
```

---

### Task 6: Remove now-unused throttle constant

`FETCH_THROTTLE_SECS` in `fetch.rs` is still used by `fetch_and_cache_pr` (the detail view path), so **do not remove it**. However, restore it to a reasonable value since it no longer affects the inbox at all — 60 seconds is fine for the detail view.

**Files:**
- Modify: `src/api/pull_requests/fetch.rs`

- [ ] **Update the constant and its comment**

```rust
/// Minimum seconds between full PR fetches (all 5 endpoints) for the detail view.
/// The inbox prefetch uses ETags instead and has no time-based throttle.
const FETCH_THROTTLE_SECS: i64 = 60;
```

- [ ] **Run `cargo test`**

```bash
cargo test 2>&1 | tail -5
```

- [ ] **Commit**

```bash
git add src/api/pull_requests/fetch.rs
git commit -m "perf: raise detail-view throttle to 60s now that inbox uses ETags"
```
