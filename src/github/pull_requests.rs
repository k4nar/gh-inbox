use crate::models::{GithubIssueComment, GithubPullRequest, GithubReviewComment};

pub async fn fetch_pull_request(
    token: &str,
    client: &reqwest::Client,
    base_url: &str,
    owner: &str,
    repo: &str,
    number: i64,
) -> Result<GithubPullRequest, reqwest::Error> {
    let url = format!("{base_url}/repos/{owner}/{repo}/pulls/{number}");
    super::send_github_request(super::github_request(client, token, &url), "GET", &url)
        .await?
        .error_for_status()?
        .json()
        .await
}

pub async fn fetch_issue_comments(
    token: &str,
    client: &reqwest::Client,
    base_url: &str,
    owner: &str,
    repo: &str,
    number: i64,
) -> Result<Vec<GithubIssueComment>, reqwest::Error> {
    let url = format!("{base_url}/repos/{owner}/{repo}/issues/{number}/comments");
    super::send_github_request(super::github_request(client, token, &url), "GET", &url)
        .await?
        .error_for_status()?
        .json()
        .await
}

pub async fn fetch_review_comments(
    token: &str,
    client: &reqwest::Client,
    base_url: &str,
    owner: &str,
    repo: &str,
    number: i64,
) -> Result<Vec<GithubReviewComment>, reqwest::Error> {
    let url = format!("{base_url}/repos/{owner}/{repo}/pulls/{number}/comments");
    super::send_github_request(super::github_request(client, token, &url), "GET", &url)
        .await?
        .error_for_status()?
        .json()
        .await
}

#[cfg(test)]
mod tests {
    use crate::models::{GithubIssueComment, GithubPullRequest, GithubReviewComment};

    fn parse_pull_request(json: &str) -> Result<GithubPullRequest, serde_json::Error> {
        serde_json::from_str(json)
    }

    fn parse_issue_comments(json: &str) -> Result<Vec<GithubIssueComment>, serde_json::Error> {
        serde_json::from_str(json)
    }

    fn parse_review_comments(json: &str) -> Result<Vec<GithubReviewComment>, serde_json::Error> {
        serde_json::from_str(json)
    }

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

    #[test]
    fn parse_valid_issue_comments() {
        let json = r#"[{
            "id": 100, "user": { "login": "bob" },
            "body": "Looks good!", "created_at": "2025-01-01T00:00:00Z",
            "html_url": "https://github.com/owner/repo/pull/42#issuecomment-100"
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

    #[test]
    fn parse_valid_review_comments() {
        let json = r#"[{
            "id": 200, "user": { "login": "carol" },
            "body": "Nit: rename this", "created_at": "2025-01-01T00:00:00Z",
            "path": "src/main.rs", "position": 10,
            "in_reply_to_id": null, "pull_request_review_id": 50,
            "html_url": "https://github.com/owner/repo/pull/42#discussion_r200"
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
            "in_reply_to_id": 200, "pull_request_review_id": 51,
            "html_url": "https://github.com/owner/repo/pull/42#discussion_r201"
        }]"#;
        let comments = parse_review_comments(json).unwrap();
        assert_eq!(comments[0].in_reply_to_id, Some(200));
    }

    #[test]
    fn parse_empty_review_comments() {
        let comments = parse_review_comments("[]").unwrap();
        assert!(comments.is_empty());
    }
}
