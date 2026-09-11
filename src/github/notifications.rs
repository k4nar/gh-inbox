use std::time::Duration;

use super::GithubClient;

/// Parse the URL with `rel="next"` from an HTTP `Link` header value.
/// Returns `None` if no next-page link is present.
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

/// One page of the notification feed, with the metadata GitHub returns
/// alongside it.
pub(crate) struct NotificationPage {
    pub notifications: Vec<crate::models::Notification>,
    /// Next page URL, from the `Link: rel="next"` header.
    pub next: Option<String>,
    /// Value of `x-poll-interval`, when GitHub sent one.
    pub poll_interval: Option<Duration>,
    /// Value of `last-modified`, to replay as `If-Modified-Since`.
    pub last_modified: Option<String>,
    /// GitHub answered `304 Not Modified`: no body, nothing changed.
    pub not_modified: bool,
}

/// The whole notification feed, after following every pagination link.
pub struct NotificationFeed {
    /// Empty when GitHub answered `304 Not Modified`.
    pub notifications: Vec<crate::models::Notification>,
    /// The poll cadence GitHub advertised while serving this feed.
    pub poll_interval: Option<Duration>,
    /// `last-modified` from the first page, to send as `If-Modified-Since` on
    /// the next poll. `None` on a 304, where the caller's stored value stands.
    pub last_modified: Option<String>,
    /// GitHub answered `304 Not Modified`: nothing changed since the value
    /// passed as `If-Modified-Since`.
    pub not_modified: bool,
}

/// Ceiling on a poll interval read from `x-poll-interval`. GitHub sends 60 and
/// raises it under load; anything beyond this is treated as nonsense.
const MAX_POLL_INTERVAL: Duration = Duration::from_secs(15 * 60);

/// Parse `x-poll-interval` (in seconds) from a notifications response.
///
/// GitHub documents this header as how often a client is *allowed* to poll
/// `GET /notifications` ("Please obey the header"), and raises the value when
/// its servers are under load, so it cannot be treated as the constant 60.
fn parse_poll_interval(response: &reqwest::Response) -> Option<Duration> {
    response
        .headers()
        .get("x-poll-interval")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
        // The loop keeps the last value seen for the life of the process, so
        // one garbage-but-numeric header would otherwise stop syncing
        // permanently.
        .map(|interval| interval.min(MAX_POLL_INTERVAL))
}

/// Fetch a single page of notifications from a fully-qualified URL.
///
/// Passing `if_modified_since` lets GitHub reply `304 Not Modified`, which per
/// its docs "leav[es] your current rate limit untouched" — the cheap way to
/// poll. A 304 has no body, so it is detected before deserializing.
pub(crate) async fn fetch_notifications_page(
    github: &super::GithubClient,
    url: &str,
    if_modified_since: Option<&str>,
) -> Result<NotificationPage, reqwest::Error> {
    let response = github
        .get_url_conditional(url, if_modified_since)
        .await?
        .error_for_status()?;

    let poll_interval = parse_poll_interval(&response);

    // 304 is not an error status, so error_for_status() passed it through.
    // Return early: the body is empty and deserializing it would fail.
    if response.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(NotificationPage {
            notifications: Vec::new(),
            next: None,
            poll_interval,
            last_modified: None,
            not_modified: true,
        });
    }

    let next = response
        .headers()
        .get("link")
        .and_then(|v| v.to_str().ok())
        .and_then(parse_next_link);
    let last_modified = response
        .headers()
        .get("last-modified")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let notifications: Vec<crate::models::Notification> = response.json().await?;
    Ok(NotificationPage {
        notifications,
        next,
        poll_interval,
        last_modified,
        not_modified: false,
    })
}

/// Follow `Link: rel="next"` from `url` until the feed is exhausted.
///
/// `if_modified_since` is sent on the first request only. Each page is a
/// distinct URL but shares one `last-modified` validator, so replaying it on
/// page 2 could 304 a page that does have content and silently truncate the
/// feed.
async fn collect_pages(
    github: &super::GithubClient,
    mut url: String,
    if_modified_since: Option<&str>,
) -> Result<NotificationFeed, reqwest::Error> {
    let mut notifications = Vec::new();
    let mut poll_interval = None;
    let mut last_modified = None;
    let mut validator = if_modified_since;
    let mut first_page = true;
    loop {
        let sent_validator = validator.is_some();
        let page = fetch_notifications_page(github, &url, validator).await?;
        validator = None;

        // Every page carries the header; the last one seen is the freshest.
        if page.poll_interval.is_some() {
            poll_interval = page.poll_interval;
        }
        if page.not_modified {
            if sent_validator {
                // A real 304, which only the first page can get: nothing has
                // been collected, and the feed is unchanged.
                return Ok(NotificationFeed {
                    notifications: Vec::new(),
                    poll_interval,
                    last_modified: None,
                    not_modified: true,
                });
            }
            // 304 for a request that carried no validator: a proxy or
            // intermediary misbehaving, since we sent nothing to revalidate
            // against. Reporting "unchanged" here would make the caller
            // discard the pages already collected and advance its cursor past
            // them, so stop and keep what we have instead.
            tracing::warn!(url = %url, "unexpected 304 for an unconditional request");
            break;
        }
        // The validator for the whole feed is the one on its first page.
        if first_page {
            last_modified = page.last_modified;
            first_page = false;
        }

        notifications.extend(page.notifications);
        match page.next {
            Some(next_url) => url = next_url,
            None => break,
        }
    }
    Ok(NotificationFeed {
        notifications,
        poll_interval,
        last_modified,
        not_modified: false,
    })
}

/// Fetch ALL notifications from GitHub, following pagination links until exhausted.
/// Used for full syncs (first run or after a >2h gap).
///
/// Never sends `If-Modified-Since`: reconciliation is by absence, and a thread
/// aging out of GitHub's ~7-week feed window does not bump `last-modified`, so
/// a 304 would hand back an empty feed and skip reconciliation entirely.
pub async fn fetch_all_notifications(
    github: &super::GithubClient,
) -> Result<NotificationFeed, reqwest::Error> {
    // `all=true` so read-but-not-done notifications stay in the feed. Without it
    // GitHub returns only unread notifications, which makes reconciliation conflate
    // "read" with "done" and silently archive PRs the user only marked as read.
    let url = format!("{}/notifications?all=true&per_page=50", github.base_url());
    // Deliberately unconditional — see the doc comment above.
    collect_pages(github, url, None).await
}

/// Fetch notifications updated after `since_iso` (ISO 8601 UTC string), following pagination.
/// Used for incremental syncs.
///
/// `if_modified_since` should be the `last-modified` value from the previous
/// poll: GitHub then answers 304 when nothing changed, which costs no rate
/// limit and no body.
pub async fn fetch_notifications_since(
    github: &super::GithubClient,
    since_iso: &str,
    if_modified_since: Option<&str>,
) -> Result<NotificationFeed, reqwest::Error> {
    let url = format!(
        "{}/notifications?all=true&since={since_iso}&per_page=50",
        github.base_url()
    );
    collect_pages(github, url, if_modified_since).await
}

pub async fn mark_thread_read(
    github: &GithubClient,
    thread_id: &str,
) -> Result<(), reqwest::Error> {
    let response = github
        .patch(&format!("/notifications/threads/{thread_id}"))
        .await?;
    // 404: the thread is gone on GitHub — nothing to mark, not an error.
    // 403 from a missing `notifications` scope must surface, or read state
    // silently never reaches GitHub and every full sync reverts it — but a
    // rate-limit 403 is transient and would otherwise toast an error per
    // action for the whole limit window.
    if response.status() == 404 || is_rate_limited(&response) {
        return Ok(());
    }
    response.error_for_status()?;
    Ok(())
}

/// GitHub signals primary and secondary rate limits with HTTP 403/429 plus a
/// `retry-after` header or an exhausted `x-ratelimit-remaining`. Those are
/// transient: log and move on instead of surfacing an error per action.
fn is_rate_limited(response: &reqwest::Response) -> bool {
    if response.status() != 403 && response.status() != 429 {
        return false;
    }
    let headers = response.headers();
    let limited = headers.contains_key("retry-after")
        || headers
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            == Some("0");
    if limited {
        tracing::warn!(url = %response.url(), "GitHub rate limited; write-back skipped");
    }
    limited
}

pub async fn mark_thread_done(
    github: &GithubClient,
    thread_id: &str,
) -> Result<(), reqwest::Error> {
    let response = github
        .delete(&format!("/notifications/threads/{thread_id}"))
        .await?;
    // 404 is fine (thread already gone); scope 403 must surface but a
    // rate-limit 403/429 is transient — see mark_thread_read.
    if response.status() == 404 || is_rate_limited(&response) {
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
    async fn mark_thread_read_errors_on_403() {
        let (base, _) = start_mock_recording(403, "PATCH").await;
        let github = GithubClient::new(std::sync::Arc::from("tok"), base);
        let result = mark_thread_read(&github, "42").await;
        assert!(result.is_err(), "missing-scope 403 must surface");
    }

    #[tokio::test]
    async fn mark_thread_read_tolerates_rate_limit_403() {
        // A secondary-rate-limit 403 carries a retry-after header — transient,
        // must not surface as an error toast per action.
        let app = Router::new().route(
            "/notifications/threads/42",
            patch(|| async {
                axum::http::Response::builder()
                    .status(403)
                    .header("retry-after", "60")
                    .body(axum::body::Body::empty())
                    .unwrap()
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let github = GithubClient::new(std::sync::Arc::from("tok"), format!("http://{addr}"));
        let result = mark_thread_read(&github, "42").await;
        assert!(result.is_ok(), "rate-limit 403 is transient, not an error");
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
    async fn mark_thread_done_errors_on_403() {
        let (base, _) = start_mock_recording(403, "DELETE").await;
        let github = GithubClient::new(std::sync::Arc::from("tok"), base);
        let result = mark_thread_done(&github, "42").await;
        assert!(result.is_err(), "missing-scope 403 must surface");
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
    use std::time::Duration;

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
        let page = fetch_notifications_page(&github, &url, None).await.unwrap();
        let (notifs, got_next) = (page.notifications, page.next);

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
        let page = fetch_notifications_page(&github, &url, None).await.unwrap();
        let (notifs, next) = (page.notifications, page.next);

        assert_eq!(notifs.len(), 1);
        assert!(next.is_none());
    }

    #[tokio::test]
    async fn reads_poll_interval_header() {
        let app = Router::new().route(
            "/notifications",
            get(|| async {
                axum::http::Response::builder()
                    .header("content-type", "application/json")
                    .header("x-poll-interval", "90")
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
        let page = fetch_notifications_page(&github, &url, None).await.unwrap();

        assert_eq!(page.poll_interval, Some(Duration::from_secs(90)));
    }

    #[tokio::test]
    async fn absurd_poll_interval_is_clamped() {
        // The loop keeps the last value for the life of the process, so an
        // unclamped garbage value would stop syncing permanently.
        let app = Router::new().route(
            "/notifications",
            get(|| async {
                axum::http::Response::builder()
                    .header("content-type", "application/json")
                    .header("x-poll-interval", "999999999")
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
        let page = fetch_notifications_page(&github, &url, None).await.unwrap();

        assert_eq!(page.poll_interval, Some(Duration::from_secs(15 * 60)));
    }

    #[tokio::test]
    async fn poll_interval_is_none_without_the_header() {
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
        let page = fetch_notifications_page(&github, &url, None).await.unwrap();

        assert!(page.poll_interval.is_none());
    }

    #[tokio::test]
    async fn conditional_header_is_sent_only_on_the_first_page() {
        // Page 2 shares the feed's one last-modified validator, so replaying
        // If-Modified-Since would 304 a page that does have content.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let base = format!("http://{addr}");
        let base_clone = base.clone();
        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();
        let seen = Arc::new(Mutex::new(Vec::<Option<String>>::new()));
        let seen_clone = seen.clone();

        let app = Router::new().route(
            "/notifications",
            get(move |req: Request| {
                let count = call_count_clone.fetch_add(1, Ordering::SeqCst);
                let base = base_clone.clone();
                let seen = seen_clone.clone();
                async move {
                    seen.lock().unwrap().push(
                        req.headers()
                            .get("if-modified-since")
                            .and_then(|v| v.to_str().ok())
                            .map(str::to_owned),
                    );
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
        let feed = super::fetch_notifications_since(
            &github,
            "2025-06-01T00:00:00Z",
            Some("Thu, 10 Sep 2026 16:35:40 GMT"),
        )
        .await
        .unwrap();

        // Both pages still arrived.
        assert_eq!(feed.notifications.len(), 2);

        let requests = seen.lock().unwrap().clone();
        assert_eq!(requests.len(), 2);
        assert!(requests[0].is_some(), "page 1 carries the validator");
        assert_eq!(requests[1], None, "page 2 must be unconditional");
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
        let notifs = super::fetch_all_notifications(&github)
            .await
            .unwrap()
            .notifications;

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
        super::fetch_notifications_since(&github, "2025-06-01T00:00:00Z", None)
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
