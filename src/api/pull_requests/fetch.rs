use sqlx::SqlitePool;

use crate::api::AppError;
use crate::db::queries::{self, CheckRunRow, CommentRow, CommitRow, PullRequestRow, ReviewRow};
use crate::github;
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

    // Fetch PR metadata
    let gh_pr = github::fetch_pull_request(github, owner, repo, number).await?;

    let head_sha = gh_pr.head.sha.clone();
    let author = gh_pr.user.login.clone();
    let pr_status = derive_pr_status(gh_pr.merged_at.as_deref(), &gh_pr.state, gh_pr.draft);

    let labels_json = serde_json::to_string(&gh_pr.labels).unwrap_or_else(|_| String::from("[]"));

    let pr_row = PullRequestRow {
        id: gh_pr.number,
        title: gh_pr.title,
        repo: full_repo.clone(),
        author: author.clone(),
        url: gh_pr.html_url,
        ci_status: None,
        last_viewed_at: None, // ON CONFLICT clause preserves existing value
        body: gh_pr.body.unwrap_or_default(),
        state: gh_pr.state,
        head_sha: head_sha.clone(),
        additions: gh_pr.additions.unwrap_or(0),
        deletions: gh_pr.deletions.unwrap_or(0),
        changed_files: gh_pr.changed_files.unwrap_or(0),
        draft: gh_pr.draft,
        merged_at: gh_pr.merged_at,
        teams: None, // ON CONFLICT clause preserves existing teams value
        labels: labels_json,
    };
    queries::upsert_pull_request(pool, &pr_row).await?;

    // Fetch issue comments (top-level conversation)
    let issue_comments = github::fetch_issue_comments(github, owner, repo, number).await?;
    for c in &issue_comments {
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
        };
        queries::upsert_comment(pool, &row).await?;
    }

    // Fetch review comments (inline on code)
    let review_comments = github::fetch_review_comments(github, owner, repo, number).await?;
    for c in &review_comments {
        let thread_id = match c.in_reply_to_id {
            Some(parent_id) => format!("review:{parent_id}"),
            None => format!("review:{}", c.id),
        };
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
        };
        queries::upsert_comment(pool, &row).await?;
    }

    // Fetch commits
    let gh_commits = github::fetch_commits(github, owner, repo, number).await?;
    for c in &gh_commits {
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

    // Fetch check runs for the head SHA
    let check_run_list = github::fetch_check_runs(github, owner, repo, &head_sha).await?;

    let ci_status = derive_ci_status(&check_run_list.check_runs);
    if ci_status.is_some() {
        sqlx::query("UPDATE pull_requests SET ci_status = ? WHERE id = ? AND repo = ?")
            .bind(&ci_status)
            .bind(number)
            .bind(&full_repo)
            .execute(pool)
            .await?;
    }

    for cr in &check_run_list.check_runs {
        let row = CheckRunRow {
            id: cr.id,
            pr_id: number,
            name: cr.name.clone(),
            status: cr.status.clone(),
            conclusion: cr.conclusion.clone(),
        };
        queries::upsert_check_run(pool, &row).await?;
    }

    // Fetch and cache reviews — failure is non-fatal; log and continue.
    match github::fetch_reviews(github, owner, repo, number).await {
        Ok(reviews) => {
            for r in &reviews {
                let row = ReviewRow {
                    id: r.id,
                    pr_id: number,
                    reviewer: r.user.login.clone(),
                    state: r.state.clone(),
                    body: r.body.clone(),
                    submitted_at: r.submitted_at.clone(),
                    html_url: r.html_url.clone(),
                };
                queries::upsert_review(pool, &row).await?;
            }
        }
        Err(err) => {
            eprintln!("[warn] fetch_reviews failed for {full_repo}#{number}: {err}");
        }
    }

    queries::set_last_fetched_now(pool, &resource_key).await?;

    Ok(Some(PrFetchResult { author, pr_status }))
}

#[cfg(test)]
mod tests {
    use axum::routing::get;
    use axum::{Router, extract::Path};
    use tokio::net::TcpListener;

    use super::*;
    use crate::db::queries;
    use crate::github::GithubClient;

    /// Start a mock GitHub API server that handles all endpoints needed by `fetch_and_cache_pr`.
    /// `reviews_json` controls what the reviews endpoint returns.
    async fn start_mock(
        reviews_json: &'static str,
        pr_labels_json: &'static str,
    ) -> (String, SqlitePool) {
        let pr_response = format!(
            r#"{{
                "number": 42,
                "title": "Test PR",
                "body": "body text",
                "state": "open",
                "draft": false,
                "merged_at": null,
                "user": {{ "login": "alice" }},
                "html_url": "https://github.com/owner/repo/pull/42",
                "head": {{ "sha": "deadbeef" }},
                "additions": 10,
                "deletions": 2,
                "changed_files": 1,
                "labels": {pr_labels_json}
            }}"#
        );

        let app = Router::new()
            .route(
                "/repos/owner/repo/pulls/42",
                get(move || async move { ([("content-type", "application/json")], pr_response) }),
            )
            .route(
                "/repos/owner/repo/issues/42/comments",
                get(|| async { ([("content-type", "application/json")], "[]") }),
            )
            .route(
                "/repos/owner/repo/pulls/42/comments",
                get(|| async { ([("content-type", "application/json")], "[]") }),
            )
            .route(
                "/repos/owner/repo/pulls/42/commits",
                get(|| async { ([("content-type", "application/json")], "[]") }),
            )
            .route(
                "/repos/owner/repo/commits/{sha}/check-runs",
                get(|Path(_sha): Path<String>| async {
                    (
                        [("content-type", "application/json")],
                        r#"{"total_count":0,"check_runs":[]}"#,
                    )
                }),
            )
            .route(
                "/repos/owner/repo/pulls/42/reviews",
                get(move || async move { ([("content-type", "application/json")], reviews_json) }),
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
            "id": 1,
            "user": { "login": "bob" },
            "state": "APPROVED",
            "body": "LGTM",
            "submitted_at": "2025-06-01T10:00:00Z",
            "html_url": "https://github.com/owner/repo/pull/42#pullrequestreview-1"
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

        // labels field should contain the serialized JSON array
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
    async fn fetch_and_cache_pr_continues_on_reviews_error() {
        // Spin up a mock server that has no reviews route (will 404), everything else is fine.
        let pr_response = r#"{
            "number": 99,
            "title": "PR without reviews endpoint",
            "body": null,
            "state": "open",
            "draft": false,
            "merged_at": null,
            "user": { "login": "charlie" },
            "html_url": "https://github.com/owner/repo/pull/99",
            "head": { "sha": "cafebabe" },
            "additions": 0,
            "deletions": 0,
            "changed_files": 0,
            "labels": []
        }"#;

        let app = Router::new()
            .route(
                "/repos/owner/repo/pulls/99",
                get(move || async move { ([("content-type", "application/json")], pr_response) }),
            )
            .route(
                "/repos/owner/repo/issues/99/comments",
                get(|| async { ([("content-type", "application/json")], "[]") }),
            )
            .route(
                "/repos/owner/repo/pulls/99/comments",
                get(|| async { ([("content-type", "application/json")], "[]") }),
            )
            .route(
                "/repos/owner/repo/pulls/99/commits",
                get(|| async { ([("content-type", "application/json")], "[]") }),
            )
            .route(
                "/repos/owner/repo/commits/{sha}/check-runs",
                get(|Path(_sha): Path<String>| async {
                    (
                        [("content-type", "application/json")],
                        r#"{"total_count":0,"check_runs":[]}"#,
                    )
                }),
            );
        // Note: no reviews route — will result in a 404 error from the server.

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let base = format!("http://{addr}");
        let pool = crate::db::init_with_path(":memory:").await;

        let github = GithubClient::new(std::sync::Arc::from("tok"), base);

        // Should succeed even though reviews endpoint is missing
        let result = fetch_and_cache_pr(&pool, &github, "owner", "repo", 99).await;
        assert!(
            result.is_ok(),
            "fetch_and_cache_pr should not fail when reviews endpoint errors"
        );

        // PR should still be in the DB
        let pr = queries::get_pull_request(&pool, "owner/repo", 99)
            .await
            .unwrap();
        assert!(
            pr.is_some(),
            "PR should be cached even when reviews fetch fails"
        );
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
