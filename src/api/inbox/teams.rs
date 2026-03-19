use sqlx::SqlitePool;

use crate::db::queries;
use crate::github;

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
        let user_teams = github::fetch_user_teams(github).await?;
        queries::replace_user_teams(pool, &user_teams).await?;
    }
    Ok(())
}
