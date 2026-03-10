use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::Deserialize;

use crate::db::queries::{self, NotificationRow};
use crate::github;
use crate::server::AppState;

use super::AppError;

/// Minimum seconds between GitHub API fetches for notifications.
const FETCH_THROTTLE_SECS: i64 = 30;

#[derive(Deserialize)]
pub struct InboxQuery {
    pub status: Option<String>,
}

/// GET /api/inbox — fetch notifications from GitHub, cache in SQLite, return JSON.
pub async fn get_inbox(
    State(state): State<AppState>,
    Query(query): Query<InboxQuery>,
) -> Result<Json<Vec<NotificationRow>>, AppError> {
    let should_fetch = match queries::get_last_fetched_epoch(&state.pool, "notifications").await? {
        Some(last) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock before UNIX epoch")
                .as_secs() as i64;
            now - last >= FETCH_THROTTLE_SECS
        }
        None => true,
    };

    if should_fetch {
        let notifications =
            github::fetch_notifications(&state.token, &state.client, &state.github_base_url)
                .await?;

        for notif in &notifications {
            // Extract PR id from the subject URL (e.g. ".../pulls/42" -> 42)
            let pr_id = notif
                .subject
                .url
                .as_deref()
                .and_then(|url| url.rsplit('/').next())
                .and_then(|s| s.parse::<i64>().ok());

            let row = NotificationRow {
                id: notif.id.clone(),
                pr_id,
                title: notif.subject.title.clone(),
                repository: notif.repository.full_name.clone(),
                reason: notif.reason.clone(),
                unread: notif.unread,
                archived: false,
                updated_at: notif.updated_at.clone(),
            };

            queries::upsert_notification(&state.pool, &row).await?;
        }

        queries::set_last_fetched_now(&state.pool, "notifications").await?;
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
    if !queries::notification_exists(&state.pool, &id).await? {
        return Err(AppError::NotFound(format!("notification {id} not found")));
    }
    queries::mark_read(&state.pool, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/inbox/:id/archive — archive a notification.
pub async fn post_archive(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    if !queries::notification_exists(&state.pool, &id).await? {
        return Err(AppError::NotFound(format!("notification {id} not found")));
    }
    queries::archive_notification(&state.pool, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST /api/inbox/:id/unarchive — unarchive a notification.
pub async fn post_unarchive(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    if !queries::notification_exists(&state.pool, &id).await? {
        return Err(AppError::NotFound(format!("notification {id} not found")));
    }
    queries::unarchive_notification(&state.pool, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
