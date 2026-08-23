//! Chat API endpoints
//!
//! Implements read-only endpoints for Phase 2:
//! - GET /api/chat/cwd - Get current working directory
//! - POST /api/chat/cwd - Set current working directory
//! - GET /api/chat/sessions - List all sessions
//! - GET /api/chat/sessions/:id/messages - Get messages for a session

use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::transcript::TranscriptReader;
use super::sessions::AppState;

/// Create chat API router
pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/status", get(get_status))
        .route("/cwd", get(get_cwd))
        .route("/cwd", post(set_cwd))
        .route("/sessions", get(list_sessions))
        .route("/sessions/:id/messages", get(get_messages))
        .with_state(state)
}

/// Response for GET /api/chat/cwd
#[derive(Serialize)]
pub struct GetCwdResponse {
    pub cwd: String,
    pub display_name: String,
}

/// Request for POST /api/chat/cwd
#[derive(Deserialize)]
pub struct SetCwdRequest {
    pub cwd: String,
}

/// Response for POST /api/chat/cwd
#[derive(Serialize)]
pub struct SetCwdResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Response for GET /api/chat/sessions
#[derive(Serialize)]
pub struct ListSessionsResponse {
    pub sessions: Vec<SessionInfo>,
}

/// Session information
#[derive(Serialize)]
pub struct SessionInfo {
    pub id: String,
    pub message_count: usize,
    pub preview: String,
    pub timestamp: Option<String>,
}

/// Response for GET /api/chat/sessions/:id/messages
#[derive(Serialize)]
pub struct GetMessagesResponse {
    pub messages: Vec<Message>,
}

/// Message structure
#[derive(Serialize)]
pub struct Message {
    pub id: String,
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
    #[serde(default)]
    pub attachments: Vec<serde_json::Value>,
}

/// GET /api/chat/status
pub async fn get_status() -> impl IntoResponse {
    Json(serde_json::json!({
        "is_processing": false,
        "version": env!("CARGO_PKG_VERSION")
    }))
}

/// GET /api/chat/cwd
pub async fn get_cwd(State(state): State<AppState>) -> impl IntoResponse {
    let inner = state.lock().await;
    let cwd = inner.directory_manager.get_cwd();
    let display_name = inner.directory_manager.get_display_name(Some(cwd.clone()));

    Json(GetCwdResponse { cwd, display_name })
}

/// POST /api/chat/cwd
pub async fn set_cwd(
    State(state): State<AppState>,
    Json(request): Json<SetCwdRequest>,
) -> impl IntoResponse {
    let inner = state.lock().await;
    let result = inner.directory_manager.set_cwd(&request.cwd);

    Json(SetCwdResponse {
        status: result.status,
        cwd: result.cwd,
        display_name: result.display_name,
        message: result.message,
    })
}

/// GET /api/chat/sessions
pub async fn list_sessions(State(state): State<AppState>) -> impl IntoResponse {
    let inner = state.lock().await;
    let sessions_dir = inner.directory_manager.get_sessions_dir();
    let reader = TranscriptReader::from_path(&sessions_dir);
    let sessions = reader.list_sessions();

    let session_infos: Vec<SessionInfo> = sessions
        .into_iter()
        .map(|s| SessionInfo {
            id: s.id,
            message_count: s.message_count,
            preview: s.preview,
            timestamp: s.timestamp,
        })
        .collect();

    Json(ListSessionsResponse {
        sessions: session_infos,
    })
}

/// GET /api/chat/sessions/:id/messages
pub async fn get_messages(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let inner = state.lock().await;
    let sessions_dir = inner.directory_manager.get_sessions_dir();
    let reader = TranscriptReader::from_path(&sessions_dir);
    let messages = reader.to_messages(&session_id);

    let api_messages: Vec<Message> = messages
        .into_iter()
        .map(|m| Message {
            id: m.id,
            role: m.role,
            content: m.content,
            timestamp: m.timestamp,
            attachments: m.attachments,
        })
        .collect();

    Json(GetMessagesResponse {
        messages: api_messages,
    })
}
