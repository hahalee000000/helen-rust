//! MCP (Model Context Protocol) client exceptions.
//!
//! Port of `helen/runtime/mcp/exceptions.py` — the MCP error hierarchy.
//! Python has `MCPError < MCPServerError`, `MCPError < MCPToolError`,
//! `MCPError < MCPTimeoutError`. In Rust we model this as an enum where
//! `matches!` covers the base-class behavior.

use std::fmt;

/// Base MCP error (Python `MCPError`).
#[derive(Debug, Clone, PartialEq)]
pub enum MCPError {
    /// `MCPServerError` — server process/communication failures.
    Server(String),
    /// `MCPToolError` — tool not found or tool execution failure.
    Tool(String),
    /// `MCPTimeoutError` — no response within the timeout.
    Timeout(String),
}

impl MCPError {
    /// The human-readable message (Python `str(e)`).
    pub fn message(&self) -> &str {
        match self {
            MCPError::Server(m) | MCPError::Tool(m) | MCPError::Timeout(m) => m,
        }
    }
}

impl fmt::Display for MCPError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for MCPError {}

/// `isinstance(e, MCPError)` — all variants are MCP errors.
pub fn is_mcp_error(_e: &MCPError) -> bool {
    true
}
