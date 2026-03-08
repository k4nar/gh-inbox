pub mod queries;

use sqlx::SqlitePool;
use sqlx::sqlite::SqlitePoolOptions;
use std::path::PathBuf;

/// Returns the path to the SQLite database file in the OS data directory.
fn db_path() -> PathBuf {
    let data_dir = dirs::data_dir().expect("failed to resolve OS data directory");
    let app_dir = data_dir.join("gh-inbox");
    std::fs::create_dir_all(&app_dir).expect("failed to create data directory");
    app_dir.join("db.sqlite")
}

/// Initializes the SQLite database: creates the file if missing and runs migrations.
pub async fn init() -> SqlitePool {
    init_with_path(&db_path().to_string_lossy()).await
}

/// Initializes the SQLite database at the given path. Used by tests to provide a custom path.
pub async fn init_with_path(path: &str) -> SqlitePool {
    let url = format!("sqlite:{}?mode=rwc", path);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("failed to connect to SQLite database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("failed to run database migrations");

    pool
}
