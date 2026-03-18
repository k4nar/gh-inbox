use sqlx::SqlitePool;

use crate::api::AppError;
use crate::db::queries::{self, CheckRunRow, CommentRow, CommitRow, PullRequestRow};
use crate::github;
use crate::models::{GithubCheckRun, PrStatus};

/// Minimum seconds between full PR fetches (all 5 endpoints) for the detail view.
/// The inbox prefetch uses ETags instead and has no time-based throttle.
const FETCH_THROTTLE_SECS: i64 = 60;

/// Summary returned after a successful fetch+cache.
pub struct PrFetchResult {
    pub author: String,
    pub pr_status: PrStatus,
}

/// Derive the PR status from its fields.
pub fn derive_pr_status(merged_at: Option<&str>, state: &str, draft: bool) -> PrStatus {
    if merged_at.is_some() {
        PrStatus::Merged
    } else if state == "closed" {
        PrStatus::Closed
    } else if draft {
        PrStatus::Draft
    } else {
        PrStatus::Open
    }
}

/// Derive a PR status from a `PullRequestRow`.
pub fn derive_pr_status_from_row(pr: &PullRequestRow) -> PrStatus {
    derive_pr_status(pr.merged_at.as_deref(), &pr.state, pr.draft)
}

/// Fetch a PR and all related data from GitHub, cache in SQLite, and return a summary.
/// Does NOT update `last_viewed_at`.
/// Returns `None` when the PR was fetched recently (throttled) — caller may still read from DB.
pub async fn fetch_and_cache_pr(
    pool: &SqlitePool,
    github: &github::GithubClient,
    owner: &str,
    repo: &str,
    number: i64,
) -> Result<Option<PrFetchResult>, AppError> {
    let full_repo = format!("{owner}/{repo}");
    let resource_key = format!("pr:{full_repo}#{number}");

    let should_fetch = match queries::get_last_fetched_epoch(pool, &resource_key).await? {
        Some(last) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock before UNIX epoch")
                .as_secs() as i64;
            now - last >= FETCH_THROTTLE_SECS
        }
        None => true,
    };

    if !should_fetch {
        return Ok(None);
    }

    // Fetch PR metadata
    let gh_pr = github::fetch_pull_request(github, owner, repo, number).await?;

    let head_sha = gh_pr.head.sha.clone();
    let author = gh_pr.user.login.clone();
    let pr_status = derive_pr_status(gh_pr.merged_at.as_deref(), &gh_pr.state, gh_pr.draft);

    let pr_row = PullRequestRow {
        id: gh_pr.number,
        title: gh_pr.title,
        repo: full_repo.clone(),
        author: author.clone(),
        url: gh_pr.html_url,
        ci_status: None,
        last_viewed_at: None, // ON CONFLICT clause preserves existing value
        body: gh_pr.body.unwrap_or_default(),
        state: gh_pr.state,
        head_sha: head_sha.clone(),
        additions: gh_pr.additions.unwrap_or(0),
        deletions: gh_pr.deletions.unwrap_or(0),
        changed_files: gh_pr.changed_files.unwrap_or(0),
        draft: gh_pr.draft,
        merged_at: gh_pr.merged_at,
        teams: None,                // ON CONFLICT clause preserves existing teams value
        labels: String::from("[]"), // TODO Task 5: populate from gh_pr.labels
    };
    queries::upsert_pull_request(pool, &pr_row).await?;

    // Fetch issue comments (top-level conversation)
    let issue_comments = github::fetch_issue_comments(github, owner, repo, number).await?;
    for c in &issue_comments {
        let row = CommentRow {
            id: c.id,
            pr_id: number,
            thread_id: Some("conversation".to_string()),
            author: c.user.login.clone(),
            body: c.body.clone(),
            created_at: c.created_at.clone(),
            comment_type: "issue_comment".to_string(),
            path: None,
            position: None,
            in_reply_to_id: None,
            html_url: Some(c.html_url.clone()),
            diff_hunk: None,
        };
        queries::upsert_comment(pool, &row).await?;
    }

    // Fetch review comments (inline on code)
    let review_comments = github::fetch_review_comments(github, owner, repo, number).await?;
    for c in &review_comments {
        let thread_id = match c.in_reply_to_id {
            Some(parent_id) => format!("review:{parent_id}"),
            None => format!("review:{}", c.id),
        };
        let row = CommentRow {
            id: c.id,
            pr_id: number,
            thread_id: Some(thread_id),
            author: c.user.login.clone(),
            body: c.body.clone(),
            created_at: c.created_at.clone(),
            comment_type: "review_comment".to_string(),
            path: Some(c.path.clone()),
            position: c.position,
            in_reply_to_id: c.in_reply_to_id,
            html_url: Some(c.html_url.clone()),
            diff_hunk: c.diff_hunk.clone(),
        };
        queries::upsert_comment(pool, &row).await?;
    }

    // Fetch commits
    let gh_commits = github::fetch_commits(github, owner, repo, number).await?;
    for c in &gh_commits {
        let first_line = c.commit.message.lines().next().unwrap_or("").to_string();
        let row = CommitRow {
            sha: c.sha.clone(),
            pr_id: number,
            message: first_line,
            author: c.commit.author.name.clone(),
            committed_at: c.commit.author.date.clone(),
        };
        queries::upsert_commit(pool, &row).await?;
    }

    // Fetch check runs for the head SHA
    let check_run_list = github::fetch_check_runs(github, owner, repo, &head_sha).await?;

    let ci_status = derive_ci_status(&check_run_list.check_runs);
    if ci_status.is_some() {
        sqlx::query("UPDATE pull_requests SET ci_status = ? WHERE id = ? AND repo = ?")
            .bind(&ci_status)
            .bind(number)
            .bind(&full_repo)
            .execute(pool)
            .await?;
    }

    for cr in &check_run_list.check_runs {
        let row = CheckRunRow {
            id: cr.id,
            pr_id: number,
            name: cr.name.clone(),
            status: cr.status.clone(),
            conclusion: cr.conclusion.clone(),
        };
        queries::upsert_check_run(pool, &row).await?;
    }

    queries::set_last_fetched_now(pool, &resource_key).await?;

    Ok(Some(PrFetchResult { author, pr_status }))
}

pub fn derive_ci_status(check_runs: &[GithubCheckRun]) -> Option<String> {
    if check_runs.is_empty() {
        return None;
    }
    let mut has_failure = false;
    let mut has_pending = false;
    for cr in check_runs {
        if cr.status != "completed" {
            has_pending = true;
        } else if cr.conclusion.as_deref() != Some("success")
            && cr.conclusion.as_deref() != Some("skipped")
            && cr.conclusion.as_deref() != Some("neutral")
        {
            has_failure = true;
        }
    }
    Some(if has_failure {
        "failure".to_string()
    } else if has_pending {
        "pending".to_string()
    } else {
        "success".to_string()
    })
}
