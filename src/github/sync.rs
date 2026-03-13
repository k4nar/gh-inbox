use std::time::Duration;

use tokio::sync::broadcast;

use crate::db::queries;
use crate::github;
use crate::models::{SyncEvent, SyncStatusKind};
use crate::server::AppState;

/// Error type for sync operations — avoids a dependency on `api::AppError`.
#[derive(Debug)]
pub enum SyncError {
    GitHub(reqwest::Error),
    Database(sqlx::Error),
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::GitHub(e) => write!(f, "GitHub error: {e}"),
            SyncError::Database(e) => write!(f, "Database error: {e}"),
        }
    }
}

impl From<reqwest::Error> for SyncError {
    fn from(err: reqwest::Error) -> Self {
        SyncError::GitHub(err)
    }
}

impl From<sqlx::Error> for SyncError {
    fn from(err: sqlx::Error) -> Self {
        SyncError::Database(err)
    }
}

/// Fetch notifications from GitHub and upsert into the database.
/// Returns the number of notifications whose `updated_at` changed (i.e. truly new/updated).
pub async fn sync_notifications(state: &AppState) -> Result<usize, SyncError> {
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

    queries::set_last_fetched_now(&state.pool, "notifications").await?;

    Ok(changed)
}

/// Run the background notification sync loop.
/// Fetches notifications immediately, then every `interval` seconds.
/// Sends events to `tx` for SSE clients.
pub async fn run_sync_loop(state: AppState, tx: broadcast::Sender<SyncEvent>) {
    let interval_secs: u64 = std::env::var("GH_INBOX_SYNC_INTERVAL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    let interval = Duration::from_secs(interval_secs);

    loop {
        // Ignore send errors — they just mean no clients are listening
        let _ = tx.send(SyncEvent::SyncStatus {
            status: SyncStatusKind::Started,
        });

        match sync_notifications(&state).await {
            Ok(count) => {
                if count > 0 {
                    let _ = tx.send(SyncEvent::NewNotifications { count });
                }
                let _ = tx.send(SyncEvent::SyncStatus {
                    status: SyncStatusKind::Completed,
                });
            }
            Err(e) => {
                eprintln!("Sync error: {e:?}");
                let _ = tx.send(SyncEvent::SyncStatus {
                    status: SyncStatusKind::Errored {
                        message: format!("{e:?}"),
                    },
                });
            }
        }

        tokio::time::sleep(interval).await;
    }
}
