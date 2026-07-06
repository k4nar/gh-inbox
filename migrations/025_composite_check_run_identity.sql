-- check_runs.id was a global PRIMARY KEY, but check runs belong to a commit,
-- not a PR: two PRs sharing a head commit report identical real check-run ids,
-- and synthesized StatusContext ids (hashed from the context name) repeat on
-- every PR that carries the same status context. With a global PK the second
-- PR's insert resolved as an UPDATE of the first PR's row — CI results leaked
-- across PRs. Scope row identity per PR, like the commits table.
ALTER TABLE check_runs RENAME TO check_runs_old;

CREATE TABLE check_runs (
    id         INTEGER NOT NULL,  -- GitHub check-run id, or synthesized (negative) for status contexts
    repo       TEXT NOT NULL,
    pr_id      INTEGER NOT NULL,
    name       TEXT NOT NULL,
    status     TEXT NOT NULL,
    conclusion TEXT,
    PRIMARY KEY (repo, pr_id, id),
    FOREIGN KEY (repo, pr_id) REFERENCES pull_requests(repo, id) ON DELETE CASCADE
);

INSERT INTO check_runs (id, repo, pr_id, name, status, conclusion)
SELECT id, repo, pr_id, name, status, conclusion FROM check_runs_old;

DROP TABLE check_runs_old;
