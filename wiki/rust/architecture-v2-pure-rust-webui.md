# Architecture V2: Pure Rust WebUI with Helen ChatSessionActor

## Design Principle

**Core requirement**: ChatSessionActor MUST be implemented in Helen language.
The WebUI layer can be in any external language (Python or Rust), but the agent
orchestration logic must use Helen's agent system (tools, skills, channels, LLM runtime).

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Browser (React SPA)                          │
│   ChatPage │ SettingsPage │ TranscriptPage                          │
│   WebSocket ←→ Rust Axum Server (127.0.0.1:8001)                   │
└───────────────────────────────┬─────────────────────────────────────┘
                                │ HTTP / WebSocket
┌───────────────────────────────▼─────────────────────────────────────┐
│                    RUST LAYER (helen-agent crate)                    │
│                                                                     │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────────────────┐  │
│  │ Axum Server │  │ Auth Module  │  │ Static Assets (rust-embed)│  │
│  │ (HTTP/WS)   │  │ (token-based)│  │ (React SPA build)         │  │
│  └──────┬──────┘  └──────────────┘  └───────────────────────────┘  │
│         │                                                           │
│  ┌──────▼──────────────────────────────────────────────────────┐   │
│  │              SessionManager (Rust)                           │   │
│  │  - Per-user session lifecycle                               │   │
│  │  - Spawns HelenActorBridge per session                      │   │
│  │  - Heartbeat monitoring                                     │   │
│  │  - Graceful shutdown                                        │   │
│  └──────┬──────────────────────────────────────────────────────┘   │
│         │                                                         │
│  ┌──────▼──────────────────────────────────────────────────────┐   │
│  │              HelenActorBridge (Rust)                         │   │
│  │                                                              │   │
│  │  Responsibilities:                                           │   │
│  │  1. Create Helen Interpreter (with full runtime)             │   │
│  │  2. Load chat_actor.helen + dependencies                     │   │
│  │  3. Spawn ChatSessionActor in background thread              │   │
│  │  4. Manage Channel (mailbox) communication                   │   │
│  │  5. Forward streaming chunks to WebSocket                    │   │
│  │  6. Handle heartbeat / exit / crash recovery                 │   │
│  │                                                              │   │
│  │  Rust ↔ Helen boundary:                                      │   │
│  │  - Rust creates Interpreter, loads .helen files              │   │
│  │  - Rust calls spawn_chat_actor() via Helen FFI               │   │
│  │  - Rust sends user_input via mailbox.send()                  │   │
│  │  - Rust receives response via mailbox.receive()              │   │
│  │  - Streaming chunks arrive via callback/channel              │   │
│  └──────┬──────────────────────────────────────────────────────┘   │
│         │                                                         │
│  ┌──────▼──────────────────────────────────────────────────────┐   │
│  │              Helen Runtime (Rust crates)                     │   │
│  │                                                              │   │
│  │  helen-interpreter  — tree-walk interpreter                  │   │
│  │  helen-runtime      — LLM runtime, agents, tools, channels   │   │
│  │  helen-stdlib       — 378+ builtins                          │   │
│  │  helen-ffi          — Python FFI (Helen→Python calls)        │   │
│  └──────────────────────────────────────────────────────────────┘   │
└───────────────────────────────┬─────────────────────────────────────┘
                                │ Interpreter executes Helen code
┌───────────────────────────────▼─────────────────────────────────────┐
│                    HELEN LAYER (chat_actor.helen + deps)             │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  chat_actor.helen (241 lines)                               │   │
│  │  - spawn_chat_actor() — lifecycle management                │   │
│  │  - tui_chat_handler_actor() — message routing               │   │
│  │  - exit_chat_actor() — graceful shutdown                    │   │
│  │  - send_heartbeat() — keepalive                             │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  chat_session_actor.helen (981 lines) — THE CORE            │   │
│  │                                                              │   │
│  │  agent ChatSessionActor(cwd, session_id, env_context, reply)│   │
│  │  {                                                           │   │
│  │    tools = CHAT_TOOLS  // 100+ tools from stdlib             │   │
│  │    transcript "persistent"                                   │   │
│  │    context { compression "graduated" ... }                   │   │
│  │    prompt """..."""  // Full system prompt                   │   │
│  │                                                              │   │
│  │    main {                                                    │   │
│  │      // Message loop: receive user_input, llm act, respond   │   │
│  │      // Slash commands: /clear, /sessions, /search, etc.     │   │
│  │      // Working memory management                            │   │
│  │      // Context compression                                  │   │
│  │    }                                                         │   │
│  │  }                                                           │   │
│  └─────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌─────────────────────────────────────────────────────────────┐   │
│  │  Supporting Helen modules (3487 lines total)                │   │
│  │                                                              │   │
│  │  commands.helen        (1178) — slash command handlers       │   │
│  │  output.helen          (909)  — output formatting            │   │
│  │  context_manager.helen (368)  — context compression          │   │
│  │  task_manager.helen    (191)  — task tracking                │   │
│  │  ui_event_queue.helen  (189)  — streaming event queue        │   │
│  │  context.helen         (165)  — environment context builder  │   │
│  │  utils.helen           (154)  — utility functions            │   │
│  │  memory_utils.helen    (113)  — working memory helpers       │   │
│  │  session_stats.helen   (78)   — session statistics           │   │
│  │  system_reminders.helen(70)   — system reminder injection    │   │
│  │  json_utils.helen      (43)   — JSON utilities               │   │
│  │  lang.helen            (29)   — language detection           │   │
│  │  contracts/contracts.helen    — output contracts             │   │
│  └─────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
```

## Rust ↔ Helen Boundary (Detailed)

### What Rust Does (Infrastructure)

| Component | Responsibility | Crate |
|-----------|---------------|-------|
| **HTTP Server** | Axum routes, static file serving, CORS | `helen-agent` |
| **WebSocket** | Real-time bidirectional communication | `helen-agent` |
| **Authentication** | Token-based single-user auth | `helen-agent` |
| **Session Management** | Track active sessions, heartbeat, cleanup | `helen-agent` |
| **Helen Interpreter** | Lex → Parse → Semantic → Execute | `helen-interpreter` |
| **LLM Runtime** | HTTP calls to LLM APIs (OpenAI, etc.) | `helen-runtime` |
| **Tool Execution** | File I/O, shell commands, quality checks | `helen-stdlib` |
| **Channel System** | Inter-thread message passing | `helen-runtime` |
| **Python FFI** | Helen→Python calls (if needed) | `helen-ffi` |
| **Transcript** | Session transcript persistence | `helen-runtime` |

### What Helen Does (Agent Logic)

| Component | Responsibility | File |
|-----------|---------------|------|
| **ChatSessionActor** | Main orchestration loop | `chat_session_actor.helen` |
| **System Prompt** | Agent identity, instructions, context | `chat_session_actor.helen` |
| **Tool Selection** | Which tools to use, when | `chat_session_actor.helen` |
| **Slash Commands** | `/clear`, `/sessions`, `/search`, etc. | `commands.helen` |
| **Context Management** | Compression, working memory | `context_manager.helen` |
| **Output Formatting** | Minimal/verbose, i18n | `output.helen` |
| **Task Tracking** | Todo management | `task_manager.helen` |
| **Session Stats** | Token usage, turn counting | `session_stats.helen` |

### The Bridge: HelenActorBridge (Rust)

```rust
// crates/helen-agent/src/actor_bridge.rs

pub struct HelenActorBridge {
    // Helen interpreter (not Send, must stay in its thread)
    // Communication via tokio channels
    input_tx: mpsc::Sender<UserInput>,
    output_rx: mpsc::Receiver<AgentOutput>,
    stream_tx: broadcast::Sender<StreamChunk>,
}

impl HelenActorBridge {
    /// Spawn the Helen interpreter + ChatSessionActor in a background thread
    pub async fn spawn(
        cwd: String,
        session_id: String,
        env_context: String,
    ) -> Result<Self, String> {
        let (input_tx, input_rx) = mpsc::channel(32);
        let (output_tx, output_rx) = mpsc::channel(32);
        let (stream_tx, _) = broadcast::channel(100);

        // Spawn Helen interpreter thread
        let stream_tx_clone = stream_tx.clone();
        std::thread::spawn(move || {
            // 1. Create interpreter with full runtime
            let mut interp = Interpreter::new();
            
            // 2. Install Python FFI (if feature enabled)
            #[cfg(feature = "python-ffi")]
            helen_ffi::install().expect("Python FFI install");
            
            // 3. Load chat_actor.helen and dependencies
            let source = std::fs::read_to_string("chat_actor.helen")
                .expect("read chat_actor.helen");
            let tokens = Scanner::new(&source, "chat_actor.helen").scan_all();
            let program = Parser::new(tokens).parse();
            interp.interpret(&program).expect("load chat_actor");
            
            // 4. Call spawn_chat_actor() — this spawns ChatSessionActor
            //    in a background thread via Helen's spawn mechanism
            let spawn_result = interp.call_function("spawn_chat_actor", vec![]);
            
            // 5. Message loop: forward Rust channels ↔ Helen mailbox
            Self::message_loop(&mut interp, input_rx, output_tx, stream_tx_clone);
        });

        Ok(Self { input_tx, output_rx, stream_tx })
    }

    /// Send user input to ChatSessionActor
    pub async fn send_message(&self, content: String, file_paths: Vec<String>) {
        self.input_tx.send(UserInput { content, file_paths }).await.ok();
    }

    /// Receive agent response
    pub async fn receive_response(&mut self) -> Option<AgentOutput> {
        self.output_rx.recv().await
    }

    /// Subscribe to streaming chunks
    pub fn subscribe_stream(&self) -> broadcast::Receiver<StreamChunk> {
        self.stream_tx.subscribe()
    }
}
```

## Data Flow

### User sends a message:

```
1. Browser → WebSocket → Rust Axum
2. Rust SessionManager → HelenActorBridge.send_message()
3. HelenActorBridge → Helen mailbox.send({type: "user_input", content, ...})
4. ChatSessionActor receives from mailbox
5. ChatSessionActor → llm act (with tools)
6. LLM response streams back via Helen's streaming mechanism
7. Streaming chunks → HelenActorBridge → broadcast → WebSocket → Browser
8. ChatSessionActor → mailbox.send({type: "response_complete", content})
9. HelenActorBridge → output_rx → Rust → WebSocket → Browser
```

### Slash command (e.g., /clear):

```
1. Browser → WebSocket → Rust
2. Rust → HelenActorBridge.send_message("/clear")
3. ChatSessionActor receives, recognizes slash command
4. Executes /clear logic in Helen (commands.helen)
5. Responds via mailbox
6. Rust → WebSocket → Browser
```

## Key Design Decisions

### 1. Helen Files Location

**Option A**: Embed at compile-time (rust-embed)
- Pros: Single binary, no file system dependencies
- Cons: Can't update Helen code without recompiling

**Option B**: Load at runtime from disk
- Pros: Can update Helen code independently
- Cons: Requires Helen files to be present

**Recommendation**: Option B for development, Option A for production.
Use environment variable `HELEN_AGENT_DIR` to override default path.

### 2. Interpreter Thread Model

Helen's `Interpreter` is `Rc`-based (not `Send`), so it must stay in one thread.
The bridge uses `std::thread::spawn` + `mpsc` channels for communication.

```
Rust (tokio)                    Helen Thread (std::thread)
     │                                │
     │── mpsc::send(UserInput) ──────→│── mailbox.send() ──→ ChatSessionActor
     │                                │
     │←─ mpsc::recv(AgentOutput) ─────│←─ mailbox.receive() ─ ChatSessionActor
     │                                │
```

### 3. Streaming Chunks

Streaming chunks bypass the Channel (mailbox) for low latency:
- Helen's LLM runtime emits chunks via callback
- Bridge captures chunks and forwards to `broadcast::Sender`
- WebSocket subscribers receive chunks in real-time

### 4. Session Persistence

- Helen's transcript system handles session persistence
- Rust SessionManager tracks active bridges (not session content)
- On restart, ChatSessionActor resumes from transcript

## Implementation Phases

### Phase 1: HelenActorBridge (Core)
- [ ] Create `actor_bridge.rs` module
- [ ] Implement Helen interpreter thread spawning
- [ ] Implement Channel-based message forwarding
- [ ] Test with simple Helen agent (not ChatSessionActor)

### Phase 2: ChatSessionActor Integration
- [ ] Load `chat_actor.helen` + dependencies
- [ ] Implement `spawn_chat_actor()` call from Rust
- [ ] Implement message protocol (user_input / response_complete / error)
- [ ] Test with mock LLM

### Phase 3: Streaming
- [ ] Implement streaming chunk capture
- [ ] Forward chunks to WebSocket
- [ ] Test real-time output

### Phase 4: Lifecycle Management
- [ ] Heartbeat mechanism
- [ ] Graceful shutdown
- [ ] Crash recovery
- [ ] Session resume

### Phase 5: Production Hardening
- [ ] Embed Helen files (rust-embed)
- [ ] Error handling and logging
- [ ] Performance optimization
- [ ] Integration tests

## Comparison: Current vs Target Architecture

| Aspect | Current (M24) | Target (V2) |
|--------|---------------|-------------|
| Web Server | Axum (Rust) | Axum (Rust) ✅ same |
| Frontend | React SPA (embedded) | React SPA (embedded) ✅ same |
| Agent Logic | Simplified Rust executor | Helen ChatSessionActor ✅ different |
| Tools | None (just LLM call) | 100+ tools via Helen ✅ different |
| Context Mgmt | None | Graduated compression ✅ different |
| Working Memory | None | 5000 tokens ✅ different |
| Slash Commands | None | Full set ✅ different |
| Transcript | Basic | Persistent, full-featured ✅ different |
| Streaming | Basic | Real-time chunks ✅ different |

## Risk Assessment

| Risk | Mitigation |
|------|-----------|
| Helen interpreter not thread-safe | Use dedicated thread per session |
| Channel message format mismatch | Define strict protocol in both Rust and Helen |
| Streaming chunk ordering | Use sequence numbers |
| Memory leaks (long-running sessions) | Implement session timeout + cleanup |
| Helen file loading failures | Embed files as fallback |

## Conclusion

The V2 architecture achieves the core requirement:
- **ChatSessionActor is 100% Helen code** (4709 lines)
- **Rust handles infrastructure** (HTTP, WebSocket, auth, interpreter lifecycle)
- **Clear boundary**: Rust spawns interpreter, Helen runs agent logic
- **No feature loss**: All Python reference features preserved

The key insight is that Rust doesn't need to reimplement the agent logic —
it just needs to provide the infrastructure to run Helen's agent system.
