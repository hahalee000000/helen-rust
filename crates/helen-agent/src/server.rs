//! Web server implementation using Axum

use axum::{
    body::Body,
    extract::{Path, Request},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{Html, IntoResponse, Response},
    routing::get,
    Json, Router,
};
use rust_embed::RustEmbed;
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

use crate::api::sessions::{AppState, AppStateInner};
use crate::auth::{AuthConfig, AuthManager};
use crate::directory::DirectoryManager;
use crate::helen_bridge::HelenBridge;
use crate::stream_registry::StreamRegistry;
use crate::upload::UploadManager;

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

/// Build the shared application state
fn build_state() -> AppState {
    let state_inner = AppStateInner {
        directory_manager: Arc::new(DirectoryManager::new(
            std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
        )),
        helen_bridge: Arc::new(HelenBridge::new(std::path::PathBuf::from("helen"))),
        upload_manager: Arc::new(UploadManager::new(std::env::current_dir().unwrap_or_default())),
        stream_registry: Arc::new(StreamRegistry::new()),
    };
    Arc::new(Mutex::new(state_inner))
}

/// Server configuration options
#[derive(Default)]
pub struct ServerOptions {
    /// Optional auth token (None = no auth)
    pub auth_token: Option<String>,
    /// Enable bridge validation endpoint
    pub enable_bridge: bool,
}

/// Start the web server with the given options
///
/// Returns a Server handle that can be used to shutdown the server.
pub async fn start_server_with_options(
    bind: &str,
    options: ServerOptions,
) -> Result<Server, Box<dyn std::error::Error>> {
    let state = build_state();

    // Build router with common routes
    let mut app: Router<AppState> = Router::new()
        .route("/health", get(health_handler))
        .route("/assets/*path", get(static_asset_handler))
        .nest("/api/chat", crate::api::chat::router(state.clone()))
        .nest("/api/chat", crate::websocket::router())
        .nest("/api/agents", crate::api::agents::router(state.clone()));

    // Add bridge endpoint if enabled
    if options.enable_bridge {
        app = app.nest("/api/bridge", crate::api::bridge::router());
    }

    // Add auth middleware if token is provided
    if options.auth_token.is_some() {
        let auth_dir = dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("helen-agent")
            .join("auth");

        let auth_config = AuthConfig {
            enabled: true,
            token: options.auth_token.unwrap_or_default(),
            config_dir: auth_dir,
        };
        let auth_manager = Arc::new(AuthManager::new(auth_config));
        app = app.layer(middleware::from_fn(auth_middleware_factory(auth_manager)));
    }

    let app = app.fallback(spa_fallback).with_state(state);

    let listener = TcpListener::bind(bind).await?;
    let local_addr = listener.local_addr()?;

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    Ok(Server { handle, local_addr })
}

/// Start the web server without authentication
pub async fn start_server(bind: &str) -> Result<Server, Box<dyn std::error::Error>> {
    start_server_with_options(bind, ServerOptions::default()).await
}

/// Start the web server with optional authentication
pub async fn start_server_with_auth(
    bind: &str,
    auth_token: Option<String>,
) -> Result<Server, Box<dyn std::error::Error>> {
    start_server_with_options(bind, ServerOptions { auth_token, ..Default::default() }).await
}

/// Start the web server with optional authentication and bridge validation
pub async fn start_server_with_bridge(
    bind: &str,
    auth_token: Option<String>,
    enable_bridge: bool,
) -> Result<Server, Box<dyn std::error::Error>> {
    start_server_with_options(
        bind,
        ServerOptions {
            auth_token,
            enable_bridge,
        },
    )
    .await
}

/// Health check endpoint
async fn health_handler() -> Json<serde_json::Value> {
    Json(json!({"status": "ok"}))
}

/// Serve embedded static assets (JS, CSS, SVG, etc.)
///
/// URL pattern: /assets/*path → embedded key: "assets/{path}"
async fn static_asset_handler(Path(path): Path<String>) -> impl IntoResponse {
    // rust_embed stores files relative to the folder root (frontend/),
    // so /assets/foo.js maps to embedded key "assets/foo.js"
    let embedded_path = format!("assets/{}", path);
    match FrontendAssets::get(&embedded_path) {
        Some(file) => {
            let mime = mime_from_path(&path);
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
                .body(Body::from(file.data.to_vec()))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// SPA fallback — serve index.html for non-API routes, 404 for API routes
async fn spa_fallback(req: Request) -> impl IntoResponse {
    let path = req.uri().path();
    if path.starts_with("/api/") {
        // API routes should return 404, not the SPA
        StatusCode::NOT_FOUND.into_response()
    } else {
        // Non-API routes serve the SPA for client-side routing
        match FrontendAssets::get("index.html") {
            Some(file) => Html(String::from_utf8_lossy(&file.data).to_string()).into_response(),
            None => Html(
                "<!DOCTYPE html><html><body><h1>Frontend not found</h1></body></html>".to_string(),
            )
            .into_response(),
        }
    }
}

/// Determine MIME type from file extension
pub fn mime_from_path(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("js") => "application/javascript",
        Some("css") => "text/css",
        Some("html") => "text/html",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("json") => "application/json",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}

/// Auth middleware factory
fn auth_middleware_factory(
    auth: Arc<AuthManager>,
) -> impl Fn(
    Request,
    Next,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = axum::response::Response> + Send>>
       + Clone
       + Send
       + 'static {
    move |req: Request, next: Next| {
        let auth = auth.clone();
        Box::pin(async move {
            if auth.is_enabled() {
                // Check for Authorization header
                let headers = req.headers();
                if let Some(auth_header) = headers.get("Authorization") {
                    if let Ok(auth_str) = auth_header.to_str() {
                        if auth_str.starts_with("Bearer ") {
                            let token = auth_str.strip_prefix("Bearer ").unwrap_or(auth_str);
                            if auth.validate_token(token) {
                                return next.run(req).await;
                            }
                        }
                    }
                }

                // Auth failed
                return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
            }

            // Auth disabled, allow access
            next.run(req).await
        })
    }
}
