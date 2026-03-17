use axum::extract::{Path, State};
use axum::http::StatusCode;

use crate::api::AppError;
use crate::db::queries;
use crate::github;
use crate::models::{GithubSyncErrorData, SyncEvent};
use crate::server::AppState;

/// POST /api/inbox/:id/archive — archive a notification (local + GitHub mark-as-done).
pub async fn post_archive(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let rows = queries::archive_notification(&state.pool, &id).await?;
    if rows == 0 {
        return Err(AppError::NotFound(format!("notification {id} not found")));
    }

    // Fire-and-forget: push done state to GitHub.
    let token = state.token.clone();
    let client = state.client.clone();
    let base_url = state.github_base_url.clone();
    let tx = state.tx.clone();
    let notification_id = id.clone();
    tokio::spawn(async move {
        if let Err(e) = github::mark_thread_done(&token, &client, &base_url, &notification_id).await
        {
            let _ = tx.send(SyncEvent::GithubSyncError(GithubSyncErrorData {
                notification_id,
                message: e.to_string(),
            }));
        }
    });

    Ok(StatusCode::NO_CONTENT)
}
