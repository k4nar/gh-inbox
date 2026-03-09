use sqlx::SqlitePool;

/// A notification row from the database.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct NotificationRow {
    pub id: String,
    pub pr_id: Option<i64>,
    pub reason: String,
    pub unread: bool,
    pub archived: bool,
    pub updated_at: String,
}

/// A pull request row from the database.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct PullRequestRow {
    pub id: i64,
    pub title: String,
    pub repo: String,
    pub author: String,
    pub url: String,
    pub ci_status: Option<String>,
    pub last_viewed_at: Option<String>,
}

/// Insert or update a notification.
pub async fn upsert_notification(pool: &SqlitePool, notif: &NotificationRow) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO notifications (id, pr_id, reason, unread, archived, updated_at)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           pr_id = excluded.pr_id,
           reason = excluded.reason,
           unread = excluded.unread,
           updated_at = excluded.updated_at,
           archived = CASE WHEN excluded.unread = 1 THEN 0 ELSE notifications.archived END",
    )
    .bind(&notif.id)
    .bind(notif.pr_id)
    .bind(&notif.reason)
    .bind(notif.unread)
    .bind(notif.archived)
    .bind(&notif.updated_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Query all non-archived (inbox) notifications.
pub async fn query_inbox(pool: &SqlitePool) -> sqlx::Result<Vec<NotificationRow>> {
    sqlx::query_as::<_, NotificationRow>(
        "SELECT id, pr_id, reason, unread, archived, updated_at
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
        "SELECT id, pr_id, reason, unread, archived, updated_at
         FROM notifications
         WHERE archived = 1
         ORDER BY updated_at DESC",
    )
    .fetch_all(pool)
    .await
}

/// Archive a notification by ID.
pub async fn archive_notification(pool: &SqlitePool, id: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE notifications SET archived = 1 WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Unarchive a notification by ID (move back to inbox).
pub async fn unarchive_notification(pool: &SqlitePool, id: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE notifications SET archived = 0 WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Mark a notification as read.
pub async fn mark_read(pool: &SqlitePool, id: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE notifications SET unread = 0 WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Insert or update a pull request.
pub async fn upsert_pull_request(pool: &SqlitePool, pr: &PullRequestRow) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO pull_requests (id, title, repo, author, url, ci_status, last_viewed_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           title = excluded.title,
           repo = excluded.repo,
           author = excluded.author,
           url = excluded.url,
           ci_status = excluded.ci_status",
    )
    .bind(pr.id)
    .bind(&pr.title)
    .bind(&pr.repo)
    .bind(&pr.author)
    .bind(&pr.url)
    .bind(&pr.ci_status)
    .bind(&pr.last_viewed_at)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    async fn test_pool() -> SqlitePool {
        db::init_with_path(":memory:").await
    }

    fn sample_notification(id: &str) -> NotificationRow {
        NotificationRow {
            id: id.to_string(),
            pr_id: Some(42),
            reason: "review_requested".to_string(),
            unread: true,
            archived: false,
            updated_at: "2025-01-01T00:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn insert_and_query_inbox() {
        let pool = test_pool().await;
        let notif = sample_notification("n1");

        upsert_notification(&pool, &notif).await.unwrap();

        let inbox = query_inbox(&pool).await.unwrap();
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].id, "n1");
        assert_eq!(inbox[0].reason, "review_requested");
        assert!(inbox[0].unread);
    }

    #[tokio::test]
    async fn archive_and_unarchive() {
        let pool = test_pool().await;
        let notif = sample_notification("n2");

        upsert_notification(&pool, &notif).await.unwrap();

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
        let mut notif = sample_notification("n3");

        upsert_notification(&pool, &notif).await.unwrap();
        notif.reason = "mention".to_string();
        upsert_notification(&pool, &notif).await.unwrap();

        let inbox = query_inbox(&pool).await.unwrap();
        assert_eq!(inbox.len(), 1);
        assert_eq!(inbox[0].reason, "mention");
    }

    #[tokio::test]
    async fn upsert_unread_moves_archived_back_to_inbox() {
        let pool = test_pool().await;
        let notif = sample_notification("n4");

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
        let mut notif = sample_notification("n6");

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
        let notif = sample_notification("n5");

        upsert_notification(&pool, &notif).await.unwrap();
        mark_read(&pool, "n5").await.unwrap();

        let inbox = query_inbox(&pool).await.unwrap();
        assert!(!inbox[0].unread);
    }

    #[tokio::test]
    async fn upsert_pull_request_roundtrip() {
        let pool = test_pool().await;
        let pr = PullRequestRow {
            id: 100,
            title: "Fix bug".to_string(),
            repo: "owner/repo".to_string(),
            author: "alice".to_string(),
            url: "https://github.com/owner/repo/pull/100".to_string(),
            ci_status: Some("success".to_string()),
            last_viewed_at: None,
        };

        upsert_pull_request(&pool, &pr).await.unwrap();

        let row = sqlx::query_as::<_, PullRequestRow>(
            "SELECT id, title, repo, author, url, ci_status, last_viewed_at FROM pull_requests WHERE id = ?",
        )
        .bind(100)
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(row.title, "Fix bug");
        assert_eq!(row.ci_status, Some("success".to_string()));
    }
}
