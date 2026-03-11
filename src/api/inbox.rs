use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::Deserialize;

use crate::db::queries::{self, NotificationRow};
use crate::github;
use crate::server::AppState;

use super::AppError;

#[derive(Deserialize)]
pub struct InboxQuery {
    pub status: Option<String>,
}

/// Fetch notifications from GitHub and upsert into the database.
/// Returns the number of notifications whose `updated_at` changed (i.e. truly new/updated).
pub async fn sync_notifications(state: &AppState) -> Result<usize, AppError> {
    // Set last_fetched_at BEFORE fetching to prevent race with concurrent bootstrap requests
    queries::set_last_fetched_now(&state.pool, "notifications").await?;

    let notifications =
        github::fetch_notifications(&state.token, &state.client, &state.github_base_url).await?;

    let mut changed = 0;
    for notif in &notifications {
        let pr_id = notif
            .subject
            .url
            .as_deref()
            .and_then(|url| url.rsplit('/').next())
            .and_then(|s| s.parse::<i64>().ok());

        let row = queries::NotificationRow {
            id: notif.id.clone(),
            pr_id,
            title: notif.subject.title.clone(),
            repository: notif.repository.full_name.clone(),
            reason: notif.reason.clone(),
            unread: notif.unread,
            archived: false,
            updated_at: notif.updated_at.clone(),
        };

        let rows_affected = queries::upsert_notification(&state.pool, &row).await?;
        if rows_affected > 0 {
            changed += 1;
        }
    }

    Ok(changed)
}

/// GET /api/inbox — return notifications from SQLite, bootstrapping from GitHub on first call.
pub async fn get_inbox(
    State(state): State<AppState>,
    Query(query): Query<InboxQuery>,
) -> Result<Json<Vec<NotificationRow>>, AppError> {
    // Bootstrap: if notifications have never been fetched, do it once inline.
    // After that, the background sync loop handles fetching.
    let has_fetched = queries::get_last_fetched_epoch(&state.pool, "notifications")
        .await?
        .is_some();

    if !has_fetched {
        sync_notifications(&state).await?;
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
