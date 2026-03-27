use sqlx::SqlitePool;

/// A check run row from the database.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct CheckRunRow {
    pub id: i64,
    pub pr_id: i64,
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
}

/// Insert or update a check run.
pub async fn upsert_check_run(pool: &SqlitePool, cr: &CheckRunRow) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO check_runs (id, pr_id, name, status, conclusion)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(id) DO UPDATE SET
           status = excluded.status,
           conclusion = excluded.conclusion",
    )
    .bind(cr.id)
    .bind(cr.pr_id)
    .bind(&cr.name)
    .bind(&cr.status)
    .bind(&cr.conclusion)
    .execute(pool)
    .await?;
    Ok(())
}

/// Query all check runs for a given PR.
pub async fn query_check_runs_for_pr(
    pool: &SqlitePool,
    pr_id: i64,
) -> sqlx::Result<Vec<CheckRunRow>> {
    sqlx::query_as::<_, CheckRunRow>(
        "SELECT id, pr_id, name, status, conclusion
         FROM check_runs
         WHERE pr_id = ?
         ORDER BY name ASC",
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

    fn sample(id: i64, pr_id: i64) -> CheckRunRow {
        CheckRunRow {
            id,
            pr_id,
            name: "CI".to_string(),
            status: "completed".to_string(),
            conclusion: Some("success".to_string()),
        }
    }

    #[tokio::test]
    async fn upsert_and_query() {
        let pool = test_pool().await;
        upsert_pull_request(&pool, &sample_pr(42)).await.unwrap();

        let cr1 = sample(1, 42);
        let mut cr2 = sample(2, 42);
        cr2.name = "Lint".to_string();
        cr2.status = "in_progress".to_string();
        cr2.conclusion = None;
        upsert_check_run(&pool, &cr1).await.unwrap();
        upsert_check_run(&pool, &cr2).await.unwrap();

        let runs = query_check_runs_for_pr(&pool, 42).await.unwrap();
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].name, "CI");
        assert_eq!(runs[1].name, "Lint");
    }

    #[tokio::test]
    async fn upsert_updates_status() {
        let pool = test_pool().await;
        upsert_pull_request(&pool, &sample_pr(42)).await.unwrap();

        let mut cr = sample(1, 42);
        cr.status = "in_progress".to_string();
        cr.conclusion = None;
        upsert_check_run(&pool, &cr).await.unwrap();

        cr.status = "completed".to_string();
        cr.conclusion = Some("success".to_string());
        upsert_check_run(&pool, &cr).await.unwrap();

        let runs = query_check_runs_for_pr(&pool, 42).await.unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, "completed");
        assert_eq!(runs[0].conclusion, Some("success".to_string()));
    }

    #[tokio::test]
    async fn query_only_for_given_pr() {
        let pool = test_pool().await;
        upsert_pull_request(&pool, &sample_pr(42)).await.unwrap();
        upsert_pull_request(&pool, &sample_pr(99)).await.unwrap();
        upsert_check_run(&pool, &sample(1, 42)).await.unwrap();
        upsert_check_run(&pool, &sample(2, 99)).await.unwrap();

        let runs = query_check_runs_for_pr(&pool, 42).await.unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].pr_id, 42);
    }
}
