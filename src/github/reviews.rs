// REST fetch functions removed — PR data now comes via GraphQL (fetch_pr_graphql.rs).
// Model type (GithubReview) remains in src/models/pull_request.rs.

#[cfg(test)]
mod tests {
    use crate::models::GithubReview;

    const ALLOWED_STATES: &[&str] = &["APPROVED", "CHANGES_REQUESTED", "DISMISSED"];

    fn parse_reviews(json: &str) -> Result<Vec<GithubReview>, serde_json::Error> {
        let all: Vec<GithubReview> = serde_json::from_str(json)?;
        Ok(all
            .into_iter()
            .filter(|r| ALLOWED_STATES.contains(&r.state.as_str()))
            .collect())
    }

    #[test]
    fn parse_valid_approved_review() {
        let json = r#"[{
            "id": 1,
            "user": { "login": "alice" },
            "state": "APPROVED",
            "body": "Looks good!",
            "submitted_at": "2025-06-01T10:00:00Z",
            "html_url": "https://github.com/owner/repo/pull/42#pullrequestreview-1"
        }]"#;
        let reviews = parse_reviews(json).unwrap();
        assert_eq!(reviews.len(), 1);
        assert_eq!(reviews[0].id, 1);
        assert_eq!(reviews[0].user.login, "alice");
        assert_eq!(reviews[0].state, "APPROVED");
        assert_eq!(reviews[0].body, "Looks good!");
    }

    #[test]
    fn parse_changes_requested_review() {
        let json = r#"[{
            "id": 2,
            "user": { "login": "bob" },
            "state": "CHANGES_REQUESTED",
            "body": "Please fix the tests.",
            "submitted_at": "2025-06-01T11:00:00Z",
            "html_url": "https://github.com/owner/repo/pull/42#pullrequestreview-2"
        }]"#;
        let reviews = parse_reviews(json).unwrap();
        assert_eq!(reviews.len(), 1);
        assert_eq!(reviews[0].state, "CHANGES_REQUESTED");
    }

    #[test]
    fn parse_filters_commented_keeps_dismissed() {
        let json = r#"[
            { "id": 1, "user": { "login": "a" }, "state": "COMMENTED",         "body": "", "submitted_at": "2025-01-01T00:00:00Z", "html_url": "" },
            { "id": 2, "user": { "login": "b" }, "state": "DISMISSED",         "body": "", "submitted_at": "2025-01-01T00:00:00Z", "html_url": "" },
            { "id": 3, "user": { "login": "c" }, "state": "APPROVED",          "body": "", "submitted_at": "2025-01-01T00:00:00Z", "html_url": "" },
            { "id": 4, "user": { "login": "d" }, "state": "CHANGES_REQUESTED", "body": "", "submitted_at": "2025-01-01T00:00:00Z", "html_url": "" }
        ]"#;
        let reviews = parse_reviews(json).unwrap();
        assert_eq!(reviews.len(), 3);
        assert!(reviews.iter().all(|r| r.state != "COMMENTED"));
        assert!(reviews.iter().any(|r| r.state == "DISMISSED"));
    }

    #[test]
    fn parse_empty_reviews() {
        let reviews = parse_reviews("[]").unwrap();
        assert!(reviews.is_empty());
    }

    #[test]
    fn parse_review_with_empty_body() {
        let json = r#"[{
            "id": 5,
            "user": { "login": "eve" },
            "state": "APPROVED",
            "body": "",
            "submitted_at": "2025-06-01T12:00:00Z",
            "html_url": "https://github.com/owner/repo/pull/42#pullrequestreview-5"
        }]"#;
        let reviews = parse_reviews(json).unwrap();
        assert_eq!(reviews.len(), 1);
        assert_eq!(reviews[0].body, "");
    }
}
