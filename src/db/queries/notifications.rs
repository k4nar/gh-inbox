use sqlx::SqlitePool;

/// A notification row from the database.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct NotificationRow {
    pub id: String,
    pub pr_id: Option<i64>,
    pub title: String,
    pub repository: String,
    pub reason: String,
    pub unread: bool,
    pub archived: bool,
    pub updated_at: String,
}

/// Insert or update a notification.
/// Returns the number of rows affected (0 if nothing changed, 1 if inserted or updated).
/// The ON CONFLICT WHERE clause ensures the UPDATE only fires when `updated_at` or `unread`
/// actually changed, so `rows_affected` is 0 for no-op upserts — atomically, no separate SELECT.
///
/// `synced_at` is always written unconditionally (even on no-op upserts) so that
/// full-sync reconciliation can archive stale rows by comparing timestamps.
pub async fn upsert_notification(
    pool: &SqlitePool,
    notif: &NotificationRow,
    synced_at: i64,
) -> sqlx::Result<u64> {
    let result = sqlx::query(
        "INSERT INTO notifications (id, pr_id, title, repository, reason, unread, archived, updated_at, synced_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           pr_id      = excluded.pr_id,
           title      = excluded.title,
           repository = excluded.repository,
           reason     = excluded.reason,
           unread     = CASE
                          WHEN excluded.reason = 'your_activity' THEN notifications.unread
                          ELSE excluded.unread
                        END,
           updated_at = excluded.updated_at,
           archived   = CASE
                          WHEN excluded.reason = 'your_activity' THEN notifications.archived
                          WHEN excluded.unread = 1               THEN 0
                          ELSE notifications.archived
                        END,
           synced_at  = excluded.synced_at
         WHERE notifications.updated_at != excluded.updated_at
            OR notifications.unread     != excluded.unread
            OR (notifications.archived = 1 AND excluded.unread = 1)",
    )
    .bind(&notif.id)
    .bind(notif.pr_id)
    .bind(&notif.title)
    .bind(&notif.repository)
    .bind(&notif.reason)
    .bind(notif.unread)
    .bind(notif.archived)
    .bind(&notif.updated_at)
    .bind(synced_at)
    .execute(pool)
    .await?;

    // When the upsert was a no-op (nothing changed), synced_at wasn't touched
    // by the ON CONFLICT clause. Stamp it now so reconciliation can tell this
    // notification was returned by GitHub during this sync cycle.
    if result.rows_affected() == 0 {
        sqlx::query("UPDATE notifications SET synced_at = ? WHERE id = ?")
            .bind(synced_at)
            .bind(&notif.id)
            .execute(pool)
            .await?;
    }

    Ok(result.rows_affected())
}

/// Query all non-archived (inbox) notifications.
pub async fn query_inbox(pool: &SqlitePool) -> sqlx::Result<Vec<NotificationRow>> {
    sqlx::query_as::<_, NotificationRow>(
        "SELECT id, pr_id, title, repository, reason, unread, archived, updated_at
         FROM notifications
         WHERE archived = 0
         ORDER BY updated_at DESC",
    )
    .fetch_all(pool)
    .await
}

/// Query all archived notifications.
pub async fn query_archived(pool: &SqlitePool) -> sqlx::Result<Vec<NotificationRow>> {
    sqlx::query_as::<_, NotificationRow>(
        "SELECT id, pr_id, title, repository, reason, unread, archived, updated_at
         FROM notifications
         WHERE archived = 1
         ORDER BY updated_at DESC",
    )
    .fetch_all(pool)
    .await
}

/// Archive a notification by ID. Returns the number of rows affected.
pub async fn archive_notification(pool: &SqlitePool, id: &str) -> sqlx::Result<u64> {
    let result = sqlx::query("UPDATE notifications SET archived = 1 WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

/// Unarchive a notification by ID (move back to inbox). Returns the number of rows affected.
pub async fn unarchive_notification(pool: &SqlitePool, id: &str) -> sqlx::Result<u64> {
    let result = sqlx::query("UPDATE notifications SET archived = 0 WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

/// Mark a notification as read. Returns the number of rows affected.
pub async fn mark_read(pool: &SqlitePool, id: &str) -> sqlx::Result<u64> {
    let result = sqlx::query("UPDATE notifications SET unread = 0 WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

/// Archive all non-archived notifications that were not touched during the
/// current sync cycle (i.e. `synced_at < sync_started_at`).
/// Returns the number of rows affected.
pub async fn archive_stale(pool: &SqlitePool, sync_started_at: i64) -> sqlx::Result<u64> {
    let result = sqlx::query(
        "UPDATE notifications SET archived = 1 \
         WHERE archived = 0 AND synced_at < ?",
    )
    .bind(sync_started_at)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> SqlitePool {
        crate::db::init_with_path(":memory:").await
    }

    fn sample(id: &str) -> NotificationRow {
        NotificationRow {
            id: id.to_string(),
            pr_id: Some(42),
            title: "Fix bug in parser".to_string(),
            repository: "owner/repo".to_string(),
            reason: "review_requested".to_string(),
            unread: true,
            archived: false,
            updated_at: "2025-01-01T00:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn insert_and_query_inbox() {
        let pool = test_pool().await;
        upsert_notification(&pool, &sample("n1"), 1).await.unwrap();
        let inbox = query_inbox(&pool).await.unwrap();
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].id, "n1");
        assert_eq!(inbox[0].reason, "review_requested");
        assert!(inbox[0].unread);
    }

    #[tokio::test]
    async fn archive_and_unarchive() {
        let pool = test_pool().await;
        upsert_notification(&pool, &sample("n2"), 1).await.unwrap();

        archive_notification(&pool, "n2").await.unwrap();
        assert_eq!(query_inbox(&pool).await.unwrap().len(), 0);
        assert_eq!(query_archived(&pool).await.unwrap().len(), 1);

        unarchive_notification(&pool, "n2").await.unwrap();
        assert_eq!(query_inbox(&pool).await.unwrap().len(), 1);
        assert_eq!(query_archived(&pool).await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn upsert_is_idempotent() {
        let pool = test_pool().await;
        let mut notif = sample("n3");
        upsert_notification(&pool, &notif, 1).await.unwrap();
        notif.reason = "mention".to_string();
        notif.updated_at = "2025-01-02T00:00:00Z".to_string();
        upsert_notification(&pool, &notif, 1).await.unwrap();
        let inbox = query_inbox(&pool).await.unwrap();
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].reason, "mention");
    }

    #[tokio::test]
    async fn upsert_unread_moves_archived_back_to_inbox() {
        let pool = test_pool().await;
        let notif = sample("n4");
        upsert_notification(&pool, &notif, 1).await.unwrap();
        archive_notification(&pool, "n4").await.unwrap();
        // Re-upserting with unread=true should unarchive (new activity)
        upsert_notification(&pool, &notif, 1).await.unwrap();
        assert_eq!(query_inbox(&pool).await.unwrap().len(), 1);
        assert_eq!(query_archived(&pool).await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn upsert_read_keeps_archived_status() {
        let pool = test_pool().await;
        let mut notif = sample("n6");
        upsert_notification(&pool, &notif, 1).await.unwrap();
        archive_notification(&pool, "n6").await.unwrap();
        // Re-upserting with unread=false should preserve archived status
        notif.unread = false;
        upsert_notification(&pool, &notif, 1).await.unwrap();
        assert_eq!(query_inbox(&pool).await.unwrap().len(), 0);
        assert_eq!(query_archived(&pool).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn mark_read_works() {
        let pool = test_pool().await;
        upsert_notification(&pool, &sample("n5"), 1).await.unwrap();
        mark_read(&pool, "n5").await.unwrap();
        let inbox = query_inbox(&pool).await.unwrap();
        assert!(!inbox[0].unread);
    }

    #[tokio::test]
    async fn archive_stale_archives_old_synced_at() {
        let pool = test_pool().await;
        // n1 and n2 synced at t=10, n3 synced at t=5 (stale)
        upsert_notification(&pool, &sample("n1"), 10).await.unwrap();
        upsert_notification(&pool, &sample("n2"), 10).await.unwrap();
        upsert_notification(&pool, &sample("n3"), 5).await.unwrap();

        let count = archive_stale(&pool, 10).await.unwrap();
        assert_eq!(count, 1);

        let archived = query_archived(&pool).await.unwrap();
        assert_eq!(archived.len(), 1);
        assert_eq!(archived[0].id, "n3");
    }

    #[tokio::test]
    async fn archive_stale_leaves_current_unchanged() {
        let pool = test_pool().await;
        upsert_notification(&pool, &sample("n1"), 10).await.unwrap();
        upsert_notification(&pool, &sample("n2"), 10).await.unwrap();

        let count = archive_stale(&pool, 10).await.unwrap();
        assert_eq!(count, 0);

        assert_eq!(query_inbox(&pool).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn archive_stale_skips_already_archived() {
        let pool = test_pool().await;
        upsert_notification(&pool, &sample("n1"), 5).await.unwrap();
        archive_notification(&pool, "n1").await.unwrap();

        // n1 is stale but already archived — rows_affected should be 0
        let count = archive_stale(&pool, 10).await.unwrap();
        assert_eq!(count, 0);
    }
}

#[cfg(test)]
mod own_activity_tests {
    use super::*;

    async fn pool() -> SqlitePool {
        crate::db::init_with_path(":memory:").await
    }

    fn row(reason: &str, unread: bool, archived: bool) -> NotificationRow {
        NotificationRow {
            id: "n1".to_string(),
            pr_id: Some(1),
            title: "PR title".to_string(),
            repository: "owner/repo".to_string(),
            reason: reason.to_string(),
            unread,
            archived,
            updated_at: "2025-01-01T00:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn own_activity_insert_starts_as_read() {
        let pool = pool().await;
        upsert_notification(&pool, &row("your_activity", false, false), 1)
            .await
            .unwrap();
        let rows = query_inbox(&pool).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].unread);
    }

    #[tokio::test]
    async fn own_activity_does_not_make_read_notification_unread() {
        let pool = pool().await;
        upsert_notification(&pool, &row("review_requested", false, false), 1)
            .await
            .unwrap();
        let mut r = row("your_activity", false, false);
        r.updated_at = "2025-01-02T00:00:00Z".to_string();
        upsert_notification(&pool, &r, 1).await.unwrap();
        let rows = query_inbox(&pool).await.unwrap();
        assert!(!rows[0].unread);
    }

    #[tokio::test]
    async fn own_activity_preserves_unread_when_already_unread() {
        let pool = pool().await;
        upsert_notification(&pool, &row("review_requested", true, false), 1)
            .await
            .unwrap();
        let mut r = row("your_activity", false, false);
        r.updated_at = "2025-01-02T00:00:00Z".to_string();
        upsert_notification(&pool, &r, 1).await.unwrap();
        let rows = query_inbox(&pool).await.unwrap();
        assert!(rows[0].unread);
    }

    #[tokio::test]
    async fn own_activity_preserves_archived_state() {
        let pool = pool().await;
        upsert_notification(&pool, &row("review_requested", false, true), 1)
            .await
            .unwrap();
        let mut r = row("your_activity", false, false);
        r.updated_at = "2025-01-02T00:00:00Z".to_string();
        upsert_notification(&pool, &r, 1).await.unwrap();
        let archived = query_archived(&pool).await.unwrap();
        assert_eq!(archived.len(), 1, "notification should remain archived");
    }
}
