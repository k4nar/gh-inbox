use gh_inbox::{app, db};

use std::net::SocketAddr;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let pool = db::init().await;
    println!("Database initialized");

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

    axum::serve(listener, app(pool))
        .await
        .expect("server error");
}
