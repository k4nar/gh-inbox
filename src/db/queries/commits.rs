use sqlx::SqlitePool;

/// A commit row from the database.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct CommitRow {
    pub sha: String,
    pub pr_id: i64,
    pub message: String,
    pub author: String,
    pub committed_at: String,
}

/// Insert or update a commit.
pub async fn upsert_commit(pool: &SqlitePool, commit: &CommitRow) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO commits (sha, pr_id, message, author, committed_at)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(sha) DO NOTHING",
    )
    .bind(&commit.sha)
    .bind(commit.pr_id)
    .bind(&commit.message)
    .bind(&commit.author)
    .bind(&commit.committed_at)
    .execute(pool)
    .await?;
    Ok(())
}

/// Query all commits for a given PR, ordered by commit time.
pub async fn query_commits_for_pr(pool: &SqlitePool, pr_id: i64) -> sqlx::Result<Vec<CommitRow>> {
    sqlx::query_as::<_, CommitRow>(
        "SELECT sha, pr_id, message, author, committed_at
         FROM commits
         WHERE pr_id = ?
         ORDER BY committed_at ASC",
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
            author_avatar_url: None,
            url: "https://github.com/owner/repo/pull/42".to_string(),
            ci_status: None,
            last_viewed_at: None,
            body: String::new(),
            state: "open".to_string(),
            head_sha: "abc123".to_string(),
            additions: 0,
            deletions: 0,
            changed_files: 0,
            draft: false,
            merged_at: None,
            teams: None,
            labels: String::from("[]"),
        }
    }

    fn sample(sha: &str, pr_id: i64) -> CommitRow {
        CommitRow {
            sha: sha.to_string(),
            pr_id,
            message: "Fix something".to_string(),
            author: "alice".to_string(),
            committed_at: "2025-01-01T00:00:00Z".to_string(),
        }
    }

    #[tokio::test]
    async fn upsert_and_query() {
        let pool = test_pool().await;
        upsert_pull_request(&pool, &sample_pr()).await.unwrap();

        let c1 = sample("aaa", 42);
        let mut c2 = sample("bbb", 42);
        c2.committed_at = "2025-01-02T00:00:00Z".to_string();
        c2.message = "Second commit".to_string();
        upsert_commit(&pool, &c1).await.unwrap();
        upsert_commit(&pool, &c2).await.unwrap();

        let commits = query_commits_for_pr(&pool, 42).await.unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].sha, "aaa");
        assert_eq!(commits[1].sha, "bbb");
    }

    #[tokio::test]
    async fn upsert_is_idempotent() {
        let pool = test_pool().await;
        upsert_pull_request(&pool, &sample_pr()).await.unwrap();

        let mut c = sample("aaa", 42);
        c.message = "aaa".to_string();
        upsert_commit(&pool, &c).await.unwrap();
        c.message = "bbb".to_string();
        upsert_commit(&pool, &c).await.unwrap();

        // The second upsert should not overwrite the first (commits are immutable)
        let commits = query_commits_for_pr(&pool, 42).await.unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].message, "aaa");
    }
}
