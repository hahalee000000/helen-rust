# Streaming Implementation Reference

Phase 1-3 streaming features for Helen language (2400+ tests).

## Architecture Overview

```
Phase 1: Stream output stdlib functions (stream_print, progress_bar, etc.)
Phase 2: llm act with on_chunk/on_complete callbacks (v1.14: merged from llm stream)
Phase 3: Legacy streaming response (StreamingResponse wrapper, streaming true field); `for await` syntax removed in v1.18 - use on_chunk callbacks instead
```

## Key Files

| File | Purpose |
|------|---------|
| `helen/stdlib/__init__.py` | IO functions: stream_print, stream_clear, progress_bar, stream_cursor_up/down |
| `helen/stdlib/stream_contracts.py` | Protocol contracts for stream functions |
| `helen/runtime/stream_contracts.py` | StreamChunk, StreamingLLMRuntime, LlmStreamCallback protocols |
| `helen/core/ast.py` | LlmActExprNode (with on_chunk/on_complete), DeclarationNode.streaming (legacy) |
| `helen/runtime/streaming_response.py` | StreamingResponse wrapper (legacy, for await removed in v1.18) |
| `helen/core/parser.py` | llm act parsing with on_chunk/on_complete |
| `helen/core/tokens.py` | STREAMING token, 'streaming' keyword |
| `helen/interpreter/interpreter.py` | visit_llm_act_expr (sync/streaming paths), _call_llm_streaming |

## Syntax

```helen
// Basic streaming (auto-prints chunks)
import std.io.*
llm act "Write a poem"

// With callback
fn handle(chunk) { stream_print("[" + chunk + "]") }
llm act "Write a story" on_chunk handle

// In agent (bare form uses rendered prompt)
agent Poet(topic: str) {
    prompt "Write about: {{topic}}"
    main { llm act }
}

// Streaming agent with on_chunk callback (v1.14+: streaming merged into llm act)
agent Streamer(topic: str) {
    description "Stream a long response"
    prompt "Write a detailed essay about: {{topic}}"
    main {
        fn handle(chunk) { stream_print(chunk) }
        llm act on_chunk handle
    }
}

main {
    Streamer("coding")
}
```

## Stdlib IO Functions (5)

| Function | Signature | Purpose |
|----------|-----------|---------|
| `stream_print` | `(text: str) -> str` | Print without newline, flush immediately |
| `stream_clear` | `() -> str` | Clear current line (ANSI \r\x1b[2K) |
| `progress_bar` | `(current, total, width=40) -> str` | Display progress bar with percentage |
| `stream_cursor_up` | `(n=1) -> str` | Move cursor up n lines (ANSI \x1b[nA) |
| `stream_cursor_down` | `(n=1) -> str` | Move cursor down n lines (ANSI \x1b[nB) |

## Test Files

- `tests/parser/test_llm_stream.py` — Parser tests for llm act with callbacks syntax (v1.14: renamed from llm stream)
- `tests/stdlib/test_stream_output.py` — Stdlib IO function tests (287 lines)
- `tests/runtime/test_streaming_response.py` — Async iteration tests

## Implementation Notes

1. **llm act is a statement, not expression** — when used as a statement with on_chunk/on_complete callbacks; `llm act` can also be used as an expression (returns full response text)
2. **v1.14 统一**: `llm stream` 已删除，流式功能合并到 `llm act`。`visit_llm_act_expr` 根据是否有 `on_chunk`/`on_complete` 回调分叉为同步/流式路径。
3. **Agent context** — automatically uses agent's model/temperature/prompt settings
4. **Fallback** — if runtime doesn't support streaming, falls back to non-streaming `act()`
5. **History** — full response text recorded to conversation history after streaming completes
6. **on_chunk callback** — must be callable; semantic error if not
7. **for await** — only valid in async context; iterates over AsyncIterable objects
8. **Chunk format compatibility** — `act_stream()` yields dicts `{"content": ...}`, not objects
   - Interpreter must handle both: `isinstance(chunk, dict)` → `chunk.get("content")`, else `chunk.content`
   - `StreamingResponse` yields plain strings (already extracted from chunks)
9. **REPL integration** — when agent uses `llm act` with `on_chunk`, REPL handler must not print return value
   - Output goes directly to stdout during execution
   - Handler adds final newline after streaming completes
