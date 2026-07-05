use sqlx::SqlitePool;

/// A commit row from the database.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct CommitRow {
    pub sha: String,
    pub repo: String,
    pub pr_id: i64,
    pub message: String,
    pub author: String,
    pub committed_at: String,
}

/// Insert or update a commit.
pub async fn upsert_commit<'e>(
    executor: impl sqlx::Executor<'e, Database = sqlx::Sqlite>,
    commit: &CommitRow,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO commits (sha, repo, pr_id, message, author, committed_at)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(repo, pr_id, sha) DO NOTHING",
    )
    .bind(&commit.sha)
    .bind(&commit.repo)
    .bind(commit.pr_id)
    .bind(&commit.message)
    .bind(&commit.author)
    .bind(&commit.committed_at)
    .execute(executor)
    .await?;
    Ok(())
}

/// Query all commits for a given PR, ordered by commit time.
pub async fn query_commits_for_pr(
    pool: &SqlitePool,
    repo: &str,
    pr_id: i64,
) -> sqlx::Result<Vec<CommitRow>> {
    sqlx::query_as::<_, CommitRow>(
        "SELECT sha, repo, pr_id, message, author, committed_at
         FROM commits
         WHERE repo = ? AND pr_id = ?
         ORDER BY committed_at ASC",
    )
    .bind(repo)
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
        crate::db::queries::test_fixtures::sample_pull_request(42)
    }

    fn sample(sha: &str, pr_id: i64) -> CommitRow {
        CommitRow {
            sha: sha.to_string(),
            repo: "owner/repo".to_string(),
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

        let commits = query_commits_for_pr(&pool, "owner/repo", 42).await.unwrap();
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
        let commits = query_commits_for_pr(&pool, "owner/repo", 42).await.unwrap();
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].message, "aaa");
    }
}
