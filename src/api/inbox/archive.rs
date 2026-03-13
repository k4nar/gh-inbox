use axum::extract::{Path, State};
use axum::http::StatusCode;

use crate::api::AppError;
use crate::db::queries;
use crate::server::AppState;

/// POST /api/inbox/:id/archive — archive a notification.
pub async fn post_archive(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let rows = queries::archive_notification(&state.pool, &id).await?;
    if rows == 0 {
        return Err(AppError::NotFound(format!("notification {id} not found")));
    }
    Ok(StatusCode::NO_CONTENT)
}
