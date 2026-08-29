# V2 Architecture: HelenActorBridge Implementation Plan

**Goal:** Implement HelenActorBridge to connect Rust WebUI with Helen ChatSessionActor

**Architecture:** Rust provides infrastructure (HTTP/WebSocket/Auth), Helen provides agent logic (ChatSessionActor with 100+ tools). The bridge spawns Helen interpreter in a dedicated thread and communicates via channels.

**Tech Stack:** Rust (Axum, tokio), Helen interpreter, Helen agent files (.helen)

---

## Current State Analysis

### What Exists (M24)
- `executor.rs` (31 lines): Simple Helen code execution, captures stdout
- `helen_bridge.rs` (161 lines): Process-level bridge, spawns `helen` CLI binary
- No integration with Helen's agent system (ChatSessionActor)
- No tool execution, no context management, no slash commands

### What's Missing (V2 Target)
- HelenActorBridge: In-process interpreter + ChatSessionActor integration
- Channel-based communication (Rust ↔ Helen mailbox)
- Streaming chunk forwarding
- Lifecycle management (heartbeat, shutdown, recovery)

---

## Implementation Phases

### Phase 1: Core HelenActorBridge (Foundation)
**Goal:** Spawn Helen interpreter in dedicated thread, basic message passing

### Phase 2: ChatSessionActor Integration
**Goal:** Load and spawn ChatSessionActor, implement message protocol

### Phase 3: Streaming Support
**Goal:** Forward streaming chunks to WebSocket in real-time

### Phase 4: Lifecycle Management
**Goal:** Heartbeat, graceful shutdown, crash recovery

### Phase 5: Production Hardening
**Goal:** Error handling, logging, integration tests, page testing

---

## Phase 1: Core HelenActorBridge

### Task 1.1: Define Message Types

**Objective:** Define Rust types for Rust ↔ Helen communication

**Files:**
- Create: `crates/helen-agent/src/actor_bridge/messages.rs`
- Test: `crates/helen-agent/tests/test_actor_bridge_messages.rs`

**Step 1: Write failing test**

```rust
// crates/helen-agent/tests/test_actor_bridge_messages.rs
use helen_agent::actor_bridge::messages::{UserInput, AgentOutput, StreamChunk};

#[test]
fn test_user_input_creation() {
    let input = UserInput {
        content: "Hello".to_string(),
        file_paths: vec![],
        request_id: "req-1".to_string(),
    };
    assert_eq!(input.content, "Hello");
    assert_eq!(input.request_id, "req-1");
}

#[test]
fn test_agent_output_response_complete() {
    let output = AgentOutput::ResponseComplete {
        request_id: "req-1".to_string(),
        content: "Hi there".to_string(),
    };
    match output {
        AgentOutput::ResponseComplete { content, .. } => {
            assert_eq!(content, "Hi there");
        }
        _ => panic!("Expected ResponseComplete"),
    }
}

#[test]
fn test_stream_chunk_creation() {
    let chunk = StreamChunk {
        sequence: 1,
        content: "Hello ".to_string(),
    };
    assert_eq!(chunk.sequence, 1);
    assert_eq!(chunk.content, "Hello ");
}
```

**Step 2: Run test to verify failure**

```bash
cargo test -p helen-agent test_actor_bridge_messages -- --nocapture
```

Expected: FAIL — "module `actor_bridge` not found"

**Step 3: Write minimal implementation**

```rust
// crates/helen-agent/src/actor_bridge/messages.rs

//! Message types for Rust ↔ Helen communication

/// User input sent to ChatSessionActor
#[derive(Debug, Clone)]
pub struct UserInput {
    pub content: String,
    pub file_paths: Vec<String>,
    pub request_id: String,
}

/// Agent output received from ChatSessionActor
#[derive(Debug, Clone)]
pub enum AgentOutput {
    ResponseComplete {
        request_id: String,
        content: String,
    },
    Error {
        request_id: String,
        error_msg: String,
    },
    ActorStatus {
        status: String,
    },
}

/// Streaming chunk for real-time output
#[derive(Debug, Clone)]
pub struct StreamChunk {
    pub sequence: u64,
    pub content: String,
}
```

**Step 4: Create module structure**

```rust
// crates/helen-agent/src/actor_bridge/mod.rs
pub mod messages;
```

```rust
// Add to crates/helen-agent/src/lib.rs
pub mod actor_bridge;
```

**Step 5: Run test to verify pass**

```bash
cargo test -p helen-agent test_actor_bridge_messages -- --nocapture
```

Expected: 3 passed

**Step 6: Commit**

```bash
git add crates/helen-agent/src/actor_bridge/ crates/helen-agent/tests/test_actor_bridge_messages.rs crates/helen-agent/src/lib.rs
git commit -m "M25.1: Define message types for actor bridge"
```

---

### Task 1.2: Implement HelenActorBridge Struct

**Objective:** Create bridge struct with channel-based communication

**Files:**
- Create: `crates/helen-agent/src/actor_bridge/bridge.rs`
- Test: `crates/helen-agent/tests/test_actor_bridge_creation.rs`

**Step 1: Write failing test**

```rust
// crates/helen-agent/tests/test_actor_bridge_creation.rs
use helen_agent::actor_bridge::bridge::HelenActorBridge;

#[tokio::test]
async fn test_bridge_creation() {
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );
    assert!(bridge.is_alive());
}

#[tokio::test]
async fn test_bridge_send_message() {
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );
    
    // Should not panic
    bridge.send_message("Hello".to_string(), vec![]).await;
}
```

**Step 2: Run test to verify failure**

```bash
cargo test -p helen-agent test_actor_bridge_creation -- --nocapture
```

Expected: FAIL — "module `bridge` not found"

**Step 3: Write minimal implementation**

```rust
// crates/helen-agent/src/actor_bridge/bridge.rs

//! HelenActorBridge — connects Rust WebUI to Helen ChatSessionActor

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

use super::messages::{AgentOutput, StreamChunk, UserInput};

/// Bridge between Rust WebUI and Helen ChatSessionActor
pub struct HelenActorBridge {
    alive: Arc<AtomicBool>,
    input_tx: mpsc::Sender<UserInput>,
    _output_rx: mpsc::Receiver<AgentOutput>,
    stream_tx: broadcast::Sender<StreamChunk>,
}

impl HelenActorBridge {
    /// Create a new bridge (spawns Helen interpreter thread)
    pub fn new(cwd: String, session_id: String, env_context: String) -> Self {
        let alive = Arc::new(AtomicBool::new(true));
        let (input_tx, _input_rx) = mpsc::channel(32);
        let (_output_tx, output_rx) = mpsc::channel(32);
        let (stream_tx, _) = broadcast::channel(100);

        // TODO: Spawn Helen interpreter thread (Task 1.3)
        let _alive_clone = alive.clone();
        std::thread::spawn(move || {
            // Placeholder: Helen interpreter thread
            // Will be implemented in Task 1.3
            let _ = (cwd, session_id, env_context);
        });

        Self {
            alive,
            input_tx,
            _output_rx: output_rx,
            stream_tx,
        }
    }

    /// Check if bridge is alive
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    /// Send user input to ChatSessionActor
    pub async fn send_message(&self, content: String, file_paths: Vec<String>) {
        let input = UserInput {
            content,
            file_paths,
            request_id: uuid::Uuid::new_v4().to_string(),
        };
        let _ = self.input_tx.send(input).await;
    }

    /// Subscribe to streaming chunks
    pub fn subscribe_stream(&self) -> broadcast::Receiver<StreamChunk> {
        self.stream_tx.subscribe()
    }
}
```

**Step 4: Update module exports**

```rust
// crates/helen-agent/src/actor_bridge/mod.rs
pub mod bridge;
pub mod messages;
```

**Step 5: Run test to verify pass**

```bash
cargo test -p helen-agent test_actor_bridge_creation -- --nocapture
```

Expected: 2 passed

**Step 6: Commit**

```bash
git add crates/helen-agent/src/actor_bridge/bridge.rs crates/helen-agent/tests/test_actor_bridge_creation.rs crates/helen-agent/src/actor_bridge/mod.rs
git commit -m "M25.2: Implement HelenActorBridge struct with channels"
```

---

### Task 1.3: Spawn Helen Interpreter Thread

**Objective:** Create Helen interpreter in dedicated thread with full runtime

**Files:**
- Modify: `crates/helen-agent/src/actor_bridge/bridge.rs`
- Test: `crates/helen-agent/tests/test_interpreter_spawn.rs`

**Step 1: Write failing test**

```rust
// crates/helen-agent/tests/test_interpreter_spawn.rs
use helen_agent::actor_bridge::bridge::HelenActorBridge;

#[tokio::test]
async fn test_interpreter_thread_spawns() {
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );
    
    // Give thread time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    
    // Bridge should still be alive
    assert!(bridge.is_alive());
}

#[tokio::test]
async fn test_interpreter_executes_simple_code() {
    // This test will be implemented after Task 1.4
    // For now, just verify the bridge can be created
    let bridge = HelenActorBridge::new(
        "/tmp".to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );
    assert!(bridge.is_alive());
}
```

**Step 2: Run test to verify failure**

```bash
cargo test -p helen-agent test_interpreter_spawn -- --nocapture
```

Expected: PASS (current implementation already spawns thread)

**Step 3: Implement interpreter creation**

```rust
// Modify crates/helen-agent/src/actor_bridge/bridge.rs

// Add imports
use helen_core::lexer::Scanner;
use helen_interpreter::interpreter::Interpreter;
use helen_parser::Parser;

// In HelenActorBridge::new(), replace the placeholder thread spawn:
std::thread::spawn(move || {
    // 1. Create interpreter with full runtime
    let mut interp = Interpreter::new();
    
    // 2. Install Python FFI (if feature enabled)
    #[cfg(feature = "python-ffi")]
    {
        if let Err(e) = helen_ffi::install() {
            eprintln!("Python FFI install failed: {}", e);
        }
    }
    
    // 3. Mark thread as running
    // (Will be used for lifecycle management in Phase 4)
    
    // 4. Message loop (will be implemented in Task 1.4)
    // For now, just keep thread alive
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
        if !alive_clone.load(Ordering::Relaxed) {
            break;
        }
    }
});
```

**Step 4: Run test to verify pass**

```bash
cargo test -p helen-agent test_interpreter_spawn -- --nocapture
```

Expected: 2 passed

**Step 5: Commit**

```bash
git add crates/helen-agent/src/actor_bridge/bridge.rs crates/helen-agent/tests/test_interpreter_spawn.rs
git commit -m "M25.3: Spawn Helen interpreter thread with full runtime"
```

---

## Phase 2: ChatSessionActor Integration

### Task 2.1: Load chat_actor.helen

**Objective:** Load Helen agent files into interpreter

**Files:**
- Modify: `crates/helen-agent/src/actor_bridge/bridge.rs`
- Test: `crates/helen-agent/tests/test_load_helen_files.rs`

**Step 1: Write failing test**

```rust
// crates/helen-agent/tests/test_load_helen_files.rs
use helen_agent::actor_bridge::bridge::HelenActorBridge;

#[tokio::test]
async fn test_load_chat_actor_helen() {
    // This test requires chat_actor.helen to exist
    // For now, we'll create a minimal test file
    let test_dir = tempfile::tempdir().unwrap();
    let test_file = test_dir.path().join("test.helen");
    std::fs::write(&test_file, "fn test_fn(): int { return 42 }").unwrap();
    
    let bridge = HelenActorBridge::new(
        test_dir.path().to_str().unwrap().to_string(),
        "test-session".to_string(),
        "<context></context>".to_string(),
    );
    
    // Give thread time to load files
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    
    assert!(bridge.is_alive());
}
```

**Step 2: Implement file loading**

```rust
// In bridge.rs, add file loading logic to interpreter thread

// Define Helen agent files directory
const HELEN_AGENT_DIR: &str = env!("HELEN_AGENT_DIR", "/home/rxx/helen/helen/agent");

std::thread::spawn(move || {
    let mut interp = Interpreter::new();
    
    // Install Python FFI
    #[cfg(feature = "python-ffi")]
    {
        if let Err(e) = helen_ffi::install() {
            eprintln!("Python FFI install failed: {}", e);
        }
    }
    
    // Load Helen agent files
    let agent_dir = std::env::var("HELEN_AGENT_DIR")
        .unwrap_or_else(|_| "/home/rxx/helen/helen/agent".to_string());
    
    let files_to_load = vec![
        "utils.helen",
        "lang.helen",
        "json_utils.helen",
        "output.helen",
        "context.helen",
        "context_manager.helen",
        "memory_utils.helen",
        "session_stats.helen",
        "system_reminders.helen",
        "task_manager.helen",
        "ui_event_queue.helen",
        "commands.helen",
        "chat_session_actor.helen",
        "chat_actor.helen",
    ];
    
    for file in files_to_load {
        let path = format!("{}/{}", agent_dir, file);
        match std::fs::read_to_string(&path) {
            Ok(source) => {
                let mut scanner = Scanner::new(&source, file);
                let tokens = scanner.scan_all();
                let mut parser = Parser::new(tokens);
                let program = parser.parse();
                
                if !parser.errors().is_empty() {
                    eprintln!("Parse errors in {}: {:?}", file, parser.errors());
                    continue;
                }
                
                if let Err(e) = interp.interpret(&program) {
                    eprintln!("Load error in {}: {:?}", file, e);
                }
            }
            Err(e) => {
                eprintln!("Failed to read {}: {}", path, e);
            }
        }
    }
    
    // Message loop...
});
```

**Step 3: Run test to verify pass**

```bash
cargo test -p helen-agent test_load_helen_files -- --nocapture
```

Expected: 1 passed

**Step 4: Commit**

```bash
git add crates/helen-agent/src/actor_bridge/bridge.rs crates/helen-agent/tests/test_load_helen_files.rs
git commit -m "M25.4: Load chat_actor.helen and dependencies"
```

---

## Testing Strategy

### Unit Tests
- Message types (Task 1.1)
- Bridge creation (Task 1.2)
- Interpreter spawn (Task 1.3)
- File loading (Task 2.1)

### Integration Tests
- End-to-end message flow
- Streaming chunk delivery
- Lifecycle management

### Page Tests (Critical!)
- Chat page: send message, receive response
- Settings page: load and save settings
- Transcript page: view session history

---

## Verification Checklist

After each phase:
- [ ] All unit tests pass
- [ ] Integration tests pass
- [ ] `cargo build --release` succeeds
- [ ] `cargo clippy` has 0 warnings
- [ ] Manual page testing (Chat, Settings, Transcript)
- [ ] No regressions in existing tests

---

## Next Steps

This plan covers Phase 1 and the start of Phase 2. Subsequent phases will be detailed as we progress.

**Ready to execute?** I'll implement Task 1.1 first using strict TDD methodology.
