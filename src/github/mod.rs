pub mod fetch_pr_graphql;
mod notifications;
pub mod sync;
mod teams;

use std::sync::Arc;

use reqwest::{Method, Response};
use serde::Serialize;

pub const GITHUB_API_BASE: &str = "https://api.github.com";

pub use fetch_pr_graphql::fetch_pr_graphql;
pub use notifications::{fetch_notifications, mark_thread_done, mark_thread_read};
pub use teams::fetch_user_teams;

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

    async fn post_json<T: Serialize + ?Sized>(
        &self,
        path: &str,
        body: &T,
    ) -> Result<Response, reqwest::Error> {
        let builder = self.request_builder(Method::POST, path).json(body);
        self.execute(builder, "POST", path).await
    }

    async fn send(&self, method: Method, path: &str) -> Result<Response, reqwest::Error> {
        let builder = self.request_builder(method.clone(), path);
        self.execute(builder, method.as_str(), path).await
    }

    fn request_builder(&self, method: Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}{path}", self.base_url);
        self.client
            .request(method, &url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "gh-inbox")
            .header("X-GitHub-Api-Version", "2026-03-10")
    }

    async fn execute(
        &self,
        builder: reqwest::RequestBuilder,
        label: &str,
        path: &str,
    ) -> Result<Response, reqwest::Error> {
        if cfg!(debug_assertions) {
            eprintln!("[debug] GitHub {label} {}{path}", self.base_url);
        }

        match builder.send().await {
            Ok(response) => {
                if cfg!(debug_assertions) {
                    eprintln!(
                        "[debug] GitHub {label} {}{path} -> {}",
                        self.base_url,
                        response.status()
                    );
                }
                Ok(response)
            }
            Err(err) => {
                if cfg!(debug_assertions) {
                    eprintln!(
                        "[debug] GitHub {label} {}{path} -> error: {err}",
                        self.base_url
                    );
                }
                Err(err)
            }
        }
    }
}
