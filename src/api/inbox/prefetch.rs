use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
use sqlx::SqlitePool;
use tokio::sync::broadcast::Sender;

use crate::api::AppError;
use crate::api::inbox::teams::ensure_user_teams_fresh;
use crate::api::pull_requests::fetch::{derive_pr_status_from_row, fetch_and_cache_pr};
use crate::db::queries;
use crate::github;
use crate::models::{PrInfoUpdatedData, PrNewComment, SyncEvent};
use crate::server::AppState;

#[derive(Deserialize)]
pub struct PrefetchRequest {
    pub items: Vec<PrefetchItem>,
}

#[derive(Deserialize, Clone)]
pub struct PrefetchItem {
    pub repository: String, // "owner/repo"
    pub pr_number: i64,
}

/// POST /api/inbox/prefetch — spawn background fetch for the listed visible PR items.
/// Returns 202 immediately; results arrive via `pr:info_updated` SSE events.
pub async fn post_prefetch(
    State(state): State<AppState>,
    Json(req): Json<PrefetchRequest>,
) -> Result<StatusCode, AppError> {
    if req.items.is_empty() {
        return Ok(StatusCode::ACCEPTED);
    }

    let pool = state.pool.clone();
    let github = state.github.clone();
    let tx = state.tx.clone();

    tokio::spawn(async move {
        do_prefetch(&pool, &github, &tx, req.items).await;
    });

    Ok(StatusCode::ACCEPTED)
}

async fn do_prefetch(
    pool: &SqlitePool,
    github: &github::GithubClient,
    tx: &Sender<SyncEvent>,
    items: Vec<PrefetchItem>,
) {
    // Ensure user teams are fresh once for the entire batch instead of once per claimed PR,
    // avoiding N sequential DB reads when many rows are visible.
    if let Err(e) = ensure_user_teams_fresh(pool, github).await {
        tracing::warn!(error = %e, "prefetch: could not refresh user teams");
        // Non-fatal — continue; team badges may be stale but PR info still fetches.
    }

    for item in items {
        if let Err(e) = fetch_one(pool, github, tx, &item).await {
            tracing::warn!(
                repository = %item.repository,
                pr_number = item.pr_number,
                error = %e,
                "prefetch error"
            );
            // Continue to next item — one failure must not abort the batch.
        }
    }
}

async fn fetch_one(
    pool: &SqlitePool,
    github: &github::GithubClient,
    tx: &Sender<SyncEvent>,
    item: &PrefetchItem,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let parts: Vec<&str> = item.repository.splitn(2, '/').collect();
    if parts.len() != 2 {
        return Ok(());
    }
    let (owner, repo_name) = (parts[0], parts[1]);

    // Fetch and cache the full PR detail (comments, commits, check runs).
    // Uses a 60s time-based throttle — no-ops if fetched recently.
    // Does NOT update last_viewed_at; that only happens when the user opens the PR.
    let fetch_result = fetch_and_cache_pr(pool, github, owner, repo_name, item.pr_number)
        .await
        .map_err(|e| format!("{e:?}"))?;

    let (author, pr_status) = match fetch_result {
        Some(r) => (r.author, r.pr_status),
        None => {
            // Throttled — data is fresh in SQLite already; read it for the SSE broadcast.
            match queries::get_pull_request(pool, &item.repository, item.pr_number).await? {
                Some(pr) => (pr.author.clone(), derive_pr_status_from_row(&pr)),
                None => return Ok(()), // Not yet in DB — nothing to broadcast.
            }
        }
    };

    // Query actual activity counts (respects last_viewed_at).
    let (new_commits, new_comments_json): (Option<i64>, Option<String>) =
        queries::get_pr_activity(pool, item.pr_number, &item.repository)
            .await
            .map_err(|e| format!("{e:?}"))?;
    let new_comments: Option<Vec<PrNewComment>> = match new_comments_json.as_deref() {
        None => None,
        Some(json) => Some(serde_json::from_str(json)?),
    };

    let new_reviews = queries::get_pr_review_activity(pool, item.pr_number)
        .await
        .map_err(|e| format!("{e:?}"))?;

    let _ = tx.send(SyncEvent::PrInfoUpdated(PrInfoUpdatedData {
        pr_id: item.pr_number,
        repository: item.repository.clone(),
        author,
        pr_status,
        new_commits,
        new_comments,
        new_reviews,
    }));

    Ok(())
}
