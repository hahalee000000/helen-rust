//! Agent management API endpoints
//!
//! Implements endpoints for:
//! - GET /api/agents/status - Get all agent statuses
//! - GET /api/agents/:name/status - Get specific agent status
//! - GET /api/agents/list - List all available agents

use axum::{
    extract::Path,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Serialize;
use std::collections::HashMap;

use super::sessions::AppState;

/// Create agents API router
pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/status", get(get_all_agents_status))
        .route("/:agent_name/status", get(get_agent_status))
        .route("/list", get(list_agents))
        .with_state(state)
}

/// Agent status information
#[derive(Serialize, Clone)]
pub struct AgentStatus {
    pub status: String,
    pub last_task: Option<String>,
}

/// Response for GET /api/agents/status
pub type AgentsStatusResponse = HashMap<String, AgentStatus>;

/// Response for GET /api/agents/:name/status
#[derive(Serialize)]
pub struct AgentStatusResponse {
    pub name: String,
    pub status: String,
    pub last_task: Option<String>,
}

/// Response for GET /api/agents/list
#[derive(Serialize)]
pub struct ListAgentsResponse {
    pub agents: Vec<String>,
}

/// Mock agent states (TODO: Query actual Helen runtime for real agent states)
fn get_mock_agent_states() -> HashMap<String, AgentStatus> {
    let mut states = HashMap::new();
    states.insert(
        "Contractor".to_string(),
        AgentStatus {
            status: "idle".to_string(),
            last_task: None,
        },
    );
    states.insert(
        "TestBuilder".to_string(),
        AgentStatus {
            status: "idle".to_string(),
            last_task: None,
        },
    );
    states.insert(
        "Implementer".to_string(),
        AgentStatus {
            status: "idle".to_string(),
            last_task: None,
        },
    );
    states.insert(
        "QualityGate".to_string(),
        AgentStatus {
            status: "idle".to_string(),
            last_task: None,
        },
    );
    states.insert(
        "SkillEvaluator".to_string(),
        AgentStatus {
            status: "idle".to_string(),
            last_task: None,
        },
    );
    states
}

/// GET /api/agents/status
pub async fn get_all_agents_status() -> Json<AgentsStatusResponse> {
    Json(get_mock_agent_states())
}

/// GET /api/agents/:agent_name/status
pub async fn get_agent_status(
    Path(agent_name): Path<String>,
) -> impl axum::response::IntoResponse {
    let states = get_mock_agent_states();
    match states.get(&agent_name) {
        Some(agent_status) => Json(serde_json::json!({
            "name": agent_name,
            "status": agent_status.status,
            "last_task": agent_status.last_task,
        }))
        .into_response(),
        None => (
            axum::http::StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("Agent {} not found", agent_name)})),
        )
            .into_response(),
    }
}

/// GET /api/agents/list
pub async fn list_agents() -> Json<Vec<String>> {
    let states = get_mock_agent_states();
    Json(states.keys().cloned().collect())
}
