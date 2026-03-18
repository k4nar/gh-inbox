use sqlx::SqlitePool;

/// A review row from the database.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct ReviewRow {
    pub id: i64,
    pub pr_id: i64,
    pub reviewer: String,
    pub state: String,
    pub body: String,
    pub submitted_at: String,
    pub html_url: String,
}

/// Insert or update a review. State and body may change on re-submission.
pub async fn upsert_review(pool: &SqlitePool, row: &ReviewRow) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO reviews (id, pr_id, reviewer, state, body, submitted_at, html_url)
         VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           state        = excluded.state,
           body         = excluded.body,
           submitted_at = excluded.submitted_at",
    )
    .bind(row.id)
    .bind(row.pr_id)
    .bind(&row.reviewer)
    .bind(&row.state)
    .bind(&row.body)
    .bind(&row.submitted_at)
    .bind(&row.html_url)
    .execute(pool)
    .await?;
    Ok(())
}

/// Query all reviews for a given PR, ordered by submission time ascending.
pub async fn query_reviews_for_pr(pool: &SqlitePool, pr_id: i64) -> sqlx::Result<Vec<ReviewRow>> {
    sqlx::query_as::<_, ReviewRow>(
        "SELECT id, pr_id, reviewer, state, body, submitted_at, html_url
         FROM reviews
         WHERE pr_id = ?
         ORDER BY submitted_at ASC",
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

    fn sample_pr(id: i64) -> PullRequestRow {
        PullRequestRow {
            id,
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
            draft: false,
            merged_at: None,
            teams: None,
            // labels will be added in Task 4 — leave it out for now, will cause compile error then
        }
    }

    fn sample_review(id: i64, pr_id: i64, state: &str) -> ReviewRow {
        ReviewRow {
            id,
            pr_id,
            reviewer: "alice".to_string(),
            state: state.to_string(),
            body: String::new(),
            submitted_at: "2025-06-01T10:00:00Z".to_string(),
            html_url: format!("https://github.com/owner/repo/pull/{pr_id}#pullrequestreview-{id}"),
        }
    }

    #[tokio::test]
    async fn upsert_and_query_roundtrip() {
        let pool = test_pool().await;
        upsert_pull_request(&pool, &sample_pr(42)).await.unwrap();

        let r = sample_review(1, 42, "APPROVED");
        upsert_review(&pool, &r).await.unwrap();

        let rows = query_reviews_for_pr(&pool, 42).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, 1);
        assert_eq!(rows[0].reviewer, "alice");
        assert_eq!(rows[0].state, "APPROVED");
    }

    #[tokio::test]
    async fn upsert_updates_state_on_conflict() {
        let pool = test_pool().await;
        upsert_pull_request(&pool, &sample_pr(42)).await.unwrap();

        let mut r = sample_review(1, 42, "CHANGES_REQUESTED");
        upsert_review(&pool, &r).await.unwrap();
        r.state = "APPROVED".to_string();
        upsert_review(&pool, &r).await.unwrap();

        let rows = query_reviews_for_pr(&pool, 42).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].state, "APPROVED");
    }

    #[tokio::test]
    async fn query_returns_empty_for_unknown_pr() {
        let pool = test_pool().await;
        let rows = query_reviews_for_pr(&pool, 999).await.unwrap();
        assert!(rows.is_empty());
    }

    #[tokio::test]
    async fn query_orders_by_submitted_at() {
        let pool = test_pool().await;
        upsert_pull_request(&pool, &sample_pr(42)).await.unwrap();

        let mut r1 = sample_review(1, 42, "APPROVED");
        r1.submitted_at = "2025-06-01T12:00:00Z".to_string();
        let mut r2 = sample_review(2, 42, "CHANGES_REQUESTED");
        r2.reviewer = "bob".to_string();
        r2.submitted_at = "2025-06-01T10:00:00Z".to_string();

        upsert_review(&pool, &r1).await.unwrap();
        upsert_review(&pool, &r2).await.unwrap();

        let rows = query_reviews_for_pr(&pool, 42).await.unwrap();
        assert_eq!(rows[0].reviewer, "bob"); // earlier
        assert_eq!(rows[1].reviewer, "alice"); // later
    }
}
