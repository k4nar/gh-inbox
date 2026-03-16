use std::collections::BTreeMap;

use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;

use crate::api::AppError;
use crate::db::queries::{self, CommentRow};
use crate::markdown::render_markdown;
use crate::server::AppState;

use super::CommentResponse;

/// A thread of comments grouped by thread_id.
#[derive(Debug, Serialize)]
pub struct ThreadResponse {
    pub thread_id: String,
    pub path: Option<String>,
    pub comments: Vec<CommentResponse>,
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
                comments,
            }
        })
        .collect();

    Ok(Json(result))
}
