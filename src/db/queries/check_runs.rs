use sqlx::SqlitePool;

/// A check run row from the database.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct CheckRunRow {
    pub id: i64,
    pub repo: String,
    pub pr_id: i64,
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
}

/// Insert or update a check run.
pub async fn upsert_check_run<'e>(
    executor: impl sqlx::Executor<'e, Database = sqlx::Sqlite>,
    cr: &CheckRunRow,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO check_runs (id, repo, pr_id, name, status, conclusion)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(repo, pr_id, id) DO UPDATE SET
           status = excluded.status,
           conclusion = excluded.conclusion",
    )
    .bind(cr.id)
    .bind(&cr.repo)
    .bind(cr.pr_id)
    .bind(&cr.name)
    .bind(&cr.status)
    .bind(&cr.conclusion)
    .execute(executor)
    .await?;
    Ok(())
}

/// Query all check runs for a given PR.
pub async fn query_check_runs_for_pr(
    pool: &SqlitePool,
    repo: &str,
    pr_id: i64,
) -> sqlx::Result<Vec<CheckRunRow>> {
    sqlx::query_as::<_, CheckRunRow>(
        "SELECT id, repo, pr_id, name, status, conclusion
         FROM check_runs
         WHERE repo = ? AND pr_id = ?
         ORDER BY name ASC",
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

    fn sample_pr(id: i64) -> PullRequestRow {
        crate::db::queries::test_fixtures::sample_pull_request(id)
    }

    fn sample(id: i64, pr_id: i64) -> CheckRunRow {
        CheckRunRow {
            id,
            repo: "owner/repo".to_string(),
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

        let runs = query_check_runs_for_pr(&pool, "owner/repo", 42)
            .await
            .unwrap();
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

        let runs = query_check_runs_for_pr(&pool, "owner/repo", 42)
            .await
            .unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].status, "completed");
        assert_eq!(runs[0].conclusion, Some("success".to_string()));
    }

    #[tokio::test]
    async fn same_check_id_on_two_prs_does_not_collide() {
        // Check runs belong to a commit: two PRs sharing a head commit report
        // the SAME real check-run id, and synthesized StatusContext ids repeat
        // for every PR carrying the same context name. Each PR must keep its
        // own row — the second insert must not steal or overwrite the first's.
        let pool = test_pool().await;
        upsert_pull_request(&pool, &sample_pr(1)).await.unwrap();
        upsert_pull_request(&pool, &sample_pr(2)).await.unwrap();

        let mut run_a = sample(-77, 1);
        run_a.conclusion = Some("failure".to_string());
        upsert_check_run(&pool, &run_a).await.unwrap();

        let mut run_b = sample(-77, 2);
        run_b.conclusion = Some("success".to_string());
        upsert_check_run(&pool, &run_b).await.unwrap();

        let runs_a = query_check_runs_for_pr(&pool, "owner/repo", 1)
            .await
            .unwrap();
        assert_eq!(runs_a.len(), 1);
        assert_eq!(runs_a[0].conclusion.as_deref(), Some("failure"));

        let runs_b = query_check_runs_for_pr(&pool, "owner/repo", 2)
            .await
            .unwrap();
        assert_eq!(runs_b.len(), 1);
        assert_eq!(runs_b[0].conclusion.as_deref(), Some("success"));
    }

    #[tokio::test]
    async fn query_only_for_given_pr() {
        let pool = test_pool().await;
        upsert_pull_request(&pool, &sample_pr(42)).await.unwrap();
        upsert_pull_request(&pool, &sample_pr(99)).await.unwrap();
        upsert_check_run(&pool, &sample(1, 42)).await.unwrap();
        upsert_check_run(&pool, &sample(2, 99)).await.unwrap();

        let runs = query_check_runs_for_pr(&pool, "owner/repo", 42)
            .await
            .unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].pr_id, 42);
    }
}
