use std::sync::atomic::Ordering;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::Deserialize;

use crate::db::queries::{self, NotificationRow};
use crate::github::sync::sync_notifications;
use crate::server::AppState;

use super::AppError;

#[derive(Deserialize)]
pub struct InboxQuery {
    pub status: Option<String>,
}

/// GET /api/inbox — return notifications from SQLite, bootstrapping from GitHub on first call.
pub async fn get_inbox(
    State(state): State<AppState>,
    Query(query): Query<InboxQuery>,
) -> Result<Json<Vec<NotificationRow>>, AppError> {
    // Bootstrap: fetch once inline on first request.
    // AtomicBool prevents concurrent bootstrap within the same AppState.
    // last_fetched_at (set after successful sync) prevents re-bootstrap across AppState instances.
    if state
        .bootstrap_done
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        let has_fetched = queries::get_last_fetched_epoch(&state.pool, "notifications")
            .await?
            .is_some();
        if !has_fetched {
            sync_notifications(&state).await?;
        }
    }

    let results = match query.status.as_deref() {
        Some("archived") => queries::query_archived(&state.pool).await?,
        _ => queries::query_inbox(&state.pool).await?,
    };
    Ok(Json(results))
}

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
