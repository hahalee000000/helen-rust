//! Files API endpoints

use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde_json::json;

use super::sessions::AppState;

/// Create files router
pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/files", post(upload_file))
        .route("/files", get(list_files))
        .route("/files/:id", get(download_file))
        .route("/files/:id", delete(delete_file))
        .with_state(state)
}

/// Upload a file
async fn upload_file(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut filename = String::new();
    let mut content = Vec::new();
    
    while let Some(field) = multipart.next_field().await.map_err(|_| StatusCode::BAD_REQUEST)? {
        let name = field.name().unwrap_or("").to_string();
        
        if name == "filename" {
            filename = field.text().await.map_err(|_| StatusCode::BAD_REQUEST)?;
        } else if name == "content" {
            let data = field.bytes().await.map_err(|_| StatusCode::BAD_REQUEST)?;
            content = data.to_vec();
        }
    }
    
    if filename.is_empty() || content.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    let inner = state.lock().await;
    let file_id = inner.file_storage.store_file(&filename, &content).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(json!({
        "id": file_id,
        "filename": filename,
        "size": content.len()
    })))
}

/// List all files
async fn list_files(State(state): State<AppState>) -> Result<Json<Vec<serde_json::Value>>, StatusCode> {
    let inner = state.lock().await;
    let files = inner.file_storage.list_files().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let result: Vec<serde_json::Value> = files
        .iter()
        .map(|f| {
            json!({
                "id": f.id,
                "filename": f.filename,
                "size": f.size,
                "created_at": f.created_at
            })
        })
        .collect();
    
    Ok(Json(result))
}

/// Download a file
async fn download_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let inner = state.lock().await;
    let content = inner.file_storage.retrieve_file(&id).map_err(|_| StatusCode::NOT_FOUND)?;
    
    Ok(content)
}

/// Delete a file
async fn delete_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let inner = state.lock().await;
    inner.file_storage.delete_file(&id).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(json!({"status": "deleted"})))
}
