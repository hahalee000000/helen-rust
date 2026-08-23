//! Web server implementation using Axum

use axum::{routing::get, Json, Router};
use serde_json::json;
use std::net::SocketAddr;
use tokio::net::TcpListener;

/// Running server handle
pub struct Server {
    handle: tokio::task::JoinHandle<()>,
    local_addr: SocketAddr,
}

impl Server {
    /// Shutdown the server
    pub async fn shutdown(self) {
        self.handle.abort();
    }

    /// Get the local address the server is bound to
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

/// Start the web server on the given bind address
///
/// Returns a Server handle that can be used to shutdown the server.
pub async fn start_server(bind: &str) -> Result<Server, Box<dyn std::error::Error>> {
    let app = Router::new().route("/health", get(health_handler));

    let listener = TcpListener::bind(bind).await?;
    let local_addr = listener.local_addr()?;

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    Ok(Server { handle, local_addr })
}

/// Health check endpoint
async fn health_handler() -> Json<serde_json::Value> {
    Json(json!({"status": "ok"}))
}
