use gh_inbox::{app, db};

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                let default = if cfg!(debug_assertions) {
                    "debug"
                } else {
                    "info"
                };
                tracing_subscriber::EnvFilter::new(default)
            }),
        )
        .init();

    let pool = db::init().await;
    tracing::info!("database initialized");

    let token = acquire_token();
    tracing::info!("GitHub token acquired");

    let addr = match std::env::var("GH_INBOX_PORT") {
        Ok(port) => format!("127.0.0.1:{port}"),
        Err(_) => "127.0.0.1:0".to_string(),
    };

    let listener = TcpListener::bind(&addr)
        .await
        .expect("failed to bind to port");

    let addr: SocketAddr = listener.local_addr().expect("failed to get local address");
    let url = format!("http://{addr}");

    tracing::info!(url, "listening");

    let mut vite_child: Option<std::process::Child> = None;

    if cfg!(debug_assertions) {
        // Dev mode: spawn the Vite dev server with hot reload
        match std::process::Command::new("npm")
            .args(["run", "dev", "--", "--open"])
            .env("GH_INBOX_PORT", addr.port().to_string())
            .spawn()
        {
            Ok(child) => {
                tracing::info!("Vite dev server starting");
                vite_child = Some(child);
            }
            Err(e) => tracing::warn!("could not start Vite dev server: {e}"),
        }
    } else {
        // Prod mode: open the browser directly to the backend
        if let Err(e) = open::that(&url) {
            tracing::warn!("failed to open browser: {e}");
        }
    }

    let (router, state) = app(pool, Arc::from(token));

    // Spawn the background sync loop
    let sync_state = state.clone();
    let sync_tx = state.tx.clone();
    tokio::spawn(async move {
        gh_inbox::github::sync::run_sync_loop(sync_state, sync_tx).await;
    });

    axum::serve(listener, router).await.expect("server error");

    // Clean up the Vite dev server when the backend shuts down
    if let Some(mut child) = vite_child {
        let _ = child.kill();
        let _ = child.wait();
    }
}

/// Run `gh auth token` once and return the token string.
fn acquire_token() -> String {
    let output = std::process::Command::new("gh")
        .args(["auth", "token"])
        .output()
        .expect("failed to run `gh auth token` — is the `gh` CLI installed?");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!("gh auth token failed: {stderr}");
    }

    String::from_utf8(output.stdout)
        .expect("token is not valid UTF-8")
        .trim()
        .to_string()
}
