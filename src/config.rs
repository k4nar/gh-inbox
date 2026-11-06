//! Runtime configuration, read from the command line and the environment
//! once at startup — the only place either is interpreted.

use clap::Parser;
use std::time::Duration;

/// GitHub notifications as an inbox.
#[derive(Parser, Debug)]
#[command(name = "gh inbox", version, about)]
struct Cli {
    /// Port to listen on (127.0.0.1). Defaults to a random free port.
    #[arg(short, long, env = "GH_INBOX_PORT")]
    port: Option<u16>,

    /// Do not open the browser on startup.
    #[arg(long)]
    no_open: bool,
}

pub struct Config {
    /// Address to bind: `127.0.0.1:<port>`, or an ephemeral port.
    pub bind_addr: String,
    /// Whether to open the browser once the server is listening.
    pub open_browser: bool,
    /// Background sync cadence: `$GH_INBOX_SYNC_INTERVAL` seconds, default 60.
    pub sync_interval: Duration,
}

impl Config {
    /// Parse CLI arguments (exits with usage on `--help` or invalid input).
    pub fn load() -> Self {
        Self::from_cli(Cli::parse())
    }

    fn from_cli(cli: Cli) -> Self {
        let bind_addr = format!("127.0.0.1:{}", cli.port.unwrap_or(0));
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
            open_browser: !cli.no_open,
            sync_interval,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Config {
        let argv = std::iter::once("gh-inbox").chain(args.iter().copied());
        Config::from_cli(Cli::try_parse_from(argv).expect("valid args"))
    }

    #[test]
    fn port_flag_sets_bind_addr() {
        assert_eq!(parse(&["--port", "8080"]).bind_addr, "127.0.0.1:8080");
        assert_eq!(parse(&["-p", "9000"]).bind_addr, "127.0.0.1:9000");
    }

    #[test]
    fn no_open_flag_disables_browser() {
        assert!(!parse(&["--no-open"]).open_browser);
    }

    #[test]
    fn invalid_port_is_rejected() {
        assert!(Cli::try_parse_from(["gh-inbox", "--port", "70000"]).is_err());
    }

    #[test]
    fn cli_debug_assert() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }
}
