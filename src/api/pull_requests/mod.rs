mod get;

use axum::Router;
use axum::routing::get;

use crate::server::AppState;

pub use get::{CheckRunResponse, PrDetailResponse, ThreadResponse};

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/api/pull-requests/{owner}/{repo}/{number}",
        get(get::get_pr),
    )
}
