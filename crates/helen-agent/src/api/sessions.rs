//! Session API endpoints

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::directory::DirectoryManager;
use crate::session::SessionManager;
use crate::storage::FileStorage;

/// Combined application state
pub struct AppStateInner {
    pub session_manager: SessionManager,
    pub file_storage: FileStorage,
    pub directory_manager: Arc<DirectoryManager>,
}

/// Shared application state
pub type AppState = Arc<Mutex<AppStateInner>>;

/// Create session router
pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/sessions", post(create_session))
        .route("/sessions", get(list_sessions))
        .route("/sessions/:id", get(get_session))
        .route("/sessions/:id", delete(delete_session))
        .with_state(state)
}

/// Create a new session
async fn create_session(State(state): State<AppState>) -> Json<serde_json::Value> {
    let inner = state.lock().await;
    let session = inner.session_manager.create_session();
    let _ = inner.session_manager.save_session(&session);

    Json(json!({
        "id": session.id,
        "created_at": session.created_at,
        "messages": []
    }))
}

/// List all sessions
async fn list_sessions(
    State(state): State<AppState>,
) -> Result<Json<Vec<serde_json::Value>>, StatusCode> {
    let inner = state.lock().await;
    let sessions = inner
        .session_manager
        .list_sessions()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let result: Vec<serde_json::Value> = sessions
        .iter()
        .map(|s| {
            json!({
                "id": s.id,
                "created_at": s.created_at,
                "updated_at": s.updated_at,
                "message_count": s.messages.len()
            })
        })
        .collect();

    Ok(Json(result))
}

/// Get a specific session
async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let inner = state.lock().await;
    let session = inner
        .session_manager
        .load_session(&id)
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(json!({
        "id": session.id,
        "created_at": session.created_at,
        "updated_at": session.updated_at,
        "messages": session.messages
    })))
}

/// Delete a session
async fn delete_session(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let inner = state.lock().await;
    inner
        .session_manager
        .delete_session(&id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({"status": "deleted"})))
}
