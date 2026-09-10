//! Test-only helpers for mocking the GitHub API.

use axum::Router;
use axum::routing::get;
use tokio::net::TcpListener;

/// Serve a static JSON body at `path` on an ephemeral port; returns the base
/// URL to hand to a `GithubClient`.
pub(crate) async fn serve_json(path: &'static str, body: &'static str) -> String {
    serve_json_with_headers(path, body, &[]).await
}

/// Like `serve_json`, but adds `headers` to the response — for the GitHub
/// response headers the sync logic reads (`x-poll-interval`, `last-modified`).
pub(crate) async fn serve_json_with_headers(
    path: &'static str,
    body: &'static str,
    headers: &[(&'static str, &'static str)],
) -> String {
    let headers = headers.to_vec();
    let app = Router::new().route(
        path,
        get(move || {
            let headers = headers.clone();
            async move {
                let mut builder =
                    axum::http::Response::builder().header("content-type", "application/json");
                for (name, value) in headers {
                    builder = builder.header(name, value);
                }
                builder.body(axum::body::Body::from(body)).unwrap()
            }
        }),
    );
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{addr}")
}
