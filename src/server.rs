use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use axum::http::HeaderValue;
#[cfg(not(debug_assertions))]
use axum::http::{StatusCode, header};
#[cfg(not(debug_assertions))]
use axum::response::{IntoResponse, Response};
use axum::{Router, middleware, routing::get};
use rand::Rng;
use sqlx::SqlitePool;
use tokio::sync::{RwLock, broadcast};

use crate::api;
use crate::github;

/// Shared application state available to all handlers.
#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub github: github::GithubClient,
    pub tx: broadcast::Sender<crate::models::SyncEvent>,
    pub bootstrap_done: Arc<AtomicBool>,
    /// PRs currently visible in the frontend viewport: (repository, pr_number).
    /// Updated by POST /api/inbox/prefetch.
    pub viewport_prs: Arc<RwLock<HashSet<(String, i64)>>>,
    /// Per-session random secret injected into index.html and required on all
    /// /api/* requests (except /api/events) as the X-Session-Token header.
    pub session_token: Arc<str>,
}

/// In release mode, the compiled frontend is embedded in the binary.
/// In debug mode, the Vite dev server handles frontend assets.
#[cfg(not(debug_assertions))]
mod embedded {
    use include_dir::{Dir, include_dir};
    pub static FRONTEND_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/frontend/dist");
}

#[cfg(not(debug_assertions))]
fn mime_from_path(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "application/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("ico") => "image/x-icon",
        Some("json") => "application/json",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        _ => "application/octet-stream",
    }
}

/// Serve an embedded file, or fall back to index.html for SPA routing.
/// Paths under /api/ are never served as static files — they should 404 normally.
#[cfg(not(debug_assertions))]
async fn static_file(
    axum::extract::State(state): axum::extract::State<AppState>,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> Response {
    if path.starts_with("api/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    serve_embedded(&path, &state.session_token)
}

#[cfg(not(debug_assertions))]
async fn index(axum::extract::State(state): axum::extract::State<AppState>) -> Response {
    serve_embedded("index.html", &state.session_token)
}

#[cfg(not(debug_assertions))]
fn serve_embedded(path: &str, session_token: &str) -> Response {
    // Try the exact path first, then fall back to index.html for SPA routing.
    let file = embedded::FRONTEND_DIR
        .get_file(path)
        .or_else(|| embedded::FRONTEND_DIR.get_file("index.html"));

    match file {
        Some(f) => {
            let file_path = f.path().to_str().unwrap_or("");
            let mime = mime_from_path(file_path);
            // Vite hashed assets (e.g. index-abc123.js) are immutable.
            // index.html must always be revalidated to pick up new deploys.
            let cache = if file_path == "index.html" || file_path.ends_with(".html") {
                "no-cache"
            } else {
                "public, max-age=31536000, immutable"
            };
            if file_path == "index.html" {
                // Inject the per-session token so the frontend can authenticate
                // its API requests without holding the GitHub PAT.
                let html = String::from_utf8_lossy(f.contents());
                let injected = html.replacen(
                    "</head>",
                    &format!(
                        r#"<meta name="x-session-token" content="{}"></head>"#,
                        session_token
                    ),
                    1,
                );
                return (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, cache)],
                    injected.into_bytes(),
                )
                    .into_response();
            }
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, cache)],
                f.contents(),
            )
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// In debug mode, just return a placeholder — Vite dev server handles the frontend.
#[cfg(debug_assertions)]
async fn index() -> &'static str {
    "gh-inbox works"
}

/// Adds security headers to every response.
///
/// `connect-src 'self'` is the critical one: even if XSS runs inside the app,
/// it cannot exfiltrate data to an external origin.
async fn add_security_headers(
    request: axum::extract::Request,
    next: middleware::Next,
) -> axum::response::Response {
    let mut response = next.run(request).await;
    let h = response.headers_mut();
    h.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    h.insert("x-frame-options", HeaderValue::from_static("DENY"));
    h.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    h.insert(
        "content-security-policy",
        HeaderValue::from_static(
            "default-src 'self'; \
             img-src 'self' https://avatars.githubusercontent.com https://github.com data:; \
             style-src 'self' 'unsafe-inline'; \
             script-src 'self'; \
             connect-src 'self'; \
             frame-ancestors 'none'; \
             base-uri 'self'; \
             form-action 'self'",
        ),
    );
    response
}

/// Rejects /api/* requests (except /api/events) that don't carry the correct
/// per-session token.  This prevents other local processes or browser tabs
/// from calling the API.  Skipped entirely in debug builds because the Vite
/// dev server serves index.html and can't inject the token.
#[allow(unused_variables)]
async fn check_session_token(
    axum::extract::State(state): axum::extract::State<AppState>,
    request: axum::extract::Request,
    next: middleware::Next,
) -> axum::response::Response {
    #[cfg(not(debug_assertions))]
    {
        use axum::response::IntoResponse;
        let path = request.uri().path();
        if path.starts_with("/api/") && path != "/api/events" {
            let provided = request
                .headers()
                .get("x-session-token")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if provided != state.session_token.as_ref() {
                return (axum::http::StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
            }
        }
    }
    next.run(request).await
}

async fn log_request(
    request: axum::extract::Request,
    next: middleware::Next,
) -> axum::response::Response {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let response = next.run(request).await;
    tracing::debug!(method = %method, path, status = response.status().as_u16());
    response
}

pub fn app(pool: SqlitePool, token: Arc<str>) -> (Router, AppState) {
    app_with_base_url(pool, token, github::GITHUB_API_BASE.to_string())
}

pub fn app_with_base_url(
    pool: SqlitePool,
    token: Arc<str>,
    github_base_url: String,
) -> (Router, AppState) {
    let (tx, _rx) = broadcast::channel(64);
    let session_token: Arc<str> = Arc::from(
        rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(32)
            .map(char::from)
            .collect::<String>()
            .as_str(),
    );
    let state = AppState {
        pool,
        github: github::GithubClient::new(token, github_base_url),
        tx,
        bootstrap_done: Arc::new(AtomicBool::new(false)),
        viewport_prs: Arc::new(RwLock::new(HashSet::new())),
        session_token,
    };

    let router = api::router();

    #[cfg(not(debug_assertions))]
    let router = router
        .route("/", get(index))
        .route("/{*path}", get(static_file));

    #[cfg(debug_assertions)]
    let router = router.route("/", get(index));

    (
        router
            .layer(middleware::from_fn_with_state(
                state.clone(),
                check_session_token,
            ))
            .layer(middleware::from_fn(add_security_headers))
            .layer(middleware::from_fn(log_request))
            .with_state(state.clone()),
        state,
    )
}
