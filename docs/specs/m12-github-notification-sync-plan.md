# M12 — GitHub Notification State Sync Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When a user marks a notification as read or archives it in gh-inbox, push the state change to GitHub's API as a fire-and-forget background task; surface failures via SSE toast.

**Architecture:** Two new HTTP helpers (`github_patch`, `github_delete`) are added to `src/github/mod.rs` following the existing `github_request` pattern. Two new async functions in `src/github/notifications.rs` call them and handle 403/404 as no-ops. The action handlers spawn `tokio::spawn` fire-and-forget tasks; on error they broadcast a new `SyncEvent::GithubSyncError` variant through the existing broadcast channel. The frontend SSE utility gains an `onGithubSyncError` registration function; `App.svelte` registers it in `onMount` and calls the existing `showError()`.

**Tech Stack:** Rust/axum/tokio/reqwest, Svelte 5, existing `toast.svelte.ts`/`sse.svelte.ts` infrastructure.

---

## File Map

| File | Action | Responsibility |
|---|---|---|
| `src/github/mod.rs` | Modify | Add `github_patch`, `github_delete` request builders |
| `src/github/notifications.rs` | Modify | Add `mark_thread_read`, `mark_thread_done` with 403/404 no-op logic |
| `src/models/sync_event.rs` | Modify | Add `GithubSyncErrorData` struct + `GithubSyncError(GithubSyncErrorData)` variant |
| `src/api/events.rs` | Modify | Add `GithubSyncError` match arm |
| `src/api/inbox/read.rs` | Modify | Spawn background GitHub call after local update |
| `src/api/inbox/archive.rs` | Modify | Spawn background GitHub call after local update |
| `tests/routes.rs` | Modify | Integration tests for fire-and-forget + SSE error broadcast |
| `frontend/src/lib/sse.svelte.ts` | Modify | Add `onGithubSyncError` registration function + `github:sync_error` listener |
| `frontend/src/lib/sse.test.ts` | Modify | Test `onGithubSyncError` fires on event |
| `frontend/src/App.svelte` | Modify | Register `onGithubSyncError` in `onMount`, call `showError()` |

---

## Task 1: Add `github_patch` and `github_delete` helpers

**Files:**
- Modify: `src/github/mod.rs`

These helpers mirror `github_request` exactly but use `client.patch()` and `client.delete()` instead of `client.get()`. They return `RequestBuilder` — callers chain `.send()` themselves.

- [ ] **Step 1: Add `github_patch` and `github_delete` to `src/github/mod.rs`**

  After the existing `github_request` function (line 16), add:

  ```rust
  fn github_patch(client: &reqwest::Client, token: &str, url: &str) -> reqwest::RequestBuilder {
      client
          .patch(url)
          .header("Authorization", format!("Bearer {token}"))
          .header("Accept", "application/vnd.github+json")
          .header("User-Agent", "gh-inbox")
          .header("X-GitHub-Api-Version", "2026-03-10")
  }

  fn github_delete(client: &reqwest::Client, token: &str, url: &str) -> reqwest::RequestBuilder {
      client
          .delete(url)
          .header("Authorization", format!("Bearer {token}"))
          .header("Accept", "application/vnd.github+json")
          .header("User-Agent", "gh-inbox")
          .header("X-GitHub-Api-Version", "2026-03-10")
  }
  ```

- [ ] **Step 2: Verify it compiles**

  ```bash
  cargo check
  ```
  Expected: no errors.

- [ ] **Step 3: Commit**

  ```bash
  git add src/github/mod.rs
  git commit -m "feat: add github_patch and github_delete request builder helpers"
  ```

---

## Task 2: Add `mark_thread_read` and `mark_thread_done` with unit tests

**Files:**
- Modify: `src/github/notifications.rs`

These functions inspect the response status before deciding whether to error. 403 and 404 are silent no-ops. Any other non-2xx returns `Err`. They do NOT call `error_for_status()` directly.

- [ ] **Step 1: Write the failing unit tests**

  At the bottom of `src/github/notifications.rs`, add a new `#[cfg(test)]` module (or extend the existing one) with these tests:

  ```rust
  #[cfg(test)]
  mod action_tests {
      use super::*;
      use axum::Router;
      use axum::extract::Request;
      use axum::routing::{delete, patch};
      use tokio::net::TcpListener;
      use std::sync::{Arc, Mutex};

      /// Start a mock server that records the last received request method,
      /// path, and selected headers, then returns the given status code.
      async fn start_mock_recording(
          status: u16,
          route_method: &'static str,
      ) -> (String, Arc<Mutex<Option<(String, String, std::collections::HashMap<String, String>)>>>) {
          let recorded = Arc::new(Mutex::new(None::<(String, String, std::collections::HashMap<String, String>)>));
          let recorded_clone = recorded.clone();

          let handler = move |req: Request| {
              let recorded = recorded_clone.clone();
              async move {
                  let method = req.method().to_string();
                  let path = req.uri().path().to_string();
                  let headers: std::collections::HashMap<String, String> = req
                      .headers()
                      .iter()
                      .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                      .collect();
                  *recorded.lock().unwrap() = Some((method, path, headers));
                  axum::http::Response::builder()
                      .status(status)
                      .body(axum::body::Body::empty())
                      .unwrap()
              }
          };

          let app = match route_method {
              "PATCH" => Router::new().route("/notifications/threads/42", patch(handler)),
              "DELETE" => Router::new().route("/notifications/threads/42", delete(handler)),
              _ => panic!("unsupported method"),
          };
          let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
          let addr = listener.local_addr().unwrap();
          let base = format!("http://{addr}");
          tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
          (base, recorded)
      }

      #[tokio::test]
      async fn mark_thread_read_uses_patch_with_correct_headers() {
          let (base, recorded) = start_mock_recording(205, "PATCH").await;
          let client = reqwest::Client::new();
          mark_thread_read("test-token", &client, &base, "42").await.unwrap();
          let (method, path, headers) = recorded.lock().unwrap().clone().unwrap();
          assert_eq!(method, "PATCH");
          assert_eq!(path, "/notifications/threads/42");
          assert_eq!(headers["authorization"], "Bearer test-token");
          assert_eq!(headers["accept"], "application/vnd.github+json");
          assert_eq!(headers["user-agent"], "gh-inbox");
          assert!(headers.contains_key("x-github-api-version"));
      }

      #[tokio::test]
      async fn mark_thread_done_uses_delete_with_correct_headers() {
          let (base, recorded) = start_mock_recording(205, "DELETE").await;
          let client = reqwest::Client::new();
          mark_thread_done("test-token", &client, &base, "42").await.unwrap();
          let (method, path, headers) = recorded.lock().unwrap().clone().unwrap();
          assert_eq!(method, "DELETE");
          assert_eq!(path, "/notifications/threads/42");
          assert_eq!(headers["authorization"], "Bearer test-token");
          assert_eq!(headers["accept"], "application/vnd.github+json");
          assert_eq!(headers["user-agent"], "gh-inbox");
          assert!(headers.contains_key("x-github-api-version"));
      }

      #[tokio::test]
      async fn mark_thread_read_succeeds_on_205() {
          let (base, _) = start_mock_recording(205, "PATCH").await;
          let client = reqwest::Client::new();
          let result = mark_thread_read("tok", &client, &base, "42").await;
          assert!(result.is_ok());
      }

      #[tokio::test]
      async fn mark_thread_read_noops_on_404() {
          let (base, _) = start_mock_recording(404, "PATCH").await;
          let client = reqwest::Client::new();
          let result = mark_thread_read("tok", &client, &base, "42").await;
          assert!(result.is_ok());
      }

      #[tokio::test]
      async fn mark_thread_read_noops_on_403() {
          let (base, _) = start_mock_recording(403, "PATCH").await;
          let client = reqwest::Client::new();
          let result = mark_thread_read("tok", &client, &base, "42").await;
          assert!(result.is_ok());
      }

      #[tokio::test]
      async fn mark_thread_read_errors_on_500() {
          let (base, _) = start_mock_recording(500, "PATCH").await;
          let client = reqwest::Client::new();
          let result = mark_thread_read("tok", &client, &base, "42").await;
          assert!(result.is_err());
      }

      #[tokio::test]
      async fn mark_thread_done_succeeds_on_205() {
          let (base, _) = start_mock_recording(205, "DELETE").await;
          let client = reqwest::Client::new();
          let result = mark_thread_done("tok", &client, &base, "42").await;
          assert!(result.is_ok());
      }

      #[tokio::test]
      async fn mark_thread_done_noops_on_404() {
          let (base, _) = start_mock_recording(404, "DELETE").await;
          let client = reqwest::Client::new();
          let result = mark_thread_done("tok", &client, &base, "42").await;
          assert!(result.is_ok());
      }

      #[tokio::test]
      async fn mark_thread_done_noops_on_403() {
          let (base, _) = start_mock_recording(403, "DELETE").await;
          let client = reqwest::Client::new();
          let result = mark_thread_done("tok", &client, &base, "42").await;
          assert!(result.is_ok());
      }

      #[tokio::test]
      async fn mark_thread_done_errors_on_500() {
          let (base, _) = start_mock_recording(500, "DELETE").await;
          let client = reqwest::Client::new();
          let result = mark_thread_done("tok", &client, &base, "42").await;
          assert!(result.is_err());
      }
  }
  ```

- [ ] **Step 2: Run tests to verify they fail**

  ```bash
  cargo test action_tests
  ```
  Expected: compilation error — `mark_thread_read` and `mark_thread_done` are not yet defined.

- [ ] **Step 3: Implement `mark_thread_read` and `mark_thread_done`**

  Add these two `pub async` functions to `src/github/notifications.rs` (above the existing `#[cfg(test)]` block):

  ```rust
  pub async fn mark_thread_read(
      token: &str,
      client: &reqwest::Client,
      base_url: &str,
      thread_id: &str,
  ) -> Result<(), reqwest::Error> {
      let url = format!("{base_url}/notifications/threads/{thread_id}");
      let response = super::github_patch(client, token, &url).send().await?;
      let status = response.status();
      if status == 403 || status == 404 {
          return Ok(());
      }
      response.error_for_status()?;
      Ok(())
  }

  pub async fn mark_thread_done(
      token: &str,
      client: &reqwest::Client,
      base_url: &str,
      thread_id: &str,
  ) -> Result<(), reqwest::Error> {
      let url = format!("{base_url}/notifications/threads/{thread_id}");
      let response = super::github_delete(client, token, &url).send().await?;
      let status = response.status();
      if status == 403 || status == 404 {
          return Ok(());
      }
      response.error_for_status()?;
      Ok(())
  }
  ```

  Also add the pub re-exports in `src/github/mod.rs`:
  ```rust
  pub use notifications::{fetch_notifications, mark_thread_done, mark_thread_read};
  ```

- [ ] **Step 4: Run tests to verify they pass**

  ```bash
  cargo test action_tests
  ```
  Expected: all 8 tests pass.

- [ ] **Step 5: Run full test suite**

  ```bash
  cargo test
  ```
  Expected: all tests pass.

- [ ] **Step 6: Commit**

  ```bash
  git add src/github/notifications.rs src/github/mod.rs
  git commit -m "feat: add mark_thread_read and mark_thread_done github functions"
  ```

---

## Task 3: Add `GithubSyncError` variant to `SyncEvent`

**Files:**
- Modify: `src/models/sync_event.rs`
- Modify: `src/api/events.rs`

The pattern exactly mirrors `PrTeamsUpdated(PrTeamsUpdatedData)`: a tuple variant wrapping a serializable data struct. The SSE event type is `github:sync_error`.

- [ ] **Step 1: Add `GithubSyncErrorData` and the variant to `src/models/sync_event.rs`**

  Add after the `PrInfoUpdatedData` struct:

  ```rust
  /// Payload serialized into the SSE `data:` field for GithubSyncError.
  #[derive(Debug, Clone, Serialize)]
  pub struct GithubSyncErrorData {
      pub notification_id: String,
      pub message: String,
  }
  ```

  Add to the `SyncEvent` enum:
  ```rust
  GithubSyncError(GithubSyncErrorData),
  ```

- [ ] **Step 2: Add the match arm in `src/api/events.rs`**

  In the `filter_map` match block, after the `PrInfoUpdated` arm, add:

  ```rust
  SyncEvent::GithubSyncError(data) => {
      let payload = serde_json::to_string(data)
          .expect("serialization of GithubSyncErrorData cannot fail");
      ("github:sync_error", payload)
  }
  ```

  The `GithubSyncErrorData` struct is accessed via the variant's tuple field — no explicit import is needed in `events.rs`. The existing import line does not need to change.

- [ ] **Step 3: Verify it compiles**

  ```bash
  cargo check
  ```
  Expected: no errors.

- [ ] **Step 4: Run tests**

  ```bash
  cargo test
  ```
  Expected: all tests pass.

- [ ] **Step 5: Commit**

  ```bash
  git add src/models/sync_event.rs src/api/events.rs
  git commit -m "feat: add GithubSyncError SSE event variant"
  ```

---

## Task 4: Wire GitHub sync into action handlers + integration tests

**Files:**
- Modify: `src/api/inbox/read.rs`
- Modify: `src/api/inbox/archive.rs`
- Modify: `tests/routes.rs`

The handler updates SQLite (unchanged), spawns a fire-and-forget task, and returns immediately. The task calls GitHub; on error it broadcasts `GithubSyncError`. The `JoinHandle` is dropped.

- [ ] **Step 0: Extract `start_mock_github_router()` from the existing `start_mock_github()`**

  The new tests need to extend the mock router with additional routes. The current `start_mock_github()` in `tests/routes.rs` both builds the router and starts the server. Extract the router-building part into a separate helper:

  ```rust
  /// Build the mock GitHub router (without starting the server).
  fn start_mock_github_router() -> Router {
      Router::new()
          .route(
              "/notifications",
              get(|| async { ([("content-type", "application/json")], MOCK_NOTIFICATIONS) }),
          )
          .route(
              "/repos/{owner}/{repo}/pulls/{number}",
              get(|| async { ([("content-type", "application/json")], MOCK_PR) }),
          )
          .route(
              "/repos/{owner}/{repo}/issues/{number}/comments",
              get(|| async { ([("content-type", "application/json")], MOCK_ISSUE_COMMENTS) }),
          )
          .route(
              "/repos/{owner}/{repo}/pulls/{number}/comments",
              get(|| async { ([("content-type", "application/json")], MOCK_REVIEW_COMMENTS) }),
          )
          .route(
              "/repos/{owner}/{repo}/pulls/{number}/commits",
              get(|| async { ([("content-type", "application/json")], MOCK_COMMITS) }),
          )
          .route(
              "/repos/{owner}/{repo}/commits/{sha}/check-runs",
              get(|| async { ([("content-type", "application/json")], MOCK_CHECK_RUNS) }),
          )
  }

  /// Start a mock GitHub API server using the standard router.
  async fn start_mock_github() -> String {
      let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
      let addr = listener.local_addr().unwrap();
      let base_url = format!("http://{addr}");
      tokio::spawn(async move {
          axum::serve(listener, start_mock_github_router()).await.unwrap();
      });
      base_url
  }
  ```

  Copy the exact routes from the existing `start_mock_github()` — verify by looking at the current `tests/routes.rs` to ensure all routes are included. The existing `start_mock_github()` is then replaced by this two-function version.

  Run `cargo test` to confirm nothing broke:
  ```bash
  cargo test
  ```
  Expected: all existing tests pass.

- [ ] **Step 1: Write the failing integration tests**

  In `tests/routes.rs`, add a helper that returns a mock GitHub with a controllable PATCH/DELETE status, and two tests:

  ```rust
  /// Mock GitHub that returns 500 for PATCH /notifications/threads/:id
  /// and 500 for DELETE /notifications/threads/:id
  async fn start_mock_github_with_sync_error() -> String {
      let mock_app = start_mock_github_router()
          .route(
              "/notifications/threads/:id",
              axum::routing::patch(|| async {
                  axum::http::Response::builder()
                      .status(500)
                      .body(axum::body::Body::empty())
                      .unwrap()
              }),
          )
          .route(
              "/notifications/threads/:id",
              axum::routing::delete(|| async {
                  axum::http::Response::builder()
                      .status(500)
                      .body(axum::body::Body::empty())
                      .unwrap()
              }),
          );
      let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
      let addr = listener.local_addr().unwrap();
      let base_url = format!("http://{addr}");
      tokio::spawn(async move { axum::serve(listener, mock_app).await.unwrap() });
      base_url
  }

  #[tokio::test]
  async fn mark_read_broadcasts_github_sync_error_on_github_failure() {
      let mock_base_url = start_mock_github_with_sync_error().await;
      let pool = gh_inbox::db::init_with_path(":memory:").await;

      // Populate DB
      let (app, _) = gh_inbox::app_with_base_url(
          pool.clone(), Arc::from("fake-token"), mock_base_url.clone(),
      );
      let _ = app.oneshot(
          axum::http::Request::builder()
              .uri("/api/inbox")
              .body(axum::body::Body::empty())
              .unwrap(),
      ).await.unwrap();

      // Subscribe to broadcast BEFORE triggering action
      let (app, state) = gh_inbox::app_with_base_url(
          pool.clone(), Arc::from("fake-token"), mock_base_url.clone(),
      );
      let mut rx = state.tx.subscribe();

      let response = app.oneshot(
          axum::http::Request::builder()
              .method(axum::http::Method::POST)
              .uri("/api/inbox/123/read")
              .body(axum::body::Body::empty())
              .unwrap(),
      ).await.unwrap();
      assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);

      // DB should be updated regardless of GitHub failure
      let notifications = gh_inbox::db::queries::query_inbox(&pool).await.unwrap();
      assert!(notifications.iter().any(|n| n.id == "123" && !n.unread));

      // SSE error event should arrive within 500ms
      let event = tokio::time::timeout(
          std::time::Duration::from_millis(500),
          rx.recv(),
      ).await.expect("timed out waiting for GithubSyncError event")
          .expect("channel closed");

      match event {
          gh_inbox::models::SyncEvent::GithubSyncError(data) => {
              assert_eq!(data.notification_id, "123");
          }
          other => panic!("expected GithubSyncError, got {:?}", other),
      }
  }

  #[tokio::test]
  async fn archive_broadcasts_github_sync_error_on_github_failure() {
      let mock_base_url = start_mock_github_with_sync_error().await;
      let pool = gh_inbox::db::init_with_path(":memory:").await;

      // Populate DB
      let (app, _) = gh_inbox::app_with_base_url(
          pool.clone(), Arc::from("fake-token"), mock_base_url.clone(),
      );
      let _ = app.oneshot(
          axum::http::Request::builder()
              .uri("/api/inbox")
              .body(axum::body::Body::empty())
              .unwrap(),
      ).await.unwrap();

      // Subscribe to broadcast BEFORE triggering action
      let (app, state) = gh_inbox::app_with_base_url(
          pool.clone(), Arc::from("fake-token"), mock_base_url.clone(),
      );
      let mut rx = state.tx.subscribe();

      let response = app.oneshot(
          axum::http::Request::builder()
              .method(axum::http::Method::POST)
              .uri("/api/inbox/123/archive")
              .body(axum::body::Body::empty())
              .unwrap(),
      ).await.unwrap();
      assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);

      // SSE error event should arrive within 500ms
      let event = tokio::time::timeout(
          std::time::Duration::from_millis(500),
          rx.recv(),
      ).await.expect("timed out waiting for GithubSyncError event")
          .expect("channel closed");

      match event {
          gh_inbox::models::SyncEvent::GithubSyncError(data) => {
              assert_eq!(data.notification_id, "123");
          }
          other => panic!("expected GithubSyncError, got {:?}", other),
      }
  }
  ```

  Note: You'll need to refactor `start_mock_github()` in `tests/routes.rs` to extract the router-building part into a `start_mock_github_router()` helper so you can extend it. See the existing `start_mock_github()` for the routes to include.

- [ ] **Step 2: Run tests to verify they fail**

  ```bash
  cargo test mark_read_broadcasts_github_sync_error
  cargo test archive_broadcasts_github_sync_error
  ```
  Expected: compilation errors (functions not yet added to handlers).

- [ ] **Step 3: Update `src/api/inbox/read.rs`**

  Replace the file contents with:

  ```rust
  use axum::extract::{Path, State};
  use axum::http::StatusCode;

  use crate::api::AppError;
  use crate::db::queries;
  use crate::github;
  use crate::models::sync_event::{GithubSyncErrorData, SyncEvent};
  use crate::server::AppState;

  /// POST /api/inbox/:id/read — mark a notification as read (local + GitHub).
  pub async fn post_mark_read(
      State(state): State<AppState>,
      Path(id): Path<String>,
  ) -> Result<StatusCode, AppError> {
      let rows = queries::mark_read(&state.pool, &id).await?;
      if rows == 0 {
          return Err(AppError::NotFound(format!("notification {id} not found")));
      }

      // Fire-and-forget: push read state to GitHub.
      let token = state.token.clone();
      let client = state.client.clone();
      let base_url = state.github_base_url.clone();
      let tx = state.tx.clone();
      let notification_id = id.clone();
      let _ = tokio::spawn(async move {
          if let Err(e) = github::mark_thread_read(&token, &client, &base_url, &notification_id).await {
              let _ = tx.send(SyncEvent::GithubSyncError(GithubSyncErrorData {
                  notification_id: notification_id.clone(),
                  message: e.to_string(),
              }));
          }
      });

      Ok(StatusCode::NO_CONTENT)
  }
  ```

- [ ] **Step 4: Update `src/api/inbox/archive.rs`**

  Replace the file contents with:

  ```rust
  use axum::extract::{Path, State};
  use axum::http::StatusCode;

  use crate::api::AppError;
  use crate::db::queries;
  use crate::github;
  use crate::models::sync_event::{GithubSyncErrorData, SyncEvent};
  use crate::server::AppState;

  /// POST /api/inbox/:id/archive — archive a notification (local + GitHub mark-as-done).
  pub async fn post_archive(
      State(state): State<AppState>,
      Path(id): Path<String>,
  ) -> Result<StatusCode, AppError> {
      let rows = queries::archive_notification(&state.pool, &id).await?;
      if rows == 0 {
          return Err(AppError::NotFound(format!("notification {id} not found")));
      }

      // Fire-and-forget: push done state to GitHub.
      let token = state.token.clone();
      let client = state.client.clone();
      let base_url = state.github_base_url.clone();
      let tx = state.tx.clone();
      let notification_id = id.clone();
      let _ = tokio::spawn(async move {
          if let Err(e) = github::mark_thread_done(&token, &client, &base_url, &notification_id).await {
              let _ = tx.send(SyncEvent::GithubSyncError(GithubSyncErrorData {
                  notification_id: notification_id.clone(),
                  message: e.to_string(),
              }));
          }
      });

      Ok(StatusCode::NO_CONTENT)
  }
  ```

- [ ] **Step 5: Run the new integration tests**

  ```bash
  cargo test mark_read_broadcasts_github_sync_error
  cargo test archive_broadcasts_github_sync_error
  ```
  Expected: both pass.

- [ ] **Step 6: Run full test suite**

  ```bash
  cargo test
  ```
  Expected: all tests pass.

- [ ] **Step 7: Commit**

  ```bash
  git add src/api/inbox/read.rs src/api/inbox/archive.rs tests/routes.rs
  git commit -m "feat: fire-and-forget GitHub sync in mark-read and archive handlers"
  ```

---

## Task 5: Frontend — Add `onGithubSyncError` to SSE utility

**Files:**
- Modify: `frontend/src/lib/sse.svelte.ts`
- Modify: `frontend/src/lib/sse.test.ts`

The pattern exactly mirrors `onPrInfoUpdated`: a module-level callback array, a registration function that pushes and returns an unsubscribe closure, and a `addEventListener` call in `connectSSE`.

- [ ] **Step 1: Write the failing test**

  In `frontend/src/lib/sse.test.ts`, add inside the `describe("SSE utility")` block:

  ```ts
  it("github:sync_error triggers registered callbacks with notificationId and message", async () => {
      const { connectSSE, onGithubSyncError, disconnectSSE } = await import(
          "./sse.svelte.ts"
      );
      connectSSE();
      const callback = vi.fn();
      onGithubSyncError(callback);
      MockEventSource.instance.simulateEvent("github:sync_error", {
          notification_id: "123",
          message: "500 Internal Server Error",
      });
      expect(callback).toHaveBeenCalledOnce();
      expect(callback).toHaveBeenCalledWith("123", "500 Internal Server Error");
      disconnectSSE();
  });

  it("onGithubSyncError unsubscribe removes callback", async () => {
      const { connectSSE, onGithubSyncError, disconnectSSE } = await import(
          "./sse.svelte.ts"
      );
      connectSSE();
      const callback = vi.fn();
      const unsub = onGithubSyncError(callback);
      unsub();
      MockEventSource.instance.simulateEvent("github:sync_error", {
          notification_id: "123",
          message: "err",
      });
      expect(callback).not.toHaveBeenCalled();
      disconnectSSE();
  });
  ```

- [ ] **Step 2: Run tests to verify they fail**

  ```bash
  cd frontend && npm test -- --run sse
  ```
  Expected: FAIL — `onGithubSyncError` is not exported.

- [ ] **Step 3: Implement `onGithubSyncError` in `sse.svelte.ts`**

  Add the callback array after the existing `prInfoUpdatedCallbacks` declaration:

  ```ts
  type GithubSyncErrorCallback = (notificationId: string, message: string) => void;
  let githubSyncErrorCallbacks: GithubSyncErrorCallback[] = [];
  ```

  Add the registration function after `onPrInfoUpdated`:

  ```ts
  export function onGithubSyncError(callback: GithubSyncErrorCallback): () => void {
      githubSyncErrorCallbacks.push(callback);
      return () => {
          githubSyncErrorCallbacks = githubSyncErrorCallbacks.filter(
              (cb) => cb !== callback,
          );
      };
  }
  ```

  Add the event listener in `connectSSE()`, after the `pr:info_updated` listener:

  ```ts
  eventSource.addEventListener("github:sync_error", (e) => {
      const { notification_id, message } = JSON.parse(
          (e as MessageEvent).data,
      ) as { notification_id: string; message: string };
      for (const cb of githubSyncErrorCallbacks) {
          cb(notification_id, message);
      }
  });
  ```

- [ ] **Step 4: Run tests to verify they pass**

  ```bash
  cd frontend && npm test -- --run sse
  ```
  Expected: all SSE tests pass.

- [ ] **Step 5: Commit**

  ```bash
  git add frontend/src/lib/sse.svelte.ts frontend/src/lib/sse.test.ts
  git commit -m "feat: add onGithubSyncError SSE listener"
  ```

---

## Task 6: Frontend — Wire into App.svelte

**Files:**
- Modify: `frontend/src/App.svelte`

`App.svelte` already imports from `sse.svelte.ts` and registers `onNewNotifications` in `onMount`. Add `onGithubSyncError` to that same import and register it alongside, calling `showError()` from the already-imported `toast.svelte.ts`.

- [ ] **Step 1: Update `frontend/src/App.svelte`**

  Update the `sse.svelte.ts` import to include `onGithubSyncError`:

  ```ts
  import {
      connectSSE,
      disconnectSSE,
      getSyncStatus,
      onGithubSyncError,
      onNewNotifications,
  } from "./lib/sse.svelte.ts";
  ```

  Add an import for `showError`:

  ```ts
  import { showError } from "./lib/toast.svelte.ts";
  ```

  Update the `onMount` block to register the new callback:

  ```ts
  onMount(() => {
      connectSSE();
      const unsubNotifications = onNewNotifications(() => {
          refreshKey++;
      });
      const unsubGithubError = onGithubSyncError((_notificationId, _message) => {
          showError("Failed to sync with GitHub");
      });
      return () => {
          unsubNotifications();
          unsubGithubError();
          disconnectSSE();
      };
  });
  ```

- [ ] **Step 2: Run frontend tests**

  ```bash
  cd frontend && npm test -- --run
  ```
  Expected: all tests pass.

- [ ] **Step 3: Run Svelte check**

  ```bash
  cd frontend && npx svelte-check
  ```
  Expected: no type errors.

- [ ] **Step 4: Commit**

  ```bash
  git add frontend/src/App.svelte
  git commit -m "feat: show toast on GitHub sync error via SSE"
  ```

---

## Task 7: Final verification

- [ ] **Step 1: Run full Rust test suite**

  ```bash
  cargo test
  ```
  Expected: all tests pass.

- [ ] **Step 2: Run full frontend test suite**

  ```bash
  cd frontend && npm test -- --run
  ```
  Expected: all tests pass.

- [ ] **Step 3: Run clippy**

  ```bash
  cargo clippy -- -D warnings
  ```
  Expected: no warnings.

- [ ] **Step 4: Update roadmap**

  In `docs/specs/roadmap.md`, add M12 as a completed milestone (all tasks checked off).

- [ ] **Step 5: Final commit**

  ```bash
  git add docs/specs/roadmap.md
  git commit -m "docs: mark M12 complete in roadmap"
  ```
