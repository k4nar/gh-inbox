use std::collections::BTreeMap;

use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;

use crate::db::queries::{self, CheckRunRow, CommentRow, CommitRow, PullRequestRow};
use crate::github;
use crate::server::AppState;

use super::AppError;

/// Minimum seconds between GitHub API fetches for a given PR.
const FETCH_THROTTLE_SECS: i64 = 30;

/// Response payload for GET /api/pull-requests/:owner/:repo/:number
#[derive(Debug, Serialize)]
pub struct PrDetailResponse {
    pub pull_request: PullRequestRow,
    pub comments: Vec<CommentRow>,
    pub commits: Vec<CommitRow>,
    pub check_runs: Vec<CheckRunResponse>,
}

#[derive(Debug, Serialize)]
pub struct CheckRunResponse {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
}

/// A thread of comments grouped by thread_id.
#[derive(Debug, Serialize)]
pub struct ThreadResponse {
    pub thread_id: String,
    pub path: Option<String>,
    pub comments: Vec<CommentRow>,
}

/// GET /api/pull-requests/:owner/:repo/:number
pub async fn get_pr(
    State(state): State<AppState>,
    Path((owner, repo, number)): Path<(String, String, i64)>,
) -> Result<Json<PrDetailResponse>, AppError> {
    let full_repo = format!("{owner}/{repo}");
    let resource_key = format!("pr:{full_repo}#{number}");

    let should_fetch = match queries::get_last_fetched_epoch(&state.pool, &resource_key).await? {
        Some(last) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock before UNIX epoch")
                .as_secs() as i64;
            now - last >= FETCH_THROTTLE_SECS
        }
        None => true,
    };

    if should_fetch {
        // Fetch PR metadata
        let gh_pr = github::fetch_pull_request(
            &state.token,
            &state.client,
            &state.github_base_url,
            &owner,
            &repo,
            number,
        )
        .await?;

        let pr_row = PullRequestRow {
            id: gh_pr.number,
            title: gh_pr.title,
            repo: full_repo.clone(),
            author: gh_pr.user.login,
            url: gh_pr.html_url,
            ci_status: None, // updated below from check runs
            last_viewed_at: None,
            body: gh_pr.body.unwrap_or_default(),
            state: gh_pr.state,
            head_sha: gh_pr.head.sha.clone(),
            additions: gh_pr.additions.unwrap_or(0),
            deletions: gh_pr.deletions.unwrap_or(0),
            changed_files: gh_pr.changed_files.unwrap_or(0),
        };
        queries::upsert_pull_request(&state.pool, &pr_row).await?;

        // Fetch issue comments (top-level conversation)
        let issue_comments = github::fetch_issue_comments(
            &state.token,
            &state.client,
            &state.github_base_url,
            &owner,
            &repo,
            number,
        )
        .await?;

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
            };
            queries::upsert_comment(&state.pool, &row).await?;
        }

        // Fetch review comments (inline on code)
        let review_comments = github::fetch_review_comments(
            &state.token,
            &state.client,
            &state.github_base_url,
            &owner,
            &repo,
            number,
        )
        .await?;

        // Build thread_id: root comments define a thread by their own ID,
        // reply comments inherit thread_id from in_reply_to_id chain.
        // We use the root comment's ID as thread_id.
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
            };
            queries::upsert_comment(&state.pool, &row).await?;
        }

        // Fetch commits for the PR
        let gh_commits = github::fetch_commits(
            &state.token,
            &state.client,
            &state.github_base_url,
            &owner,
            &repo,
            number,
        )
        .await?;

        for c in &gh_commits {
            let first_line = c.commit.message.lines().next().unwrap_or("").to_string();
            let row = CommitRow {
                sha: c.sha.clone(),
                pr_id: number,
                message: first_line,
                author: c.commit.author.name.clone(),
                committed_at: c.commit.author.date.clone(),
            };
            queries::upsert_commit(&state.pool, &row).await?;
        }

        // Fetch check runs for the head SHA
        let check_run_list = github::fetch_check_runs(
            &state.token,
            &state.client,
            &state.github_base_url,
            &owner,
            &repo,
            &gh_pr.head.sha,
        )
        .await?;

        // Derive overall CI status from check runs
        let ci_status = derive_ci_status(&check_run_list.check_runs);
        if ci_status.is_some() {
            sqlx::query("UPDATE pull_requests SET ci_status = ? WHERE id = ? AND repo = ?")
                .bind(&ci_status)
                .bind(number)
                .bind(&full_repo)
                .execute(&state.pool)
                .await?;
        }

        // Cache individual check runs in DB
        for cr in &check_run_list.check_runs {
            let row = CheckRunRow {
                id: cr.id,
                pr_id: number,
                name: cr.name.clone(),
                status: cr.status.clone(),
                conclusion: cr.conclusion.clone(),
            };
            queries::upsert_check_run(&state.pool, &row).await?;
        }

        queries::set_last_fetched_now(&state.pool, &resource_key).await?;
    }

    // Update last_viewed_at
    queries::update_last_viewed_at(&state.pool, number).await?;

    // Read from DB
    let pr = queries::get_pull_request(&state.pool, &full_repo, number)
        .await?
        .ok_or_else(|| AppError::Database(sqlx::Error::RowNotFound))?;

    let comments = queries::query_comments_for_pr(&state.pool, number).await?;
    let commits = queries::query_commits_for_pr(&state.pool, number).await?;

    let check_runs: Vec<CheckRunResponse> = queries::query_check_runs_for_pr(&state.pool, number)
        .await?
        .into_iter()
        .map(|cr| CheckRunResponse {
            name: cr.name,
            status: cr.status,
            conclusion: cr.conclusion,
        })
        .collect();

    Ok(Json(PrDetailResponse {
        pull_request: pr,
        comments,
        commits,
        check_runs,
    }))
}

/// GET /api/pull-requests/:owner/:repo/:number/threads
pub async fn get_threads(
    State(state): State<AppState>,
    Path((owner, repo, number)): Path<(String, String, i64)>,
) -> Result<Json<Vec<ThreadResponse>>, AppError> {
    let _full_repo = format!("{owner}/{repo}");
    let comments = queries::query_comments_for_pr(&state.pool, number).await?;

    let mut threads: BTreeMap<String, Vec<CommentRow>> = BTreeMap::new();
    for c in comments {
        let tid = c
            .thread_id
            .clone()
            .unwrap_or_else(|| format!("orphan:{}", c.id));
        threads.entry(tid).or_default().push(c);
    }

    let result: Vec<ThreadResponse> = threads
        .into_iter()
        .map(|(thread_id, comments)| {
            let path = comments.iter().find_map(|c| c.path.clone());
            ThreadResponse {
                thread_id,
                path,
                comments,
            }
        })
        .collect();

    Ok(Json(result))
}

/// Derive an overall CI status string from a list of check runs.
fn derive_ci_status(check_runs: &[crate::models::GithubCheckRun]) -> Option<String> {
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
