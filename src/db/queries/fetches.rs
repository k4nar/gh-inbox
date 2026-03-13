use sqlx::SqlitePool;

/// Get the last fetched epoch (seconds since UNIX epoch) for a resource.
pub async fn get_last_fetched_epoch(
    pool: &SqlitePool,
    resource: &str,
) -> sqlx::Result<Option<i64>> {
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT fetched_at FROM last_fetched_at WHERE resource = ?")
            .bind(resource)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|r| r.0))
}

/// Set the last fetched timestamp for a resource to now (epoch seconds).
pub async fn set_last_fetched_now(pool: &SqlitePool, resource: &str) -> sqlx::Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before UNIX epoch")
        .as_secs() as i64;
    sqlx::query(
        "INSERT INTO last_fetched_at (resource, fetched_at)
         VALUES (?, ?)
         ON CONFLICT(resource) DO UPDATE SET fetched_at = excluded.fetched_at",
    )
    .bind(resource)
    .bind(now)
    .execute(pool)
    .await?;
    Ok(())
}
