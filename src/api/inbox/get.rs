use std::sync::atomic::Ordering;

use axum::Json;
use axum::extract::{Query, State};
use serde::Deserialize;
use sqlx::SqlitePool;
use tokio::sync::broadcast::Sender;

use crate::api::AppError;
use crate::api::inbox::teams::{ensure_user_teams_fresh, fetch_teams_for_pr};
use crate::db::queries::{self, InboxItem};
use crate::github;
use crate::github::sync::sync_notifications;
use crate::models::SyncEvent;
use crate::server::AppState;

#[derive(Deserialize)]
pub struct InboxQuery {
    pub status: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}

#[derive(serde::Serialize)]
pub struct PaginatedInbox {
    pub items: Vec<InboxItem>,
    pub total: i64,
    pub page: u32,
    pub per_page: u32,
}

/// GET /api/inbox — return enriched notifications, spawn async team fetch if needed.
pub async fn get_inbox(
    State(state): State<AppState>,
    Query(query): Query<InboxQuery>,
) -> Result<Json<PaginatedInbox>, AppError> {
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

    let page = query.page.unwrap_or(1).max(1);
    let per_page = query.per_page.unwrap_or(25).clamp(1, 100);
    let offset = (page - 1) * per_page;

    let (items, total) = match query.status.as_deref() {
        Some("archived") => {
            queries::query_archived_enriched_paginated(&state.pool, per_page, offset).await?
        }
        _ => queries::query_inbox_enriched_paginated(&state.pool, per_page, offset).await?,
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
            let github = state.github.clone();
            let tx = state.tx.clone();
            tokio::spawn(fetch_teams_background(pool, github, tx, claimed, repo_map));
        }
    }

    Ok(Json(PaginatedInbox {
        items,
        total,
        page,
        per_page,
    }))
}

async fn fetch_teams_background(
    pool: SqlitePool,
    github: github::GithubClient,
    tx: Sender<SyncEvent>,
    pr_ids: Vec<i64>,
    repo_map: std::collections::HashMap<i64, String>,
) {
    if let Err(e) = do_fetch_teams(&pool, &github, &tx, &pr_ids, &repo_map).await {
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
    github: &github::GithubClient,
    tx: &Sender<SyncEvent>,
    pr_ids: &[i64],
    repo_map: &std::collections::HashMap<i64, String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    ensure_user_teams_fresh(pool, github).await?;

    for &pr_id in pr_ids {
        let repo = match repo_map.get(&pr_id) {
            Some(r) => r,
            None => continue,
        };
        fetch_teams_for_pr(pool, github, tx, pr_id, repo).await?;
    }
    Ok(())
}
