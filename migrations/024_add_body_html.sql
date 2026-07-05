-- Markdown was rendered (comrak + ammonia) on every GET of the PR detail, for
-- the PR body and every comment. Since the cache is wiped and rewritten from
-- each GitHub snapshot anyway, render once at cache time and store the HTML.
-- Existing rows start empty and are filled by the next fetch of their PR.
ALTER TABLE pull_requests ADD COLUMN body_html TEXT NOT NULL DEFAULT '';
ALTER TABLE comments ADD COLUMN body_html TEXT NOT NULL DEFAULT '';
