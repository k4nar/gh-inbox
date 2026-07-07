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
        sync_lock: Arc::new(tokio::sync::Mutex::new(())),
        manual_sync_queued: Arc::new(std::sync::atomic::AtomicBool::new(false)),
    }
}

// ── simple mock (no URL capture) ──────────────────────────────────────

async fn start_mock(response: &'static str) -> String {
    crate::github::test_support::serve_json("/notifications", response).await
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
    "unread": true,
    "updated_at": "2025-01-02T00:00:00Z",
    "subject": {
        "title": "Release note",
        "url": null,
        "type": "Release"
    },
    "repository": { "full_name": "owner/repo" }
}]"#;

// Same PR as ONE_NOTIFICATION but already read (unread=false). With `all=true`
// GitHub keeps returning read-but-not-done notifications like this one.
const READ_NOTIFICATION: &str = r#"[{
    "id": "1",
    "reason": "review_requested",
    "unread": false,
    "updated_at": "2025-01-01T00:00:00Z",
    "subject": {
        "title": "Fix bug",
        "url": "https://api.github.com/repos/owner/repo/pulls/42",
        "type": "PullRequest"
    },
    "repository": { "full_name": "owner/repo" }
}]"#;

// ── preserved tests ───────────────────────────────────────────────────

#[tokio::test]
async fn inserts_notification_and_returns_changed_count() {
    let state = make_state(start_mock(ONE_NOTIFICATION).await).await;

    let result = sync_notifications(&state).await.unwrap();
    assert_eq!(result.changed.len(), 1);

    let inbox = queries::query_inbox(&state.pool).await.unwrap();
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0].id, "1");
    assert_eq!(inbox[0].reason, "review_requested");
}

#[tokio::test]
async fn idempotent_when_data_unchanged() {
    let state = make_state(start_mock(ONE_NOTIFICATION).await).await;

    assert_eq!(sync_notifications(&state).await.unwrap().changed.len(), 1);
    // Second call: incremental (recent last_fetched_at), same data → 0 changed
    assert_eq!(sync_notifications(&state).await.unwrap().changed.len(), 0);

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
async fn issue_subject_gives_null_pr_id() {
    const ISSUE_NOTIFICATION: &str = r#"[{
        "id": "3",
        "reason": "mention",
        "unread": true,
        "updated_at": "2025-01-03T00:00:00Z",
        "subject": {
            "title": "Crash on startup",
            "url": "https://api.github.com/repos/owner/repo/issues/431",
            "type": "Issue"
        },
        "repository": { "full_name": "owner/repo" }
    }]"#;
    let state = make_state(start_mock(ISSUE_NOTIFICATION).await).await;
    sync_notifications(&state).await.unwrap();

    let inbox = queries::query_inbox(&state.pool).await.unwrap();
    assert_eq!(
        inbox[0].pr_id, None,
        "an Issue notification must not be linked to a same-number PR"
    );
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
        uri.contains("all=true"),
        "Sync must use all=true so read notifications stay in the feed, got: {uri}"
    );
}

#[tokio::test]
async fn full_sync_when_last_fetch_over_2h_does_not_include_since() {
    let (base, captured) = start_mock_capturing(ONE_NOTIFICATION).await;
    let state = make_state(base).await;

    let three_hours_ago = now_epoch() - 3 * 3600;
    sqlx::query("INSERT INTO last_fetched_at (resource, fetched_at) VALUES ('notifications', ?)")
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
    sqlx::query("INSERT INTO last_fetched_at (resource, fetched_at) VALUES ('notifications', ?)")
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
    let expected_iso = epoch_to_iso(epoch).unwrap();
    // Only check the date portion (YYYY-MM-DD) to avoid sub-second jitter.
    let expected_date = &expected_iso[..10];

    sqlx::query("INSERT INTO last_fetched_at (resource, fetched_at) VALUES ('notifications', ?)")
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
async fn full_sync_keeps_read_notification_still_in_github_feed() {
    // Regression: a notification the user marked read (but not done) is still
    // returned by GitHub thanks to `all=true`. Reconciliation must NOT archive
    // it — read keeps the PR in the inbox, only "done" should archive.
    let state = make_state(start_mock(READ_NOTIFICATION).await).await;

    // Pre-insert id="1" as read + unarchived with stale synced_at (=0).
    queries::upsert_notification(
        &state.pool,
        &queries::NotificationRow {
            id: "1".to_string(),
            pr_id: Some(42),
            title: "Fix bug".to_string(),
            repository: "owner/repo".to_string(),
            reason: "review_requested".to_string(),
            unread: false,
            archived: false,
            updated_at: "2025-01-01T00:00:00Z".to_string(),
        },
        0,
        false,
    )
    .await
    .unwrap();

    // Full sync (no last_fetched_at) — GitHub still returns id="1" as read.
    sync_notifications(&state).await.unwrap();

    let archived = queries::query_archived(&state.pool).await.unwrap();
    assert!(
        archived.is_empty(),
        "read-but-not-done notification must not be archived, got: {archived:?}"
    );

    let inbox = queries::query_inbox(&state.pool).await.unwrap();
    assert_eq!(inbox.len(), 1, "read notification should stay in the inbox");
    assert_eq!(inbox[0].id, "1");
    assert!(!inbox[0].unread, "notification should remain read");
}

#[tokio::test]
async fn first_seen_read_notification_starts_archived_on_full_sync() {
    // A thread the DB has never tracked, already read on GitHub, arriving via
    // a FULL sync (no cursor → cold start): the user handled it elsewhere
    // (possibly marked done — the API can't tell), so it must not flood the
    // inbox.
    let state = make_state(start_mock(READ_NOTIFICATION).await).await;

    sync_notifications(&state).await.unwrap();

    assert!(queries::query_inbox(&state.pool).await.unwrap().is_empty());
    let archived = queries::query_archived(&state.pool).await.unwrap();
    assert_eq!(archived.len(), 1);
    assert_eq!(archived[0].id, "1");
}

#[tokio::test]
async fn first_seen_read_notification_stays_in_inbox_on_incremental_sync() {
    // Mid-session, a brand-new notification the user happens to read on
    // github.com within one 30s tick must still land in the inbox — the
    // cold-start policy only applies to full syncs.
    let state = make_state(start_mock(READ_NOTIFICATION).await).await;
    queries::set_last_fetched_epoch(&state.pool, "notifications", now_epoch())
        .await
        .unwrap();

    sync_notifications(&state).await.unwrap();

    assert!(
        queries::query_archived(&state.pool)
            .await
            .unwrap()
            .is_empty()
    );
    let inbox = queries::query_inbox(&state.pool).await.unwrap();
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0].id, "1");
    assert!(!inbox[0].unread, "arrives read, but stays in the inbox");
}

#[tokio::test]
async fn first_seen_read_notification_revives_on_new_activity() {
    let state = make_state(start_mock(READ_NOTIFICATION).await).await;
    sync_notifications(&state).await.unwrap();
    assert_eq!(queries::query_archived(&state.pool).await.unwrap().len(), 1);

    // New activity arrives: GitHub reports the thread unread again — the
    // standard revival path must move it back to the inbox.
    let mut revived = queries::NotificationRow {
        id: "1".to_string(),
        pr_id: Some(42),
        title: "Fix bug".to_string(),
        repository: "owner/repo".to_string(),
        reason: "review_requested".to_string(),
        unread: true,
        archived: false,
        updated_at: "2025-01-02T00:00:00Z".to_string(),
    };
    revived.unread = true;
    queries::upsert_notification(&state.pool, &revived, now_epoch(), false)
        .await
        .unwrap();

    let inbox = queries::query_inbox(&state.pool).await.unwrap();
    assert_eq!(inbox.len(), 1);
    assert!(inbox[0].unread);
}

#[tokio::test]
async fn full_sync_archives_notifications_missing_from_github() {
    // id="1" is in the mock response; id="99" is not (marked done/gone on
    // GitHub) → should be archived.
    let state = make_state(start_mock(ONE_NOTIFICATION).await).await;

    // Pre-insert two notifications with old synced_at so they appear stale
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
        0,
        false,
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
        0,
        false,
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
