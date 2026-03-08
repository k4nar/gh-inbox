CREATE TABLE pull_requests (
    id              INTEGER PRIMARY KEY NOT NULL,
    title           TEXT NOT NULL,
    repo            TEXT NOT NULL,
    author          TEXT NOT NULL,
    url             TEXT NOT NULL,
    ci_status       TEXT,
    last_viewed_at  TEXT
);
