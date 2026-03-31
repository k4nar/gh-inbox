use std::time::Duration;

use tokio::sync::broadcast;

use crate::api::pull_requests::fetch::{derive_pr_status_from_row, fetch_and_cache_pr};
use crate::db::queries;
use crate::models::{PrInfoUpdatedData, PrNewComment, SyncEvent, SyncStatusKind};
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

const FULL_SYNC_THRESHOLD_SECS: i64 = 2 * 60 * 60; // 2 hours

pub(crate) fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before UNIX epoch")
        .as_secs() as i64
}

/// Convert a Unix epoch (seconds) to an ISO 8601 UTC string suitable for
/// the GitHub API `since` parameter. Avoids adding a time-crate dependency.
fn epoch_to_iso(epoch: i64) -> String {
    let mut rem = epoch as u64;
    let ss = rem % 60;
    rem /= 60;
    let mm = rem % 60;
    rem /= 60;
    let hh = rem % 24;
    rem /= 24;
    let (y, mo, d) = days_since_epoch_to_ymd(rem as u32);
    format!("{y:04}-{mo:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z")
}

fn days_since_epoch_to_ymd(mut days: u32) -> (u32, u32, u32) {
    let mut year = 1970u32;
    loop {
        let diy = if is_leap_year(year) { 366 } else { 365 };
        if days < diy {
            break;
        }
        days -= diy;
        year += 1;
    }
    const MONTH_DAYS: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1u32;
    for (i, &base) in MONTH_DAYS.iter().enumerate() {
        let dim = if i == 1 && is_leap_year(year) {
            29
        } else {
            base
        };
        if days < dim {
            break;
        }
        days -= dim;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap_year(y: u32) -> bool {
    (y.is_multiple_of(4) && !y.is_multiple_of(100)) || y.is_multiple_of(400)
}

/// A notification that changed during sync.
pub struct ChangedNotification {
    pub repository: String,
    pub pr_id: Option<i64>,
}

/// Fetch notifications from GitHub and upsert into the database.
///
/// **Full sync** (first run or last fetch >2h ago): fetches all pages and
/// archives any local notification that GitHub no longer returns.
///
/// **Incremental sync** (recent last fetch): fetches only notifications
/// changed since the last fetch using the `since=` parameter.
///
/// Returns the notifications that changed (upserted or reconciliation-archived).
pub async fn sync_notifications(state: &AppState) -> Result<Vec<ChangedNotification>, SyncError> {
    let last_fetched = queries::get_last_fetched_epoch(&state.pool, "notifications").await?;
    let now = now_epoch();

    let is_full_sync = last_fetched
        .map(|t| now - t > FULL_SYNC_THRESHOLD_SECS)
        .unwrap_or(true);

    let notifications = if is_full_sync {
        super::fetch_all_notifications(&state.github).await?
    } else {
        let since_iso = epoch_to_iso(last_fetched.unwrap());
        super::fetch_notifications_since(&state.github, &since_iso).await?
    };

    let mut changed = Vec::new();
    let mut returned_ids: Vec<String> = Vec::new();

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
            unread: notif.unread && notif.reason != "your_activity",
            archived: false,
            updated_at: notif.updated_at.clone(),
        };

        let rows_affected = queries::upsert_notification(&state.pool, &row).await?;
        if rows_affected > 0 {
            changed.push(ChangedNotification {
                repository: notif.repository.full_name.clone(),
                pr_id,
            });
        }
        returned_ids.push(notif.id.clone());
    }

    // Full sync reconciliation: archive notifications GitHub no longer returns.
    // Guard: skip if returned_ids is empty to avoid archiving everything on an
    // unexpected empty response.
    if is_full_sync && !returned_ids.is_empty() {
        let id_refs: Vec<&str> = returned_ids.iter().map(|s| s.as_str()).collect();
        let archived_count = queries::archive_if_not_in(&state.pool, &id_refs).await?;
        // Push dummy entries so run_sync_loop fires SSE events for reconciled items.
        for _ in 0..archived_count {
            changed.push(ChangedNotification {
                repository: String::new(),
                pr_id: None,
            });
        }
    }

    queries::set_last_fetched_now(&state.pool, "notifications").await?;

    Ok(changed)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::Mutex;

    use axum::Router;
    use axum::extract::Request;
    use axum::routing::get;
    use tokio::net::TcpListener;
    use tokio::sync::broadcast;

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
            viewport_prs: Arc::new(tokio::sync::RwLock::new(std::collections::HashSet::new())),
            session_token: Arc::from("test-session-token"),
            sync_in_progress: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    // ── simple mock (no URL capture) ──────────────────────────────────────

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

    // ── URL-capturing mock ────────────────────────────────────────────────

    async fn start_mock_capturing(response: &'static str) -> (String, Arc<Mutex<Option<String>>>) {
        let captured = Arc::new(Mutex::new(None::<String>));
        let captured_clone = captured.clone();
        let app = Router::new().route(
            "/notifications",
            get(move |req: Request| {
                let cap = captured_clone.clone();
                async move {
                    *cap.lock().unwrap() = Some(req.uri().to_string());
                    axum::http::Response::builder()
                        .header("content-type", "application/json")
                        .body(axum::body::Body::from(response))
                        .unwrap()
                }
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (format!("http://{addr}"), captured)
    }

    // ── notification fixtures ─────────────────────────────────────────────

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

    // ── preserved tests ───────────────────────────────────────────────────

    #[tokio::test]
    async fn inserts_notification_and_returns_changed_count() {
        let state = make_state(start_mock(ONE_NOTIFICATION).await).await;

        let changed = sync_notifications(&state).await.unwrap();
        assert_eq!(changed.len(), 1);

        let inbox = queries::query_inbox(&state.pool).await.unwrap();
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].id, "1");
        assert_eq!(inbox[0].reason, "review_requested");
    }

    #[tokio::test]
    async fn idempotent_when_data_unchanged() {
        let state = make_state(start_mock(ONE_NOTIFICATION).await).await;

        assert_eq!(sync_notifications(&state).await.unwrap().len(), 1);
        // Second call: incremental (recent last_fetched_at), same data → 0 changed
        assert_eq!(sync_notifications(&state).await.unwrap().len(), 0);

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

    // ── new two-mode tests ────────────────────────────────────────────────

    #[tokio::test]
    async fn full_sync_when_never_fetched_does_not_include_since() {
        let (base, captured) = start_mock_capturing(ONE_NOTIFICATION).await;
        let state = make_state(base).await;
        // No last_fetched_at set → full sync

        sync_notifications(&state).await.unwrap();

        let uri = captured.lock().unwrap().clone().unwrap();
        assert!(
            !uri.contains("since="),
            "Full sync should not send since=, got: {uri}"
        );
        assert!(
            !uri.contains("all=true"),
            "Should not use all=true, got: {uri}"
        );
    }

    #[tokio::test]
    async fn full_sync_when_last_fetch_over_2h_does_not_include_since() {
        let (base, captured) = start_mock_capturing(ONE_NOTIFICATION).await;
        let state = make_state(base).await;

        let three_hours_ago = now_epoch() - 3 * 3600;
        sqlx::query(
            "INSERT INTO last_fetched_at (resource, fetched_at) VALUES ('notifications', ?)",
        )
        .bind(three_hours_ago)
        .execute(&state.pool)
        .await
        .unwrap();

        sync_notifications(&state).await.unwrap();

        let uri = captured.lock().unwrap().clone().unwrap();
        assert!(
            !uri.contains("since="),
            "Stale full sync should not send since=, got: {uri}"
        );
    }

    #[tokio::test]
    async fn incremental_sync_when_recently_fetched_includes_since() {
        let (base, captured) = start_mock_capturing(ONE_NOTIFICATION).await;
        let state = make_state(base).await;

        let one_min_ago = now_epoch() - 60;
        sqlx::query(
            "INSERT INTO last_fetched_at (resource, fetched_at) VALUES ('notifications', ?)",
        )
        .bind(one_min_ago)
        .execute(&state.pool)
        .await
        .unwrap();

        sync_notifications(&state).await.unwrap();

        let uri = captured.lock().unwrap().clone().unwrap();
        assert!(
            uri.contains("since="),
            "Incremental sync should send since=, got: {uri}"
        );
    }

    #[tokio::test]
    async fn incremental_sync_since_value_matches_last_fetched_at() {
        let (base, captured) = start_mock_capturing("[]").await;
        let state = make_state(base).await;

        // Use a recent epoch (1 min ago) so incremental sync fires.
        // Compute the expected ISO prefix so the assertion stays correct regardless of date.
        let epoch = now_epoch() - 60;
        let expected_iso = epoch_to_iso(epoch);
        // Only check the date portion (YYYY-MM-DD) to avoid sub-second jitter.
        let expected_date = &expected_iso[..10];

        sqlx::query(
            "INSERT INTO last_fetched_at (resource, fetched_at) VALUES ('notifications', ?)",
        )
        .bind(epoch)
        .execute(&state.pool)
        .await
        .unwrap();

        sync_notifications(&state).await.unwrap();

        let uri = captured.lock().unwrap().clone().unwrap();
        assert!(
            uri.contains(&format!("since={expected_date}")),
            "URI should contain since={expected_date}, got: {uri}"
        );
    }

    #[tokio::test]
    async fn own_activity_notification_does_not_change_unread() {
        let your_activity_notification = r#"[{
            "id": "1",
            "reason": "your_activity",
            "unread": true,
            "updated_at": "2025-01-01T00:00:00Z",
            "subject": {
                "title": "Fix bug",
                "url": "https://api.github.com/repos/owner/repo/pulls/42",
                "type": "PullRequest"
            },
            "repository": { "full_name": "owner/repo" }
        }]"#;
        let state = make_state(start_mock(your_activity_notification).await).await;

        // Pre-seed a read row with older updated_at so the WHERE clause fires on sync
        sqlx::query(
            "INSERT INTO notifications (id, pr_id, title, repository, reason, unread, archived, updated_at)
             VALUES ('1', 42, 'Fix bug', 'owner/repo', 'review_requested', 0, 0, '2024-12-31T00:00:00Z')",
        )
        .execute(&state.pool)
        .await
        .unwrap();

        sync_notifications(&state).await.unwrap();

        let rows = queries::query_inbox(&state.pool).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert!(
            !rows[0].unread,
            "own_activity should not mark the notification unread"
        );
    }

    #[tokio::test]
    async fn full_sync_archives_notifications_missing_from_github() {
        // id="1" is in the mock response; id="99" is not → should be archived
        let state = make_state(start_mock(ONE_NOTIFICATION).await).await;

        // Pre-insert two notifications
        queries::upsert_notification(
            &state.pool,
            &queries::NotificationRow {
                id: "1".to_string(),
                pr_id: Some(42),
                title: "Fix bug".to_string(),
                repository: "owner/repo".to_string(),
                reason: "review_requested".to_string(),
                unread: true,
                archived: false,
                updated_at: "2025-01-01T00:00:00Z".to_string(),
            },
        )
        .await
        .unwrap();
        queries::upsert_notification(
            &state.pool,
            &queries::NotificationRow {
                id: "99".to_string(),
                pr_id: None,
                title: "Gone".to_string(),
                repository: "owner/repo".to_string(),
                reason: "mention".to_string(),
                unread: true,
                archived: false,
                updated_at: "2025-01-01T00:00:00Z".to_string(),
            },
        )
        .await
        .unwrap();

        // Full sync (no last_fetched_at) — GitHub returns only id="1"
        sync_notifications(&state).await.unwrap();

        let archived = queries::query_archived(&state.pool).await.unwrap();
        assert_eq!(archived.len(), 1, "id=99 should be archived");
        assert_eq!(archived[0].id, "99");

        let inbox = queries::query_inbox(&state.pool).await.unwrap();
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].id, "1");
    }
}

/// Run the background notification sync loop.
/// Fetches notifications immediately, then every `interval` seconds.
/// Sends events to `tx` for SSE clients.
/// When notifications change for PRs in the viewport, auto-fetches their data.
pub async fn run_sync_loop(state: AppState, tx: broadcast::Sender<SyncEvent>) {
    let interval_secs: u64 = std::env::var("GH_INBOX_SYNC_INTERVAL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);
    let interval = Duration::from_secs(interval_secs);

    // Always start with a full sync so that any state accumulated while the
    // service was stopped (notifications cleared on GitHub, etc.) is reconciled
    // immediately on startup rather than waiting up to 2h.
    let _ = sqlx::query("DELETE FROM last_fetched_at WHERE resource = 'notifications'")
        .execute(&state.pool)
        .await;

    loop {
        // Ignore send errors — they just mean no clients are listening
        let _ = tx.send(SyncEvent::SyncStatus {
            status: SyncStatusKind::Started,
        });

        match sync_notifications(&state).await {
            Ok(changed) => {
                let count = changed.len();
                if count > 0 {
                    tracing::info!(count, "new notifications fetched");
                    let _ = tx.send(SyncEvent::NewNotifications { count });

                    // Auto-fetch PR data for changed notifications in the viewport.
                    auto_fetch_viewport_prs(&state, &tx, &changed).await;
                }
                let _ = tx.send(SyncEvent::SyncStatus {
                    status: SyncStatusKind::Completed,
                });
            }
            Err(e) => {
                tracing::warn!(error = %e, "notification sync failed");
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

/// For changed notifications whose PR is currently in the viewport,
/// fetch fresh PR data and broadcast SSE updates.
async fn auto_fetch_viewport_prs(
    state: &AppState,
    tx: &broadcast::Sender<SyncEvent>,
    changed: &[ChangedNotification],
) {
    let viewport = state.viewport_prs.read().await;
    if viewport.is_empty() {
        return;
    }

    for notif in changed {
        let pr_id = match notif.pr_id {
            Some(id) => id,
            None => continue,
        };
        if !viewport.contains(&(notif.repository.clone(), pr_id)) {
            continue;
        }

        let parts: Vec<&str> = notif.repository.splitn(2, '/').collect();
        if parts.len() != 2 {
            continue;
        }
        let (owner, repo_name) = (parts[0], parts[1]);

        match fetch_and_cache_pr(&state.pool, &state.github, owner, repo_name, pr_id).await {
            Ok(_) => {
                // Read the updated PR row and broadcast SSE.
                if let Ok(Some(pr_row)) =
                    queries::get_pull_request(&state.pool, &notif.repository, pr_id).await
                {
                    let pr_status = derive_pr_status_from_row(&pr_row);
                    let ci_status = pr_row.ci_status.clone();
                    let teams: Option<Vec<String>> = pr_row
                        .teams
                        .as_deref()
                        .and_then(|json| serde_json::from_str(json).ok());

                    let (new_commits, new_comments_json) =
                        queries::get_pr_activity(&state.pool, pr_id, &notif.repository)
                            .await
                            .unwrap_or((None, None));
                    let new_comments: Option<Vec<PrNewComment>> = new_comments_json
                        .as_deref()
                        .and_then(|json| serde_json::from_str(json).ok());

                    let new_reviews = queries::get_pr_review_activity(&state.pool, pr_id)
                        .await
                        .unwrap_or(None);

                    let _ = tx.send(SyncEvent::PrInfoUpdated(PrInfoUpdatedData {
                        pr_id,
                        repository: notif.repository.clone(),
                        author: pr_row.author.clone(),
                        pr_status,
                        ci_status,
                        new_commits,
                        new_comments,
                        new_reviews,
                        teams,
                    }));
                }
            }
            Err(e) => {
                tracing::warn!(
                    repository = %notif.repository,
                    pr_id,
                    error = ?e,
                    "auto-fetch viewport PR failed"
                );
            }
        }
    }
}
