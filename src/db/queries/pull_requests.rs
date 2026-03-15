use sqlx::SqlitePool;

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
    pub draft: bool,
    pub merged_at: Option<String>,
    pub teams: Option<String>, // raw JSON string; deserialized at API layer
}

/// Insert or update a pull request.
pub async fn upsert_pull_request(pool: &SqlitePool, pr: &PullRequestRow) -> sqlx::Result<()> {
    sqlx::query(
		"INSERT INTO pull_requests (id, title, repo, author, url, ci_status, last_viewed_at, body, state, head_sha, additions, deletions, changed_files, draft, merged_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           title         = excluded.title,
           repo          = excluded.repo,
           author        = excluded.author,
           url           = excluded.url,
           ci_status     = excluded.ci_status,
           body          = excluded.body,
           state         = excluded.state,
           head_sha      = excluded.head_sha,
           additions     = excluded.additions,
           deletions     = excluded.deletions,
           changed_files = excluded.changed_files,
           draft         = excluded.draft,
           merged_at     = excluded.merged_at",
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
	.bind(pr.draft)
	.bind(&pr.merged_at)
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
		"SELECT id, title, repo, author, url, ci_status, last_viewed_at, body, state, head_sha, additions, deletions, changed_files, draft, merged_at, teams
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

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> SqlitePool {
        crate::db::init_with_path(":memory:").await
    }

    fn sample(id: i64) -> PullRequestRow {
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
            draft: false,
            merged_at: None,
            teams: None,
        }
    }

    #[tokio::test]
    async fn upsert_roundtrip() {
        let pool = test_pool().await;
        upsert_pull_request(&pool, &sample(100)).await.unwrap();
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
    async fn get_not_found() {
        let pool = test_pool().await;
        let result = get_pull_request(&pool, "owner/repo", 999).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn upsert_does_not_overwrite_teams() {
        let pool = test_pool().await;
        upsert_pull_request(&pool, &sample(200)).await.unwrap();
        // Manually set teams
        sqlx::query("UPDATE pull_requests SET teams = '[\"acme/platform\"]' WHERE id = ?")
            .bind(200_i64)
            .execute(&pool)
            .await
            .unwrap();
        // Upsert again
        upsert_pull_request(&pool, &sample(200)).await.unwrap();
        let row = get_pull_request(&pool, "owner/repo", 200)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(row.teams.as_deref(), Some("[\"acme/platform\"]"));
    }

    #[tokio::test]
    async fn upsert_updates_draft_and_merged_at() {
        let pool = test_pool().await;
        upsert_pull_request(&pool, &sample(300)).await.unwrap(); // draft=false, merged_at=None
        let mut updated = sample(300);
        updated.draft = true;
        updated.merged_at = Some("2025-06-01T00:00:00Z".to_string());
        upsert_pull_request(&pool, &updated).await.unwrap();
        let row = get_pull_request(&pool, "owner/repo", 300)
            .await
            .unwrap()
            .unwrap();
        assert!(row.draft);
        assert_eq!(row.merged_at.as_deref(), Some("2025-06-01T00:00:00Z"));
    }

    #[tokio::test]
    async fn update_last_viewed_at_works() {
        let pool = test_pool().await;
        upsert_pull_request(&pool, &sample(42)).await.unwrap();
        assert!(
            get_pull_request(&pool, "owner/repo", 42)
                .await
                .unwrap()
                .unwrap()
                .last_viewed_at
                .is_none()
        );
        update_last_viewed_at(&pool, 42).await.unwrap();
        assert!(
            get_pull_request(&pool, "owner/repo", 42)
                .await
                .unwrap()
                .unwrap()
                .last_viewed_at
                .is_some()
        );
    }
}
