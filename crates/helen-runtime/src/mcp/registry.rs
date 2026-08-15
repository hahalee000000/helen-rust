//! MCP (Model Context Protocol) tool registry.
//!
//! Port of `helen/runtime/mcp/registry.py` — integrates MCP tools into
//! Helen's tool system.

use super::server_manager::MCPServerManager;
use serde_json::{json, Value};
use std::path::Path;

/// Integrates MCP tools into Helen's tool system.
///
/// Manages the lifecycle of MCP servers and provides a unified interface
/// for discovering and calling MCP tools.
pub struct MCPToolRegistry {
    manager: MCPServerManager,
    initialized: bool,
}

impl MCPToolRegistry {
    pub fn new() -> Self {
        MCPToolRegistry {
            manager: MCPServerManager::new(),
            initialized: false,
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// `initialize(config_path)` — initialize MCP servers and register tools.
    ///
    /// Python parity:
    /// - Safe to call multiple times (idempotent).
    /// - Errors are logged but don't raise (MCP is optional).
    /// - Only marked as initialized if at least one server starts.
    pub fn initialize(&mut self, config_path: &Path) {
        if self.initialized {
            return;
        }

        self.manager.load_config(config_path);

        // Check if any servers were loaded.
        if self.manager.clients.is_empty() {
            return;
        }

        self.manager.start_all();

        // Only mark as initialized if at least one server is running.
        if !self.manager.clients.is_empty() {
            self.initialized = true;
        }
    }

    /// `get_tool_schemas()` — get OpenAI-format tool schemas for all MCP tools.
    pub fn get_tool_schemas(&mut self) -> Vec<Value> {
        if !self.initialized {
            return Vec::new();
        }
        self.manager.get_all_tools()
    }

    /// `dispatch(tool_name, arguments)` — dispatch a tool call to the
    /// appropriate MCP server, returning a JSON string compatible with
    /// Helen's tool system.
    pub fn dispatch(&mut self, tool_name: &str, arguments: Value) -> String {
        if !self.initialized {
            return json!({
                "error": format!("MCP tool '{}' not available (registry not initialized)", tool_name),
            })
            .to_string();
        }

        match self.manager.call_tool(tool_name, arguments) {
            Ok(result) => serde_json::to_string(&result).unwrap_or_else(|_| "{}".into()),
            Err(e) => json!({ "error": e.message() }).to_string(),
        }
    }

    /// `shutdown()` — shut down all MCP servers.
    pub fn shutdown(&mut self) {
        if !self.initialized {
            return;
        }
        self.manager.shutdown_all();
        self.initialized = false;
    }
}

impl Default for MCPToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
