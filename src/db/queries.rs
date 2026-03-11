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
    pub body: String,
    pub state: String,
    pub head_sha: String,
    pub additions: i64,
    pub deletions: i64,
    pub changed_files: i64,
}

/// A comment row from the database.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct CommentRow {
    pub id: i64,
    pub pr_id: i64,
    pub thread_id: Option<String>,
    pub author: String,
    pub body: String,
    pub created_at: String,
    pub comment_type: String,
    pub path: Option<String>,
    pub position: Option<i64>,
    pub in_reply_to_id: Option<i64>,
}

/// Insert or update a notification.
/// Returns the number of rows affected (0 if nothing changed, 1 if inserted or updated).
pub async fn upsert_notification(pool: &SqlitePool, notif: &NotificationRow) -> sqlx::Result<u64> {
    // Check if the row already exists with the same updated_at and unread status.
    // If so, skip the upsert and report 0 changes (nothing new from GitHub).
    let existing: Option<(String, bool)> =
        sqlx::query_as("SELECT updated_at, unread FROM notifications WHERE id = ?")
            .bind(&notif.id)
            .fetch_optional(pool)
            .await?;

    let is_change = match &existing {
        Some((existing_updated_at, existing_unread)) => {
            existing_updated_at != &notif.updated_at || *existing_unread != notif.unread
        }
        None => true, // new row is always a change
    };

    sqlx::query(
        "INSERT INTO notifications (id, pr_id, title, repository, reason, unread, archived, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           pr_id = excluded.pr_id,
           title = excluded.title,
           repository = excluded.repository,
           reason = excluded.reason,
           unread = excluded.unread,
           updated_at = excluded.updated_at,
           archived = CASE WHEN excluded.unread = 1 THEN 0 ELSE notifications.archived END",
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

    Ok(if is_change { 1 } else { 0 })
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

/// Get the last fetched epoch (seconds since UNIX epoch) for a resource.
pub async fn get_last_fetched_epoch(
    pool: &SqlitePool,
    resource: &str,
) -> sqlx::Result<Option<i64>> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT fetched_at FROM last_fetched_at WHERE resource = ?")
            .bind(resource)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|r| r.0))
}

/// Set the last fetched timestamp for a resource to now (epoch seconds).
pub async fn set_last_fetched_now(pool: &SqlitePool, resource: &str) -> sqlx::Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before UNIX epoch")
        .as_secs() as i64;
    sqlx::query(
        "INSERT INTO last_fetched_at (resource, fetched_at)
         VALUES (?, ?)
         ON CONFLICT(resource) DO UPDATE SET fetched_at = excluded.fetched_at",
    )
    .bind(resource)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}

/// Insert or update a pull request.
pub async fn upsert_pull_request(pool: &SqlitePool, pr: &PullRequestRow) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO pull_requests (id, title, repo, author, url, ci_status, last_viewed_at, body, state, head_sha, additions, deletions, changed_files)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           title = excluded.title,
           repo = excluded.repo,
           author = excluded.author,
           url = excluded.url,
           ci_status = excluded.ci_status,
           body = excluded.body,
           state = excluded.state,
           head_sha = excluded.head_sha,
           additions = excluded.additions,
           deletions = excluded.deletions,
           changed_files = excluded.changed_files",
    )
    .bind(pr.id)
    .bind(&pr.title)
    .bind(&pr.repo)
    .bind(&pr.author)
    .bind(&pr.url)
    .bind(&pr.ci_status)
    .bind(&pr.last_viewed_at)
    .bind(&pr.body)
    .bind(&pr.state)
    .bind(&pr.head_sha)
    .bind(pr.additions)
    .bind(pr.deletions)
    .bind(pr.changed_files)
    .execute(pool)
    .await?;
    Ok(())
}

/// Get a pull request by repo and number.
pub async fn get_pull_request(
    pool: &SqlitePool,
    repo: &str,
    number: i64,
) -> sqlx::Result<Option<PullRequestRow>> {
    sqlx::query_as::<_, PullRequestRow>(
        "SELECT id, title, repo, author, url, ci_status, last_viewed_at, body, state, head_sha, additions, deletions, changed_files
         FROM pull_requests
         WHERE repo = ? AND id = ?",
    )
    .bind(repo)
    .bind(number)
    .fetch_optional(pool)
    .await
}

/// Update last_viewed_at to now (ISO 8601) for a pull request.
pub async fn update_last_viewed_at(pool: &SqlitePool, pr_id: i64) -> sqlx::Result<()> {
    sqlx::query("UPDATE pull_requests SET last_viewed_at = datetime('now') WHERE id = ?")
        .bind(pr_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Insert or update a comment.
pub async fn upsert_comment(pool: &SqlitePool, comment: &CommentRow) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO comments (id, pr_id, thread_id, author, body, created_at, comment_type, path, position, in_reply_to_id)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           body = excluded.body,
           thread_id = excluded.thread_id",
    )
    .bind(comment.id)
    .bind(comment.pr_id)
    .bind(&comment.thread_id)
    .bind(&comment.author)
    .bind(&comment.body)
    .bind(&comment.created_at)
    .bind(&comment.comment_type)
    .bind(&comment.path)
    .bind(comment.position)
    .bind(comment.in_reply_to_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Query all comments for a given PR, ordered by creation time.
pub async fn query_comments_for_pr(pool: &SqlitePool, pr_id: i64) -> sqlx::Result<Vec<CommentRow>> {
    sqlx::query_as::<_, CommentRow>(
        "SELECT id, pr_id, thread_id, author, body, created_at, comment_type, path, position, in_reply_to_id
         FROM comments
         WHERE pr_id = ?
         ORDER BY created_at ASC",
    )
    .bind(pr_id)
    .fetch_all(pool)
    .await
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

    fn sample_pr(id: i64) -> PullRequestRow {
        PullRequestRow {
            id,
            title: "Fix bug".to_string(),
            repo: "owner/repo".to_string(),
            author: "alice".to_string(),
            url: "https://github.com/owner/repo/pull/100".to_string(),
            ci_status: Some("success".to_string()),
            last_viewed_at: None,
            body: "PR body".to_string(),
            state: "open".to_string(),
            head_sha: "abc123".to_string(),
            additions: 10,
            deletions: 3,
            changed_files: 2,
        }
    }

    fn sample_comment(id: i64, pr_id: i64) -> CommentRow {
        CommentRow {
            id,
            pr_id,
            thread_id: None,
            author: "bob".to_string(),
            body: "Looks good!".to_string(),
            created_at: "2025-01-01T00:00:00Z".to_string(),
            comment_type: "issue_comment".to_string(),
            path: None,
            position: None,
            in_reply_to_id: None,
        }
    }

    #[tokio::test]
    async fn upsert_pull_request_roundtrip() {
        let pool = test_pool().await;
        let pr = sample_pr(100);

        upsert_pull_request(&pool, &pr).await.unwrap();

        let row = get_pull_request(&pool, "owner/repo", 100)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(row.title, "Fix bug");
        assert_eq!(row.ci_status, Some("success".to_string()));
        assert_eq!(row.body, "PR body");
        assert_eq!(row.state, "open");
        assert_eq!(row.head_sha, "abc123");
        assert_eq!(row.additions, 10);
        assert_eq!(row.deletions, 3);
        assert_eq!(row.changed_files, 2);
    }

    #[tokio::test]
    async fn get_pull_request_not_found() {
        let pool = test_pool().await;
        let result = get_pull_request(&pool, "owner/repo", 999).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn upsert_and_query_comments() {
        let pool = test_pool().await;
        let pr = sample_pr(42);
        upsert_pull_request(&pool, &pr).await.unwrap();

        let c1 = sample_comment(1, 42);
        let mut c2 = sample_comment(2, 42);
        c2.created_at = "2025-01-02T00:00:00Z".to_string();
        c2.body = "LGTM".to_string();

        upsert_comment(&pool, &c1).await.unwrap();
        upsert_comment(&pool, &c2).await.unwrap();

        let comments = query_comments_for_pr(&pool, 42).await.unwrap();
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0].body, "Looks good!");
        assert_eq!(comments[1].body, "LGTM");
    }

    #[tokio::test]
    async fn upsert_comment_is_idempotent() {
        let pool = test_pool().await;
        let pr = sample_pr(42);
        upsert_pull_request(&pool, &pr).await.unwrap();

        let mut c = sample_comment(1, 42);
        upsert_comment(&pool, &c).await.unwrap();
        c.body = "Updated body".to_string();
        upsert_comment(&pool, &c).await.unwrap();

        let comments = query_comments_for_pr(&pool, 42).await.unwrap();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].body, "Updated body");
    }

    #[tokio::test]
    async fn update_last_viewed_at_works() {
        let pool = test_pool().await;
        let pr = sample_pr(42);
        upsert_pull_request(&pool, &pr).await.unwrap();

        assert!(
            get_pull_request(&pool, "owner/repo", 42)
                .await
                .unwrap()
                .unwrap()
                .last_viewed_at
                .is_none()
        );

        update_last_viewed_at(&pool, 42).await.unwrap();

        let row = get_pull_request(&pool, "owner/repo", 42)
            .await
            .unwrap()
            .unwrap();
        assert!(row.last_viewed_at.is_some());
    }

    #[tokio::test]
    async fn review_comments_with_threading() {
        let pool = test_pool().await;
        let pr = sample_pr(42);
        upsert_pull_request(&pool, &pr).await.unwrap();

        // A root review comment
        let mut root = sample_comment(10, 42);
        root.comment_type = "review_comment".to_string();
        root.path = Some("src/main.rs".to_string());
        root.position = Some(5);
        root.thread_id = Some("thread-1".to_string());
        upsert_comment(&pool, &root).await.unwrap();

        // A reply to it
        let mut reply = sample_comment(11, 42);
        reply.comment_type = "review_comment".to_string();
        reply.path = Some("src/main.rs".to_string());
        reply.position = Some(5);
        reply.in_reply_to_id = Some(10);
        reply.thread_id = Some("thread-1".to_string());
        reply.created_at = "2025-01-02T00:00:00Z".to_string();
        upsert_comment(&pool, &reply).await.unwrap();

        let comments = query_comments_for_pr(&pool, 42).await.unwrap();
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0].thread_id, Some("thread-1".to_string()));
        assert_eq!(comments[1].thread_id, Some("thread-1".to_string()));
    }
}
