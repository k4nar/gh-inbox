use sqlx::SqlitePool;

use crate::api::AppError;
use crate::db::queries::{self, CheckRunRow, CommentRow, CommitRow, PullRequestRow, ReviewRow};
use crate::github;
use crate::github::fetch_pr_graphql::GraphqlPrData;
use crate::models::{GithubCheckRun, PrStatus};

/// Minimum seconds between full PR fetches (all 5 endpoints) for the detail view.
/// The inbox prefetch uses ETags instead and has no time-based throttle.
const FETCH_THROTTLE_SECS: i64 = 60;

/// Summary returned after a successful fetch+cache.
pub struct PrFetchResult {
    pub author: String,
    pub pr_status: PrStatus,
}

/// Derive the PR status from its fields.
pub fn derive_pr_status(merged_at: Option<&str>, state: &str, draft: bool) -> PrStatus {
    if merged_at.is_some() {
        PrStatus::Merged
    } else if state == "closed" {
        PrStatus::Closed
    } else if draft {
        PrStatus::Draft
    } else {
        PrStatus::Open
    }
}

/// Derive a PR status from a `PullRequestRow`.
pub fn derive_pr_status_from_row(pr: &PullRequestRow) -> PrStatus {
    derive_pr_status(pr.merged_at.as_deref(), &pr.state, pr.draft)
}

/// Fetch a PR and all related data from GitHub, cache in SQLite, and return a summary.
/// Does NOT update `last_viewed_at`.
/// Returns `None` when the PR was fetched recently (throttled) — caller may still read from DB.
pub async fn fetch_and_cache_pr(
    pool: &SqlitePool,
    github: &github::GithubClient,
    owner: &str,
    repo: &str,
    number: i64,
) -> Result<Option<PrFetchResult>, AppError> {
    let full_repo = format!("{owner}/{repo}");
    let resource_key = format!("pr:{full_repo}#{number}");

    let should_fetch = match queries::get_last_fetched_epoch(pool, &resource_key).await? {
        Some(last) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock before UNIX epoch")
                .as_secs() as i64;
            now - last >= FETCH_THROTTLE_SECS
        }
        None => true,
    };

    if !should_fetch {
        return Ok(None);
    }

    // Single GraphQL call replaces 6+ REST calls
    let data = github::fetch_pr_graphql(github, owner, repo, number).await?;

    cache_pr_data(pool, &data, &full_repo, number).await?;

    let author = data.pull_request.user.login.clone();
    let pr_status = derive_pr_status(
        data.pull_request.merged_at.as_deref(),
        &data.pull_request.state,
        data.pull_request.draft,
    );

    queries::set_last_fetched_now(pool, &resource_key).await?;

    Ok(Some(PrFetchResult { author, pr_status }))
}

/// Cache all PR data from a GraphQL response into SQLite.
pub async fn cache_pr_data(
    pool: &SqlitePool,
    data: &GraphqlPrData,
    full_repo: &str,
    number: i64,
) -> Result<(), AppError> {
    let gh_pr = &data.pull_request;

    let labels_json = serde_json::to_string(&gh_pr.labels).unwrap_or_else(|_| String::from("[]"));

    let pr_row = PullRequestRow {
        id: gh_pr.number,
        title: gh_pr.title.clone(),
        repo: full_repo.to_string(),
        author: gh_pr.user.login.clone(),
        url: gh_pr.html_url.clone(),
        ci_status: None,
        last_viewed_at: None,
        body: gh_pr.body.clone().unwrap_or_default(),
        state: gh_pr.state.clone(),
        head_sha: gh_pr.head.sha.clone(),
        additions: gh_pr.additions.unwrap_or(0),
        deletions: gh_pr.deletions.unwrap_or(0),
        changed_files: gh_pr.changed_files.unwrap_or(0),
        draft: gh_pr.draft,
        merged_at: gh_pr.merged_at.clone(),
        teams: None,
        labels: labels_json,
    };
    queries::upsert_pull_request(pool, &pr_row).await?;

    // Issue comments
    for c in &data.issue_comments {
        let row = CommentRow {
            id: c.id,
            pr_id: number,
            thread_id: Some("conversation".to_string()),
            author: c.user.login.clone(),
            body: c.body.clone(),
            created_at: c.created_at.clone(),
            comment_type: "issue_comment".to_string(),
            path: None,
            position: None,
            in_reply_to_id: None,
            html_url: Some(c.html_url.clone()),
            diff_hunk: None,
            resolved: false,
        };
        queries::upsert_comment(pool, &row).await?;
    }

    // Review comments
    for c in &data.review_comments {
        let thread_id = match c.in_reply_to_id {
            Some(parent_id) => format!("review:{parent_id}"),
            None => format!("review:{}", c.id),
        };
        let resolved = data
            .review_thread_states
            .get(&c.id)
            .or_else(|| {
                c.in_reply_to_id
                    .and_then(|pid| data.review_thread_states.get(&pid))
            })
            .copied()
            .unwrap_or(false);
        let row = CommentRow {
            id: c.id,
            pr_id: number,
            thread_id: Some(thread_id),
            author: c.user.login.clone(),
            body: c.body.clone(),
            created_at: c.created_at.clone(),
            comment_type: "review_comment".to_string(),
            path: Some(c.path.clone()),
            position: c.position,
            in_reply_to_id: c.in_reply_to_id,
            html_url: Some(c.html_url.clone()),
            diff_hunk: c.diff_hunk.clone(),
            resolved,
        };
        queries::upsert_comment(pool, &row).await?;
    }

    // Commits
    for c in &data.commits {
        let first_line = c.commit.message.lines().next().unwrap_or("").to_string();
        let row = CommitRow {
            sha: c.sha.clone(),
            pr_id: number,
            message: first_line,
            author: c.commit.author.name.clone(),
            committed_at: c.commit.author.date.clone(),
        };
        queries::upsert_commit(pool, &row).await?;
    }

    // Check runs
    let ci_status = derive_ci_status(&data.check_runs.check_runs);
    if ci_status.is_some() {
        sqlx::query("UPDATE pull_requests SET ci_status = ? WHERE id = ? AND repo = ?")
            .bind(&ci_status)
            .bind(number)
            .bind(full_repo)
            .execute(pool)
            .await?;
    }

    for cr in &data.check_runs.check_runs {
        let row = CheckRunRow {
            id: cr.id,
            pr_id: number,
            name: cr.name.clone(),
            status: cr.status.clone(),
            conclusion: cr.conclusion.clone(),
        };
        queries::upsert_check_run(pool, &row).await?;
    }

    // Reviews
    for r in &data.reviews {
        let row = ReviewRow {
            id: r.id,
            pr_id: number,
            reviewer: r.user.login.clone(),
            state: r.state.clone(),
            body: r.body.clone(),
            submitted_at: r.submitted_at.clone(),
            html_url: r.html_url.clone(),
        };
        if let Err(e) = queries::upsert_review(pool, &row).await {
            eprintln!("[warn] upsert_review failed for review {}: {e}", row.id);
        }
    }

    // Teams: intersect requested reviewer teams with the user's own teams.
    // Only update if user_teams is populated (i.e. ensure_user_teams_fresh has run).
    let user_teams: std::collections::HashSet<String> = queries::get_all_user_teams(pool)
        .await?
        .into_iter()
        .collect();
    if !user_teams.is_empty() {
        let matched: Vec<&str> = data
            .requested_reviewer_team_slugs
            .iter()
            .filter(|t| user_teams.contains(*t))
            .map(String::as_str)
            .collect();
        let teams_json = serde_json::to_string(&matched).unwrap_or_else(|_| "[]".to_string());
        if let Err(e) = queries::update_teams(pool, number, &teams_json).await {
            eprintln!("[warn] update_teams failed for pr {number}: {e}");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use axum::Router;
    use axum::routing::post;
    use tokio::net::TcpListener;

    use super::*;
    use crate::db::queries;
    use crate::github::GithubClient;

    fn graphql_response(reviews_json: &str, labels_json: &str) -> String {
        format!(
            r#"{{
              "data": {{
                "repository": {{
                  "pullRequest": {{
                    "number": 42,
                    "title": "Test PR",
                    "body": "body text",
                    "state": "OPEN",
                    "isDraft": false,
                    "mergedAt": null,
                    "additions": 10,
                    "deletions": 2,
                    "changedFiles": 1,
                    "url": "https://github.com/owner/repo/pull/42",
                    "author": {{ "login": "alice" }},
                    "headRefOid": "deadbeef",
                    "labels": {{ "nodes": {labels_json} }},
                    "comments": {{ "nodes": [] }},
                    "reviewThreads": {{ "nodes": [] }},
                    "allCommits": {{ "nodes": [] }},
                    "headCommit": {{ "nodes": [{{ "commit": {{ "statusCheckRollup": null }} }}] }},
                    "reviews": {{ "nodes": {reviews_json} }}
                  }}
                }}
              }}
            }}"#
        )
    }

    async fn start_mock(
        reviews_json: &'static str,
        labels_json: &'static str,
    ) -> (String, SqlitePool) {
        let response = graphql_response(reviews_json, labels_json);

        let app = Router::new().route(
            "/graphql",
            post(move || {
                let response = response.clone();
                async move { ([("content-type", "application/json")], response) }
            }),
        );

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let base = format!("http://{addr}");

        let pool = crate::db::init_with_path(":memory:").await;
        (base, pool)
    }

    #[tokio::test]
    async fn fetch_and_cache_pr_stores_review() {
        let reviews_json = r#"[{
            "databaseId": 1,
            "author": { "login": "bob" },
            "state": "APPROVED",
            "body": "LGTM",
            "submittedAt": "2025-06-01T10:00:00Z",
            "url": "https://github.com/owner/repo/pull/42#pullrequestreview-1"
        }]"#;

        let (base, pool) = start_mock(reviews_json, "[]").await;
        let github = GithubClient::new(std::sync::Arc::from("tok"), base);

        let result = fetch_and_cache_pr(&pool, &github, "owner", "repo", 42)
            .await
            .unwrap();
        assert!(result.is_some());

        let reviews = queries::query_reviews_for_pr(&pool, 42).await.unwrap();
        assert_eq!(reviews.len(), 1);
        assert_eq!(reviews[0].id, 1);
        assert_eq!(reviews[0].reviewer, "bob");
        assert_eq!(reviews[0].state, "APPROVED");
        assert_eq!(reviews[0].body, "LGTM");
    }

    #[tokio::test]
    async fn fetch_and_cache_pr_stores_labels() {
        let labels_json =
            r#"[{"name":"bug","color":"d73a4a"},{"name":"enhancement","color":"a2eeef"}]"#;

        let (base, pool) = start_mock("[]", labels_json).await;
        let github = GithubClient::new(std::sync::Arc::from("tok"), base);

        fetch_and_cache_pr(&pool, &github, "owner", "repo", 42)
            .await
            .unwrap();

        let pr = queries::get_pull_request(&pool, "owner/repo", 42)
            .await
            .unwrap()
            .expect("PR should be in DB");

        assert!(
            pr.labels.contains("bug"),
            "labels JSON should contain 'bug'"
        );
        assert!(
            pr.labels.contains("enhancement"),
            "labels JSON should contain 'enhancement'"
        );
        assert!(
            pr.labels.contains("d73a4a"),
            "labels JSON should contain color"
        );
    }

    #[tokio::test]
    async fn fetch_and_cache_pr_with_no_reviews() {
        let (base, pool) = start_mock("[]", "[]").await;
        let github = GithubClient::new(std::sync::Arc::from("tok"), base);

        let result = fetch_and_cache_pr(&pool, &github, "owner", "repo", 42).await;
        assert!(result.is_ok());

        let pr = queries::get_pull_request(&pool, "owner/repo", 42)
            .await
            .unwrap();
        assert!(pr.is_some(), "PR should be cached");

        let reviews = queries::query_reviews_for_pr(&pool, 42).await.unwrap();
        assert!(reviews.is_empty());
    }
}

pub fn derive_ci_status(check_runs: &[GithubCheckRun]) -> Option<String> {
    if check_runs.is_empty() {
        return None;
    }
    let mut has_failure = false;
    let mut has_pending = false;
    for cr in check_runs {
        if cr.status != "completed" {
            has_pending = true;
        } else if cr.conclusion.as_deref() != Some("success")
            && cr.conclusion.as_deref() != Some("skipped")
            && cr.conclusion.as_deref() != Some("neutral")
        {
            has_failure = true;
        }
    }
    Some(if has_failure {
        "failure".to_string()
    } else if has_pending {
        "pending".to_string()
    } else {
        "success".to_string()
    })
}
