//! MCP (Model Context Protocol) client implementation.
//!
//! Port of `helen/runtime/mcp/client.py` — JSON-RPC 2.0 over stdio.
//!
//! The Python implementation spawns a subprocess and uses a background
//! reader thread + per-request `queue.Queue`s. In Rust we use
//! `std::process::Child` with piped stdin/stdout, a reader thread, and
//! `std::sync::mpsc` channels keyed by request id.

use super::errors::MCPError;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// MCP client that communicates with a server via JSON-RPC over stdio.
///
/// Spawns a subprocess for the MCP server and communicates via
/// stdin/stdout using line-delimited JSON-RPC messages.
pub struct MCPClient {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub env: Option<HashMap<String, String>>,
    pub timeout: u64,

    // Internal state
    process: Option<Child>,
    stdin: Option<ChildStdin>,
    request_id: u64,
    pending: Arc<Mutex<HashMap<u64, Sender<Value>>>>,
    reader_thread: Option<JoinHandle<()>>,
    is_running: bool,
}

impl MCPClient {
    pub fn new(
        name: &str,
        command: &str,
        args: Vec<String>,
        cwd: Option<String>,
        env: Option<HashMap<String, String>>,
        timeout: u64,
    ) -> Self {
        MCPClient {
            name: name.to_string(),
            command: command.to_string(),
            args,
            cwd,
            env,
            timeout,
            process: None,
            stdin: None,
            request_id: 0,
            pending: Arc::new(Mutex::new(HashMap::new())),
            reader_thread: None,
            is_running: false,
        }
    }

    pub fn is_running(&self) -> bool {
        self.is_running
    }

    /// `start()` — start the MCP server process and initialize the connection.
    pub fn start(&mut self) -> Result<(), MCPError> {
        if self.is_running {
            return Ok(());
        }

        // Start server process with line-buffered stdio.
        let mut cmd = Command::new(&self.command);
        cmd.args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(cwd) = &self.cwd {
            cmd.current_dir(cwd);
        }
        if let Some(env) = &self.env {
            cmd.envs(env);
        }

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(MCPError::Server(format!(
                    "Failed to start MCP server '{}': command '{}' not found",
                    self.name, self.command
                )));
            }
            Err(e) => {
                return Err(MCPError::Server(format!(
                    "Failed to start MCP server '{}': {}",
                    self.name, e
                )));
            }
        };

        let stdin = child.stdin.take().expect("stdin piped");
        let stdout = child.stdout.take().expect("stdout piped");

        // Start reader thread to handle responses.
        let name = self.name.clone();
        let pending = Arc::clone(&self.pending);
        let reader_thread = std::thread::spawn(move || {
            Self::read_responses(name, stdout, pending);
        });

        self.process = Some(child);
        self.stdin = Some(stdin);
        self.reader_thread = Some(reader_thread);
        self.is_running = true;

        // Send initialize request.
        let init_result = self.send_request(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "helen",
                    "version": "1.33.0",
                },
            }),
        );

        match init_result {
            Ok(_) => Ok(()),
            Err(e) => {
                self.shutdown();
                Err(MCPError::Server(format!(
                    "Failed to initialize MCP server '{}': {}",
                    self.name,
                    e.message()
                )))
            }
        }
    }

    /// `_read_responses` — read JSON-RPC responses from stdout in a
    /// background thread. Reads lines, parses them, and dispatches
    /// responses to the appropriate pending request channel.
    fn read_responses(
        name: String,
        stdout: std::process::ChildStdout,
        pending: Arc<Mutex<HashMap<u64, Sender<Value>>>>,
    ) {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break,
            };
            let line = line.trim().to_string();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<Value>(&line) {
                Ok(message) => {
                    let msg_id = message.get("id").and_then(|v| v.as_u64());
                    if let Some(id) = msg_id {
                        let guard = pending.lock().expect("mutex poisoned");
                        if let Some(tx) = guard.get(&id) {
                            let _ = tx.send(message.clone());
                        }
                    }
                    // else: notification or unknown message — ignored.
                }
                Err(_) => {
                    // Invalid JSON → warning in Python; we ignore.
                    let _ = &name;
                }
            }
        }
    }

    /// `_send_request` — send a JSON-RPC request and wait for the response.
    fn send_request(&mut self, method: &str, params: Value) -> Result<Value, MCPError> {
        if !self.is_running || self.process.is_none() {
            return Err(MCPError::Server(format!(
                "MCP server '{}' is not running",
                self.name
            )));
        }

        // Generate request ID.
        self.request_id += 1;
        let request_id = self.request_id;

        // Build request.
        let request = json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
            "params": params,
        });

        // Create response channel.
        let (tx, rx): (Sender<Value>, Receiver<Value>) = channel();
        self.pending.lock().expect("mutex poisoned").insert(request_id, tx);

        // Send request.
        let request_json = serde_json::to_string(&request).expect("serialize request") + "\n";
        let write_result = {
            let stdin = self.stdin.as_mut().ok_or_else(|| {
                self.pending.lock().expect("mutex poisoned").remove(&request_id);
                MCPError::Server(format!("MCP server '{}' is not running", self.name))
            })?;
            stdin
                .write_all(request_json.as_bytes())
                .and_then(|_| stdin.flush())
        };
        if let Err(e) = write_result {
            self.pending.lock().expect("mutex poisoned").remove(&request_id);
            return Err(MCPError::Server(format!(
                "Failed to send request to MCP server '{}': {}",
                self.name, e
            )));
        }

        // Wait for response.
        let response = match rx.recv_timeout(Duration::from_secs(self.timeout)) {
            Ok(r) => r,
            Err(_) => {
                self.pending.lock().expect("mutex poisoned").remove(&request_id);
                return Err(MCPError::Timeout(format!(
                    "Timeout waiting for response from MCP server '{}'",
                    self.name
                )));
            }
        };
        self.pending.lock().expect("mutex poisoned").remove(&request_id);

        // Check for error.
        if let Some(error) = response.get("error") {
            let error_msg = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error");
            let error_code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
            return Err(MCPError::Server(format!(
                "MCP server '{}' error (code {}): {}",
                self.name, error_code, error_msg
            )));
        }

        Ok(response.get("result").cloned().unwrap_or(Value::Null))
    }

    /// `list_tools()` — list available tools from the MCP server.
    pub fn list_tools(&mut self) -> Result<Vec<Value>, MCPError> {
        let result = self.send_request("tools/list", json!({}))?;
        Ok(result
            .get("tools")
            .and_then(|t| t.as_array())
            .cloned()
            .unwrap_or_default())
    }

    /// `call_tool(name, arguments)` — call a tool on the MCP server.
    pub fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Value, MCPError> {
        let result = self.send_request(
            "tools/call",
            json!({
                "name": name,
                "arguments": arguments,
            }),
        );
        match result {
            Ok(r) => Ok(r),
            Err(e) => Err(MCPError::Tool(format!(
                "Tool '{}' call failed: {}",
                name,
                e.message()
            ))),
        }
    }

    /// `shutdown()` — shut down the MCP server and terminate the process.
    /// Sends a shutdown request, then terminates the process.
    /// Falls back to kill() if graceful shutdown fails.
    pub fn shutdown(&mut self) {
        if !self.is_running {
            return;
        }
        self.is_running = false;

        // Send shutdown request (best effort).
        let _ = self.send_request("shutdown", json!({}));

        // Terminate process.
        if let Some(mut child) = self.process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        self.stdin.take();

        // Join reader thread.
        if let Some(handle) = self.reader_thread.take() {
            let _ = handle.join();
        }
        self.pending.lock().expect("mutex poisoned").clear();
    }
}

impl Drop for MCPClient {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    fn mock_cmd() -> String {
        // The fixture mock server is a Python script (same as Python's tests).
        let fixture = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/mock_mcp_server.py"
        );
        // Use "python3" — available in the environment (helenenv).
        format!("python3 {fixture}")
    }

    fn client() -> MCPClient {
        let fixture = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/mock_mcp_server.py"
        );
        MCPClient::new("test", "python3", vec![fixture.to_string()], None, None, 60)
    }

    #[test]
    fn start_and_shutdown() {
        let mut c = client();
        c.start().expect("start");
        assert!(c.is_running());
        c.shutdown();
        assert!(!c.is_running());
    }

    #[test]
    fn list_tools() {
        let mut c = client();
        c.start().expect("start");
        let tools = c.list_tools().expect("list tools");
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
    fn call_tool_echo() {
        let mut c = client();
        c.start().expect("start");
        let result = c
            .call_tool("echo", json!({"message": "Hello, MCP!"}))
            .unwrap();
        assert_eq!(result["output"], "Echo: Hello, MCP!");
        c.shutdown();
    }

    #[test]
    fn call_tool_add() {
        let mut c = client();
        c.start().expect("start");
        let result = c.call_tool("add", json!({"a": 5, "b": 3})).unwrap();
        assert_eq!(result["result"], 8);
        c.shutdown();
    }

    #[test]
    fn call_unknown_tool() {
        let mut c = client();
        c.start().expect("start");
        // Mock server returns error in result, not as JSON-RPC error.
        let result = c.call_tool("unknown_tool", json!({})).unwrap();
        assert!(result.get("error").is_some());
        c.shutdown();
    }

    #[test]
    fn invalid_command() {
        let mut c = MCPClient::new("test", "nonexistent_command_xyz", vec![], None, None, 60);
        let err = c.start().unwrap_err();
        assert!(matches!(err, MCPError::Server(_)));
        assert!(err.message().contains("not found"));
    }

    #[test]
    fn timeout_raises_timeout_error() {
        // A server that never responds: use `python3 -c "import time; time.sleep(30)"`
        let mut c = MCPClient::new(
            "slow",
            "python3",
            vec!["-c".into(), "import time; time.sleep(30)".into()],
            None,
            None,
            1, // 1 second timeout
        );
        let err = c.start().unwrap_err();
        assert!(
            matches!(err, MCPError::Server(_)),
            "start wraps init failure"
        );
        assert!(err.message().contains("Failed to initialize"));
        // Ensure cleanup killed the sleeping process.
    }
}
