use crate::models::{
    GithubCheckRunList, GithubIssueComment, GithubPullRequest, GithubReviewComment, Notification,
};

/// Parse a JSON string from the GitHub notifications API into typed structs.
pub fn parse_notifications(json: &str) -> Result<Vec<Notification>, serde_json::Error> {
    serde_json::from_str(json)
}

pub fn parse_pull_request(json: &str) -> Result<GithubPullRequest, serde_json::Error> {
    serde_json::from_str(json)
}

pub fn parse_issue_comments(json: &str) -> Result<Vec<GithubIssueComment>, serde_json::Error> {
    serde_json::from_str(json)
}

pub fn parse_review_comments(json: &str) -> Result<Vec<GithubReviewComment>, serde_json::Error> {
    serde_json::from_str(json)
}

pub fn parse_check_runs(json: &str) -> Result<GithubCheckRunList, serde_json::Error> {
    serde_json::from_str(json)
}

pub const GITHUB_API_BASE: &str = "https://api.github.com";

/// Helper to build an authenticated GitHub API request.
fn github_request(client: &reqwest::Client, token: &str, url: &str) -> reqwest::RequestBuilder {
    client
        .get(url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "gh-inbox")
        .header("X-GitHub-Api-Version", "2022-11-28")
}

/// Fetch notifications from the GitHub REST API.
pub async fn fetch_notifications(
    token: &str,
    client: &reqwest::Client,
    base_url: &str,
) -> Result<Vec<Notification>, reqwest::Error> {
    let url = format!("{base_url}/notifications");
    github_request(client, token, &url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
}

/// Fetch a pull request from the GitHub REST API.
pub async fn fetch_pull_request(
    token: &str,
    client: &reqwest::Client,
    base_url: &str,
    owner: &str,
    repo: &str,
    number: i64,
) -> Result<GithubPullRequest, reqwest::Error> {
    let url = format!("{base_url}/repos/{owner}/{repo}/pulls/{number}");
    github_request(client, token, &url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
}

/// Fetch issue comments (top-level conversation) for a PR.
pub async fn fetch_issue_comments(
    token: &str,
    client: &reqwest::Client,
    base_url: &str,
    owner: &str,
    repo: &str,
    number: i64,
) -> Result<Vec<GithubIssueComment>, reqwest::Error> {
    let url = format!("{base_url}/repos/{owner}/{repo}/issues/{number}/comments");
    github_request(client, token, &url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
}

/// Fetch review comments (inline on code) for a PR.
pub async fn fetch_review_comments(
    token: &str,
    client: &reqwest::Client,
    base_url: &str,
    owner: &str,
    repo: &str,
    number: i64,
) -> Result<Vec<GithubReviewComment>, reqwest::Error> {
    let url = format!("{base_url}/repos/{owner}/{repo}/pulls/{number}/comments");
    github_request(client, token, &url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
}

/// Fetch check runs (CI status) for a given commit SHA.
pub async fn fetch_check_runs(
    token: &str,
    client: &reqwest::Client,
    base_url: &str,
    owner: &str,
    repo: &str,
    sha: &str,
) -> Result<GithubCheckRunList, reqwest::Error> {
    let url = format!("{base_url}/repos/{owner}/{repo}/commits/{sha}/check-runs");
    github_request(client, token, &url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_NOTIFICATION: &str = r#"[
        {
            "id": "1",
            "reason": "review_requested",
            "unread": true,
            "updated_at": "2025-01-01T00:00:00Z",
            "subject": {
                "title": "Fix bug in parser",
                "url": "https://api.github.com/repos/owner/repo/pulls/42",
                "type": "PullRequest"
            },
            "repository": {
                "full_name": "owner/repo"
            }
        }
    ]"#;

    #[test]
    fn parse_valid_notification() {
        let result = parse_notifications(VALID_NOTIFICATION).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "1");
        assert_eq!(result[0].reason, "review_requested");
        assert!(result[0].unread);
        assert_eq!(result[0].subject.title, "Fix bug in parser");
        assert_eq!(result[0].subject.subject_type, "PullRequest");
        assert_eq!(result[0].repository.full_name, "owner/repo");
    }

    #[test]
    fn parse_empty_array() {
        let result = parse_notifications("[]").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn parse_missing_fields() {
        let json = r#"[{"id": "1"}]"#;
        let result = parse_notifications(json);
        assert!(result.is_err());
    }

    #[test]
    fn parse_malformed_json() {
        let result = parse_notifications("not json");
        assert!(result.is_err());
    }

    #[test]
    fn parse_null_subject_url() {
        let json = r#"[
            {
                "id": "2",
                "reason": "mention",
                "unread": false,
                "updated_at": "2025-06-01T12:00:00Z",
                "subject": {
                    "title": "Release v1.0",
                    "url": null,
                    "type": "Release"
                },
                "repository": {
                    "full_name": "org/project"
                }
            }
        ]"#;
        let result = parse_notifications(json).unwrap();
        assert_eq!(result.len(), 1);
        assert!(result[0].subject.url.is_none());
    }

    #[test]
    fn parse_multiple_notifications() {
        let json = r#"[
            {
                "id": "1",
                "reason": "review_requested",
                "unread": true,
                "updated_at": "2025-01-01T00:00:00Z",
                "subject": { "title": "PR 1", "url": null, "type": "PullRequest" },
                "repository": { "full_name": "a/b" }
            },
            {
                "id": "2",
                "reason": "mention",
                "unread": false,
                "updated_at": "2025-01-02T00:00:00Z",
                "subject": { "title": "Issue 1", "url": null, "type": "Issue" },
                "repository": { "full_name": "c/d" }
            }
        ]"#;
        let result = parse_notifications(json).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "1");
        assert_eq!(result[1].id, "2");
    }

    // --- Pull request parse tests ---

    const VALID_PR: &str = r#"{
        "number": 42,
        "title": "Fix bug in parser",
        "body": "This fixes the parser bug.",
        "state": "open",
        "user": { "login": "alice" },
        "html_url": "https://github.com/owner/repo/pull/42",
        "head": { "sha": "abc123" },
        "additions": 10,
        "deletions": 3,
        "changed_files": 2
    }"#;

    #[test]
    fn parse_valid_pull_request() {
        let pr = parse_pull_request(VALID_PR).unwrap();
        assert_eq!(pr.number, 42);
        assert_eq!(pr.title, "Fix bug in parser");
        assert_eq!(pr.body, Some("This fixes the parser bug.".to_string()));
        assert_eq!(pr.state, "open");
        assert_eq!(pr.user.login, "alice");
        assert_eq!(pr.head.sha, "abc123");
        assert_eq!(pr.additions, Some(10));
    }

    #[test]
    fn parse_pr_with_null_body() {
        let json = r#"{
            "number": 1, "title": "T", "body": null, "state": "open",
            "user": { "login": "u" }, "html_url": "h",
            "head": { "sha": "s" }, "additions": null, "deletions": null, "changed_files": null
        }"#;
        let pr = parse_pull_request(json).unwrap();
        assert!(pr.body.is_none());
        assert!(pr.additions.is_none());
    }

    #[test]
    fn parse_pr_missing_fields() {
        let json = r#"{"number": 1}"#;
        assert!(parse_pull_request(json).is_err());
    }

    // --- Issue comment parse tests ---

    #[test]
    fn parse_valid_issue_comments() {
        let json = r#"[{
            "id": 100, "user": { "login": "bob" },
            "body": "Looks good!", "created_at": "2025-01-01T00:00:00Z"
        }]"#;
        let comments = parse_issue_comments(json).unwrap();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].id, 100);
        assert_eq!(comments[0].user.login, "bob");
        assert_eq!(comments[0].body, "Looks good!");
    }

    #[test]
    fn parse_empty_issue_comments() {
        let comments = parse_issue_comments("[]").unwrap();
        assert!(comments.is_empty());
    }

    // --- Review comment parse tests ---

    #[test]
    fn parse_valid_review_comments() {
        let json = r#"[{
            "id": 200, "user": { "login": "carol" },
            "body": "Nit: rename this", "created_at": "2025-01-01T00:00:00Z",
            "path": "src/main.rs", "position": 10,
            "in_reply_to_id": null, "pull_request_review_id": 50
        }]"#;
        let comments = parse_review_comments(json).unwrap();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].path, "src/main.rs");
        assert_eq!(comments[0].position, Some(10));
        assert!(comments[0].in_reply_to_id.is_none());
    }

    #[test]
    fn parse_review_comment_with_reply() {
        let json = r#"[{
            "id": 201, "user": { "login": "dave" },
            "body": "Done!", "created_at": "2025-01-02T00:00:00Z",
            "path": "src/main.rs", "position": 10,
            "in_reply_to_id": 200, "pull_request_review_id": 51
        }]"#;
        let comments = parse_review_comments(json).unwrap();
        assert_eq!(comments[0].in_reply_to_id, Some(200));
    }

    #[test]
    fn parse_empty_review_comments() {
        let comments = parse_review_comments("[]").unwrap();
        assert!(comments.is_empty());
    }

    // --- Check run parse tests ---

    #[test]
    fn parse_valid_check_runs() {
        let json = r#"{
            "total_count": 2,
            "check_runs": [
                { "id": 1, "name": "CI", "status": "completed", "conclusion": "success" },
                { "id": 2, "name": "Lint", "status": "in_progress", "conclusion": null }
            ]
        }"#;
        let result = parse_check_runs(json).unwrap();
        assert_eq!(result.total_count, 2);
        assert_eq!(result.check_runs.len(), 2);
        assert_eq!(result.check_runs[0].conclusion, Some("success".to_string()));
        assert!(result.check_runs[1].conclusion.is_none());
    }

    #[test]
    fn parse_empty_check_runs() {
        let json = r#"{ "total_count": 0, "check_runs": [] }"#;
        let result = parse_check_runs(json).unwrap();
        assert_eq!(result.total_count, 0);
        assert!(result.check_runs.is_empty());
    }
}
