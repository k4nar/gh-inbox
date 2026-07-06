use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use futures::StreamExt;
use serde::Deserialize;
use sqlx::SqlitePool;
use tokio::sync::broadcast::Sender;

use crate::api::AppError;
use crate::api::inbox::teams::ensure_user_teams_fresh;
use crate::github;
use crate::models::SyncEvent;
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
/// Also updates the viewport set so the sync loop can auto-fetch these PRs.
pub async fn post_prefetch(
    State(state): State<AppState>,
    Json(req): Json<PrefetchRequest>,
) -> Result<StatusCode, AppError> {
    if req.items.is_empty() {
        return Ok(StatusCode::ACCEPTED);
    }

    // Update viewport set (replace, not accumulate).
    {
        let new_viewport: std::collections::HashSet<(String, i64)> = req
            .items
            .iter()
            .map(|item| (item.repository.clone(), item.pr_number))
            .collect();
        let mut viewport = state.viewport_prs.write().await;
        *viewport = new_viewport;
    }

    let pool = state.pool.clone();
    let github = state.github.clone();
    let tx = state.tx.clone();

    tokio::spawn(async move {
        do_prefetch(&pool, &github, &tx, req.items).await;
    });

    Ok(StatusCode::ACCEPTED)
}

/// How many PRs to fetch from GitHub at once during a viewport prefetch.
/// A full viewport is ~20 rows: strictly sequential fetches made first-paint
/// enrichment take 20 round-trips, while unbounded parallelism would burst
/// the GitHub API. A small window cuts the latency several-fold.
const PREFETCH_CONCURRENCY: usize = 3;

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

    futures::stream::iter(items)
        .for_each_concurrent(PREFETCH_CONCURRENCY, |item| async move {
            if let Err(e) = fetch_one(pool, github, tx, &item).await {
                tracing::warn!(
                    repository = %item.repository,
                    pr_number = item.pr_number,
                    error = %e,
                    "prefetch error"
                );
                // One failure must not abort the batch.
            }
        })
        .await;
}

async fn fetch_one(
    pool: &SqlitePool,
    github: &github::GithubClient,
    tx: &Sender<SyncEvent>,
    item: &PrefetchItem,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    crate::github::pr_cache::refresh_pr_and_broadcast(
        pool,
        github,
        tx,
        &item.repository,
        item.pr_number,
    )
    .await
}
