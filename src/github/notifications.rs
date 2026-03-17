use crate::models::Notification;

use super::GithubClient;

pub async fn mark_thread_read(
    github: &GithubClient,
    thread_id: &str,
) -> Result<(), reqwest::Error> {
    let url = github.url(&format!("/notifications/threads/{thread_id}"));
    let response = github.patch(&url).await?;
    let status = response.status();
    if status == 403 || status == 404 {
        return Ok(());
    }
    response.error_for_status()?;
    Ok(())
}

pub async fn mark_thread_done(
    github: &GithubClient,
    thread_id: &str,
) -> Result<(), reqwest::Error> {
    let url = github.url(&format!("/notifications/threads/{thread_id}"));
    let response = github.delete(&url).await?;
    let status = response.status();
    if status == 403 || status == 404 {
        return Ok(());
    }
    response.error_for_status()?;
    Ok(())
}

pub async fn fetch_notifications(
    github: &GithubClient,
) -> Result<Vec<Notification>, reqwest::Error> {
    let url = github.url("/notifications");
    github.get(&url).await?.error_for_status()?.json().await
}

#[cfg(test)]
mod tests {
    use crate::models::Notification;

    fn parse_notifications(json: &str) -> Result<Vec<Notification>, serde_json::Error> {
        serde_json::from_str(json)
    }

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

#[cfg(test)]
mod action_tests {
    use super::*;
    use axum::Router;
    use axum::extract::Request;
    use axum::routing::{delete, patch};
    use std::sync::{Arc, Mutex};
    use tokio::net::TcpListener;

    /// Start a mock server that records the last received request method,
    /// path, and selected headers, then returns the given status code.
    async fn start_mock_recording(
        status: u16,
        route_method: &'static str,
    ) -> (
        String,
        Arc<Mutex<Option<(String, String, std::collections::HashMap<String, String>)>>>,
    ) {
        let recorded = Arc::new(Mutex::new(
            None::<(String, String, std::collections::HashMap<String, String>)>,
        ));
        let recorded_clone = recorded.clone();

        let handler = move |req: Request| {
            let recorded = recorded_clone.clone();
            async move {
                let method = req.method().to_string();
                let path = req.uri().path().to_string();
                let headers: std::collections::HashMap<String, String> = req
                    .headers()
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                    .collect();
                *recorded.lock().unwrap() = Some((method, path, headers));
                axum::http::Response::builder()
                    .status(status)
                    .body(axum::body::Body::empty())
                    .unwrap()
            }
        };

        let app = match route_method {
            "PATCH" => Router::new().route("/notifications/threads/42", patch(handler)),
            "DELETE" => Router::new().route("/notifications/threads/42", delete(handler)),
            _ => panic!("unsupported method"),
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://{addr}");
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        (base, recorded)
    }

    #[tokio::test]
    async fn mark_thread_read_uses_patch_with_correct_headers() {
        let (base, recorded) = start_mock_recording(205, "PATCH").await;
        let github = GithubClient::new(std::sync::Arc::from("test-token"), base);
        mark_thread_read(&github, "42").await.unwrap();
        let (method, path, headers) = recorded.lock().unwrap().clone().unwrap();
        assert_eq!(method, "PATCH");
        assert_eq!(path, "/notifications/threads/42");
        assert_eq!(headers["authorization"], "Bearer test-token");
        assert_eq!(headers["accept"], "application/vnd.github+json");
        assert_eq!(headers["user-agent"], "gh-inbox");
        assert!(headers.contains_key("x-github-api-version"));
    }

    #[tokio::test]
    async fn mark_thread_done_uses_delete_with_correct_headers() {
        let (base, recorded) = start_mock_recording(205, "DELETE").await;
        let github = GithubClient::new(std::sync::Arc::from("test-token"), base);
        mark_thread_done(&github, "42").await.unwrap();
        let (method, path, headers) = recorded.lock().unwrap().clone().unwrap();
        assert_eq!(method, "DELETE");
        assert_eq!(path, "/notifications/threads/42");
        assert_eq!(headers["authorization"], "Bearer test-token");
        assert_eq!(headers["accept"], "application/vnd.github+json");
        assert_eq!(headers["user-agent"], "gh-inbox");
        assert!(headers.contains_key("x-github-api-version"));
    }

    #[tokio::test]
    async fn mark_thread_read_succeeds_on_205() {
        let (base, _) = start_mock_recording(205, "PATCH").await;
        let github = GithubClient::new(std::sync::Arc::from("tok"), base);
        let result = mark_thread_read(&github, "42").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn mark_thread_read_noops_on_404() {
        let (base, _) = start_mock_recording(404, "PATCH").await;
        let github = GithubClient::new(std::sync::Arc::from("tok"), base);
        let result = mark_thread_read(&github, "42").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn mark_thread_read_noops_on_403() {
        let (base, _) = start_mock_recording(403, "PATCH").await;
        let github = GithubClient::new(std::sync::Arc::from("tok"), base);
        let result = mark_thread_read(&github, "42").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn mark_thread_read_errors_on_500() {
        let (base, _) = start_mock_recording(500, "PATCH").await;
        let github = GithubClient::new(std::sync::Arc::from("tok"), base);
        let result = mark_thread_read(&github, "42").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn mark_thread_done_succeeds_on_205() {
        let (base, _) = start_mock_recording(205, "DELETE").await;
        let github = GithubClient::new(std::sync::Arc::from("tok"), base);
        let result = mark_thread_done(&github, "42").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn mark_thread_done_noops_on_404() {
        let (base, _) = start_mock_recording(404, "DELETE").await;
        let github = GithubClient::new(std::sync::Arc::from("tok"), base);
        let result = mark_thread_done(&github, "42").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn mark_thread_done_noops_on_403() {
        let (base, _) = start_mock_recording(403, "DELETE").await;
        let github = GithubClient::new(std::sync::Arc::from("tok"), base);
        let result = mark_thread_done(&github, "42").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn mark_thread_done_errors_on_500() {
        let (base, _) = start_mock_recording(500, "DELETE").await;
        let github = GithubClient::new(std::sync::Arc::from("tok"), base);
        let result = mark_thread_done(&github, "42").await;
        assert!(result.is_err());
    }
}
