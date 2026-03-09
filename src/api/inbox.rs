use axum::Json;
use axum::extract::State;

use crate::db::queries::{self, NotificationRow};
use crate::github;
use crate::server::AppState;

use super::AppError;

/// Minimum seconds between GitHub API fetches for notifications.
const FETCH_THROTTLE_SECS: i64 = 30;

/// GET /api/inbox — fetch notifications from GitHub, cache in SQLite, return JSON.
pub async fn get_inbox(
    State(state): State<AppState>,
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

    let inbox = queries::query_inbox(&state.pool).await?;
    Ok(Json(inbox))
}
