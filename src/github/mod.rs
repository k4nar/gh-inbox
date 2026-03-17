mod check_runs;
mod commits;
mod notifications;
mod pull_requests;
pub mod sync;
mod teams;

use std::sync::Arc;

use reqwest::{Method, Response};

pub const GITHUB_API_BASE: &str = "https://api.github.com";

pub use check_runs::fetch_check_runs;
pub use commits::fetch_commits;
pub use notifications::{fetch_notifications, mark_thread_done, mark_thread_read};
pub use pull_requests::{fetch_issue_comments, fetch_pull_request, fetch_review_comments};
pub use teams::{fetch_requested_reviewer_teams, fetch_user_teams};

#[derive(Clone)]
pub struct GithubClient {
    client: reqwest::Client,
    token: Arc<str>,
    base_url: String,
}

impl GithubClient {
    pub fn new(token: Arc<str>, base_url: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            token,
            base_url,
        }
    }

    async fn get(&self, path: &str) -> Result<Response, reqwest::Error> {
        self.send(Method::GET, path).await
    }

    async fn patch(&self, path: &str) -> Result<Response, reqwest::Error> {
        self.send(Method::PATCH, path).await
    }

    async fn delete(&self, path: &str) -> Result<Response, reqwest::Error> {
        self.send(Method::DELETE, path).await
    }

    async fn send(&self, method: Method, path: &str) -> Result<Response, reqwest::Error> {
        let url = format!("{}{path}", self.base_url);

        if cfg!(debug_assertions) {
            eprintln!("[debug] GitHub {method} {url}");
        }

        let request = self
            .client
            .request(method.clone(), &url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "gh-inbox")
            .header("X-GitHub-Api-Version", "2026-03-10");

        match request.send().await {
            Ok(response) => {
                if cfg!(debug_assertions) {
                    eprintln!("[debug] GitHub {method} {url} -> {}", response.status());
                }
                Ok(response)
            }
            Err(err) => {
                if cfg!(debug_assertions) {
                    eprintln!("[debug] GitHub {method} {url} -> error: {err}");
                }
                Err(err)
            }
        }
    }
}
