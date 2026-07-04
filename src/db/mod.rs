pub mod queries;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::path::PathBuf;
use std::str::FromStr;

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
    // WAL lets the sync loop, prefetch tasks and request handlers write
    // concurrently with readers; the busy timeout makes contending writers
    // wait instead of failing with SQLITE_BUSY. (In-memory test databases
    // silently ignore WAL.)
    let options = SqliteConnectOptions::from_str(&format!("sqlite:{}", path))
        .expect("invalid database path")
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .busy_timeout(std::time::Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .expect("failed to connect to SQLite database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("failed to run database migrations");

    pool
}
