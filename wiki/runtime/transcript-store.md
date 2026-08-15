<!-- helen-rust edition: crates/helen-runtime transcript (M8) — SQLite backend byte-compatible with Python; JSONL interop verified both directions (D8). -->

# TranscriptStore SSOT

> **v1.16 New Feature** — Single Source of Truth for messages

TranscriptStore is a core runtime component introduced in Helen v1.16. It persists all conversation messages, providing complete audit trails and session recovery capabilities.

---

## 📋 Table of Contents

- [Design Goals](#design-goals)
- [Architecture Overview](#architecture-overview)
- [Core Components](#core-components)
- [Usage Guide](#usage-guide)
- [Configuration Details](#configuration-details)
- [Performance Optimization](#performance-optimization)
- [Best Practices](#best-practices)

---

## Design Goals

### Why TranscriptStore?

Before v1.16, Helen's conversation history management had the following problems:

1. **Dual-write divergence**: `_history` and `TranscriptStore` were dual-written with inconsistent semantics
2. **Destructive compression**: Compression replaced in-place, losing the complete conversation history
3. **No persistence**: All messages stayed in memory, causing memory pressure in long sessions
4. **Debugging difficulty**: Impossible to trace back "what the LLM saw vs what the original conversation was"

### TranscriptStore's Solution

| Problem | Solution |
|------|---------|
| Dual-write divergence | **Single write point**: All messages written only to TranscriptStore |
| Destructive compression | **Non-destructive**: Compression only appends BoundaryMarker, messages are not modified |
| No persistence | **Persistence-first**: JSONL/SQLite backends, memory retains only the active window |
| Debugging difficulty | **Full audit**: Any historical view can be reconstructed |

---

## Architecture Overview

### Four-Layer Architecture

```
┌─────────────────────────────────────────┐
│  Layer 4: User Interface                 │
│  • REPL commands (:transcript, :sessions)│
│  • Stdlib functions (get_session_id, etc)│
└─────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────┐
│  Layer 3: View Layer                     │
│  • read_view() reconstructs compressed  │
│    view                                 │
│  • View Cache (dirty flag + cache)      │
│  • UUID index (O(1) lookup)             │
└─────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────┐
│  Layer 2: Storage Layer                  │
│  • TranscriptStore (in-memory)          │
│  • LRU Cache (max_memory_items)         │
│  • BoundaryMarker (compression records) │
└─────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────┐
│  Layer 1: Persistence Layer              │
│  • JSONLBackend (append-only)           │
│  • SQLiteBackend (WAL mode, indexed)    │
└─────────────────────────────────────────┘
```

### Data Flow

```
User Input
    ↓
_add_to_history()
    ↓
TranscriptStore.append()  ← SSOT (sole write point)
    ↓
Backend.append()  ← Persistence
    ↓
LRU Eviction (if needed)  ← Memory optimization

LLM Call
    ↓
_prepare_history_for_llm()
    ↓
TranscriptStore.read_view()  ← Applies BoundaryMarker
    ↓
View Cache (O(1) if no changes)
    ↓
LLM API Call
```

---

## Core Components

### 1. TranscriptStore

**Location**: `helen/runtime/transcript_store.py`

TranscriptStore is the Single Source of Truth (SSOT) for messages, responsible for:

- **Message storage**: Append-only list, never modified/deleted
- **UUID indexing**: O(1) lookup via `get(uuid)`
- **View reconstruction**: `read_view()` applies BoundaryMarkers
- **Compression recording**: `record_compression()` appends BoundaryMarker
- **LRU caching**: Automatically evicts old messages to backend

**Key attributes**:

```python
class TranscriptStore:
    transcript: list[Message | BoundaryMarker]  # append-only
    _uuid_index: dict[str, int]                 # UUID → index
    _backend: TranscriptStoreBackend            # Persistence backend
    _max_memory_items: int                      # LRU cache size
    _offloaded_count: int                       # Count of items evicted to backend
    _dirty: bool                                # View cache invalidation flag
    _cached_view: list[Message] | None          # Cached view
```

**Core methods**:

| Method | Description | Time Complexity |
|------|------|-----------|
| `append(msg)` | Append message, assign UUID | O(1) |
| `get(uuid)` | UUID lookup | O(1) |
| `read_view()` | Reconstruct compressed view | O(n) first time, O(1) cached |
| `record_compression(...)` | Record compression event | O(1) |
| `get_compression_audit()` | Get compression audit | O(b), b=number of boundaries |

### 2. BoundaryMarker

**Location**: `helen/runtime/transcript_store.py`

BoundaryMarker records compression events without modifying original messages:

```python
@dataclass
class BoundaryMarker:
    uuid: str                          # Boundary marker UUID
    anchor_uuid: str                   # Anchor message UUID (first message after compression)
    head_uuid: str                     # Compression range start UUID
    tail_uuid: str                     # Compression range end UUID
    summary: str                       # Compression summary
    layer: str                         # Compression layer name
    timestamp: float                   # Compression timestamp
    original_token_count: int          # Token count before compression
    compressed_token_count: int        # Token count after compression
```

**How it works**:

```
Original messages: [msg1] [msg2] [msg3] [msg4] [msg5]
                ↓ Compress msg1-msg3
Transcript: [msg1] [msg2] [msg3] [msg4] [msg5] [BoundaryMarker]
                                                  ↓
read_view(): [summary] [msg4] [msg5]
```

### 3. Backend (Persistence Backend)

#### JSONLBackend

**Features**:
- ✅ Simple, human-readable (one JSON per line)
- ✅ Crash-safe (append-only)
- ✅ Easy to debug with tail/grep
- ⚠️ No index, slower queries with large message counts

**Format**:
```json
{"type": "message", "role": "user", "content": "Hello", "uuid": "abc123", ...}
{"type": "boundary_marker", "uuid": "marker1", "layer": "auto_compact", ...}
```

#### SQLiteBackend

**Features**:
- ✅ WAL mode, high-performance writes (<1ms/message)
- ✅ UUID indexed, O(1) lookup
- ✅ Transaction-safe, supports concurrent reads
- ⚠️ Binary format, not easy to inspect directly

**Schema**:
```sql
CREATE TABLE transcript (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid TEXT UNIQUE NOT NULL,
    type TEXT NOT NULL,  -- 'message' or 'boundary_marker'
    data TEXT NOT NULL,  -- JSON
    timestamp REAL NOT NULL
);
CREATE INDEX idx_uuid ON transcript(uuid);
CREATE INDEX idx_timestamp ON transcript(timestamp);
```

### 4. SessionManager

**Location**: `helen/runtime/session_manager.py`

SessionManager manages session lifecycle:

```python
class SessionManager:
    def create_session(self) -> str:
        """Create new session, return session_id"""
        
    def get_session_path(self, session_id: str) -> Path:
        """Get session transcript file path"""
        
    def list_sessions(self) -> list[dict]:
        """List all sessions (sorted by modification time)"""
        
    def delete_session(self, session_id: str) -> bool:
        """Delete session"""
        
    def cleanup_old_sessions(self, keep_count: int = 100) -> int:
        """Clean up old sessions, keep most recent N"""
```

**Session directory structure**:
```
~/.helen/sessions/
├── session_1783492628_d9d9c0aa/
│   └── transcript.jsonl  (or transcript.db)
├── session_1783492600_abc12345/
│   └── transcript.jsonl
└── ...
```

### 4.5 Session Scope (v1.20)

Before v1.20, all transcripts were stored in `~/.helen/sessions/` (global). v1.20 introduces the concept of **scope**: transcripts can be isolated per application in their respective project directories.

#### Scope Modes

| Mode | Path | Use Case |
|------|------|----------|
| `global` | `~/.helen/sessions/` | REPL exploration, cross-project sharing, short scripts |
| `project` | `<project>/.helen/sessions/` | Long-lived applications, production deployments, containerized |
| `auto` (default) | Detects project directory — project if found, global otherwise | Recommended default |

#### Project Detection

Project root is detected by walking upward looking for one of these markers:
- `.helen/` (directory) — but excludes the user-global `~/.helen/`
- `helen.yaml` / `helen.yml` / `helen.toml`

#### Priority

1. **`HELEN_SESSION_DIR` environment variable**: Absolute priority, forces the specified path
2. **`session_scope` config**: `auto` (default) / `global` / `project`
3. **Fallback to `~/.helen/sessions/`**

#### Configuration

```yaml
# ~/.helen/config.yaml
transcript:
  enabled: true
  backend: "sqlite"
  session_scope: "auto"          # "auto" | "global" | "project"
  session_dir: "~/.helen/sessions"             # Only when scope=global
  project_session_dir: ".helen/sessions"       # Only when scope=project
  max_memory_items: 1000
```

#### Runtime Query and Modification

```helen
// View current session directory
let info = get_session_dir()
print("Path: " + info["session_dir"])
print("Scope: " + info["scope"])
print("Project root: " + str(info["project_dir"]))

// Switch at runtime (does not modify config.yaml, current process only)
set_session_dir("./my_app_sessions")
```

#### Design Principle

**Transcripts are application data, not language infrastructure**. Let transcripts follow the application rather than the language installation:
- Application is directory — `rm -rf .helen/` cleans all state
- When copying/moving application directory, transcripts move with it
- Containerized scenarios: `WORKDIR` already contains transcripts, no need to mount `~/.helen`
- Multi-application machines: each application's transcripts are naturally isolated

Interactive scenarios like REPL explicitly use `session_scope: "global"` to maintain cross-project history continuity.

### 4.6 Session Deletion (v1.21)

v1.21 adds three stdlib functions for permanently deleting TranscriptStore session data.

#### Design Principle

Helen adopts an **audit-trail-first** deletion strategy:

| Operation Type | Function | Deletes Persisted Data |
|---------|------|:----------------:|
| Logical delete (message-level) | `delete_message(uuid)` | ❌ Preserved |
| Logical clear (session-level) | `clear_context()` | ❌ Preserved (adds BoundaryMarker) |
| Permanent delete (session-level) | `delete_session(id)` | ✅ Deleted |
| Permanent delete (current session) | `delete_current_session()` | ✅ Deleted |
| Batch cleanup | `cleanup_sessions()` | ✅ Deleted |

Logical deletion preserves persisted data for audit; permanent deletion actually frees disk space.

#### delete_session(session_id)

Permanently deletes all data for the specified session:

```helen
let r = delete_session("session_1720435200_a1b2c3d4")
// {"status": "ok", "session_id": "...", "freed_bytes": 10240, "message": "..."}
```

**Safety restriction**: Cannot delete the current session; use `delete_current_session()` instead.

#### delete_current_session(confirm?)

Permanently deletes the current session, requires explicit confirmation:

```helen
// Step 1: See confirmation prompt
let r = delete_current_session()
// {"status": "error", "message": "Set confirm=true to delete current session"}

// Step 2: Confirm deletion
let r = delete_current_session(confirm=true)
// {"status": "ok", "session_id": "...", "freed_bytes": 8192}
```

After deletion, the interpreter continues running, but the current TranscriptStore is cleared and subsequent messages are written to a new session.

#### cleanup_sessions(keep_count?, older_than_days?)

Batch cleanup of old sessions, supports two strategies:

```helen
// Strategy 1: Keep most recent N sessions
let r = cleanup_sessions(keep_count=50)
// {"status": "ok", "deleted_count": 15, "freed_bytes": 1536000}

// Strategy 2: Delete sessions older than N days
let r = cleanup_sessions(older_than_days=30)

// Strategy 3: Combined (deletes only when both conditions are met)
let r = cleanup_sessions(keep_count=50, older_than_days=30)
```

**Safety restriction**: The current session is never cleaned up, even if it falls outside the retention range.

#### Use Cases

| Scenario | Recommended Function |
|------|---------|
| Periodic cleanup in long-running Agents | `cleanup_sessions(keep_count=100)` |
| Privacy compliance (GDPR right to be forgotten) | `delete_session(user_session_id)` |
| Test environment cleanup | `cleanup_sessions(keep_count=0)` |
| Reset current session | `delete_current_session(confirm=true)` |

#### Chinese Aliases

| English | Chinese |
|------|------|
| `delete_session` | `删除会话` |
| `delete_current_session` | `删除当前会话` |
| `cleanup_sessions` | `清理会话` |

### 4.7 Session Metadata (v1.23.3)

The first line of each new transcript file automatically writes a **`session_meta`** record, capturing startup context information.

#### Record Format

First line of JSONL file:

```json
{
  "type": "session_meta",
  "argv": ["helen", "my_app.helen", "--mode", "test"],
  "timestamp": 1720435200.123456,
  "helen_version": "1.23.3",
  "python_version": "3.12.13",
  "platform": "linux-aarch64",
  "cwd": "/home/user/project",
  "session_id": "session_1720435200_a1b2c3d4",
  "session_scope": "project"
}
```

#### Field Descriptions

| Field | Description |
|------|------|
| `argv` | Program name + all invocation arguments |
| `timestamp` | Startup time (Unix epoch, microsecond precision) |
| `helen_version` | Helen version |
| `python_version` | Python interpreter version |
| `platform` | OS/architecture (e.g., `linux-aarch64`) |
| `cwd` | Working directory at startup |
| `session_id` | Session ID (matches directory name) |
| `session_scope` | `global` / `project` / `custom` |

#### Design Goals

- **Session identification**: `cat transcript.jsonl | head -1` instantly tells which program produced this
- **Audit trail**: Complete record of startup parameters for issue reproduction
- **Debugging convenience**: Knows when the program started, with what parameters, in what environment
- **Version tracking**: Records Helen and Python versions for compatibility issue investigation

#### Runtime Query

```helen
let meta = get_session_meta()
if meta["status"] == "ok" {
    let data = meta["data"]
    print("Startup command: " + str(data["argv"]))
    print("Helen version: " + data["helen_version"])
    print("Working directory: " + data["cwd"])
}

// Chinese alias
let meta_zh = 获取会话元数据()
```

#### Backward Compatibility

- **Old transcript files** (without meta line): `get_session_meta()` returns `{"status": "error"}`
- **New code reading old transcript**: `read_meta()` returns `None`, callers handle gracefully
- **Old code reading new transcript**: Skips `type == "session_meta"` lines (does not affect message list)

#### JSONL vs SQLite

- **JSONL**: Meta as first line of file, `load_all()` skips automatically
- **SQLite**: `session_meta` single-row table, `load_all()` queries from `messages` table (automatically isolated)

### 5. LRU Cache

**How it works**:

```python
# Check when appending messages
if len(self.transcript) > self._max_memory_items:
    self._evict_old_items()

def _evict_old_items(self):
    # Keep 80% to avoid frequent eviction
    target_size = int(self._max_memory_items * 0.8)
    items_to_evict = len(self.transcript) - target_size
    
    # Evict oldest messages (already in backend)
    evicted = self.transcript[:items_to_evict]
    self.transcript = self.transcript[items_to_evict:]
    self._offloaded_count += len(evicted)
    
    # Update UUID index
    self._uuid_index.clear()
    for i, item in enumerate(self.transcript):
        self._uuid_index[item.uuid] = i
```

**Memory usage**:

| Scenario | Memory Usage | Notes |
|------|---------|------|
| 100 messages | ~1MB | All in memory |
| 1K messages | ~10MB | All in memory |
| 10K messages | ~10MB | LRU cache active |
| 100K messages | ~50MB | LRU cache active |

---

## Usage Guide

### REPL Commands

#### :transcript

Shows the current session's effective view (after compression applied):

```bash
> :transcript
Current transcript view (15 messages):
  [1] [user] Hello
  [2] [assistant] Hi there
  ...

Stats: 20 total items, 15 messages, 5 compression boundaries
```

**Options**:
- `:transcript --full` — Show full transcript (including compressed messages)
- `:transcript --audit` — Show compression audit trail

#### :sessions

List all sessions:

```bash
> :sessions
Transcript sessions (5 total):
  [1] session_1783492628_d9d9c0aa
       Modified: 2026-07-08 15:30:00, Size: 2.5 KB, Messages: ~50
  [2] session_1783492600_abc12345
       Modified: 2026-07-08 14:00:00, Size: 1.2 KB, Messages: ~20
```

#### :session_id

Show current session ID:

```bash
> :session_id
Current session: session_1783492628_d9d9c0aa
```

### Stdlib Functions

#### get_session_id()

```helen
let session = get_session_id()
print("Current session: {session}")
```

#### list_sessions()

```helen
let sessions = list_sessions()
for s in sessions {
    print("{s.session_id}: {s.size_bytes} bytes")
}
```

#### replay_transcript()

```helen
// Replay current session
let messages = replay_transcript()
for msg in messages {
    print("[{msg.role}] {msg.content}")
}

// Replay specified session, including compressed messages
let full = replay_transcript("session_1783492628_d9d9c0aa", true)
```

#### export_transcript()

```helen
// Export as JSON
export_transcript("my_chat.json", "json")

// Export as Markdown
export_transcript("my_chat.md", "markdown")

// Export as plain text
export_transcript("my_chat.txt", "text")
```

#### search_transcript() (v1.22+)

Searches persistent transcript by **content**. Unlike `search_context()` (which only searches the current active context), `search_transcript()` can search across sessions and agents.

```helen
// Search within current session
let matches = search_transcript("authentication bug")

// Search across all sessions (cross-session discovery)
let matches = search_transcript("database schema", scope="all")

// Regex matching
let matches = search_transcript("fix.*bug", regex=true)

// Search only user messages
let matches = search_transcript("TODO", role="user")

// Limit results
let matches = search_transcript("TODO", limit=20)

// Chinese alias
let matches = 搜索会话("authentication bug")
```

**Return format**:

```helen
// Each match contains:
{
    session_id: "session_xxx",
    message_uuid: "uuid-...",
    role: "user",
    content: "full message content",
    snippet: "...fragment around the match...",
    match_position: 42,
}
```

**Parameters**:

| Parameter | Type | Default | Description |
|---|---|---|---|
| `query` | str | (required) | Search content, substring or regex |
| `session_id` | str? | null | Specify session (ignored when `scope="all"`) |
| `scope` | str | `"current"` | `"current"` / `"all"` / `"global"` / `"project"` |
| `role` | str | `""` | Filter by role: `"user"` / `"assistant"` / `"tool"` / `""` (all) |
| `regex` | bool | `false` | Whether to use regex matching |
| `limit` | int | `50` | Maximum results to return |

**Typical usage**:

```helen
// Scenario 1: Find a discussion across sessions
let matches = search_transcript("database schema", scope="all", limit=5)
for m in matches {
    print("Session {m.session_id}: {m.snippet}")
}

// Scenario 2: Restore full context after finding it
if len(matches) > 0 {
    restore_context(matches[0].session_id)
}
```

#### Invocation Tree (v1.22+)

Each message carries three new fields forming the invocation tree:
- `agent_name`: The agent name that produced this message (`None` for top-level)
- `invocation_id`: Unique ID for this `main {}` execution
- `parent_invocation_id`: The parent invocation's invocation_id

**Query functions**:

```helen
// List all invocations (filterable by agent, paginated)
let invs = list_invocations()
// [{invocation_id, agent_name, parent_invocation_id, message_count, ...}, ...]

let a_runs = list_invocations(agent="Researcher", limit=10)

// Get single invocation metadata
let info = get_invocation("inv_1784272795_a61bcdaf")
// {agent_name: "A", message_count: 4, parent_invocation_id: "inv_top", ...}

// Get complete invocation tree (nested structure)
let tree = get_invocation_tree()
// {
//   invocation_id: "inv_top", agent_name: null, children: [
//     {invocation_id: "inv_1", agent_name: "A", children: [...]},
//     {invocation_id: "inv_2", agent_name: "B", children: []},
//   ]
// }

// Invocation path string (for debugging)
print(invocation_path("inv_3"))
// "top -> A -> C"

// Chinese aliases
列出调用()
获取调用("inv_xxx")
获取调用树()
调用路径("inv_xxx")
```

**Extended `replay_transcript` filtering**:

```helen
// Only see agent A's messages
let a_msgs = replay_transcript(agent="A")

// Only see A's last run
let last_run = replay_transcript(agent="A", last_only=true)

// See a specific invocation and its sub-calls
let subtree = replay_transcript(invocation_id="inv_1", include_subtree=true)
```

**Extended `restore_context` filtering**:

```helen
// Restore only agent A's most recent run to active context
restore_context("session_xxx", agent="A", last_only=true)

// Restore a specific invocation and its subtree
restore_context("session_xxx", invocation_id="inv_1", include_subtree=true)
```

**Isolation semantics**: Active context is filtered by `invocation_id`, each agent `main {}` call is fresh. See [[runtime/context-management|Context Management Architecture §0.5]].

#### get_compression_audit()

```helen
let audit = get_compression_audit()
for event in audit {
    print("{event.layer}: {event.original_token_count} -> {event.compressed_token_count}")
}
```

---

## Configuration Details

### Basic Configuration

Edit `~/.helen/config.yaml`:

```yaml
transcript:
  enabled: true              # Enable TranscriptStore (default: true)
  backend: "sqlite"          # Backend type: "jsonl" or "sqlite"
  session_dir: "~/.helen/sessions"  # Session storage directory
  max_memory_items: 1000     # LRU cache size (default: 1000)
```

### Backend Selection Guide

| Scenario | Recommended Backend | Reason |
|------|---------|------|
| Development/debugging | JSONL | Human-readable, easy to tail/grep |
| Production | SQLite | High performance, index-optimized |
| Long sessions (>10K) | SQLite | WAL mode, memory-efficient |
| Quick prototyping | JSONL | No extra dependencies |

### Memory Optimization

For long sessions, adjust the LRU cache:

```yaml
transcript:
  max_memory_items: 500      # Reduce memory footprint
```

**Memory usage formula**:
```
Memory ≈ max_memory_items × average message size
       ≈ 1000 × 10KB
       ≈ 10MB
```

---

## Performance Optimization

### Write Performance

| Backend | Latency | Throughput | Notes |
|------|------|--------|------|
| JSONL | <1ms | 1000 msg/s | Append-only, fast |
| SQLite WAL | <1ms | 2000 msg/s | Batch commits, concurrent reads |

### Read Performance

| Operation | Time Complexity | Notes |
|------|-----------|------|
| `get(uuid)` | O(1) | UUID index |
| `read_view()` | O(1) | View cache |
| `read_view()` (first time) | O(n) | Reconstruct view |

### Memory Optimization

- **LRU eviction**: Automatically evicts old messages to backend
- **View caching**: Dirty flag + cache, avoids redundant computation
- **On-demand loading**: Only loads most recent messages when loading from backend

---

## Best Practices

### 1. Use SQLite in Production

```yaml
transcript:
  backend: "sqlite"
  max_memory_items: 1000
```

**Advantages**:
- High-performance writes (WAL mode)
- UUID indexing (O(1) lookup)
- Transaction-safe

### 2. Periodically Clean Up Old Sessions

```helen
// In long-running applications
let sessions = list_sessions()
if len(sessions) > 100 {
    // TODO: Add cleanup_sessions() stdlib function
    // Or use SessionManager.cleanup_old_sessions()
}
```

### 3. Export Important Sessions

```helen
// Export when session ends
export_transcript("important_session.json", "json")
```

### 4. Monitor Compression Efficiency

```helen
let audit = get_compression_audit()
let total_saved = 0
for event in audit {
    total_saved += event.original_token_count - event.compressed_token_count
}
print("Total tokens saved: {total_saved}")
```

### 5. Use JSONL When Debugging

```yaml
transcript:
  backend: "jsonl"  # Easy to tail -f and grep
```

```bash
# Watch transcript in real-time
tail -f ~/.helen/sessions/*/transcript.jsonl

# Search for specific messages
grep "error" ~/.helen/sessions/*/transcript.jsonl
```

---

## Troubleshooting

### TranscriptStore Not Enabled

**Check configuration**:
```yaml
# ~/.helen/config.yaml
transcript:
  enabled: true  # Ensure this is true
```

### Session File Not Created

**Check permissions**:
```bash
ls -la ~/.helen/sessions/
```

Ensure Helen has write permissions.

### Excessive Memory Usage

**Reduce LRU cache**:
```yaml
transcript:
  max_memory_items: 500  # Reduce to 500
```

### Session Recovery Failed

**Check if session exists**:
```bash
ls ~/.helen/sessions/<session_id>/
```

Ensure the transcript file is complete.

---

## Technical Details

### Compression Flow

```
Compression Trigger (usage > threshold)
    ↓
graduated_compress() / traditional_compress()
    ↓
Return compressed list
    ↓
TranscriptStore.record_compression(
    head_uuid=compressed_msgs[0].uuid,
    tail_uuid=compressed_msgs[-1].uuid,
    anchor_uuid=anchor.uuid,
    summary=summary_text,
    layer=layer_name,
    original_token_count=original_tokens,
    compressed_token_count=compressed_tokens,
)
    ↓
Append BoundaryMarker to transcript
    ↓
Invalidate view cache (_dirty = True)
```

### View Reconstruction Algorithm

```python
def read_view(self) -> list[Message]:
    # 1. Collect all BoundaryMarkers
    compressed_ranges = []
    for item in self.transcript:
        if isinstance(item, BoundaryMarker):
            compressed_ranges.append((
                item.head_uuid, item.tail_uuid, 
                item.anchor_uuid, item.summary
            ))
    
    # 2. Build compressed UUID set
    compressed_uuids = set()
    for head, tail, anchor, summary in compressed_ranges:
        for i in range(head_idx, tail_idx + 1):
            compressed_uuids.add(self.transcript[i].uuid)
    
    # 3. Reconstruct view (skip compressed messages, insert summaries)
    result = []
    for item in self.transcript:
        if isinstance(item, Message):
            if item.uuid not in compressed_uuids:
                result.append(item)
            elif item.uuid == anchor:
                result.append(Message(role="system", content=summary))
    
    return result
```

### UUID Generation

```python
def _generate_uuid() -> str:
    """Generate 12-character hex UUID (16^12 ≈ 2.8×10^14)"""
    return uuid4().hex[:12]
```

**Collision probability**:
- 1M messages: ~0.0000002% (very low)
- 1B messages: ~0.2% (still acceptable)

---

## Related Documentation

- [[runtime/context-management|Context Management Architecture]] — Unified compression entry point
- [[runtime/history|History Management]] — Token budget, truncation strategies
- [[toolchain/stdlib|Standard Library]] — Transcript functions
- [[toolchain/cli|Command Line Tools]] — REPL commands

---

**Last Updated**: 2026-07-08 | **Version**: v1.16 | **Status**: ✅ Production-ready
