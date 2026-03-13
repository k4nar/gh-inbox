use axum::extract::{Path, State};
use axum::http::StatusCode;

use crate::api::AppError;
use crate::db::queries;
use crate::server::AppState;

/// POST /api/inbox/:id/read — mark a notification as read.
pub async fn post_mark_read(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let rows = queries::mark_read(&state.pool, &id).await?;
    if rows == 0 {
        return Err(AppError::NotFound(format!("notification {id} not found")));
    }
    Ok(StatusCode::NO_CONTENT)
}
