CREATE TABLE commits (
    sha          TEXT PRIMARY KEY NOT NULL,
    pr_id        INTEGER NOT NULL,
    message      TEXT NOT NULL,
    author       TEXT NOT NULL,
    committed_at TEXT NOT NULL
);
