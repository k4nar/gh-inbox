use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::http::{Method, StatusCode};
use axum::{Router, routing::get};
use http_body_util::BodyExt;
use tokio::net::TcpListener;
use tower::util::ServiceExt;

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
        "created_at": "2025-06-01T11:00:00Z"
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
        "pull_request_review_id": 50
    },
    {
        "id": 2002,
        "user": { "login": "alice" },
        "body": "Done!",
        "created_at": "2025-06-01T13:00:00Z",
        "path": "src/main.rs",
        "position": 10,
        "in_reply_to_id": 2001,
        "pull_request_review_id": 51
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

/// Start a mock GitHub API server that returns canned responses.
async fn start_mock_github() -> String {
    let mock_app = Router::new()
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
        );

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{addr}");

    tokio::spawn(async move {
        axum::serve(listener, mock_app).await.unwrap();
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
    let notifications: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

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
    let notifications: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
    assert!(notifications.is_empty());
}

#[tokio::test]
async fn get_pr_detail_returns_metadata_comments_and_checks() {
    let mock_base_url = start_mock_github().await;
    let pool = gh_inbox::db::init_with_path(":memory:").await;
    let (app, _state) = gh_inbox::app_with_base_url(pool, Arc::from("fake-token"), mock_base_url);

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

    // last_viewed_at should be set
    assert!(detail["pull_request"]["last_viewed_at"].is_string());

    // body_html should be rendered HTML, not raw markdown
    assert!(
        detail["pull_request"]["body_html"].is_string(),
        "body_html should be a string"
    );
    assert!(
        detail["pull_request"]["body_html"]
            .as_str()
            .unwrap()
            .contains("<p>"),
        "body_html should be wrapped in <p>"
    );
    // Each comment should also have body_html
    let comments = detail["comments"].as_array().unwrap();
    if !comments.is_empty() {
        assert!(
            comments[0]["body_html"].is_string(),
            "comment body_html should be a string"
        );
    }
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
    let inbox: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
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
    let archived: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
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
    let inbox: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
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
    let inbox: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
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
