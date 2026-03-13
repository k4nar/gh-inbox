use axum::extract::{Path, State};
use axum::http::StatusCode;

use crate::api::AppError;
use crate::db::queries;
use crate::server::AppState;

/// POST /api/inbox/:id/unarchive — unarchive a notification.
pub async fn post_unarchive(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let rows = queries::unarchive_notification(&state.pool, &id).await?;
    if rows == 0 {
        return Err(AppError::NotFound(format!("notification {id} not found")));
    }
    Ok(StatusCode::NO_CONTENT)
}
