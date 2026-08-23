//! Web server implementation using Axum

use axum::{response::Html, routing::get, Json, Router};
use rust_embed::RustEmbed;
use serde_json::json;
use std::net::SocketAddr;
use tokio::net::TcpListener;

/// Embedded frontend assets
#[derive(RustEmbed)]
#[folder = "frontend/"]
struct FrontendAssets;

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
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/health", get(health_handler));

    let listener = TcpListener::bind(bind).await?;
    let local_addr = listener.local_addr()?;

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    Ok(Server { handle, local_addr })
}

/// Index handler — serves the embedded frontend
async fn index_handler() -> Html<String> {
    match FrontendAssets::get("index.html") {
        Some(file) => Html(String::from_utf8_lossy(&file.data).to_string()),
        None => Html("<!DOCTYPE html><html><body><h1>Frontend not found</h1></body></html>".to_string()),
    }
}

/// Health check endpoint
async fn health_handler() -> Json<serde_json::Value> {
    Json(json!({"status": "ok"}))
}
