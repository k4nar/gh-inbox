mod check_runs;
mod commits;
mod notifications;
mod pull_requests;
pub mod sync;
mod teams;

pub use check_runs::fetch_check_runs;
pub use commits::fetch_commits;
pub use notifications::fetch_notifications;
pub use pull_requests::{
    fetch_issue_comments, fetch_pull_request, fetch_pull_request_conditional, fetch_review_comments,
};
pub use teams::{fetch_requested_reviewer_teams, fetch_user_teams};

pub const GITHUB_API_BASE: &str = "https://api.github.com";

fn github_request(client: &reqwest::Client, token: &str, url: &str) -> reqwest::RequestBuilder {
    client
        .get(url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "gh-inbox")
        .header("X-GitHub-Api-Version", "2026-03-10")
}

/// Result of a conditional HTTP request using `If-None-Match`.
#[must_use]
pub enum ConditionalResponse<T> {
    /// Server returned 304 — resource unchanged, use cached data.
    NotModified,
    /// Server returned 200 — fresh data and (optionally) a new ETag.
    Modified { data: T, etag: Option<String> },
}
