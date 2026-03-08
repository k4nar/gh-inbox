use std::sync::Arc;

use axum::{Router, routing::get};
use sqlx::SqlitePool;

use crate::api;
use crate::github;

/// Shared application state available to all handlers.
#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub token: Arc<str>,
    pub client: reqwest::Client,
    pub github_base_url: String,
}

async fn index() -> &'static str {
    "gh-inbox works"
}

pub fn app(pool: SqlitePool, token: Arc<str>) -> Router {
    app_with_base_url(pool, token, github::GITHUB_API_BASE.to_string())
}

pub fn app_with_base_url(pool: SqlitePool, token: Arc<str>, github_base_url: String) -> Router {
    let state = AppState {
        pool,
        token,
        client: reqwest::Client::new(),
        github_base_url,
    };
    Router::new()
        .route("/", get(index))
        .route("/api/inbox", get(api::inbox::get_inbox))
        .with_state(state)
}
