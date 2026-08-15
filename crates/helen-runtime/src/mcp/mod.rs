//! MCP (Model Context Protocol) client for Helen.
//!
//! Port of `helen/runtime/mcp/` — allows Helen to discover and call tools
//! from external MCP servers over JSON-RPC 2.0 (stdio).
//!
//! ```no_run
//! use helen_runtime::mcp::MCPToolRegistry;
//!
//! let mut registry = MCPToolRegistry::new();
//! registry.initialize(std::path::Path::new(".mcp.json"));
//!
//! // Get tool schemas (OpenAI format)
//! let tools = registry.get_tool_schemas();
//!
//! // Call a tool
//! let result = registry.dispatch("echo", serde_json::json!({"message": "Hello"}));
//!
//! // Cleanup
//! registry.shutdown();
//! ```

pub mod client;
pub mod config;
pub mod errors;
pub mod registry;
pub mod server_manager;

pub use client::MCPClient;
pub use config::{MCPConfig, MCPServerConfig};
pub use errors::MCPError;
pub use registry::MCPToolRegistry;
pub use server_manager::MCPServerManager;
