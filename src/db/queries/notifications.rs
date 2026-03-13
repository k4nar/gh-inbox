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
pub async fn upsert_notification(pool: &SqlitePool, notif: &NotificationRow) -> sqlx::Result<u64> {
    let result = sqlx::query(
		"INSERT INTO notifications (id, pr_id, title, repository, reason, unread, archived, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           pr_id = excluded.pr_id,
           title = excluded.title,
           repository = excluded.repository,
           reason = excluded.reason,
           unread = excluded.unread,
           updated_at = excluded.updated_at,
           archived = CASE WHEN excluded.unread = 1 THEN 0 ELSE notifications.archived END
         WHERE notifications.updated_at != excluded.updated_at
            OR notifications.unread != excluded.unread
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
	.execute(pool)
	.await?;

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
        upsert_notification(&pool, &sample("n1")).await.unwrap();
        let inbox = query_inbox(&pool).await.unwrap();
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].id, "n1");
        assert_eq!(inbox[0].reason, "review_requested");
        assert!(inbox[0].unread);
    }

    #[tokio::test]
    async fn archive_and_unarchive() {
        let pool = test_pool().await;
        upsert_notification(&pool, &sample("n2")).await.unwrap();

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
        upsert_notification(&pool, &notif).await.unwrap();
        notif.reason = "mention".to_string();
        notif.updated_at = "2025-01-02T00:00:00Z".to_string();
        upsert_notification(&pool, &notif).await.unwrap();
        let inbox = query_inbox(&pool).await.unwrap();
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].reason, "mention");
    }

    #[tokio::test]
    async fn upsert_unread_moves_archived_back_to_inbox() {
        let pool = test_pool().await;
        let notif = sample("n4");
        upsert_notification(&pool, &notif).await.unwrap();
        archive_notification(&pool, "n4").await.unwrap();
        // Re-upserting with unread=true should unarchive (new activity)
        upsert_notification(&pool, &notif).await.unwrap();
        assert_eq!(query_inbox(&pool).await.unwrap().len(), 1);
        assert_eq!(query_archived(&pool).await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn upsert_read_keeps_archived_status() {
        let pool = test_pool().await;
        let mut notif = sample("n6");
        upsert_notification(&pool, &notif).await.unwrap();
        archive_notification(&pool, "n6").await.unwrap();
        // Re-upserting with unread=false should preserve archived status
        notif.unread = false;
        upsert_notification(&pool, &notif).await.unwrap();
        assert_eq!(query_inbox(&pool).await.unwrap().len(), 0);
        assert_eq!(query_archived(&pool).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn mark_read_works() {
        let pool = test_pool().await;
        upsert_notification(&pool, &sample("n5")).await.unwrap();
        mark_read(&pool, "n5").await.unwrap();
        let inbox = query_inbox(&pool).await.unwrap();
        assert!(!inbox[0].unread);
    }
}
