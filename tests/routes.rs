use std::sync::Arc;

use axum::http::StatusCode;
use axum::{Router, routing::get};
use http_body_util::BodyExt;
use tokio::net::TcpListener;
use tower::util::ServiceExt;

#[tokio::test]
async fn get_root_returns_200() {
    let pool = gh_inbox::db::init_with_path(":memory:").await;
    let app = gh_inbox::app(pool, Arc::from("fake-token"));

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
    let app = gh_inbox::app(pool, Arc::from("fake-token"));

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

/// Start a mock GitHub API server that returns canned notification JSON.
async fn start_mock_github() -> String {
    let mock_response = r#"[
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

    let mock_app = Router::new().route(
        "/notifications",
        get(move || async move { ([("content-type", "application/json")], mock_response) }),
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
    let app = gh_inbox::app_with_base_url(pool, Arc::from("fake-token"), mock_base_url);

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
    let app = gh_inbox::app_with_base_url(pool, Arc::from("fake-token"), mock_base_url);

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
