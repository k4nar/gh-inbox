//! Test-only helpers for mocking the GitHub API.

use axum::Router;
use axum::routing::get;
use tokio::net::TcpListener;

/// Serve a static JSON body at `path` on an ephemeral port; returns the base
/// URL to hand to a `GithubClient`.
pub(crate) async fn serve_json(path: &'static str, body: &'static str) -> String {
    let app = Router::new().route(
        path,
        get(move || async move { ([("content-type", "application/json")], body) }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}")
}
