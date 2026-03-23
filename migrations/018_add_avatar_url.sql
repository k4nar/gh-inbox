ALTER TABLE pull_requests ADD COLUMN author_avatar_url TEXT;
ALTER TABLE comments ADD COLUMN author_avatar_url TEXT;
ALTER TABLE reviews ADD COLUMN reviewer_avatar_url TEXT;
