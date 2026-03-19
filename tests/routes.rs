use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::http::{Method, StatusCode};
use axum::{Router, routing::get};
use http_body_util::BodyExt;
use tokio::net::TcpListener;
use tower::util::ServiceExt;

/// Extract the "items" array from a paginated inbox response body.
fn parse_inbox_items(body: &[u8]) -> Vec<serde_json::Value> {
    let result: serde_json::Value = serde_json::from_slice(body).unwrap();
    result["items"].as_array().unwrap().clone()
}

#[tokio::test]
async fn get_root_returns_200() {
    let pool = gh_inbox::db::init_with_path(":memory:").await;
    let (app, _state) = gh_inbox::app(pool, Arc::from("fake-token"));

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"gh-inbox works");
}

#[tokio::test]
async fn unknown_route_returns_404() {
    let pool = gh_inbox::db::init_with_path(":memory:").await;
    let (app, _state) = gh_inbox::app(pool, Arc::from("fake-token"));

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/nonexistent")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

const MOCK_NOTIFICATIONS: &str = r#"[
    {
        "id": "123",
        "reason": "review_requested",
        "unread": true,
        "updated_at": "2025-06-01T10:00:00Z",
        "subject": {
            "title": "Add feature X",
            "url": "https://api.github.com/repos/owner/repo/pulls/42",
            "type": "PullRequest"
        },
        "repository": {
            "full_name": "owner/repo"
        }
    }
]"#;

const MOCK_PR: &str = r#"{
    "number": 42,
    "title": "Add feature X",
    "body": "This adds feature X.",
    "state": "open",
    "user": { "login": "alice" },
    "html_url": "https://github.com/owner/repo/pull/42",
    "head": { "sha": "abc123def456" },
    "additions": 15,
    "deletions": 3,
    "changed_files": 4
}"#;

const MOCK_ISSUE_COMMENTS: &str = r#"[
    {
        "id": 1001,
        "user": { "login": "bob" },
        "body": "Looks good overall!",
        "created_at": "2025-06-01T11:00:00Z",
        "html_url": "https://github.com/owner/repo/pull/42#issuecomment-1001"
    }
]"#;

const MOCK_REVIEW_COMMENTS: &str = r#"[
    {
        "id": 2001,
        "user": { "login": "carol" },
        "body": "Nit: rename this variable",
        "created_at": "2025-06-01T12:00:00Z",
        "path": "src/main.rs",
        "position": 10,
        "in_reply_to_id": null,
        "pull_request_review_id": 50,
        "html_url": "https://github.com/owner/repo/pull/42#discussion_r2001"
    },
    {
        "id": 2002,
        "user": { "login": "alice" },
        "body": "Done!",
        "created_at": "2025-06-01T13:00:00Z",
        "path": "src/main.rs",
        "position": 10,
        "in_reply_to_id": 2001,
        "pull_request_review_id": 51,
        "html_url": "https://github.com/owner/repo/pull/42#discussion_r2002"
    }
]"#;

const MOCK_COMMITS: &str = r#"[
    {
        "sha": "abc123def456",
        "commit": {
            "message": "Add feature X\n\nDetailed description",
            "author": { "name": "Alice", "date": "2025-06-01T09:00:00Z" }
        }
    },
    {
        "sha": "def789ghi012",
        "commit": {
            "message": "Fix tests",
            "author": { "name": "Alice", "date": "2025-06-01T10:00:00Z" }
        }
    }
]"#;

const MOCK_CHECK_RUNS: &str = r#"{
    "total_count": 2,
    "check_runs": [
        { "id": 1, "name": "CI", "status": "completed", "conclusion": "success" },
        { "id": 2, "name": "Lint", "status": "completed", "conclusion": "success" }
    ]
}"#;

const MOCK_REVIEWS: &str = r#"[
    {
        "id": 9001,
        "user": { "login": "bob" },
        "state": "APPROVED",
        "body": "Looks good to me!",
        "submitted_at": "2025-06-01T14:00:00Z",
        "html_url": "https://github.com/owner/repo/pull/42#pullrequestreview-9001"
    }
]"#;

/// Mock GitHub that returns 500 for PATCH /notifications/threads/:id
/// and 500 for DELETE /notifications/threads/:id
async fn start_mock_github_with_sync_error() -> String {
    let mock_app = start_mock_github_router()
        .route(
            "/notifications/threads/{id}",
            axum::routing::patch(|| async {
                axum::http::Response::builder()
                    .status(500)
                    .body(axum::body::Body::empty())
                    .unwrap()
            }),
        )
        .route(
            "/notifications/threads/{id}",
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

    // First app instance: populate the DB by fetching /api/inbox.
    // We discard its AppState — we don't need to subscribe to its broadcast channel.
    let (app, _) =
        gh_inbox::app_with_base_url(pool.clone(), Arc::from("fake-token"), mock_base_url.clone());
    let _ = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/inbox")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Second app instance: subscribe to its broadcast channel BEFORE triggering the action.
    // The fire-and-forget task spawned inside this app broadcasts to this same channel,
    // so we must subscribe here — not to the first instance's channel — to receive the event.
    let (app, state) =
        gh_inbox::app_with_base_url(pool.clone(), Arc::from("fake-token"), mock_base_url.clone());
    let mut rx = state.tx.subscribe();

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method(axum::http::Method::POST)
                .uri("/api/inbox/123/read")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);

    // DB should be updated regardless of GitHub failure
    let notifications = gh_inbox::db::queries::query_inbox(&pool).await.unwrap();
    assert!(notifications.iter().any(|n| n.id == "123" && !n.unread));

    // SSE error event should arrive within 500ms
    let event = tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv())
        .await
        .expect("timed out waiting for GithubSyncError event")
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

    // First app instance: populate the DB by fetching /api/inbox.
    // We discard its AppState — we don't need to subscribe to its broadcast channel.
    let (app, _) =
        gh_inbox::app_with_base_url(pool.clone(), Arc::from("fake-token"), mock_base_url.clone());
    let _ = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/inbox")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Second app instance: subscribe to its broadcast channel BEFORE triggering the action.
    // The fire-and-forget task spawned inside this app broadcasts to this same channel,
    // so we must subscribe here — not to the first instance's channel — to receive the event.
    let (app, state) =
        gh_inbox::app_with_base_url(pool.clone(), Arc::from("fake-token"), mock_base_url.clone());
    let mut rx = state.tx.subscribe();

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method(axum::http::Method::POST)
                .uri("/api/inbox/123/archive")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::NO_CONTENT);

    // DB should be updated regardless of GitHub failure
    let archived = gh_inbox::db::queries::query_archived(&pool).await.unwrap();
    assert!(archived.iter().any(|n| n.id == "123"));

    // SSE error event should arrive within 500ms
    let event = tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv())
        .await
        .expect("timed out waiting for GithubSyncError event")
        .expect("channel closed");

    match event {
        gh_inbox::models::SyncEvent::GithubSyncError(data) => {
            assert_eq!(data.notification_id, "123");
        }
        other => panic!("expected GithubSyncError, got {:?}", other),
    }
}

/// Build the mock GitHub API router with canned responses.
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
        .route(
            "/repos/{owner}/{repo}/pulls/{number}/reviews",
            get(|| async { ([("content-type", "application/json")], MOCK_REVIEWS) }),
        )
}

/// Start a mock GitHub API server that returns canned responses.
async fn start_mock_github() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{addr}");
    tokio::spawn(async move {
        axum::serve(listener, start_mock_github_router())
            .await
            .unwrap();
    });
    base_url
}

#[tokio::test]
async fn get_api_inbox_returns_notifications() {
    let mock_base_url = start_mock_github().await;
    let pool = gh_inbox::db::init_with_path(":memory:").await;
    let (app, _state) = gh_inbox::app_with_base_url(pool, Arc::from("fake-token"), mock_base_url);

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

#[tokio::test]
async fn get_api_inbox_empty_returns_empty_array() {
    let mock_app = Router::new().route(
        "/notifications",
        get(|| async { ([("content-type", "application/json")], "[]") }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let mock_base_url = format!("http://{addr}");

    tokio::spawn(async move {
        axum::serve(listener, mock_app).await.unwrap();
    });

    let pool = gh_inbox::db::init_with_path(":memory:").await;
    let (app, _state) = gh_inbox::app_with_base_url(pool, Arc::from("fake-token"), mock_base_url);

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
    assert!(notifications.is_empty());
    let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(result["total"], 0);
}

#[tokio::test]
async fn get_pr_detail_returns_metadata_comments_and_checks() {
    let mock_base_url = start_mock_github().await;
    let pool = gh_inbox::db::init_with_path(":memory:").await;
    let (app, _state) =
        gh_inbox::app_with_base_url(pool.clone(), Arc::from("fake-token"), mock_base_url.clone());

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/pull-requests/owner/repo/42")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let detail: serde_json::Value = serde_json::from_slice(&body).unwrap();

    // PR metadata
    assert_eq!(detail["pull_request"]["title"], "Add feature X");
    assert_eq!(detail["pull_request"]["author"], "alice");
    assert_eq!(detail["pull_request"]["state"], "open");
    assert_eq!(detail["pull_request"]["additions"], 15);
    assert_eq!(detail["pull_request"]["body"], "This adds feature X.");

    // Comments (1 issue + 2 review = 3 total)
    let comments = detail["comments"].as_array().unwrap();
    assert_eq!(comments.len(), 3);

    // Commits
    let commits = detail["commits"].as_array().unwrap();
    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0]["sha"], "abc123def456");
    assert_eq!(commits[0]["message"], "Add feature X"); // first line only
    assert_eq!(commits[0]["author"], "Alice");
    assert_eq!(commits[1]["sha"], "def789ghi012");

    // Check runs
    let check_runs = detail["check_runs"].as_array().unwrap();
    assert_eq!(check_runs.len(), 2);
    assert_eq!(check_runs[0]["name"], "CI");
    assert_eq!(check_runs[0]["conclusion"], "success");

    // Reviews
    let reviews = detail["reviews"].as_array().unwrap();
    assert_eq!(reviews.len(), 1);
    assert_eq!(reviews[0]["state"], "APPROVED");
    assert_eq!(reviews[0]["reviewer"], "bob");

    // Labels (empty for this PR since MOCK_PR has no labels field)
    assert!(detail["labels"].is_array(), "labels should be an array");

    // On first visit, previous_viewed_at is null and last_viewed_at is null
    // (we capture last_viewed_at before calling update_last_viewed_at).
    assert!(
        detail["previous_viewed_at"].is_null(),
        "previous_viewed_at should be null on first visit"
    );
    assert!(
        detail["pull_request"]["last_viewed_at"].is_null(),
        "last_viewed_at should be null before any visit is recorded"
    );

    // body_html should be rendered HTML, not raw markdown
    assert!(
        detail["pull_request"]["body_html"].is_string(),
        "body_html should be a string"
    );
    assert!(
        detail["pull_request"]["body_html"]
            .as_str()
            .unwrap()
            .contains("<p>This adds feature X.</p>"),
        "body_html should be rendered as a paragraph"
    );
    // Each comment should also have body_html
    let comments = detail["comments"].as_array().unwrap();
    if !comments.is_empty() {
        assert!(
            comments[0]["body_html"].is_string(),
            "comment body_html should be a string"
        );
    }

    // Second visit: previous_viewed_at should now be a timestamp string.
    let (app2, _state2) =
        gh_inbox::app_with_base_url(pool.clone(), Arc::from("fake-token"), mock_base_url.clone());
    let response2 = app2
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/pull-requests/owner/repo/42")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response2.status(), StatusCode::OK);
    let body2 = response2.into_body().collect().await.unwrap().to_bytes();
    let detail2: serde_json::Value = serde_json::from_slice(&body2).unwrap();
    assert!(
        detail2["previous_viewed_at"].is_string(),
        "previous_viewed_at should be a timestamp on second visit"
    );
}

#[tokio::test]
async fn get_pr_detail_returns_check_runs_from_cache() {
    let mock_base_url = start_mock_github().await;
    let pool = gh_inbox::db::init_with_path(":memory:").await;

    // First request populates the cache
    let (app, _state) =
        gh_inbox::app_with_base_url(pool.clone(), Arc::from("fake-token"), mock_base_url.clone());
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/pull-requests/owner/repo/42")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Second request within throttle window — should still return check runs from DB
    let (app, _state) =
        gh_inbox::app_with_base_url(pool.clone(), Arc::from("fake-token"), mock_base_url.clone());
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/pull-requests/owner/repo/42")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let detail: serde_json::Value = serde_json::from_slice(&body).unwrap();

    let check_runs = detail["check_runs"].as_array().unwrap();
    assert_eq!(
        check_runs.len(),
        2,
        "check runs should be returned from cache"
    );
    assert_eq!(check_runs[0]["name"], "CI");
}

#[tokio::test]
async fn get_pr_threads_groups_comments() {
    let mock_base_url = start_mock_github().await;
    let pool = gh_inbox::db::init_with_path(":memory:").await;
    // First fetch PR detail to populate the DB
    let (app, _state) =
        gh_inbox::app_with_base_url(pool.clone(), Arc::from("fake-token"), mock_base_url.clone());
    let _ = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/pull-requests/owner/repo/42")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Now fetch threads
    let (app, _state) = gh_inbox::app_with_base_url(pool, Arc::from("fake-token"), mock_base_url);
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/pull-requests/owner/repo/42/threads")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let threads: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

    // Should have 2 threads: "conversation" (issue comments) and "review:2001" (review comments)
    assert_eq!(threads.len(), 2);

    // Find the conversation thread
    let conv = threads
        .iter()
        .find(|t| t["thread_id"] == "conversation")
        .unwrap();
    assert_eq!(conv["comments"].as_array().unwrap().len(), 1);
    assert!(conv["path"].is_null());

    // Find the review thread
    let review = threads
        .iter()
        .find(|t| t["thread_id"] == "review:2001")
        .unwrap();
    assert_eq!(review["comments"].as_array().unwrap().len(), 2);
    assert_eq!(review["path"], "src/main.rs");

    // Each comment in each thread should have body_html
    if !threads.is_empty() {
        let first_thread_comments = threads[0]["comments"].as_array().unwrap();
        if !first_thread_comments.is_empty() {
            assert!(
                first_thread_comments[0]["body_html"].is_string(),
                "thread comment body_html should be a string"
            );
        }
    }
}

/// Helper: build app with mock GitHub, fetch /api/inbox to populate DB, return (pool, base_url).
async fn setup_populated_inbox() -> (sqlx::SqlitePool, String) {
    let mock_base_url = start_mock_github().await;
    let pool = gh_inbox::db::init_with_path(":memory:").await;

    // Fetch inbox to populate the DB with the mock notification
    let (app, _state) =
        gh_inbox::app_with_base_url(pool.clone(), Arc::from("fake-token"), mock_base_url.clone());
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

    (pool, mock_base_url)
}

#[tokio::test]
async fn archive_removes_from_inbox_and_appears_in_archived() {
    let (pool, mock_base_url) = setup_populated_inbox().await;

    // Archive notification "123"
    let (app, _state) =
        gh_inbox::app_with_base_url(pool.clone(), Arc::from("fake-token"), mock_base_url.clone());
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method(Method::POST)
                .uri("/api/inbox/123/archive")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Inbox should be empty (throttle prevents re-fetch, so we see cached state)
    let (app, _state) =
        gh_inbox::app_with_base_url(pool.clone(), Arc::from("fake-token"), mock_base_url.clone());
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/inbox")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let inbox = parse_inbox_items(&body);
    assert!(inbox.is_empty());

    // Archived should have the notification
    let (app, _state) = gh_inbox::app_with_base_url(pool, Arc::from("fake-token"), mock_base_url);
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/inbox?status=archived")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let archived = parse_inbox_items(&body);
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0]["id"], "123");
}

#[tokio::test]
async fn unarchive_moves_back_to_inbox() {
    let (pool, mock_base_url) = setup_populated_inbox().await;

    // Archive then unarchive
    let (app, _state) =
        gh_inbox::app_with_base_url(pool.clone(), Arc::from("fake-token"), mock_base_url.clone());
    let _ = app
        .oneshot(
            axum::http::Request::builder()
                .method(Method::POST)
                .uri("/api/inbox/123/archive")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let (app, _state) =
        gh_inbox::app_with_base_url(pool.clone(), Arc::from("fake-token"), mock_base_url.clone());
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method(Method::POST)
                .uri("/api/inbox/123/unarchive")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Should be back in inbox
    let (app, _state) = gh_inbox::app_with_base_url(pool, Arc::from("fake-token"), mock_base_url);
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/inbox")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let inbox = parse_inbox_items(&body);
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0]["id"], "123");
}

#[tokio::test]
async fn mark_read_sets_unread_false() {
    let (pool, mock_base_url) = setup_populated_inbox().await;

    // Mark as read
    let (app, _state) =
        gh_inbox::app_with_base_url(pool.clone(), Arc::from("fake-token"), mock_base_url.clone());
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method(Method::POST)
                .uri("/api/inbox/123/read")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // Verify unread is false
    let (app, _state) = gh_inbox::app_with_base_url(pool, Arc::from("fake-token"), mock_base_url);
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/inbox")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let inbox = parse_inbox_items(&body);
    assert_eq!(inbox.len(), 1);
    assert!(!inbox[0]["unread"].as_bool().unwrap());
}

#[tokio::test]
async fn post_to_nonexistent_notification_returns_404() {
    let pool = gh_inbox::db::init_with_path(":memory:").await;
    let mock_base_url = start_mock_github().await;
    let (app, _state) = gh_inbox::app_with_base_url(pool, Arc::from("fake-token"), mock_base_url);

    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method(Method::POST)
                .uri("/api/inbox/nonexistent/archive")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn sse_receives_sync_events() {
    let mock_base_url = start_mock_github().await;
    let pool = gh_inbox::db::init_with_path(":memory:").await;
    let (router, state) =
        gh_inbox::app_with_base_url(pool, Arc::from("fake-token"), mock_base_url.clone());

    // Start the app as a real server so we can make streaming requests
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    // Connect to SSE endpoint
    let client = reqwest::Client::new();
    let mut response = client
        .get(format!("http://{addr}/api/events"))
        .send()
        .await
        .unwrap();

    // Trigger events by sending directly to the broadcast channel
    use gh_inbox::models::{SyncEvent, SyncStatusKind};
    state
        .tx
        .send(SyncEvent::SyncStatus {
            status: SyncStatusKind::Started,
        })
        .unwrap();
    state
        .tx
        .send(SyncEvent::NewNotifications { count: 3 })
        .unwrap();
    state
        .tx
        .send(SyncEvent::SyncStatus {
            status: SyncStatusKind::Completed,
        })
        .unwrap();

    // Read SSE chunks with a timeout
    let mut events = Vec::new();
    let timeout = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while let Some(chunk) = response.chunk().await.unwrap() {
            let text = String::from_utf8_lossy(&chunk);
            for line in text.lines() {
                if line.starts_with("event:") {
                    events.push(line.trim_start_matches("event:").trim().to_string());
                }
            }
            if events.len() >= 3 {
                break;
            }
        }
    })
    .await;

    assert!(timeout.is_ok(), "Timed out waiting for SSE events");
    assert!(events.contains(&"sync:status".to_string()));
    assert!(events.contains(&"notifications:new".to_string()));
}

#[tokio::test]
async fn get_inbox_is_db_only_after_initial_fetch() {
    let call_count = Arc::new(AtomicUsize::new(0));
    let counter = call_count.clone();

    // Custom mock that counts requests
    let mock_app = Router::new().route(
        "/notifications",
        get(move || {
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                ([("content-type", "application/json")], MOCK_NOTIFICATIONS)
            }
        }),
    );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let mock_base_url = format!("http://{addr}");
    tokio::spawn(async move {
        axum::serve(listener, mock_app).await.unwrap();
    });

    let pool = gh_inbox::db::init_with_path(":memory:").await;

    // First request: should bootstrap (call GitHub once)
    let (app, _state) =
        gh_inbox::app_with_base_url(pool.clone(), Arc::from("fake-token"), mock_base_url.clone());
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
    assert_eq!(call_count.load(Ordering::SeqCst), 1);

    // Second request: should NOT call GitHub (reads from DB)
    let (app, _state) =
        gh_inbox::app_with_base_url(pool.clone(), Arc::from("fake-token"), mock_base_url);
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
    // Still only 1 call — second request was DB-only
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn sse_no_event_when_nothing_changed() {
    let mock_base_url = start_mock_github().await;
    let pool = gh_inbox::db::init_with_path(":memory:").await;
    let (router, state) =
        gh_inbox::app_with_base_url(pool.clone(), Arc::from("fake-token"), mock_base_url.clone());

    // Bootstrap: populate DB with initial data
    let response = router
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/inbox")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    // Sync again — same data, should return 0 changes
    use gh_inbox::github::sync::sync_notifications;
    let changed = sync_notifications(&state).await.unwrap();
    assert_eq!(
        changed, 0,
        "No changes expected when syncing identical data"
    );
}

#[tokio::test]
async fn post_prefetch_returns_202_and_populates_pr_data() {
    let mock_base_url = start_mock_github().await;
    let pool = gh_inbox::db::init_with_path(":memory:").await;

    // Bootstrap inbox so the notification row exists
    let (app, _state) =
        gh_inbox::app_with_base_url(pool.clone(), Arc::from("fake-token"), mock_base_url.clone());
    let r = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/inbox")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);

    // Confirm PR row doesn't exist yet
    assert!(
        gh_inbox::db::queries::get_pull_request(&pool, "owner/repo", 42)
            .await
            .unwrap()
            .is_none()
    );

    // Call prefetch endpoint
    let (app, _state) =
        gh_inbox::app_with_base_url(pool.clone(), Arc::from("fake-token"), mock_base_url.clone());
    let body = serde_json::json!({
        "items": [{ "repository": "owner/repo", "pr_number": 42 }]
    });
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .method(Method::POST)
                .uri("/api/inbox/prefetch")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    // Give the background task time to complete
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // PR row should now be populated
    let pr = gh_inbox::db::queries::get_pull_request(&pool, "owner/repo", 42)
        .await
        .unwrap()
        .expect("PR row should exist after prefetch");
    assert_eq!(pr.author, "alice");
    assert_eq!(pr.state, "open");
}

#[tokio::test]
async fn sse_receives_pr_info_updated_on_prefetch() {
    let mock_base_url = start_mock_github().await;
    let pool = gh_inbox::db::init_with_path(":memory:").await;

    // Bind a real server for SSE streaming (we need a persistent connection)
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (router, state) =
        gh_inbox::app_with_base_url(pool.clone(), Arc::from("fake-token"), mock_base_url.clone());
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    // Connect to SSE first so a receiver is registered before we send
    let client = reqwest::Client::new();
    let mut sse_resp = client
        .get(format!("http://{addr}/api/events"))
        .send()
        .await
        .unwrap();

    // Small delay so the SSE handler has subscribed to the channel
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Trigger a pr:info_updated event via the broadcast channel
    use gh_inbox::models::{PrInfoUpdatedData, SyncEvent};
    state
        .tx
        .send(SyncEvent::PrInfoUpdated(PrInfoUpdatedData {
            pr_id: 42,
            repository: "owner/repo".to_string(),
            author: "alice".to_string(),
            pr_status: gh_inbox::models::PrStatus::Open,
            new_commits: None,
            new_comments: None,
            new_reviews: None,
        }))
        .unwrap();

    // Collect SSE chunks
    let mut events: Vec<String> = Vec::new();
    let timeout = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while let Some(chunk) = sse_resp.chunk().await.unwrap() {
            let text = String::from_utf8_lossy(&chunk);
            for line in text.lines() {
                if line.starts_with("event:") {
                    events.push(line.trim_start_matches("event:").trim().to_string());
                }
            }
            if events.contains(&"pr:info_updated".to_string()) {
                break;
            }
        }
    })
    .await;

    assert!(
        timeout.is_ok(),
        "Timed out waiting for pr:info_updated event"
    );
    assert!(events.contains(&"pr:info_updated".to_string()));
}

#[tokio::test]
async fn get_api_inbox_paginates_results() {
    let mock_base_url = start_mock_github().await;
    let pool = gh_inbox::db::init_with_path(":memory:").await;

    // Insert 5 notifications directly into DB
    for i in 1i64..=5 {
        gh_inbox::db::queries::upsert_notification(
            &pool,
            &gh_inbox::db::queries::NotificationRow {
                id: format!("n{i}"),
                pr_id: Some(i),
                title: format!("PR {i}"),
                repository: "owner/repo".to_string(),
                reason: "review_requested".to_string(),
                unread: true,
                archived: false,
                updated_at: format!("2025-01-0{i}T00:00:00Z"),
            },
        )
        .await
        .unwrap();
    }

    // Mark bootstrap as done (store epoch seconds, matching the INTEGER column schema)
    sqlx::query("INSERT INTO last_fetched_at (resource, fetched_at) VALUES ('notifications', strftime('%s', 'now'))")
        .execute(&pool)
        .await
        .unwrap();

    // Page 1 with per_page=2
    let (app, _state) =
        gh_inbox::app_with_base_url(pool.clone(), Arc::from("fake-token"), mock_base_url.clone());
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/inbox?page=1&per_page=2")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(result["items"].as_array().unwrap().len(), 2);
    assert_eq!(result["total"], 5);
    assert_eq!(result["page"], 1);
    assert_eq!(result["per_page"], 2);

    // Page 3 with per_page=2: 1 item
    let (app, _state) =
        gh_inbox::app_with_base_url(pool.clone(), Arc::from("fake-token"), mock_base_url.clone());
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/inbox?page=3&per_page=2")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(result["items"].as_array().unwrap().len(), 1);
    assert_eq!(result["total"], 5);

    // Page beyond range
    let (app, _state) =
        gh_inbox::app_with_base_url(pool.clone(), Arc::from("fake-token"), mock_base_url.clone());
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/inbox?page=10&per_page=2")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(result["items"].as_array().unwrap().is_empty());
    assert_eq!(result["total"], 5);

    // Default params
    let (app, _state) = gh_inbox::app_with_base_url(pool, Arc::from("fake-token"), mock_base_url);
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/inbox")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(result["items"].as_array().unwrap().len(), 5);
    assert_eq!(result["page"], 1);
    assert_eq!(result["per_page"], 25);
}

#[tokio::test]
async fn get_api_inbox_archived_paginates() {
    let mock_base_url = start_mock_github().await;
    let pool = gh_inbox::db::init_with_path(":memory:").await;

    // Insert 3 archived notifications
    for i in 1..=3 {
        sqlx::query(
            "INSERT INTO notifications (id, pr_id, title, repository, reason, unread, archived, updated_at)
             VALUES (?, ?, ?, 'owner/repo', 'mention', 0, 1, ?)",
        )
        .bind(format!("a{i}"))
        .bind(i as i64)
        .bind(format!("Archived PR {i}"))
        .bind(format!("2025-02-0{i}T00:00:00Z"))
        .execute(&pool)
        .await
        .unwrap();
    }

    // Mark bootstrap as done
    sqlx::query(
        "INSERT INTO last_fetched_at (resource, fetched_at) VALUES ('notifications', strftime('%s', 'now'))",
    )
    .execute(&pool)
    .await
    .unwrap();

    let (app, _state) = gh_inbox::app_with_base_url(pool, Arc::from("fake-token"), mock_base_url);
    let response = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/inbox?status=archived&page=1&per_page=2")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(result["items"].as_array().unwrap().len(), 2);
    assert_eq!(result["total"], 3);
    assert_eq!(result["page"], 1);
    assert_eq!(result["per_page"], 2);
}
