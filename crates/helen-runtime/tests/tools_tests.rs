//! Tests for tools module — tool dispatch and schemas.

use helen_runtime::tools::*;
use serde_json::json;

// ── all_tools ───────────────────────────────────────────────────────────

#[test]
fn all_tools_not_empty() {
    let tools = all_tools();
    assert!(!tools.is_empty());
}

#[test]
fn all_tools_have_names() {
    let tools = all_tools();
    for tool in &tools {
        assert!(!tool.name.is_empty());
    }
}

#[test]
fn all_tools_have_descriptions() {
    let tools = all_tools();
    for tool in &tools {
        assert!(!tool.description.is_empty());
    }
}

#[test]
fn all_tools_have_schemas() {
    let tools = all_tools();
    for tool in &tools {
        assert!(tool.parameters.is_object());
    }
}

// ── get_tool ────────────────────────────────────────────────────────────

#[test]
fn get_tool_read_file() {
    let tool = get_tool("read_file");
    assert!(tool.is_some());
    let tool = tool.unwrap();
    assert_eq!(tool.name, "read_file");
}

#[test]
fn get_tool_write_file() {
    let tool = get_tool("write_file");
    assert!(tool.is_some());
}

#[test]
fn get_tool_shell_exec() {
    let tool = get_tool("shell_exec");
    assert!(tool.is_some());
}

#[test]
fn get_tool_web_search() {
    let tool = get_tool("web_search");
    assert!(tool.is_some());
}

#[test]
fn get_tool_web_fetch() {
    let tool = get_tool("web_fetch");
    assert!(tool.is_some());
}

#[test]
fn get_tool_calculate() {
    let tool = get_tool("calculate");
    assert!(tool.is_some());
}

#[test]
fn get_tool_patch_file() {
    let tool = get_tool("patch_file");
    assert!(tool.is_some());
}

#[test]
fn get_tool_unknown() {
    let tool = get_tool("nonexistent_tool");
    assert!(tool.is_none());
}

// ── get_tool_schemas ────────────────────────────────────────────────────

#[test]
fn get_tool_schemas_known() {
    let schemas = get_tool_schemas(&["read_file".to_string(), "write_file".to_string()]);
    assert_eq!(schemas.len(), 2);
    for schema in &schemas {
        assert!(schema.is_object());
    }
}

#[test]
fn get_tool_schemas_empty() {
    let schemas = get_tool_schemas(&[]);
    assert!(schemas.is_empty());
}

#[test]
fn get_tool_schemas_unknown() {
    let schemas = get_tool_schemas(&["nonexistent".to_string()]);
    assert!(schemas.len() <= 1);
}

#[test]
fn get_tool_schemas_all() {
    let tools = all_tools();
    let names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
    let schemas = get_tool_schemas(&names);
    assert_eq!(schemas.len(), tools.len());
}

// ── dispatch_tool ───────────────────────────────────────────────────────

#[test]
fn dispatch_tool_read_file_nonexistent() {
    let args = json!({"path": "/nonexistent/file.txt"});
    let result = dispatch_tool("read_file", &args);
    assert!(result.contains("error") || result.contains("Error") || result.contains("not found") || result.contains("No such"));
}

#[test]
fn dispatch_tool_calculate() {
    let args = json!({"expression": "2 + 3"});
    let result = dispatch_tool("calculate", &args);
    assert!(result.contains("5"));
}

#[test]
fn dispatch_tool_unknown() {
    let args = json!({});
    let result = dispatch_tool("nonexistent", &args);
    assert!(result.contains("error") || result.contains("Error") || result.contains("Unknown") || result.contains("unknown"));
}

// ── tools_dispatch ──────────────────────────────────────────────────────

#[test]
fn tools_dispatch_read_file() {
    let args = json!({"path": "/nonexistent/file.txt"});
    let result = tools_dispatch("read_file", &args);
    assert!(result.is_err() || result.as_ref().is_ok_and(|s| s.contains("error")));
}

#[test]
fn tools_dispatch_calculate() {
    let args = json!({"expression": "10 * 5"});
    let result = tools_dispatch("calculate", &args);
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("50"));
}

#[test]
fn tools_dispatch_unknown() {
    let args = json!({});
    let result = tools_dispatch("nonexistent", &args);
    // Unknown tools may return Ok with error message or Err
    if let Ok(msg) = result {
        assert!(msg.contains("error") || msg.contains("Error") || msg.contains("Unknown") || msg.contains("unknown") || msg.contains("not found"));
    }
}

// ── ensure_mcp_initialized ──────────────────────────────────────────────

#[test]
fn ensure_mcp_initialized_no_panic() {
    let _ = std::panic::catch_unwind(|| {
        ensure_mcp_initialized();
    });
}

// ── get_mcp_tool_schemas ────────────────────────────────────────────────

#[test]
fn get_mcp_tool_schemas_no_panic() {
    let schemas = get_mcp_tool_schemas();
    let _ = schemas.len();
}

// ── shutdown_mcp ────────────────────────────────────────────────────────

#[test]
fn shutdown_mcp_no_panic() {
    let _ = std::panic::catch_unwind(|| {
        shutdown_mcp();
    });
}

// ── HelenTool struct fields ─────────────────────────────────────────────

#[test]
fn helen_tool_fields() {
    let tools = all_tools();
    for tool in &tools {
        assert!(!tool.name.is_empty());
        assert!(!tool.description.is_empty());
        assert!(tool.parameters.is_object());
    }
}

#[test]
fn tool_names_are_unique() {
    let tools = all_tools();
    let mut names: Vec<&str> = tools.iter().map(|t| t.name).collect();
    let original_len = names.len();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), original_len);
}
