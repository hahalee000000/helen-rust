//! MCP (Model Context Protocol) server manager.
//!
//! Port of `helen/runtime/mcp/server_manager.py` — manages multiple MCP
//! server instances and their tools.

use super::client::MCPClient;
use super::config::MCPConfig;
use super::errors::MCPError;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

/// Manages multiple MCP server instances.
///
/// Responsibilities:
/// - Loading configuration from `.mcp.json`
/// - Starting and stopping MCP servers
/// - Discovering tools from all servers
/// - Routing tool calls to the appropriate server
pub struct MCPServerManager {
    /// server_name -> client
    pub clients: HashMap<String, MCPClient>,
    /// tool_name -> server_name
    tool_mapping: HashMap<String, String>,
}

impl MCPServerManager {
    pub fn new() -> Self {
        MCPServerManager {
            clients: HashMap::new(),
            tool_mapping: HashMap::new(),
        }
    }

    /// `load_config(config_path)` — load MCP configuration from `.mcp.json`.
    pub fn load_config(&mut self, config_path: &Path) {
        let config = MCPConfig::from_file(config_path);

        for server_config in config.servers {
            let client = MCPClient::new(
                &server_config.name,
                &server_config.command,
                server_config.args.clone(),
                server_config.cwd.clone(),
                server_config.env_vars.clone(),
                server_config.tool_timeout_sec,
            );
            self.clients.insert(server_config.name.clone(), client);
        }
    }

    /// `start_all()` — start all MCP servers and discover their tools.
    ///
    /// Servers that fail to start are logged and skipped.
    /// Tool name conflicts are logged as warnings (first server wins).
    pub fn start_all(&mut self) {
        // Snapshot names to avoid borrow issues while iterating.
        let names: Vec<String> = self.clients.keys().cloned().collect();
        for name in names {
            let Some(client) = self.clients.get_mut(&name) else {
                continue;
            };
            match client.start() {
                Ok(()) => {}
                Err(e) => {
                    eprintln!("Failed to start MCP server '{}': {}", name, e.message());
                    continue;
                }
            }

            // Discover tools.
            match client.list_tools() {
                Ok(tools) => {
                    for tool in tools {
                        let Some(tool_name) = tool.get("name").and_then(|n| n.as_str()) else {
                            continue;
                        };
                        let tool_name = tool_name.to_string();
                        if self.tool_mapping.contains_key(&tool_name) {
                            eprintln!(
                                "MCP tool '{}' from server '{}' conflicts with server '{}' (first server wins)",
                                tool_name,
                                name,
                                self.tool_mapping[&tool_name]
                            );
                        } else {
                            self.tool_mapping.insert(tool_name.clone(), name.clone());
                        }
                    }
                }
                Err(e) => {
                    eprintln!(
                        "Failed to list tools from MCP server '{}': {}",
                        name,
                        e.message()
                    );
                }
            }
        }
    }

    /// `get_all_tools()` — get OpenAI-format tool schemas for all MCP tools.
    pub fn get_all_tools(&mut self) -> Vec<Value> {
        let mut tools = Vec::new();
        let names: Vec<String> = self.clients.keys().cloned().collect();

        for server_name in names {
            let Some(client) = self.clients.get_mut(&server_name) else {
                continue;
            };
            match client.list_tools() {
                Ok(mcp_tools) => {
                    for tool in mcp_tools {
                        // Convert MCP tool schema to OpenAI format.
                        let openai_tool = json!({
                            "type": "function",
                            "function": {
                                "name": tool.get("name").cloned().unwrap_or(Value::Null),
                                "description": tool
                                    .get("description")
                                    .and_then(|d| d.as_str())
                                    .unwrap_or(""),
                                "parameters": tool
                                    .get("inputSchema")
                                    .cloned()
                                    .unwrap_or(Value::Object(Default::default())),
                            },
                        });
                        tools.push(openai_tool);
                    }
                }
                Err(e) => {
                    eprintln!(
                        "Failed to list tools from MCP server '{}': {}",
                        server_name,
                        e.message()
                    );
                }
            }
        }
        tools
    }

    /// `call_tool(tool_name, arguments)` — call an MCP tool by name.
    pub fn call_tool(&mut self, tool_name: &str, arguments: Value) -> Result<Value, MCPError> {
        let server_name = self
            .tool_mapping
            .get(tool_name)
            .cloned()
            .ok_or_else(|| MCPError::Server(format!("Unknown MCP tool: {tool_name}")))?;

        let client = self
            .clients
            .get_mut(&server_name)
            .ok_or_else(|| MCPError::Server(format!("MCP server '{server_name}' not found")))?;

        client.call_tool(tool_name, arguments)
    }

    /// `shutdown_all()` — shut down all MCP servers.
    pub fn shutdown_all(&mut self) {
        let names: Vec<String> = self.clients.keys().cloned().collect();
        for name in names {
            if let Some(client) = self.clients.get_mut(&name) {
                client.shutdown();
            }
        }
        self.clients.clear();
        self.tool_mapping.clear();
    }
}

impl Default for MCPServerManager {
    fn default() -> Self {
        Self::new()
    }
}
