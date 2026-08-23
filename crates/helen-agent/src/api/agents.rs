//! Agents API endpoints

use axum::{routing::get, Json, Router};

/// Create agents router
pub fn router() -> Router {
    Router::new().route("/list", get(list_agents))
}

/// List all available agents
async fn list_agents() -> Json<Vec<String>> {
    Json(vec![
        "Contractor".to_string(),
        "TestBuilder".to_string(),
        "Implementer".to_string(),
        "QualityGate".to_string(),
        "SkillEvaluator".to_string(),
    ])
}
