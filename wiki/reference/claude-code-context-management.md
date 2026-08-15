# Claude Code Context Management Technical Deep Dive

> Compiled from arXiv:2604.14228v2 (Liu et al., 2026) and Anthropic official documentation

**Date**: 2026-07-06
**Sources**:
- [arXiv:2604.14228v2 — "Dive into Claude Code"](https://arxiv.org/html/2604.14228v2)
- [Context Editing API](https://platform.claude.com/docs/en/build-with-claude/context-editing)
- [Context Windows](https://platform.claude.com/docs/en/build-with-claude/context-windows)

---

## 1. Overall Architecture Overview

Claude Code's context management consists of three independent but complementary systems:

```
┌──────────────────────────────────────────────────────────────┐
│                     Client (Claude Code)                      │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │  5-Layer Graduated Compaction Pipeline                 │  │
│  │  Location: query.ts:365-453                             │  │
│  │  Timing: Before every model call                       │  │
│  │                                                        │  │
│  │  Layer 1: Budget Reduction  (zero cost, always on)     │  │
│  │  Layer 2: Snip              (zero cost, feature flag)  │  │
│  │  Layer 3: Microcompact      (zero cost, cache-aware    │  │
│  │                                 path optional)         │  │
│  │  Layer 4: Context Collapse  (zero cost, read-time      │  │
│  │                                 projection)            │  │
│  │  Layer 5: Auto-Compact      (LLM call, last resort)   │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │  Additional Mechanisms                                  │  │
│  │  - Reactive Compaction (REACTIVE_COMPACT flag)          │  │
│  │  - Prompt-too-long recovery cascade                     │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│                     API Layer (Anthropic)                     │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │  Context Editing (server-side editing)                  │  │
│  │  Beta: context-management-2025-06-27                    │  │
│  │                                                        │  │
│  │  Strategy 1: clear_tool_uses_20250919                   │  │
│  │  Strategy 2: clear_thinking_20251015                    │  │
│  │  Strategy 3: compact_20260112                           │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │  Context Awareness                                      │  │
│  │  - Auto-injects token budget labels into system prompt  │  │
│  │  - Injects remaining capacity updates after each tool   │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │  Context Window                                         │  │
│  │  - 200K tokens (older models)                           │  │
│  │  - 1M tokens (Claude 4.6+ series)                       │  │
│  │  - Overflow behavior: Claude 4.5+ stops gracefully;     │  │
│  │    older models return errors                           │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
└──────────────────────────────────────────────────────────────┘
```

---

## 2. The 5-Layer Graduated Compaction Pipeline (Core)

### Design Philosophy: "Cheapest Move First"

From the original paper:

> "The graduated design reflects a lazy-degradation principle: apply the least disruptive compression first, escalating only when cheaper strategies prove insufficient."

The five layers are arranged from lowest to highest cost; each layer only activates when cheaper layers prove insufficient:

| Layer | Name | Inference Cost | Enabled State | Core Mechanism |
|-------|------|----------------|---------------|----------------|
| 1 | Budget Reduction | Zero (pure content replacement) | **Always enabled** | Large tool outputs → reference pointers |
| 2 | Snip | Zero (pure message trimming) | Feature flag `HISTORY_SNIP` | Discard stale history segments |
| 3 | Microcompact | Zero (structural operations) | Temporal path always on; cache path flag `CACHED_MICROCOMPACT` | Clear old tool results by tool_use_id |
| 4 | Context Collapse | Zero (read-time projection) | Feature flag `CONTEXT_COLLAPSE` | Project collapsed view at read time; does not modify underlying data |
| 5 | Auto-Compact | **One LLM inference** | Enabled by default; user can disable | LLM semantic summary |

### Execution Flow

```python
# query.ts:365-453 — Pipeline before every model call
def prepare_messages_for_query(messages_for_query):
    """Five-layer pipeline executes sequentially"""

    # Layer 1: Always runs — replace oversized tool outputs
    messages = apply_tool_result_budget(messages_for_query)

    # Layer 2: Feature flag controlled — trim stale history
    if feature("HISTORY_SNIP"):
        result = snip_compact_needed(messages)
        messages = result.messages
        snip_tokens_freed = result.tokens_freed  # Key: passed to auto-compact

    # Layer 3: Temporal path always runs; cache path flag controlled
    result = microcompact(messages)
    messages = result.messages
    compaction_info = result.compaction_info

    # Layer 4: Feature flag controlled — collapse view
    if feature("CONTEXT_COLLAPSE"):
        messages = apply_collapse_if_needed(messages)

    # Layer 5: Last resort — only triggers when all four prior layers are insufficient
    if context_still_exceeds_pressure_threshold(messages):
        messages = compact_conversation(messages)  # LLM call

    return messages
```

**Key design details**:
- Layer 1 runs before Layer 3 because Layer 1 operates at the content level while Layer 3 operates by ID — the two compose without conflict
- The token count freed by Layer 2 (`snip_tokens_freed`) is explicitly passed to Layer 5, because token counters are inferred from the `usage` field of the last assistant message, and snip does not modify that message
- Layer 4 is the only layer that does not modify the message array — it is a pure read-time projection

---

### Layer 1: Budget Reduction

**Function**: `applyToolResultBudget()`
**Cost**: Zero inference cost (pure content replacement)
**Status**: Always enabled, no feature flag

#### Mechanism

For each tool result message, check whether its size exceeds the `maxResultSizeChars` limit:
- Exceeds → Replace with a "content reference" (content reference / pointer)
- Does not exceed → Keep as-is

#### Exempted Tools

Tools whose `maxResultSizeChars` is a **non-finite value** (`Infinity` or unset) are not subject to budget reduction.

#### Persistence

Content replacements are persisted to agent and session query sources so they can be reconstructed on resume.

#### Relationship with Microcompact

Budget Reduction runs before Microcompact because:
- Budget Reduction = **content-level** operation (checks tool result content size)
- Microcompact = **structural-level** operation (identifies pairs to compress by tool_use_id, without inspecting content)

#### Details Not Provided in the Paper
- The specific value of `maxResultSizeChars`
- The exact format of "content references"

---

### Layer 2: Snip

**Function**: `snipCompactIfNeeded()`
**Cost**: Zero inference cost (pure message trimming)
**Status**: Feature flag `HISTORY_SNIP`

#### Mechanism

Lightweight trimming that removes older history segments. Addresses the "temporal depth" problem — stale turns that accumulate as the conversation progresses.

#### Return Value

```python
{
    "messages": [...],           # The trimmed message list
    "tokensFreed": int,          # Number of tokens freed
    "boundaryMessage": Message,  # Boundary message (marks the snip point)
}
```

#### Critical Pipeline Handoff

The `snipTokensFreed` value is explicitly passed to auto-compact. The reason:
- The main token counter infers context size from the `usage` field of the most recent assistant message
- Snip does not modify that message, so its original `input_tokens` remain attached
- Without explicit handoff, snip's savings would be invisible to the counter

#### What Makes a Turn "Stale"

The paper does not provide a precise algorithm or threshold. It only says snip "removes older history segments."

---

### Layer 3: Microcompact — Core Innovation

**Cost**: Zero inference cost
**Status**: Temporal path always enabled; cache-aware path controlled by `CACHED_MICROCOMPACT` flag

#### Two Sub-paths

1. **Temporal path** (always runs) — time-based compression
2. **Cache-aware path** (flag controlled) — uses actual cache deletion data returned by the API

#### Core Mechanism: Operates by tool_use_id

> "Microcompact operates purely by `tool_use_id` and **never inspects content**."

This is Microcompact's key structural insight:
- Identifies which tool_use/tool_result pairs to compress via tool_use_id
- **Does not inspect the content of tool results**
- Operates at a different level than Budget Reduction, with no conflict

#### Subtle Design of the Cache-Aware Path

When the cache-aware path is enabled:
- **Boundary messages are deferred until after the API response**
- Uses the API's **actual `cache_deleted_input_tokens`** rather than estimated values
- The system does not guess how much cache was freed — it reads the actual number from the response

#### Return Value

```python
{
    "messages": [...],
    "compactionInfo": {
        "pendingCacheEdits": [...]  # Pending edits for the cache-aware path
    }
}
```

#### Inferred Handling of tool_use vs tool_result

Although the paper does not explicitly describe block-level transformation, it can be inferred:
- **tool_use blocks** (tool call decisions in assistant messages) → Preserved
- **tool_result blocks** (tool return data in user messages) → Replaced with placeholder text

This means the model "remembers what it decided to do" (tool_use) but "does not remember what the tool returned" (tool_result).

---

### Layer 4: Context Collapse

**Function**: `applyCollapsesIfNeeded()`
**Cost**: Zero inference cost
**Status**: Feature flag `CONTEXT_COLLAPSE`

#### Architectural Uniqueness: Pure Read-Time Projection

> "Nothing is yielded; the collapsed view is a read-time projection over the REPL's full history. Summary messages live in the collapse store, not the REPL array. This is what makes collapses persist across turns."

**Fundamental difference from the other four layers**:
| Layer | Modifies Message Array | Underlying Data |
|-------|------------------------|-----------------|
| Budget Reduction | ✅ Modifies | Modified |
| Snip | ✅ Modifies | Modified |
| Microcompact | ✅ Modifies | Modified |
| **Context Collapse** | ❌ Does not modify | **Not modified** |
| Auto-Compact | ✅ Modifies | Modified |

Context Collapse is the only layer that does not modify the underlying data.

#### How It Works

```
Underlying Storage (REPL array)        Read-Time View (messagesForQuery)
┌─────────────────────┐     ┌─────────────────────────┐
│ Turn 1: user+asst   │     │                         │
│ Turn 2: user+asst   │ ──→ │ [Collapse summary:      │
│ Turn 3: user+asst   │     │  first N turns]          │
│ Turn 4: user+asst   │     │ Turn N-2: user+asst     │
│ Turn 5: user+asst   │     │ Turn N-1: user+asst     │
└─────────────────────┘     │ Turn N:   user+asst     │
                            └─────────────────────────┘
Full history is never modified        Model only sees the collapsed view
```

#### Storage

- Summary messages are stored in a separate **"collapse store"**
- Collapses persist across turns (because they live in the store, not a temporary array)
- Invisible to users — "operates without user-visible output"

---

### Layer 5: Auto-Compact — Last Resort

**Function**: `compactConversation()` (compact.ts)
**Cost**: One full LLM inference call
**Status**: Enabled by default; user can configure off

#### Trigger Condition

> "Auto-compact fires **only when the context still exceeds the pressure threshold** after all four previous shapers have run."

In other words, LLM compression is triggered only when the first four layers are all insufficient.

#### LLM Summary Flow

```python
def compact_conversation(messages):
    # 1. PreCompact hooks fire first — allow hooks to inject custom instructions
    hook_results = run_pre_compact_hooks()

    # 2. Create summary request
    summary_prompt = get_compact_prompt()  # Summary prompt

    # 3. Call LLM to generate compressed summary (one full inference call)
    summary_response = llm_call(
        messages=build_summary_messages(messages),
        prompt=summary_prompt,
    )

    # 4. Build post-compaction messages
    return build_post_compact_messages(
        boundary_marker,       # Compression boundary marker
        summary_messages,      # Summary messages
        messages_to_keep,      # Most recent messages retained
        attachments,           # Runtime state attachments
        hook_results,          # Hook results
    )
```

#### The "mostly-append" Design of Boundary Markers

```python
# build_post_compact_messages returns:
[
    boundary_marker,       # Compression boundary, with preserved-segment metadata
    ...summary_messages,   # LLM-generated summary
    ...messages_to_keep,   # Retained recent messages
    ...attachments,        # Runtime state (plans, skills, agents)
    ...hook_results,       # Hook-injected content
]

# Boundary marker carries metadata:
boundary_marker.metadata = {
    "headUuid": "...",     # UUID of the head preserved segment
    "anchorUuid": "...",   # Anchor UUID
    "tailUuid": "...",     # UUID of the tail preserved segment
}
```

These UUIDs enable the session loader to **repair message links at read time**:
- Preserved messages retain their original `parentUuids`
- The loader uses boundary metadata to link correctly

**Key design principle**: Compaction typically **does not modify or delete previously written transcript lines** — it only appends new boundary and summary events.

#### Runtime State Reconstruction After Compaction

Compaction discards previous attachment messages but does not discard the underlying state. Therefore:
- After compaction, attachment builders republish from **live app state** (plans, skills, async agents)
- Ensures the model is aware of current in-progress work

#### Cache Behavior (Experimental Data)

A GrowthBook feature flag controls whether the compaction path reuses the main conversation's prompt cache:

```
Experiment (January 2026):
- "false path" (no cache reuse) → 98% cache miss
- But only consumed ~0.76% of fleet cache_creation tokens
```

#### Details Not Provided in the Paper
- The exact "pressure threshold" value
- The exact content of `getCompactPrompt()`
- The model used for the summary call
- The exact token target for the summary

---

## 3. Additional Compression Mechanisms

### 3.1 Reactive Compaction

**Feature flag**: `REACTIVE_COMPACT`

```
Trigger condition: During a turn's execution, context approaches capacity
Behavior: Summarizes only enough content to free space
Limitation: hasAttemptedReactiveCompact flag ensures at most one trigger per turn
```

### 3.2 Prompt-too-long Recovery Cascade

When the API returns a `prompt_too_long` error:

```
Step 1: Attempt context-collapse overflow recovery
Step 2: If that fails → attempt reactive compaction
Step 3: If still failing → terminate, reason: 'prompt_too_long'
```

---

## 4. Server-Side Context Editing API

### 4.1 API Overview

```
Beta header: context-management-2025-06-27
Runs at: Server side (API side), applied before the prompt reaches Claude
Client state: Unmodified — the client maintains the full unmodified conversation history
```

**Core principle**: Context editing is **server-side applied**. The client application maintains the full unmodified conversation history and does not need to synchronize with the edited version.

### 4.2 Strategy 1: Tool Result Clearing (clear_tool_uses_20250919)

#### Parameter Details

| Parameter | Default | Description |
|-----------|---------|-------------|
| `trigger` | 100,000 input_tokens | Strategy activation threshold. Can be `input_tokens` or `tool_uses` type |
| `keep` | 3 tool_uses | Number of recent tool call/result pairs to retain after clearing. The API removes the oldest in chronological order |
| `clear_at_least` | None | Ensures each activation clears at least this many tokens. If it cannot clear at least this amount, the strategy **is not applied** |
| `exclude_tools` | None | List of tool names that are never cleared. Protects important context |
| `clear_tool_inputs` | false | Whether to also clear tool call parameters. By default only clears results, keeping Claude's tool calls visible |

#### Behavioral Details

```
On activation:
1. API clears the oldest tool results in chronological order
2. Each cleared result is replaced with placeholder text so Claude knows it was removed
3. By default preserves Claude's tool_use blocks (tool call decisions)
4. If clear_tool_inputs=true, also clears tool call parameters

What is preserved:
✅ System prompt — never cleared
✅ User messages — never cleared
✅ Assistant text — never cleared
✅ tool_use blocks (default) — Claude's decision records

What is cleared:
❌ Old tool_result content — raw data returned by tools
❌ (optional) tool_use parameters — tool call arguments
```

#### Code Examples

```python
# Basic usage
response = client.beta.messages.create(
    model="claude-opus-4-8",
    max_tokens=4096,
    messages=[{"role": "user", "content": "Search for the latest AI advances"}],
    tools=[{"type": "web_search_20250305", "name": "web_search"}],
    betas=["context-management-2025-06-27"],
    context_management={
        "edits": [{"type": "clear_tool_uses_20250919"}]
    },
)

# Advanced configuration
response = client.beta.messages.create(
    model="claude-opus-4-8",
    max_tokens=4096,
    messages=[{"role": "user", "content": "Create a Python calculator"}],
    tools=[
        {"type": "text_editor_20250728", "name": "str_replace_based_edit_tool"},
        {"type": "web_search_20250305", "name": "web_search"},
    ],
    betas=["context-management-2025-06-27"],
    context_management={
        "edits": [{
            "type": "clear_tool_uses_20250919",
            "trigger": {"type": "input_tokens", "value": 30000},
            "keep": {"type": "tool_uses", "value": 3},
            "clear_at_least": {"type": "input_tokens", "value": 5000},
            "exclude_tools": ["web_search"],  # web_search results are never cleared
        }]
    },
)
```

#### Cache Behavior

```
Clearing tool results → invalidates cached prompt prefix
Each clear → incurs cache write cost
Subsequent requests → can reuse the newly cached prefix

Best practice:
- Use clear_at_least to ensure each clearing removes enough tokens
  to justify the cache invalidation cost
- Otherwise, frequent small clearings will continuously invalidate the cache
```

### 4.3 Strategy 2: Thinking Block Clearing (clear_thinking_20251015)

#### Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `keep` | Model-specific | Number of recent assistant turns with thinking blocks to retain |

**keep format**:
```python
{"type": "thinking_turns", "value": 2}  # Keep the most recent 2 turns of thinking
"all"                                    # Keep all thinking blocks (maximize cache hits)
```

**Default behavior per model class**:

| Model Class | Keeps All Thinking | Only Keeps Last Turn |
|-------------|--------------------|---------------------|
| Opus | 4.5+ | 4.1 and below |
| Sonnet | 4.6+ | 4.5 and below |
| Haiku | (none) | All models |

#### Cache Behavior

```
Keeping thinking blocks → cache remains valid ✅
Clearing thinking blocks → cache is invalidated at the clearing point ❌

Trade-off when choosing the keep parameter:
- More thinking blocks = more reasoning continuity = better cache
- Fewer thinking blocks = more context space
```

#### Code Examples

```python
# Keep the most recent 2 turns of thinking
response = client.beta.messages.create(
    model="claude-opus-4-8",
    max_tokens=16000,
    messages=[{"role": "user", "content": "Hello"}],
    thinking={"type": "adaptive"},
    betas=["context-management-2025-06-27"],
    context_management={
        "edits": [{
            "type": "clear_thinking_20251015",
            "keep": {"type": "thinking_turns", "value": 2},
        }]
    },
)

# Combining two strategies (note ordering: thinking must come first)
context_management={
    "edits": [
        {
            "type": "clear_thinking_20251015",          # ← must be first
            "keep": {"type": "thinking_turns", "value": 2},
        },
        {
            "type": "clear_tool_uses_20250919",
            "trigger": {"type": "input_tokens", "value": 50000},
            "keep": {"type": "tool_uses", "value": 5},
        },
    ]
}
```

### 4.4 Response Format

```json
{
  "id": "msg_013Zva2CMHLNnXjNJJKqJ2EF",
  "role": "assistant",
  "content": [...],
  "usage": {...},
  "context_management": {
    "applied_edits": [
      {
        "type": "clear_thinking_20251015",
        "cleared_thinking_turns": 3,
        "cleared_input_tokens": 15000
      },
      {
        "type": "clear_tool_uses_20250919",
        "cleared_tool_uses": 8,
        "cleared_input_tokens": 50000
      }
    ]
  }
}
```

**Special response for the Token counting endpoint**:
```json
{
  "input_tokens": 25000,
  "context_management": {
    "original_input_tokens": 70000
  }
}
// Shows 70K before clearing → 25K after clearing, saving 45K tokens
```

### 4.5 Strategy 3: Server-Side Compaction (compact_20260112)

**Anthropic's recommendation**: Server-side compaction is the **preferred strategy** for managing long conversations, superseding the SDK-side `compaction_control` (deprecated).

#### Basic Information

| Item | Description |
|------|-------------|
| Beta header | `compact-2026-01-12` |
| Strategy type | `compact_20260112` |
| Runs at | Server side (API side) |
| Client state | Client maintains full history; API handles compaction automatically |

#### How It Works

```
1. Client sends a request with context_management.edits configuration
2. API monitors input_tokens and triggers compaction when threshold is reached
3. Claude compresses old conversation history into a structured summary
4. Response includes a type="compaction" content block
5. Subsequent requests append the response to the message list; the API automatically discards content before the compaction block
```

**Key features**:
- ✅ No client-side compaction code needed
- ✅ Automatic token counting
- ✅ Multiple compactions supported (same conversation can trigger multiple times)
- ✅ Compatible with server tools (web_search, etc.)

#### Parameter Details

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `trigger` | object | Required | Trigger condition. Currently only supports `{"type": "input_tokens", "value": N}` |
| `trigger.type` | string | - | Must be `"input_tokens"` |
| `trigger.value` | int | - | Trigger threshold (input token count) |
| `pause_after_compaction` | bool | false | Whether to pause the response after generating the summary |
| `custom_instructions` | string | Model-specific default prompt | Custom summary prompt; **completely replaces** the default prompt |

#### Basic Usage Example

```python
# Python
client = anthropic.Anthropic()

response = client.beta.messages.create(
    betas=["compact-2026-01-12"],
    model="claude-opus-4-8",
    max_tokens=4096,
    messages=[{"role": "user", "content": "Hello, Claude"}],
    context_management={
        "edits": [
            {
                "type": "compact_20260112",
                "trigger": {"type": "input_tokens", "value": 150000},
            }
        ]
    },
)
```

```bash
# cURL
curl https://api.anthropic.com/v1/messages \
  -H "x-api-key: $ANTHROPIC_API_KEY" \
  -H "anthropic-version: 2023-06-01" \
  -H "anthropic-beta: compact-2026-01-12" \
  -H "content-type: application/json" \
  -d '{
    "model": "claude-opus-4-8",
    "max_tokens": 4096,
    "messages": [{"role": "user", "content": "Hello"}],
    "context_management": {
      "edits": [{
        "type": "compact_20260112",
        "trigger": {"type": "input_tokens", "value": 150000}
      }]
    }
  }'
```

#### Full Long Conversation Example

```python
client = anthropic.Anthropic()
messages: list[dict] = []

def chat(user_message: str) -> str:
    messages.append({"role": "user", "content": user_message})

    response = client.beta.messages.create(
        betas=["compact-2026-01-12"],
        model="claude-opus-4-8",
        max_tokens=4096,
        messages=messages,
        context_management={
            "edits": [{
                "type": "compact_20260112",
                "trigger": {"type": "input_tokens", "value": 100000},
            }]
        },
    )

    # Append response (which may include compaction block) to message list
    messages.append({"role": "assistant", "content": response.content})
    return response.content[0].text if response.content else ""

# Can call chat() indefinitely; the API handles compression automatically
chat("Help me build a Python web scraper")
chat("Add support for JavaScript-rendered pages")
chat("Now add rate limiting and error handling")
# ... continue for hundreds of turns
```

#### pause_after_compaction Mode

Enabling `pause_after_compaction` allows inserting additional messages after compaction (e.g., preserving recent exchanges):

```python
response = client.beta.messages.create(
    betas=["compact-2026-01-12"],
    model="claude-opus-4-8",
    max_tokens=4096,
    messages=messages,
    context_management={
        "edits": [{
            "type": "compact_20260112",
            "trigger": {"type": "input_tokens", "value": 100000},
            "pause_after_compaction": True,
        }]
    },
)

# Check whether compaction pause was triggered
if response.stop_reason == "compaction":
    # Response contains only the compaction block
    compaction_block = response.content[0]

    # Preserve the most recent exchange (e.g., last 3 messages)
    preserved = messages[-3:]

    # Build new message list: compaction + preserved messages
    messages = [
        {"role": "assistant", "content": [compaction_block]},
        *preserved,
    ]

    # Continue requesting
    response = client.beta.messages.create(
        betas=["compact-2026-01-12"],
        model="claude-opus-4-8",
        max_tokens=4096,
        messages=messages,
        context_management={
            "edits": [{
                "type": "compact_20260112",
                "trigger": {"type": "input_tokens", "value": 100000},
            }]
        },
    )
```

#### Custom Summary Prompt

```python
response = client.beta.messages.create(
    betas=["compact-2026-01-12"],
    model="claude-opus-4-8",
    max_tokens=4096,
    messages=messages,
    context_management={
        "edits": [{
            "type": "compact_20260112",
            "trigger": {"type": "input_tokens", "value": 100000},
            "custom_instructions": "Focus on preserving code snippets, variable names, and technical decisions.",
        }]
    },
)
```

**Note**: `custom_instructions` **completely replaces** the default prompt, not supplements it.

#### Token Budget Tracking

Long tasks can combine `trigger` and compaction counters to estimate total consumption:

```python
TRIGGER_THRESHOLD = 100_000
TOTAL_TOKEN_BUDGET = 3_000_000
n_compactions = 0

response = client.beta.messages.create(
    betas=["compact-2026-01-12"],
    model="claude-opus-4-8",
    max_tokens=4096,
    messages=messages,
    context_management={
        "edits": [{
            "type": "compact_20260112",
            "trigger": {"type": "input_tokens", "value": TRIGGER_THRESHOLD},
            "pause_after_compaction": True,
        }]
    },
)

# Track compaction count to estimate total consumption
if response.stop_reason == "compaction":
    n_compactions += 1
    estimated_total = n_compactions * TRIGGER_THRESHOLD
    if estimated_total >= TOTAL_TOKEN_BUDGET:
        # Budget exhausted; ask the model to wrap up
        messages.append({
            "role": "user",
            "content": "Please wrap up your current work and summarize the final state."
        })
```

#### Response Format

When compaction is triggered, the response includes a content block of type `compaction`:

```json
{
  "id": "msg_123",
  "type": "message",
  "role": "assistant",
  "content": [
    {
      "type": "compaction",
      "summary": "The user requested help building a web scraper..."
    }
  ],
  "stop_reason": "compaction",  // or "end_turn" if compaction was not triggered
  "usage": {
    "input_tokens": 150000,
    "output_tokens": 500
  },
  "context_management": {
    "original_input_tokens": 180000
  }
}
```

#### Cache Behavior

```
After compaction:
- The summary is new content and requires a cache write
- Without a cache breakpoint, the system prompt cache is also invalidated

Best practice:
- Set a cache breakpoint at the end of the system prompt
- The system prompt remains independently cached
- On compaction, only a new summary cache entry needs to be written
```

#### Billing and Limits

- Compaction requires an additional sampling step, counted toward rate limits and billing
- The `usage` array in the response shows usage for each sampling step
- Re-applying a previous `compaction` block **incurs no additional cost**

#### Working with Server Tools

When using server tools (web_search, etc.):
- Compaction trigger check occurs at the start of each sampling iteration
- A single request may trigger multiple compactions
- The SDK may miscalculate token usage (includes cumulative reads from server tool internal calls)

**Recommendation**: When using server tools, avoid client-side compaction; prefer server-side compaction.

#### Known Issues

The model occasionally calls tools instead of writing a summary during the internal summarization step. Workaround:

```python
"custom_instructions": "Include relevant information in the summary for continuing the task in the next context window. Do not call any tools while writing this summary; respond with text only."
```

#### Supported Models

- Claude Opus 4.x series
- Claude Sonnet 4.x series
- Uses the model from the request for summarization (**cannot use a cheaper model**)

---

### 4.6 SDK Compaction (Deprecated)

**Anthropic strongly recommends server-side compaction; SDK compaction is deprecated.**

The SDK's `compaction_control` parameter is deprecated in the Python, TypeScript, and Ruby SDKs and will be removed in a future version. The SDK emits a deprecation warning when enabled.

**Problems with SDK compaction**:
- Runs client-side; requires additional integration code
- Token usage calculation is inaccurate (especially with server tools)
- Client-side limitations

**Only use SDK compaction when you need client-side control over the summarization process.**

---

### 4.7 Strategy Selection Guide

| Scenario | Recommended Strategy |
|----------|---------------------|
| Long conversations (>100k tokens) | `compact_20260112` (server-side compaction) |
| Tool-intensive workflows | `clear_tool_uses_20250919` |
| Using extended thinking | `clear_thinking_20251015` |
| Need fine-grained control | Combine multiple strategies |
| Need to maintain reasoning continuity | `clear_thinking` + retain recent thinking |

**Strategy combination example**:
```python
context_management={
    "edits": [
        {
            "type": "clear_thinking_20251015",  # must be first
            "keep": {"type": "thinking_turns", "value": 2},
        },
        {
            "type": "clear_tool_uses_20250919",
            "trigger": {"type": "input_tokens", "value": 50000},
            "keep": {"type": "tool_uses", "value": 5},
            "exclude_tools": ["web_search"],
        },
        {
            "type": "compact_20260112",
            "trigger": {"type": "input_tokens", "value": 150000},
        },
    ]
}
```

**Note**: `clear_thinking` must come before other strategies.

## 5. Context Awareness

### 5.1 Auto-Injection Mechanism

The Anthropic API automatically injects context awareness labels for supported models:

**Budget label in the system prompt**:
```xml
<budget:token_budget>200000</budget:token_budget>
```

**Updates after each tool call**:
```xml
<system_warning>Token usage: 35000/200000; 165000 remaining</system_warning>
```

### 5.2 Supported Models

| Model | Budget Value |
|-------|-------------|
| Claude Sonnet 5, Sonnet 4.6 | 1M tokens |
| Claude Sonnet 4.5, Haiku 4.5 | 200K tokens |

### 5.3 Updated Model Behavior

Claude Opus 4.7+, Fable 5, Mythos 5 **no longer receive these injected labels**; instead, they can use task budgets (beta feature) to explicitly set budgets.

---

## 6. Context Window Management

### 6.1 Window Sizes

| Model | Window Size | Max Output |
|-------|------------|------------|
| Claude Opus 4.8/4.7/4.6 | 1M | — |
| Claude Sonnet 5/4.6 | 1M | — |
| Claude Fable 5 / Mythos 5 | 1M | 128K tokens |
| Claude Sonnet 4.5 | 200K | — |
| Other models | 200K | — |

### 6.2 What Counts Toward the Context Window

```
What counts:
✅ System prompt
✅ Each message in the messages array
✅ Tool results, images, documents
✅ Tool definitions
✅ Claude-generated output (including extended thinking)
✅ All conversation history

⚠️ Prompt cache tokens also count toward the window
   (cache only affects cost, not counting)
```

### 6.3 Overflow Behavior

```
Case 1: Input only exceeds the window
  → All models return 400 error: "prompt is too long"

Case 2: Input + max_tokens exceeds the window
  Claude 4.5+ and newer models:
    → API accepts the request; generates until the limit then stops
    → stop_reason: "model_context_window_exceeded"
  Older models:
    → Returns a validation error
    → New behavior can be enabled via beta header:
      model-context-window-exceeded-2025-08-26
```

### 6.4 Extended Thinking and Context

**Models that retain thinking blocks** (default):
- Opus 4.5+, Sonnet 4.6+, Fable 5, Mythos 5, Mythos Preview
- Previous thinking blocks are billed as input tokens

**Models that automatically strip thinking blocks** (default):
- Earlier Opus/Sonnet models, all Haiku models
- The API automatically strips them from conversation history

**Key requirement**: When returning tool results, you must include the **complete unmodified thinking block** (including the cryptographic signature). Modifying thinking blocks causes API errors.

---

## 7. Key Design Principles Summary

### 7.1 "Actions > Data" Principle

```
Claude Code's core insight:

✅ Preserve tool_use blocks — "what the LLM decided to do"
❌ Clear tool_result content — "what the tool returned"

Rationale:
- Raw tool output (file contents, search results) is large and usually no longer needed after processing
- The LLM's tool call decisions record the reasoning path and have reference value for future decisions
- Use 20% of tokens to preserve 80% of decision context
```

### 7.2 "Cheapest Move First" Principle

```
Cost ladder:
  Zero cost → Content replacement (Layer 1)
  Zero cost → Message trimming (Layer 2)
  Zero cost → Structural compression (Layer 3)
  Zero cost → Read-time projection (Layer 4)
  LLM call → Semantic compression (Layer 5)  ← only when all prior layers are insufficient

Each layer only activates when cheaper layers prove insufficient
```

### 7.3 "mostly-append" Persistence Principle

```
Compaction does not modify or delete previous transcript lines
Only appends new boundary and summary events
Preserved messages retain their original parentUuids
At read time, boundary metadata repairs the message chain
```

### 7.4 "Cache-Aware" Principle

```
Microcompact cache path:
- Defers boundary messages until after the API response
- Uses actual cache_deleted_input_tokens
- Does not guess; reads from the response

Cache invalidation cost management:
- clear_at_least ensures each clearing removes enough tokens
- Avoids frequent small clearings that continuously invalidate the cache
```

### 7.5 "Context Quality > Quantity" Principle

```
From Anthropic documentation:

"Context is a finite resource with diminishing returns —
 irrelevant content degrades model focus."

"More context isn't automatically better.
 As token count grows, accuracy and recall degrade
 (phenomenon known as 'context rot')."

"Curating what's in context is just as important
 as how much space is available."
```

---

## 8. Quantitative Data

### Performance Data

| Metric | Value | Source |
|--------|-------|--------|
| Performance improvement with context editing enabled | **29%** | Anthropic report |
| Impact of cleanup code on Claude Code | Token reduction **7-8%**, file review reduction **34%** | Controlled experiment |
| Prompt cache expiration | Expires after **5 minutes** of inactivity | Paper KAIROS section |
| Fleet cache cost (non-reuse path) | **~0.76%** of fleet cache_creation | January 2026 experiment |
| Non-reuse path cache miss rate | **98%** | Same as above |
| Auto-approval rate trajectory | <50 sessions **~20%** → 750 sessions **>40%** | Longitudinal usage data |
| 100-step task success rate at 95% per-step accuracy | **0.6%** | Cited research |

### Compression Effect

```
Typical scenario: 50-turn conversation with heavy tool usage

Graduated compression flow:
  Original: ~200K tokens (near the 200K window)
  Layer 1: Replace large tool outputs → ~160K (20% reduction)
  Layer 2: Discard stale turns → ~130K (35% reduction)
  Layer 3: Clear old tool results → ~80K (60% reduction)
           Preserve all tool_use decisions
  Layer 4: Collapse view → ~60K (70% reduction)
  Layer 5: LLM compaction → ~40K (80% reduction)
           Semantic summary preserves key information

Typical recovery: 60-70% of the context window
Next compaction trigger: At 60% of new available capacity
```

---

## 9. Correspondence with Helen

| Claude Code Concept | Helen Equivalent | Gap |
|--------------------|-----------------|-----|
| 5-layer graduated compaction | 1 layer (80% trigger) | Missing 4 layers |
| Microcompact (clear tool results by ID) | None | Core missing |
| Context Collapse (read-time projection) | None | Novel concept |
| Auto-Compact (LLM semantic compaction) | "summarize" (just concatenation) | Not true LLM summary |
| Reactive Compaction | None | Missing |
| Context Editing API | None | Missing server-side editing |
| Context Awareness (token labels) | None | Model does not know remaining capacity |
| Tool result clearing strategy | Truncate to 16K | Coarser |
| Thinking block clearing strategy | No extended thinking | N/A |
| mostly-append persistence | None | Missing |
| Cache-aware compaction | None | Missing |
| Boundary marker UUID chain repair | None | Missing |
| "Actions > Data" distinction | No differentiated treatment of messages | Core missing |
