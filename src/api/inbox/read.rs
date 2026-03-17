use axum::extract::{Path, State};
use axum::http::StatusCode;

use crate::api::AppError;
use crate::db::queries;
use crate::github;
use crate::models::{GithubSyncErrorData, SyncEvent};
use crate::server::AppState;

/// POST /api/inbox/:id/read — mark a notification as read (local + GitHub).
pub async fn post_mark_read(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let rows = queries::mark_read(&state.pool, &id).await?;
    if rows == 0 {
        return Err(AppError::NotFound(format!("notification {id} not found")));
    }

    // Fire-and-forget: push read state to GitHub.
    let github = state.github.clone();
    let tx = state.tx.clone();
    let notification_id = id.clone();
    tokio::spawn(async move {
        if let Err(e) = github::mark_thread_read(&github, &notification_id).await {
            let _ = tx.send(SyncEvent::GithubSyncError(GithubSyncErrorData {
                notification_id,
                message: e.to_string(),
            }));
        }
    });

    Ok(StatusCode::NO_CONTENT)
}
