use std::collections::HashMap;

use sqlx::SqlitePool;

#[derive(Debug, serde::Serialize)]
pub struct InboxOptions {
    pub repos: Vec<String>,
    pub teams: Vec<String>,
    pub authors: Vec<String>,
    pub repo_counts: HashMap<String, i64>,
    pub team_counts: HashMap<String, i64>,
    pub author_counts: HashMap<String, i64>,
}

pub async fn get_inbox_options(pool: &SqlitePool, archived: bool) -> sqlx::Result<InboxOptions> {
    let archived_flag: i32 = i32::from(archived);

    // Repos shown in the sidebar, with notification counts.
    //
    // Inbox: one grouped query yields both the ordered list (capped at 100) and the
    // counts. Archived: the list is windowed to repos seen in the most recent 200
    // archived notifications, but counts must cover the full archived set (matching
    // what a repo filter shows in the list), so those need separate queries.
    let (repos, repo_counts): (Vec<String>, HashMap<String, i64>) = if archived {
        let repo_rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT repository \
             FROM (SELECT repository FROM notifications WHERE archived = 1 ORDER BY updated_at DESC LIMIT 200) \
             ORDER BY repository LIMIT 100",
        )
        .fetch_all(pool)
        .await?;
        let count_rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT repository, COUNT(*) FROM notifications WHERE archived = 1 GROUP BY repository",
        )
        .fetch_all(pool)
        .await?;
        (
            repo_rows.into_iter().map(|r| r.0).collect(),
            count_rows.into_iter().collect(),
        )
    } else {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT repository, COUNT(*) FROM notifications WHERE archived = 0 \
             GROUP BY repository ORDER BY repository LIMIT 100",
        )
        .fetch_all(pool)
        .await?;
        let repos = rows.iter().map(|(r, _)| r.clone()).collect();
        (repos, rows.into_iter().collect())
    };

    // Codeowner teams that have notifications in this view, with counts. A single
    // grouped query (restricted to the user's teams, ordered by slug) gives both the
    // list and the counts — a team with no matching notifications produces no row, so
    // it is naturally omitted.
    let team_count_rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT jt.value, COUNT(DISTINCT n.id) \
         FROM notifications n \
         JOIN pull_requests pr ON pr.id = n.pr_id AND pr.repo = n.repository \
         JOIN json_each(pr.teams) jt \
         WHERE n.archived = ? AND jt.value IN (SELECT slug FROM user_teams) \
         GROUP BY jt.value ORDER BY jt.value LIMIT 100",
    )
    .bind(archived_flag)
    .fetch_all(pool)
    .await?;
    let teams: Vec<String> = team_count_rows.iter().map(|(t, _)| t.clone()).collect();
    let team_counts: HashMap<String, i64> = team_count_rows.into_iter().collect();

    // Authors of linked PRs, with notification counts.
    //
    // Inbox: one grouped query yields both the ordered list (capped at 100) and the
    // counts. Archived: the list is windowed to the recent 200 archived notifications,
    // but counts cover the full archived set, so they are separate queries.
    let (authors, author_counts): (Vec<String>, HashMap<String, i64>) = if archived {
        let author_rows: Vec<(String,)> = sqlx::query_as(
            "SELECT DISTINCT pr.author \
             FROM (SELECT repository, pr_id FROM notifications WHERE pr_id IS NOT NULL AND archived = 1 ORDER BY updated_at DESC LIMIT 200) n \
             JOIN pull_requests pr ON pr.id = n.pr_id AND pr.repo = n.repository \
             ORDER BY pr.author LIMIT 100",
        )
        .fetch_all(pool)
        .await?;
        let count_rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT pr.author, COUNT(*) \
             FROM notifications n \
             JOIN pull_requests pr ON pr.id = n.pr_id AND pr.repo = n.repository \
             WHERE n.archived = 1 \
             GROUP BY pr.author",
        )
        .fetch_all(pool)
        .await?;
        (
            author_rows.into_iter().map(|a| a.0).collect(),
            count_rows.into_iter().collect(),
        )
    } else {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT pr.author, COUNT(*) \
             FROM notifications n \
             JOIN pull_requests pr ON pr.id = n.pr_id AND pr.repo = n.repository \
             WHERE n.archived = 0 \
             GROUP BY pr.author ORDER BY pr.author LIMIT 100",
        )
        .fetch_all(pool)
        .await?;
        let authors = rows.iter().map(|(a, _)| a.clone()).collect();
        (authors, rows.into_iter().collect())
    };

    Ok(InboxOptions {
        repos,
        teams,
        authors,
        repo_counts,
        team_counts,
        author_counts,
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
        assert!(opts.teams.is_empty());
        assert!(opts.authors.is_empty());
    }

    #[tokio::test]
    async fn returns_distinct_repos() {
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
    async fn returns_only_user_teams_with_notifications() {
        let pool = test_pool().await;
        crate::db::queries::replace_user_teams(
            &pool,
            &["acme/platform".to_string(), "acme/backend".to_string()],
        )
        .await
        .unwrap();
        // A notification + PR tagged with acme/platform only — acme/backend has none.
        sqlx::query(
            "INSERT INTO notifications (id, title, repository, reason, unread, archived, updated_at, pr_id)
             VALUES ('n1', 'T', 'acme/api', 'mention', 0, 0, '2025-01-01', 1)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO pull_requests (id, title, repo, author, url, ci_status, body, state, head_sha, additions, deletions, changed_files, draft, labels, teams)
             VALUES (1, 'T', 'acme/api', 'alice', 'https://x.com', NULL, '', 'open', 'abc', 0, 0, 0, 0, '[]', '[\"acme/platform\"]')",
        )
        .execute(&pool)
        .await
        .unwrap();
        let opts = get_inbox_options(&pool, false).await.unwrap();
        // acme/backend is hidden because it has no notifications.
        assert_eq!(opts.teams, vec!["acme/platform"]);
        assert_eq!(opts.team_counts.get("acme/platform"), Some(&1));
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
    async fn returns_author_counts_per_author() {
        let pool = test_pool().await;
        for (nid, pr_id, author) in [("n1", 1_i64, "alice"), ("n2", 2, "alice"), ("n3", 3, "bob")] {
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
                "INSERT INTO pull_requests (id, title, repo, author, url, ci_status, body, state, head_sha, additions, deletions, changed_files, draft, labels)
                 VALUES (?, 'T', 'acme/api', ?, 'https://x.com', NULL, '', 'open', 'abc', 0, 0, 0, 0, '[]')",
            )
            .bind(pr_id)
            .bind(author)
            .execute(&pool)
            .await
            .unwrap();
        }
        let opts = get_inbox_options(&pool, false).await.unwrap();
        assert_eq!(opts.author_counts.get("alice"), Some(&2));
        assert_eq!(opts.author_counts.get("bob"), Some(&1));
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
