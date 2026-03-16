use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;

use crate::api::AppError;
use crate::db::queries::{self, CommentRow, CommitRow, PullRequestRow};
use crate::models::{PrInfoUpdatedData, SyncEvent};
use crate::server::AppState;

use super::fetch::{derive_pr_status_from_row, fetch_and_cache_pr};

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

/// GET /api/pull-requests/:owner/:repo/:number
pub async fn get_pr(
    State(state): State<AppState>,
    Path((owner, repo, number)): Path<(String, String, i64)>,
) -> Result<Json<PrDetailResponse>, AppError> {
    let full_repo = format!("{owner}/{repo}");

    // Fetch and cache from GitHub (handles throttle internally).
    let fetch_result = fetch_and_cache_pr(
        &state.pool,
        &state.client,
        &state.token,
        &state.github_base_url,
        &owner,
        &repo,
        number,
    )
    .await?;

    // Update last_viewed_at before reading so pr.last_viewed_at is the current timestamp.
    queries::update_last_viewed_at(&state.pool, number).await?;

    let pr = queries::get_pull_request(&state.pool, &full_repo, number)
        .await?
        .ok_or_else(|| AppError::Database(sqlx::Error::RowNotFound))?;

    // Send SSE so the inbox list immediately reflects 0 new activity and correct metadata.
    // fetch_result carries fresher author/status when we just fetched; fall back to the DB row.
    let (author, pr_status) = match fetch_result {
        Some(ref r) => (r.author.clone(), r.pr_status.clone()),
        None => (pr.author.clone(), derive_pr_status_from_row(&pr)),
    };
    let _ = state.tx.send(SyncEvent::PrInfoUpdated(PrInfoUpdatedData {
        pr_id: number,
        repository: full_repo.clone(),
        author,
        pr_status,
        new_commits: Some(0),
        new_comments: Some(vec![]),
    }));

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
