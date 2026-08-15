//! M9 integration tests — MCP client + registry against the mock server
//! (port of `tests/runtime/test_mcp_integration.py`).

use helen_runtime::mcp::{MCPClient, MCPError, MCPToolRegistry};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// Serialize tests that touch the global MCP registry (Python tests use
/// module-global `_mcp_registry`; the Rust port uses a static).
static REGISTRY_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn registry_lock() -> &'static Mutex<()> {
    REGISTRY_LOCK.get_or_init(|| Mutex::new(()))
}

fn mock_server_path() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/mock_mcp_server.py"
    ))
}

fn python() -> &'static str {
    "python3"
}

fn client() -> MCPClient {
    MCPClient::new(
        "test",
        python(),
        vec![mock_server_path().to_string_lossy().to_string()],
        None,
        None,
        60,
    )
}

// ---------------------------------------------------------------------------
// TestMCPClient
// ---------------------------------------------------------------------------

#[test]
fn client_start_and_shutdown() {
    let mut c = client();
    c.start().unwrap();
    assert!(c.is_running());
    c.shutdown();
    assert!(!c.is_running());
}

#[test]
fn client_list_tools() {
    let mut c = client();
    c.start().unwrap();
    let tools = c.list_tools().unwrap();
    assert_eq!(tools.len(), 2);
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t.get("name").and_then(|n| n.as_str()))
        .collect();
    assert!(names.contains(&"echo"));
    assert!(names.contains(&"add"));
    c.shutdown();
}

#[test]
fn client_call_tool_echo() {
    let mut c = client();
    c.start().unwrap();
    let result = c
        .call_tool("echo", json!({"message": "Hello, MCP!"}))
        .unwrap();
    assert_eq!(result["output"], "Echo: Hello, MCP!");
    c.shutdown();
}

#[test]
fn client_call_tool_add() {
    let mut c = client();
    c.start().unwrap();
    let result = c.call_tool("add", json!({"a": 5, "b": 3})).unwrap();
    assert_eq!(result["result"], 8);
    c.shutdown();
}

#[test]
fn client_call_unknown_tool() {
    let mut c = client();
    c.start().unwrap();
    // Mock server returns error in result, not as JSON-RPC error.
    let result = c.call_tool("unknown_tool", json!({})).unwrap();
    assert!(result.get("error").is_some());
    c.shutdown();
}

#[test]
fn client_invalid_command() {
    let mut c = MCPClient::new("test", "nonexistent_command_xyz", vec![], None, None, 60);
    let err = c.start().unwrap_err();
    assert!(matches!(err, MCPError::Server(_)));
    assert!(err.message().contains("not found"));
}

#[test]
fn client_server_crash_wraps_clear_error() {
    // Server exits immediately → initialize times out → wrapped MCPServerError
    // (Python parity: "Failed to initialize MCP server 'crash': Timeout waiting
    //  for response from MCP server 'crash'").
    let mut c = MCPClient::new(
        "crash",
        "python3",
        vec!["-c".into(), "import sys; sys.exit(1)".into()],
        None,
        None,
        2,
    );
    let err = c.start().unwrap_err();
    assert!(matches!(err, MCPError::Server(_)));
    assert_eq!(
        err.message(),
        "Failed to initialize MCP server 'crash': Timeout waiting for response from MCP server 'crash'"
    );
    // Clean shutdown after crash.
    c.shutdown();
    assert!(!c.is_running());
}

// ---------------------------------------------------------------------------
// TestMCPToolRegistry
// ---------------------------------------------------------------------------

fn write_mcp_config(dir: &std::path::Path) -> PathBuf {
    let path = dir.join(".mcp.json");
    let config = json!({
        "mcpServers": {
            "mock": {
                "command": python(),
                "args": [mock_server_path().to_string_lossy().to_string()],
            }
        }
    });
    std::fs::write(&path, serde_json::to_string(&config).unwrap()).unwrap();
    path
}

#[test]
fn registry_initialize_and_get_tools() {
    let _g = registry_lock().lock().unwrap();
    let dir = std::env::temp_dir().join(format!("mcp_reg_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = write_mcp_config(&dir);

    let mut registry = MCPToolRegistry::new();
    registry.initialize(&config_path);
    assert!(registry.is_initialized());

    let tools = registry.get_tool_schemas();
    assert_eq!(tools.len(), 2);
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t["function"]["name"].as_str())
        .collect();
    assert!(names.contains(&"echo"));
    assert!(names.contains(&"add"));

    registry.shutdown();
}

#[test]
fn registry_dispatch_tool() {
    let _g = registry_lock().lock().unwrap();
    let dir = std::env::temp_dir().join(format!("mcp_disp_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = write_mcp_config(&dir);

    let mut registry = MCPToolRegistry::new();
    registry.initialize(&config_path);

    let result_json = registry.dispatch("echo", json!({"message": "Test"}));
    let result: Value = serde_json::from_str(&result_json).unwrap();
    assert_eq!(result["output"], "Echo: Test");

    registry.shutdown();
}

#[test]
fn registry_dispatch_unknown_tool() {
    let _g = registry_lock().lock().unwrap();
    let dir = std::env::temp_dir().join(format!("mcp_unk_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = write_mcp_config(&dir);

    let mut registry = MCPToolRegistry::new();
    registry.initialize(&config_path);

    let result_json = registry.dispatch("unknown_tool", json!({}));
    let result: Value = serde_json::from_str(&result_json).unwrap();
    assert!(result.get("error").is_some());

    registry.shutdown();
}

#[test]
fn registry_initialize_without_config() {
    let dir = std::env::temp_dir().join(format!("mcp_none_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = dir.join(".mcp.json"); // don't create the file

    let mut registry = MCPToolRegistry::new();
    registry.initialize(&config_path);

    // Should not crash, but not initialized.
    assert!(!registry.is_initialized());
    assert!(registry.get_tool_schemas().is_empty());
}

// ---------------------------------------------------------------------------
// TestMCPIntegration — tools.rs integration
// ---------------------------------------------------------------------------

#[test]
fn tools_integration_mcp_tools_available() {
    let _g = registry_lock().lock().unwrap();
    // Reset global MCP registry.
    helen_runtime::shutdown_mcp();

    let dir = std::env::temp_dir().join(format!("mcp_tools_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = write_mcp_config(&dir);

    // Initialize MCP (Python: tools.initialize_mcp).
    helen_runtime::initialize_mcp(&config_path);

    // Get tool schemas for MCP tool names (Python: get_tool_schemas([name])).
    let schemas = helen_runtime::get_tool_schemas(&["echo".to_string(), "add".to_string()]);
    let names: Vec<&str> = schemas
        .iter()
        .filter_map(|s| s["function"]["name"].as_str())
        .collect();
    assert!(names.contains(&"echo"));
    assert!(names.contains(&"add"));

    // Dispatch MCP tool (Python: dispatch_tool falls back to MCP).
    let result_json = helen_runtime::dispatch_tool("echo", &json!({"message": "Integration test"}));
    let result: Value = serde_json::from_str(&result_json).unwrap();
    assert_eq!(result["output"], "Echo: Integration test");

    helen_runtime::shutdown_mcp();
}

#[test]
fn tools_integration_unknown_tool_error() {
    let _g = registry_lock().lock().unwrap();
    helen_runtime::shutdown_mcp();

    // Without MCP initialized, unknown tool returns error JSON.
    let result_json = helen_runtime::dispatch_tool("unknown_tool", &json!({}));
    let result: Value = serde_json::from_str(&result_json).unwrap();
    assert!(result.get("error").is_some());

    helen_runtime::shutdown_mcp();
}

#[test]
fn tools_ensure_mcp_auto_discovers_from_cwd() {
    let _g = registry_lock().lock().unwrap();
    helen_runtime::shutdown_mcp();

    // Python `_ensure_mcp_initialized` reads `Path.cwd() / ".mcp.json"`.
    // Simulate monkeypatch.chdir(tmp_path) by changing the process cwd
    // (serialized under the registry lock so no other test races on cwd).
    let dir = std::env::temp_dir().join(format!("mcp_cwd_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let config_path = write_mcp_config(&dir);

    let orig_cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&dir).unwrap();
    let result = (|| {
        // `get_tool_schemas` triggers `ensure_mcp_initialized` which reads
        // cwd/.mcp.json and lazily starts the mock server.
        let schemas = helen_runtime::get_tool_schemas(&["echo".to_string()]);
        let names: Vec<&str> = schemas
            .iter()
            .filter_map(|s| s["function"]["name"].as_str())
            .collect();
        assert!(
            names.contains(&"echo"),
            "MCP auto-discovery failed: {names:?}"
        );

        // Dispatch also auto-initializes (Python `dispatch_mcp_tool`).
        let result_json = helen_runtime::dispatch_tool("add", &json!({"a": 2, "b": 3}));
        let result: Value = serde_json::from_str(&result_json).unwrap();
        assert_eq!(result["result"], 5);
    })();
    std::env::set_current_dir(&orig_cwd).unwrap();
    helen_runtime::shutdown_mcp();
    result;
    let _ = &config_path;
}
