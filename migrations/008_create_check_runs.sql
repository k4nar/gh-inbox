CREATE TABLE check_runs (
    id        INTEGER PRIMARY KEY NOT NULL,
    pr_id     INTEGER NOT NULL,
    name      TEXT NOT NULL,
    status    TEXT NOT NULL,
    conclusion TEXT,
    FOREIGN KEY (pr_id) REFERENCES pull_requests(id) ON DELETE CASCADE
);
