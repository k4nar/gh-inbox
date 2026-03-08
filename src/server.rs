use axum::{Router, routing::get};

async fn index() -> &'static str {
    "gh-inbox works"
}

pub fn app() -> Router {
    Router::new().route("/", get(index))
}
