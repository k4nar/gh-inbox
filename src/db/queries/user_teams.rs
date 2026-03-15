use sqlx::SqlitePool;

/// Get all stored user team slugs (e.g. "acme/platform").
pub async fn get_all_user_teams(pool: &SqlitePool) -> sqlx::Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as("SELECT slug FROM user_teams ORDER BY slug")
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

/// Replace all user teams atomically (DELETE + INSERT in one transaction).
/// Also updates last_fetched_at for key "user_teams".
pub async fn replace_user_teams(pool: &SqlitePool, slugs: &[String]) -> sqlx::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM user_teams")
        .execute(&mut *tx)
        .await?;
    for slug in slugs {
        sqlx::query("INSERT INTO user_teams (slug) VALUES (?)")
            .bind(slug)
            .execute(&mut *tx)
            .await?;
    }
    // Update last_fetched_at inside same transaction
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before UNIX epoch")
        .as_secs() as i64;
    sqlx::query(
        "INSERT INTO last_fetched_at (resource, fetched_at) VALUES ('user_teams', ?)
         ON CONFLICT(resource) DO UPDATE SET fetched_at = excluded.fetched_at",
    )
    .bind(now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> SqlitePool {
        crate::db::init_with_path(":memory:").await
    }

    #[tokio::test]
    async fn get_empty() {
        let pool = test_pool().await;
        let teams = get_all_user_teams(&pool).await.unwrap();
        assert!(teams.is_empty());
    }

    #[tokio::test]
    async fn replace_and_get_roundtrip() {
        let pool = test_pool().await;
        let slugs = vec!["acme/platform".to_string(), "acme/backend".to_string()];
        replace_user_teams(&pool, &slugs).await.unwrap();
        let result = get_all_user_teams(&pool).await.unwrap();
        assert_eq!(result, vec!["acme/backend", "acme/platform"]); // sorted
    }

    #[tokio::test]
    async fn replace_is_atomic_not_append() {
        let pool = test_pool().await;
        replace_user_teams(&pool, &["acme/old".to_string()])
            .await
            .unwrap();
        replace_user_teams(&pool, &["acme/new".to_string()])
            .await
            .unwrap();
        let result = get_all_user_teams(&pool).await.unwrap();
        assert_eq!(result, vec!["acme/new"]);
    }

    #[tokio::test]
    async fn replace_updates_last_fetched_at() {
        let pool = test_pool().await;
        replace_user_teams(&pool, &["acme/x".to_string()])
            .await
            .unwrap();
        let fetched = crate::db::queries::get_last_fetched_epoch(&pool, "user_teams")
            .await
            .unwrap();
        assert!(fetched.is_some());
    }
}
