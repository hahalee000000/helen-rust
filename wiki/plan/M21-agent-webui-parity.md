# M21: Agent WebUI Feature Parity

**Goal:** Align `hr agent` (Rust) with `hp agent` (Python) backend implementation

**Status:** Planning

**Date:** 2026-08-23

---

## Executive Summary

The Rust agent webui (`hr agent`) currently has a skeleton implementation with basic infrastructure but lacks the Helen-specific logic that makes the Python webui (`hp agent`) functional. This plan outlines a phased approach to achieve feature parity.

### Current State

**Python Backend (hp agent) — 23 API Endpoints:**
- ✅ Full Helen runtime integration via FFI
- ✅ Long-running actor with streaming callbacks
- ✅ Transcript-based session management (SSOT)
- ✅ Directory-based session boundaries (CWD = session)
- ✅ WebSocket broadcast with streaming protocol
- ✅ File upload with multimodal support
- ✅ Hint injection and stream cancellation

**Rust Backend (hr agent) — 5 API Endpoints:**
- ✅ Basic Axum server with static assets
- ⚠️ Generic session/file storage (not Helen-based)
- ❌ No Helen runtime integration
- ❌ No transcript reading
- ❌ No directory management
- ❌ Echo WebSocket (not streaming)
- ❌ No file upload for chat

### Target State

Full feature parity with Python backend, using Rust-native implementations:
- Direct integration with `helen-runtime` crate (no FFI overhead)
- Transcript-based session management
- Directory-based session boundaries
- WebSocket streaming with broadcast
- File upload with multimodal support

---

## Architecture Overview

### Python Architecture

```
Frontend (React)
    ↓ HTTP/WebSocket
FastAPI Backend
    ↓
HelenBridge (FFI)
    ↓
Python Helen Runtime
    ↓
ChatSessionActor (long-running)
    ↓
Transcript Store (.helen/sessions/*/transcript.jsonl)
```

**Key Services:**
- `HelenBridge` — Spawns Helen actor, manages streaming via FFI callbacks
- `ChannelActorManager` — Actor lifecycle with heartbeat
- `DirectoryManager` — CWD-based session management
- `SessionIndex` — Transcript → frontend messages
- `StreamRegistry` — Track active streams
- `HintInjector` — Inject hints via FFI queue
- `WebSocketManager` — Broadcast mode for multi-tab sync

### Rust Architecture (Target)

```
Frontend (React) — same as Python
    ↓ HTTP/WebSocket
Axum Backend
    ↓
HelenBridge (Rust)
    ↓
helen-runtime crate (direct)
    ↓
ChatSessionActor (long-running)
    ↓
Transcript Store (.helen/sessions/*/transcript.jsonl)
```

**Key Services (to implement):**
- `HelenBridge` — Spawn Helen actor, stream via callbacks
- `DirectoryManager` — CWD-based session management
- `TranscriptReader` — Parse transcript.jsonl
- `SessionIndex` — Transcript → frontend messages
- `StreamRegistry` — Track active streams
- `WebSocketManager` — Broadcast mode

---

## Implementation Phases

### Phase 1: Foundation (Core Infrastructure)

**Goal:** Establish data model and directory management

**Duration:** 2-3 days

#### Task 1.1: DirectoryManager

**File:** `crates/helen-agent/src/services/directory_manager.rs`

**Responsibilities:**
- Track current working directory (CWD)
- Map CWD → session ID (SHA256 hash)
- Provide paths to `.helen/` subdirectories
- Handle CWD switching with validation

**Key Functions:**
```rust
pub struct DirectoryManager {
    current_cwd: RwLock<PathBuf>,
}

impl DirectoryManager {
    pub fn new(initial_cwd: PathBuf) -> Self;
    pub fn get_current_cwd(&self) -> PathBuf;
    pub fn set_current_cwd(&self, path: &str) -> Result<SetCwdResult>;
    pub fn cwd_to_session_id(cwd: &Path) -> String;
    pub fn get_display_name(cwd: &Path) -> String;
    pub fn get_project_helen_dir(cwd: &Path) -> PathBuf;
    pub fn get_project_memory_path(cwd: &Path) -> PathBuf;
    pub fn get_project_user_path(cwd: &Path) -> PathBuf;
}
```

**Tests:**
- Unit tests for CWD mapping and path generation
- Integration test for CWD switching

#### Task 1.2: TranscriptReader

**File:** `crates/helen-agent/src/services/transcript_reader.rs`

**Responsibilities:**
- Read `.helen/sessions/{sid}/transcript.jsonl`
- Parse JSONL entries
- Filter test messages and metadata
- Extract session metadata

**Key Functions:**
```rust
pub struct TranscriptReader;

impl TranscriptReader {
    pub fn get_transcript_path(cwd: &Path, session_id: &str) -> Option<PathBuf>;
    pub fn read_transcript_entries(session_id: &str) -> Result<Vec<TranscriptEntry>>;
    pub fn get_current_helen_session_id(cwd: &Path) -> String;
    pub fn filter_test_messages(entries: Vec<TranscriptEntry>) -> Vec<TranscriptEntry>;
}

pub struct TranscriptEntry {
    pub entry_type: String,
    pub role: Option<String>,
    pub content: serde_json::Value,
    pub uuid: Option<String>,
    pub timestamp: Option<i64>,
}
```

**Tests:**
- Parse sample transcript files
- Filter test messages correctly
- Handle malformed JSON gracefully

#### Task 1.3: SessionIndex

**File:** `crates/helen-agent/src/services/session_index.rs`

**Responsibilities:**
- Convert transcript entries → frontend Message format
- Extract user input from multimodal content
- Filter boilerplate (system prompts)
- Handle attachments (images, audio, media_ref)

**Key Functions:**
```rust
pub struct SessionIndex;

impl SessionIndex {
    pub fn transcript_to_messages(session_id: &str) -> Result<Vec<FrontendMessage>>;
    pub fn read_session_preview(session_id: &str, max_chars: usize) -> String;
}

pub struct FrontendMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub attachments: Vec<Attachment>,
    pub timestamp: Option<String>,
}

pub struct Attachment {
    pub id: String,
    pub filename: String,
    pub mime_type: String,
    pub size: u64,
    pub url: String,
}
```

**Complex Logic:**
- `_extract_from_boilerplate()` — Separate user input from system prompts
- `_parse_user_content()` — Handle multimodal arrays (text, image_url, input_audio, media_ref)
- `_filter_system_hints()` — Extract [System Hint] lines
- `_attachment_from_data_url()` — Parse data URLs for historical attachments

**Tests:**
- Convert various transcript formats
- Handle multimodal content correctly
- Filter boilerplate accurately

#### Task 1.4: Update AppState

**File:** `crates/helen-agent/src/api/sessions.rs`

**Changes:**
- Add `DirectoryManager` to `AppStateInner`
- Keep generic `SessionManager` for backward compatibility (optional)
- Update initialization to use CWD from environment

```rust
pub struct AppStateInner {
    pub directory_manager: DirectoryManager,
    pub session_manager: SessionManager,  // Keep for now
    pub file_storage: FileStorage,
}
```

**Deliverable:** Can read current session's transcript and convert to messages

---

### Phase 2: Chat API (Read-Only)

**Goal:** Implement read-only chat endpoints

**Duration:** 2-3 days

#### Task 2.1: GET /api/chat/dir

**File:** `crates/helen-agent/src/api/chat.rs`

**Implementation:**
```rust
async fn get_directory(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let inner = state.lock().await;
    let cwd = inner.directory_manager.get_current_cwd();
    let display_name = inner.directory_manager.get_display_name(&cwd);
    let session_id = DirectoryManager::cwd_to_session_id(&cwd);
    
    Json(json!({
        "cwd": cwd,
        "display_name": display_name,
        "session_id": session_id,
        "helen_session_id": null  // TODO: Get from HelenBridge
    }))
}
```

#### Task 2.2: POST /api/chat/dir

**Implementation:**
```rust
async fn change_directory(
    State(state): State<AppState>,
    Json(body): Json<ChangeDirRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let inner = state.lock().await;
    let result = inner.directory_manager.set_current_cwd(&body.path)?;
    
    Ok(Json(json!({
        "status": "ok",
        "cwd": result.cwd,
        "display_name": result.display_name,
        "session_id": DirectoryManager::cwd_to_session_id(&result.cwd),
    })))
}
```

#### Task 2.3: GET /api/chat/dir/messages

**Implementation:**
```rust
async fn get_directory_messages(
    State(state): State<AppState>,
    Query(params): Query<MessagesQuery>,
) -> Json<Vec<FrontendMessage>> {
    let inner = state.lock().await;
    let cwd = inner.directory_manager.get_current_cwd();
    let session_id = SessionIndex::get_current_helen_session_id(&cwd);
    
    let messages = SessionIndex::transcript_to_messages(&session_id)
        .unwrap_or_default();
    
    // Apply offset and limit
    let messages = messages.into_iter()
        .skip(params.offset.unwrap_or(0))
        .take(params.limit.unwrap_or(100))
        .collect();
    
    Json(messages)
}
```

#### Task 2.4: GET /api/chat/sessions

**Implementation:**
```rust
async fn list_sessions(
    State(state): State<AppState>,
) -> Json<Vec<SessionInfo>> {
    let inner = state.lock().await;
    let cwd = inner.directory_manager.get_current_cwd();
    let sessions_dir = cwd.join(".helen").join("sessions");
    
    let mut sessions = Vec::new();
    if sessions_dir.exists() {
        for entry in fs::read_dir(&sessions_dir).unwrap() {
            let entry = entry.unwrap();
            if !entry.path().is_dir() { continue; }
            
            let transcript = entry.path().join("transcript.jsonl");
            if !transcript.exists() { continue; }
            
            let stat = fs::metadata(&transcript).unwrap();
            let message_count = count_messages_in_transcript(&transcript);
            
            sessions.push(SessionInfo {
                session_id: entry.file_name().to_string_lossy().to_string(),
                created_at: stat.created().unwrap().timestamp(),
                modified_at: stat.modified().unwrap().timestamp(),
                size_bytes: stat.len(),
                message_count,
                preview: SessionIndex::read_session_preview(&entry.file_name().to_string_lossy(), 50),
            });
        }
    }
    
    sessions.sort_by(|a, b| b.modified_at.cmp(&a.modified_at));
    Json(sessions)
}
```

#### Task 2.5: GET /api/chat/sessions/{id}/transcript

**Implementation:**
```rust
async fn get_transcript(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let inner = state.lock().await;
    let cwd = inner.directory_manager.get_current_cwd();
    
    let transcript_path = TranscriptReader::get_transcript_path(&cwd, &session_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    
    let entries = TranscriptReader::read_transcript_entries(&session_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let entries = TranscriptReader::filter_test_messages(entries);
    let entries = entries.into_iter()
        .filter(|e| e.entry_type != "session_meta")
        .collect::<Vec<_>>();
    
    // Count roles and tool calls
    let mut roles = HashMap::new();
    let mut tool_calls_count = 0;
    for e in &entries {
        if e.entry_type == "message" {
            if let Some(role) = &e.role {
                *roles.entry(role.clone()).or_insert(0) += 1;
            }
            // Count tool calls from content
            if let Some(content) = e.content.as_str() {
                if content.starts_with("Tool calls:") {
                    tool_calls_count += content.matches("\\w+\\(").count();
                }
            }
        }
    }
    
    Ok(Json(json!({
        "session_id": session_id,
        "file": transcript_path,
        "total_entries": entries.len(),
        "roles": roles,
        "tool_calls_count": tool_calls_count,
        "entries": entries,
    })))
}
```

#### Task 2.6: GET /api/chat/sessions/{id}/messages

**Implementation:**
```rust
async fn get_session_messages(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Json<Vec<FrontendMessage>> {
    let messages = SessionIndex::transcript_to_messages(&session_id)
        .unwrap_or_default();
    Json(messages)
}
```

**Deliverable:** Frontend can load chat history and switch sessions

---

### Phase 3: Helen Runtime Integration (Critical)

**Goal:** Spawn Helen actor and stream responses

**Duration:** 5-7 days (most complex phase)

#### Task 3.1: HelenBridge

**File:** `crates/helen-agent/src/services/helen_bridge.rs`

**Responsibilities:**
- Spawn Helen actor process
- Manage actor lifecycle (spawn, send messages, receive chunks)
- Stream chunks via callbacks
- Handle heartbeat

**Key Design Decision:**
Use `helen-runtime` crate directly (no subprocess, no FFI).

**Implementation Sketch:**
```rust
pub struct HelenBridge {
    actor_handle: RwLock<Option<ActorHandle>>,
    stream_callbacks: RwLock<HashMap<String, StreamCallback>>,
}

impl HelenBridge {
    pub async fn ensure_actor(&self) -> Result<SpawnResult>;
    pub async fn send_message(&self, input: &str, files: Vec<PathBuf>) -> Result<String>;
    pub async fn run_chat_streaming(
        &self,
        input: &str,
        session_id: &str,
        files: Vec<PathBuf>,
        callback: StreamCallback,
    ) -> Result<()>;
    pub async fn get_session_id(&self) -> String;
    pub async fn list_sessions(&self) -> Vec<SessionInfo>;
}

pub type StreamCallback = Arc<dyn Fn(StreamEvent) + Send + Sync>;

pub enum StreamEvent {
    LlmChunk { content: String },
    StatusUpdate { data: serde_json::Value },
    AgentStart { content: String },
    AgentEnd { content: String },
    ProcessingStart,
    ProcessingComplete,
    Error { content: String },
    Complete,
}
```

**Challenges:**
- Understanding the `helen-runtime` actor API
- Implementing streaming callbacks in Rust
- Managing async/sync boundaries

**Tests:**
- Spawn actor successfully
- Send message and receive response
- Stream chunks correctly

#### Task 3.2: Streaming Protocol

**File:** `crates/helen-agent/src/services/stream_registry.rs`

**Responsibilities:**
- Track active streaming sessions
- Provide `is_processing()` flag
- Thread-safe registration/unregistration

**Implementation:**
```rust
pub struct StreamRegistry {
    active_sessions: RwLock<HashSet<String>>,
}

impl StreamRegistry {
    pub fn register(&self, session_id: &str);
    pub fn unregister(&self, session_id: &str);
    pub fn is_processing(&self) -> bool;
}
```

#### Task 3.3: POST /api/chat/send (via WebSocket)

**Note:** The Python backend doesn't have a POST /send endpoint. All chat goes through WebSocket.

**Implementation:** See Task 3.4 (WebSocket handler)

#### Task 3.4: WebSocket Handler

**File:** `crates/helen-agent/src/websocket/mod.rs`

**Responsibilities:**
- Accept WebSocket connections
- Broadcast messages to all connections (multi-tab sync)
- Handle streaming chunks from HelenBridge
- Process user messages

**Implementation:**
```rust
pub struct WebSocketManager {
    connections: RwLock<Vec<Arc<WebSocketSink>>>,
}

impl WebSocketManager {
    pub async fn connect(&self, socket: WebSocket) -> Arc<WebSocketSink>;
    pub fn disconnect(&self, sink: &Arc<WebSocketSink>);
    pub async fn broadcast(&self, message: serde_json::Value);
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let inner = state.lock().await;
    let sink = inner.ws_manager.connect(socket).await;
    
    // Message loop
    loop {
        let msg = sink.recv().await;
        match msg {
            Some(Ok(Message::Text(text))) => {
                let data: serde_json::Value = serde_json::from_str(&text).unwrap();
                
                if data["type"] == "chat" {
                    let user_input = data["message"].as_str().unwrap();
                    let files = data["files"].as_array().unwrap_or(&vec![]);
                    
                    // Spawn streaming task
                    let state_clone = state.clone();
                    let sink_clone = sink.clone();
                    tokio::spawn(async move {
                        let bridge = state_clone.lock().await.helen_bridge.clone();
                        let session_id = "..."; // Get from DirectoryManager
                        
                        let callback = Arc::new(move |event: StreamEvent| {
                            let message = match event {
                                StreamEvent::LlmChunk { content } => {
                                    json!({"type": "llm_chunk", "data": {"content": content}})
                                }
                                // ... handle other events
                            };
                            
                            // Broadcast to all connections
                            tokio::spawn(async move {
                                state_clone.lock().await.ws_manager.broadcast(message).await;
                            });
                        });
                        
                        bridge.run_chat_streaming(user_input, session_id, files, callback).await.ok();
                    });
                }
            }
            Some(Ok(Message::Close(_))) => break,
            _ => {}
        }
    }
    
    inner.ws_manager.disconnect(&sink);
}
```

#### Task 3.5: GET /api/chat/status

**Update existing endpoint:**
```rust
async fn get_status(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let inner = state.lock().await;
    let is_processing = inner.stream_registry.is_processing();
    
    Json(json!({
        "is_processing": is_processing,
        "version": env!("CARGO_PKG_VERSION"),
        "config": {
            "helen_path": "...", // Get from HelenBridge
        }
    }))
}
```

**Deliverable:** Can send messages and receive streaming responses

---

### Phase 4: File Upload & Media

**Goal:** Support file attachments in chat

**Duration:** 2-3 days

#### Task 4.1: POST /api/chat/upload

**File:** `crates/helen-agent/src/api/chat.rs`

**Implementation:**
```rust
async fn upload_file(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let inner = state.lock().await;
    let cwd = inner.directory_manager.get_current_cwd();
    let upload_dir = cwd.join(".helen").join("uploads");
    fs::create_dir_all(&upload_dir).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    let mut filename = String::new();
    let mut content = Vec::new();
    
    while let Some(field) = multipart.next_field().await.unwrap() {
        let name = field.name().unwrap_or("").to_string();
        
        if name == "file" {
            filename = field.file_name().unwrap_or("upload").to_string();
            content = field.bytes().await.unwrap().to_vec();
        }
    }
    
    // Validate size (50MB limit)
    if content.len() > 50 * 1024 * 1024 {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    
    // Validate MIME type
    let mime = mime_guess::from_path(&filename).first_or_octet_stream();
    if !is_allowed_mime(&mime) {
        return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    
    // Save file
    let file_id = Uuid::new_v4().to_string();
    let file_path = upload_dir.join(&file_id);
    fs::write(&file_path, &content).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    Ok(Json(json!({
        "id": file_id,
        "filename": filename,
        "size": content.len(),
        "url": format!("/api/chat/uploads/{}/file", file_id),
    })))
}

fn is_allowed_mime(mime: &mime::Mime) -> bool {
    let allowed = [
        "image/jpeg", "image/png", "image/gif", "image/webp",
        "audio/mpeg", "audio/wav", "audio/ogg", "audio/mp4",
        "video/mp4", "video/webm", "video/quicktime",
    ];
    allowed.contains(&mime.essence_str())
}
```

#### Task 4.2: GET /api/chat/uploads/{id}/file

**Implementation:**
```rust
async fn download_upload(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let inner = state.lock().await;
    let cwd = inner.directory_manager.get_current_cwd();
    let file_path = cwd.join(".helen").join("uploads").join(&id);
    
    if !file_path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }
    
    let content = fs::read(&file_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mime = mime_guess::from_path(&file_path).first_or_octet_stream();
    
    Ok((
        [(header::CONTENT_TYPE, mime.to_string())],
        content,
    ))
}
```

#### Task 4.3: GET /api/chat/sessions/{id}/media/{filename}

**Implementation:**
```rust
async fn get_session_media(
    State(state): State<AppState>,
    Path((session_id, filename)): Path<(String, String)>,
) -> Result<impl IntoResponse, StatusCode> {
    // Validate filename (prevent path traversal)
    if filename.contains('/') || filename.contains('\\') || filename.contains("..") {
        return Err(StatusCode::BAD_REQUEST);
    }
    
    let inner = state.lock().await;
    let cwd = inner.directory_manager.get_current_cwd();
    let transcript_path = TranscriptReader::get_transcript_path(&cwd, &session_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    
    let media_dir = transcript_path.parent().unwrap().join("media");
    let media_path = media_dir.join(&filename);
    
    // Security check: ensure file is within media directory
    let real_media = fs::canonicalize(&media_path).map_err(|_| StatusCode::NOT_FOUND)?;
    let real_media_dir = fs::canonicalize(&media_dir).map_err(|_| StatusCode::NOT_FOUND)?;
    if !real_media.starts_with(&real_media_dir) {
        return Err(StatusCode::FORBIDDEN);
    }
    
    let content = fs::read(&media_path).map_err(|_| StatusCode::NOT_FOUND)?;
    let mime = mime_guess::from_path(&media_path).first_or_octet_stream();
    
    Ok((
        [(header::CONTENT_TYPE, mime.to_string())],
        content,
    ))
}
```

#### Task 4.4: Update Transcript Parser

**File:** `crates/helen-agent/src/services/session_index.rs`

**Changes:**
- Handle `media_ref` parts in multimodal content
- Generate correct URLs for session media

**Implementation:**
```rust
fn _parse_user_content(content: &serde_json::Value) -> (String, Vec<Attachment>, bool) {
    if let Some(array) = content.as_array() {
        let mut text_lines = Vec::new();
        let mut attachments = Vec::new();
        
        for part in array {
            if let Some(text) = part.as_str() {
                text_lines.push(text.to_string());
            } else if let Some(obj) = part.as_object() {
                let ptype = obj.get("type").and_then(|t| t.as_str()).unwrap_or("");
                
                if ptype == "text" {
                    if let Some(text) = obj.get("text").and_then(|t| t.as_str()) {
                        text_lines.push(text.to_string());
                    }
                } else if ptype == "image_url" {
                    let url = obj.get("image_url")
                        .and_then(|iu| iu.get("url"))
                        .and_then(|u| u.as_str())
                        .unwrap_or("");
                    if !url.is_empty() {
                        attachments.push(_attachment_from_data_url(url, attachments.len() + 1));
                    }
                } else if ptype == "media_ref" {
                    let path = obj.get("path").and_then(|p| p.as_str()).unwrap_or("");
                    if !path.is_empty() {
                        let path = Path::new(path);
                        let filename = path.file_name().unwrap().to_string_lossy().to_string();
                        let sid = path.parent().unwrap().parent().unwrap().file_name().unwrap().to_string_lossy().to_string();
                        let mime = obj.get("mime").and_then(|m| m.as_str()).unwrap_or("application/octet-stream");
                        let size = obj.get("size").and_then(|s| s.as_u64()).unwrap_or(0);
                        
                        attachments.push(Attachment {
                            id: format!("media-{}", attachments.len() + 1),
                            filename,
                            mime_type: mime.to_string(),
                            size,
                            url: format!("/api/chat/sessions/{}/media/{}", sid, filename),
                        });
                    }
                }
            }
        }
        
        let joined = text_lines.join("\n");
        let (user_text, _) = _extract_from_boilerplate(&joined);
        (user_text, attachments, false)
    } else if let Some(s) = content.as_str() {
        // Plain string
        if s.trim().starts_with("__helen_") {
            return ("".to_string(), vec![], true);
        }
        let (user_text, _) = _extract_from_boilerplate(s);
        (user_text, vec![], false)
    } else {
        ("".to_string(), vec![], false)
    }
}
```

**Deliverable:** Can upload files and see them in chat

---

### Phase 5: Advanced Features

**Goal:** Implement remaining features

**Duration:** 2-3 days

#### Task 5.1: POST /api/chat/stop

**Implementation:**
```rust
async fn stop_chat(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let inner = state.lock().await;
    let bridge = inner.helen_bridge.clone();
    
    // Cancel active stream
    bridge.cancel_session().await;
    
    Json(json!({"status": "ok"}))
}
```

**Note:** Requires implementing `cancel_session()` in HelenBridge

#### Task 5.2: POST /api/chat/hint

**Implementation:**
```rust
async fn inject_hint(
    State(state): State<AppState>,
    Json(body): Json<HintRequest>,
) -> Json<serde_json::Value> {
    let inner = state.lock().await;
    let bridge = inner.helen_bridge.clone();
    
    // Inject hint into Helen runtime
    bridge.inject_hint(&body.text).await;
    
    Json(json!({"status": "ok"}))
}
```

**Note:** Requires implementing hint injection in HelenBridge

#### Task 5.3: DELETE /api/chat/sessions/{id}

**Implementation:**
```rust
async fn delete_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Json<serde_json::Value> {
    let inner = state.lock().await;
    let cwd = inner.directory_manager.get_current_cwd();
    
    // Delete transcript directory
    let session_dir = cwd.join(".helen").join("sessions").join(&session_id);
    if session_dir.exists() {
        fs::remove_dir_all(&session_dir).ok();
    }
    
    Json(json!({"status": "ok", "message": "Session deleted"}))
}
```

#### Task 5.4: GET /api/agents/status

**File:** `crates/helen-agent/src/api/agents.rs`

**Implementation:**
```rust
async fn get_all_agents_status() -> Json<serde_json::Value> {
    // Return mock data for now
    // TODO: Query actual agent status from Helen runtime
    Json(json!({
        "Contractor": {"status": "idle", "last_task": null},
        "TestBuilder": {"status": "idle", "last_task": null},
        "Implementer": {"status": "idle", "last_task": null},
        "QualityGate": {"status": "idle", "last_task": null},
        "SkillEvaluator": {"status": "idle", "last_task": null},
    }))
}
```

#### Task 5.5: GET /api/agents/{name}/status

**Implementation:**
```rust
async fn get_agent_status(
    Path(agent_name): Path<String>,
) -> Json<serde_json::Value> {
    // Return mock data for now
    Json(json!({
        "name": agent_name,
        "status": "idle",
        "last_task": null,
    }))
}
```

**Deliverable:** Full feature parity with Python backend

---

## Testing Strategy

### Unit Tests

**Location:** `crates/helen-agent/src/services/*.rs`

**Coverage:**
- DirectoryManager: CWD mapping, path generation, switching
- TranscriptReader: JSONL parsing, filtering
- SessionIndex: Message conversion, boilerplate filtering, attachment handling
- StreamRegistry: Registration, thread safety

### Integration Tests

**Location:** `crates/helen-agent/tests/`

**Coverage:**
- API endpoint responses (using reqwest)
- WebSocket streaming (using tokio-tungstenite)
- File upload/download
- Session management

### Manual Testing

**Tools:**
- `curl` for HTTP endpoints
- Browser for frontend integration
- `websocat` for WebSocket testing

**Test Scenarios:**
1. Load chat history from existing transcript
2. Switch directories and verify session changes
3. Send message and receive streaming response
4. Upload file and see attachment in chat
5. Delete session and verify cleanup

---

## Migration Notes

### Python → Rust Mapping

| Python | Rust |
|--------|------|
| `os.getcwd()` | `std::env::current_dir()` |
| `os.chdir(path)` | `std::env::set_current_dir(path)` |
| `pathlib.Path` | `std::path::Path` |
| `asyncio.Queue` | `tokio::sync::mpsc` or `tokio::sync::broadcast` |
| `threading.Lock` | `std::sync::Mutex` or `tokio::sync::Mutex` |
| `json.loads()` | `serde_json::from_str()` |
| `json.dumps()` | `serde_json::to_string()` |
| `FastAPI` | `axum::Router` |
| `WebSocket` | `axum::extract::ws::WebSocket` |
| `Multipart` | `axum::extract::Multipart` |

### Key Differences

1. **Async Model:**
   - Python: `asyncio` with `async/await`
   - Rust: `tokio` with `async/await` (similar)

2. **Error Handling:**
   - Python: Exceptions with `try/except`
   - Rust: `Result<T, E>` with `?` operator

3. **Concurrency:**
   - Python: GIL limits true parallelism
   - Rust: True parallelism with threads

4. **Memory Management:**
   - Python: Garbage collected
   - Rust: Ownership and borrowing

---

## Dependencies

### New Crates

Add to `crates/helen-agent/Cargo.toml`:

```toml
[dependencies]
# Existing...
mime = "0.3"
mime_guess = "2"
sha2 = "0.10"  # For session ID hashing
regex = "1"  # For boilerplate filtering
base64 = "0.21"  # For data URL parsing

# Already present
helen-runtime = { version = "0.1.3", path = "../helen-runtime" }
```

---

## Risks and Mitigations

### Risk 1: Helen Runtime API Complexity

**Risk:** The `helen-runtime` actor API may be complex or undocumented.

**Mitigation:**
- Study Python implementation thoroughly
- Start with simple message passing before streaming
- Add extensive logging for debugging

### Risk 2: Streaming Callback Design

**Risk:** Bridging async Rust with streaming callbacks is tricky.

**Mitigation:**
- Use `tokio::sync::broadcast` for fan-out
- Test with simple echo before integrating Helen
- Profile for performance bottlenecks

### Risk 3: Transcript Parsing Edge Cases

**Risk:** Transcript format may have edge cases not covered by tests.

**Mitigation:**
- Port Python tests verbatim
- Add fuzzing for malformed JSON
- Log parsing errors for investigation

---

## Success Criteria

### Phase 1 (Foundation)
- [ ] DirectoryManager passes all unit tests
- [ ] TranscriptReader parses sample transcripts correctly
- [ ] SessionIndex converts messages accurately

### Phase 2 (Read-Only API)
- [ ] Frontend loads chat history from transcript
- [ ] Can switch directories and see different sessions
- [ ] All read-only endpoints return correct data

### Phase 3 (Streaming)
- [ ] Can send message and receive streaming response
- [ ] WebSocket broadcasts to multiple tabs
- [ ] `is_processing` flag works correctly

### Phase 4 (Files)
- [ ] Can upload files up to 50MB
- [ ] Attachments display in chat
- [ ] Session media accessible via URL

### Phase 5 (Advanced)
- [ ] Can cancel active stream
- [ ] Can inject hints
- [ ] Can delete sessions
- [ ] Agent status endpoints return data

### Final
- [ ] All Python backend tests pass in Rust
- [ ] Frontend works identically with both backends
- [ ] No regressions in existing functionality
- [ ] Performance ≥ Python backend (10x target)

---

## Timeline

| Phase | Duration | Dependencies |
|-------|----------|--------------|
| Phase 1: Foundation | 2-3 days | None |
| Phase 2: Read-Only API | 2-3 days | Phase 1 |
| Phase 3: Streaming | 5-7 days | Phase 2 |
| Phase 4: Files | 2-3 days | Phase 3 |
| Phase 5: Advanced | 2-3 days | Phase 3 |
| **Total** | **13-19 days** | |

---

## Conclusion

This plan provides a clear path to feature parity between the Rust and Python agent backends. The phased approach ensures incremental progress and early validation of critical components (especially Helen runtime integration).

**Next Steps:**
1. Review and approve this plan
2. Begin Phase 1 implementation
3. Set up testing infrastructure
4. Document any API discoveries for future reference
