use sqlx::SqlitePool;

pub async fn get_preference(pool: &SqlitePool, key: &str) -> sqlx::Result<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM user_preferences WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| r.0))
}

pub async fn upsert_preference(pool: &SqlitePool, key: &str, value: &str) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO user_preferences (key, value) VALUES (?, ?)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
    )
    .bind(key)
    .bind(value)
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
    async fn get_preference_returns_none_when_missing() {
        let pool = test_pool().await;
        let val = get_preference(&pool, "theme").await.unwrap();
        assert_eq!(val, None);
    }

    #[tokio::test]
    async fn upsert_and_get_roundtrip() {
        let pool = test_pool().await;
        upsert_preference(&pool, "theme", "dark").await.unwrap();
        let val = get_preference(&pool, "theme").await.unwrap();
        assert_eq!(val, Some("dark".to_string()));
    }

    #[tokio::test]
    async fn upsert_preference_is_idempotent() {
        let pool = test_pool().await;
        upsert_preference(&pool, "theme", "light").await.unwrap();
        upsert_preference(&pool, "theme", "dark").await.unwrap();
        let val = get_preference(&pool, "theme").await.unwrap();
        assert_eq!(val, Some("dark".to_string()));
    }
}
