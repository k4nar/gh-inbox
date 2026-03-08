use gh_inbox::{app, db};

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let pool = db::init().await;
    println!("Database initialized");

    let token = acquire_token();
    println!("GitHub token acquired");

    let addr = match std::env::var("GH_INBOX_PORT") {
        Ok(port) => format!("127.0.0.1:{port}"),
        Err(_) => "127.0.0.1:0".to_string(),
    };

    let listener = TcpListener::bind(&addr)
        .await
        .expect("failed to bind to port");

    let addr: SocketAddr = listener.local_addr().expect("failed to get local address");
    let url = format!("http://{addr}");

    println!("Listening on {url}");

    if let Err(e) = open::that(&url) {
        eprintln!("Failed to open browser: {e}");
    }

    axum::serve(listener, app(pool, Arc::from(token)))
        .await
        .expect("server error");
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
