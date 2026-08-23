//! Chat API endpoints
//!
//! Implements endpoints for:
//! - GET /api/chat/cwd - Get current working directory
//! - POST /api/chat/cwd - Set current working directory
//! - GET /api/chat/sessions - List all sessions
//! - GET /api/chat/sessions/:id/messages - Get messages for a session
//! - DELETE /api/chat/sessions/:id - Delete a session
//! - GET /api/chat/status - Chat status (includes is_processing)

use axum::{
    extract::{Multipart, Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::sessions::AppState;
use crate::server::mime_from_path;
use crate::transcript::TranscriptReader;
use crate::upload::UploadManager;

/// Create chat API router
pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/status", get(get_status))
        .route("/cwd", get(get_cwd))
        .route("/cwd", post(set_cwd))
        .route("/dir", get(get_directory))
        .route("/dir", post(set_directory))
        .route("/dir/messages", get(get_directory_messages))
        .route("/sessions", get(list_sessions))
        .route("/sessions/:id/messages", get(get_messages))
        .route("/sessions/:id/transcript", get(get_transcript))
        .route("/sessions/:id/media/:filename", get(get_session_media))
        .route("/sessions/:id", delete(delete_session))
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
pub async fn get_status(State(state): State<AppState>) -> impl IntoResponse {
    let inner = state.lock().await;
    let is_processing = inner.stream_registry.is_processing();

    Json(serde_json::json!({
        "is_processing": is_processing,
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

/// DELETE /api/chat/sessions/:id
///
/// Delete a session's transcript directory (cascading delete)
pub async fn delete_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    // Validate session_id to prevent path traversal
    if session_id.contains('/') || session_id.contains('\\') || session_id.contains("..") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid session ID"})),
        )
            .into_response();
    }

    let inner = state.lock().await;
    let sessions_dir = inner.directory_manager.get_sessions_dir();
    let reader = TranscriptReader::from_path(&sessions_dir);

    // Check if session exists
    if reader.get_transcript_path(&session_id).is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Session not found"})),
        )
            .into_response();
    }

    // Delete the session directory via TranscriptReader
    match reader.delete_session(&session_id) {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "ok", "message": "Session deleted"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to delete session: {}", e)})),
        )
            .into_response(),
    }
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
                crate::upload::UploadError::FileTooLarge { .. } => StatusCode::PAYLOAD_TOO_LARGE,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, Json(serde_json::json!({"error": e.to_string()}))).into_response()
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
            (status, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

// === Directory endpoints (aliases for /cwd with additional fields) ===

/// Response for GET /api/chat/dir
#[derive(Serialize)]
pub struct GetDirectoryResponse {
    pub cwd: String,
    pub display_name: String,
    pub session_id: String,
    pub helen_session_id: Option<String>,
}

/// GET /api/chat/dir
///
/// Get current working directory information (alias for /cwd with session_id)
pub async fn get_directory(State(state): State<AppState>) -> impl IntoResponse {
    let inner = state.lock().await;
    let cwd = inner.directory_manager.get_cwd();
    let display_name = inner.directory_manager.get_display_name(Some(cwd.clone()));
    let session_id = inner.directory_manager.cwd_to_session_id(&cwd);

    // Try to get Helen session ID
    let helen_session_id = None; // TODO: Query Helen bridge for session ID

    Json(GetDirectoryResponse {
        cwd,
        display_name,
        session_id,
        helen_session_id,
    })
}

/// Request for POST /api/chat/dir
#[derive(Deserialize)]
pub struct SetDirectoryRequest {
    pub path: String,
}

/// Response for POST /api/chat/dir
#[derive(Serialize)]
pub struct SetDirectoryResponse {
    pub status: String,
    pub cwd: String,
    pub display_name: String,
    pub session_id: String,
    pub helen_session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// POST /api/chat/dir
///
/// Change working directory (alias for /cwd with session_id)
pub async fn set_directory(
    State(state): State<AppState>,
    Json(request): Json<SetDirectoryRequest>,
) -> impl IntoResponse {
    let inner = state.lock().await;
    let result = inner.directory_manager.set_cwd(&request.path);

    let cwd = result.cwd.unwrap_or_default();
    let session_id = inner.directory_manager.cwd_to_session_id(&cwd);
    let helen_session_id = None; // TODO: Query Helen bridge for session ID

    Json(SetDirectoryResponse {
        status: result.status,
        cwd,
        display_name: result.display_name.unwrap_or_default(),
        session_id,
        helen_session_id,
        message: result.message,
    })
}

/// GET /api/chat/dir/messages
///
/// Get message history for current working directory (from transcript)
pub async fn get_directory_messages(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<MessagesQuery>,
) -> impl IntoResponse {
    let inner = state.lock().await;
    let sessions_dir = inner.directory_manager.get_sessions_dir();
    let reader = TranscriptReader::from_path(&sessions_dir);

    // Get current session's messages
    let cwd = inner.directory_manager.get_cwd();
    let session_id = inner.directory_manager.cwd_to_session_id(&cwd);
    let messages = reader.to_messages(&session_id);

    let api_messages: Vec<Message> = messages
        .into_iter()
        .skip(params.offset.unwrap_or(0))
        .take(params.limit.unwrap_or(100))
        .map(|m| Message {
            id: m.id,
            role: m.role,
            content: m.content,
            timestamp: m.timestamp,
            attachments: m.attachments,
        })
        .collect();

    Json(api_messages)
}

/// Query parameters for messages endpoint
#[derive(Deserialize, Default)]
pub struct MessagesQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

// === Transcript and media endpoints ===

/// GET /api/chat/sessions/:id/transcript
///
/// Get raw Helen transcript (complete LLM context record)
pub async fn get_transcript(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    // Validate session_id to prevent path traversal
    if session_id.contains('/') || session_id.contains('\\') || session_id.contains("..") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid session ID"})),
        )
            .into_response();
    }

    let inner = state.lock().await;
    let sessions_dir = inner.directory_manager.get_sessions_dir();
    let reader = TranscriptReader::from_path(&sessions_dir);

    // Get transcript path
    let transcript_path = match reader.get_transcript_path(&session_id) {
        Some(path) => path,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Session not found"})),
            )
                .into_response();
        }
    };

    // Read transcript file
    let mut entries = Vec::new();
    let mut line_num = 0;

    if let Ok(file) = std::fs::File::open(&transcript_path) {
        use std::io::BufRead;
        let reader = std::io::BufReader::new(file);
        for line in reader.lines() {
            line_num += 1;
            if let Ok(line) = line {
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<serde_json::Value>(&line) {
                    Ok(mut entry) => {
                        entry["_line"] = serde_json::json!(line_num);
                        entries.push(entry);
                    }
                    Err(e) => {
                        entries.push(serde_json::json!({
                            "type": "parse_error",
                            "line": line_num,
                            "error": e.to_string(),
                            "raw": &line[..line.len().min(200)]
                        }));
                    }
                }
            }
        }
    }

    // Filter out test messages and metadata
    entries.retain(|e| {
        if let Some(content) = e.get("content").and_then(|c| c.as_str()) {
            if content.starts_with("[TEST]") {
                return false;
            }
        }
        e.get("type").and_then(|t| t.as_str()) != Some("session_meta")
    });

    // Count roles and tool calls
    let mut roles: HashMap<String, usize> = HashMap::new();
    let mut tool_calls_count = 0;

    for e in &entries {
        if e.get("type").and_then(|t| t.as_str()) == Some("message") {
            if let Some(role) = e.get("role").and_then(|r| r.as_str()) {
                *roles.entry(role.to_string()).or_insert(0) += 1;
            }
            if let Some(tool_calls) = e.get("tool_calls") {
                if let Some(arr) = tool_calls.as_array() {
                    tool_calls_count += arr.len();
                }
            } else if let Some(content) = e.get("content").and_then(|c| c.as_str()) {
                if content.starts_with("Tool calls:") {
                    // Count function calls in text format
                    tool_calls_count += content.matches('(').count();
                }
            }
        }
    }

    Json(serde_json::json!({
        "session_id": session_id,
        "file": transcript_path.to_string_lossy(),
        "total_entries": entries.len(),
        "roles": roles,
        "tool_calls_count": tool_calls_count,
        "entries": entries,
    }))
    .into_response()
}

/// GET /api/chat/sessions/:id/media/:filename
///
/// Serve media files (images, audio, etc.) from session attachments
pub async fn get_session_media(
    State(state): State<AppState>,
    Path((session_id, filename)): Path<(String, String)>,
) -> impl IntoResponse {
    // Validate session_id and filename to prevent path traversal
    if session_id.contains('/') || session_id.contains('\\') || session_id.contains("..") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid session ID"})),
        )
            .into_response();
    }

    if filename.contains('/')
        || filename.contains('\\')
        || filename.contains("..")
        || filename.is_empty()
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid filename"})),
        )
            .into_response();
    }

    let inner = state.lock().await;
    let sessions_dir = inner.directory_manager.get_sessions_dir();
    let reader = TranscriptReader::from_path(&sessions_dir);

    // Get transcript path to find media directory
    let transcript_path = match reader.get_transcript_path(&session_id) {
        Some(path) => path,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Session not found"})),
            )
                .into_response();
        }
    };

    let media_dir = transcript_path.parent().unwrap().join("media");
    let media_path = media_dir.join(&filename);

    // Security check: ensure file is within media directory
    let real_media = match std::fs::canonicalize(&media_path) {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Media file not found"})),
            )
                .into_response();
        }
    };

    let real_media_dir = match std::fs::canonicalize(&media_dir) {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Media directory not found"})),
            )
                .into_response();
        }
    };

    if !real_media.starts_with(&real_media_dir) {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"error": "Access denied"})),
        )
            .into_response();
    }

    if !media_path.exists() || !media_path.is_file() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Media file not found"})),
        )
            .into_response();
    }

    // Determine MIME type
    let mime = mime_from_path(&filename);

    // Read and serve file
    match std::fs::read(&media_path) {
        Ok(content) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime)
            .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
            .body(axum::body::Body::from(content))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
