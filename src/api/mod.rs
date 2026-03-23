pub mod events;
pub mod inbox;
pub mod preferences;
pub mod pull_requests;

mod error;
pub use error::AppError;

use axum::Router;

use crate::server::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(inbox::router())
        .merge(pull_requests::router())
        .merge(events::router())
        .merge(preferences::router())
}
