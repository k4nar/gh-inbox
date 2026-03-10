ALTER TABLE comments ADD COLUMN comment_type TEXT NOT NULL DEFAULT 'issue_comment';
ALTER TABLE comments ADD COLUMN path TEXT;
ALTER TABLE comments ADD COLUMN position INTEGER;
ALTER TABLE comments ADD COLUMN in_reply_to_id INTEGER;
