use super::GithubClient;

/// Parse the URL with `rel="next"` from an HTTP `Link` header value.
/// Returns `None` if no next-page link is present.
#[allow(dead_code)]
fn parse_next_link(link: &str) -> Option<String> {
    for part in link.split(',') {
        let mut url: Option<String> = None;
        let mut is_next = false;
        for segment in part.trim().split(';') {
            let segment = segment.trim();
            if segment.starts_with('<') && segment.ends_with('>') {
                url = Some(segment[1..segment.len() - 1].to_string());
            } else if segment == r#"rel="next""# {
                is_next = true;
            }
        }
        if is_next {
            return url;
        }
    }
    None
}

/// Fetch a single page of notifications from a fully-qualified URL.
/// Returns the deserialized notifications and the next-page URL (from the
/// `Link: rel="next"` response header), if any.
#[allow(dead_code)]
pub(crate) async fn fetch_notifications_page(
    github: &super::GithubClient,
    url: &str,
) -> Result<(Vec<crate::models::Notification>, Option<String>), reqwest::Error> {
    let response = github.get_url(url).await?.error_for_status()?;
    let next = response
        .headers()
        .get("link")
        .and_then(|v| v.to_str().ok())
        .and_then(parse_next_link);
    let notifications: Vec<crate::models::Notification> = response.json().await?;
    Ok((notifications, next))
}

/// Fetch ALL notifications from GitHub, following pagination links until exhausted.
/// Used for full syncs (first run or after a >2h gap).
#[allow(dead_code)]
pub async fn fetch_all_notifications(
    github: &super::GithubClient,
) -> Result<Vec<crate::models::Notification>, reqwest::Error> {
    let mut url = format!("{}/notifications?all=true&per_page=50", github.base_url());
    let mut all = Vec::new();
    loop {
        let (page, next) = fetch_notifications_page(github, &url).await?;
        all.extend(page);
        match next {
            Some(next_url) => url = next_url,
            None => break,
        }
    }
    Ok(all)
}

/// Fetch notifications updated after `since_iso` (ISO 8601 UTC string), following pagination.
/// Used for incremental syncs.
pub async fn fetch_notifications_since(
    github: &super::GithubClient,
    since_iso: &str,
) -> Result<Vec<crate::models::Notification>, reqwest::Error> {
    let mut url = format!(
        "{}/notifications?all=true&since={since_iso}&per_page=50",
        github.base_url()
    );
    let mut all = Vec::new();
    loop {
        let (page, next) = fetch_notifications_page(github, &url).await?;
        all.extend(page);
        match next {
            Some(next_url) => url = next_url,
            None => break,
        }
    }
    Ok(all)
}

pub async fn mark_thread_read(
    github: &GithubClient,
    thread_id: &str,
) -> Result<(), reqwest::Error> {
    let response = github
        .patch(&format!("/notifications/threads/{thread_id}"))
        .await?;
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
    let response = github
        .delete(&format!("/notifications/threads/{thread_id}"))
        .await?;
    let status = response.status();
    if status == 403 || status == 404 {
        return Ok(());
    }
    response.error_for_status()?;
    Ok(())
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

#[cfg(test)]
mod page_tests {
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use axum::Router;
    use axum::extract::Request;
    use axum::routing::get;
    use tokio::net::TcpListener;

    use super::{fetch_notifications_page, parse_next_link};
    use crate::github::GithubClient;

    const ONE_NOTIFICATION: &str = r#"[{
        "id": "1",
        "reason": "review_requested",
        "unread": true,
        "updated_at": "2025-01-01T00:00:00Z",
        "subject": {
            "title": "Fix bug",
            "url": "https://api.github.com/repos/owner/repo/pulls/42",
            "type": "PullRequest"
        },
        "repository": { "full_name": "owner/repo" }
    }]"#;

    const PAGE_1: &str = r#"[{
        "id": "1",
        "reason": "review_requested",
        "unread": true,
        "updated_at": "2025-01-01T00:00:00Z",
        "subject": { "title": "PR 1", "url": null, "type": "PullRequest" },
        "repository": { "full_name": "owner/repo" }
    }]"#;

    const PAGE_2: &str = r#"[{
        "id": "2",
        "reason": "mention",
        "unread": false,
        "updated_at": "2025-01-02T00:00:00Z",
        "subject": { "title": "PR 2", "url": null, "type": "PullRequest" },
        "repository": { "full_name": "owner/repo" }
    }]"#;

    // --- parse_next_link unit tests ---

    #[test]
    fn parses_next_from_multi_rel_header() {
        let link = r#"<https://api.github.com/notifications?page=2>; rel="next", <https://api.github.com/notifications?page=5>; rel="last""#;
        assert_eq!(
            parse_next_link(link),
            Some("https://api.github.com/notifications?page=2".to_string())
        );
    }

    #[test]
    fn returns_none_when_no_next_rel() {
        let link = r#"<https://api.github.com/notifications?page=5>; rel="last""#;
        assert_eq!(parse_next_link(link), None);
    }

    #[test]
    fn returns_none_for_empty_string() {
        assert_eq!(parse_next_link(""), None);
    }

    // --- fetch_notifications_page integration tests ---

    #[tokio::test]
    async fn returns_notifications_and_next_url_from_link_header() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://{addr}");
        let next_url = format!("{base}/notifications?page=2&per_page=50");
        let next_url_clone = next_url.clone();

        let app = Router::new().route(
            "/notifications",
            get(move || {
                let next = next_url_clone.clone();
                async move {
                    axum::http::Response::builder()
                        .header("content-type", "application/json")
                        .header("link", format!(r#"<{next}>; rel="next""#))
                        .body(axum::body::Body::from(ONE_NOTIFICATION))
                        .unwrap()
                }
            }),
        );
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let github = GithubClient::new(Arc::from("fake-token"), base.clone());
        let url = format!("{base}/notifications?per_page=50");
        let (notifs, got_next) = fetch_notifications_page(&github, &url).await.unwrap();

        assert_eq!(notifs.len(), 1);
        assert_eq!(notifs[0].id, "1");
        assert_eq!(got_next, Some(next_url));
    }

    #[tokio::test]
    async fn returns_none_next_when_no_link_header() {
        let app = Router::new().route(
            "/notifications",
            get(|| async {
                axum::http::Response::builder()
                    .header("content-type", "application/json")
                    .body(axum::body::Body::from(ONE_NOTIFICATION))
                    .unwrap()
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://{addr}");
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let github = GithubClient::new(Arc::from("fake-token"), base.clone());
        let url = format!("{base}/notifications?per_page=50");
        let (notifs, next) = fetch_notifications_page(&github, &url).await.unwrap();

        assert_eq!(notifs.len(), 1);
        assert!(next.is_none());
    }

    #[tokio::test]
    async fn fetch_all_follows_link_headers_across_pages() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://{addr}");
        let base_clone = base.clone();
        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        let app = Router::new().route(
            "/notifications",
            get(move || {
                let count = call_count_clone.fetch_add(1, Ordering::SeqCst);
                let base = base_clone.clone();
                async move {
                    if count == 0 {
                        let next = format!("{base}/notifications?page=2&per_page=50");
                        axum::http::Response::builder()
                            .header("content-type", "application/json")
                            .header("link", format!(r#"<{next}>; rel="next""#))
                            .body(axum::body::Body::from(PAGE_1))
                            .unwrap()
                    } else {
                        axum::http::Response::builder()
                            .header("content-type", "application/json")
                            .body(axum::body::Body::from(PAGE_2))
                            .unwrap()
                    }
                }
            }),
        );
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let github = GithubClient::new(Arc::from("fake-token"), base);
        let notifs = super::fetch_all_notifications(&github).await.unwrap();

        assert_eq!(notifs.len(), 2);
        assert_eq!(notifs[0].id, "1");
        assert_eq!(notifs[1].id, "2");
    }

    #[tokio::test]
    async fn fetch_since_includes_since_param_in_url() {
        let captured_uri = Arc::new(Mutex::new(String::new()));
        let captured_clone = captured_uri.clone();

        let app = Router::new().route(
            "/notifications",
            get(move |req: Request| {
                let captured = captured_clone.clone();
                async move {
                    *captured.lock().unwrap() = req.uri().to_string();
                    axum::http::Response::builder()
                        .header("content-type", "application/json")
                        .body(axum::body::Body::from("[]"))
                        .unwrap()
                }
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://{addr}");
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let github = GithubClient::new(Arc::from("fake-token"), base);
        super::fetch_notifications_since(&github, "2025-06-01T00:00:00Z")
            .await
            .unwrap();

        let uri = captured_uri.lock().unwrap().clone();
        assert!(
            uri.contains("since="),
            "URI should contain since= param, got: {uri}"
        );
        assert!(
            uri.contains("2025-06-01"),
            "URI should contain the since date, got: {uri}"
        );
    }
}
