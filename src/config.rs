//! Runtime configuration, read from the environment once at startup —
//! the only place environment variables are interpreted.

use std::time::Duration;

pub struct Config {
    /// Address to bind: `127.0.0.1:$GH_INBOX_PORT`, or an ephemeral port.
    pub bind_addr: String,
    /// Background sync cadence: `$GH_INBOX_SYNC_INTERVAL` seconds, default 30.
    pub sync_interval: Duration,
}

impl Config {
    pub fn from_env() -> Self {
        let bind_addr = match std::env::var("GH_INBOX_PORT") {
            Ok(port) => format!("127.0.0.1:{port}"),
            Err(_) => "127.0.0.1:0".to_string(),
        };
        let sync_interval = Duration::from_secs(
            std::env::var("GH_INBOX_SYNC_INTERVAL")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
        );
        Self {
            bind_addr,
            sync_interval,
        }
    }
}
