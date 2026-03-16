ALTER TABLE pull_requests ADD COLUMN draft     BOOLEAN NOT NULL DEFAULT 0;
ALTER TABLE pull_requests ADD COLUMN merged_at TEXT;
ALTER TABLE pull_requests ADD COLUMN teams     TEXT;
-- NULL     = team fetch not yet attempted
-- 'fetching' = fetch in progress (concurrency sentinel)
-- '[]'     = fetched, no user-owned teams are requested reviewers
-- '[...]'  = JSON array of matched team slugs e.g. '["acme/platform"]'
