-- Whether the PR is currently in its repository's merge queue (GraphQL
-- `isInMergeQueue`). Drives the "queued" status shown instead of "open".
-- Existing rows start at 0 and are corrected by the next fetch of their PR.
ALTER TABLE pull_requests ADD COLUMN in_merge_queue BOOLEAN NOT NULL DEFAULT 0;
