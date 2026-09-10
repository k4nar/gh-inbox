pub mod fetch_pr_graphql;
mod notifications;
pub mod pr_cache;
pub mod sync;
mod teams;
#[cfg(test)]
pub(crate) mod test_support;

use std::sync::Arc;
use std::time::Duration;

use reqwest::{Method, Response};
use serde::Serialize;

pub const GITHUB_API_BASE: &str = "https://api.github.com";

pub use fetch_pr_graphql::fetch_pr_graphql;
pub use notifications::{
    fetch_all_notifications, fetch_notifications_since, mark_thread_done, mark_thread_read,
};
pub use teams::fetch_user_teams;

/// Format an error together with its full `source()` chain.
///
/// `reqwest::Error`'s `Display` prints only the kind and the URL — e.g.
/// "error sending request for url (…)" — while the cause that actually
/// explains the failure (dns error, connection reset, timed out, certificate
/// error) is reachable only through `source()`. Logging without the chain
/// makes every transport failure look identical.
pub fn error_chain(err: &dyn std::error::Error) -> String {
    let mut out = err.to_string();
    let mut source = err.source();
    while let Some(e) = source {
        out.push_str(": ");
        out.push_str(&e.to_string());
        source = e.source();
    }
    out
}

/// Total budget for one GitHub request, connection included.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Budget for establishing the TCP+TLS connection alone.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// How often to PING an idle-but-open HTTP/2 connection.
const H2_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(20);
/// How long to wait for a PING reply before declaring the connection dead.
const H2_KEEPALIVE_TIMEOUT: Duration = Duration::from_secs(10);

/// Build the HTTP client shared by every GitHub call.
///
/// api.github.com serves HTTP/2, so reqwest multiplexes all our requests onto
/// a single pooled connection per host. Two defaults are wrong for a
/// long-running desktop process:
///
/// - No request timeout at all, so a stalled request hangs forever. The sync
///   loop awaits its tick, so one hung request stops syncing indefinitely.
/// - HTTP/2 keepalive off, and `pool_idle_timeout` only evicts *idle*
///   connections. A sync every minute keeps the connection out of the idle
///   state, so a connection that dies silently (laptop sleep, VPN or network
///   change) is never evicted and every later request fails with a transport
///   error until the process restarts.
///
/// PINGs let hyper notice a dead peer and drop the connection; the timeouts
/// bound anything that still gets stuck.
fn build_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .connect_timeout(CONNECT_TIMEOUT)
        .http2_keep_alive_interval(H2_KEEPALIVE_INTERVAL)
        .http2_keep_alive_timeout(H2_KEEPALIVE_TIMEOUT)
        .http2_keep_alive_while_idle(true)
        .build()
        .unwrap_or_else(|e| {
            // Only TLS/resolver initialisation can fail here. A default client
            // still works; it just lacks the hardening above.
            tracing::warn!(error = %error_chain(&e), "could not build the tuned HTTP client");
            reqwest::Client::new()
        })
}

#[derive(Clone)]
pub struct GithubClient {
    client: reqwest::Client,
    token: Arc<str>,
    base_url: String,
}

impl GithubClient {
    pub fn new(token: Arc<str>, base_url: String) -> Self {
        Self {
            client: build_http_client(),
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

    /// Fetch a fully-qualified URL with the standard GitHub auth headers.
    /// Used for following pagination `Link: rel="next"` URLs.
    pub(crate) async fn get_url(&self, url: &str) -> Result<Response, reqwest::Error> {
        let builder = self
            .client
            .request(Method::GET, url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", "gh-inbox")
            .header("X-GitHub-Api-Version", "2026-03-10");
        self.execute(builder, "GET", url).await
    }

    /// The base URL this client sends requests to (e.g. `https://api.github.com`).
    /// Needed by callers that build full pagination URLs.
    pub fn base_url(&self) -> &str {
        &self.base_url
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
        match builder.send().await {
            Ok(response) => Ok(response),
            Err(err) => {
                tracing::warn!(method = label, path, base_url = %self.base_url, error = %error_chain(&err), "GitHub request failed");
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::error_chain;

    #[tokio::test]
    async fn error_chain_exposes_the_cause_display_hides() {
        // Nothing listens on port 1, so this is a transport error — the same
        // `Kind::Request` the sync loop reports as "error sending request".
        let err = reqwest::Client::new()
            .get("http://127.0.0.1:1/notifications")
            .send()
            .await
            .expect_err("connecting to port 1 must fail");

        // What the logs used to show: the kind and the URL, nothing more.
        let display = err.to_string();
        assert!(display.starts_with("error sending request"));

        // What they show now: the chain down to the OS error.
        let chain = error_chain(&err);
        assert!(chain.starts_with(&display), "chain extends Display");
        assert!(
            chain.len() > display.len(),
            "chain must add the cause Display omits, got {chain:?}"
        );
    }

    #[test]
    fn error_chain_of_a_sourceless_error_is_just_its_display() {
        let err = std::io::Error::other("boom");
        assert_eq!(error_chain(&err), "boom");
    }
}
