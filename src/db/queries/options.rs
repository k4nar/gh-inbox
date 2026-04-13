use sqlx::SqlitePool;

#[derive(Debug, serde::Serialize)]
pub struct InboxOptions {
    pub repos: Vec<String>,
    pub orgs: Vec<String>,
    pub teams: Vec<String>,
    pub authors: Vec<String>,
}

pub async fn get_inbox_options(pool: &SqlitePool) -> sqlx::Result<InboxOptions> {
    let repo_rows: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT repository FROM notifications ORDER BY repository LIMIT 100",
    )
    .fetch_all(pool)
    .await?;

    let repos: Vec<String> = repo_rows.into_iter().map(|r| r.0).collect();

    let orgs: Vec<String> = {
        let mut seen = std::collections::HashSet::new();
        repos
            .iter()
            .filter_map(|r| r.split_once('/').map(|(org, _)| org.to_string()))
            .filter(|org| seen.insert(org.clone()))
            .take(100)
            .collect()
    };

    let team_rows: Vec<(String,)> =
        sqlx::query_as("SELECT slug FROM user_teams ORDER BY slug LIMIT 100")
            .fetch_all(pool)
            .await?;

    let author_rows: Vec<(String,)> = sqlx::query_as(
        "SELECT DISTINCT author FROM pull_requests \
         WHERE id IN (SELECT pr_id FROM notifications WHERE pr_id IS NOT NULL) \
         ORDER BY author LIMIT 100",
    )
    .fetch_all(pool)
    .await?;

    Ok(InboxOptions {
        repos,
        orgs,
        teams: team_rows.into_iter().map(|t| t.0).collect(),
        authors: author_rows.into_iter().map(|a| a.0).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> SqlitePool {
        crate::db::init_with_path(":memory:").await
    }

    #[tokio::test]
    async fn returns_empty_when_no_data() {
        let pool = test_pool().await;
        let opts = get_inbox_options(&pool).await.unwrap();
        assert!(opts.repos.is_empty());
        assert!(opts.orgs.is_empty());
        assert!(opts.teams.is_empty());
        assert!(opts.authors.is_empty());
    }

    #[tokio::test]
    async fn returns_distinct_repos_and_derives_orgs() {
        let pool = test_pool().await;
        for (id, repo) in [("n1", "acme/api"), ("n2", "acme/web"), ("n3", "beta/app")] {
            sqlx::query(
                "INSERT INTO notifications (id, title, repository, reason, unread, archived, updated_at)
                 VALUES (?, 'T', ?, 'mention', 0, 0, '2025-01-01')",
            )
            .bind(id)
            .bind(repo)
            .execute(&pool)
            .await
            .unwrap();
        }
        let opts = get_inbox_options(&pool).await.unwrap();
        assert_eq!(opts.repos, vec!["acme/api", "acme/web", "beta/app"]);
        assert_eq!(opts.orgs, vec!["acme", "beta"]);
    }

    #[tokio::test]
    async fn returns_teams_from_user_teams_table() {
        let pool = test_pool().await;
        crate::db::queries::replace_user_teams(
            &pool,
            &["acme/platform".to_string(), "acme/backend".to_string()],
        )
        .await
        .unwrap();
        let opts = get_inbox_options(&pool).await.unwrap();
        assert_eq!(opts.teams, vec!["acme/backend", "acme/platform"]);
    }

    #[tokio::test]
    async fn returns_authors_from_linked_pull_requests() {
        let pool = test_pool().await;
        sqlx::query(
            "INSERT INTO notifications (id, title, repository, reason, unread, archived, updated_at, pr_id)
             VALUES ('n1', 'T', 'acme/api', 'mention', 0, 0, '2025-01-01', 1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO pull_requests (id, title, repo, author, url, ci_status, body, state, head_sha, additions, deletions, changed_files, draft, labels)
             VALUES (1, 'T', 'acme/api', 'alice', 'https://github.com/acme/api/pull/1', NULL, '', 'open', 'abc', 0, 0, 0, 0, '[]')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let opts = get_inbox_options(&pool).await.unwrap();
        assert_eq!(opts.authors, vec!["alice"]);
    }

    #[tokio::test]
    async fn respects_limit_of_100() {
        let pool = test_pool().await;
        for i in 0..150_u32 {
            sqlx::query(
                "INSERT INTO notifications (id, title, repository, reason, unread, archived, updated_at)
                 VALUES (?, 'T', ?, 'mention', 0, 0, '2025-01-01')",
            )
            .bind(format!("n{i}"))
            .bind(format!("org{i}/repo{i}"))
            .execute(&pool)
            .await
            .unwrap();
        }
        let opts = get_inbox_options(&pool).await.unwrap();
        assert_eq!(opts.repos.len(), 100);
    }
}
