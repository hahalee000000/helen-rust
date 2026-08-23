//! Embedded agent .helen files

use rust_embed::RustEmbed;

/// Embedded agent assets
#[derive(RustEmbed)]
#[folder = "agent/"]
struct AgentAssets;

/// List all embedded agent files
pub fn list_embedded_agents() -> Vec<String> {
    AgentAssets::iter()
        .filter(|f| f.ends_with(".helen"))
        .map(|f| f.to_string())
        .collect()
}

/// Get the content of an embedded agent file
pub fn get_embedded_agent(name: &str) -> Option<String> {
    AgentAssets::get(name).map(|f| String::from_utf8_lossy(&f.data).to_string())
}
