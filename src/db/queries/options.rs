use std::collections::HashMap;

use sqlx::SqlitePool;

#[derive(Debug, serde::Serialize)]
pub struct InboxOptions {
    pub repos: Vec<String>,
    pub orgs: Vec<String>,
    pub teams: Vec<String>,
    pub authors: Vec<String>,
    pub repo_counts: HashMap<String, i64>,
    pub team_counts: HashMap<String, i64>,
}

pub async fn get_inbox_options(pool: &SqlitePool, archived: bool) -> sqlx::Result<InboxOptions> {
    let archived_flag: i32 = i32::from(archived);

    // Repos shown in the sidebar. The archived view is limited to repos seen in the
    // most recent 200 archived notifications; the inbox shows all repos. Capped at 100.
    let repos: Vec<String> = if archived {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT repository \
             FROM (SELECT repository FROM notifications WHERE archived = 1 ORDER BY updated_at DESC LIMIT 200) \
             ORDER BY repository LIMIT 100",
        )
        .fetch_all(pool)
        .await?;
        rows.into_iter().map(|r| r.0).collect()
    } else {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT repository FROM notifications WHERE archived = 0 ORDER BY repository LIMIT 100",
        )
        .fetch_all(pool)
        .await?;
        rows.into_iter().map(|r| r.0).collect()
    };

    // Notification counts per repo, over the full archived/inbox set so the sidebar
    // badge matches what a repo filter shows in the list (which is not windowed).
    let repo_count_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT repository, COUNT(*) as cnt FROM notifications WHERE archived = ? \
         GROUP BY repository",
    )
    .bind(archived_flag)
    .fetch_all(pool)
    .await?;
    let repo_counts: HashMap<String, i64> = repo_count_rows.into_iter().collect();

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

    let teams: Vec<String> = team_rows.into_iter().map(|t| t.0).collect();

    let team_counts: HashMap<String, i64> = if teams.is_empty() {
        HashMap::new()
    } else {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT jt.value, COUNT(DISTINCT n.id) \
             FROM notifications n \
             JOIN pull_requests pr ON pr.id = n.pr_id AND pr.repo = n.repository \
             JOIN json_each(pr.teams) jt \
             WHERE n.archived = ? AND jt.value IN (SELECT slug FROM user_teams) \
             GROUP BY jt.value",
        )
        .bind(archived_flag)
        .fetch_all(pool)
        .await?;
        rows.into_iter().collect()
    };

    let author_rows: Vec<(String,)> = if archived {
        sqlx::query_as(
            "SELECT DISTINCT author FROM pull_requests \
             WHERE id IN ( \
                 SELECT pr_id FROM (SELECT pr_id FROM notifications WHERE pr_id IS NOT NULL AND archived = 1 ORDER BY updated_at DESC LIMIT 200) \
             ) ORDER BY author LIMIT 100",
        )
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as(
            "SELECT DISTINCT author FROM pull_requests \
             WHERE id IN (SELECT pr_id FROM notifications WHERE pr_id IS NOT NULL AND archived = 0) \
             ORDER BY author LIMIT 100",
        )
        .fetch_all(pool)
        .await?
    };

    Ok(InboxOptions {
        repos,
        orgs,
        teams,
        authors: author_rows.into_iter().map(|a| a.0).collect(),
        repo_counts,
        team_counts,
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
        let opts = get_inbox_options(&pool, false).await.unwrap();
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
        let opts = get_inbox_options(&pool, false).await.unwrap();
        assert_eq!(opts.repos, vec!["acme/api", "acme/web", "beta/app"]);
        assert_eq!(opts.orgs, vec!["acme", "beta"]);
    }

    #[tokio::test]
    async fn archived_view_returns_repos_from_last_200_archived() {
        let pool = test_pool().await;
        sqlx::query(
            "INSERT INTO notifications (id, title, repository, reason, unread, archived, updated_at)
             VALUES ('n1', 'T', 'acme/api', 'mention', 0, 1, '2025-01-02')",
        )
        .execute(&pool)
        .await
        .unwrap();
        // Inbox notification — should not appear in archived options
        sqlx::query(
            "INSERT INTO notifications (id, title, repository, reason, unread, archived, updated_at)
             VALUES ('n2', 'T', 'other/repo', 'mention', 0, 0, '2025-01-01')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let opts = get_inbox_options(&pool, true).await.unwrap();
        assert_eq!(opts.repos, vec!["acme/api"]);
    }

    #[tokio::test]
    async fn excludes_archived_notifications_from_repos_and_authors() {
        let pool = test_pool().await;
        // Inbox notification
        sqlx::query(
            "INSERT INTO notifications (id, title, repository, reason, unread, archived, updated_at, pr_id)
             VALUES ('n1', 'T', 'acme/api', 'mention', 0, 0, '2025-01-01', 1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        // Archived notification — should be excluded
        sqlx::query(
            "INSERT INTO notifications (id, title, repository, reason, unread, archived, updated_at, pr_id)
             VALUES ('n2', 'T', 'other/repo', 'mention', 0, 1, '2025-01-01', 2)",
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
        sqlx::query(
            "INSERT INTO pull_requests (id, title, repo, author, url, ci_status, body, state, head_sha, additions, deletions, changed_files, draft, labels)
             VALUES (2, 'T', 'other/repo', 'bob', 'https://github.com/other/repo/pull/2', NULL, '', 'open', 'def', 0, 0, 0, 0, '[]')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let opts = get_inbox_options(&pool, false).await.unwrap();
        assert_eq!(opts.repos, vec!["acme/api"]);
        assert_eq!(opts.authors, vec!["alice"]);
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
        let opts = get_inbox_options(&pool, false).await.unwrap();
        assert_eq!(opts.teams, vec!["acme/backend", "acme/platform"]);
    }

    #[tokio::test]
    async fn returns_repo_counts_per_repository() {
        let pool = test_pool().await;
        for (id, repo) in [("n1", "acme/api"), ("n2", "acme/api"), ("n3", "acme/web")] {
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
        let opts = get_inbox_options(&pool, false).await.unwrap();
        assert_eq!(opts.repo_counts.get("acme/api"), Some(&2));
        assert_eq!(opts.repo_counts.get("acme/web"), Some(&1));
    }

    #[tokio::test]
    async fn repo_counts_cover_full_archived_set_not_just_the_window() {
        let pool = test_pool().await;
        // 201 archived notifications for the same repo: the repo list is windowed to the
        // most recent 200, but the count must reflect all of them (matching the filter).
        for i in 0..201_u32 {
            sqlx::query(
                "INSERT INTO notifications (id, title, repository, reason, unread, archived, updated_at)
                 VALUES (?, 'T', 'acme/api', 'mention', 0, 1, '2025-01-01')",
            )
            .bind(format!("n{i}"))
            .execute(&pool)
            .await
            .unwrap();
        }
        let opts = get_inbox_options(&pool, true).await.unwrap();
        assert_eq!(opts.repos, vec!["acme/api"]);
        assert_eq!(opts.repo_counts.get("acme/api"), Some(&201));
    }

    #[tokio::test]
    async fn returns_team_counts_for_user_teams() {
        let pool = test_pool().await;
        crate::db::queries::replace_user_teams(&pool, &["acme/platform".to_string()])
            .await
            .unwrap();
        for (nid, pr_id) in [("n1", 1_i64), ("n2", 2)] {
            sqlx::query(
                "INSERT INTO notifications (id, title, repository, reason, unread, archived, updated_at, pr_id)
                 VALUES (?, 'T', 'acme/api', 'mention', 0, 0, '2025-01-01', ?)",
            )
            .bind(nid)
            .bind(pr_id)
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO pull_requests (id, title, repo, author, url, ci_status, body, state, head_sha, additions, deletions, changed_files, draft, labels, teams)
                 VALUES (?, 'T', 'acme/api', 'alice', 'https://github.com/acme/api/pull/1', NULL, '', 'open', 'abc', 0, 0, 0, 0, '[]', '[\"acme/platform\"]')",
            )
            .bind(pr_id)
            .execute(&pool)
            .await
            .unwrap();
        }
        let opts = get_inbox_options(&pool, false).await.unwrap();
        assert_eq!(opts.team_counts.get("acme/platform"), Some(&2));
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
        let opts = get_inbox_options(&pool, false).await.unwrap();
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
        let opts = get_inbox_options(&pool, false).await.unwrap();
        assert_eq!(opts.repos.len(), 100);
    }
}
