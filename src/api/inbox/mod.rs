mod archive;
mod get;
mod prefetch;
mod read;
pub(crate) mod teams;
mod unarchive;

use axum::Router;
use axum::routing::{get, post};

use crate::server::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/inbox", get(get::get_inbox))
        .route("/api/inbox/prefetch", post(prefetch::post_prefetch))
        .route("/api/inbox/{id}/read", post(read::post_mark_read))
        .route("/api/inbox/{id}/archive", post(archive::post_archive))
        .route("/api/inbox/{id}/unarchive", post(unarchive::post_unarchive))
}
