CREATE TABLE IF NOT EXISTS last_fetched_at (
    resource    TEXT PRIMARY KEY NOT NULL,
    fetched_at  TEXT NOT NULL
);
