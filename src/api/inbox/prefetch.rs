use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
use sqlx::SqlitePool;
use tokio::sync::broadcast::Sender;

use crate::api::AppError;
use crate::api::inbox::teams::{ensure_user_teams_fresh, fetch_teams_for_pr};
use crate::api::pull_requests::fetch::fetch_and_cache_pr_meta;
use crate::db::queries;
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
    let client = state.client.clone();
    let token = state.token.clone();
    let base_url = state.github_base_url.clone();
    let tx = state.tx.clone();

    tokio::spawn(async move {
        do_prefetch(&pool, &client, &token, &base_url, &tx, req.items).await;
    });

    Ok(StatusCode::ACCEPTED)
}

async fn do_prefetch(
    pool: &SqlitePool,
    client: &reqwest::Client,
    token: &Arc<str>,
    base_url: &str,
    tx: &Sender<SyncEvent>,
    items: Vec<PrefetchItem>,
) {
    // Ensure user teams are fresh once for the entire batch instead of once per claimed PR,
    // avoiding N sequential DB reads when many rows are visible.
    if let Err(e) = ensure_user_teams_fresh(pool, client, token, base_url).await {
        eprintln!("prefetch: could not refresh user teams: {e}");
        // Non-fatal — continue; team badges may be stale but PR info still fetches.
    }

    for item in items {
        if let Err(e) = fetch_one(pool, client, token, base_url, tx, &item).await {
            eprintln!(
                "prefetch error for {}/#{}: {e}",
                item.repository, item.pr_number
            );
            // Continue to next item — one failure must not abort the batch.
        }
    }
}

async fn fetch_one(
    pool: &SqlitePool,
    client: &reqwest::Client,
    token: &Arc<str>,
    base_url: &str,
    tx: &Sender<SyncEvent>,
    item: &PrefetchItem,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let parts: Vec<&str> = item.repository.splitn(2, '/').collect();
    if parts.len() != 2 {
        return Ok(());
    }
    let (owner, repo_name) = (parts[0], parts[1]);

    // fetch_and_cache_pr_meta uses ETags — no time-based throttle needed.
    let fetch_result = fetch_and_cache_pr_meta(
        pool,
        client,
        token,
        base_url,
        owner,
        repo_name,
        item.pr_number,
    )
    .await
    .map_err(|e| format!("{e:?}"))?;

    let (author, pr_status) = match fetch_result {
        Some(r) => (r.author, r.pr_status),
        None => return Ok(()), // PR not in DB and 304 received — nothing to broadcast.
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

    let _ = tx.send(SyncEvent::PrInfoUpdated(PrInfoUpdatedData {
        pr_id: item.pr_number,
        repository: item.repository.clone(),
        author,
        pr_status,
        new_commits,
        new_comments,
    }));

    // Fetch teams for this PR now that the PR row exists.
    // set_teams_fetching atomically claims the row (NULL → 'fetching'); if it returns
    // this id, we own the fetch. If the row was already 'fetching' or has teams, skip.
    let claimed = queries::set_teams_fetching(pool, &[item.pr_number])
        .await
        .map_err(|e| format!("{e:?}"))?;
    if claimed.contains(&item.pr_number) {
        // ensure_user_teams_fresh was already called once in do_prefetch.
        if let Err(e) = fetch_teams_for_pr(
            pool,
            client,
            token,
            base_url,
            tx,
            item.pr_number,
            &item.repository,
        )
        .await
        {
            eprintln!(
                "prefetch team error for {}/#{}: {e}",
                item.repository, item.pr_number
            );
            // Reset so the next inbox load can retry.
            let _ = sqlx::query(
                "UPDATE pull_requests SET teams = NULL WHERE id = ? AND teams = 'fetching'",
            )
            .bind(item.pr_number)
            .execute(pool)
            .await;
        }
    }

    Ok(())
}
