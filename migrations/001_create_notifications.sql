CREATE TABLE notifications (
    id          TEXT PRIMARY KEY NOT NULL,
    pr_id       INTEGER,
    reason      TEXT NOT NULL,
    unread      BOOLEAN NOT NULL DEFAULT 1,
    archived    BOOLEAN NOT NULL DEFAULT 0,
    updated_at  TEXT NOT NULL
);
