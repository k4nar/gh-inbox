use sqlx::SqlitePool;
use tokio::sync::broadcast::Sender;

use crate::db::queries;
use crate::github;
use crate::models::{PrTeamsUpdatedData, SyncEvent};

/// Ensure user_teams cache is fresh (24-hour TTL).
pub async fn ensure_user_teams_fresh(
    pool: &SqlitePool,
    github: &github::GithubClient,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let last_fetched = queries::get_last_fetched_epoch(pool, "user_teams")
        .await
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs() as i64;
    if last_fetched.map(|t| now_secs - t > 86_400).unwrap_or(true) {
        let user_teams = crate::github::fetch_user_teams(github).await?;
        queries::replace_user_teams(pool, &user_teams).await?;
    }
    Ok(())
}

/// Fetch reviewer teams for a single PR, update the DB, and fire an SSE event.
/// Assumes user_teams are already fresh in the DB.
pub async fn fetch_teams_for_pr(
    pool: &SqlitePool,
    github: &github::GithubClient,
    tx: &Sender<SyncEvent>,
    pr_id: i64,
    repository: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let parts: Vec<&str> = repository.splitn(2, '/').collect();
    if parts.len() != 2 {
        return Ok(());
    }
    let (owner, repo_name) = (parts[0], parts[1]);

    let user_teams: std::collections::HashSet<String> = queries::get_all_user_teams(pool)
        .await?
        .into_iter()
        .collect();

    let reviewer_teams =
        crate::github::fetch_requested_reviewer_teams(github, owner, repo_name, pr_id).await?;

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

    Ok(())
}
