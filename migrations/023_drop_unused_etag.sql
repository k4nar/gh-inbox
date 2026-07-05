-- Migration 011 added an etag column for conditional PR fetches that were
-- never implemented (PR data moved to a single GraphQL query, which has no
-- ETag support; fetches are throttled by timestamp instead). No code ever
-- read or wrote the column — drop it.
ALTER TABLE last_fetched_at DROP COLUMN etag;
