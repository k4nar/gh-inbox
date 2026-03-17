use sqlx::SqlitePool;

/// A comment author with their new-comment count, deserialized from JSON.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CommentAuthorCount {
    pub author: String,
    pub count: i64,
}

/// Enriched inbox row: notification joined with PR data + activity summary.
#[derive(Debug, Clone, serde::Serialize)]
pub struct InboxItem {
    // from notifications
    pub id: String,
    pub pr_id: Option<i64>,
    pub title: String,
    pub repository: String,
    pub reason: String,
    pub unread: bool,
    pub archived: bool,
    pub updated_at: String,
    // from pull_requests (None when no linked PR row)
    pub author: Option<String>,
    pub pr_status: Option<String>,
    // activity since last_viewed_at (None = first visit)
    pub new_commits: Option<i64>,
    pub new_comments: Option<Vec<CommentAuthorCount>>,
    // teams (None = fetch not attempted or in progress)
    pub teams: Option<Vec<String>>,
}

/// Raw DB row before JSON deserialization of new_comments and teams.
#[derive(Debug, sqlx::FromRow)]
struct InboxItemRow {
    pub id: String,
    pub pr_id: Option<i64>,
    pub title: String,
    pub repository: String,
    pub reason: String,
    pub unread: bool,
    pub archived: bool,
    pub updated_at: String,
    pub author: Option<String>,
    pub pr_status: Option<String>,
    pub new_commits: Option<i64>,
    pub new_comments_json: Option<String>,
    pub teams_json: Option<String>,
}

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

fn to_inbox_item(row: InboxItemRow) -> Result<InboxItem, serde_json::Error> {
    let new_comments: Option<Vec<CommentAuthorCount>> = match row.new_comments_json.as_deref() {
        None => None,
        Some(json) => Some(serde_json::from_str(json)?),
    };
    let teams: Option<Vec<String>> = match row.teams_json.as_deref() {
        None | Some("fetching") => None,
        Some(json) => Some(serde_json::from_str(json)?),
    };
    Ok(InboxItem {
        id: row.id,
        pr_id: row.pr_id,
        title: row.title,
        repository: row.repository,
        reason: row.reason,
        unread: row.unread,
        archived: row.archived,
        updated_at: row.updated_at,
        author: row.author,
        pr_status: row.pr_status,
        new_commits: row.new_commits,
        new_comments,
        teams,
    })
}

const INBOX_ENRICHED_SQL: &str = "
    SELECT
        n.id, n.pr_id, n.title, n.repository, n.reason,
        n.unread, n.archived, n.updated_at,
        pr.author,
        CASE
            WHEN pr.merged_at IS NOT NULL THEN 'merged'
            WHEN pr.state = 'closed'      THEN 'closed'
            WHEN pr.draft = 1             THEN 'draft'
            WHEN pr.id IS NOT NULL        THEN 'open'
            ELSE NULL
        END as pr_status,
        CASE WHEN pr.last_viewed_at IS NULL THEN NULL
             ELSE (SELECT COUNT(*) FROM commits c
                   WHERE c.pr_id = pr.id AND c.committed_at > pr.last_viewed_at)
        END as new_commits,
        CASE WHEN pr.last_viewed_at IS NULL THEN NULL
             ELSE COALESCE((
                 SELECT json_group_array(json_object('author', author, 'count', cnt))
                 FROM (SELECT author, COUNT(*) as cnt FROM comments
                       WHERE pr_id = pr.id AND created_at > pr.last_viewed_at
                       GROUP BY author
                       ORDER BY cnt DESC, author ASC)
             ), '[]')
        END as new_comments_json,
        pr.teams as teams_json
    FROM notifications n
    LEFT JOIN pull_requests pr ON pr.id = n.pr_id AND pr.repo = n.repository
";

/// Query inbox (unarchived) notifications with enrichment.
pub async fn query_inbox_enriched(
    pool: &SqlitePool,
) -> Result<Vec<InboxItem>, crate::api::AppError> {
    let rows = sqlx::query_as::<_, InboxItemRow>(&format!(
        "{INBOX_ENRICHED_SQL} WHERE n.archived = 0 ORDER BY n.updated_at DESC"
    ))
    .fetch_all(pool)
    .await
    .map_err(crate::api::AppError::Database)?;

    rows.into_iter()
        .map(to_inbox_item)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::api::AppError::Internal(e.to_string()))
}

/// Query archived notifications with enrichment.
pub async fn query_archived_enriched(
    pool: &SqlitePool,
) -> Result<Vec<InboxItem>, crate::api::AppError> {
    let rows = sqlx::query_as::<_, InboxItemRow>(&format!(
        "{INBOX_ENRICHED_SQL} WHERE n.archived = 1 ORDER BY n.updated_at DESC"
    ))
    .fetch_all(pool)
    .await
    .map_err(crate::api::AppError::Database)?;

    rows.into_iter()
        .map(to_inbox_item)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::api::AppError::Internal(e.to_string()))
}

/// Query inbox (unarchived) notifications with enrichment — paginated.
/// Returns (items, total_count).
#[allow(dead_code)]
pub async fn query_inbox_enriched_paginated(
    pool: &SqlitePool,
    limit: u32,
    offset: u32,
) -> Result<(Vec<InboxItem>, i64), crate::api::AppError> {
    let rows = sqlx::query_as::<_, InboxItemRow>(&format!(
        "{INBOX_ENRICHED_SQL} WHERE n.archived = 0 ORDER BY n.updated_at DESC LIMIT ? OFFSET ?"
    ))
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(crate::api::AppError::Database)?;

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notifications WHERE archived = 0")
        .fetch_one(pool)
        .await
        .map_err(crate::api::AppError::Database)?;

    let items = rows
        .into_iter()
        .map(to_inbox_item)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::api::AppError::Internal(e.to_string()))?;

    Ok((items, total.0))
}

/// Query archived notifications with enrichment — paginated.
/// Returns (items, total_count).
#[allow(dead_code)]
pub async fn query_archived_enriched_paginated(
    pool: &SqlitePool,
    limit: u32,
    offset: u32,
) -> Result<(Vec<InboxItem>, i64), crate::api::AppError> {
    let rows = sqlx::query_as::<_, InboxItemRow>(&format!(
        "{INBOX_ENRICHED_SQL} WHERE n.archived = 1 ORDER BY n.updated_at DESC LIMIT ? OFFSET ?"
    ))
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(crate::api::AppError::Database)?;

    let total: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM notifications WHERE archived = 1")
        .fetch_one(pool)
        .await
        .map_err(crate::api::AppError::Database)?;

    let items = rows
        .into_iter()
        .map(to_inbox_item)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| crate::api::AppError::Internal(e.to_string()))?;

    Ok((items, total.0))
}

/// Atomically mark a set of PRs as 'fetching' teams (concurrency guard).
/// Only transitions rows from NULL → 'fetching'. Returns the IDs that were actually changed.
pub async fn set_teams_fetching(pool: &SqlitePool, pr_ids: &[i64]) -> sqlx::Result<Vec<i64>> {
    if pr_ids.is_empty() {
        return Ok(vec![]);
    }
    let mut changed = vec![];
    for &id in pr_ids {
        let result = sqlx::query(
            "UPDATE pull_requests SET teams = 'fetching' WHERE id = ? AND teams IS NULL",
        )
        .bind(id)
        .execute(pool)
        .await?;
        if result.rows_affected() > 0 {
            changed.push(id);
        }
    }
    Ok(changed)
}

/// Query new-commits and new-comments-json for a specific PR since its last_viewed_at.
/// Returns (None, None) when last_viewed_at is NULL (first visit) or when the PR row is missing.
pub async fn get_pr_activity(
    pool: &SqlitePool,
    pr_id: i64,
    repository: &str,
) -> sqlx::Result<(Option<i64>, Option<String>)> {
    let row: Option<(Option<i64>, Option<String>)> = sqlx::query_as(
        "SELECT
            CASE WHEN pr.last_viewed_at IS NULL THEN NULL
                 ELSE (SELECT COUNT(*) FROM commits c WHERE c.pr_id = pr.id AND c.committed_at > pr.last_viewed_at)
            END as new_commits,
            CASE WHEN pr.last_viewed_at IS NULL THEN NULL
                 ELSE COALESCE((
                     SELECT json_group_array(json_object('author', author, 'count', cnt))
                     FROM (SELECT author, COUNT(*) as cnt FROM comments
                           WHERE pr_id = pr.id AND created_at > pr.last_viewed_at
                           GROUP BY author ORDER BY cnt DESC, author ASC)
                 ), '[]')
            END as new_comments_json
         FROM pull_requests pr WHERE pr.id = ? AND pr.repo = ?",
    )
    .bind(pr_id)
    .bind(repository)
    .fetch_optional(pool)
    .await?;
    Ok(row.unwrap_or((None, None)))
}

/// Store the resolved teams JSON for a PR.
pub async fn update_teams(pool: &SqlitePool, pr_id: i64, teams_json: &str) -> sqlx::Result<()> {
    sqlx::query("UPDATE pull_requests SET teams = ? WHERE id = ?")
        .bind(teams_json)
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

    #[tokio::test]
    async fn query_inbox_enriched_returns_inbox_items() {
        let pool = test_pool().await;
        let notif = crate::db::queries::NotificationRow {
            id: "n1".to_string(),
            pr_id: Some(42),
            title: "Fix bug".to_string(),
            repository: "owner/repo".to_string(),
            reason: "review_requested".to_string(),
            unread: true,
            archived: false,
            updated_at: "2025-01-01T00:00:00Z".to_string(),
        };
        crate::db::queries::upsert_notification(&pool, &notif)
            .await
            .unwrap();
        upsert_pull_request(
            &pool,
            &PullRequestRow {
                id: 42,
                title: "Fix bug".to_string(),
                repo: "owner/repo".to_string(),
                author: "alice".to_string(),
                url: "https://github.com/owner/repo/pull/42".to_string(),
                ci_status: None,
                last_viewed_at: Some("2025-01-01T00:00:00Z".to_string()),
                body: String::new(),
                state: "open".to_string(),
                head_sha: "abc".to_string(),
                additions: 0,
                deletions: 0,
                changed_files: 0,
                draft: false,
                merged_at: None,
                teams: None,
            },
        )
        .await
        .unwrap();

        let items = query_inbox_enriched(&pool).await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].author.as_deref(), Some("alice"));
        assert_eq!(items[0].pr_status.as_deref(), Some("open"));
        assert_eq!(items[0].new_commits, Some(0));
        assert_eq!(items[0].new_comments.as_ref().map(|v| v.len()), Some(0));
        assert!(items[0].teams.is_none()); // NULL → None
    }

    #[tokio::test]
    async fn pr_status_draft() {
        let pool = test_pool().await;
        let notif = crate::db::queries::NotificationRow {
            id: "n2".to_string(),
            pr_id: Some(43),
            title: "Draft PR".to_string(),
            repository: "owner/repo".to_string(),
            reason: "review_requested".to_string(),
            unread: true,
            archived: false,
            updated_at: "2025-01-02T00:00:00Z".to_string(),
        };
        crate::db::queries::upsert_notification(&pool, &notif)
            .await
            .unwrap();
        upsert_pull_request(
            &pool,
            &PullRequestRow {
                id: 43,
                title: "Draft PR".to_string(),
                repo: "owner/repo".to_string(),
                author: "bob".to_string(),
                url: "u".to_string(),
                ci_status: None,
                last_viewed_at: Some("2025-01-01T00:00:00Z".to_string()),
                body: String::new(),
                state: "open".to_string(),
                head_sha: "x".to_string(),
                additions: 0,
                deletions: 0,
                changed_files: 0,
                draft: true,
                merged_at: None,
                teams: None,
            },
        )
        .await
        .unwrap();
        let items = query_inbox_enriched(&pool).await.unwrap();
        let item = items.iter().find(|i| i.pr_id == Some(43)).unwrap();
        assert_eq!(item.pr_status.as_deref(), Some("draft"));
    }

    #[tokio::test]
    async fn pr_status_merged() {
        let pool = test_pool().await;
        let notif = crate::db::queries::NotificationRow {
            id: "n3".to_string(),
            pr_id: Some(44),
            title: "Merged PR".to_string(),
            repository: "owner/repo".to_string(),
            reason: "mention".to_string(),
            unread: false,
            archived: false,
            updated_at: "2025-01-03T00:00:00Z".to_string(),
        };
        crate::db::queries::upsert_notification(&pool, &notif)
            .await
            .unwrap();
        upsert_pull_request(
            &pool,
            &PullRequestRow {
                id: 44,
                title: "Merged".to_string(),
                repo: "owner/repo".to_string(),
                author: "carol".to_string(),
                url: "u".to_string(),
                ci_status: None,
                last_viewed_at: Some("2025-01-01T00:00:00Z".to_string()),
                body: String::new(),
                state: "closed".to_string(),
                head_sha: "y".to_string(),
                additions: 0,
                deletions: 0,
                changed_files: 0,
                draft: false,
                merged_at: Some("2025-01-02T12:00:00Z".to_string()),
                teams: None,
            },
        )
        .await
        .unwrap();
        let items = query_inbox_enriched(&pool).await.unwrap();
        let item = items.iter().find(|i| i.pr_id == Some(44)).unwrap();
        assert_eq!(item.pr_status.as_deref(), Some("merged"));
    }

    #[tokio::test]
    async fn first_visit_shows_none_activity() {
        let pool = test_pool().await;
        let notif = crate::db::queries::NotificationRow {
            id: "n4".to_string(),
            pr_id: Some(45),
            title: "New PR".to_string(),
            repository: "owner/repo".to_string(),
            reason: "review_requested".to_string(),
            unread: true,
            archived: false,
            updated_at: "2025-01-04T00:00:00Z".to_string(),
        };
        crate::db::queries::upsert_notification(&pool, &notif)
            .await
            .unwrap();
        upsert_pull_request(
            &pool,
            &PullRequestRow {
                id: 45,
                title: "New PR".to_string(),
                repo: "owner/repo".to_string(),
                author: "dave".to_string(),
                url: "u".to_string(),
                ci_status: None,
                last_viewed_at: None, // never visited
                body: String::new(),
                state: "open".to_string(),
                head_sha: "z".to_string(),
                additions: 0,
                deletions: 0,
                changed_files: 0,
                draft: false,
                merged_at: None,
                teams: None,
            },
        )
        .await
        .unwrap();
        let items = query_inbox_enriched(&pool).await.unwrap();
        let item = items.iter().find(|i| i.pr_id == Some(45)).unwrap();
        assert!(item.new_commits.is_none());
        assert!(item.new_comments.is_none());
    }

    #[tokio::test]
    async fn get_pr_activity_returns_none_when_never_viewed() {
        let pool = test_pool().await;
        upsert_pull_request(&pool, &sample(10)).await.unwrap(); // last_viewed_at = None
        let (commits, comments) = get_pr_activity(&pool, 10, "owner/repo").await.unwrap();
        assert!(commits.is_none(), "expect None when last_viewed_at is NULL");
        assert!(comments.is_none());
    }

    #[tokio::test]
    async fn get_pr_activity_returns_zero_after_viewing() {
        let pool = test_pool().await;
        upsert_pull_request(&pool, &sample(20)).await.unwrap();
        update_last_viewed_at(&pool, 20).await.unwrap();
        let (commits, comments) = get_pr_activity(&pool, 20, "owner/repo").await.unwrap();
        assert_eq!(commits, Some(0));
        // No comments yet, but the field should be Some (not None) now that last_viewed_at is set
        assert!(
            comments.is_some(),
            "expect Some([]) when last_viewed_at is set and no new comments"
        );
    }

    #[tokio::test]
    async fn get_pr_activity_returns_none_for_missing_pr() {
        let pool = test_pool().await;
        let (commits, comments) = get_pr_activity(&pool, 999, "owner/repo").await.unwrap();
        assert!(commits.is_none());
        assert!(comments.is_none());
    }

    #[tokio::test]
    async fn query_inbox_enriched_paginates() {
        let pool = test_pool().await;
        for i in 1..=3 {
            let notif = crate::db::queries::NotificationRow {
                id: format!("n{i}"),
                pr_id: Some(i),
                title: format!("PR {i}"),
                repository: "owner/repo".to_string(),
                reason: "review_requested".to_string(),
                unread: true,
                archived: false,
                updated_at: format!("2025-01-0{i}T00:00:00Z"),
            };
            crate::db::queries::upsert_notification(&pool, &notif)
                .await
                .unwrap();
        }
        let (items, total) = query_inbox_enriched_paginated(&pool, 2, 0).await.unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(total, 3);
        let (items, total) = query_inbox_enriched_paginated(&pool, 2, 2).await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(total, 3);
        let (items, total) = query_inbox_enriched_paginated(&pool, 2, 4).await.unwrap();
        assert!(items.is_empty());
        assert_eq!(total, 3);
    }

    #[tokio::test]
    async fn query_archived_enriched_paginates() {
        let pool = test_pool().await;
        for i in 1..=3 {
            let notif = crate::db::queries::NotificationRow {
                id: format!("a{i}"),
                pr_id: Some(100 + i),
                title: format!("Archived PR {i}"),
                repository: "owner/repo".to_string(),
                reason: "mention".to_string(),
                unread: false,
                archived: true,
                updated_at: format!("2025-02-0{i}T00:00:00Z"),
            };
            crate::db::queries::upsert_notification(&pool, &notif)
                .await
                .unwrap();
        }
        let (items, total) = query_archived_enriched_paginated(&pool, 2, 0)
            .await
            .unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(total, 3);
    }

    #[tokio::test]
    async fn set_teams_fetching_is_idempotent() {
        let pool = test_pool().await;
        upsert_pull_request(&pool, &sample(300)).await.unwrap();
        let changed = set_teams_fetching(&pool, &[300]).await.unwrap();
        assert_eq!(changed, vec![300]);
        // Second call: already 'fetching', should not count
        let changed2 = set_teams_fetching(&pool, &[300]).await.unwrap();
        assert!(changed2.is_empty());
    }
}
