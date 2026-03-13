CREATE TABLE commits (
    sha          TEXT PRIMARY KEY NOT NULL,
    pr_id        INTEGER NOT NULL,
    message      TEXT NOT NULL,
    author       TEXT NOT NULL,
    committed_at TEXT NOT NULL,
    FOREIGN KEY (pr_id) REFERENCES pull_requests(id) ON DELETE CASCADE
);
