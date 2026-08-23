//! Chat API endpoints

use axum::{routing::get, Json, Router};
use serde_json::json;

/// Create chat router
pub fn router() -> Router {
    Router::new().route("/status", get(get_status))
}

/// Get chat status
async fn get_status() -> Json<serde_json::Value> {
    Json(json!({
        "is_processing": false,
        "version": env!("CARGO_PKG_VERSION"),
    }))
}
