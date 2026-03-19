CREATE TABLE IF NOT EXISTS reviews (
    id           INTEGER PRIMARY KEY,
    pr_id        INTEGER NOT NULL,
    reviewer     TEXT NOT NULL,
    state        TEXT NOT NULL,
    body         TEXT NOT NULL DEFAULT '',
    submitted_at TEXT NOT NULL,
    html_url     TEXT NOT NULL DEFAULT '',
    FOREIGN KEY (pr_id) REFERENCES pull_requests(id)
);
CREATE INDEX IF NOT EXISTS idx_reviews_pr_id ON reviews(pr_id);
