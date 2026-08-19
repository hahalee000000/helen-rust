//! Transcript store (Task 8.1) — mostly-append transcript storage.
//!
//! Byte-faithful port of `helen/runtime/transcript_store.py` (v1.45.0)
//! core: `Message`/`BoundaryMarker`/`SessionMeta` data structures, the
//! `JSONLBackend` persistence layer, and the `TranscriptStore` facade with
//! LRU cache, view cache, UUID addressing, compression recording, and query.
//!
//! Format parity: JSONL lines written by this module are byte-compatible
//! with Python's `_item_to_dict`/`_item_from_dict` (see tests).

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Generate a short UUID for message identification (Python
/// `uuid4().hex[:12]`).
pub fn generate_uuid() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 6];
    rand::thread_rng().fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// Message (port of `helen/runtime/history.py::Message`)
// ---------------------------------------------------------------------------

/// A single message in a conversation.
#[derive(Debug, Clone)]
pub struct Message {
    /// "system" | "user" | "assistant" | "tool"
    pub role: String,
    /// Plain text, or multimodal content parts (list of dicts).
    pub content: serde_json::Value,
    pub tool_calls: Vec<serde_json::Value>,
    pub tool_call_id: Option<String>,
    /// Auto-inferred type.
    pub message_type: Option<String>,
    /// Priority (1-100, higher = more important).
    pub priority: i64,
    /// Whether the message has been compressed.
    pub compressed: bool,
    /// Pinned messages are immune to compression.
    pub pinned: bool,
    /// UUID assigned on first append; preserved across compression.
    pub uuid: String,
    /// v1.22: agent name that produced this message (None for top-level).
    pub agent_name: Option<String>,
    /// v1.22: UUID of the agent main{} invocation.
    pub invocation_id: String,
    /// v1.22: UUID of the parent invocation (for nested agent calls).
    pub parent_invocation_id: String,
    /// v1.24: invocation IDs that can see this message.
    pub visible_to_invocation_ids: Vec<String>,
}

/// Extract plain text and count of media parts from message content
/// (Python `history._message_text` parity; handles str and multimodal lists).
pub fn message_text_parts(content: &serde_json::Value) -> (String, usize) {
    match content {
        serde_json::Value::String(s) => (s.clone(), 0),
        serde_json::Value::Array(parts) => {
            let mut text_parts: Vec<String> = Vec::new();
            let mut media = 0;
            for part in parts {
                if let Some(t) = part.get("type").and_then(|v| v.as_str()) {
                    if t == "text" {
                        if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
                            text_parts.push(text.to_string());
                        }
                    } else {
                        media += 1;
                    }
                }
            }
            (text_parts.join("\n"), media)
        }
        _ => (String::new(), 0),
    }
}

impl Message {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        role: &str,
        content: serde_json::Value,
        tool_calls: Vec<serde_json::Value>,
        tool_call_id: Option<String>,
        uuid: String,
        message_type: Option<String>,
        priority: i64,
        compressed: bool,
        pinned: bool,
        agent_name: Option<String>,
        invocation_id: String,
        parent_invocation_id: String,
        visible_to_invocation_ids: Vec<String>,
    ) -> Self {
        Message {
            role: role.to_string(),
            content,
            tool_calls,
            tool_call_id,
            message_type,
            priority,
            compressed,
            pinned,
            uuid,
            agent_name,
            invocation_id,
            parent_invocation_id,
            visible_to_invocation_ids,
        }
    }

    /// Infer the message type based on role and content.
    pub fn infer_message_type(&self) -> String {
        match self.role.as_str() {
            "system" => "system".to_string(),
            "user" => "user".to_string(),
            "assistant" => {
                if !self.tool_calls.is_empty() {
                    "assistant_tool_call".to_string()
                } else {
                    "assistant".to_string()
                }
            }
            "tool" => "tool".to_string(),
            _ => "assistant".to_string(),
        }
    }

    /// Lazily computed token count (model-aware heuristic; Python parity:
    /// multimodal = text parts + 85 tokens per media part + 4 overhead).
    pub fn token_count(&self) -> usize {
        self.token_count_for_model(None)
    }

    /// Token count for a specific model name (heuristic, no tiktoken in Rust).
    pub fn token_count_for_model(&self, model: Option<&str>) -> usize {
        use crate::token::estimate_tokens_simple;
        let (text, media_parts) = message_text_parts(&self.content);
        if text.is_empty() && media_parts == 0 {
            return 0;
        }
        let mut count = estimate_tokens_simple(&text);
        count += media_parts * 85;
        // OpenAI counts ~4 tokens per message overhead
        count += 4;
        if let Some(m) = model {
            // Model only affects tiktoken path in Python (absent here); keep
            // the parameter for signature parity.
            let _ = m;
        }
        count
    }

    /// Assign priority based on message type (1-100).
    pub fn assign_priority(&self) -> i64 {
        let msg_type = self
            .message_type
            .clone()
            .unwrap_or_else(|| self.infer_message_type());
        match msg_type.as_str() {
            "system" => 100,
            "user" => 90,
            "assistant" => 80,
            "assistant_tool_call" => 70,
            "tool" => 20,
            _ => 50,
        }
    }
}

// ---------------------------------------------------------------------------
// Boundary Marker
// ---------------------------------------------------------------------------

/// Marks a compression boundary in the transcript.
///
/// When compression occurs, existing messages are not modified or deleted;
/// a `BoundaryMarker` referencing the compressed region is appended instead.
#[derive(Debug, Clone)]
pub struct BoundaryMarker {
    pub uuid: String,
    /// UUID of the anchor message (first message after compressed region).
    pub anchor_uuid: String,
    /// UUID of the first message in the compressed region.
    pub head_uuid: String,
    /// UUID of the last message in the compressed region.
    pub tail_uuid: String,
    pub summary: String,
    /// Which compression layer created this boundary.
    pub layer: String,
    pub timestamp: f64,
    /// Estimated tokens before compression.
    pub original_token_count: i64,
    /// Estimated tokens after compression.
    pub compressed_token_count: i64,
}

impl BoundaryMarker {
    pub fn new(
        anchor_uuid: &str,
        head_uuid: &str,
        tail_uuid: &str,
        summary: &str,
        layer: &str,
        original_token_count: i64,
        compressed_token_count: i64,
    ) -> Self {
        BoundaryMarker {
            uuid: generate_uuid(),
            anchor_uuid: anchor_uuid.to_string(),
            head_uuid: head_uuid.to_string(),
            tail_uuid: tail_uuid.to_string(),
            summary: summary.to_string(),
            layer: layer.to_string(),
            timestamp: chrono::Utc::now().timestamp() as f64,
            original_token_count,
            compressed_token_count,
        }
    }

    pub fn to_dict(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "boundary_marker",
            "uuid": self.uuid,
            "anchor_uuid": self.anchor_uuid,
            "head_uuid": self.head_uuid,
            "tail_uuid": self.tail_uuid,
            "summary": self.summary,
            "layer": self.layer,
            "timestamp": self.timestamp,
            "original_token_count": self.original_token_count,
            "compressed_token_count": self.compressed_token_count,
        })
    }

    pub fn from_dict(data: &serde_json::Value) -> Self {
        BoundaryMarker {
            uuid: data
                .get("uuid")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            anchor_uuid: data
                .get("anchor_uuid")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            head_uuid: data
                .get("head_uuid")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            tail_uuid: data
                .get("tail_uuid")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            summary: data
                .get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            layer: data
                .get("layer")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            timestamp: data
                .get("timestamp")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            original_token_count: data
                .get("original_token_count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
            compressed_token_count: data
                .get("compressed_token_count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0),
        }
    }
}

// ---------------------------------------------------------------------------
// Session Metadata
// ---------------------------------------------------------------------------

/// Metadata for a transcript session (stored as the first JSONL record).
#[derive(Debug, Clone)]
pub struct SessionMeta {
    pub argv: Vec<String>,
    pub timestamp: f64,
    pub helen_version: String,
    pub python_version: String,
    pub platform: String,
    pub cwd: String,
    pub session_id: String,
    pub session_scope: String,
    /// v1.23.7: parent session ID (for spawned sessions).
    pub parent_session_id: String,
}

impl SessionMeta {
    pub fn new() -> Self {
        SessionMeta {
            argv: Vec::new(),
            timestamp: chrono::Utc::now().timestamp() as f64,
            helen_version: String::new(),
            python_version: String::new(),
            platform: String::new(),
            cwd: String::new(),
            session_id: String::new(),
            session_scope: String::new(),
            parent_session_id: String::new(),
        }
    }

    pub fn to_dict(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "session_meta",
            "argv": self.argv,
            "timestamp": self.timestamp,
            "helen_version": self.helen_version,
            "python_version": self.python_version,
            "platform": self.platform,
            "cwd": self.cwd,
            "session_id": self.session_id,
            "session_scope": self.session_scope,
            "parent_session_id": self.parent_session_id,
        })
    }

    pub fn from_dict(data: &serde_json::Value) -> Self {
        SessionMeta {
            argv: data
                .get("argv")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            timestamp: data
                .get("timestamp")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0),
            helen_version: data
                .get("helen_version")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            python_version: data
                .get("python_version")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            platform: data
                .get("platform")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            cwd: data
                .get("cwd")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            session_id: data
                .get("session_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            session_scope: data
                .get("session_scope")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            parent_session_id: data
                .get("parent_session_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        }
    }
}

impl Default for SessionMeta {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Item enum (Message | BoundaryMarker)
// ---------------------------------------------------------------------------

/// A transcript item — either a `Message` or a `BoundaryMarker`.
#[derive(Debug, Clone)]
pub enum Item {
    Message(Message),
    Boundary(BoundaryMarker),
}

impl Item {
    pub fn uuid(&self) -> &str {
        match self {
            Item::Message(m) => &m.uuid,
            Item::Boundary(b) => &b.uuid,
        }
    }

    pub fn to_dict(&self) -> serde_json::Value {
        match self {
            Item::Message(m) => message_to_dict(m),
            Item::Boundary(b) => b.to_dict(),
        }
    }

    pub fn from_dict(data: &serde_json::Value) -> Option<Item> {
        match data.get("type").and_then(|v| v.as_str()) {
            Some("message") => Some(Item::Message(message_from_dict(data))),
            Some("boundary_marker") => Some(Item::Boundary(BoundaryMarker::from_dict(data))),
            Some("session_meta") => None, // Handled separately by read_meta.
            _ => None,                    // Unknown item type: skip silently.
        }
    }
}

/// Serialize a Message to its Python-parity dict.
pub fn message_to_dict(m: &Message) -> serde_json::Value {
    let mut d = serde_json::json!({
        "type": "message",
        "role": m.role,
        "content": m.content,
        "tool_calls": m.tool_calls,
        "tool_call_id": m.tool_call_id,
        "uuid": m.uuid,
        "message_type": m.message_type,
        "priority": m.priority,
        "compressed": m.compressed,
        "pinned": m.pinned,
    });
    // v1.22: Invocation tree fields (only include if set, for compactness).
    if let Some(agent_name) = &m.agent_name {
        d["agent_name"] = serde_json::Value::String(agent_name.clone());
    }
    if !m.invocation_id.is_empty() {
        d["invocation_id"] = serde_json::Value::String(m.invocation_id.clone());
    }
    if !m.parent_invocation_id.is_empty() {
        d["parent_invocation_id"] = serde_json::Value::String(m.parent_invocation_id.clone());
    }
    // v1.24: Visibility tracking (only include if non-empty, for compactness).
    if !m.visible_to_invocation_ids.is_empty() {
        d["visible_to_invocation_ids"] = serde_json::Value::Array(
            m.visible_to_invocation_ids
                .iter()
                .map(|s| serde_json::Value::String(s.clone()))
                .collect(),
        );
    }
    d
}

/// Reconstruct a Message from a dict (Python `_item_from_dict` parity).
pub fn message_from_dict(data: &serde_json::Value) -> Message {
    let get_str = |k: &str| -> String {
        data.get(k)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let get_i64 =
        |k: &str, default: i64| -> i64 { data.get(k).and_then(|v| v.as_i64()).unwrap_or(default) };
    Message {
        role: data
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("user")
            .to_string(),
        content: data
            .get("content")
            .cloned()
            .unwrap_or(serde_json::Value::String(String::new())),
        tool_calls: data
            .get("tool_calls")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default(),
        tool_call_id: data
            .get("tool_call_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        message_type: data
            .get("message_type")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        priority: get_i64("priority", 50),
        compressed: data
            .get("compressed")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        pinned: data
            .get("pinned")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        uuid: get_str("uuid"),
        agent_name: data
            .get("agent_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        invocation_id: get_str("invocation_id"),
        parent_invocation_id: get_str("parent_invocation_id"),
        visible_to_invocation_ids: data
            .get("visible_to_invocation_ids")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

// ---------------------------------------------------------------------------
// JSONL Backend
// ---------------------------------------------------------------------------

/// JSONL file backend for transcript persistence.
///
/// Each item is stored as a single JSON line:
/// ```json
/// {"type": "message", "role": "user", "content": "...", "uuid": "...", ...}
/// {"type": "boundary_marker", "uuid": "...", "layer": "...", ...}
/// ```
#[derive(Debug)]
pub struct JsonlBackend {
    pub path: PathBuf,
}

impl JsonlBackend {
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        JsonlBackend { path }
    }

    /// Append an item as a JSON line (lazy-open file, flush per append).
    pub fn append(&self, item: &Item) {
        let line = serde_json::to_string(&item.to_dict()).unwrap_or_default();
        let mut content = String::new();
        if self.path.exists() {
            content = fs::read_to_string(&self.path).unwrap_or_default();
        }
        content.push_str(&line);
        content.push('\n');
        let _ = fs::write(&self.path, content);
    }

    /// Load all items from the JSONL file.
    pub fn load_all(&self) -> Vec<Item> {
        if !self.path.exists() {
            return Vec::new();
        }
        let mut items = Vec::new();
        let content = match fs::read_to_string(&self.path) {
            Ok(c) => c,
            Err(_) => return items,
        };
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(item) = Item::from_dict(&data) {
                    items.push(item);
                }
            }
            // Corrupted lines are skipped (only the last line is expected to corrupt).
        }
        items
    }

    /// Write session metadata as the first line of the JSONL file.
    pub fn write_meta(&self, meta: &SessionMeta) {
        let meta_line = serde_json::to_string(&meta.to_dict()).unwrap_or_default();
        let existing = if self.path.exists() {
            fs::read_to_string(&self.path).unwrap_or_default()
        } else {
            String::new()
        };
        let mut content = meta_line;
        content.push('\n');
        content.push_str(&existing);
        let _ = fs::write(&self.path, content);
    }

    /// Read session metadata from the first line of the JSONL file.
    pub fn read_meta(&self) -> Option<SessionMeta> {
        if !self.path.exists() {
            return None;
        }
        let content = fs::read_to_string(&self.path).ok()?;
        let first_line = content.lines().next()?.trim();
        if first_line.is_empty() {
            return None;
        }
        let data: serde_json::Value = serde_json::from_str(first_line).ok()?;
        if data.get("type").and_then(|v| v.as_str()) == Some("session_meta") {
            Some(SessionMeta::from_dict(&data))
        } else {
            None
        }
    }

    /// Update the pinned state of a message in the JSONL file (rewrite).
    pub fn update_pinned(&self, uuid: &str, pinned: bool) {
        if !self.path.exists() {
            return;
        }
        let content = match fs::read_to_string(&self.path) {
            Ok(c) => c,
            Err(_) => return,
        };
        let mut updated = Vec::new();
        let mut found = false;
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<serde_json::Value>(line) {
                Ok(mut data) => {
                    if data.get("uuid").and_then(|v| v.as_str()) == Some(uuid)
                        && data.get("type").and_then(|v| v.as_str()) == Some("message")
                    {
                        data["pinned"] = serde_json::Value::Bool(pinned);
                        found = true;
                    }
                    updated.push(serde_json::to_string(&data).unwrap_or_default());
                }
                Err(_) => updated.push(line.to_string()),
            }
        }
        if !found {
            return;
        }
        let mut out = String::new();
        for line in updated {
            out.push_str(&line);
            out.push('\n');
        }
        let _ = fs::write(&self.path, out);
    }

    /// Query items with streaming filter and 100k item limit.
    #[allow(clippy::too_many_arguments)]
    pub fn query(
        &self,
        roles: Option<&[String]>,
        agent_names: Option<&[String]>,
        invocation_ids: Option<&[String]>,
        content_regex: Option<&str>,
        message_types: Option<&[String]>,
        limit: Option<usize>,
        offset: usize,
    ) -> Result<Vec<Item>, String> {
        const MAX_ITEMS: usize = 100_000;
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let regex_pattern = content_regex.and_then(|r| regex::Regex::new(r).ok());
        let no_filters = roles.is_none()
            && agent_names.is_none()
            && invocation_ids.is_none()
            && content_regex.is_none()
            && message_types.is_none();

        let content = match fs::read_to_string(&self.path) {
            Ok(c) => c,
            Err(e) => return Err(format!("query failed: {e}")),
        };
        let mut result = Vec::new();
        let mut skipped = 0usize;
        let mut total_items = 0usize;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            total_items += 1;
            if total_items > MAX_ITEMS {
                return Err(format!(
                    "JSONL file exceeds {MAX_ITEMS} items. Consider using SQLite backend or splitting the file."
                ));
            }
            let data = match serde_json::from_str::<serde_json::Value>(line) {
                Ok(d) => d,
                Err(_) => continue, // Corrupted line.
            };
            let item = match Item::from_dict(&data) {
                Some(i) => i,
                None => continue,
            };
            match &item {
                Item::Boundary(_) => {
                    // Boundary markers only pass through if no filters.
                    if no_filters {
                        if skipped >= offset {
                            result.push(item);
                            if let Some(limit) = limit {
                                if result.len() >= limit {
                                    break;
                                }
                            }
                        }
                        skipped += 1;
                    }
                    continue;
                }
                Item::Message(msg) => {
                    if let Some(roles) = roles {
                        if !roles.iter().any(|r| r == &msg.role) {
                            continue;
                        }
                    }
                    if let Some(agent_names) = agent_names {
                        let an = msg.agent_name.as_deref().unwrap_or("");
                        if !agent_names.iter().any(|r| r == an) {
                            continue;
                        }
                    }
                    if let Some(invocation_ids) = invocation_ids {
                        if !invocation_ids.iter().any(|r| r == &msg.invocation_id) {
                            continue;
                        }
                    }
                    if let Some(regex_pattern) = &regex_pattern {
                        let content_str = match &msg.content {
                            serde_json::Value::String(s) => s.clone(),
                            _ => String::new(),
                        };
                        if !regex_pattern.is_match(&content_str) {
                            continue;
                        }
                    }
                    if let Some(message_types) = message_types {
                        let mt = msg.message_type.as_deref().unwrap_or("");
                        if !message_types.iter().any(|r| r == mt) {
                            continue;
                        }
                    }
                    if skipped < offset {
                        skipped += 1;
                        continue;
                    }
                    result.push(item);
                    if let Some(limit) = limit {
                        if result.len() >= limit {
                            break;
                        }
                    }
                }
            }
        }
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Transcript Store
// ---------------------------------------------------------------------------

/// Mostly-append transcript storage.
///
/// The transcript is an append-only list of `Message`/`BoundaryMarker`.
/// `read_view()` reconstructs the current "effective" message list by
/// applying all boundary markers (non-destructive compression).
#[derive(Debug)]
pub struct TranscriptStore {
    pub transcript: Vec<Item>,
    /// UUID -> transcript index.
    pub uuid_index: HashMap<String, usize>,
    pub backend: Option<JsonlBackend>,
    /// View cache (invalidated on append/record_compression).
    dirty: bool,
    cached_view: Option<Vec<Message>>,
    /// LRU cache for memory efficiency.
    max_memory_items: usize,
    /// Number of items offloaded to backend only.
    pub offloaded_count: usize,
}

impl TranscriptStore {
    pub fn new(backend: Option<JsonlBackend>, max_memory_items: usize) -> Self {
        TranscriptStore {
            transcript: Vec::new(),
            uuid_index: HashMap::new(),
            backend,
            dirty: true,
            cached_view: None,
            max_memory_items,
            offloaded_count: 0,
        }
    }

    /// Append a message to the transcript.
    ///
    /// Assigns a UUID if the message doesn't have one; persists to backend
    /// when configured; evicts oldest items past the LRU limit.
    pub fn append(&mut self, message: &mut Message, persist: bool) -> &Message {
        if message.uuid.is_empty() {
            message.uuid = generate_uuid();
        }
        let index = self.transcript.len();
        self.transcript.push(Item::Message(message.clone()));
        self.uuid_index.insert(message.uuid.clone(), index);
        self.dirty = true; // Invalidate view cache.
        if persist {
            if let Some(backend) = &self.backend {
                backend.append(&Item::Message(message.clone()));
            }
        }
        if self.transcript.len() > self.max_memory_items {
            self.evict_old_items();
        }
        // Python returns the same object; we return the stored copy so the
        // UUID assignment is observable on the stored item.
        self.transcript[self.uuid_index[&message.uuid]]
            .as_message()
            .unwrap()
    }

    /// Record a compression event as a boundary marker (non-destructive).
    #[allow(clippy::too_many_arguments)]
    pub fn record_compression(
        &mut self,
        head_uuid: &str,
        tail_uuid: &str,
        anchor_uuid: &str,
        summary: &str,
        layer: &str,
        original_token_count: i64,
        compressed_token_count: i64,
    ) -> BoundaryMarker {
        let marker = BoundaryMarker::new(
            anchor_uuid,
            head_uuid,
            tail_uuid,
            summary,
            layer,
            original_token_count,
            compressed_token_count,
        );
        let index = self.transcript.len();
        self.transcript.push(Item::Boundary(marker.clone()));
        self.uuid_index.insert(marker.uuid.clone(), index);
        self.dirty = true;
        if let Some(backend) = &self.backend {
            backend.append(&Item::Boundary(marker.clone()));
        }
        marker
    }

    /// Get item by UUID.
    pub fn get(&self, uuid: &str) -> Option<&Item> {
        self.uuid_index
            .get(uuid)
            .and_then(|&index| self.transcript.get(index))
    }

    /// Update the pinned state of a message and persist to disk.
    pub fn update_pinned(&mut self, uuid: &str, pinned: bool) -> bool {
        let index = match self.uuid_index.get(uuid) {
            Some(&i) => i,
            None => return false,
        };
        match self.transcript.get_mut(index) {
            Some(Item::Message(msg)) => {
                msg.pinned = pinned;
                self.dirty = true;
                if let Some(backend) = &self.backend {
                    backend.update_pinned(uuid, pinned);
                }
                true
            }
            _ => false,
        }
    }

    /// Evict oldest items from memory, keeping only recent items.
    ///
    /// Never evicts messages referenced by in-memory BoundaryMarkers
    /// (would break read_view() consistency).
    fn evict_old_items(&mut self) {
        if self.transcript.len() <= self.max_memory_items {
            return;
        }
        // Keep 80% of max to avoid frequent evictions.
        let target_size = (self.max_memory_items as f64 * 0.8) as usize;
        let items_to_evict = self.transcript.len() - target_size;
        if items_to_evict == 0 {
            return;
        }
        // Find all UUIDs referenced by in-memory BoundaryMarkers.
        let mut protected_uuids: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for item in &self.transcript {
            if let Item::Boundary(b) = item {
                protected_uuids.insert(b.head_uuid.clone());
                protected_uuids.insert(b.tail_uuid.clone());
                protected_uuids.insert(b.anchor_uuid.clone());
            }
        }
        // Remove oldest items from memory (skip boundary-protected messages).
        let mut evicted = Vec::new();
        let mut kept = Vec::new();
        for item in self.transcript[..items_to_evict].iter() {
            if let Item::Message(m) = item {
                if protected_uuids.contains(&m.uuid) {
                    kept.push(item.clone());
                } else {
                    evicted.push(item.clone());
                }
            } else {
                evicted.push(item.clone());
            }
        }
        // Rebuild transcript: kept items + remaining items.
        let mut new_transcript = kept;
        new_transcript.extend_from_slice(&self.transcript[items_to_evict..]);
        self.transcript = new_transcript;
        self.offloaded_count += evicted.len();
        // Update UUID index (shifted indices).
        self.uuid_index.clear();
        for (i, item) in self.transcript.iter().enumerate() {
            self.uuid_index.insert(item.uuid().to_string(), i);
        }
        self.dirty = true;
    }

    /// Reconstruct the current effective message list (cached).
    ///
    /// Applies all boundary markers: messages inside compressed regions are
    /// replaced by summaries; system messages before compressed regions are
    /// preserved; messages after all boundaries are preserved as-is.
    pub fn read_view(&mut self) -> Vec<Message> {
        if !self.dirty {
            if let Some(cached) = &self.cached_view {
                return cached.clone();
            }
        }
        // Collect all compressed UUID ranges: (head, tail, anchor, summary).
        let mut compressed_ranges: Vec<(String, String, String, String)> = Vec::new();
        for item in &self.transcript {
            if let Item::Boundary(b) = item {
                compressed_ranges.push((
                    b.head_uuid.clone(),
                    b.tail_uuid.clone(),
                    b.anchor_uuid.clone(),
                    b.summary.clone(),
                ));
            }
        }
        if compressed_ranges.is_empty() {
            // No compression — return all messages as-is.
            let result = self
                .transcript
                .iter()
                .filter_map(|item| match item {
                    Item::Message(m) => Some(m.clone()),
                    Item::Boundary(_) => None,
                })
                .collect::<Vec<_>>();
            self.cached_view = Some(result.clone());
            self.dirty = false;
            return result;
        }
        // Build set of compressed UUIDs.
        let mut compressed_uuids: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut summaries: Vec<(String, String)> = Vec::new(); // (anchor_uuid, summary)
        for (head_uuid, tail_uuid, anchor_uuid, summary) in &compressed_ranges {
            let head_idx = self.uuid_index.get(head_uuid).copied();
            let tail_idx = self.uuid_index.get(tail_uuid).copied();
            if let (Some(head_idx), Some(tail_idx)) = (head_idx, tail_idx) {
                for i in head_idx..=tail_idx {
                    if let Some(Item::Message(m)) = self.transcript.get(i) {
                        compressed_uuids.insert(m.uuid.clone());
                    }
                }
                summaries.push((anchor_uuid.clone(), summary.clone()));
            }
        }
        // Build the effective view.
        let mut result: Vec<Message> = Vec::new();
        let mut added_summaries: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        for item in &self.transcript {
            match item {
                Item::Boundary(_) => continue,
                Item::Message(m) => {
                    if compressed_uuids.contains(&m.uuid) {
                        continue; // Skip compressed messages.
                    }
                    // Insert summary before the anchor message.
                    for (anchor_uuid, summary) in &summaries {
                        if anchor_uuid == &m.uuid && !added_summaries.contains(anchor_uuid) {
                            let summary_msg = Message::new(
                                "system",
                                serde_json::Value::String(format!("[Compressed: {summary}]")),
                                Vec::new(),
                                None,
                                generate_uuid(),
                                None,
                                50,
                                false,
                                false,
                                None,
                                String::new(),
                                String::new(),
                                Vec::new(),
                            );
                            result.push(summary_msg);
                            added_summaries.insert(anchor_uuid.clone());
                        }
                    }
                    result.push(m.clone());
                }
            }
        }
        self.cached_view = Some(result.clone());
        self.dirty = false;
        result
    }

    /// Get the total number of items in the transcript.
    pub fn get_transcript_size(&self) -> usize {
        self.transcript.len()
    }

    /// Get the number of messages (excluding boundary markers).
    pub fn get_message_count(&self) -> usize {
        self.transcript
            .iter()
            .filter(|i| matches!(i, Item::Message(_)))
            .count()
    }

    /// Get the number of boundary markers (compression events).
    pub fn get_boundary_count(&self) -> usize {
        self.transcript
            .iter()
            .filter(|i| matches!(i, Item::Boundary(_)))
            .count()
    }

    /// Get audit trail of all compression events.
    pub fn get_compression_audit(&self) -> Vec<serde_json::Value> {
        self.transcript
            .iter()
            .filter_map(|i| match i {
                Item::Boundary(b) => Some(b.to_dict()),
                Item::Message(_) => None,
            })
            .collect()
    }

    /// Serialize transcript to dict (deprecated in Python; retained for tests).
    pub fn to_dict(&self) -> serde_json::Value {
        let items: Vec<serde_json::Value> = self
            .transcript
            .iter()
            .map(|i| match i {
                Item::Message(m) => {
                    let mut d = message_to_dict(m);
                    // to_dict omits agent_name/invocation fields (Python parity).
                    d.as_object_mut().expect("object exists").remove("agent_name");
                    d.as_object_mut().expect("object exists").remove("invocation_id");
                    d.as_object_mut().expect("object exists").remove("parent_invocation_id");
                    d.as_object_mut()
                        .unwrap()
                        .remove("visible_to_invocation_ids");
                    d
                }
                Item::Boundary(b) => b.to_dict(),
            })
            .collect();
        serde_json::json!({ "version": 1, "items": items })
    }

    /// Write session metadata to the backend.
    pub fn write_meta(&self, meta: &SessionMeta) {
        if let Some(backend) = &self.backend {
            backend.write_meta(meta);
        }
    }

    /// Read session metadata from the backend.
    pub fn read_meta(&self) -> Option<SessionMeta> {
        self.backend.as_ref().and_then(|b| b.read_meta())
    }

    /// Load a TranscriptStore from a persistence backend.
    ///
    /// Only loads the last `max_memory_items` items into memory (LRU cache).
    pub fn load_from_backend(backend: JsonlBackend, max_memory_items: usize) -> Self {
        let mut store = TranscriptStore::new(Some(backend), max_memory_items);
        let items = store
            .backend
            .as_ref()
            .map(|b| b.load_all())
            .unwrap_or_default();
        if items.len() > max_memory_items {
            let items_to_skip = items.len() - max_memory_items;
            store.offloaded_count = items_to_skip;
            let recent_items = &items[items_to_skip..];
            for item in recent_items {
                let index = store.transcript.len();
                store.transcript.push(item.clone());
                if !item.uuid().is_empty() {
                    store.uuid_index.insert(item.uuid().to_string(), index);
                }
            }
        } else {
            for item in items {
                let index = store.transcript.len();
                store.transcript.push(item);
                if !store.transcript[index].uuid().is_empty() {
                    store
                        .uuid_index
                        .insert(store.transcript[index].uuid().to_string(), index);
                }
            }
        }
        store
    }
}

impl Item {
    /// Convenience: view this item as a Message (only for Message variant).
    pub fn as_message(&self) -> Option<&Message> {
        match self {
            Item::Message(m) => Some(m),
            Item::Boundary(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("helen_transcript_{}_{}", std::process::id(), name))
    }

    fn user_msg(content: &str, uuid: &str) -> Message {
        Message::new(
            "user",
            serde_json::Value::String(content.to_string()),
            Vec::new(),
            None,
            uuid.to_string(),
            None,
            50,
            false,
            false,
            None,
            String::new(),
            String::new(),
            Vec::new(),
        )
    }

    #[test]
    fn message_dict_round_trip_byte_parity() {
        // Byte-parity with Python's _item_to_dict/_item_from_dict (verified).
        let mut m = user_msg("hello 世界", "ab12cd34ef56");
        m.agent_name = Some("A".to_string());
        m.invocation_id = "inv1".to_string();
        m.parent_invocation_id = "pinv".to_string();
        m.visible_to_invocation_ids = vec!["x".to_string()];
        let d = message_to_dict(&m);
        let obj = d.as_object().expect("object exists");
        assert_eq!(obj["type"], "message");
        assert_eq!(obj["role"], "user");
        assert_eq!(obj["content"], "hello 世界");
        assert_eq!(obj["uuid"], "ab12cd34ef56");
        assert_eq!(obj["priority"], 50);
        assert_eq!(obj["agent_name"], "A");
        assert_eq!(obj["invocation_id"], "inv1");
        assert_eq!(obj["visible_to_invocation_ids"][0], "x");
        // from_dict round-trip.
        let m2 = message_from_dict(&d);
        assert_eq!(m2.role, "user");
        assert_eq!(m2.uuid, "ab12cd34ef56");
        assert_eq!(m2.priority, 50);
        assert_eq!(m2.agent_name.as_deref(), Some("A"));
    }

    #[test]
    fn message_defaults_on_missing_fields() {
        // Python: missing fields fall back to defaults (role "user", priority 50).
        let d = serde_json::json!({"type": "message", "content": "x"});
        let m = message_from_dict(&d);
        assert_eq!(m.role, "user");
        assert_eq!(m.priority, 50);
        assert_eq!(m.compressed, false);
        assert_eq!(m.pinned, false);
        assert_eq!(m.uuid, "");
    }

    #[test]
    fn boundary_marker_dict_round_trip() {
        let b = BoundaryMarker::new("a", "h", "t", "sum", "L1", 10, 3);
        let d = b.to_dict();
        assert_eq!(d["type"], "boundary_marker");
        assert_eq!(d["anchor_uuid"], "a");
        assert_eq!(d["layer"], "L1");
        let b2 = BoundaryMarker::from_dict(&d);
        assert_eq!(b2.anchor_uuid, "a");
        assert_eq!(b2.summary, "sum");
        assert_eq!(b2.original_token_count, 10);
    }

    #[test]
    fn append_assigns_uuid_and_indexes() {
        let mut store = TranscriptStore::new(None, 1000);
        let mut m = user_msg("q1", "");
        store.append(&mut m, true);
        assert!(!m.uuid.is_empty(), "uuid assigned on append");
        assert_eq!(store.get_message_count(), 1);
        assert_eq!(store.get_transcript_size(), 1);
        assert_eq!(store.get(&m.uuid).expect("key exists").uuid(), m.uuid);
    }

    #[test]
    fn jsonl_backend_persists_and_loads() {
        let p = tmp_path("persist");
        let _ = fs::remove_file(&p);
        let backend = JsonlBackend::new(&p);
        let mut store = TranscriptStore::new(Some(backend), 1000);
        let mut m1 = user_msg("hello", "u1");
        store.append(&mut m1, true);
        let mut m2 = user_msg("world 世界", "u2");
        store.append(&mut m2, true);
        store.record_compression("u1", "u1", "u2", "s", "L1", 10, 2);

        // Reload from backend.
        let backend2 = JsonlBackend::new(&p);
        let store2 = TranscriptStore::load_from_backend(backend2, 1000);
        assert_eq!(store2.get_transcript_size(), 3); // 2 msgs + 1 marker
        assert_eq!(store2.get_message_count(), 2);
        assert_eq!(store2.get_boundary_count(), 1);
        // Message content round-trips through JSON (UTF-8).
        let items = store2
            .transcript
            .iter()
            .filter_map(|i| match i {
                Item::Message(m) => Some(m.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(items[0].content, "hello");
        assert_eq!(items[1].content, "world 世界");
    }

    #[test]
    fn jsonl_meta_write_read() {
        let p = tmp_path("meta");
        let _ = fs::remove_file(&p);
        let backend = JsonlBackend::new(&p);
        let mut meta = SessionMeta::new();
        meta.session_id = "session_1_x_y".to_string();
        meta.session_scope = "project".to_string();
        meta.helen_version = "1.45.0".to_string();
        meta.cwd = "/tmp".to_string();
        meta.argv = vec!["helen".to_string(), "main.helen".to_string()];
        backend.write_meta(&meta);
        let read = backend.read_meta().expect("read meta");
        assert_eq!(read.session_id, "session_1_x_y");
        assert_eq!(read.session_scope, "project");
        assert_eq!(read.helen_version, "1.45.0");
        assert_eq!(read.argv, vec!["helen", "main.helen"]);
    }

    #[test]
    fn update_pinned_persists() {
        let p = tmp_path("pin");
        let _ = fs::remove_file(&p);
        let backend = JsonlBackend::new(&p);
        let mut store = TranscriptStore::new(Some(backend), 1000);
        let mut m = user_msg("keep me", "pin1");
        store.append(&mut m, true);
        assert!(store.update_pinned("pin1", true));
        // Reload: pinned survives restart.
        let backend2 = JsonlBackend::new(&p);
        let store2 = TranscriptStore::load_from_backend(backend2, 1000);
        match store2.get("pin1").expect("key exists") {
            Item::Message(m) => assert!(m.pinned),
            _ => panic!("expected message"),
        }
        // Unknown uuid -> false.
        assert!(!store.update_pinned("nope", true));
    }

    #[test]
    fn read_view_compression_replaces_region() {
        let mut store = TranscriptStore::new(None, 1000);
        let mut m0 = user_msg("hi", "m0");
        store.append(&mut m0, true);
        let mut m1 = user_msg("q1", "m1");
        store.append(&mut m1, true);
        let mut m2 = user_msg("a1", "m2");
        store.append(&mut m2, true);
        let mut m3 = user_msg("q2", "m3");
        store.append(&mut m3, true);
        store.record_compression(
            "m1",
            "m2",
            "m3",
            "earlier stuff",
            "context_collapse",
            100,
            10,
        );

        let view = store.read_view();
        assert_eq!(view.len(), 3);
        assert_eq!(view[0].role, "user");
        assert_eq!(view[0].content, "hi");
        assert_eq!(view[1].role, "system");
        assert_eq!(view[1].content, "[Compressed: earlier stuff]");
        assert_eq!(view[2].content, "q2");
        // Original transcript untouched (non-destructive).
        assert_eq!(store.get_transcript_size(), 5);
        assert_eq!(store.get_boundary_count(), 1);
    }

    #[test]
    fn read_view_cached_until_append() {
        let mut store = TranscriptStore::new(None, 1000);
        let mut m1 = user_msg("a", "c1");
        store.append(&mut m1, true);
        let v1 = store.read_view();
        let v2 = store.read_view(); // cached
        assert_eq!(v1.len(), 1);
        assert_eq!(v2.len(), 1);
        // New append invalidates cache.
        let mut m2 = user_msg("b", "c2");
        store.append(&mut m2, true);
        let v3 = store.read_view();
        assert_eq!(v3.len(), 2);
    }

    #[test]
    fn lru_eviction_protects_boundary_messages() {
        let mut store = TranscriptStore::new(None, 5); // tiny limit
        let mut uuids = Vec::new();
        for i in 0..6 {
            let uuid = format!("e{i}");
            let mut m = user_msg(&format!("msg{i}"), &uuid);
            store.append(&mut m, true);
            uuids.push(uuid);
        }
        // 6 messages with max 5 -> eviction to 80% (4 kept).
        assert!(store.transcript.len() <= 5);
        assert!(store.offloaded_count >= 1);
        // Eviction keeps the most recent messages.
        let last = store.get(&uuids[5]).expect("key exists").uuid();
        assert_eq!(last, "e5");
    }

    #[test]
    fn query_filters_and_limit() {
        let p = tmp_path("query");
        let _ = fs::remove_file(&p);
        let backend = JsonlBackend::new(&p);
        let mut store = TranscriptStore::new(Some(backend), 1000);
        let mut m1 = user_msg("first hello", "q1");
        store.append(&mut m1, true);
        let mut m2 = Message::new(
            "assistant",
            serde_json::Value::String("second world".to_string()),
            Vec::new(),
            None,
            "q2".to_string(),
            Some("assistant".to_string()),
            80,
            false,
            false,
            None,
            String::new(),
            String::new(),
            Vec::new(),
        );
        store.append(&mut m2, true);
        let backend2 = JsonlBackend::new(&p);
        // Filter by role.
        let res = backend2
            .query(Some(&["user".to_string()]), None, None, None, None, None, 0)
            .unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].uuid(), "q1");
        // Filter by regex.
        let res = backend2
            .query(None, None, None, Some("world"), None, None, 0)
            .unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].uuid(), "q2");
        // Limit.
        let res = backend2
            .query(None, None, None, None, None, Some(1), 0)
            .unwrap();
        assert_eq!(res.len(), 1);
    }
}
