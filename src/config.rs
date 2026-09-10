//! Runtime configuration, read from the environment once at startup —
//! the only place environment variables are interpreted.

use std::time::Duration;

pub struct Config {
    /// Address to bind: `127.0.0.1:$GH_INBOX_PORT`, or an ephemeral port.
    pub bind_addr: String,
    /// Background sync cadence: `$GH_INBOX_SYNC_INTERVAL` seconds, default 60.
    pub sync_interval: Duration,
}

impl Config {
    pub fn from_env() -> Self {
        let bind_addr = match std::env::var("GH_INBOX_PORT") {
            Ok(port) => format!("127.0.0.1:{port}"),
            Err(_) => "127.0.0.1:0".to_string(),
        };
        // 60s, not less: `GET /notifications` returns `x-poll-interval: 60`,
        // and GitHub documents that header as how often you are *allowed* to
        // poll ("Please obey the header"). Polling faster risks a secondary
        // rate limit, which applies account-wide across REST and GraphQL.
        let sync_interval = Duration::from_secs(
            std::env::var("GH_INBOX_SYNC_INTERVAL")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
        );
        Self {
            bind_addr,
            sync_interval,
        }
    }
}
