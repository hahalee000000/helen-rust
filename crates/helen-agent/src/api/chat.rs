//! Chat API endpoints
//!
//! Implements read-only endpoints for Phase 2:
//! - GET /api/chat/cwd - Get current working directory
//! - POST /api/chat/cwd - Set current working directory
//! - GET /api/chat/sessions - List all sessions
//! - GET /api/chat/sessions/:id/messages - Get messages for a session

use axum::{
    extract::{Multipart, Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::transcript::TranscriptReader;
use crate::upload::UploadManager;
use super::sessions::AppState;

/// Create chat API router
pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/status", get(get_status))
        .route("/cwd", get(get_cwd))
        .route("/cwd", post(set_cwd))
        .route("/sessions", get(list_sessions))
        .route("/sessions/:id/messages", get(get_messages))
        .route("/upload", post(upload_file))
        .route("/uploads/:id/file", get(get_upload_file))
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

/// POST /api/chat/upload
///
/// Accept multipart/form-data file upload, save to .helen/uploads/<upload_id>/
pub async fn upload_file(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut filename: Option<String> = None;
    let mut content_type: Option<String> = None;
    let mut content: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let name = field.name().unwrap_or("").to_string();

        if name == "file" {
            filename = field.file_name().map(|s| s.to_string());
            content_type = field.content_type().map(|s| s.to_string());
            let data = field.bytes().await.unwrap_or_default();
            content = Some(data.to_vec());
        }
    }

    let filename = match filename {
        Some(f) if !f.is_empty() => f,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "No file provided"})),
            )
                .into_response();
        }
    };

    let content_type = match content_type {
        Some(ct) if !ct.is_empty() => ct,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "No content type provided"})),
            )
                .into_response();
        }
    };

    let content = match content {
        Some(c) if !c.is_empty() => c,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Empty file content"})),
            )
                .into_response();
        }
    };

    let inner = state.lock().await;
    let upload_mgr = inner.upload_manager.clone();
    drop(inner);

    match upload_mgr.save_upload(&filename, &content_type, &content) {
        Ok(result) => (StatusCode::OK, Json(serde_json::to_value(result).unwrap())).into_response(),
        Err(e) => {
            let status = match &e {
                crate::upload::UploadError::UnsupportedMimeType(_) => StatusCode::BAD_REQUEST,
                crate::upload::UploadError::FileTooLarge { .. } => {
                    StatusCode::PAYLOAD_TOO_LARGE
                }
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (
                status,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

/// GET /api/chat/uploads/:id/file
///
/// Serve an uploaded file with correct MIME type
pub async fn get_upload_file(
    State(state): State<AppState>,
    Path(upload_id): Path<String>,
) -> impl IntoResponse {
    // Validate upload_id first (before accessing state)
    if UploadManager::validate_upload_id(&upload_id).is_err() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid upload_id"})),
        )
            .into_response();
    }

    let inner = state.lock().await;
    let upload_mgr = inner.upload_manager.clone();
    drop(inner);

    match upload_mgr.read_upload_file(&upload_id) {
        Ok((content, mime_type)) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime_type)
            .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
            .body(axum::body::Body::from(content))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
        Err(e) => {
            let status = match &e {
                crate::upload::UploadError::FileNotFound
                | crate::upload::UploadError::MetadataNotFound => StatusCode::NOT_FOUND,
                crate::upload::UploadError::AccessDenied => StatusCode::FORBIDDEN,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (
                status,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}
