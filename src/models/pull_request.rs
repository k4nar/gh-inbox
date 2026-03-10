use serde::{Deserialize, Serialize};

/// A GitHub pull request, parsed from the REST API response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubPullRequest {
    pub number: i64,
    pub title: String,
    pub body: Option<String>,
    pub state: String,
    pub user: GithubUser,
    pub html_url: String,
    pub head: GithubHead,
    pub additions: Option<i64>,
    pub deletions: Option<i64>,
    pub changed_files: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubUser {
    pub login: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubHead {
    pub sha: String,
}

/// A GitHub issue comment (top-level PR conversation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubIssueComment {
    pub id: i64,
    pub user: GithubUser,
    pub body: String,
    pub created_at: String,
}

/// A GitHub pull request review comment (inline on code).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubReviewComment {
    pub id: i64,
    pub user: GithubUser,
    pub body: String,
    pub created_at: String,
    pub path: String,
    pub position: Option<i64>,
    pub in_reply_to_id: Option<i64>,
    pub pull_request_review_id: Option<i64>,
}

/// A GitHub check run (CI status).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubCheckRun {
    pub id: i64,
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
}

/// Wrapper for the check-runs API response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubCheckRunList {
    pub total_count: i64,
    pub check_runs: Vec<GithubCheckRun>,
}
