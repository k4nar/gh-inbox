CREATE TABLE comments (
    id          INTEGER PRIMARY KEY NOT NULL,
    pr_id       INTEGER NOT NULL,
    thread_id   TEXT,
    author      TEXT NOT NULL,
    body        TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    FOREIGN KEY (pr_id) REFERENCES pull_requests(id)
);
