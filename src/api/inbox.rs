use axum::Json;
use axum::extract::State;

use crate::db::queries::{self, NotificationRow};
use crate::github;
use crate::server::AppState;

use super::AppError;

/// GET /api/inbox — fetch notifications from GitHub, cache in SQLite, return JSON.
pub async fn get_inbox(
    State(state): State<AppState>,
) -> Result<Json<Vec<NotificationRow>>, AppError> {
    let notifications =
        github::fetch_notifications(&state.token, &state.client, &state.github_base_url).await?;

    for notif in &notifications {
        // Extract PR id from the subject URL (e.g. ".../pulls/42" -> 42), default to 0
        let pr_id = notif
            .subject
            .url
            .as_deref()
            .and_then(|url| url.rsplit('/').next())
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(0);

        let row = NotificationRow {
            id: notif.id.clone(),
            pr_id,
            reason: notif.reason.clone(),
            unread: notif.unread,
            archived: false,
            updated_at: notif.updated_at.clone(),
        };

        queries::upsert_notification(&state.pool, &row).await?;
    }

    let inbox = queries::query_inbox(&state.pool).await?;
    Ok(Json(inbox))
}
