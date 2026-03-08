use crate::models::Notification;

/// Parse a JSON string from the GitHub notifications API into typed structs.
pub fn parse_notifications(json: &str) -> Result<Vec<Notification>, serde_json::Error> {
    serde_json::from_str(json)
}

/// Fetch notifications from the GitHub REST API.
pub async fn fetch_notifications(
    token: &str,
    client: &reqwest::Client,
) -> Result<Vec<Notification>, reqwest::Error> {
    let response = client
        .get("https://api.github.com/notifications")
        .header("Authorization", format!("Bearer {token}"))
        .header("Accept", "application/vnd.github+json")
        .header("User-Agent", "gh-inbox")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await?
        .error_for_status()?;

    response.json::<Vec<Notification>>().await
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
}
