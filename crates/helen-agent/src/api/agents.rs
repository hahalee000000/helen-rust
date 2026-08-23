//! Agent management API endpoints
//!
//! Implements:
//! - GET /api/agents/status — all agents status
//! - GET /api/agents/list — list available agents
//! - GET /api/agents/:name/status — single agent status

use axum::{
    extract::Path,
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde_json::json;

use super::sessions::AppState;

/// Create agents API router
pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/status", get(get_all_agents_status))
        .route("/list", get(list_agents))
        .route("/:name/status", get(get_agent_status))
        .with_state(state)
}

/// GET /api/agents/status — get all agents status
pub async fn get_all_agents_status() -> Json<serde_json::Value> {
    // Mock data matching Python implementation
    // In a real implementation, this would query the Helen runtime
    let agent_states = json!({
        "Contractor": {"status": "idle", "last_task": null},
        "TestBuilder": {"status": "idle", "last_task": null},
        "Implementer": {"status": "idle", "last_task": null},
        "QualityGate": {"status": "idle", "last_task": null},
        "SkillEvaluator": {"status": "idle", "last_task": null}
    });

    Json(agent_states)
}

/// GET /api/agents/list — list all available agents
pub async fn list_agents() -> Json<Vec<String>> {
    Json(vec![
        "Contractor".to_string(),
        "TestBuilder".to_string(),
        "Implementer".to_string(),
        "QualityGate".to_string(),
        "SkillEvaluator".to_string(),
    ])
}

/// GET /api/agents/:name/status — get single agent status
pub async fn get_agent_status(
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let known_agents = [
        "Contractor",
        "TestBuilder",
        "Implementer",
        "QualityGate",
        "SkillEvaluator",
    ];

    if !known_agents.contains(&name.as_str()) {
        return Ok(Json(json!({
            "error": format!("Agent {} not found", name)
        })));
    }

    Ok(Json(json!({
        "name": name,
        "status": "idle",
        "last_task": null
    })))
}
