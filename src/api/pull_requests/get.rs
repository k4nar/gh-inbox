use std::collections::BTreeMap;

use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;

use crate::api::AppError;
use crate::db::queries::{self, CommentRow, CommitRow, PullRequestRow};
use crate::markdown::render_markdown;
use crate::models::{PrInfoUpdatedData, SyncEvent};
use crate::server::AppState;

use super::fetch::{derive_pr_status_from_row, fetch_and_cache_pr};

/// PR data returned in the API response (DB row + rendered body).
#[derive(Debug, Serialize)]
pub struct PullRequestResponse {
    #[serde(flatten)]
    pub inner: PullRequestRow,
    pub body_html: String,
}

/// Comment data returned in the API response (DB row + rendered body).
#[derive(Debug, Serialize)]
pub struct CommentResponse {
    #[serde(flatten)]
    pub inner: CommentRow,
    pub body_html: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ReviewResponse {
    pub id: i64,
    pub reviewer: String,
    pub reviewer_avatar_url: Option<String>,
    pub state: String,
    pub body: String,
    pub submitted_at: String,
    pub html_url: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct LabelResponse {
    pub name: String,
    pub color: String,
}

/// A thread of comments grouped by thread_id.
#[derive(Debug, Serialize)]
pub struct ThreadResponse {
    pub thread_id: String,
    pub path: Option<String>,
    pub resolved: bool,
    pub comments: Vec<CommentResponse>,
}

/// Response payload for GET /api/pull-requests/:owner/:repo/:number
#[derive(Debug, Serialize)]
pub struct PrDetailResponse {
    pub pull_request: PullRequestResponse,
    pub threads: Vec<ThreadResponse>,
    pub commits: Vec<CommitRow>,
    pub check_runs: Vec<CheckRunResponse>,
    pub previous_viewed_at: Option<String>,
    pub reviews: Vec<ReviewResponse>,
    pub labels: Vec<LabelResponse>,
}

#[derive(Debug, Serialize)]
pub struct CheckRunResponse {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
}

fn build_threads(comments: Vec<CommentRow>) -> Vec<ThreadResponse> {
    let mut map: BTreeMap<String, Vec<CommentRow>> = BTreeMap::new();
    for c in comments {
        let tid = c
            .thread_id
            .clone()
            .unwrap_or_else(|| format!("orphan:{}", c.id));
        map.entry(tid).or_default().push(c);
    }
    map.into_iter()
        .map(|(thread_id, comments)| {
            let path = comments.iter().find_map(|c| c.path.clone());
            let resolved = comments.first().map(|c| c.resolved).unwrap_or(false);
            let comments = comments
                .into_iter()
                .map(|c| {
                    let body_html = render_markdown(&c.body);
                    CommentResponse {
                        inner: c,
                        body_html,
                    }
                })
                .collect();
            ThreadResponse {
                thread_id,
                path,
                resolved,
                comments,
            }
        })
        .collect()
}

/// GET /api/pull-requests/:owner/:repo/:number
pub async fn get_pr(
    State(state): State<AppState>,
    Path((owner, repo, number)): Path<(String, String, i64)>,
) -> Result<Json<PrDetailResponse>, AppError> {
    let full_repo = format!("{owner}/{repo}");

    // Fetch and cache from GitHub (handles throttle internally).
    let fetch_result =
        fetch_and_cache_pr(&state.pool, &state.github, &owner, &repo, number).await?;

    // Read PR from DB first to capture the previous last_viewed_at.
    let pr = queries::get_pull_request(&state.pool, &full_repo, number)
        .await?
        .ok_or_else(|| AppError::Database(sqlx::Error::RowNotFound))?;

    let previous_viewed_at = pr.last_viewed_at.clone();

    // Now update last_viewed_at to mark the current visit.
    queries::update_last_viewed_at(&state.pool, number).await?;

    // fetch_result carries fresher author/status when we just fetched; fall back to the DB row.
    let (author, pr_status) = match fetch_result {
        Some(ref r) => (r.author.clone(), r.pr_status.clone()),
        None => (pr.author.clone(), derive_pr_status_from_row(&pr)),
    };

    // Send SSE so the inbox list immediately reflects 0 new activity and correct metadata.
    let teams: Option<Vec<String>> = pr
        .teams
        .as_deref()
        .and_then(|json| serde_json::from_str(json).ok());
    let _ = state.tx.send(SyncEvent::PrInfoUpdated(PrInfoUpdatedData {
        pr_id: number,
        repository: full_repo.clone(),
        author,
        pr_status,
        ci_status: pr.ci_status.clone(),
        new_commits: Some(0),
        new_comments: Some(vec![]),
        new_reviews: Some(vec![]),
        teams,
    }));

    let labels: Vec<LabelResponse> =
        serde_json::from_str(&pr.labels).map_err(|e| AppError::Internal(e.to_string()))?;

    let body_html = render_markdown(&pr.body);
    let pull_request = PullRequestResponse {
        inner: pr,
        body_html,
    };

    let threads = build_threads(queries::query_comments_for_pr(&state.pool, number).await?);
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

    let reviews: Vec<ReviewResponse> = queries::query_reviews_for_pr(&state.pool, number)
        .await?
        .into_iter()
        .map(|r| ReviewResponse {
            id: r.id,
            reviewer: r.reviewer,
            reviewer_avatar_url: r.reviewer_avatar_url,
            state: r.state,
            body: r.body,
            submitted_at: r.submitted_at,
            html_url: r.html_url,
        })
        .collect();

    Ok(Json(PrDetailResponse {
        pull_request,
        threads,
        commits,
        check_runs,
        previous_viewed_at,
        reviews,
        labels,
    }))
}
