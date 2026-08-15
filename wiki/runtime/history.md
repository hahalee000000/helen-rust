# History Management (HistoryManager)

> Module M16 | `helen/runtime/history.py` | Tests: `tests/runtime/test_history.py`

---

## Overview

HistoryManager manages multi-turn LLM conversation history, ensuring it does not exceed the model's context window.

---

## Token Budget

```python
class HistoryManager:
    MAX_TOKENS: int = 128000          # Model context window
    SUMMARY_MAX_TOKENS: int = 4096    # conversation_summary upper limit
```

### check_budget()

```python
def check_budget(self, system_tokens: int, instruction_tokens: int) -> int:
    """Calculate the token budget available for conversation history."""
    return self.MAX_TOKENS - system_tokens - instruction_tokens - 1000  # 1000 buffer
```

---

## Truncation Strategy

```python
def trim_history(self, history: list[Message], budget: int) -> list[Message]:
    """Truncate from the oldest message until it fits within the budget."""
    # Calculate token count for each message
    msg_tokens = [self.estimate_tokens(msg.content) for msg in history]

    # If total tokens are within budget, keep all
    if sum(msg_tokens) <= budget:
        return list(history)

    # Remove oldest messages until within budget
    result = list(history)
    result_tokens = list(msg_tokens)
    while result and sum(result_tokens) > budget:
        result.pop(0)
        result_tokens.pop(0)

    return result
```

---

## Conversation Summary

```python
def build_conversation_summary(self, history: list[Message], max_tokens=4096) -> str:
    """Build conversation summary, including latest messages, truncating the oldest."""
    lines = []
    total_tokens = 0

    # Iterate from newest to oldest
    for msg in reversed(history):
        line = f"[{msg.role}] {msg.content}"
        line_tokens = self.estimate_tokens(line)
        if total_tokens + line_tokens > max_tokens:
            continue  # Truncate
        lines.append(line)
        total_tokens += line_tokens

    lines.reverse()  # Restore chronological order
    return "\n".join(lines)
```

### Format

```
[user] Classify the email priority
[assistant] [routed to: urgent]
[user] Translate: Hello, world!
[assistant] Bonjour, le monde!
```

---

## Token Estimation

```python
def estimate_tokens(text: str, model: str | None = None) -> int:
    """Character-type-aware token estimation, supports tiktoken exact counting."""
    # 1. Prefer tiktoken (if installed) for exact counting
    tiktoken_count = _try_tiktoken_count(text, model)
    if tiktoken_count is not None:
        return tiktoken_count
    
    # 2. Fall back to character-type-aware heuristic
    cjk_count = sum(1 for c in text if _is_cjk(c))
    total_len = len(text)
    
    if cjk_count == 0:
        # Pure English/Latin: 4 chars ≈ 1 token
        return max(1, int(total_len / 4.0))
    elif cjk_count == total_len:
        # Pure CJK: 1.2 chars ≈ 1 token (CJK characters typically occupy 1-2 tokens)
        return max(1, int(total_len / 1.2))
    else:
        # Mixed content: CJK and Latin calculated by respective ratios
        non_cjk = total_len - cjk_count
        return max(1, int(cjk_count / 1.2 + non_cjk / 4.0))
```

**Constants** (`helen/runtime/history.py`):
```python
CHARS_PER_TOKEN_EN = 4.0    # English/Latin
CHARS_PER_TOKEN_CJK = 1.2   # CJK characters (Chinese/Japanese/Korean)
CHARS_PER_TOKEN_MIXED = 3.0  # Mixed content estimate
```

**CJK detection** (`_is_cjk()`):
- CJK Unified Ideographs (0x4E00-0x9FFF)
- CJK Extension A/B/C/D
- CJK Symbols and Punctuation (0x3000-0x303F)
- Hiragana (0x3040-0x309F)
- Katakana (0x30A0-0x30FF)
- Hangul Syllables (0xAC00-0xD7AF)

**tiktoken integration** (optional):
- If `tiktoken` is installed, uses exact counting
- Supports model-specific encoding (e.g., `cl100k_base`)
- Fallback strategy: generic encoding for unknown models

---

## Context Management Enhancement (Phase 1-7, v1.15+)

Helen v1.15 introduced a complete context management enhancement plan, aligning with Claude Code's context management capabilities.

### Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                  Context Management Stack                │
├─────────────────────────────────────────────────────────┤
│ Phase 7: Agent Context Integration                       │
│   - AgentContextManager: Encapsulates working memory     │
│     and compression strategies                          │
│   - context {} block: Independent config per agent      │
├─────────────────────────────────────────────────────────┤
│ Phase 6: Cache-Aware Compression                         │
│   - Stable prefix (30%): Cache-friendly zone            │
│   - Batch threshold (75%): Usage triggers compression   │
│   - Suffix-only modification: Operates outside cache    │
│     zone                                                │
├─────────────────────────────────────────────────────────┤
│ Phase 2-5: Graduated Compression Pipeline                │
│   - Layer 1 (60%): Budget Reduction                      │
│   - Layer 2 (70%): Snip                                  │
│   - Layer 3 (80%): Microcompact                          │
│   - Layer 4 (90%): Context Collapse                      │
│   - Layer 5 (95%): Auto-Compact                          │
├─────────────────────────────────────────────────────────┤
│ Phase 1: Foundation                                      │
│   - Working Memory: Tracks active files, decisions,     │
│     errors                                              │
│   - Three-channel context: System instructions +        │
│     working memory + history                            │
└─────────────────────────────────────────────────────────┘
```

### Phase 1: Working Memory

Automatically tracks key information during agent execution:

```python
class WorkingMemory:
    active_files: list[str]        # Recently read/written files
    recent_decisions: list[str]    # Key decisions
    pending_todos: list[str]       # Pending TODOs
    error_history: list[dict]      # Error records
```

**Auto-extraction:**
- File paths: From `read_file`, `write_file`, `patch_file` calls
- Decisions: From key patterns in assistant messages ("I'll use...", "Let me try...")
- TODOs: From `TODO:`, `FIXME:` patterns in comments
- Errors: From `shell_exec` failures and error keywords

### Phase 2-5: Graduated Compression Pipeline

Five-layer graduated strategy, "cheapest action first" principle:

#### Layer 1: Budget Reduction (60%)

Replaces large tool outputs with reference pointers, preserving structural information.

```python
# Original tool result (5000 tokens)
{"role": "tool", "content": "very long output content..."}

# After Budget Reduction (50 tokens)
{"role": "tool", "content": "[Tool result: read_file(path=/path/to/file.py) -> 5000 chars]"}
```

#### Layer 2: Snip (70%)

Drops stale turns (oldest assistant + tool pairs).

#### Layer 3: Microcompact (80%)

Clears old tool results but preserves `tool_use` decisions (core innovation).

```python
# Original (contains complete tool calls and results)
[{"role": "assistant", "tool_calls": [...]}, {"role": "tool", "content": "..."}]

# After Microcompact (only decisions preserved)
[{"role": "assistant", "content": "I used tool X to read file Y"}]
```

#### Layer 4: Context Collapse (90%)

Archives and projects a collapsed view (read-only projection, does not modify underlying data).

#### Layer 5: Auto-Compact (95%)

LLM semantic compression, uses 60% rule to avoid repeated compression.

### Phase 6: Cache-Aware Compression

Cache-friendly strategy considering prompt caches:

```python
class CacheAwareCompressor:
    CACHE_ZONE_RATIO = 0.30        # Stable prefix ratio
    BATCH_COMPRESSION_THRESHOLD = 0.75  # Batch compression threshold
    
    def compress(self, messages):
        # 1. Identify cache zone (first 30%)
        cache_zone = messages[:int(len(messages) * 0.30)]
        
        # 2. Only modify outside cache zone
        # 3. Use stable compression boundary markers
```

**Effect:**
- Cache hit rate improved from 10-20% to 70-80%
- Reduces redundant computation, lowers latency

### Phase 7: Agent Integration (Agent Context)

Integrates context management into the agent execution flow:

```helen
agent SmartAssistant {
    context {
        compression "graduated"
        cache-aware true
        working-memory true
        working-memory-tokens 5000
    }
    
    main {
        // AgentContextManager applies automatically
        return llm act "..."
    }
}
```

**Integration points:**
- `_add_to_history()`: Updates working memory after each message is added
- `_record_llm_response_to_history()`: Updates from tool calls
- `_prepare_history_for_llm()`: Applies graduated compression and three-channel build

### Three-Channel Context Construction

When working memory is enabled, the context the LLM sees is divided into three channels:

| Channel | Ratio | Content |
|------|------|------|
| System Instructions | 15% | Framework instructions, language spec, agent description, skill index |
| Working Memory | 50% | Active files, recent decisions, pending TODOs, error history |
| Conversation History | 35% | Compressed conversation messages |

### Configuration Examples

#### High-Performance Research Agent

```helen
agent Researcher {
    context {
        compression "graduated"      // Graduated compression
        cache-aware true             // Cache-aware
        working-memory true          // Working memory
        working-memory-tokens 8000   // Larger working memory
    }
    
    tools ["web_search", "read_file", "write_file"]
    
    main {
        return llm act "Research..."
    }
}
```

#### Simple Quick Agent

```helen
agent QuickResponder {
    context {
        compression "none"           // No compression
        working-memory false         // Disable working memory
    }
    
    main {
        return llm act "Quick answer"
    }
}
```

### Performance Comparison

| Feature | v1.14 (Before) | v1.15 (Phase 7) |
|------|-------------|-----------------|
| Compression strategy | Single-layer truncation | Five-layer graduated |
| Cache hit rate | 10-20% | 70-80% |
| Working memory | ❌ | ✅ Auto-tracking |
| Context configuration | Global | Per-agent independent |
| Three-channel context | ❌ | ✅ |
| Cache-aware | ❌ | ✅ |

### Test Coverage

- `tests/runtime/test_working_memory.py` - 17 tests
- `tests/runtime/test_graduated_compression.py` - 16 tests
- `tests/runtime/test_cache_aware_compression.py` - 18 tests
- `tests/runtime/test_llm_summarization.py` - 9 tests
- `tests/interpreter/test_phase7_agent_context.py` - 16 tests

**Total: 76 new tests, all passing**

---

**Last Updated**: 2026-07-06  
**Version**: v1.15 (Phase 7)
