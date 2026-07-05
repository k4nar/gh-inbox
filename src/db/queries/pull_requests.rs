use sqlx::{QueryBuilder, SqlitePool};

/// Enriched inbox row: notification joined with PR data.
/// Activity counts (new_commits, new_comments, new_reviews) are delivered via SSE,
/// not from this query.
#[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
#[ts(export)]
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
    pub author_avatar_url: Option<String>,
    // Values come from a SQL CASE mirroring the PrStatus enum.
    #[ts(as = "Option<crate::models::PrStatus>")]
    pub pr_status: Option<String>,
    pub ci_status: Option<String>,
    // teams (None = fetch not attempted or in progress)
    pub teams: Option<Vec<String>>,
}

/// Raw DB row before JSON deserialization of teams.
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
    pub author_avatar_url: Option<String>,
    pub pr_status: Option<String>,
    pub ci_status: Option<String>,
    pub teams_json: Option<String>,
}

/// A pull request row from the database.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct PullRequestRow {
    pub id: i64,
    pub title: String,
    pub repo: String,
    pub author: String,
    pub author_avatar_url: Option<String>,
    pub url: String,
    pub ci_status: Option<String>,
    pub last_viewed_at: Option<String>,
    pub body: String,
    /// Markdown of `body` rendered (and sanitized) at cache time.
    pub body_html: String,
    pub state: String,
    pub head_sha: String,
    pub additions: i64,
    pub deletions: i64,
    pub changed_files: i64,
    pub draft: bool,
    pub merged_at: Option<String>,
    /// Raw JSON string, deserialized at the API layer.
    /// NULL = team fetch not yet attempted; '[]' = fetched, no matching teams;
    /// '[...]' = matched team slugs. Note: migration 009 also documents a
    /// 'fetching' sentinel, but no code ever wrote it — the migration comment
    /// can't be corrected because sqlx checksums applied migration files.
    /// A literal 'fetching' value would fail the JSON parse in to_inbox_item.
    pub teams: Option<String>,
    pub labels: String, // JSON array, default "[]"
}

/// Filter parameters for inbox queries. All fields are AND-combined.
#[derive(Debug, Default)]
pub struct FilterParams {
    pub repo: Option<String>,
    pub team: Option<String>,
    pub author: Option<String>,
    /// Show only PRs whose status is in this set (empty = no include filter).
    pub state_include: Vec<String>,
    /// Hide PRs whose status is in this set.
    pub state_exclude: Vec<String>,
}

/// SQL predicate (over the `pr` alias) that is true for PRs with the given
/// status, or `None` for unrecognised values. The branches are mutually
/// exclusive and mirror the `pr_status` CASE used when selecting rows, so
/// filtering by a status matches exactly the rows displayed with that status.
fn state_predicate(state: &str) -> Option<&'static str> {
    match state {
        "merged" => Some("pr.merged_at IS NOT NULL"),
        "closed" => Some("(pr.merged_at IS NULL AND pr.state = 'closed')"),
        "draft" => Some("(pr.merged_at IS NULL AND pr.state != 'closed' AND pr.draft = 1)"),
        "open" => Some("(pr.merged_at IS NULL AND pr.state != 'closed' AND pr.draft = 0)"),
        _ => None,
    }
}

fn push_filter_conditions(qb: &mut QueryBuilder<sqlx::Sqlite>, filters: &FilterParams) {
    if let Some(repo) = &filters.repo {
        qb.push(" AND n.repository = ");
        qb.push_bind(repo.clone());
    }
    if let Some(team) = &filters.team {
        qb.push(" AND EXISTS (SELECT 1 FROM json_each(pr.teams) WHERE value = ");
        qb.push_bind(team.clone());
        qb.push(")");
    }
    if let Some(author) = &filters.author {
        qb.push(" AND pr.author = ");
        qb.push_bind(author.clone());
    }

    // Status include/exclude. Predicates come from a fixed allowlist, so pushing
    // them as literal SQL is safe (no user-controlled text reaches the query).
    let includes: Vec<&'static str> = filters
        .state_include
        .iter()
        .filter_map(|s| state_predicate(s))
        .collect();
    if !includes.is_empty() {
        qb.push(" AND (");
        qb.push(includes.join(" OR "));
        qb.push(")");
    }
    for pred in filters
        .state_exclude
        .iter()
        .filter_map(|s| state_predicate(s))
    {
        qb.push(" AND NOT ");
        qb.push(pred);
    }
}

/// Insert or update a pull request.
pub async fn upsert_pull_request<'e>(
    executor: impl sqlx::Executor<'e, Database = sqlx::Sqlite>,
    pr: &PullRequestRow,
) -> sqlx::Result<()> {
    sqlx::query(
		"INSERT INTO pull_requests (id, title, repo, author, author_avatar_url, url, ci_status, last_viewed_at, body, body_html, state, head_sha, additions, deletions, changed_files, draft, merged_at, labels)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(repo, id) DO UPDATE SET
           title             = excluded.title,
           author            = excluded.author,
           author_avatar_url = excluded.author_avatar_url,
           url               = excluded.url,
           ci_status         = excluded.ci_status,
           body              = excluded.body,
           body_html         = excluded.body_html,
           state             = excluded.state,
           head_sha          = excluded.head_sha,
           additions         = excluded.additions,
           deletions         = excluded.deletions,
           changed_files     = excluded.changed_files,
           draft             = excluded.draft,
           merged_at         = excluded.merged_at,
           labels            = excluded.labels",
	)
	.bind(pr.id)
	.bind(&pr.title)
	.bind(&pr.repo)
	.bind(&pr.author)
	.bind(&pr.author_avatar_url)
	.bind(&pr.url)
	.bind(&pr.ci_status)
	.bind(&pr.last_viewed_at)
	.bind(&pr.body)
	.bind(&pr.body_html)
	.bind(&pr.state)
	.bind(&pr.head_sha)
	.bind(pr.additions)
	.bind(pr.deletions)
	.bind(pr.changed_files)
	.bind(pr.draft)
	.bind(&pr.merged_at)
	.bind(&pr.labels)
	.execute(executor)
	.await?;
    Ok(())
}

/// Delete all cached child rows (comments, commits, check runs, reviews) for a
/// PR. Called before re-inserting from a fresh GitHub snapshot so rows that no
/// longer exist upstream (deleted comments, check runs from a previous push)
/// do not linger in the cache.
pub async fn delete_pr_children(
    conn: &mut sqlx::SqliteConnection,
    repo: &str,
    pr_id: i64,
) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM comments WHERE repo = ? AND pr_id = ?")
        .bind(repo)
        .bind(pr_id)
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM commits WHERE repo = ? AND pr_id = ?")
        .bind(repo)
        .bind(pr_id)
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM check_runs WHERE repo = ? AND pr_id = ?")
        .bind(repo)
        .bind(pr_id)
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM reviews WHERE repo = ? AND pr_id = ?")
        .bind(repo)
        .bind(pr_id)
        .execute(&mut *conn)
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
		"SELECT id, title, repo, author, author_avatar_url, url, ci_status, last_viewed_at, body, body_html, state, head_sha, additions, deletions, changed_files, draft, merged_at, teams, labels
         FROM pull_requests
         WHERE repo = ? AND id = ?",
	)
	.bind(repo)
	.bind(number)
	.fetch_optional(pool)
	.await
}

/// Update last_viewed_at to now (ISO 8601) for a pull request.
pub async fn update_last_viewed_at(pool: &SqlitePool, repo: &str, pr_id: i64) -> sqlx::Result<()> {
    sqlx::query("UPDATE pull_requests SET last_viewed_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE repo = ? AND id = ?")
        .bind(repo)
        .bind(pr_id)
        .execute(pool)
        .await?;
    Ok(())
}

fn to_inbox_item(row: InboxItemRow) -> Result<InboxItem, serde_json::Error> {
    let teams: Option<Vec<String>> = row
        .teams_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()?;
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
        author_avatar_url: row.author_avatar_url,
        pr_status: row.pr_status,
        ci_status: row.ci_status,
        teams,
    })
}

/// Query enriched notifications — paginated.
/// `archived` selects inbox (false) or archived (true) view.
/// Returns (items, total_count).
async fn query_enriched_paginated(
    pool: &SqlitePool,
    archived: bool,
    limit: u32,
    offset: u32,
    filters: &FilterParams,
) -> Result<(Vec<InboxItem>, i64), crate::api::AppError> {
    let archived_val = i32::from(archived);

    let mut qb: QueryBuilder<sqlx::Sqlite> = QueryBuilder::new(
        "SELECT \
             n.id, n.pr_id, n.title, n.repository, n.reason, \
             n.unread, n.archived, n.updated_at, \
             pr.author, pr.author_avatar_url, \
             CASE \
                 WHEN pr.merged_at IS NOT NULL THEN 'merged' \
                 WHEN pr.state = 'closed'      THEN 'closed' \
                 WHEN pr.draft = 1             THEN 'draft' \
                 WHEN pr.id IS NOT NULL        THEN 'open' \
                 ELSE NULL \
             END as pr_status, \
             pr.ci_status, pr.teams as teams_json \
         FROM notifications n \
         LEFT JOIN pull_requests pr ON pr.id = n.pr_id AND pr.repo = n.repository \
         WHERE n.archived = ",
    );
    qb.push_bind(archived_val);
    push_filter_conditions(&mut qb, filters);
    qb.push(" ORDER BY n.updated_at DESC LIMIT ");
    qb.push_bind(limit);
    qb.push(" OFFSET ");
    qb.push_bind(offset);

    let rows = qb
        .build_query_as::<InboxItemRow>()
        .fetch_all(pool)
        .await
        .map_err(crate::api::AppError::Database)?;

    let mut count_qb: QueryBuilder<sqlx::Sqlite> = QueryBuilder::new(
        "SELECT COUNT(*) FROM notifications n \
         LEFT JOIN pull_requests pr ON pr.id = n.pr_id AND pr.repo = n.repository \
         WHERE n.archived = ",
    );
    count_qb.push_bind(archived_val);
    push_filter_conditions(&mut count_qb, filters);

    let total: (i64,) = count_qb
        .build_query_as::<(i64,)>()
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

/// Query inbox (unarchived) notifications with enrichment — paginated.
pub async fn query_inbox_enriched_paginated(
    pool: &SqlitePool,
    limit: u32,
    offset: u32,
    filters: &FilterParams,
) -> Result<(Vec<InboxItem>, i64), crate::api::AppError> {
    query_enriched_paginated(pool, false, limit, offset, filters).await
}

/// Query archived notifications with enrichment — paginated.
pub async fn query_archived_enriched_paginated(
    pool: &SqlitePool,
    limit: u32,
    offset: u32,
    filters: &FilterParams,
) -> Result<(Vec<InboxItem>, i64), crate::api::AppError> {
    query_enriched_paginated(pool, true, limit, offset, filters).await
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
                 ELSE (SELECT COUNT(*) FROM commits c WHERE c.repo = pr.repo AND c.pr_id = pr.id AND c.committed_at > pr.last_viewed_at)
            END as new_commits,
            CASE WHEN pr.last_viewed_at IS NULL THEN NULL
                 ELSE COALESCE((
                     SELECT json_group_array(json_object('author', author, 'count', cnt))
                     FROM (SELECT author, COUNT(*) as cnt FROM comments
                           WHERE repo = pr.repo AND pr_id = pr.id AND created_at > pr.last_viewed_at
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
pub async fn update_teams<'e>(
    executor: impl sqlx::Executor<'e, Database = sqlx::Sqlite>,
    repo: &str,
    pr_id: i64,
    teams_json: &str,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE pull_requests SET teams = ? WHERE repo = ? AND id = ?")
        .bind(teams_json)
        .bind(repo)
        .bind(pr_id)
        .execute(executor)
        .await?;
    Ok(())
}

/// Update the CI status for a PR. Passing `None` clears it.
pub async fn update_ci_status<'e>(
    executor: impl sqlx::Executor<'e, Database = sqlx::Sqlite>,
    pr_id: i64,
    repo: &str,
    ci_status: Option<&str>,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE pull_requests SET ci_status = ? WHERE id = ? AND repo = ?")
        .bind(ci_status)
        .bind(pr_id)
        .bind(repo)
        .execute(executor)
        .await?;
    Ok(())
}

#[cfg(test)]
#[path = "pull_requests_tests.rs"]
mod tests;
