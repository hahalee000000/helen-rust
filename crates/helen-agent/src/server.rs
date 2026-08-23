//! Web server implementation using Axum

use axum::{response::Html, routing::get, Json, Router};
use rust_embed::RustEmbed;
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use crate::api::sessions::{AppState, AppStateInner};
use crate::session::SessionManager;
use crate::storage::FileStorage;

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
    // Create session storage directory
    let session_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("helen-agent")
        .join("sessions");
    
    // Create file storage directory
    let file_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("helen-agent")
        .join("files");
    
    let session_manager = SessionManager::new(session_dir);
    let file_storage = FileStorage::new(file_dir);
    
    let state_inner = AppStateInner {
        session_manager,
        file_storage,
    };
    let state: AppState = Arc::new(Mutex::new(state_inner));

    let app: Router<AppState> = Router::new()
        .route("/", get(index_handler))
        .route("/health", get(health_handler))
        .nest("/api/chat", crate::api::chat::router())
        .nest("/api/chat", crate::websocket::router())
        .nest("/api/agents", crate::api::agents::router())
        .nest("/api", crate::api::sessions::router(state.clone()))
        .nest("/api", crate::api::files::router(state.clone()));

    let app = app.with_state(state);

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
