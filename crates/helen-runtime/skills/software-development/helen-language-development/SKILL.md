---
name: helen-language-development
description: "Helen Language Development Patterns — AST/parser/interpreter extension, async/await, exception hierarchy, scope isolation, shared variables, v1.14 features (Shared Store, Channel, llm act streaming unification), v1.16 TranscriptStore SSOT"
version: 1.16.0
author: Helen Team
license: MIT
tags: [helen, language-design, interpreter, async, parser, streaming, tool-calls, ffi, python-integration, contract-first, stdlib, closures, protocols, pipe-operator, pattern-matching, chinese-keywords, scope-isolation, shared-let, shared-store, channel, v1.10, v1.11, v1.12, v1.13, v1.14, v1.16, transcript, ssot]
---
<!-- helen-rust edition: for developing the RUST implementation, see wiki/MAINTENANCE.md + wiki/rust/migration-notes.md + the differential-porting skill. This skill describes Python-side language development; the Rust port mirrors it via differential testing. -->


# Helen Language Development

Development patterns and pitfalls for the Helen programming language (~/helen/).

## Quick Start & Environment

- **Python**: 3.12+ (required)
- **Package Manager**: uv (recommended) or pip
- **Install**: `cd ~/helen && uv pip install -e ".[dev]"`
- **CLI**: `helen` (REPL), `helen <file>` (run), `helen check <file>` (lint)
- **Tests**: `cd ~/helen && pytest`
- **Git remote**: `https://github.com/hahalee000000/helen.git`
- **File extension**: `.helen` (not `.hellen`)
- **Current version**: v1.14 (includes: scope isolation v1.10, shared let v1.10, isolation enhancements + shared store v1.12, channel v1.13, llm stream merged into llm act v1.14)

## When to Use

- Extending Helen's AST, parser, interpreter, or semantic analyzer
- Implementing new language features (control flow, async, exceptions, etc.)
- Debugging parser/interpreter issues
- Adding new predefined exceptions or error types
- Working with the Pratt parser framework
- Implementing Python FFI for accessing Python libraries
- Using contract-first + TDD workflow for language features
- Integrating external systems or libraries with Helen

## Core Architecture

```
helen/
├── core/
│   ├── ast.py          # AST nodes (frozen dataclasses with Visitor pattern)
│   ├── parser.py       # Pratt parser (prefix/infix rules)
│   ├── lexer.py        # Scanner/tokenizer
│   └── tokens.py       # Token types
├── interpreter/
│   ├── interpreter.py  # Main visitor (Visitor[object])
│   ├── environment.py  # Scope chain with snapshot() for isolation
│   ├── exceptions.py   # Exception hierarchy (HelenRuntimeError base)
│   └── task.py         # Async Task + AggregateError
├── semantic/
│   ├── analyzer.py     # Semantic analysis (Visitor[None])
│   └── type_utils.py   # Shared type_from_typenode() utility
├── runtime/
│   ├── tools.py        # Built-in tool registry (web_search, read_file, etc.)
│   ├── constants.py    # Centralized constants (URLs, thresholds, limits)
│   ├── llm_runtime.py  # LLMRuntime interface (sync + async)
│   └── hermes_cli_llm.py  # Hermes CLI-based LLM runtime
└── stdlib/
    ├── system.py       # System ops (shell=False default, PID/signal validation)
    └── network.py      # Network ops (URL validation, download size limits)
```

## Critical Patterns

### 1. Extending the AST

When adding a new node type (e.g., `AsyncCallExprNode`):

1. **Add to `ast.py`**: Frozen dataclass inheriting from `ExpressionNode` or `StatementNode`
2. **Add visitor method to `Visitor[R]`**: Abstract method `visit_<node_type>(self, node: NodeType) -> R`
3. **Add to `ASTPrinter`**: Concrete implementation returning S-expression string
4. **Update all Visitor implementations**:
   - `Interpreter` in `interpreter.py`
   - `SemanticAnalyzer` in `semantic/analyzer.py`
   - `MockVisitor` in `tests/core/test_ast.py`
5. **Run tests**: Missing visitor methods cause `TypeError: Can't instantiate abstract class`

**Pitfall**: AST nodes are `@dataclass(frozen=True)` — cannot modify attributes after creation. Use mock objects in tests instead of trying to override `accept()`.

### 2. Pratt Parser Extension

When adding a new prefix operator (e.g., `async`):

```python
# In Parser.__init__():
self._rules[TokenType.ASYNC].prefix = self._async_call_expr
self._rules[TokenType.ASYNC].precedence = Precedence.UNARY

# Prefix function:
def _async_call_expr(self) -> AsyncCallExprNode:
    start = self._previous()  # ← CRITICAL: token already consumed!
    call_expr = self._expression(Precedence.NONE)
    # ... build and return node
```

**CRITICAL PITFALL**: Pratt parser framework calls `self._advance()` BEFORE invoking the prefix function. The prefix function must use `self._previous()` to get the operator token, NOT `self._advance()`. Using `_advance()` consumes the NEXT token, causing parse errors.

### 3. Exception Hierarchy

All catchable exceptions must:

1. **Inherit from `HelenRuntimeError`** (not Python's `Exception`)
2. **Be added to `_PREDEFINED_EXCEPTIONS` dict** in `exceptions.py`
3. **Be added to `_PREDEFINED_EXCEPTIONS` frozenset** in `semantic/analyzer.py`

```python
# In exceptions.py:
@dataclass
class AggregateError(HelenRuntimeError):
    errors: list[Exception] | None = None
    # ...

_PREDEFINED_EXCEPTIONS: dict[str, type[HelenRuntimeError]] = {
    # ...
    "AggregateError": AggregateError,
}

# In semantic/analyzer.py:
_PREDEFINED_EXCEPTIONS = frozenset({
    # ...
    "AggregateError",
})
```

**Pitfall**: If an exception doesn't inherit `HelenRuntimeError`, `try-catch` cannot catch it (the interpreter only catches `HelenRuntimeError`). If it's not in the predefined sets, semantic analysis rejects it.

### 4. Async/Await Implementation (v1.10)

**Architecture**:
- `async Agent()` creates a **pending Task** (not executed immediately)
- `await [tasks]` executes all pending tasks concurrently using `asyncio`
- Each task gets an **environment snapshot** for isolation
- Uses `asyncio.to_thread()` for sync interpreter code (global thread pool, fixed memory)

**Key components**:

```python
# Task.pending() stores execution context
Task.pending(
    call_node=node.call,
    interpreter=self,
    env_snapshot=self.environment.snapshot()
)

# Environment.snapshot() deep-copies scope chain
def snapshot(self) -> Environment:
    parent_snapshot = self.parent.snapshot() if self.parent else None
    new_env = Environment(parent=parent_snapshot)
    new_env.values = self.values.copy()  # shallow copy of current scope
    return new_env
```

**Pitfall**: Environment snapshot must be taken BEFORE `async` expression evaluates. Otherwise, mutations between `async` and `await` corrupt the snapshot.

### 5. Agent Scope Isolation (v1.10/v1.12)

**Rules**:
- Module-level `let` is NOT visible in `agent main {}`
- Module-level `const` is auto-visible (read-only)
- `shared let` is visible and writable in `agent main {}` (v1.12: **value types only**)
- Closures in `agent main {}` use value capture (v1.12)
- Parameters of reference type (list/dict) are wrapped in ReadOnlyView (v1.12)

**Implementation**:

```python
# In SemanticAnalyzer
def visit_agent_decl(self, node: AgentDeclNode):
    # Create isolated scope for agent main
    self.env.begin_scope()

    # Check: module-level let should not be accessible
    for name in used_names:
        if name in module_level_lets:
            raise SemanticError(f"'{name}' is not visible in agent main")

    # v1.12: Also check function_vars initializers
    for var in node.function_vars:
        self._check_initializer_in_agent_scope(var.initializer)

    # v1.12: shared let must be value type
    for var in shared_let_declarations:
        if not self._is_value_type(var.type):
            raise SemanticError(f"shared let must be value type")

    # Allow: const and shared let
    self._check_body(node.main_body)

    self.env.end_scope()

# In Interpreter._call_agent
def _call_agent(self, agent, args):
    call_env = Environment()

    # v1.12: Wrap mutable params in ReadOnlyView
    for param in agent.params:
        value = args.get(param.name)
        if isinstance(value, (list, dict)):
            value = ReadOnlyView(value)
        call_env.define(param.name, value)

    # v1.12: Evaluate defaults in agent env, not caller env
    old_env = self.environment
    self.environment = call_env
    try:
        # ... evaluate defaults and function_vars
    finally:
        self.environment = old_env
```

**Pitfalls**:
- Forgetting to isolate agent main scope causes module-level `let` to leak
- v1.11 bug: defaults evaluated in caller env (fixed in v1.12)
- v1.11 bug: closures captured entire env (fixed in v1.12 with value capture)

### 6. Shared Let Tracking (v1.10/v1.12)

**Implementation**:

```python
# In SemanticAnalyzer
shared_vars: set[str] = set()

def visit_shared_decl(self, node: SharedDeclNode):
    self.shared_vars.add(node.name)
    self.env.define(node.name, node.type)

def visit_agent_decl(self, node: AgentDeclNode):
    # Track which shared vars are accessed
    accessed_shared = self._find_shared_access(node.main_body)

    # Validate: all accessed vars must be declared as shared
    for name in accessed_shared:
        if name not in self.shared_vars:
            raise SemanticError(f"'{name}' is not a shared variable")
```

**Pitfall**: Imported `shared let` from modules must be tracked correctly. The analyzer needs to follow imports.

## v1.10 Features

### Chinese Punctuation Support

Helen v1.10 supports Chinese fullwidth punctuation:

| English | Chinese | Description |
|---------|---------|-------------|
| `(` / `)` | `（` / `）` | Parentheses |
| `[` / `]` | `【` / `】` | Brackets |
| `{` / `}` | `｛` / `｝` | Braces |
| `,` | `，` | Comma |
| `.` | `。` | Period |
| `:` | `：` | Colon |
| `;` | `；` | Semicolon |
| `=` | `＝` | Equals |
| `+` | `＋` | Plus |
| `-` | `－` | Minus |
| `*` | `＊` | Asterisk |
| `/` | `／` | Slash |

**Implementation**: Lexer converts fullwidth to ASCII before tokenization.

### Chinese Quotes

| English | Chinese |
|---------|---------|
| `"..."` | `"..."` or `"..."` |
| `'...'` | `'...'` or `'...'` |

### Short-Circuit Evaluation

```helen
// && and || short-circuit
let result = a && b  // b not evaluated if a is false
let result = a || b  // b not evaluated if a is true
```

### Subscript/Field Assignment

```helen
// v1.10: assignment targets
arr[i] = x           // subscript assignment
obj.field = x        // field assignment
map["key"] = value   // map key assignment
```

### 92 Bilingual Keywords

Helen supports 92 keywords in both English and Chinese. See `references/chinese-keyword-implementation.md` for the full mapping table.

**Added in v1.12**: `store`/`仓库` (Shared Store declaration)
**Added in v1.13**: `channel`/`通道` (Channel declaration)
**Removed in v1.14**: `stream`/`流式执行` (streaming merged into `llm act`)

## v1.12 Features: Shared Store + Isolation Enhancements

### Shared Store (`shared store`)

`shared store` provides a structured way to share mutable reference types across agents:

```python
# AST: SharedStoreDeclNode(StatementNode)
# Token: STORE (keyword "store"/"仓库")

# Parser: _shared_store_decl() parses name, body with fn/let/const members
# Analyzer: visit_shared_store_decl() registers in symbol table, checks duplicates
# Interpreter: visit_shared_store_decl() creates SharedStore instance

class SharedStore:
    """Thread-safe shared state (RLock). Private fields: _prefix."""
    _INTERNAL_ATTRS = frozenset({'_name', '_fields', '_methods', '_lock'})

    def __getattr__(self, name):  # blocks _-prefixed names
    def __setattr__(self, name, value):  # blocks _-prefixed, methods
    def get_field(self, name):  # with lock
    def set_field(self, name, value):  # with lock
```

### Isolation Enhancements

- **Parameter defaults**: evaluate in agent env, not caller env
- **functions{} variables**: evaluate in agent env
- **Closure value capture**: snapshot values instead of env reference
- **Reference type parameters**: auto-wrap in ReadOnlyView
- **Compound assignment**: `arr[i]=x`, `obj.field=x` now check isolation
- **@open/@strict/@sandbox**: isolation level decorators

### `@` (AT) Token

- `TokenType.AT` — single-char operator (v1.12)
- Used for agent isolation decorators: `@open`, `@strict`, `@sandbox`
- Parser: `_parse_decorator()` before agent/fn/import declarations

## v1.13 Features: Channel

### Channel (`channel`)

`channel` declares type-safe communication endpoints between agents. At runtime, it reuses the `SharedStore` class.

```python
# AST: ChannelDeclNode(StatementNode)
# Token: CHANNEL (keyword "channel"/"通道")

# Parser: _channel_decl() — structurally identical to _shared_store_decl()
# Analyzer: visit_channel_decl() — registers as kind="channel"
# Interpreter: visit_channel_decl() — creates SharedStore instance (same as shared store)
```

**Semantic difference**: channel is a communication endpoint (message-passing), shared store is a shared state container (shared-memory). Runtime behavior is identical.

## v1.14 Features: llm stream merged into llm act

### Breaking Change

- `llm stream` keyword removed
- `STREAM` TokenType removed
- `LlmStreamStmtNode` removed
- `LlmActExprNode` gained `on_chunk`/`on_complete` optional fields
- `visit_llm_stream_stmt` removed; `visit_llm_act_expr` dispatches to sync or streaming path based on callback presence

```python
# llm_mixin.py
has_streaming = node.on_chunk is not None or node.on_complete is not None
if has_streaming:
    return self._visit_llm_act_streaming(...)
else:
    return self._visit_llm_act_sync(...)
```

**Keyword count change**: 94 → 92 (removed `stream`/`流式执行`)

## Testing Patterns

### Contract-First + TDD

1. **Write Python contract** (type hints, docstring)
2. **Write failing test** (Helen test file)
3. **Implement function** (minimal to pass)
4. **Refactor** (maintain green tests)

### Test Organization

```
tests/
├── core/           # Lexer, parser, AST
├── semantic/       # Semantic analyzer
├── interpreter/    # Interpreter, async
├── execution/      # End-to-end tests
├── runtime/        # LLM runtime, tools
├── stdlib/         # Standard library
└── integration/    # Full agent tests
```

### Running Tests

```bash
# All tests
pytest

# Specific module
pytest tests/core/

# Single test
pytest tests/execution/test_functions.py::test_function_call -v

# Helen's built-in test framework
helen test my_test.helen
helen test my_test.helen --only "test_name" --suite "suite_name"
```

## On-Demand References

For detailed implementation guides, read these reference files:

- **Parser patterns**: `references/parser-disambiguation.md`, `references/parser-optional-expression.md`
- **Interpreter patterns**: `references/interpreter-execution-patterns.md`, `references/interpreter-sentinels.md`
- **Async/await**: `references/v1.6-v1.7-v1.8-implementation.md`, `references/comprehensive-async-testing.md`
- **Python FFI**: `references/python-ffi-implementation.md`, `references/ffi-and-agents.md`
- **Stdlib implementation**: `references/stdlib-implementation-patterns.md`
- **Streaming**: `references/streaming-implementation.md`, `references/true-sse-streaming.md`
- **Chinese keywords**: `references/chinese-keyword-implementation.md`
- **Import system**: `references/import-system-debugging.md`
- **v1.6-v1.8**: `references/v1.6-v1.7-v1.8-implementation.md` (closures, protocols, pipe, pattern matching)

## Common Pitfalls

### 1. Sentinel Flow

Break/Continue/Return sentinels must propagate through all control flow:

```python
# In visit_while_stmt
while condition:
    result = self._eval(body)
    if isinstance(result, BreakSentinel):
        break
    if isinstance(result, ReturnSentinel):
        return result  # Must propagate!
```

### 2. Pratt Parser Ambiguity

When an expression can be parsed multiple ways, use precedence:

```python
# Example: `a ? b : c ? d : e`
# Should parse as: `a ? b : (c ? d : e)` (right-associative)
self._rules[TokenType.QUESTION].precedence = Precedence.TERNARY
self._rules[TokenType.QUESTION].right_associative = True
```

### 3. Environment Snapshot Timing

Always snapshot BEFORE async evaluation:

```python
# Correct
snapshot = self.env.snapshot()
task = Task.pending(snapshot=snapshot)

# Wrong (mutations between snapshot and task creation)
task = Task.pending()  # Too late!
snapshot = self.env.snapshot()
```

## TranscriptStore SSOT (v1.16)

### Architecture Overview

TranscriptStore is the Single Source of Truth (SSOT) for messages introduced in v1.16, replacing the previous dual-write architecture:

```
helen/runtime/
├── transcript_store.py    # TranscriptStore + BoundaryMarker + Backends
├── session_manager.py     # Session lifecycle management
└── config.py              # get_transcript_config()

helen/interpreter/
├── agent_context.py       # _init_transcript_store() + _record_compression_ssot()
└── interpreter.py         # _history @property (read-only view)

helen/stdlib/
└── transcript.py          # 6 stdlib functions
```

### Key Components

1. **TranscriptStore**: append-only storage, supports JSONL/SQLite backends
2. **BoundaryMarker**: records compression events (non-destructive compression)
3. **SessionManager**: session creation/resume/cleanup
4. **LRU Cache**: memory optimization (boundary-aware eviction)
5. **View Cache**: dirty flag + cache, O(1) reads

### Implementation Patterns

**Adding stdlib function**:
```python
# 1. helen/stdlib/transcript.py
def my_function() -> str:
    """My transcript function."""
    if _interpreter_agent_context is None:
        return ""
    store = getattr(_interpreter_agent_context, "transcript_store", None)
    if store is None:
        return ""
    return store.my_method()

# 2. helen/stdlib/__init__.py
from helen.stdlib.transcript import my_function as _my_function

BuiltinFunction("my_function", "...", "my_function()", _my_function, "transcript")

# 3. helen/stdlib/locales/zh.py
"我的函数": "my_function",
```

**Recording compression**:
```python
# In agent_context.py:_compress_history()
if self._transcript_store is not None:
    self._record_compression_ssot(
        original=history,
        compressed=compressed,
        layer=layer_name,
    )
```

**SSOT property**:
```python
# In interpreter.py
@property
def _history(self) -> list[HistoryMessage]:
    """Read-only view from TranscriptStore."""
    if self._agent_context.transcript_store is not None:
        return self._agent_context.transcript_store.read_view()
    return self._interpreter_history
```

### Testing

```bash
# TranscriptStore tests
pytest tests/runtime/test_transcript_store.py
pytest tests/runtime/test_transcript_persistence.py
pytest tests/runtime/test_session_manager.py
pytest tests/runtime/test_phase4_features.py

# Integration tests
pytest tests/integration/test_phase1_ssot.py

# Stdlib tests
pytest tests/stdlib/test_transcript.py
```

## Related Skills

- **helen-syntax** — Helen syntax reference (including v1.10 features)
- **helen-stdlib** — Standard library usage guide
- **helen-agent-patterns** — Agent design patterns

---

## ⚠️ Critical Knowledge: ImportResolver Cache Mechanism

### Problem Background

Developers working in REPL, Jupyter, or long-running services often encounter "new code not taking effect" after modifying `.helen` files. This is because `ImportResolver` uses an **in-memory cache**.

### Cache Behavior

```python
# helen/runtime/import_resolver.py
class ImportResolver:
    def __init__(self):
        self._cached_results: dict[str, ImportResult] = {}  # In-memory cache
        self._loaded: set[str] = set()                      # Circular import detection
```

| Scenario | Cache Behavior | New Code Takes Effect? |
|----------|---------------|----------------------|
| `helen main.helen` (CLI) | New process each time, no cache | ✅ Always takes effect |
| `import` in REPL | Cached after first load | ❌ Not effective after modification |
| Web service reusing Interpreter | Cached after first load | ❌ Not effective after modification |
| New Interpreter per request | Cache auto-cleared | ✅ Always takes effect |

### Key Pitfall

**❌ Wrong pattern** (common during development):
```python
from helen.interpreter import Interpreter

interp = Interpreter()
interp.execute_file("agent.helen")  # Loads v1

# Modify agent.helen...

interp.execute_file("agent.helen")  # ❌ Still v1!
```

**✅ Correct pattern** (Approach 1):
```python
def run_agent():
    interp = Interpreter()  # Create new instance each time
    return interp.execute_file("agent.helen")
```

**✅ Correct pattern** (Approach 2 - manual cache clear):
```python
interp = Interpreter()
interp.execute_file("agent.helen")

# After modifying the file
interp.import_resolver._cached_results.clear()
interp.import_resolver._loaded.clear()

interp.execute_file("agent.helen")  # ✅ New code takes effect
```

**✅ Correct pattern** (Approach 3 - mtime check):
```python
import os

class SmartInterpreter:
    def __init__(self):
        self.interp = Interpreter()
        self._mtimes = {}

    def execute_if_changed(self, path: str):
        mtime = os.path.getmtime(path)
        if self._mtimes.get(path, 0) < mtime:
            self.interp.import_resolver._cached_results.clear()
            self._mtimes[path] = mtime
        return self.interp.execute_file(path)
```

### Helen vs Python Cache Comparison

| Feature | Helen ImportResolver | Python `__pycache__` |
|---------|---------------------|---------------------|
| Cache location | In-memory | Disk (`.pyc`) |
| Cross-process | ❌ Not persisted | ✅ Persisted |
| Process restart | Auto-cleared | Preserved |
| After file modification | Manual clear needed | Auto-invalidated (mtime) |

### Why Doesn't Helen Use Disk Cache?

1. **Helen is an interpreted language** — no bytecode compilation step
2. **Parsing is fast** — cache benefit is marginal
3. **Developer experience first** — avoids "ghost of old code" problems
4. **Process isolation** — each execution starts with a clean environment

### Debugging Tips

```python
# Check cache status
print(f"Cached: {len(interp.import_resolver._cached_results)} files")
print(f"Loaded: {interp.import_resolver._loaded}")

# Force clear
interp.import_resolver._cached_results.clear()
interp.import_resolver._loaded.clear()
```

### Related Documentation

- `wiki/runtime/import.md` — Full cache mechanism documentation
- `wiki/reference/08-modules.md` — Development-time considerations
- GitHub Issue #15 — Problem diagnosis report

---

**Last updated**: 2026-07-16
**Version**: v1.21
- **helen-testing** — Testing framework and TDD workflow
