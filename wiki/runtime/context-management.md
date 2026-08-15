<!-- helen-rust edition: crates/helen-runtime context (M8). -->

# Context Management Architecture

> **Version**: v1.23 | **Last Updated**: 2026-07-18
> Unified description of Helen's context management system, replacing the scattered descriptions in `agent_context.md`, `graduated_compression.md`, `cache_aware_compression.md`, and `working_memory.md`.

---

## 0. Design Philosophy and Lifecycle

> Before diving into implementation details, let's clarify the essential question of context management: **How long should Context exist?**

### 0.1 Core Distinction: Context vs Transcript

Helen has two seemingly overlapping but actually distinct systems:

| | **Context** | **Transcript** |
|---|---|---|
| Nature | Information the LLM **currently sees** | Complete record of what the LLM **has ever said** |
| Purpose | Support reasoning | Audit, recovery, tracing |
| Mutability | Compressed, trimmed, replaced | Append-only, immutable ([[runtime/transcript-store|TranscriptStore SSOT]]) |
| Lifecycle | Session-level, destroyable | Persistent (SQLite/JSONL) |
| Analogy | Workbench — what's currently in use | Filing cabinet — all history is here |

**Design Principle**: Context management strives for **maximizing information quality within a limited window**; Transcript strives for **completeness without loss**. The separation allows the system to be both efficient (context can be aggressively compressed without concern) and safe (transcript is complete and auditable).

> Context is "what the LLM should know right now"; Transcript is "what the LLM has ever known."

### 0.2 Four-Layer Lifecycle Architecture

Context duration is layered, with each layer serving a different reasoning granularity:

```
┌─────────────────────────────────────────────────────────────┐
│  Layer 3: Transcript (Audit Layer)                          │
│  Lifecycle: Permanent                                       │
│  Responsibility: Does not participate in LLM reasoning,     │
│    but can be restored via replay_transcript()              │
│  Implementation: [[runtime/transcript-store|TranscriptStore SSOT]] │
├─────────────────────────────────────────────────────────────┤
│  Layer 2: Pinned Context (Persistent Focus Layer)           │
│  Lifecycle: Across llm act calls, within Agent session      │
│  Responsibility: User-explicitly pinned critical info,      │
│    immune to all 5 compression layers                       │
│  Implementation: pin_message(uuid), list_pinned_messages(), │
│                  working_memory_set()                       │
├─────────────────────────────────────────────────────────────┤
│  Layer 1: Active Context (Active Layer) ⭐ Core             │
│  Lifecycle: Agent session (during main {} execution)        │
│  Responsibility: Conversation history + tool call results   │
│    across multiple llm act calls                            │
│  Implementation: AgentContextManager + graduated            │
│    compression + three-channel                              │
├─────────────────────────────────────────────────────────────┤
│  Layer 0: Working Memory (Immediate Layer)                  │
│  Lifecycle: Within a single llm act call                    │
│  Responsibility: Current task focus, active files,          │
│    recent decisions                                         │
│  Implementation: WorkingMemory auto-update                  │
└─────────────────────────────────────────────────────────────┘
```

**Key Design Decisions**:

1. **Layer 0 is ephemeral attention guidance** — it can be destroyed once a single `llm act` returns; no persistence needed.
2. **Layer 1 boundary = a single `main {}` execution** (implemented in v1.22) — each agent `main {}` call gets a fresh context, discarded when main {} exits. Even multiple calls of the same agent do not share context. See §0.5.
3. **Layer 2 is the mechanism for users to actively declare "this information is important"** — it shares the same token window as Layer 1 but enjoys compression immunity.
4. **Layer 3 is not Context** — it is the SSOT audit record, restored on demand via `replay_transcript()` / `export_transcript()`.

### 0.3 How Long Should Context Persist?

| Question | Answer | Rationale |
|---|---|---|
| Across `llm act` calls? | ✅ Yes | This is the core value of Active Context — multi-turn tool calls need continuity |
| Across agents? | ❌ Not implicitly shared | Explicitly passed via [[interpreter/spawn|Channel / SharedStore]] to avoid scope pollution |
| Across Helen processes? | ❌ No | Process restart usually means user intent has changed; restore via Transcript instead |
| Across user sessions (day/week-level)? | ❌ Not context's responsibility | This is the job of [[runtime/skills|Skills]] / external memory / files |

**Why cross-process recovery should not rely on context persistence**:

1. Cross-process recovery would require deserializing the entire context state (environment, compression markers, pin state, working memory) — very complex.
2. User intent usually changes upon process restart, reducing the value of old context.
3. If continuity is truly needed, it should be done through **explicit export/import** or **file persistence** (see "Practical path for cross-session recovery" below).
4. Stripping the "persistence" responsibility from context allows context to be aggressively compressed without worrying about information loss.

**What should "memory" across sessions rely on**:

- **`restore_context(session_id)`** ⭐ (v1.21+): Directly restores an old transcript session as active context. Internally reads TranscriptStore, preserves all fields (`tool_calls`/`tool_call_id`/`compressed`/`pinned`/`uuid`), calls `import_context()` to populate current history. **One-step, no manual format adaptation needed.**
- **`search_transcript(query)`** ⭐ (v1.22+): Searches persistent transcript by **content**. Supports `scope="all"` for cross-session search, `regex=true` for regex matching, `role="user"` for role filtering. In typical scenarios, users can't remember session_id but remember content — use `search_transcript` to find matching session_ids, then `restore_context` to recover. See [[runtime/transcript-store|TranscriptStore SSOT §search_transcript]].
- **`export_context()` / `import_context(data)`**: Exports context snapshot (messages + working_memory + config) to file before session ends, reads back and imports on next startup. Suitable when both working_memory and config need to be saved.
- **`replay_transcript(session_id)`**: Reads old transcript message list (for audit/viewing), does **not** automatically inject into current context, and the return format is incompatible with `import_context()`.
- **File persistence**: User writes critical information to files, reads back on next startup.
- **Skills**: Codify patterns, preferences, and project conventions as skills — independent of context.

**`restore_context` vs `resume_session`**:

| | `restore_context(session_id)` | `resume_session(session_id)` |
|---|---|---|
| Restoration target | **Active Context** (conversation history seen by LLM) | **Active Context** + maintains audit trail continuity |
| Operation | Clears current history, imports old session messages | Imports old session messages into current store |
| Can LLM see restored messages? | ✅ Yes | ✅ Yes (after v1.23 fix) |
| Invocation tree filtering | Supported (filter by agent/invocation) | Not supported (imports all messages) |
| session_id change | Keeps current session_id | Keeps current session_id (after v1.23 fix) |
| Use case | Resume specific agent/invocation from old session | Restore all messages from an entire old session |

**v1.23 change**: `resume_session` changed from "replacing transcript store reference" to "importing messages into current store". This means:
- Restored messages are now visible to the LLM (tagged with current invocation_id)
- Current session_id remains unchanged (audit trail stays continuous)
- If precise recovery by agent/invocation is needed, use `restore_context`

### 0.4 Lifecycle Semantics of `context {}` Configuration

The `context { ... }` configuration in agent declarations binds Active Context behavior policies:

```helen
agent MyAgent {
    context {
        compression "graduated"       // Compression strategy
        cache-aware true              // Cache-aware
        working-memory true           // Working memory
        working-memory-tokens 5000    // Working memory token budget
    }
    main { ... }
}
```

**Semantics**: These configurations control how Active Context manages itself during the Agent session, **not cross-session persistence**. Each `helen` process startup begins with a fresh context. To continue an old session, there are two approaches:

```helen
// Approach 1 (recommended): Restore active context directly from old transcript session
let sessions = list_sessions()
// ... select the session_id to restore ...
let r = restore_context("session_1783492628_d9d9c0aa")
// r: {status: "ok", restored_messages: 42, boundary_markers: 3, note: "..."}

// Approach 2: Export/import full snapshot (preserves working_memory + config)
// Save before session ends
let snapshot = export_context()
write_file("context_snapshot.json", to_json(snapshot.context))
// Restore at new session startup
let saved = parse_json(read_file("context_snapshot.json"))
import_context(saved)
```

**Approach 1 vs Approach 2**:
- `restore_context(session_id)`: Restores **messages** with complete fields (including tool_calls, pinned, compressed, uuid). Does **not** restore working_memory and config (since transcript doesn't store these).
- `export_context() / import_context()`: Restores messages + working_memory + config all together, but requires writing to file first then reading back.

**Future consideration**:
- If Helen supports agent reuse/pooling, it needs to be clarified — whether context is reused across multiple executions of the same agent. Design tendency: **no reuse**, each execution gets fresh context. Cross-execution continuity is achieved via `restore_context()` or parameters.

### 0.5 v1.22 Implementation: Per-Main Fresh Context + Invocation Tree

> **Status**: Implemented (v1.22). See `reports/v1.22-invocation-tree-proposal.md`.

v1.22 implements the above design principles as two core mechanisms:

**1. Per-Main Fresh Context (every main {} is fresh)**

Each time the interpreter enters an agent `main {}` (or top-level main), it creates a new `invocation_id`. The `_history` property is filtered by `invocation_id` — the LLM only sees messages from the current invocation. When main {} exits, the invocation ends; the next call is fresh again.

Implementation location: `helen/interpreter/interpreter.py`
- `_enter_invocation(agent_name)` / `_exit_invocation()`: Manage the invocation stack
- `_call_agent`: Calls `_enter_invocation` when entering an agent, `_exit_invocation` in the finally block
- `visit_main_block`: Top-level main also creates an invocation
- `_history` property: Filters by `_current_invocation_id`

**2. Invocation Tree**

Each message carries three new fields (in the `Message` dataclass of `helen/runtime/history.py`):
- `agent_name`: The agent name that produced this message (`None` for top-level)
- `invocation_id`: Unique ID for this main {} execution
- `parent_invocation_id`: The parent invocation's invocation_id (builds the call tree)

The transcript still records **all** messages (SSOT audit complete), but active context is filtered by invocation.

**Query API** (`helen/stdlib/transcript.py`):
- `list_invocations(session_id?, agent?, limit?, offset?)`: List invocations
- `get_invocation(invocation_id, session_id?)`: Get single invocation metadata
- `get_invocation_tree(session_id?)`: Get complete invocation tree (nested structure)
- `invocation_path(invocation_id, session_id?)`: Invocation path string (e.g., `top -> A -> C`)

**Extended filtering parameters**:
- `replay_transcript(..., agent?, invocation_id?, last_only?, include_subtree?)`
- `restore_context(session_id, invocation_id?, agent?, last_only?, include_subtree?)`

**Isolation Boundaries**:

| Scenario | Active context shared? |
|---|---|
| Multiple `llm act` calls within the same agent's `main {}` | Accumulates (required for tool loop) |
| Two calls of the same agent's `main {}` | Isolated (fresh each time) |
| Different agents' `main {}` | Isolated |
| Nested calls: Outer calls Inner | Outer cannot see Inner's messages |
| `spawn A()` concurrency | Isolated |
| Across `helen` processes | Isolated |

**Chinese aliases**: `列出调用`, `获取调用`, `获取调用树`, `调用路径`.

### 0.6 v1.23 Fix: Invocation Isolation Implementation Correction

> **Status**: Fixed (v1.23, 2026-07-18).

v1.22 implemented the per-main fresh context design, but v1.23 found and fixed a critical implementation defect:

**Problem 1: `_prepare_history_for_llm()` bypassed invocation filtering**

In v1.22, `_prepare_history_for_llm()` directly read `transcript_store.read_view()`, bypassing the `_history` property's invocation_id filtering. This caused agents to see each other's context, violating the per-main fresh context design principle.

**Fix**: `_prepare_history_for_llm()` now uniformly uses `self._history` (which includes invocation_id filtering).

**Problem 2: `_import_context()` dual-store inconsistency**

`_import_context()` wrote to both `_interpreter_history` and `TranscriptStore`, causing data inconsistency. Moreover, imported messages were not tagged with `invocation_id`, preventing correct isolation.

**Fix**: Changed to single-write strategy — only writes to TranscriptStore when enabled, otherwise only writes to `_interpreter_history`. Imported messages are tagged with the current `invocation_id`.

**Problem 3: `resume_session()` semantic error**

`resume_session()` directly replaced the TranscriptStore reference, causing restored messages to escape invocation isolation control.

**Fix**: Changed to import messages into the current store and tag with `invocation_id`, maintaining audit trail continuity.

**Validation tests**:

```helen
// Bug before v1.23 (now fixed)
agent AgentA { main { return llm act "I am Alice" } }
agent AgentB { main { return llm act "What is my name?" } }

let a = AgentA()
let b = AgentB()
// v1.22 (bug): AgentB could answer "Alice" ❌
// v1.23 (fix): AgentB cannot see AgentA's context ✅
```

**Related files**:
- `helen/interpreter/llm_mixin.py`: `_prepare_history_for_llm()` fix
- `helen/stdlib/context.py`: `_import_context()` single-write strategy
- `helen/stdlib/transcript.py`: `resume_session()` import semantics
- `tests/interpreter/test_v123_invocation_isolation.py`: New isolation validation tests

---

## 1. Overview

Helen's context management is composed of four cooperating subsystems, aiming to maximize the quality of information the LLM receives within a limited context window:

```
┌──────────────────────────────────────────────────────────────────┐
│                    AgentContextManager (Unified Entry)            │
│                  helen/interpreter/agent_context.py                │
│                                                                   │
│   prepare_context(system_prompt, history, max_tokens)             │
│       │                                                           │
│       ├─► _compress_history()        ← Unified compression entry │
│       │     ├─ strategy="none"        → Skip                     │
│       │     ├─ strategy="traditional" → HistoryManager           │
│       │     │                          single-layer compression  │
│       │     └─ strategy="graduated"   → 5-layer pipeline         │
│       │     └─ if cache_aware: _apply_cache_aware_wrap()         │
│       │          Preserve first 30% messages, re-run base        │
│       │          strategy on compressible suffix                 │
│       │                                                           │
│       └─► build_three_channel_context() ← Three-channel build    │
│             ├─ Channel 1 (15%): System instructions              │
│             ├─ Channel 2 (50%): Working memory (budget-truncated)│
│             └─ Channel 3 (35%): Conversation history             │
└──────────────────────────────────────────────────────────────────┘
```

### Subsystem Reference

| Subsystem | File | Responsibility |
|--------|------|------|
| Working Memory | `runtime/working_memory.py` | Tracks current task state (active files, decisions, TODOs, errors) |
| Graduated Compression | `runtime/graduated_compression.py` | 5-layer graduated compression pipeline (zero-cost first) |
| Cache-Aware | Integrated in `agent_context.py` | Preserves stable prefix, improves prompt cache hit rate |
| History Management | `runtime/history.py` | Message data structure, token estimation, traditional compression |

---

## 2. AgentContextManager — Unified Entry

### 2.1 Class Definition

```python
class AgentContextManager:
    def __init__(
        self,
        working_memory_tokens: int = 5000,
        compression_strategy: str = "graduated",  # "none" | "graduated" | "traditional"
        working_memory_enabled: bool = True,
        cache_aware_enabled: bool = True,
        *,
        compression_enabled: bool | None = None,  # Backward-compat shim
    ):
```

**Meanings of `compression_strategy` values**:

| Strategy | Path | Characteristics |
|------|------|------|
| `"none"` | Skip compression | Suitable for short conversations |
| `"graduated"` | `graduated_compress()` — 5-layer pipeline | Cheapest-action-first, zero-cost layers don't call LLM |
| `"traditional"` | `HistoryManager.enforce_limit()` | Legacy single-layer summarize/truncate, simple and predictable |

**`cache_aware_enabled`**: Wraps (not replaces) the base strategy. When enabled, preserves the first 30% of messages as a cache-stable zone, running base compression only on the suffix. Can be combined with either `graduated` or `traditional`.

**Backward compatibility**: `compression_enabled=True/False` still works, mapped to `"graduated"` / `"none"` via property.

### 2.2 Core Call Flow

```
prepare_context()
    │
    ├─► _compress_history(history, max_tokens)
    │     │
    │     │  # Step 1: Select base compression
    │     ├─ strategy == "none" or len(history) <= 10:
    │     │     return history  (skip)
    │     │
    │     ├─ strategy == "traditional":
    │     │     return HistoryManager(context_window=max_tokens).enforce_limit(history)
    │     │
    │     └─ strategy == "graduated":
    │           return graduated_compress(history, usage_ratio, max_tokens)
    │
    │     # Step 2: Cache-aware wrapping (if cache_aware_enabled)
    │     └─ _apply_cache_aware_wrap(compressed, original_history, max_tokens)
    │           cache_zone = original_history[:N]       # First 30%, preserved as-is
    │           suffix     = original_history[N:]        # Suffix
    │           return cache_zone + base_compress(suffix, adjusted_budget)
    │
    └─► build_three_channel_context(system_prompt, working_memory, compressed)
          ├─ Channel 1: System prompt (truncated to 15% budget)
          ├─ Channel 2: working_memory.to_context(budget_chars=50%*max_tokens)
          └─ Channel 3: History messages (fill 35% budget from newest to oldest)
```

### 2.3 Interpreter Integration

```python
# interpreter.py — Initialization
self._agent_context = AgentContextManager(
    working_memory_tokens=5000,
    compression_strategy="graduated",
    working_memory_enabled=True,
    cache_aware_enabled=True,
)

# llm_mixin.py — Apply agent's context {} config before each llm act
self._agent_context.compression_strategy = ctx_config.compression
self._agent_context.working_memory_enabled = ctx_config.working_memory
self._agent_context.cache_aware_enabled = ctx_config.cache_aware
```

### 2.4 Agent Declaration Configuration

```helen
agent SmartAssistant {
    context {
        compression "graduated"       // "none" | "graduated" | "traditional"
        cache-aware true              // Cache-aware wrapping
        working-memory true           // Working memory
        working-memory-tokens 5000    // Working memory token budget
    }
    main { ... }
}

// Chinese keywords
agent 智能助手 {
    上下文 {
        压缩 "graduated"
        缓存感知 true
        工作记忆 true
        工作记忆词元 5000
    }
    主逻辑 { ... }
}
```

---

## 3. Working Memory

### 3.1 Data Structure

```python
@dataclass
class WorkingMemory:
    task_description: str = ""         # Current task description (never evicted, highest priority)
    active_files: list[str]            # Active files (evicted under token budget)
    recent_decisions: list[str]        # Recent decisions (evicted under token budget)
    pending_todos: list[str]           # Pending TODOs (evicted under token budget)
    error_history: list[dict]          # Error records (evicted under token budget)
    max_tokens: int = 5000             # Token budget
```

**Token-level eviction (v1.15+)**: `_add_active_file`, `_add_decision`, `_add_todo`, `_add_error` each check total token count after adding. When `max_tokens` is exceeded, the oldest entries are evicted from lowest to highest priority:

```
Eviction order (first evicted → last evicted):
Pending TODOs → Recent Decisions → Active Files → Error History
(task_description is never evicted)
```

**Difference from legacy version**: The old version used hardcoded list length limits (10/10/20/5); the new version uses token-budget-driven eviction. More precise when entries vary in size — large entries (long paths, long errors) trigger eviction sooner.

### 3.2 Auto-Update

Updated automatically via `AgentContextManager` at two points:

- **`update_from_message(content, role)`**: Regex-extracts file references, TODOs, decisions from message content
- **`update_from_tool_call(tool_name, tool_args, tool_result)`**: Structured tracking from tool calls

| Tool | Effect |
|------|------|
| `read_file` | Add to `active_files` |
| `write_file` / `patch_file` | Add to `active_files` + record decision |
| `shell_exec` (failure) | Add to `error_history` |
| `glob_files` / `grep_files` | Record search decision |

### 3.3 Formatted Output and Budget Truncation

`to_context(budget_chars=None)` formats working memory as a Markdown string:

```
## Current Task
Fix authentication bug

## Recent Errors
- Command: pytest
  Error: 3 failed

## Active Files
- src/auth.py
- tests/test_auth.py
```

**When `budget_chars` is provided**, partitions are progressively dropped from lowest to highest priority:

```
Pending TODOs (dropped first)
    ↓
Recent Decisions
    ↓
Active Files
    ↓
Recent Errors
    ↓
Current Task (dropped last, body truncated to line boundary if needed)
```

### 3.4 Known Limitations

The `max_tokens` field is currently only used for budget calculations in `build_three_channel_context`. `WorkingMemory` itself does not perform token-level eviction — it relies only on list length limits (10/10/20/5).

---

## 4. Graduated Compression Pipeline

### 4.1 Five-Layer Pipeline

Located in `helen/runtime/graduated_compression.py`, design principle: "cheapest action first" — each layer only triggers when cheaper layers are insufficient.

| Layer | Threshold | Strategy | Cost | Mechanism |
|------|------|------|------|------|
| Layer 1 | 60% | Budget Reduction | Zero | Replaces >4000-char tool results with reference pointers |
| Layer 2 | 70% | Snip | Zero | Drops stale turns, keeps recent 8 turns |
| Layer 3 | 80% | Microcompact | Zero | Clears old tool result content, preserves `tool_use` decisions ⭐ |
| Layer 4 | 90% | Context Collapse | Zero | Archives old turns, **projects timeline view** (segmented summaries preserving temporal structure) |
| Layer 5 | 95% | Auto-Compact | **Zero or High** | Preferentially calls `LLMSummarizer` for semantic summary; falls back to zero-cost structural summary when LLM unavailable |

**Pinned message compression immunity** (v1.19): Messages marked by `pin_message(uuid)` are preserved across all 5 layers: Layer 1 doesn't replace their content, Layer 2 doesn't drop them, Layer 3 doesn't clear them, Layer 4 doesn't archive them, Layer 5 doesn't summarize them. Used to protect critical context (system prompts, key decisions, few-shot examples, etc.). `Message.pinned: bool` field added to `history.py`, persisted alongside in `TranscriptStore`.

**Pin recovery across agent restarts** (v1.30): `list_pinned_messages()` enumerates all `pinned=true` messages from the TranscriptStore (SSOT). `ContextManager.init()` calls it on startup to rebuild the in-memory `pinned_uuids` list, so `/pin` display state matches the actual persisted pin state. Without this, compression protection works across restarts (because it reads `msg.pinned` directly from transcript), but `/pin` would show an empty list.

**Layer 4 improvement (timeline preservation)**: Inspired by RCC (Recurrent Context Compression) and CogCanvas, Context Collapse now segments old messages (10 per block), extracts file references, tool usage, and user intent from each segment, generating a timeline view that preserves the temporal structure of task progress.

**Layer 5 improvement (LLM semantic summary)**: When `llm_client` parameter is provided, `_auto_compact` calls `LLMSummarizer` to generate high-quality semantic summaries, preserving task objectives, key decisions, file changes, etc. Falls back to zero-cost structural summary (extracting file paths, tool counts, user intent, etc.) when LLM is unavailable.

### 4.2 API

```python
def graduated_compress(
    history: list[Message],
    usage_ratio: float,
    max_tokens: int = 131072,
    llm_client: Callable | None = None,  # New: Layer 5 LLM client
) -> tuple[list[Message], str]:
    """
    Args:
        llm_client: Optional LLM client for Layer 5 semantic summary.
                    Signature: llm_client(messages) -> str
                    When None, Layer 5 falls back to structural summary.

    Returns:
        (compressed_history, layer_used)
        layer_used: "none" | "budget_reduction" | "snip" |
                    "microcompact" | "context_collapse" | "auto_compact"
    """
```

### 4.3 Microcompact Core Innovation

Distinguishes between "actions" (`tool_use` blocks) and "data" (`tool_result` content):
- ✅ Preserves `tool_use` blocks — "what the LLM decided to do"
- ❌ Clears old `tool_result` content — "what the tool returned"

Effect: Uses 20% of tokens to preserve 80% of decision context.

### 4.4 Context Collapse Timeline View

**Design idea**: Inspired by CogCanvas and RCC, preserves the temporal structure of conversation, avoiding the "when did this happen" information loss of traditional summaries.

**Algorithm**:
1. Segments old messages (10 per block)
2. For each segment, extracts:
   - Time markers (message index range)
   - File references (regex-extracted paths)
   - Tool usage (counts tool_calls)
   - User intent (first line truncated)
3. Generates timeline summary + global statistics

**Example output**:
```
[Context Collapse: 30 turns archived as timeline]
  [0-10] Files: main.py, utils.py | Tools: read_file(3), write_file(1) | Tasks: Fix auth bug
  [10-20] Files: auth.py, test_auth.py | Tools: shell_exec(2) | Tasks: Run tests
  [20-30] Files: config.yaml | Tools: patch_file(1)
[Global] Turns: 15u/15a | Tool calls: 12 | Errors: 2
[Preserved: last 20 turns for continuity]
```

### 4.5 Auto-Compact LLM Semantic Summary

**Enablement**: Pass `llm_client` during `AgentContextManager` initialization:

```python
agent_context = AgentContextManager(
    compression_strategy="graduated",
    llm_client=my_llm_client,  # Signature: (messages) -> str
)
```

**LLM summary format** (generated by `LLMSummarizer`):
```
## Task Objective
[User objective]

## Key Decisions
- [Decision 1 and reasoning]

## File Changes
- path/to/file.py: [change description]

## Completed
- [completed items]

## Pending
- [ ] [pending items]
```

**Fallback mechanism**: Automatically falls back to structural summary on LLM call failure, ensuring the compression pipeline is not interrupted.

---

## 5. Cache-Aware Compression

### 5.1 Design Motivation

Most LLM APIs support prompt caching: the conversation prefix is cached, reducing cost by 50-90% on reuse. **Modifying the prefix invalidates the cache**.

Traditional graduated compression (e.g., Snip dropping early messages, Context Collapse inserting summaries at the beginning) inadvertently breaks the cache.

### 5.2 Current Implementation: Wrap Mode

Cache-awareness acts as a **wrapping layer** rather than an independent strategy:

```
Original history: [msg1, msg2, ..., msg50]
                ↓
┌─ cache zone (first 30%) ─┐  ┌─ compressible zone (last 70%) ─┐
│ msg1..msg15 (preserved)  │  │ msg16..msg50 (base compression)│
└──────────────────────────┘  └────────────────────────────────┘
                ↓
Result: [msg1..msg15 (unchanged)] + [compressed msg16..msg50]
```

**Core guarantee**: Prefix unchanged → prompt cache hit.

### 5.3 Combinations with Base Strategies

| Combination | Behavior |
|------|------|
| `graduated` + `cache_aware` | 5-layer pipeline applied only to suffix |
| `traditional` + `cache_aware` | Single-layer compression applied only to suffix |
| `none` + `cache_aware` | Meaningless, skipped |

### 5.4 Constants

```python
DEFAULT_CACHE_ZONE_RATIO = 0.30     # First 30% as cache zone
MIN_CACHE_ZONE_MESSAGES = 5         # Minimum 5 messages
BATCH_COMPRESSION_THRESHOLD = 0.75  # Trigger when usage ≥75%
```

---

## 6. Three-Channel Context

### 6.1 Budget Allocation

`build_three_channel_context()` divides context into three channels:

| Channel | Budget | Content | Truncation Method |
|------|------|------|----------|
| Channel 1 | 15% × max_tokens | System prompt | Character-level truncation |
| Channel 2 | min(50% × max_tokens, working_memory.max_tokens) | Working memory | Partition-priority dropping |
| Channel 3 | 35% × max_tokens | Conversation history | Fill from newest to oldest |

### 6.2 Channel 2 Budget Truncation

Working memory performs budget truncation via `to_context(budget_chars=...)`. When content exceeds budget, partitions are progressively dropped by priority (see §3.3).

---

## 7. Traditional Compression

### 7.1 HistoryManager

Located in `helen/runtime/history.py`, this is Helen's early compression implementation. Used when `compression_strategy="traditional"`.

```python
class HistoryManager:
    compression_mode: str  # "summarize" | "truncate" | "none"

    def enforce_limit(self, history, budget_ratio=0.8) -> list[Message]:
        """Three-layer compression: recent messages preserved → middle compressed → oldest dropped"""
```

### 7.2 Difference from Graduated Compression

| Dimension | Traditional | Graduated |
|------|---------|---------|
| Layers | Single | 5 |
| Trigger | One-shot after exceeding budget | Gradual (60%→70%→80%→90%→95%) |
| Content selection | No differentiation | Distinguishes actions from data |
| LLM calls | No | No (all layers zero-cost) |
| Use case | Simple short conversations | Long-running Agents |

---

## 8. Stdlib Functions

### 8.1 `clear_context()`

```helen
let result = clear_context()
// Returns: {status: "ok", cleared_messages: 15, cleared_tokens: 8000}
```

Clears conversation history.

**Known limitation**: Currently only clears `_interpreter_history`, not `AgentContextManager.working_memory`. Active files, decisions, TODOs, and errors in working memory persist after clearing.

### 8.2 `compress_context(strategy)`

```helen
let result = compress_context("auto")      // Per HistoryManager.compression_mode
let result = compress_context("summarize")  // Concatenates old messages as summary
let result = compress_context("truncate")   // Drops old messages
let result = compress_context("none")       // No-op
// Returns: {status, original_messages, compressed_messages, original_tokens, compressed_tokens, strategy}
```

**Known limitation**: In the current implementation, the return values of `enforce_limit()` / `_summarize_compress()` / `_truncate_compress()` are discarded, so the history is not actually modified (bug).

### 8.3 `compress_context(target, keep_recent)`

```helen
let result = compress_context(target="tool_results", keep_recent=5)
// Clears old tool results, preserves tool_use decisions
let result = compress_context(target="stale_turns", keep_recent=8)
// Drops stale turns
```

### 8.4 `clear_context()` / `compress_context()` Chinese Aliases

```helen
清除上下文()     // = clear_context()
压缩上下文()     // = compress_context()
```

### 8.5 v1.19 Additions: Context Inspection and Fine-Grained Operations

Before v1.19, context management APIs only had "bulk clear" and "bulk compress" coarse-grained actions — agents could neither **see** the current context state nor **manipulate** individual messages. v1.19 fills in APIs across **6 dimensions**.

#### 8.5.1 Inspection

```helen
// context_stats() — Detailed statistics
let stats = context_stats()
// Returns:
// {
//   status: "ok",
//   message_count: 42,        // Total message count
//   total_tokens: 18000,      // Estimated total tokens
//   usage_ratio: 0.45,        // Context window usage (0.0–1.0+)
//   max_tokens: 40000,        // Configured context window size
//   by_role: {system: 1, user: 15, assistant: 14, tool: 12},
//   compressed_count: 5,      // Number of compressed messages
//   pinned_count: 2,          // Number of pinned messages
// }

// context_usage() — Simplified version, returns only usage ratio
if context_usage() > 0.7 {
    compress_context("auto")
}
```

#### 8.5.2 Single Message Access and Operations

```helen
// Read
get_message(uuid)      // Read message snapshot by UUID

// Write
insert_message(role, content, position?)   // Insert new message (appends to end by default)
replace_message(uuid, new_content)         // Replace message content
delete_message(uuid)                       // Logical delete (preserved in audit)

// Pin (Compression Immunity)
pin_message(uuid)             // Pin message, skipped by all 5 compression layers
unpin_message(uuid)           // Unpin
list_pinned_messages()        // List all pinned messages: [{uuid, role, snippet, token_count}]
```

Pinned messages are preserved across Layers 1–5:
- Layer 1: Does not replace their tool output
- Layer 2: Does not drop (even if "stale" turn)
- Layer 3: Does not clear their content
- Layer 4: Does not archive (preserved in projection view)
- Layer 5: Does not participate in semantic summary

#### 8.5.3 Working Memory Access (P1)

```helen
// Read (empty key returns all)
let data = working_memory_get("task")         // Returns task description
let all = working_memory_get()                // Returns all fields

// Write (list-type keys append by default, can also replace entirely)
working_memory_set("task", "Build feature X")
working_memory_set("active_files", "new.py")  // Append
working_memory_set("active_files", ["a.py"])  // Replace

// Remove (empty item clears the entire field)
working_memory_remove("task")
working_memory_remove("active_files", "old.py")

// Clear all
working_memory_clear()
```

**Available keys**: `task` | `active_files` | `decisions` | `todos` | `errors`

#### 8.5.4 Runtime Configuration (P2)

Before v1.19, these configurations could only be declared in `agent context {}` blocks. v1.19 supports runtime modification.

```helen
set_compression_strategy("graduated")   // "graduated" | "traditional" | "none"
set_context_window(64000)               // Set context window size (token count)
set_working_memory_enabled(true)        // Toggle working memory
set_cache_aware(true)                   // Toggle cache-aware
let cfg = get_context_config()          // Query current config
// cfg: {compression_strategy, max_tokens, working_memory_enabled, cache_aware_enabled, ...}
```

#### 8.5.5 Query (P3)

```helen
// Full-text search
let r = search_context("TODO", role="user", limit=10)
// r.matches: [{uuid, role, snippet, index}, ...]

// Context slice
let slice = context_slice(start=5, end=20, role="")
// slice.messages: [{uuid, role, content, token_count, compressed, pinned, index}, ...]
```

#### 8.5.6 Multi-Agent Context Sharing (P2/P3)

```helen
// Export current context as transferable dict
let snapshot = export_context()
// snapshot.context: {messages, working_memory, config}

// Import context (replaces current history)
import_context(snapshot.context)

// Fork: Returns deep copy with same structure as export_context
let forked = fork_context()
// Modifying forked does not affect original context
```

Typical uses: Pass current conversation context to another agent via Channel; save context to disk; fork to explore multiple directions in parallel.

#### 8.5.6b Cross-Session Recovery (v1.21+)

```helen
// List all old sessions
let sessions = list_sessions()
for s in sessions {
    print("{s.session_id}: {s.message_count} msgs, scope={s.scope}")
}

// Restore active context directly from old transcript session
let r = restore_context("session_1783492628_d9d9c0aa")
// r: {
//   status: "ok",
//   restored_messages: 42,
//   session_id: "session_1783492628_d9d9c0aa",
//   boundary_markers: 3,       // Number of compression boundary markers skipped
//   note: "Working memory and context config are not persisted..."
// }

// Chinese alias
let r2 = 恢复上下文("session_1783492628_d9d9c0aa")
```

**Semantics of `restore_context(session_id)`**:

1. Reads the specified session's TranscriptStore from disk
2. Iterates all Messages (skipping BoundaryMarkers), preserving complete fields: `role`, `content`, `tool_calls`, `tool_call_id`, `uuid`, `compressed`, `pinned`
3. Internally calls `import_context()` to replace current `_interpreter_history`
4. After restoration, the next `llm act` call can see all messages from the old session

**Limitation**: Only restores messages. Does **not** restore working_memory and context config (transcript doesn't store these). Use `working_memory_set()` / `set_compression_strategy()` etc. for manual restoration when needed.

**Difference from `resume_session()`**:

| | `restore_context` | `resume_session` |
|---|---|---|
| Restoration target | Active Context (what LLM sees) | TranscriptStore (audit record) |
| Can LLM see restored messages? | ✅ Yes | ❌ No (only swaps SSOT reference) |
| Use case | Continue work from old session | Switch to old transcript stream |

#### 8.5.7 Lifecycle Hooks (P1)

```helen
// Register compression event callback
on_compression(callback)
// callback receives: {layer, original_tokens, compressed_tokens, ...}

// Register context overflow callback (reserved interface)
on_context_overflow(callback)

// Pass None to clear callback
on_compression(None)
```

#### 8.5.8 Chinese Aliases (v1.19 all 24 + v1.21 new 1)

| English Name | Chinese Name |
|--------|--------|
| context_stats | 上下文统计 |
| context_usage | 上下文占用率 |
| get_message | 获取消息 |
| delete_message | 删除消息 |
| pin_message | 钉住消息 |
| unpin_message | 取消钉住 |
| list_pinned_messages | 已钉住消息 / 钉住列表 |
| insert_message | 插入消息 |
| replace_message | 替换消息 |
| working_memory_get | 获取工作记忆 |
| working_memory_set | 设置工作记忆 |
| working_memory_remove | 移除工作记忆 |
| working_memory_clear | 清空工作记忆 |
| set_compression_strategy | 设置压缩策略 |
| set_context_window | 设置上下文窗口 |
| set_working_memory_enabled | 设置工作记忆开关 |
| set_cache_aware | 设置缓存感知 |
| get_context_config | 获取上下文配置 |
| search_context | 搜索上下文 |
| context_slice | 上下文切片 |
| export_context | 导出上下文 |
| import_context | 导入上下文 |
| fork_context | 分叉上下文 |
| **restore_context** | **恢复上下文** (v1.21+) |
| on_compression | 压缩回调 |
| on_context_overflow | 溢出回调 |

#### 8.5.9 Internalized

The stdlib function `classify_message` has been internalized to `_classify_message` and is no longer exposed externally. Chinese alias "消息分类" is removed accordingly.

---

## 9. Known Issues and Limitations

> The following are known issues in the current architecture, sorted by severity.

### 9.1 Architectural Issues

**Fixed**:
- ✅ `LLMSummarizer` integrated into Layer 5: Passed via `graduated_compress(llm_client=...)`, falls back to structural summary when LLM unavailable
- ✅ `WorkingMemory` changed to token-level eviction: `_evict_to_budget()` evicts oldest entries by priority

### 9.2 Documentation-Code Mismatch

**Fixed**:
- ✅ `wiki/runtime/working_memory.md`: Updated with correct API (`to_context(budget_chars)`, Channel 2 budget truncation)
- ✅ `wiki/runtime/history.md`: Updated `estimate_tokens` description, documenting character-type awareness and CJK support

---

## 10. Data Flow Overview

```
User input prompt
    │
    ▼
visit_llm_act_expr()
    │
    ├─► Read agent.context_config
    │   └─► Update AgentContextManager's strategy/cache_aware/working_memory settings
    │
    ├─► _add_to_history("user", prompt)
    │   ├─► Append Message to self._history
    │   ├─► HistoryManager.enforce_limit()    ← Traditional compression (first layer)
    │   └─► agent_context.update_from_message() ← Update working memory
    │
    ├─► LLM call + tool loop
    │   └─► _record_llm_response_to_history()
    │       ├─► Append assistant/tool Message
    │       └─► agent_context.update_from_tool_call() ← Update working memory
    │
    └─► _prepare_history_for_llm()
        └─► agent_context.prepare_context()
            ├─► _compress_history()           ← Graduated compression + cache-aware wrapping (second layer)
            └─► build_three_channel_context() ← Three-channel build
                ├─ Channel 1: System instructions (15%)
                ├─ Channel 2: Working memory (50%, budget-truncated)
                └─ Channel 3: Conversation history (35%)
                    │
                    ▼
                Send to LLM API
```

**Note**: The diagram labels "first layer" and "second layer" compression — this is the root of the current dual-compression issue. `enforce_limit()` in `_add_to_history()` uses the traditional algorithm to compress once, then `_compress_history()` in `prepare_context()` uses the graduated algorithm to compress again.

---

## 11. File Index

| File | Responsibility |
|------|------|
| `helen/interpreter/agent_context.py` | Unified entry: AgentContextManager, working memory update, compression orchestration, three-channel build |
| `helen/runtime/working_memory.py` | WorkingMemory dataclass, `to_context(budget_chars)` formatting, `build_three_channel_context()` |
| `helen/runtime/graduated_compression.py` | 5-layer graduated pipeline, `graduated_compress()`, per-layer functions |
| `helen/runtime/cache_aware_compression.py` | `CacheAwareCompressor` class (currently not called by agent_context), convenience functions |
| `helen/runtime/history.py` | Message dataclass, HistoryManager (traditional compression, token estimation) |
| `helen/runtime/llm_summarizer.py` | LLMSummarizer, `auto_compact()` (currently not integrated) |
| `helen/stdlib/context.py` | `clear_context()`, `compress_context()`, `compress_context_target()` |
| `helen/core/ast.py` | `ContextConfigNode` AST node |
| `helen/core/parser.py` | `context {}` block parsing |
| `helen/interpreter/llm_mixin.py` | `context_config` application, history update integration |

---

## 12. References

The context management system described in this document draws from the following academic research:

- **RCC (Recurrent Context Compression)** — Segmented summary preserving temporal structure → Layer 4 Context Collapse
- **CogCanvas** — Preserving temporal details, avoiding information loss → Layer 4 timeline view
- **DAST (Dynamic Allocation)** — Dynamically allocating compression tokens (future improvement direction)

See [[runtime/context-compression-research|Context Compression Research]].

---

**Last Updated**: 2026-07-17
**Version**: v1.22
