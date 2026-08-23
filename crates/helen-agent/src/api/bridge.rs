//! Bridge validation API endpoints

use axum::{http::StatusCode, routing::post, Json, Router};
use serde::{Deserialize, Serialize};

use crate::api::sessions::AppState;
use crate::executor::execute_helen;

/// Bridge validation request
#[derive(Debug, Deserialize)]
pub struct BridgeValidationRequest {
    pub code: String,
}

/// Bridge validation response
#[derive(Debug, Serialize)]
pub struct BridgeValidationResponse {
    pub success: bool,
    pub output: Option<String>,
    pub error: Option<String>,
}

/// Create bridge router
pub fn router() -> Router<AppState> {
    Router::new().route("/validate", post(validate_bridge))
}

/// Validate Helen code via bridge
async fn validate_bridge(
    Json(req): Json<BridgeValidationRequest>,
) -> Result<Json<BridgeValidationResponse>, StatusCode> {
    match execute_helen(&req.code).await {
        Ok(output) => Ok(Json(BridgeValidationResponse {
            success: true,
            output: Some(output),
            error: None,
        })),
        Err(error) => Ok(Json(BridgeValidationResponse {
            success: false,
            output: None,
            error: Some(error),
        })),
    }
}
