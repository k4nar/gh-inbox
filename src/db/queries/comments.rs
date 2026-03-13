use sqlx::SqlitePool;

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
    use crate::db::queries::{PullRequestRow, upsert_pull_request};

    async fn test_pool() -> SqlitePool {
        crate::db::init_with_path(":memory:").await
    }

    fn sample_pr() -> PullRequestRow {
        PullRequestRow {
            id: 42,
            title: "Fix bug".to_string(),
            repo: "owner/repo".to_string(),
            author: "alice".to_string(),
            url: "https://github.com/owner/repo/pull/42".to_string(),
            ci_status: None,
            last_viewed_at: None,
            body: String::new(),
            state: "open".to_string(),
            head_sha: "abc123".to_string(),
            additions: 0,
            deletions: 0,
            changed_files: 0,
        }
    }

    fn sample(id: i64, pr_id: i64) -> CommentRow {
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
    async fn upsert_and_query() {
        let pool = test_pool().await;
        upsert_pull_request(&pool, &sample_pr()).await.unwrap();

        let c1 = sample(1, 42);
        let mut c2 = sample(2, 42);
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
    async fn upsert_is_idempotent() {
        let pool = test_pool().await;
        upsert_pull_request(&pool, &sample_pr()).await.unwrap();

        let mut c = sample(1, 42);
        upsert_comment(&pool, &c).await.unwrap();
        c.body = "Updated body".to_string();
        upsert_comment(&pool, &c).await.unwrap();

        let comments = query_comments_for_pr(&pool, 42).await.unwrap();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].body, "Updated body");
    }

    #[tokio::test]
    async fn threading() {
        let pool = test_pool().await;
        upsert_pull_request(&pool, &sample_pr()).await.unwrap();

        let mut root = sample(10, 42);
        root.comment_type = "review_comment".to_string();
        root.path = Some("src/main.rs".to_string());
        root.position = Some(5);
        root.thread_id = Some("thread-1".to_string());
        upsert_comment(&pool, &root).await.unwrap();

        let mut reply = sample(11, 42);
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
