//! MCP (Model Context Protocol) configuration loader.
//!
//! Port of `helen/runtime/mcp/config.py` — loads MCP server configuration
//! from `.mcp.json` files.

use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Configuration for a single MCP server (Python `MCPServerConfig`).
#[derive(Debug, Clone, PartialEq)]
pub struct MCPServerConfig {
    /// Server name (from `.mcp.json` key).
    pub name: String,
    /// Command to start the server (e.g., "node", "python").
    pub command: String,
    /// Command arguments (e.g., ["server.js", "--stdio"]).
    pub args: Vec<String>,
    /// Working directory for the server process.
    pub cwd: Option<String>,
    /// Environment variables to pass to the server.
    pub env_vars: Option<HashMap<String, String>>,
    /// Timeout for tool calls in seconds.
    pub tool_timeout_sec: u64,
}

/// Configuration for all MCP servers in a project (Python `MCPConfig`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MCPConfig {
    /// List of MCP server configurations.
    pub servers: Vec<MCPServerConfig>,
}

impl MCPConfig {
    /// `MCPConfig.from_file(path)` — load MCP configuration from `.mcp.json`.
    ///
    /// Python parity:
    /// - Returns empty config if the file doesn't exist (no error).
    /// - Returns empty config if JSON parsing fails (warning logged).
    /// - Skips servers without a required `command` field.
    pub fn from_file(path: &Path) -> MCPConfig {
        if !path.exists() {
            return MCPConfig { servers: vec![] };
        }

        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(_) => {
                // OSError → warning, empty config
                return MCPConfig { servers: vec![] };
            }
        };

        let data: Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => {
                // json.JSONDecodeError → warning, empty config
                return MCPConfig { servers: vec![] };
            }
        };

        let mut servers = Vec::new();
        let mcp_servers = data.get("mcpServers");

        let obj = match mcp_servers {
            Some(Value::Object(o)) => o,
            _ => &serde_json::Map::new(),
        };

        for (name, config) in obj {
            // Skip servers without required 'command' field.
            if !config.is_object() {
                continue;
            }
            let cfg = config.as_object().unwrap();
            let Some(command) = cfg.get("command").and_then(|c| c.as_str()) else {
                continue;
            };
            // 'command' must be a string.
            if !cfg.get("command").unwrap().is_string() {
                continue;
            }

            // args must be a list if present.
            let args = match cfg.get("args") {
                Some(Value::Array(a)) => a
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect(),
                Some(_) => continue, // not a list → skip
                None => vec![],
            };

            // cwd must be a string if present.
            let cwd = match cfg.get("cwd") {
                Some(Value::String(s)) => Some(s.clone()),
                Some(_) => continue, // not a string → skip
                None => None,
            };

            // env_vars must be a dict if present.
            let env_vars = match cfg.get("env_vars") {
                Some(Value::Object(o)) => Some(
                    o.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect(),
                ),
                Some(_) => continue, // not a dict → skip
                None => None,
            };

            let tool_timeout_sec = cfg
                .get("tool_timeout_sec")
                .and_then(|v| v.as_u64())
                .unwrap_or(60);

            // Resolve cwd relative to the config file directory.
            let cwd = cwd.map(|c| {
                let cwd_path = PathBuf::from(&c);
                if cwd_path.is_absolute() {
                    c
                } else {
                    // Relative to config file directory. Normalize away
                    // "./" components to match Python's pathlib behavior.
                    let parent = path.parent().unwrap_or(Path::new(""));
                    let joined = parent.join(cwd_path);
                    normalize_path(&joined).to_string_lossy().to_string()
                }
            });

            servers.push(MCPServerConfig {
                name: name.clone(),
                command: command.to_string(),
                args,
                cwd,
                env_vars,
                tool_timeout_sec,
            });
        }

        MCPConfig { servers }
    }
}

/// Normalize a path (resolve `.` components) to match Python's pathlib.
fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_config(data: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("mcp_cfg_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(".mcp.json");
        std::fs::write(&path, data).unwrap();
        (dir, path)
    }

    #[test]
    fn load_config_file_not_found() {
        let config = MCPConfig::from_file(Path::new("/nonexistent/.mcp.json"));
        assert_eq!(config.servers.len(), 0);
    }

    #[test]
    fn load_config_empty_file() {
        let (_, path) = tmp_config("{}");
        let config = MCPConfig::from_file(&path);
        assert_eq!(config.servers.len(), 0);
    }

    #[test]
    fn load_config_single_server() {
        let (_, path) = tmp_config(
            r#"{
                "mcpServers": {
                    "test-server": {
                        "command": "node",
                        "args": ["server.js", "--stdio"],
                        "cwd": "/tmp",
                        "tool_timeout_sec": 30
                    }
                }
            }"#,
        );
        let config = MCPConfig::from_file(&path);
        assert_eq!(config.servers.len(), 1);
        let server = &config.servers[0];
        assert_eq!(server.name, "test-server");
        assert_eq!(server.command, "node");
        assert_eq!(server.args, vec!["server.js", "--stdio"]);
        assert_eq!(server.cwd.as_deref(), Some("/tmp"));
        assert_eq!(server.tool_timeout_sec, 30);
    }

    #[test]
    fn load_config_multiple_servers() {
        let (_, path) = tmp_config(
            r#"{
                "mcpServers": {
                    "server1": {"command": "python", "args": ["server1.py"]},
                    "server2": {"command": "node", "args": ["server2.js"]}
                }
            }"#,
        );
        let config = MCPConfig::from_file(&path);
        assert_eq!(config.servers.len(), 2);
        let names: Vec<&str> = config.servers.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"server1"));
        assert!(names.contains(&"server2"));
    }

    #[test]
    fn load_config_missing_command() {
        let (_, path) = tmp_config(
            r#"{
                "mcpServers": {
                    "valid-server": {"command": "node", "args": ["server.js"]},
                    "invalid-server": {"args": ["server.js"]}
                }
            }"#,
        );
        let config = MCPConfig::from_file(&path);
        assert_eq!(config.servers.len(), 1);
        assert_eq!(config.servers[0].name, "valid-server");
    }

    #[test]
    fn load_config_invalid_json() {
        let (_, path) = tmp_config("{invalid json");
        let config = MCPConfig::from_file(&path);
        assert_eq!(config.servers.len(), 0);
    }

    #[test]
    fn load_config_default_values() {
        let (_, path) = tmp_config(
            r#"{
                "mcpServers": {
                    "test-server": {"command": "node"}
                }
            }"#,
        );
        let config = MCPConfig::from_file(&path);
        assert_eq!(config.servers.len(), 1);
        let server = &config.servers[0];
        assert!(server.args.is_empty());
        assert!(server.cwd.is_none());
        assert!(server.env_vars.is_none());
        assert_eq!(server.tool_timeout_sec, 60); // Default
    }

    #[test]
    fn load_config_relative_cwd() {
        let (dir, path) = tmp_config(
            r#"{
                "mcpServers": {
                    "test-server": {"command": "node", "cwd": "./subdir"}
                }
            }"#,
        );
        let config = MCPConfig::from_file(&path);
        assert_eq!(config.servers.len(), 1);
        let server = &config.servers[0];
        // Resolved to absolute path relative to config file dir
        let resolved = server.cwd.as_deref().unwrap();
        assert!(std::path::Path::new(resolved).is_absolute());
        assert_eq!(resolved, dir.join("subdir").to_string_lossy());
    }
}
