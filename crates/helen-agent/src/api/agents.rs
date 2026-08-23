//! Agent management API endpoints
//!
//! Implements:
//! - GET /api/agents/status — all agents status
//! - GET /api/agents/list — list available agents
//! - GET /api/agents/:name/status — single agent status
//!
//! TODO: Replace mock data with actual Helen runtime queries.
//! Currently returns hardcoded agent states matching the Python implementation.
//! When the Helen runtime exposes agent lifecycle APIs, these endpoints should
//! query real agent state (running, idle, error, last_task, etc.).

use axum::{
    extract::Path,
    routing::get,
    Json, Router,
};
use serde_json::json;

use super::sessions::AppState;

/// Known agent names (matches Python Helen implementation)
const KNOWN_AGENTS: &[&str] = &[
    "Contractor",
    "TestBuilder",
    "Implementer",
    "QualityGate",
    "SkillEvaluator",
];

/// Create agents API router
pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/status", get(get_all_agents_status))
        .route("/list", get(list_agents))
        .route("/:name/status", get(get_agent_status))
        .with_state(state)
}

/// GET /api/agents/status — get all agents status
///
/// TODO: Query actual Helen runtime for real agent states
pub async fn get_all_agents_status() -> Json<serde_json::Value> {
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
    Json(KNOWN_AGENTS.iter().map(|s| s.to_string()).collect())
}

/// GET /api/agents/:name/status — get single agent status
///
/// TODO: Query actual Helen runtime for real agent state
pub async fn get_agent_status(
    Path(name): Path<String>,
) -> Json<serde_json::Value> {
    if !KNOWN_AGENTS.contains(&name.as_str()) {
        return Json(json!({
            "error": format!("Agent {} not found", name)
        }));
    }

    Json(json!({
        "name": name,
        "status": "idle",
        "last_task": null
    }))
}
