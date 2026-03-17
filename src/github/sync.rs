use std::time::Duration;

use tokio::sync::broadcast;

use crate::db::queries;
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
    let notifications = super::fetch_notifications(&state.github).await?;

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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;

    use axum::{Router, routing::get};
    use tokio::net::TcpListener;

    use super::*;
    use crate::db::queries;
    use crate::github::GithubClient;

    async fn make_state(base_url: String) -> AppState {
        let pool = crate::db::init_with_path(":memory:").await;
        let (tx, _rx) = broadcast::channel(8);
        AppState {
            pool,
            github: GithubClient::new(Arc::from("fake-token"), base_url),
            tx,
            bootstrap_done: Arc::new(AtomicBool::new(false)),
        }
    }

    async fn start_mock(response: &'static str) -> String {
        let app = Router::new().route(
            "/notifications",
            get(move || async move { ([("content-type", "application/json")], response) }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        format!("http://{addr}")
    }

    const ONE_NOTIFICATION: &str = r#"[{
        "id": "1",
        "reason": "review_requested",
        "unread": true,
        "updated_at": "2025-01-01T00:00:00Z",
        "subject": {
            "title": "Fix bug",
            "url": "https://api.github.com/repos/owner/repo/pulls/42",
            "type": "PullRequest"
        },
        "repository": { "full_name": "owner/repo" }
    }]"#;

    const NULL_URL_NOTIFICATION: &str = r#"[{
        "id": "2",
        "reason": "mention",
        "unread": false,
        "updated_at": "2025-01-02T00:00:00Z",
        "subject": {
            "title": "Release note",
            "url": null,
            "type": "Release"
        },
        "repository": { "full_name": "owner/repo" }
    }]"#;

    #[tokio::test]
    async fn inserts_notification_and_returns_changed_count() {
        let state = make_state(start_mock(ONE_NOTIFICATION).await).await;

        let changed = sync_notifications(&state).await.unwrap();
        assert_eq!(changed, 1);

        let inbox = queries::query_inbox(&state.pool).await.unwrap();
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].id, "1");
        assert_eq!(inbox[0].reason, "review_requested");
    }

    #[tokio::test]
    async fn idempotent_when_data_unchanged() {
        let state = make_state(start_mock(ONE_NOTIFICATION).await).await;

        assert_eq!(sync_notifications(&state).await.unwrap(), 1);
        assert_eq!(sync_notifications(&state).await.unwrap(), 0);

        assert_eq!(queries::query_inbox(&state.pool).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn pr_id_extracted_from_subject_url() {
        let state = make_state(start_mock(ONE_NOTIFICATION).await).await;
        sync_notifications(&state).await.unwrap();

        let inbox = queries::query_inbox(&state.pool).await.unwrap();
        assert_eq!(inbox[0].pr_id, Some(42));
    }

    #[tokio::test]
    async fn null_subject_url_gives_null_pr_id() {
        let state = make_state(start_mock(NULL_URL_NOTIFICATION).await).await;
        sync_notifications(&state).await.unwrap();

        let inbox = queries::query_inbox(&state.pool).await.unwrap();
        assert_eq!(inbox[0].pr_id, None);
    }
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
