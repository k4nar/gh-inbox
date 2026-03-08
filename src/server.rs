use std::sync::Arc;

use axum::{Router, routing::get};
use sqlx::SqlitePool;

/// Shared application state available to all handlers.
#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub token: Arc<str>,
    pub client: reqwest::Client,
}

async fn index() -> &'static str {
    "gh-inbox works"
}

pub fn app(pool: SqlitePool, token: Arc<str>) -> Router {
    let state = AppState {
        pool,
        token,
        client: reqwest::Client::new(),
    };
    Router::new().route("/", get(index)).with_state(state)
}
