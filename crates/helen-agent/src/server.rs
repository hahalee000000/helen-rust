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
        .route("/health", get(health_handler))
        .route("/assets/*path", get(static_asset_handler))
        .nest("/api/chat", crate::api::chat::router())
        .nest("/api/chat", crate::websocket::router())
        .nest("/api/agents", crate::api::agents::router())
        .nest("/api", crate::api::sessions::router(state.clone()))
        .nest("/api", crate::api::files::router(state.clone()))
        .fallback(spa_fallback);

    let app = app.with_state(state);

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

/// Serve embedded static assets (JS, CSS, SVG, etc.)
async fn static_asset_handler(Path(path): Path<String>) -> impl IntoResponse {
    match FrontendAssets::get(&path) {
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
            None => Html("<!DOCTYPE html><html><body><h1>Frontend not found</h1></body></html>".to_string()).into_response(),
        }
    }
}

/// Determine MIME type from file extension
fn mime_from_path(path: &str) -> &'static str {
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

/// Start the web server with optional authentication
///
/// If `auth_token` is Some, authentication is enabled with the given token.
/// If `auth_token` is None, authentication is disabled.
pub async fn start_server_with_auth(
    bind: &str,
    auth_token: Option<String>,
) -> Result<Server, Box<dyn std::error::Error>> {
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

    // Create auth config directory
    let auth_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("helen-agent")
        .join("auth");

    let session_manager = SessionManager::new(session_dir);
    let file_storage = FileStorage::new(file_dir);

    let state_inner = AppStateInner {
        session_manager,
        file_storage,
    };
    let state: AppState = Arc::new(Mutex::new(state_inner));

    // Create auth manager
    let auth_config = AuthConfig {
        enabled: auth_token.is_some(),
        token: auth_token.unwrap_or_default(),
        config_dir: auth_dir,
    };
    let auth_manager = Arc::new(AuthManager::new(auth_config));

    let app: Router<AppState> = Router::new()
        .route("/health", get(health_handler))
        .route("/assets/*path", get(static_asset_handler))
        .nest("/api/chat", crate::api::chat::router())
        .nest("/api/chat", crate::websocket::router())
        .nest("/api/agents", crate::api::agents::router())
        .nest("/api", crate::api::sessions::router(state.clone()))
        .nest("/api", crate::api::files::router(state.clone()))
        .fallback(spa_fallback)
        .layer(middleware::from_fn(auth_middleware_factory(auth_manager)));

    let app = app.with_state(state);

    let listener = TcpListener::bind(bind).await?;
    let local_addr = listener.local_addr()?;

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    Ok(Server { handle, local_addr })
}

/// Start the web server with optional authentication and bridge validation
///
/// If `auth_token` is Some, authentication is enabled with the given token.
/// If `enable_bridge` is true, the bridge validation endpoint is enabled.
pub async fn start_server_with_bridge(
    bind: &str,
    auth_token: Option<String>,
    enable_bridge: bool,
) -> Result<Server, Box<dyn std::error::Error>> {
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

    // Create auth config directory
    let auth_dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("helen-agent")
        .join("auth");

    let session_manager = SessionManager::new(session_dir);
    let file_storage = FileStorage::new(file_dir);

    let state_inner = AppStateInner {
        session_manager,
        file_storage,
    };
    let state: AppState = Arc::new(Mutex::new(state_inner));

    // Create auth manager
    let auth_config = AuthConfig {
        enabled: auth_token.is_some(),
        token: auth_token.unwrap_or_default(),
        config_dir: auth_dir,
    };
    let auth_manager = Arc::new(AuthManager::new(auth_config));

    let mut app: Router<AppState> = Router::new()
        .route("/health", get(health_handler))
        .route("/assets/*path", get(static_asset_handler))
        .nest("/api/chat", crate::api::chat::router())
        .nest("/api/chat", crate::websocket::router())
        .nest("/api/agents", crate::api::agents::router())
        .nest("/api", crate::api::sessions::router(state.clone()))
        .nest("/api", crate::api::files::router(state.clone()));

    // Add bridge endpoint if enabled
    if enable_bridge {
        app = app.nest("/api/bridge", crate::api::bridge::router());
    }

    let app = app
        .fallback(spa_fallback)
        .layer(middleware::from_fn(auth_middleware_factory(auth_manager)))
        .with_state(state);

    let listener = TcpListener::bind(bind).await?;
    let local_addr = listener.local_addr()?;

    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    Ok(Server { handle, local_addr })
}
