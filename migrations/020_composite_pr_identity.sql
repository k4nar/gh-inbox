-- PR numbers are only unique per repository, but pull_requests was keyed by the
-- bare number and every child table joined on it. Two watched repos with the same
-- PR number overwrote each other's cached row and merged their comments, commits,
-- check runs and reviews. Rebuild the tables around the composite identity
-- (repo, number). SQLite cannot alter primary keys, so each table is recreated
-- and its rows copied; child rows get their repo backfilled from the parent row
-- (rows already misattributed by a past collision self-heal on the next fetch).

ALTER TABLE pull_requests RENAME TO pull_requests_old;
ALTER TABLE comments RENAME TO comments_old;
ALTER TABLE commits RENAME TO commits_old;
ALTER TABLE check_runs RENAME TO check_runs_old;
ALTER TABLE reviews RENAME TO reviews_old;

CREATE TABLE pull_requests (
    id                INTEGER NOT NULL,   -- PR number (per-repo)
    title             TEXT NOT NULL,
    repo              TEXT NOT NULL,      -- "owner/name"
    author            TEXT NOT NULL,
    author_avatar_url TEXT,
    url               TEXT NOT NULL,
    ci_status         TEXT,
    last_viewed_at    TEXT,
    body              TEXT NOT NULL DEFAULT '',
    state             TEXT NOT NULL DEFAULT 'open',
    head_sha          TEXT NOT NULL DEFAULT '',
    additions         INTEGER NOT NULL DEFAULT 0,
    deletions         INTEGER NOT NULL DEFAULT 0,
    changed_files     INTEGER NOT NULL DEFAULT 0,
    draft             BOOLEAN NOT NULL DEFAULT 0,
    merged_at         TEXT,
    teams             TEXT,
    labels            TEXT NOT NULL DEFAULT '[]',
    PRIMARY KEY (repo, id)
);

INSERT INTO pull_requests (id, title, repo, author, author_avatar_url, url, ci_status, last_viewed_at, body, state, head_sha, additions, deletions, changed_files, draft, merged_at, teams, labels)
SELECT id, title, repo, author, author_avatar_url, url, ci_status, last_viewed_at, body, state, head_sha, additions, deletions, changed_files, draft, merged_at, teams, labels
FROM pull_requests_old;

CREATE TABLE comments (
    id                INTEGER PRIMARY KEY NOT NULL,  -- GitHub comment id (global)
    repo              TEXT NOT NULL,
    pr_id             INTEGER NOT NULL,
    thread_id         TEXT,
    author            TEXT NOT NULL,
    author_avatar_url TEXT,
    body              TEXT NOT NULL,
    created_at        TEXT NOT NULL,
    comment_type      TEXT NOT NULL DEFAULT 'issue_comment',
    path              TEXT,
    position          INTEGER,
    in_reply_to_id    INTEGER,
    html_url          TEXT,
    diff_hunk         TEXT,
    resolved          INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (repo, pr_id) REFERENCES pull_requests(repo, id) ON DELETE CASCADE
);

INSERT INTO comments (id, repo, pr_id, thread_id, author, author_avatar_url, body, created_at, comment_type, path, position, in_reply_to_id, html_url, diff_hunk, resolved)
SELECT c.id, p.repo, c.pr_id, c.thread_id, c.author, c.author_avatar_url, c.body, c.created_at, c.comment_type, c.path, c.position, c.in_reply_to_id, c.html_url, c.diff_hunk, c.resolved
FROM comments_old c JOIN pull_requests_old p ON p.id = c.pr_id;

CREATE TABLE commits (
    sha          TEXT NOT NULL,
    repo         TEXT NOT NULL,
    pr_id        INTEGER NOT NULL,
    message      TEXT NOT NULL,
    author       TEXT NOT NULL,
    committed_at TEXT NOT NULL,
    PRIMARY KEY (repo, pr_id, sha),
    FOREIGN KEY (repo, pr_id) REFERENCES pull_requests(repo, id) ON DELETE CASCADE
);

INSERT INTO commits (sha, repo, pr_id, message, author, committed_at)
SELECT c.sha, p.repo, c.pr_id, c.message, c.author, c.committed_at
FROM commits_old c JOIN pull_requests_old p ON p.id = c.pr_id;

CREATE TABLE check_runs (
    id         INTEGER PRIMARY KEY NOT NULL,  -- GitHub check run id (global)
    repo       TEXT NOT NULL,
    pr_id      INTEGER NOT NULL,
    name       TEXT NOT NULL,
    status     TEXT NOT NULL,
    conclusion TEXT,
    FOREIGN KEY (repo, pr_id) REFERENCES pull_requests(repo, id) ON DELETE CASCADE
);

INSERT INTO check_runs (id, repo, pr_id, name, status, conclusion)
SELECT c.id, p.repo, c.pr_id, c.name, c.status, c.conclusion
FROM check_runs_old c JOIN pull_requests_old p ON p.id = c.pr_id;

CREATE TABLE reviews (
    id                  INTEGER PRIMARY KEY,  -- GitHub review id (global)
    repo                TEXT NOT NULL,
    pr_id               INTEGER NOT NULL,
    reviewer            TEXT NOT NULL,
    reviewer_avatar_url TEXT,
    state               TEXT NOT NULL,
    body                TEXT NOT NULL DEFAULT '',
    submitted_at        TEXT NOT NULL,
    html_url            TEXT NOT NULL DEFAULT '',
    FOREIGN KEY (repo, pr_id) REFERENCES pull_requests(repo, id) ON DELETE CASCADE
);

INSERT INTO reviews (id, repo, pr_id, reviewer, reviewer_avatar_url, state, body, submitted_at, html_url)
SELECT r.id, p.repo, r.pr_id, r.reviewer, r.reviewer_avatar_url, r.state, r.body, r.submitted_at, r.html_url
FROM reviews_old r JOIN pull_requests_old p ON p.id = r.pr_id;

DROP TABLE comments_old;
DROP TABLE commits_old;
DROP TABLE check_runs_old;
DROP TABLE reviews_old;
DROP TABLE pull_requests_old;

CREATE INDEX idx_comments_repo_pr ON comments(repo, pr_id);
CREATE INDEX idx_check_runs_repo_pr ON check_runs(repo, pr_id);
CREATE INDEX idx_reviews_repo_pr ON reviews(repo, pr_id);
