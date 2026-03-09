CREATE TABLE notifications (
    id          TEXT PRIMARY KEY NOT NULL,
    pr_id       INTEGER,
    title       TEXT NOT NULL DEFAULT '',
    repository  TEXT NOT NULL DEFAULT '',
    reason      TEXT NOT NULL,
    unread      BOOLEAN NOT NULL DEFAULT 1,
    archived    BOOLEAN NOT NULL DEFAULT 0,
    updated_at  TEXT NOT NULL
);
