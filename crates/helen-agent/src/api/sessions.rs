//! Application state shared across API handlers
//!
//! Defines `AppState` and `AppStateInner` — the shared state container
//! used by all API endpoints (chat, agents, websocket, bridge).

use std::sync::Arc;
use tokio::sync::Mutex;

use crate::directory::DirectoryManager;
use crate::helen_bridge::HelenBridge;
use crate::stream_registry::StreamRegistry;
use crate::upload::UploadManager;

/// Combined application state
pub struct AppStateInner {
    pub directory_manager: Arc<DirectoryManager>,
    pub helen_bridge: Arc<HelenBridge>,
    pub upload_manager: Arc<UploadManager>,
    pub stream_registry: Arc<StreamRegistry>,
}

/// Shared application state (thread-safe via Arc<Mutex>)
pub type AppState = Arc<Mutex<AppStateInner>>;
