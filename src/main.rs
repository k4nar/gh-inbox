mod server;

use std::net::SocketAddr;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("failed to bind to a random port");

    let addr: SocketAddr = listener.local_addr().expect("failed to get local address");
    let url = format!("http://{addr}");

    println!("Listening on {url}");

    if let Err(e) = open::that(&url) {
        eprintln!("Failed to open browser: {e}");
    }

    axum::serve(listener, server::app())
        .await
        .expect("server error");
}
