use sqlx::SqlitePool;

use crate::models::ReviewSummary;

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

/// Returns reviews submitted after the PR's `last_viewed_at`.
/// Returns `Ok(None)` when `last_viewed_at` is NULL (first visit — semantics: unknown baseline).
/// Returns `Ok(Some(vec))` (possibly empty) when `last_viewed_at` is set.
// Will be wired in Task 7 (prefetch.rs); suppress dead_code until then.
#[allow(dead_code)]
pub async fn get_pr_review_activity(
    pool: &SqlitePool,
    pr_id: i64,
) -> sqlx::Result<Option<Vec<ReviewSummary>>> {
    // Fetch last_viewed_at for this PR.
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT last_viewed_at FROM pull_requests WHERE id = ?")
            .bind(pr_id)
            .fetch_optional(pool)
            .await?;

    let last_viewed_at = match row {
        None => return Ok(None),          // PR not in DB yet
        Some((None,)) => return Ok(None), // first visit
        Some((Some(ts),)) => ts,
    };

    let reviews: Vec<(String, String)> = sqlx::query_as(
        "SELECT reviewer, state FROM reviews WHERE pr_id = ? AND submitted_at > ? ORDER BY submitted_at ASC",
    )
    .bind(pr_id)
    .bind(&last_viewed_at)
    .fetch_all(pool)
    .await?;

    Ok(Some(
        reviews
            .into_iter()
            .map(|(reviewer, state)| ReviewSummary { reviewer, state })
            .collect(),
    ))
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
            labels: String::from("[]"),
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

    #[tokio::test]
    async fn get_pr_review_activity_returns_none_when_never_viewed() {
        let pool = test_pool().await;
        upsert_pull_request(&pool, &sample_pr(50)).await.unwrap(); // last_viewed_at = NULL
        let result = get_pr_review_activity(&pool, 50).await.unwrap();
        assert!(result.is_none(), "expect None when last_viewed_at is NULL");
    }

    #[tokio::test]
    async fn get_pr_review_activity_returns_none_for_missing_pr() {
        let pool = test_pool().await;
        let result = get_pr_review_activity(&pool, 999).await.unwrap();
        assert!(result.is_none(), "expect None when PR is not in DB");
    }

    #[tokio::test]
    async fn get_pr_review_activity_returns_empty_when_no_reviews_after_last_viewed() {
        let pool = test_pool().await;
        let mut pr = sample_pr(60);
        pr.last_viewed_at = Some("2025-06-01T12:00:00Z".to_string());
        upsert_pull_request(&pool, &pr).await.unwrap();
        // Insert a review BEFORE last_viewed_at
        let mut old_review = sample_review(1, 60, "APPROVED");
        old_review.submitted_at = "2025-06-01T10:00:00Z".to_string();
        upsert_review(&pool, &old_review).await.unwrap();

        let result = get_pr_review_activity(&pool, 60).await.unwrap();
        let reviews = result.expect("should be Some since last_viewed_at is set");
        assert!(reviews.is_empty(), "old review should not appear");
    }

    #[tokio::test]
    async fn get_pr_review_activity_returns_reviews_after_last_viewed() {
        let pool = test_pool().await;
        let mut pr = sample_pr(70);
        pr.last_viewed_at = Some("2025-06-01T09:00:00Z".to_string());
        upsert_pull_request(&pool, &pr).await.unwrap();
        // Insert review AFTER last_viewed_at
        let mut new_review = sample_review(2, 70, "CHANGES_REQUESTED");
        new_review.reviewer = "bob".to_string();
        new_review.submitted_at = "2025-06-01T11:00:00Z".to_string();
        upsert_review(&pool, &new_review).await.unwrap();

        let result = get_pr_review_activity(&pool, 70).await.unwrap();
        let reviews = result.expect("should be Some");
        assert_eq!(reviews.len(), 1);
        assert_eq!(reviews[0].reviewer, "bob");
        assert_eq!(reviews[0].state, "CHANGES_REQUESTED");
    }
}
