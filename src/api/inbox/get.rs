use std::sync::Arc;
use std::sync::atomic::Ordering;

use axum::Json;
use axum::extract::{Query, State};
use serde::Deserialize;
use sqlx::SqlitePool;
use tokio::sync::broadcast::Sender;

use crate::api::AppError;
use crate::db::queries::{self, InboxItem};
use crate::github::sync::sync_notifications;
use crate::models::{PrTeamsUpdatedData, SyncEvent};
use crate::server::AppState;

#[derive(Deserialize)]
pub struct InboxQuery {
    pub status: Option<String>,
}

/// GET /api/inbox — return enriched notifications, spawn async team fetch if needed.
pub async fn get_inbox(
    State(state): State<AppState>,
    Query(query): Query<InboxQuery>,
) -> Result<Json<Vec<InboxItem>>, AppError> {
    // Bootstrap on first request
    if state
        .bootstrap_done
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        let has_fetched = queries::get_last_fetched_epoch(&state.pool, "notifications")
            .await
            .map_err(AppError::Database)?
            .is_some();
        if !has_fetched {
            sync_notifications(&state).await?;
        }
    }

    let items = match query.status.as_deref() {
        Some("archived") => queries::query_archived_enriched(&state.pool).await?,
        _ => queries::query_inbox_enriched(&state.pool).await?,
    };

    // Collect PR ids where teams is None (covers both NULL and 'fetching' in DB).
    // set_teams_fetching only transitions NULL → 'fetching', so 'fetching' rows are skipped atomically.
    let teams_none_ids: Vec<i64> = items
        .iter()
        .filter(|item| item.teams.is_none() && item.pr_id.is_some())
        .filter_map(|item| item.pr_id)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    if !teams_none_ids.is_empty() {
        let claimed = queries::set_teams_fetching(&state.pool, &teams_none_ids)
            .await
            .map_err(AppError::Database)?;
        if !claimed.is_empty() {
            let repo_map: std::collections::HashMap<i64, String> = items
                .iter()
                .filter_map(|item| item.pr_id.map(|id| (id, item.repository.clone())))
                .collect();
            let pool = state.pool.clone();
            let client = state.client.clone();
            let token = state.token.clone();
            let base_url = state.github_base_url.clone();
            let tx = state.tx.clone();
            tokio::spawn(fetch_teams_background(
                pool, client, token, base_url, tx, claimed, repo_map,
            ));
        }
    }

    Ok(Json(items))
}

async fn fetch_teams_background(
    pool: SqlitePool,
    client: reqwest::Client,
    token: Arc<str>,
    base_url: String,
    tx: Sender<SyncEvent>,
    pr_ids: Vec<i64>,
    repo_map: std::collections::HashMap<i64, String>,
) {
    if let Err(e) = do_fetch_teams(&pool, &client, &token, &base_url, &tx, &pr_ids, &repo_map).await
    {
        eprintln!("team fetch error: {e}");
        // Reset 'fetching' rows back to NULL so the next inbox load retries
        for id in &pr_ids {
            let _ = sqlx::query(
                "UPDATE pull_requests SET teams = NULL WHERE id = ? AND teams = 'fetching'",
            )
            .bind(id)
            .execute(&pool)
            .await;
        }
    }
}

async fn do_fetch_teams(
    pool: &SqlitePool,
    client: &reqwest::Client,
    token: &str,
    base_url: &str,
    tx: &Sender<SyncEvent>,
    pr_ids: &[i64],
    repo_map: &std::collections::HashMap<i64, String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use crate::github;

    // Ensure user_teams is fresh (24h TTL)
    let last_fetched = queries::get_last_fetched_epoch(pool, "user_teams")
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs() as i64;
    let needs_refresh = last_fetched.map(|t| now_secs - t > 86_400).unwrap_or(true);

    if needs_refresh {
        let user_teams = github::fetch_user_teams(client, token, base_url).await?;
        queries::replace_user_teams(pool, &user_teams).await?;
    }

    let user_teams: std::collections::HashSet<String> = queries::get_all_user_teams(pool)
        .await?
        .into_iter()
        .collect();

    for &pr_id in pr_ids {
        let repo = match repo_map.get(&pr_id) {
            Some(r) => r,
            None => continue,
        };
        let parts: Vec<&str> = repo.splitn(2, '/').collect();
        if parts.len() != 2 {
            continue;
        }
        let (owner, repo_name) = (parts[0], parts[1]);

        let reviewer_teams = github::fetch_requested_reviewer_teams(
            client, token, base_url, owner, repo_name, pr_id,
        )
        .await?;

        let matched: Vec<String> = reviewer_teams
            .into_iter()
            .filter(|t| user_teams.contains(t))
            .collect();

        let teams_json = serde_json::to_string(&matched)?;
        queries::update_teams(pool, pr_id, &teams_json).await?;

        let _ = tx.send(SyncEvent::PrTeamsUpdated(PrTeamsUpdatedData {
            pr_id,
            teams: matched,
        }));
    }
    Ok(())
}
