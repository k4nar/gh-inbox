use axum::{Router, routing::get};
use sqlx::SqlitePool;

async fn index() -> &'static str {
    "gh-inbox works"
}

pub fn app(pool: SqlitePool) -> Router {
    Router::new().route("/", get(index)).with_state(pool)
}
