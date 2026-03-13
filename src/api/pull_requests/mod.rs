mod get;
mod threads;

use axum::Router;
use axum::routing::get;

use crate::server::AppState;

pub use get::{CheckRunResponse, PrDetailResponse};
pub use threads::ThreadResponse;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/pull-requests/{owner}/{repo}/{number}",
            get(get::get_pr),
        )
        .route(
            "/api/pull-requests/{owner}/{repo}/{number}/threads",
            get(threads::get_threads),
        )
}
