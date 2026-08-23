# Helen Agent WebUI Port — Implementation Plan

**Goal:** Port Python `helen agent` WebUI to pure Rust, eliminating Python dependency for production use.

**Architecture:** Axum web server (Rust) + embedded React frontend + embedded agent .helen files. Direct Helen execution (no Python bridge needed). Optional `--test-bridge` mode for FFI validation.

**Tech Stack:**
- Axum (async web framework)
- Tokio (async runtime)
- rust-embed (static file embedding)
- tokio-tungstenite (WebSocket)
- serde_json (JSON handling)

---

## Phase 1: Foundation (MVP Web Server)

### Task 1.1: Create helen-agent crate structure

**Objective:** Set up new crate with minimal Axum server

**Files:**
- Create: `crates/helen-agent/Cargo.toml`
- Create: `crates/helen-agent/src/lib.rs`
- Create: `crates/helen-agent/src/server.rs`
- Test: `crates/helen-agent/tests/test_server.rs`

**Step 1: Write failing test**

```rust
// crates/helen-agent/tests/test_server.rs
#[tokio::test]
async fn test_server_starts_and_responds_to_health() {
    let server = helen_agent::server::start_server("127.0.0.1:0").await.unwrap();
    let port = server.local_addr().port();
    
    let resp = reqwest::get(format!("http://127.0.0.1:{}/health", port))
        .await.unwrap();
    
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
    
    server.shutdown().await;
}
```

**Step 2: Run test to verify failure**

```bash
cargo test -p helen-agent test_server_starts_and_responds_to_health
```
Expected: FAIL — "helen_agent not found"

**Step 3: Write minimal implementation**

```rust
// crates/helen-agent/src/lib.rs
pub mod server;

// crates/helen-agent/src/server.rs
use axum::{Router, routing::get, Json};
use serde_json::json;
use std::net::SocketAddr;
use tokio::net::TcpListener;

pub struct Server {
    handle: tokio::task::JoinHandle<()>,
    local_addr: SocketAddr,
}

impl Server {
    pub async fn shutdown(self) {
        self.handle.abort();
    }
    
    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }
}

pub async fn start_server(bind: &str) -> Result<Server, Box<dyn std::error::Error>> {
    let app = Router::new()
        .route("/health", get(health_handler));
    
    let listener = TcpListener::bind(bind).await?;
    let local_addr = listener.local_addr()?;
    
    let handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    
    Ok(Server { handle, local_addr })
}

async fn health_handler() -> Json<serde_json::Value> {
    Json(json!({"status": "ok"}))
}
```

**Step 4: Run test to verify pass**

```bash
cargo test -p helen-agent test_server_starts_and_responds_to_health
```
Expected: PASS

**Step 5: Commit**

```bash
git add crates/helen-agent/
git commit -m "M17: add helen-agent crate with basic Axum server"
```

---

### Task 1.2: Embed React frontend

**Objective:** Pre-build React app and embed in Rust binary

**Files:**
- Create: `crates/helen-agent/build.rs`
- Create: `crates/helen-agent/frontend/` (copy from Python)
- Modify: `crates/helen-agent/Cargo.toml`
- Test: `crates/helen-agent/tests/test_frontend.rs`

**Step 1: Write failing test**

```rust
#[tokio::test]
async fn test_serves_index_html() {
    let server = helen_agent::server::start_server("127.0.0.1:0").await.unwrap();
    let port = server.local_addr().port();
    
    let resp = reqwest::get(format!("http://127.0.0.1:{}/", port))
        .await.unwrap();
    
    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    assert!(body.contains("<!DOCTYPE html>") || body.contains("<html"));
    
    server.shutdown().await;
}
```

**Step 2: Run test to verify failure**

```bash
cargo test -p helen-agent test_serves_index_html
```
Expected: FAIL — 404

**Step 3: Write minimal implementation**

```rust
// build.rs
use std::fs;
use std::path::Path;

fn main() {
    // Copy pre-built frontend from Python source
    let src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().parent().unwrap()
        .join("../helen/helen/agent/webui/frontend/dist");
    
    let dest = Path::new(env!("OUT_DIR")).join("frontend");
    if src.exists() {
        fs::create_dir_all(&dest).ok();
        copy_dir_all(&src, &dest).ok();
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}

// server.rs — add rust-embed
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "frontend/"]
struct FrontendAssets;

// In start_server:
let app = Router::new()
    .route("/", get(index_handler))
    .route("/health", get(health_handler));

async fn index_handler() -> impl axum::response::IntoResponse {
    match FrontendAssets::get("index.html") {
        Some(file) => axum::response::Html(file.data.into_owned()),
        None => axum::response::Html("<h1>Frontend not found</h1>".to_string()),
    }
}
```

**Step 4: Run test to verify pass**

```bash
cargo test -p helen-agent test_serves_index_html
```
Expected: PASS

**Step 5: Commit**

```bash
git add crates/helen-agent/
git commit -m "M17: embed React frontend in binary"
```

---

### Task 1.3: Add REST API endpoints

**Objective:** Port key REST endpoints from Python

**Files:**
- Create: `crates/helen-agent/src/api/mod.rs`
- Create: `crates/helen-agent/src/api/chat.rs`
- Create: `crates/helen-agent/src/api/agents.rs`
- Test: `crates/helen-agent/tests/test_api.rs`

**Step 1: Write failing tests**

```rust
#[tokio::test]
async fn test_get_chat_status() {
    let server = helen_agent::server::start_server("127.0.0.1:0").await.unwrap();
    let port = server.local_addr().port();
    
    let resp = reqwest::get(format!("http://127.0.0.1:{}/api/chat/status", port))
        .await.unwrap();
    
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("is_processing").is_some());
    
    server.shutdown().await;
}

#[tokio::test]
async fn test_get_agents_list() {
    let server = helen_agent::server::start_server("127.0.0.1:0").await.unwrap();
    let port = server.local_addr().port();
    
    let resp = reqwest::get(format!("http://127.0.0.1:{}/api/agents/list", port))
        .await.unwrap();
    
    assert_eq!(resp.status(), 200);
    let body: Vec<String> = resp.json().await.unwrap();
    assert!(!body.is_empty());
    
    server.shutdown().await;
}
```

**Step 2: Run tests to verify failure**

```bash
cargo test -p helen-agent test_api
```
Expected: FAIL — 404

**Step 3: Write minimal implementation**

```rust
// api/chat.rs
use axum::{Router, routing::get, Json};
use serde_json::json;

pub fn router() -> Router {
    Router::new()
        .route("/status", get(get_status))
}

async fn get_status() -> Json<serde_json::Value> {
    Json(json!({
        "is_processing": false,
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

// api/agents.rs
use axum::{Router, routing::get, Json};

pub fn router() -> Router {
    Router::new()
        .route("/list", get(list_agents))
}

async fn list_agents() -> Json<Vec<String>> {
    Json(vec![
        "Contractor".to_string(),
        "TestBuilder".to_string(),
        "Implementer".to_string(),
    ])
}

// server.rs — integrate routers
use crate::api::{chat, agents};

let app = Router::new()
    .route("/", get(index_handler))
    .route("/health", get(health_handler))
    .nest("/api/chat", chat::router())
    .nest("/api/agents", agents::router());
```

**Step 4: Run tests to verify pass**

```bash
cargo test -p helen-agent test_api
```
Expected: PASS

**Step 5: Commit**

```bash
git add crates/helen-agent/
git commit -m "M17: add REST API endpoints for chat and agents"
```

---

## Phase 2: WebSocket Support

### Task 2.1: Add WebSocket endpoint

**Objective:** Implement WebSocket for real-time chat

**Files:**
- Create: `crates/helen-agent/src/websocket/mod.rs`
- Modify: `crates/helen-agent/src/server.rs`
- Test: `crates/helen-agent/tests/test_websocket.rs`

**Step 1: Write failing test**

```rust
#[tokio::test]
async fn test_websocket_connects() {
    let server = helen_agent::server::start_server("127.0.0.1:0").await.unwrap();
    let port = server.local_addr().port();
    
    let url = format!("ws://127.0.0.1:{}/api/chat/ws", port);
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    
    // Send a test message
    use tokio_tungstenite::tungstenite::Message;
    ws.send(Message::Text(r#"{"type":"ping"}"#.to_string())).await.unwrap();
    
    // Should receive a response
    let msg = ws.next().await.unwrap().unwrap();
    assert!(msg.is_text());
    
    server.shutdown().await;
}
```

**Step 2: Run test to verify failure**

```bash
cargo test -p helen-agent test_websocket_connects
```
Expected: FAIL — connection refused

**Step 3: Write minimal implementation**

```rust
// websocket/mod.rs
use axum::{
    extract::ws::{WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};

pub fn router() -> Router {
    Router::new().route("/ws", get(ws_handler))
}

async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    use axum::extract::ws::Message;
    
    while let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {
            match msg {
                Message::Text(text) => {
                    // Echo back for now
                    socket.send(Message::Text(format!("Echo: {}", text))).await.ok();
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    }
}

// server.rs — add WebSocket router
use crate::websocket;

let app = Router::new()
    // ... existing routes
    .nest("/api/chat", chat::router())
    .nest("/api/chat", websocket::router());
```

**Step 4: Run test to verify pass**

```bash
cargo test -p helen-agent test_websocket_connects
```
Expected: PASS

**Step 5: Commit**

```bash
git add crates/helen-agent/
git commit -m "M17: add WebSocket endpoint for real-time chat"
```

---

## Phase 3: Helen Execution

### Task 3.1: Embed agent .helen files

**Objective:** Embed agent programs in binary (like skills)

**Files:**
- Modify: `crates/helen-agent/build.rs`
- Create: `crates/helen-agent/src/agent_files.rs`
- Test: `crates/helen-agent/tests/test_agent_files.rs`

**Step 1: Write failing test**

```rust
#[test]
fn test_embedded_agent_files_accessible() {
    let files = helen_agent::agent_files::list_embedded_agents();
    assert!(files.contains(&"chat_actor.helen".to_string()));
    
    let content = helen_agent::agent_files::get_embedded_agent("chat_actor.helen");
    assert!(content.is_some());
    assert!(content.unwrap().contains("agent"));
}
```

**Step 2: Run test to verify failure**

```bash
cargo test -p helen-agent test_embedded_agent_files
```
Expected: FAIL — module not found

**Step 3: Write minimal implementation**

```rust
// build.rs — add agent files
fn main() {
    // ... existing frontend copy
    let agent_src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap().parent().unwrap()
        .join("../helen/helen/agent");
    
    let agent_dest = Path::new(env!("OUT_DIR")).join("agent");
    if agent_src.exists() {
        fs::create_dir_all(&agent_dest).ok();
        // Copy only .helen files
        for entry in fs::read_dir(&agent_src)? {
            let entry = entry?;
            if entry.path().extension().map_or(false, |e| e == "helen") {
                fs::copy(entry.path(), agent_dest.join(entry.file_name()))?;
            }
        }
    }
}

// agent_files.rs
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "agent/"]
struct AgentAssets;

pub fn list_embedded_agents() -> Vec<String> {
    AgentAssets::iter()
        .filter(|f| f.ends_with(".helen"))
        .map(|f| f.to_string())
        .collect()
}

pub fn get_embedded_agent(name: &str) -> Option<String> {
    AgentAssets::get(name)
        .map(|f| String::from_utf8_lossy(&f.data).to_string())
}
```

**Step 4: Run test to verify pass**

```bash
cargo test -p helen-agent test_embedded_agent_files
```
Expected: PASS

**Step 5: Commit**

```bash
git add crates/helen-agent/
git commit -m "M17: embed agent .helen files in binary"
```

---

### Task 3.2: Execute Helen programs via WebSocket

**Objective:** Run .helen programs and stream output via WebSocket

**Files:**
- Create: `crates/helen-agent/src/executor.rs`
- Modify: `crates/helen-agent/src/websocket/mod.rs`
- Test: `crates/helen-agent/tests/test_executor.rs`

**Step 1: Write failing test**

```rust
#[tokio::test]
async fn test_execute_simple_helen_program() {
    let code = r#"main { print("Hello from Helen!") }"#;
    let output = helen_agent::executor::execute_helen(code).await.unwrap();
    assert!(output.contains("Hello from Helen!"));
}
```

**Step 2: Run test to verify failure**

```bash
cargo test -p helen-agent test_execute_simple_helen_program
```
Expected: FAIL — module not found

**Step 3: Write minimal implementation**

```rust
// executor.rs
use helen_runtime::Runtime;
use helen_interpreter::Interpreter;

pub async fn execute_helen(code: &str) -> Result<String, Box<dyn std::error::Error>> {
    let runtime = Runtime::new();
    let mut interpreter = Interpreter::new(&runtime);
    
    // Parse and execute
    let ast = helen_parser::parse(code)?;
    let result = interpreter.execute(&ast)?;
    
    Ok(result.to_string())
}
```

**Step 4: Run test to verify pass**

```bash
cargo test -p helen-agent test_execute_simple_helen_program
```
Expected: PASS

**Step 5: Commit**

```bash
git add crates/helen-agent/
git commit -m "M17: execute Helen programs and capture output"
```

---

## Phase 4: CLI Integration

### Task 4.1: Add `helen agent` command

**Objective:** Integrate web server into CLI

**Files:**
- Modify: `crates/helen-rust/src/main.rs`
- Test: `crates/helen-rust/tests/test_agent_command.rs`

**Step 1: Write failing test**

```rust
#[test]
fn test_agent_command_exists() {
    let output = std::process::Command::new("cargo")
        .args(["run", "--bin", "helen", "--", "agent", "--help"])
        .output()
        .unwrap();
    
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("agent") || stdout.contains("webui"));
}
```

**Step 2: Run test to verify failure**

```bash
cargo test -p helen-rust test_agent_command_exists
```
Expected: FAIL — "agent" not recognized

**Step 3: Write minimal implementation**

```rust
// main.rs — add agent subcommand
#[derive(Subcommand)]
enum Commands {
    // ... existing commands
    /// Start the agent web UI
    Agent {
        /// Port to bind
        #[arg(short, long, default_value = "8000")]
        port: u16,
        
        /// Open browser automatically
        #[arg(long)]
        open: bool,
    },
}

// In main():
Commands::Agent { port, open } => {
    let bind = format!("127.0.0.1:{}", port);
    println!("🚀 Starting Helen Agent WebUI on http://{}", bind);
    
    if open {
        open_browser(&format!("http://{}", bind));
    }
    
    helen_agent::server::start_server(&bind).await?;
}
```

**Step 4: Run test to verify pass**

```bash
cargo test -p helen-rust test_agent_command_exists
```
Expected: PASS

**Step 5: Commit**

```bash
git add crates/helen-rust/
git commit -m "M17: add 'helen agent' CLI command"
```

---

## Phase 5: Testing & Polish

### Task 5.1: Integration tests

**Objective:** End-to-end tests for full workflow

**Files:**
- Create: `tests/integration/test_agent_webui.rs`

**Step 1: Write integration test**

```rust
#[tokio::test]
async fn test_full_agent_workflow() {
    // Start server
    let server = helen_agent::server::start_server("127.0.0.1:0").await.unwrap();
    let port = server.local_addr().port();
    
    // 1. Check health
    let resp = reqwest::get(format!("http://127.0.0.1:{}/health", port)).await.unwrap();
    assert_eq!(resp.status(), 200);
    
    // 2. Load frontend
    let resp = reqwest::get(format!("http://127.0.0.1:{}/", port)).await.unwrap();
    assert_eq!(resp.status(), 200);
    
    // 3. Connect WebSocket
    let url = format!("ws://127.0.0.1:{}/api/chat/ws", port);
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();
    
    // 4. Send chat message
    use tokio_tungstenite::tungstenite::Message;
    ws.send(Message::Text(r#"{"type":"chat","message":"Hello"}"#.to_string())).await.unwrap();
    
    // 5. Receive response
    let msg = ws.next().await.unwrap().unwrap();
    assert!(msg.is_text());
    
    server.shutdown().await;
}
```

**Step 2: Run integration test**

```bash
cargo test --test test_agent_webui
```
Expected: PASS

**Step 3: Commit**

```bash
git add tests/
git commit -m "M17: add integration tests for agent webui"
```

---

## Summary

| Phase | Tasks | Effort | Status |
|-------|-------|--------|--------|
| 1. Foundation | 3 tasks | 1 day | ⏳ TODO |
| 2. WebSocket | 1 task | 0.5 day | ⏳ TODO |
| 3. Helen Execution | 2 tasks | 1 day | ⏳ TODO |
| 4. CLI Integration | 1 task | 0.5 day | ⏳ TODO |
| 5. Testing | 1 task | 0.5 day | ⏳ TODO |

**Total: ~3.5 days**

## Next Steps

Ready to execute using TDD methodology. Shall I proceed with Task 1.1?
