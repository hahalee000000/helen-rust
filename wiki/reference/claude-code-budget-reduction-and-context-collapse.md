# Claude Code Context Management Deep Dive: Budget Reduction and Context Collapse

> In-depth analysis of two key layers in the Claude Code compaction pipeline

**Date**: 2026-07-06
**Source**: arXiv:2604.14228v2 (Liu et al., 2026)

---

## 1. Budget Reduction (Layer 1): Large Tool Outputs → Reference Pointers

### 1.1 Core Mechanism

**Function**: `applyToolResultBudget()`
**Location**: query.ts (executed before every model call)
**Status**: Always enabled, no feature flag
**Cost**: Zero inference cost (pure content replacement)

#### How It Works

Budget Reduction is the first layer of the compaction pipeline, executed before every model call. Its core responsibility is:

```
For each tool result message:
  Check whether its size exceeds the maxResultSizeChars limit
  ├── Exceeds → Replace with a "content reference"
  └── Does not exceed → Keep as-is
```

#### Key Design Decisions

**1. Exemption Mechanism**

Certain tools are marked as "exempt" and not subject to budget reduction:

```typescript
// The maxResultSizeChars field in tool definitions
// If Infinity or unset (non-finite), the tool's output is not reduced
{
  name: "important_tool",
  maxResultSizeChars: Infinity,  // Exempt: preserve full output
}

{
  name: "regular_tool",
  maxResultSizeChars: 10000,     // Normal: replaced if over 10000 chars
}
```

**Design rationale**: Some tool outputs are critical for subsequent reasoning (e.g., key error messages, structured data) and must not be truncated.

**2. Content Reference**

The paper does not provide the exact format of "content references," but its structure can be inferred:

```
Original tool result (15KB):
┌─────────────────────────────────────────┐
│ import React from 'react';              │
│ import { useState } from 'react';       │
│ ... (15KB of code content) ...          │
│ export default Component;               │
└─────────────────────────────────────────┘

Replaced content reference (~200 chars):
┌─────────────────────────────────────────┐
│ [read_file: src/component.tsx, 15KB]    │
│ First 100 chars: import React from...   │
│ Last 100 chars: ...export default...    │
└─────────────────────────────────────────┘
```

**Inference basis**:
- The paper says "replacing oversized outputs with content references"
- The paper mentions "persisted for agent and session query sources to enable reconstruction on resume"
- This implies the reference contains enough information for the model to know the file exists, its size, and its head/tail content

**3. Persistence and Recovery**

```typescript
// Content replacements are persisted to disk
// Stored in agent and session query sources
// Purpose: can be reconstructed on session resume

// Pseudocode:
persistContentReplacement({
  tool_use_id: "toolu_01ABC...",
  original_content_hash: "sha256:...",
  replacement_pointer: "[read_file: src/main.py, 12KB]",
  source: "agent_query" | "session_query",
  timestamp: Date.now(),
})

// On resume:
reconstructFromDisk() → Restores replacement records; can continue compressing history
```

### 1.2 Compositional Design with Microcompact

**Key insight**: Budget Reduction runs before Microcompact because the two operate at different levels and compose without conflict.

```
Execution order:
1. Budget Reduction → Checks content (content-level)
   └─ Replaces oversized tool outputs with reference pointers

2. Microcompact → Operates by ID (ID-level)
   └─ Identifies pairs to compress via tool_use_id
   └─ Does not inspect content; only manipulates structure
```

**Why this ordering matters**:

```
If reversed:
  Microcompact runs first → Clears old tool_result content
  Budget Reduction runs second → Tries to inspect already-cleared content → ineffective

Correct order:
  Budget Reduction runs first → Checks and replaces oversized content
  Microcompact runs second → Compresses by ID; does not care what the content is
```

**Code-level composition**:

```typescript
// Execution order in query.ts:365-453
function prepareMessagesForQuery(messagesForQuery) {
  // Layer 1: Always runs
  messages = applyToolResultBudget(messagesForQuery);
  // At this point: oversized tool outputs are replaced with reference pointers
  // But tool_use_ids remain unchanged

  // ... Layer 2: Snip ...

  // Layer 3: Operates by tool_use_id
  messages = microcompact(messages);
  // At this point: old tool_use/tool_result pairs are compressed
  // Does not inspect content; only identifies by ID
}
```

### 1.3 Practical Effect Estimation

```
Scenario: 50-turn conversation; each read_file returns 10-50KB of code

Original state:
  - 50 read_file calls
  - Average 20KB per call
  - Total ~1MB of tool output
  - Approximately 250K tokens (exceeds 200K window)

After Budget Reduction:
  - 50 read_file calls
  - Outputs over 10KB are replaced with reference pointers (~200 chars)
  - Suppose 30 are replaced, 20 are retained
  - 30 × 200 chars + 20 × 20KB = 6KB + 400KB = ~406KB
  - Approximately 100K tokens (60% reduction)

What is preserved:
  ✅ tool_use blocks (model remembers "which files I read")
  ✅ Reference pointers (model knows "the file exists, size X KB")
  ✅ Head/tail content (model can see the beginning and end of the file)
  ❌ Full content (lost, but can be recovered by re-reading the file)
```

### 1.4 Details Not Provided in the Paper

The following information is **not explicitly stated** in the paper and requires source code inspection or experimental inference:

1. **Default value of `maxResultSizeChars`**
   - The paper only says "configurable size"
   - Estimated: 10KB-50KB (based on typical code file sizes)

2. **Exact format of content references**
   - The paper only says "content references"
   - Estimated to include: tool name, file name, size, head/tail fragments

3. **Which tools are marked as exempt**
   - The paper only says "exempt tools"
   - Estimated: tools like `search_files`, `grep` that return structured results may be exempt

4. **Exact storage location for persistence**
   - The paper says "persisted for agent and session query sources"
   - Estimated: stored in the session transcript (JSONL)

---

## 2. Context Collapse (Layer 4): Read-Time Projection

### 2.1 Core Mechanism

**Function**: `applyCollapsesIfNeeded()`
**Location**: query.ts (feature-gated, dynamic `require()`)
**Feature Flag**: `CONTEXT_COLLAPSE`
**Cost**: Zero inference cost (pure read-time projection)
**Status**: Does not modify underlying data

#### How It Works

Context Collapse is the **only layer** among the five that does not modify the message array. It is a **read-time projection** that projects a collapsed view over the full underlying history.

```
Underlying Storage (REPL array)              Read-Time View (messagesForQuery)
┌─────────────────────────┐       ┌───────────────────────────┐
│ Turn 1: user+assistant  │       │                           │
│ Turn 2: user+assistant  │       │ [Collapse summary:        │
│ Turn 3: user+assistant  │       │  first 20 turns]          │
│ ...                     │  ──→  │ Turn 21: user+assistant   │
│ Turn 20: user+assistant │       │ Turn 22: user+assistant   │
│ Turn 21: user+assistant │       │ ...                       │
│ ...                     │       │ Turn 50: user+assistant   │
│ Turn 50: user+assistant │       └───────────────────────────┘
└─────────────────────────┘
Full history is never modified        Model only sees the collapsed view
(append-only JSONL)                   (projected into messagesForQuery)
```

### 2.2 Source Code Comment Interpretation

The paper quotes key comments from the source code:

> **"Nothing is yielded; the collapsed view is a read-time projection over the REPL's full history. Summary messages live in the collapse store, not the REPL array. This is what makes collapses persist across turns."**

Sentence-by-sentence interpretation:

**"Nothing is yielded"**
- Context Collapse does not produce new messages
- Unlike Auto-Compact, which generates summary messages and inserts them into the history
- It merely "projects" a view

**"read-time projection over the REPL's full history"**
- The REPL array is the underlying storage (append-only JSONL)
- "read-time" means dynamically computed at read time
- "projection" is a database term referring to selecting a subset from the full data

**"Summary messages live in the collapse store, not the REPL array"**
- Collapse summaries are stored in a separate "collapse store"
- Not written into the REPL array (underlying history)
- This is a key architectural decision

**"This is what makes collapses persist across turns"**
- Because summaries are in a separate store
- They persist across turns
- Even as the REPL array grows, the collapsed view remains effective

### 2.3 Fundamental Difference from Other Layers

| Layer | Modifies Message Array | Underlying Data | Persistence Method |
|-------|------------------------|-----------------|-------------------|
| Budget Reduction | ✅ Modifies | Modified (replaces content) | Writes to REPL array |
| Snip | ✅ Modifies | Modified (deletes messages) | Writes to REPL array |
| Microcompact | ✅ Modifies | Modified (clears content) | Writes to REPL array |
| **Context Collapse** | ❌ Does not modify | **Not modified** | **collapse store** |
| Auto-Compact | ✅ Modifies | Modified (appends summary) | Writes to REPL array |

**Why does Context Collapse choose "not modifying"?**

```
Design philosophy: append-only design prioritized over auditability

Advantages:
✅ Can resume (restore full history from disk)
✅ Can fork (create branches based on full history)
✅ Can audit (review full history)
✅ No information is lost (only hidden, not deleted)

Disadvantages:
❌ Structured queries require post-hoc reconstruction
   (e.g., "show all tool calls that modified file X" requires scanning full history)
```

### 2.4 Inferred Implementation Details

The paper does not provide the exact implementation, but it can be inferred from the architecture:

#### Collapse Store Data Structure

```typescript
// Inferred collapse store structure
interface CollapseStore {
  sessionId: string;
  collapses: Collapse[];
}

interface Collapse {
  id: string;
  turnRange: [number, number];  // Collapsed turn range [1, 20]
  summary: string;               // Collapse summary
  createdAt: number;             // Creation timestamp
  tokenCount: number;            // Token count of the summary
}

// Example:
{
  sessionId: "sess_01ABC...",
  collapses: [
    {
      id: "collapse_001",
      turnRange: [1, 20],
      summary: "First 20 turns: User requested a fix for auth.test.ts; model read auth.ts and auth.test.ts, discovered the test failure cause...",
      createdAt: 1704067200000,
      tokenCount: 500,
    },
    {
      id: "collapse_002",
      turnRange: [21, 40],
      summary: "Turns 21-40: Model modified auth.ts, ran tests, fixed 3 bugs...",
      createdAt: 1704067500000,
      tokenCount: 600,
    }
  ]
}
```

#### Pseudocode for applyCollapsesIfNeeded()

```typescript
function applyCollapsesIfNeeded(messagesForQuery: Message[]): Message[] {
  if (!feature("CONTEXT_COLLAPSE")) {
    return messagesForQuery;  // Feature flag off; do not collapse
  }

  const collapseStore = loadCollapseStore(sessionId);
  if (collapseStore.collapses.length === 0) {
    return messagesForQuery;  // No collapses; return original view
  }

  // Build collapsed view
  const collapsedView: Message[] = [];

  for (const collapse of collapseStore.collapses) {
    // Check if current messages fall within this collapse range
    const messagesInRange = messagesForQuery.filter(
      msg => msg.turnNumber >= collapse.turnRange[0]
          && msg.turnNumber <= collapse.turnRange[1]
    );

    if (messagesInRange.length > 0) {
      // Replace these messages with the collapse summary
      collapsedView.push({
        role: "system",
        content: `[Conversation collapse] ${collapse.summary}`,
        turnNumber: collapse.turnRange[0],
        isCollapseSummary: true,
      });
    }
  }

  // Add messages that are not collapsed
  const nonCollapsedMessages = messagesForQuery.filter(
    msg => !collapseStore.collapses.some(
      c => msg.turnNumber >= c.turnRange[0] && msg.turnNumber <= c.turnRange[1]
    )
  );

  collapsedView.push(...nonCollapsedMessages);

  // Sort by turn number
  collapsedView.sort((a, b) => a.turnNumber - b.turnNumber);

  return collapsedView;
}
```

### 2.5 Collapse Triggering and Creation

The paper does not explain how collapses are triggered, but it can be inferred:

```
Inferred collapse trigger conditions:

1. Turn count threshold
   - When conversation exceeds N turns (e.g., 50 turns)
   - Automatically create collapse summary

2. Token threshold
   - When history exceeds M tokens (e.g., 100K)
   - Create collapse for the earliest N turns

3. Explicit trigger
   - User or system invokes a collapse command
   - e.g., "/compact" or auto-compaction

How collapse summaries are generated:
- Possibly using an LLM to generate a summary
- Could also be simple rule-based extraction (e.g., "First N turns, involving files X, Y, Z")
- The paper does not specify
```

### 2.6 Collaboration with Other Compression Layers

```
Execution order and division of responsibilities:

Layer 1 (Budget Reduction):
  └─ Replace individual oversized tool outputs
  └─ Does not care about turns; only cares about individual message size

Layer 2 (Snip):
  └─ Discard stale turns
  └─ Modifies REPL array (deletes messages)

Layer 3 (Microcompact):
  └─ Clear old tool result content
  └─ Operates by tool_use_id

Layer 4 (Context Collapse):  ← The only non-modifying layer
  └─ Project collapsed view
  └─ Does not modify REPL array
  └─ Summaries in collapse store

Layer 5 (Auto-Compact):
  └─ LLM semantic compression
  └─ Generate summary and write to REPL array

Collaboration example:
  1. Budget Reduction replaces large outputs → reduces individual message size
  2. Snip discards stale turns → reduces turn count
  3. Microcompact clears old results → reduces tool data size
  4. Context Collapse projects collapsed view → model sees compressed view
  5. Auto-Compact generates summary → last resort
```

### 2.7 Feature Flag and Dynamic Loading

```typescript
// Feature-gated loading in query.ts
// Uses dynamic require() due to bun:bundle tree-shaking constraints

// Wrong approach (would be tree-shaken away):
import { applyCollapsesIfNeeded } from "./contextCollapse";
if (feature("CONTEXT_COLLAPSE")) {
  messages = applyCollapsesIfNeeded(messages);
}

// Correct approach (dynamic require):
if (feature("CONTEXT_COLLAPSE")) {
  const { applyCollapsesIfNeeded } = require("./contextCollapse");
  messages = applyCollapsesIfNeeded(messages);
}
```

**Why use dynamic `require()`?**

- Bun's bundler performs tree-shaking at compile time
- `feature()` only works within if/ternary conditions
- Static `import` is analyzed by the bundler and may be removed
- Dynamic `require()` bypasses tree-shaking, ensuring the code is available at runtime

### 2.8 Practical Effect Estimation

```
Scenario: 100-turn conversation, averaging 2K tokens per turn

Original state:
  - 100 turns × 2K tokens = 200K tokens
  - Just reaches the 200K window limit
  - Cannot continue the conversation

After Context Collapse (assuming first 60 turns are collapsed):
  - Collapse summary: 60 turns → 1 summary (~2K tokens)
  - Non-collapsed messages: 40 turns × 2K tokens = 80K tokens
  - Total: 2K + 80K = 82K tokens
  - 59% reduction
  - Can continue the conversation

Key features:
  ✅ Full history still on disk (can resume/fork/audit)
  ✅ Model only sees the collapsed view (saves context)
  ✅ Collapse summaries are persisted (valid across turns)
  ✅ Collapses can be "expanded" (if history review is needed)
```

### 2.9 Details Not Provided in the Paper

1. **How collapse summaries are generated**
   - Using an LLM or rules?
   - What is the quality of the summaries?
   - The paper does not specify

2. **Collapse trigger conditions**
   - Turn threshold? Token threshold? Explicit command?
   - The paper does not specify

3. **Quality control of collapse summaries**
   - How is it ensured that summaries retain key information?
   - The paper does not specify

4. **How to "expand" collapses**
   - How does the user or system restore the full history view?
   - The paper does not specify

5. **Merging strategy for multiple collapses**
   - If there are multiple collapses, how are they combined?
   - The paper does not specify

---

## 3. Comparison Summary

| Dimension | Budget Reduction | Context Collapse |
|-----------|-----------------|------------------|
| **Layer** | Layer 1 | Layer 4 |
| **Cost** | Zero | Zero |
| **Feature Flag** | None (always enabled) | `CONTEXT_COLLAPSE` |
| **Modifies message array** | ✅ Yes | ❌ No |
| **Modifies underlying data** | ✅ Yes (replaces content) | ❌ No (pure projection) |
| **Operation granularity** | Individual messages | Turn ranges |
| **Preserved information** | Reference pointers (head/tail + size) | Collapse summaries |
| **Recoverability** | Restored via disk records | Full history always available |
| **Primary target** | Reduce individual oversized outputs | Reduce volume of long conversations |
| **Relationship with other layers** | Runs before Microcompact | Independent projection; does not interfere with other layers |
| **Persistence** | Writes to REPL array | collapse store (independent) |
| **User visibility** | Low (content is replaced) | Low ("operates without user-visible output") |

---

## 4. Implications for Helen

### 4.1 Insights from Budget Reduction

**Core idea**: Instead of deleting tool outputs, replace them with "reference pointers"

```python
# Simplified version Helen could implement

def budget_reduction(messages, max_chars=10000):
    """Budget Reduction: Replace oversized tool outputs"""
    for msg in messages:
        if msg.role == "tool" and len(msg.content) > max_chars:
            # Preserve head and tail
            head = msg.content[:200]
            tail = msg.content[-200:]
            # Replace with reference
            msg.content = f"[Tool result: {msg.tool_name}, {len(msg.content)} chars]\n"
            msg.content += f"First 200 chars: {head}...\n"
            msg.content += f"Last 200 chars: ...{tail}"
            msg._original_hash = hash(msg.content)  # For recovery
    return messages
```

**Key design decisions**:
1. Preserve head and tail (model can see the beginning and end of the file)
2. Preserve size information (model knows the file's scale)
3. Preserve tool_use_id (composes with Microcompact)

### 4.2 Insights from Context Collapse

**Core idea**: Do not modify underlying data; only project a view

```python
# Simplified version Helen could implement

class CollapseStore:
    """Collapse storage"""
    def __init__(self):
        self.collapses = []  # List[Collapse]

    def add_collapse(self, turn_range, summary):
        self.collapses.append({
            "turn_range": turn_range,
            "summary": summary,
        })

def context_collapse(messages, collapse_store):
    """Context Collapse: Project collapsed view"""
    collapsed_view = []

    # Add collapse summaries
    for collapse in collapse_store.collapses:
        start, end = collapse["turn_range"]
        collapsed_view.append({
            "role": "system",
            "content": f"[Conversation collapse: turns {start}-{end}]\n{collapse['summary']}",
        })

    # Add messages that are not collapsed
    for msg in messages:
        if not any(start <= msg.turn <= end for start, end in collapse_store.get_ranges()):
            collapsed_view.append(msg)

    return collapsed_view
```

**Key design decisions**:
1. Do not modify the `messages` list (underlying history)
2. Return a new `collapsed_view` (read-time projection)
3. Collapse summaries are in an independent `CollapseStore`

---

## 5. References

- arXiv:2604.14228v2 — "Dive into Claude Code: The Design Space of Today's and Future AI Agent Systems" (Liu et al., UCL, April 2026)
- Claude Code source v2.1.88 (inferred through paper analysis)
- query.ts:365-453 (compaction pipeline execution location)
- compact.ts (Auto-Compact implementation)
- sessionStorage.ts (session persistence)
