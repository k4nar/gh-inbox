use axum::http::StatusCode;
use http_body_util::BodyExt;
use tower::util::ServiceExt;

#[tokio::test]
async fn get_root_returns_200() {
    let pool = gh_inbox::db::init_with_path(":memory:").await;
    let app = gh_inbox::app(pool);

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
    let app = gh_inbox::app(pool);

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
