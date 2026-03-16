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

/// Get the stored ETag for a resource, if any.
pub async fn get_etag(pool: &SqlitePool, resource: &str) -> sqlx::Result<Option<String>> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT etag FROM last_fetched_at WHERE resource = ?")
            .bind(resource)
            .fetch_optional(pool)
            .await?;
    Ok(row.and_then(|r| r.0))
}

/// Upsert the fetch timestamp and ETag for a resource.
pub async fn set_fetch_state(
    pool: &SqlitePool,
    resource: &str,
    etag: Option<&str>,
) -> sqlx::Result<()> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before UNIX epoch")
        .as_secs() as i64;
    sqlx::query(
        "INSERT INTO last_fetched_at (resource, fetched_at, etag)
         VALUES (?, ?, ?)
         ON CONFLICT(resource) DO UPDATE SET fetched_at = excluded.fetched_at, etag = excluded.etag",
    )
    .bind(resource)
    .bind(now)
    .bind(etag)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn test_pool() -> SqlitePool {
        crate::db::init_with_path(":memory:").await
    }

    #[tokio::test]
    async fn returns_none_for_unknown_resource() {
        let pool = test_pool().await;
        let result = get_last_fetched_epoch(&pool, "notifications")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn set_and_get_round_trip() {
        let pool = test_pool().await;
        let before = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        set_last_fetched_now(&pool, "notifications").await.unwrap();

        let fetched = get_last_fetched_epoch(&pool, "notifications")
            .await
            .unwrap()
            .expect("should have a value");
        let after = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        assert!(fetched >= before && fetched <= after);
    }

    #[tokio::test]
    async fn set_twice_does_not_error() {
        let pool = test_pool().await;
        set_last_fetched_now(&pool, "pull_requests").await.unwrap();
        set_last_fetched_now(&pool, "pull_requests").await.unwrap();
        assert!(
            get_last_fetched_epoch(&pool, "pull_requests")
                .await
                .unwrap()
                .is_some()
        );
    }

    #[tokio::test]
    async fn different_resources_are_independent() {
        let pool = test_pool().await;
        set_last_fetched_now(&pool, "notifications").await.unwrap();

        assert!(
            get_last_fetched_epoch(&pool, "pull_requests")
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn get_etag_returns_none_when_not_set() {
        let pool = test_pool().await;
        set_last_fetched_now(&pool, "res").await.unwrap();
        let etag = get_etag(&pool, "res").await.unwrap();
        assert!(etag.is_none());
    }

    #[tokio::test]
    async fn set_and_get_etag_round_trip() {
        let pool = test_pool().await;
        set_fetch_state(&pool, "res", Some("\"abc123\""))
            .await
            .unwrap();
        let etag = get_etag(&pool, "res").await.unwrap();
        assert_eq!(etag.as_deref(), Some("\"abc123\""));
    }

    #[tokio::test]
    async fn set_fetch_state_without_etag_clears_previous_etag() {
        let pool = test_pool().await;
        set_fetch_state(&pool, "res", Some("\"old\""))
            .await
            .unwrap();
        set_fetch_state(&pool, "res", None).await.unwrap();
        let etag = get_etag(&pool, "res").await.unwrap();
        assert!(etag.is_none());
    }
}
