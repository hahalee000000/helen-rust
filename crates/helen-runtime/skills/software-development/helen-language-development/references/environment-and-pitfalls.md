---
name: helen-language
category: devops
description: "Helen programming language development — compiler architecture, testing, debugging, and workflow at ~/helen/."
trigger: "Any task involving the Helen programming language — building, testing, debugging, running, or modifying the Helen compiler/interpreter at ~/helen/."
---

# Helen Language

Helen is an AI-native DSL being developed at `/home/admin/helen/`. All 966 tests pass.

## Environment

- **System Python is 3.6** — do NOT use `python3` or `pip` for Helen work
- **Use Hermes venv Python 3.11**: `~/.hermes/hermes-agent/venv/bin/python`
- **Install**: `cd ~/helen && venv_python -m pip install -e ".[dev]"` (requires pip 21.3+; older pip needs the `setup.py` shim already in the repo)
- **CLI**: `helen` (no arguments = REPL), `helen <file>` = run, `helen check <file>` = lint
- **Tests**: `cd ~/helen && venv_python -m pytest`
- **Git remote**: `https://github.com/hahalee000000/helen.git`

## Additional References

- **Stdlib functions, Python FFI pitfalls & agent development patterns**: See `references/ffi-and-agents.md` for stdlib functions (write_file, append_file, mkdir_p, path_*, substring, trim_prefix/suffix), FFI quirks to avoid, and patterns for building autonomous agents with tools and persistent memory. **Prefer stdlib over Python FFI whenever possible.**

### Python Version Compatibility

**Helen is compatible with Python 3.8+** (verified 2026-06). The `pyproject.toml` declares `requires-python = ">=3.8"`.

**Why 3.8 works despite 3.10+ syntax**: All files use `from __future__ import annotations`, which defers annotation evaluation. This means `X | Y` union types and `list[X]` generics are never evaluated at runtime — they're just strings in `__annotations__`. No `match/case`, `dict |` merge, `str.removeprefix`, or other 3.9+ features are used.

**Implication**: Users with Python 3.8 (e.g., Ubuntu 20.04 default) can install and run Helen without upgrading Python.

## Quick Reference

**File extensions**: `.helen` (not `.hellen` — renamed globally)
**Package name**: `helen` (not `hellen` — renamed globally)
**Wiki**: `/home/admin/wiki/helen/` (not `wiki/hellen`)
- **Install dev deps**: `cd ~/helen && venv_python -m pip install -e ".[dev]"`
- **CLI entry**: `~/.local/bin/helen` (binary named `helen`, not `hellen`)
- **Machine**: 1.8GB RAM + 8GB swap — parser operations can OOM
- **Git repo**: `~/helen/`, remote at `https://github.com/hahalee000000/helen` (GitHub HTTPS)

## Architecture (7 Phases Complete)

| Phase | Component | Key Detail |
|-------|-----------|------------|
| 0 | Lexer | Tokenization, source tracking |
| 1 | Parser | Pratt parsing (10 levels) + recursive descent, 38+ AST nodes |
| 2 | SemanticAnalyzer | Symbol tables, type checking, scope management |
| 3 | Interpreter | Environment-based execution |
| 4 | LLM Integration | Agent calls, async, structured output |
| 5 | Runtime | Hermes runtime, history, memory, import resolver |
| 6 | CLI/REPL | `helen` CLI, formatter, docgen |
| 7 | LSP/StdLib/VS Code | Language server, stdlib functions, VS Code extension |

## Development Approach

- **Contract-first + TDD**: Contracts → Tests → Consistency Gate → Implementation → Quality Gate
- **Code is source of truth**; HLD at `~/documents/Helen_High_Level_Design_v1.2.md`
- Quality gates: coverage ≥ 80%, cyclomatic complexity < 15, Grade ≥ A
- **Always use git**: repo initialized from day 1, commit after each meaningful change
- **Self-hosting priority**: When building Helen features, prioritize writing them in Helen language itself. Use Python only for infrastructure (REPL, CLI) that cannot be expressed in Helen. This demonstrates Helen's capability to build real-world applications.

### Self-Hosting Pattern: Building Helen Applications in Helen

**Principle**: When a feature can be implemented in Helen, do so. Python is only for infrastructure that Helen cannot express (REPL loop, CLI argument parsing, etc.).

**Example**: Helen language assistant (2026-06)
- ✅ **Helen program** (`helenlab/helen_assistant.helen`): Loads documentation, builds context, calls LLM
- ✅ **Python integration** (`helen/cli/repl.py`): `:ask` command loads and executes the Helen program

**Architecture**:
```
┌─────────────────────────────────────────┐
│  Python REPL (infrastructure)           │
│  :ask <question>                        │
└─────────────┬───────────────────────────┘
              │ loads & executes
              ▼
┌─────────────────────────────────────────┐
│  Helen Program (core logic)             │
│  agent HelenAssistant(question: str) {  │
│    functions {                          │
│      fn load_docs(): str {            │
│        return read_file("docs/...")     │
│      }                                  │
│      fn build_context(): str { ... }  │
│    }                                    │
│    main {                               │
│      let ctx = build_context()          │
│      return llm act ctx                 │
│    }                                    │
│  }                                      │
└─────────────────────────────────────────┘
```

**Benefits**:
1. Demonstrates Helen's real-world capability
2. Uses Helen's stdlib (`read_file`, string operations)
3. Uses Helen's LLM integration (`llm act` with `on_chunk` callback for streaming output with tool support)
4. Python code is minimal (just loading/executing the Helen program)

**When to use**: Any feature where the core logic can be expressed in Helen (knowledge bases, data processing, LLM workflows, etc.). Reserve Python for true infrastructure (REPL event loop, CLI parsing, system integration).

### Knowledge-Based Agent Pattern

**Pattern**: Build agents that load external knowledge (documentation, code, data) and use LLM to answer questions.

**Implementation**:
```helen
import std.core.*
agent KnowledgeAssistant(query: str) {
    prompt "You are an expert assistant. Answer questions based on the provided context."
    
    functions {
        fn load_knowledge(): str {
            // Load documentation, code, or data
            let docs = read_file("path/to/knowledge.md")
            return docs
        }
        
        fn build_context(): str {
            let knowledge = load_knowledge()
            // String concatenation to build full context
            let context = "Context:\n" + knowledge + "\n\nQuestion: " + query
            return context
        }
    }
    
    main {
        let context = build_context()
        let answer = llm act context
        return answer
    }
}
```

**Key capabilities used**:
- `read_file(path)` — load external knowledge (stdlib)
- String concatenation (`+`) — build context
- `llm act` — call LLM with constructed prompt
- `functions { }` block — internal helper functions
- Agent parameters — inject query at call time

**Use cases**:
- Language assistants (load language docs)
- Code reviewers (load coding standards)
- Domain experts (load domain knowledge)
- Data analysts (load data files)

**Pitfall**: Large knowledge bases may exceed LLM context limits. Helen now has automatic context window protection (HLD 3.12):
- History is auto-trimmed to fit model's context window (model-aware lookup for 40+ models)
- When history exceeds 60% of context window, old messages are compressed into a summary
- API context-too-large errors auto-retry with message trimming
- Tool results per turn capped at `MAX_TOOL_RESULTS_PER_TURN = 10`

For very large knowledge bases, still implement chunking or summarization in `build_context()` — don't rely solely on auto-trimming.

## SemanticAnalyzer Key Files

- `helen/semantic/analyzer.py` — `SemanticAnalyzer` visitor, type checking
- `helen/semantic/symbols.py` — `SymbolTable`, `Scope`, `Symbol`
- `helen/semantic/types.py` — Type hierarchy, `type_compatible()`, `type_of_literal()`

## Naming Conventions

- **Language name**: `helen` (not `hellen`, not `Helen` in code paths)
- **File extension**: `.helen` (not `.hellen`)
- **Package**: `helen` (import `helen.core.*`, not `hellen.core.*`)
- **CLI**: `helen` command
- **Wiki docs**: `~/wiki/helen/` (not `~/wiki/hellen/`)
- User cares about naming consistency — always check all references when renaming

## Pitfalls

### Type Checking on Reassignment

The `visit_binary_op` method handles `x = value` (reassignment) as a `BinaryOpNode` with `TokenType.ASSIGN`. **This path must check type compatibility**, not just const protection. Before the fix, `let email: str? = null; email = 3.7` silently passed.

Current fix (in `visit_binary_op`):
```python
if node.operator.type == TokenType.ASSIGN:
    if isinstance(node.left, VariableNode):
        self._check_const_assignment(node.left.name, node.span)
        sym = self.symbols.resolve(node.left.name)
        if sym is not None and sym.type_node is not None:
            expected = self._type_from_typenode(sym.type_node)
            actual = self._infer_type(node.right)
            if not type_compatible(actual, expected):
                self.errors.error(
                    ErrorCode.SEMANTIC_TYPE_ERROR,
                    f"cannot assign {actual.name} to '{node.left.name}' of type {expected.name}",
                    node.span,
                )
```

### `_type_from_typenode` Must Handle Composite Types

The method originally only handled flat `TypeNode(name="str")`. It now recursively handles:
- `OptionalTypeNode` → `OptionalType(self._type_from_typenode(inner))`
- `UnionTypeNode` → `UnionType([self._type_from_typenode(m) for m in members])`
- `LiteralTypeNode` → `LiteralType(values)`

Without this, `str?` (optional types) crash with `AttributeError: 'OptionalTypeNode' has no attribute 'name'`.

### Lexer: true/false/null Must Carry Python Literals

**Bug**: `helen/core/lexer.py` `_identifier_or_keyword()` set `literal=None` for ALL keyword tokens, including `true`, `false`, `null`. This caused `type_of_literal(None)` → `NullType` for `true`, so `let x: str = "hello"; x = true` reported `cannot assign NullType` instead of `cannot assign BoolType`.

**Fix**: After keyword lookup, assign the correct Python literal:
```python
if tt == TokenType.TRUE: literal = True
elif tt == TokenType.FALSE: literal = False
elif tt == TokenType.NULL_KW: literal = None
else: literal = None
```

### Number Type Compatibility Is Strict

`type_of_literal` now distinguishes integer and float literals:
- `int` literal (e.g., `42`) → `IntType` (was `NumberType` before 2026-06 fix)
- `float` literal (e.g., `2.7`) → `FloatType` (was `NumberType` before 2026-06 fix)

Compatibility rules:
- `IntType` only accepts `IntType` and base `NumberType`. It **rejects** `FloatType` — no narrowing.
- `FloatType` accepts `FloatType`, `IntType`, and `NumberType` — widening allowed.

**Important**: `_infer_type()` can only infer types for **literal expressions** (LiteralNode, ListLiteralNode, MapLiteralNode). For all other expressions (BinaryOpNode, VariableNode, CallNode, etc.), it returns `AnyType`. This means compile-time type checking only catches mismatches when values are literals. **Runtime type checking** (in interpreter) catches mismatches for variables too — see "Two-Layer Type Checking Pattern" below.

### Return Type Checking in `visit_return_stmt`

**Added 2026-06**: `visit_return_stmt` now validates return value type against the function's declared return type.

Implementation pattern:
1. `visit_function_decl` saves `prev_return_type = self._current_return_type` and sets `self._current_return_type = self._type_from_typenode(node.return_type)` before analyzing the body
2. `visit_return_stmt` checks: if `_current_return_type is not None` and `_infer_type(node.value)` returns a concrete type (not `AnyType`), verify `type_compatible(actual, expected)`
3. `visit_function_decl` restores `self._current_return_type = prev_return_type` in the `finally` block (supports nested functions)

**Limitation**: `_infer_type()` only handles literals — expressions like `a + b` return `AnyType` and skip the compile-time check. **Runtime return type checking** is not yet implemented (only parameter checking has runtime layer). This means `fn add(a,b): int { return a + b; }` with `add(1, 1.7)` returning `2.7` is NOT caught. Full type inference for expressions is a future enhancement.

### Function Parameter Type Checking (Two-Layer Pattern)

**Added 2026-06**: When function parameters have type annotations, Helen performs type checking at both compile-time and runtime.

**Compile-time check** (in `SemanticAnalyzer.visit_call`):
- Stores parameter types in `_function_param_types` dict during `visit_function_decl`
- When calling a function, checks if arguments are literals (`LiteralNode`)
- If literal, uses `type_of_literal()` to get actual type and verifies `type_compatible(actual, expected)`
- Reports error immediately if mismatch: `"argument N type 'X' is not compatible with parameter type 'Y'"`

**Runtime check** (in `Interpreter._call_function`):
- Before binding parameters, iterates through `func.params` and checks each argument value
- Uses `type_of_literal(actual_value)` to infer Helen type from Python value
- If type is not `AnyType` and not compatible, raises `HelenRuntimeError`
- This catches cases where variables hold wrong types: `let x = 1.5; add(x, 2)` where `add` expects `int`

**Two-Layer Type Checking Pattern**:
When implementing type checking in Helen, always implement BOTH layers:
1. **Compile-time** (SemanticAnalyzer): Fast feedback for literal values, catches obvious errors early
2. **Runtime** (Interpreter): Catches type mismatches for variables whose types are unknown at compile-time

This pattern is necessary because `_infer_type()` only handles literals. Variables, expressions, and function calls return `AnyType` at compile-time, so runtime checking is essential for complete type safety.

**Example**:
```helen
fn add(a: int, b: int): int { return a + b; }

add(1.5, 2.7)    // ❌ Compile error: FloatType not compatible with IntType
let x = 1.5
add(x, 2)        // ❌ Runtime error: FloatType not compatible with IntType
add(1, 2)        // ✅ OK
```

**Type compatibility rules** (same as return type checking):
- `int` → `float` ✅ (int is subtype of float)
- `float` → `int` ❌ (no narrowing)
- any type → `any` ✅

### Function Declarations: Register-Then-Rollback Pattern

**Bug found 2026-06**: When a function definition had a body error (e.g., undeclared variable), the symbol was still registered in the `SymbolTable`. Subsequent redefinition attempts got "duplicate declaration" errors, and calling the function reported "uncallable" because the `Interpreter._functions` dict never received it (semantic errors blocked interpretation).

**Fix**: `visit_function_decl` in `analyzer.py` now uses a **register-then-rollback** pattern:
1. Record error count before body analysis (`errors_before_body`)
2. Register the symbol normally
3. Analyze the function body
4. If `len(errors.errors) > errors_before_body`, call `self.symbols.undefine(node.name)` to remove the symbol

This allows the user to fix and redefine the function without "duplicate declaration" errors.

**Supporting infrastructure added**:
- `Scope.undefine(name)` / `SymbolTable.undefine(name)` — remove symbols from scope
- `SemanticAnalyzer.reset()` — full state reset for REPL `:reset` command
- `SemanticAnalyzer.undefine(name)` — remove from global scope
- `Interpreter.undefine_function(name)` / `undefine_agent(name)` — remove from runtime registry
- `Interpreter.list_definitions()` — list all user-defined functions/agents
- `Interpreter.reset_definitions()` — clear all user definitions, keep stdlib

### Parser: `_return_stmt` Must Consume Trailing Semicolon

**Bug found 2026-06**: `_return_stmt()` parsed `return <expr>` but did NOT consume the trailing semicolon. This caused `return name;` to fail with "Expected expression, got SEMICOLON" because the semicolon was left unconsumed and hit the next parsing step.

**Fix**: Added `self._match(TokenType.SEMICOLON)` after expression parsing in `_return_stmt()`:
```python
def _return_stmt(self) -> ReturnStmtNode:
    start = self._previous()
    value: ExpressionNode | None = None
    if not self._check(TokenType.SEMICOLON, TokenType.RIGHT_BRACE, TokenType.EOF):
        value = self._expression()
    self._match(TokenType.SEMICOLON)  # ← added
    return ReturnStmtNode(value=value, span=start.span)
```

### REPL Commands: `:help`, `:reset`, `:list`, `:undefine`

**Added 2026-06**: The REPL now supports colon-prefixed management commands for interactive development:
- `:help` — show available commands
- `:reset` — clear all definitions (functions, agents, symbol table), re-register stdlib
- `:list` — list all user-defined functions and agents
- `:undefine <name>` — remove a specific function or agent from both interpreter and analyzer

Commands are handled in `_handle_repl_command()` in `helen/cli/repl.py`. They are only processed when `buffer_lines` is empty (not mid-multiline-input).

### Pitfall: Type Checking Must Have Both Compile-Time and Runtime Layers

**Lesson from 2026-06**: When implementing type checking for a feature (return types, parameters, etc.), do NOT stop at compile-time checking only. The user will expect runtime checking too.

**Why**: Helen's `_infer_type()` only handles literals. For variables, expressions, and function calls, it returns `AnyType`. This means compile-time checking alone cannot catch type mismatches involving variables.

**Wrong approach**: Only implement `SemanticAnalyzer` checks → user asks "运行时不能进行类型检查吗？" (can't you do runtime checking?)

**Right approach**: Implement BOTH layers:
1. Compile-time in `SemanticAnalyzer` (fast feedback for literals)
2. Runtime in `Interpreter` (catches variable type mismatches)

**Example**: When adding parameter type checking, I initially only added compile-time checks in `visit_call`. User pointed out that `let x = 1.5; add(x, 2)` should also fail. Solution: add runtime check in `_call_function` that validates actual argument values before binding parameters.

**General principle**: For any type safety feature in Helen, ask "what happens if the value is a variable?" If compile-time can't answer it, runtime must.

### SemanticAnalyzer: `match` Statement Requires `default` Branch

**Design decision enforced in semantic analysis**: The `match` statement must have a `default` branch. This is checked in `_check_match_completeness()` which calls `_check_branch_completeness()`.

**Error code**: `MATCH_NO_DEFAULT`
**Error message**: `"match must have a default branch"`

**Rationale**:
1. **Safety**: Prevents silent failures when no case matches. Without `default`, unmatched values cause the entire `match` to be skipped, potentially hiding bugs.
2. **Consistency with `llm if`**: The `llm if` statement (LLM-driven branching) also requires `default` because LLM output is unpredictable.
3. **Exhaustiveness by convention**: Similar to Rust's exhaustive match checking, but enforced via mandatory `default` rather than compile-time exhaustiveness analysis.

**Example**:
```helen
// ❌ Semantic error: match must have a default branch
import std.core.*
match status {
    case "active" { print("Active"); }
    case "pending" { print("Pending"); }
}

// ✅ Correct
match status {
    case "active" { print("Active"); }
    case "pending" { print("Pending"); }
    default { print("Unknown status"); }
}
```

**Implementation** (`helen/semantic/analyzer.py`):
```python
def _check_match_completeness(self, node: MatchStmtNode) -> None:
    """Validate that match has a default branch."""
    self._check_branch_completeness(bool(node.default), node.span, "match")

def _check_branch_completeness(self, has_default: bool, span, stmt_type: str) -> None:
    """Report error if llm-if or match has no default branch."""
    if not has_default:
        code = ErrorCode.LLM_IF_NO_DEFAULT if stmt_type == "llm_if" else ErrorCode.MATCH_NO_DEFAULT
        self.errors.error(code, f"{stmt_type} must have a default branch", span)
```

**Note**: This is a deliberate design choice, not a limitation. If users want optional `default`, they would need to request a language design change.

### Builtin Shadowing Allowed

In `visit_var_decl`, when `symbols.define()` returns an existing symbol, the code **allows shadowing** if the existing symbol has `kind == "builtin"`. This enables `let len = len(nums)` — the user variable `len` shadows the stdlib `len()` function in the local scope. Without this, all stdlib names (`len`, `print`, `str`, `int`, `range`, etc.) would be "duplicate declaration" errors.

### Interpreter: `interpret()` Must Execute Program Exactly Once

**Critical bug found via tutorial testing**: `interpret()` was executing the program **twice** — once via `self.visit_program(program)` and again via a manual `for stmt in program.statements` loop. This caused stale environment state from the first run to pollute the second run. Example: `let len = len(nums)` worked on the first pass (builtin `len` found), but on the second pass the variable `len` (value `3`) shadowed the builtin, so `len(nums)` resolved to the int `3` which is not callable → `'len' is not callable`.

**Fix**: `interpret()` now calls `self.visit_program(program)` once, captures the result, unwraps sentinels at the top level, and returns:
```python
result = self.visit_program(program)
if isinstance(result, ReturnSentinel):
    return result.value
if isinstance(result, (BreakSentinel, ContinueSentinel)):
    return None
return result
```

### Interpreter: `_execute_stmts` Must Propagate Sentinels to Loop Handlers

**Bug**: `BreakSentinel` and `ContinueSentinel` were consumed inside `_execute_stmts` and returned as `None`, so loop handlers (`visit_while_stmt`, `visit_for_stmt`) could not distinguish a break from a normal completion. This caused loops to continue past break statements.

**Fix**: `_execute_stmts` now propagates sentinels so the calling loop handler can decide what to do:
- `ReturnSentinel` → propagate immediately (exit function)
- `BreakSentinel` / `ContinueSentinel` → propagate immediately (loop handler decides break/continue)
- Otherwise → return the last non-sentinel result

This was later refined further: `visit_while_stmt` and `visit_for_stmt` consume the sentinel and return the last non-sentinel result (or `None`), and `interpret()` unwraps sentinels at the top level. The full sentinel flow is documented in `references/interpreter-sentinels.md`.

Similarly, `visit_while_stmt` must not return `BreakSentinel` as its result — it should break and return the last non-sentinel result (or `None`).

### Interpreter: `_execute_stmts` Sentinel Flow (Refined)

`_execute_stmts` must **propagate** sentinels so loop handlers can consume them:
- `ReturnSentinel` → propagate immediately (exit function)
- `BreakSentinel` / `ContinueSentinel` → propagate immediately (loop handler decides)
- Otherwise → return the last non-sentinel result

`visit_while_stmt` and `visit_for_stmt` consume the sentinel internally:
- On `BreakSentinel`: break the loop, return `None` (not the sentinel)
- On `ContinueSentinel`: continue to next iteration
- On `ReturnSentinel`: propagate immediately

`interpret()` unwraps sentinels at the top level:
- `ReturnSentinel` → return `.value`
- `BreakSentinel` / `ContinueSentinel` → return `None`
- Otherwise → return the result

**Key principle**: sentinels should never leak to the user — they are internal control flow markers consumed by the nearest handler.

### Parser: `if` and `while` Without Parentheses Supported

The `_if_stmt()` and `_while_stmt()` methods now accept both parenthesized and non-parenthesized conditions. Parentheses are optional:
- `if (cond) { ... }` ✅
- `if cond { ... }` ✅
- `while (cond) { ... }` ✅
- `while cond { ... }` ✅

This design choice makes Helen syntax more flexible and consistent with modern languages like Swift and Rust. The `for` statement never required parentheses: `for x in expr { ... }`.

### Parser: `functions {}` Block in Agent Body — Full Pipeline

Agent bodies can contain a `functions { fn ... fn ... }` block that groups internal function definitions. The full pipeline spans AST → Parser → Interpreter:

**AST** (`AgentDeclNode`): Has `functions: list[FunctionDeclNode] = field(default_factory=list)` field. The `default_factory` is critical — 32+ existing test constructors use keyword args and must not break.

**Parser** (`_agent_decl`): Must COLLECT function declarations into a list, not just parse-and-discard:
```python
agent_functions: list = []
# ...
elif self._match(TokenType.FUNCTIONS):
    self._consume(TokenType.LEFT_BRACE, "Expected '{' after 'functions'.")
    while not self._check(TokenType.RIGHT_BRACE, TokenType.EOF):
        if self._match(TokenType.FN):
            agent_functions.append(self._function_decl())  # ← collect, not discard
        # ...
    self._consume(TokenType.RIGHT_BRACE, "Expected '}' after functions block.")
# ...
return AgentDeclNode(..., functions=agent_functions)
```

**Interpreter** (`_call_agent`): Must register agent functions before executing `main`, unregister after:
```python
registered_names: list[str] = []
for func_node in agent.functions:
    self._functions[func_node.name] = func_node
    registered_names.append(func_node.name)
try:
    # execute main block
finally:
    for fname in registered_names:
        self._functions.pop(fname, None)  # cleanup to avoid leaking
```

**Pitfall — stdlib in isolated environments**: `_call_agent` creates a fresh `Environment()` (HLD 3.5.2 isolation — no parent variable inheritance). But stdlib functions (`len`, `print`, `str`, etc.) must still be available. Fix: inject stdlib into the fresh environment:
```python
call_env = Environment()
from helen.stdlib import stdlib as _stdlib
for _name in _stdlib.names:
    _builtin = _stdlib.lookup(_name)
    if _builtin is not None:
        call_env.define(_name, _builtin.fn)
```

**Lesson**: "Parse but don't store" is a silent bug — syntax check passes, but runtime fails with "'validate_input' is not callable". Always trace parsed constructs through to where they're consumed.

### Parser: Only `:` Valid for Function Return Types (v1.8.1+)

Function declarations now only accept `:` syntax for return types: `fn add(a: int, b: int): int`. The `->` syntax has been removed for consistency — parameters use `name: type`, so return types also use `: type`. The `_function_decl()` method now only checks `self._match(TokenType.COLON)`.

### Parser: Stray `RIGHT_BRACE` Must Not Cause Infinite Loop

**Bug**: `_declaration()` returned `None` on `RIGHT_BRACE` without advancing the token position. Combined with the outer `parse()` loop (`while not self._at_end()`), this caused an infinite loop stuck on `RIGHT_BRACE` — `_check` was called 696,000+ times without progress.

**Fix**: `_declaration()` now calls `self._advance()` when it sees `RIGHT_BRACE`, consuming the token before returning `None`.

### Parser: `call` Keyword Requires Special Handling

**Bug**: `call AgentName("arg1", "arg2")` — the `_call_kw()` method originally parsed `call` as a plain `VariableNode(name="call")`, so the full expression became a `BinaryOpNode` chain instead of a `CallNode`. This caused `"undeclared variable 'call'"` errors in semantic analysis.

**Fix**: `_call_kw()` must consume the following `IDENTIFIER` and parse a full `CallNode` when followed by `LEFT_PAREN`:
```python
def _call_kw(self) -> ExpressionNode:
    if self._check(TokenType.IDENTIFIER):
        self._advance()
        callee = VariableNode(name=self._previous().lexeme, span=self._previous().span)
        if self._check(TokenType.LEFT_PAREN):
            self._advance()  # consume '('
            return self._call(callee)
        return callee
    prev = self._previous()
    return VariableNode(name=prev.lexeme, span=prev.span)
```

### SemanticAnalyzer: `visit_call` Must Skip Agent Names

**Bug**: When calling an agent via `call Translator("Hello")`, the semantic analyzer's `visit_call` method visited the callee (`Translator`) as a `VariableNode`, which triggered `visit_variable` and reported `"undeclared variable 'Translator'"`. Agents are registered in `_agent_names`, not in the `SymbolTable`.

**Fix**: In `visit_call`, check if the callee name is in `self._agent_names` before calling `node.callee.accept(self)`:
```python
def visit_call(self, node: CallNode) -> None:
    if isinstance(node.callee, VariableNode):
        callee_name = node.callee.name
        if callee_name not in self._agent_names:
            node.callee.accept(self)
    else:
        node.callee.accept(self)
    # ... rest: validate agent params if callee is a known agent
```

### Tutorial Testing: `❌`/`✅` Emojis in Comments Trigger False "Expected Fail"

**Pitfall**: The tutorial test runner (`tests/tutorial/run_tutorial_tests.py`) uses `extract_helen_blocks()` which marks a code block as "expected fail" if it contains `❌` or `"Error:"` **anywhere in the block**, including comments. Example:

```helen
let global_x = 100
// print(local_x)  // ❌ 未定义   ← this ❌ in a comment triggers "expected fail"
```

The test expects this block to fail, but it actually parses and runs fine. **When writing tutorial examples, keep `❌` only in blocks that are genuinely intended to produce errors**, or put failing examples in separate blocks from working code.

### Tutorial Markdown: Use Correct Language Tags for Code Blocks

**Pitfall**: Tutorial 05 (`05-agents.md`) originally had Python code inside a `helen` block, causing the test runner to try parsing Python syntax as Helen. **Always use the correct language tag** (`python` for Python examples, `helen` for Helen code) when writing multi-language tutorials.

### Stale `.pyc` Cache After Source Edits

**Pitfall**: After editing Helen source files (especially removing debug prints or changing behavior), the REPL or `helen <file>` may still exhibit the OLD behavior. This is caused by stale Python bytecode cache (`.pyc` files in `__pycache__/` directories).

**Symptom**: User reports a bug is still present after you fixed the source. `grep` confirms the source is clean, but the behavior persists.

**Fix**: Clear all bytecode caches:
```bash
cd ~/helen && find . -name "*.pyc" -delete && find . -name "__pycache__" -type d -exec rm -rf {} + 2>/dev/null
```

**When to do this**: After ANY source edit that changes runtime behavior — especially removing `print()` statements, changing control flow, or modifying interpreter logic. Do it proactively before testing, not reactively after the user reports the fix didn't work.

**Why it happens**: Python caches compiled `.pyc` files keyed by source mtime. If the mtime resolution is coarse (1s on some filesystems) or if the edit happened within the same second, Python may serve the stale `.pyc` instead of recompiling.

### Interpreter.py: Avoid Unicode Arrows in Docstrings

The file `helen/interpreter/interpreter.py` contained Unicode arrow characters `→` (U+2192) in docstring comments, which caused `SyntaxError: invalid character` in Python 3.11. If editing this file, use `->` (ASCII) instead of `→`. A bulk fix: `sed -i 's/→/->/g' helen/interpreter/interpreter.py`.

### REPL Must Include SemanticAnalyzer

**Critical pitfall**: The REPL's `_execute_input()` originally went Lexer → Parser → Interpreter, completely bypassing `SemanticAnalyzer`. This means **type checking, scope validation, and all semantic analysis were silently skipped in REPL mode** — they only worked in `helen <file>` (which has the full pipeline in `__main__.py`).

Symptom: `let email: str = "hello"; email = false` silently accepted in REPL, but `helen <file>` would have caught it.

Fix in `helen/cli/repl.py`:
- Create persistent `SemanticAnalyzer` at REPL startup (shares `SymbolTable` with `Interpreter`)
- Insert `analyzer.analyze(program)` between Parser and Interpreter in `_execute_input()`
- Reset `_in_loop` and `_in_function` counters in `analyze()` to prevent state accumulation across REPL turns

This is a pattern to watch for: **any interactive/REPL mode must run the full compiler pipeline**, not just parse + execute.

### REPL and Script Mode Both Use Real LLM Runtime

**Updated 2026-06**: Both REPL and script mode (`helen <file>`) now use `HttpLLMRuntime` for real LLM calls.

| Mode | Runtime | Speed | Notes |
|------|---------|-------|-------|
| **REPL** (`helen`) | HttpLLMRuntime | 7-11s/call | Direct HTTP to OpenAI-compatible API |
| **Script** (`helen <file>`) | HttpLLMRuntime | 7-11s/call | Same as REPL (was MockLLMRuntime before fix) |
| HermesCLILLMRuntime | fallback | 15-17s/call | Spawns `hermes -z` subprocess |

**Lesson**: `MockLLMRuntime` is only for unit tests. Production code (both REPL and script) must use `HttpLLMRuntime`.

### Test Updates When Implementing Previously-Stub Methods

When you implement a method that previously raised `NotImplementedError`:
1. Find the existing test class (e.g., `TestNotImplementedMethods`)
2. **Rename the class** to reflect the new behavior (e.g., `TestImplementedMethods`)
3. **Update assertions** from `with pytest.raises(NotImplementedError)` to actual behavior checks
4. **Add `pytest` import** if not present (the old try/except pattern didn't need it)

Example:
```python
# Old test (expected NotImplementedError):
def test_load_tool_raises(self):
    try:
        self.runtime.load_tool("search")
        assert False
    except NotImplementedError:
        pass

# New test (method now works):
def test_load_tool_returns_schema(self):
    tool = self.runtime.load_tool("search")
    assert tool.name == "search"
```

### Skills Are Nested — Must Recursively Walk Directories

The HelenHermesRuntime's `list_skills()` must use `os.walk()` to find SKILL.md files, not just `os.listdir()`. Skills are organized in nested directories:
```
~/.hermes/skills/
├── mlops/
│   └── inference/
│       └── serving-llms-vllm/
│           └── SKILL.md    ← this is a skill!
├── helen-language/
│   └── SKILL.md
```

Using `os.walk(base)` and checking for `"SKILL.md" in files` finds all skills regardless of nesting depth.

### YAML Frontmatter Parsing Without External Libraries

SKILL.md files have YAML frontmatter between `---` markers. Parse it manually:
```python
def _parse_skill_frontmatter(path: str) -> dict[str, str]:
    with open(path) as f:
        content = f.read()
    if not content.startswith("---"):
        return {}
    end = content.find("---", 3)
    if end < 0:
        return {}
    yaml_text = content[3:end].strip()
    result = {}
    for line in yaml_text.split("\n"):
        if ":" in line:
            key, _, value = line.partition(":")
            result[key.strip()] = value.strip().strip('"').strip("'")
    return result
```

This avoids the `pyyaml` dependency for simple key:value frontmatter.

### Cross-Cutting Rename Checklist

When renaming the language/project (e.g., `hellen` → `helen`), check ALL these locations:
1. **Package directory**: `hellen/` → `helen/`
2. **All imports**: `from hellen.*` → `from helen.*` (search all .py files)
3. **File extensions**: `.hellen` → `.helen` (source files + VSCode grammar + code references)
4. **CLI command**: pyproject.toml `[project.scripts]` entry
5. **pyproject.toml**: package name, `tool.setuptools.packages.find`, coverage source
6. **VSCode extension**: `package.json` language ID, `syntaxes/*.tmLanguage.json` filename + scopeName
7. **Wiki directory**: `~/wiki/hellen/` → `~/wiki/helen/` + all .md references
8. **Design docs**: HLD filenames and content
9. **Hermes skills**: skill directory name, SKILL.md frontmatter name/description/trigger, `.usage.json`
10. **pip editable install**: re-run `pip install -e .` (updates `direct_url.json`)
11. **Git remote**: verify remote URL after rename

### `find_files` vs `search_files` — 正确用法

两个文件搜索工具有不同用途：

- **`find_files(path, pattern)`**：按 glob 模式查找文件（类似 `find` 命令）。示例：`find_files(path="src/", pattern="**/*.py")`
- **`search_files(path, pattern, regex=false)`**：按内容搜索文件（类似 `grep`）。示例：`search_files(path="src/", pattern="def process")`

注意：`search_files` 没有 `target` 或 `file_glob` 参数。要查找文件名匹配的特定文件，用 `find_files`。

### Import Bug: `node.path` vs `node.module_path`

**Bug found 2026-06**: `visit_import_stmt` in interpreter accesses `node.path` but the AST defines the field as `node.module_path`. This causes `AttributeError: 'ImportStmtNode' object has no attribute 'path'` at runtime when executing any `import` statement.

```python
# Interpreter bug (visit_import_stmt):
result = self.import_resolver.resolve(node.path, current_file)  # ❌ should be node.module_path

# AST definition (ImportStmtNode):
module_path: str  # ← the correct field name
```

Fix: change `node.path` → `node.module_path` in `helen/interpreter/interpreter.py`.

### Import Data Files: Alias Registration, Duplicate Detection, Format Field

**Bugs found 2026-06**: Three interacting bugs when importing data files (`.json`, `.md`, `.txt`, `.yaml`):

**Bug 1 — SemanticAnalyzer only registered alias when `as` was explicit**: `visit_import_stmt` in `analyzer.py` only called `symbols.define()` when `node.alias` was set. If user wrote `import "config.json"` (no `as cfg`), no variable was registered → "undeclared variable 'config'" when accessing it.

**Fix**: For data files, always register a variable — use `node.alias` if provided, otherwise derive from filename:
```python
if path.endswith(('.json', '.md', '.txt', '.yaml', '.yml')):
    alias = node.alias if node.alias else os.path.splitext(os.path.basename(path))[0]
    sym = Symbol(alias, kind="import", is_const=False)
    self.symbols.define(alias, sym)
```

**Bug 2 — Duplicate import detection blocked re-import with different alias**: `_imported_paths` set caused early return on second import of same file, skipping alias registration. `import "config.json"` then `import "config.json" as cfg` → `cfg` never registered.

**Fix**: Only apply `_imported_paths` dedup for `.helen` files. Data files should always register their alias:
```python
if path.endswith('.helen'):
    if path in self._imported_paths:
        return
    self._imported_paths.add(path)
```

**Bug 3 — Import resolver hardcoded `format="helen"` in cached return**: When a data file was imported twice, `ImportResolver.resolve()` returned `ImportResult(format="helen")` for the cached result, causing the interpreter to treat JSON data as Helen code.

**Fix**: Use `self._detect_format(resolved)` instead of hardcoding `"helen"`:
```python
return ImportResult(
    path=abs_resolved, content=self._data[filename_alias],
    format=self._detect_format(resolved)  # ← was hardcoded "helen"
)
```

**Lesson**: When a construct has both a "data" path and a "code" path (like imports), the dedup/caching logic must handle both paths correctly. Test with: (1) import without alias, (2) import with alias, (3) same file imported twice with different aliases.

### `catch` Typed Syntax: No `as` Keyword

**HLD EBNF** (§3.3.4) says: `CatchClause → "catch" IDENTIFIER ("as" IDENTIFIER)? "{" Statement* "}"`
This implies `catch RuntimeError as err` should be valid.

**Reality**: The parser (`_catch_clause`) expects `catch Type varname { }` — the `as` keyword is **not** implemented despite the HLD. Using `catch RuntimeError as err` produces `E0301: Expected error variable name.`

**Correct syntax**: `catch RuntimeError err { ... }` (type name followed by variable name, no `as`).
**Wrong**: `catch RuntimeError(err)` — parenthesized syntax rejected.
**Wrong**: `catch RuntimeError as err` — `as` keyword not supported in parser.

**If adding `as` support**: Modify `_catch_clause` in parser.py to `_match(TokenType.AS)` between the type and the variable name.

### Recursion Depth Limit: ~109 Layers

**Discovered 2026-06**: Helen's practical recursion depth limit is approximately **109 layers**, despite Python's default limit of 1000.

**Why so low?** Each Helen function call traverses multiple interpreter layers:
- `Scanner.scan_all()` → tokenization
- `Parser.parse()` → syntax analysis  
- `SemanticAnalyzer.analyze()` → semantic checks
- `Interpreter.interpret()` → execution
- `visit_call()` → `_call_function()` → `visit_*()` methods

These layers consume ~9 stack frames per Helen function call, so 1000 Python frames ÷ 9 ≈ 109 Helen calls.

**Test case**:
```helen
fn factorial(n: int): int {
    if (n <= 1) { return 1; }
    return n * factorial(n - 1);
}
factorial(109);  // ✅ Success
factorial(110);  // ❌ RecursionError
```

**Discovery method** (binary search):
```python
import sys
from helen.core.lexer import Scanner
from helen.core.parser import Parser
from helen.core.errors import ErrorReporter
from helen.semantic.analyzer import SemanticAnalyzer
from helen.interpreter.interpreter import Interpreter

# Binary search for max depth
low, high = 100, 500
while low <= high:
    mid = (low + high) // 2
    code = f'fn factorial(n:int):int {{ if(n<=1){{return 1;}} return n*factorial(n-1); }} factorial({mid});'
    # ... instantiate pipeline, try/except RecursionError
```

**Potential solutions** (not yet implemented):
1. **Increase Python limit**: `sys.setrecursionlimit(10000)` — simple but doesn't address root cause
2. **Custom depth tracking**: Add recursion counter in `Interpreter`, raise `HelenRecursionError` at configurable limit — more controlled
3. **Tail-call optimization**: Convert tail-recursive functions to loops — requires compiler analysis

**Implication**: For algorithms requiring deep recursion (DFS, tree traversal), users should either:
- Use iterative implementations
- Request the interpreter to increase Python's recursion limit
- Wait for tail-call optimization (future enhancement)

### Runtime Error Handling: Must Report AND Raise

**Bug found 2026-06**: `_runtime_error()` only reported errors to `ErrorReporter` but did NOT raise exceptions. This meant `try-catch` could not catch runtime errors like division by zero.

**Symptom**: User wrote `try { x / 0 } catch RuntimeError err { ... }` but the catch block never executed. The error was printed but not caught.

**Root cause**: `try-catch` in Helen works by catching `HelenRuntimeError` exceptions (see `visit_try_stmt` in interpreter). If `_runtime_error()` only calls `self.errors.error()` without raising, there's no exception to catch.

**Fix**: `_runtime_error()` must do BOTH:
```python
def _runtime_error(self, span: SourceSpan | None, message: str) -> None:
    """Report a runtime error and raise an exception."""
    self.errors.error(ErrorCode.RUNTIME_ERROR, message, span)  # Report for error collection
    raise RuntimeError(message, span)  # Raise for try-catch
```

**Pattern**: Any runtime error that should be catchable by user code must:
1. Report to `ErrorReporter` (for error display/logging)
2. Raise a `HelenRuntimeError` subclass (for try-catch mechanism)

**Test helper implication**: Test `_run()` helpers that execute code expecting errors must catch `HelenRuntimeError`:
```python
def _run(*stmts) -> tuple:
    from helen.interpreter.exceptions import RuntimeError as HelenRuntimeError
    # ...
    try:
        result = interp.interpret(prog)
    except HelenRuntimeError:
        result = None
    return result, errors
```

**Error code**: Added `RUNTIME_ERROR = 351` to `ErrorCode` enum in `helen/core/errors.py`.

### `throw` Statement (Added 2026-06)

**Syntax** (two forms):
```helen
throw RuntimeError("custom message")   // with message
throw LLMError                         // default message
```
- Semicolon is optional
- Exception type must be a predefined type (validated by SemanticAnalyzer)
- Message expression is evaluated at runtime via `node.message.accept(self)`

**Predefined exception hierarchy** (in `helen/interpreter/exceptions.py`):
```
AnyError (root)
├── LLMError
│   ├── TimeoutError
│   └── ModelError
├── ToolError
├── RuntimeError
└── AggregateError    (multiple async task failures)
```

**Catch syntax** (no parentheses, no `as` keyword):
```helen
catch RuntimeError err { ... }    // ✅ correct
catch RuntimeError(err) { ... }   // ❌ wrong
catch RuntimeError as err { ... } // ❌ not supported
```

**Inheritance matching**: `catch LLMError err` also catches `TimeoutError` and `ModelError` (via `error_matches()` using `isinstance()`).

**Keywords count**: 41 (was 43 before `throw` was added, then `choose` and `option` removed in 2026-06 when `llm choose` was merged into `llm if`; `stream` removed in v1.14 when merged into `act`)

### `llm act` Can Run Independently (Not Just in Agents)

**User question 2026-06**: "Can `llm act` run without any function, agent, or prompt?"

**Answer**: Yes, `llm act` can run at the top level of a script or in the REPL, but it **must have a prompt expression**. It does not require being inside an agent or function.

```helen
// ✅ Independent use at top level
import std.core.*
llm act "What is 2+2?"
let result = llm act "Translate this text"
print(llm act "Summarize: " + content)

// ❌ Error: must have a prompt
llm act   // Error: Expected expression
```

**Agent fields are optional context**: When `llm act` runs inside an agent's `main` block, it reads `description`, `model`, `temperature`, `max-turns` from the agent declaration via `_get_agent_setting()`. The `description` is injected as a section of the LLM system prompt (role definition); `model`/`temperature`/`max-turns` configure the runtime call. When run outside an agent, these default to `None`/`None`/`1.0`/`1` and the LLM runtime uses its own defaults. (v1.27.2 fix: `description` was previously dropped because `_get_agent_setting`'s field map omitted it.)

### REPL: Readline + Unicode Input (Final Approach)

**Lesson learned 2026-06**: Two REPL input features conflict — `io.TextIOWrapper` for CJK robustness vs `readline` for cursor movement/history. **You cannot have both.**

**Wrong approach** (removed): Wrapping stdin with `io.TextIOWrapper(sys.stdin.buffer, errors='replace')` broke readline entirely — arrow keys, history, and cursor movement stopped working.

**Correct approach** (current): Enable readline, catch `UnicodeDecodeError` as safety net:
```python
try:
    import readline
    readline.parse_and_bind("tab: complete")
except ImportError:
    pass  # readline not available on all platforms
```

```python
except UnicodeDecodeError as e:
    print(f"Input encoding error: {e}. Please try again.", file=sys.stderr)
    buffer_lines.clear()
    continue
```

**Key insight**: Python's `input()` with readline already handles UTF-8 reasonably well on modern systems. The `TextIOWrapper` approach was over-engineering that broke more than it fixed. If CJK crashes are rare, the `except UnicodeDecodeError` safety net is sufficient.

### REPL: Multi-Line Mode Escape Hatches

**Added 2026-06**: Users could get stuck in `...` (multi-line) mode when they made a syntax error mid-block with no way to recover.

**Three escape methods**:

| Method | Effect |
|--------|--------|
| **Empty line** (Enter at `...` prompt) | Forces execution of accumulated buffer, even if braces are unbalanced |
| **Ctrl+C** | Cancels multi-line input, clears buffer, returns to `>>>` prompt |
| **Ctrl+D** | Exits entire REPL |

**Implementation** (in `repl_command()` main loop):
```python
except KeyboardInterrupt:
    if buffer_lines:
        print("\n(multi-line input cancelled)")
        buffer_lines.clear()
        continue
    print("\nInterrupted")
    break

# Empty line in multi-line mode forces execution
if buffer_lines and not line.strip():
    buffer = "\n".join(buffer_lines)
    # Execute even if braces unbalanced
    ...
    buffer_lines = []
    continue
```

**Lesson**: Any multi-line input mode MUST provide at least two escape routes — one that tries to execute (empty line) and one that cancels (Ctrl+C). Users WILL get stuck otherwise.

### While Loop: Assignment vs Declaration Shadowing

**Bug found via tutorial testing**: `let count = count + 1` inside a `while` loop creates a **new local variable** that shadows the outer `count`, so the loop condition `count < 5` always sees the original value → **infinite loop**.

```helen
import std.core.*
let count = 0
while (count < 5) {
    print(count)
    count = count + 1    // ✅ assignment — modifies outer variable
    // let count = count + 1    // ❌ new declaration — shadows, causes infinite loop
}
```

When writing or reviewing while-loop tutorials, **always use assignment** (`count = count + 1`) not `let`. The tutorial test runner skips while loops without `break` to guard against this exact infinite loop scenario.

### spawn + Channel (Tutorial 07) — Concurrent Execution (v1.18)

**Replaces the old async/await/detach model.** `spawn` spawns an agent in a daemon thread and returns a `Channel` (mailbox) for bidirectional communication.

**Working syntax**:
```helen
// spawn returns a Channel (mailbox) immediately - agent runs in background thread
import std.concurrency.*
import std.core.*
agent Worker(input: str, reply: Channel) {
    main {
        let result = process(input)
        reply.send(result)
    }
}

let m1 = spawn Worker("input A")
let m2 = spawn Worker("input B")

// receive() blocks until the agent sends a result
let r1 = m1.receive()
let r2 = m2.receive()
let results = [r1, r2]
print(results)  // ["result A", "result B"]

// fire-and-forget: just don't call receive()
spawn Worker("fire and forget")

// multi-channel select (first-ready wins)
let ready = mailbox_select([m1, m2])

// cancel a spawned agent
m1.cancel()
```

**Architecture — Thread-based with Channel message queue**:

The spawned agent runs in an isolated daemon thread with a deep-copied snapshot of the parent environment. Communication happens exclusively through the `Channel` (dual-queue: `to_spawned` / `from_spawned`).

**Execution flow**:
```
spawn Worker("A")  -> Thread(started) -> isolated Interpreter(env_snapshot)
Worker main runs        -> reply.send(result) -> from_spawned queue
mailbox.receive()       -> queue.get() -> blocks until result arrives -> resultA
```

**Isolation**: `environment.snapshot()` deep-copies ALL variables (including `SharedStore`). The spawned agent cannot accidentally mutate parent state. To share mutable state explicitly, pass a `SharedStore` reference through the `Channel`:
```helen
shared store Counter { count: int = 0 }
let counter = Counter()
let mailbox = spawn Worker(counter)  // reference passed in message
```

**Key implementation files**:
- `helen/runtime/channel.py` - `Channel` (dual-queue container) + `ChannelEndpoint` (main/spawned sides)
- `helen/interpreter/interpreter.py` - `visit_spawn_expr` (snapshot + thread creation)
- `helen/interpreter/environment.py` - `snapshot()` (full deep copy)
- `helen/stdlib/mailbox.py` - `mailbox_select()` for multi-channel select

**Pratt parser note**: `spawn` is registered as an expression prefix. The Pratt framework consumes the `SPAWN` token before calling `_spawn_expr`, so use `self._previous()` to access it (not `self._advance()`).

**Pitfall — SharedStore methods are NOT deep-copied**: When snapshot deep-copies a `SharedStore`, the data fields are copied but the bound methods are not (they reference closures over the original interpreter). To call methods on a shared store inside a spawned agent, pass the store reference explicitly through the Channel rather than relying on the snapshot copy.

### Tutorial `llm if` Syntax Conflict — FIXED

Tutorials 06 (06-llm-statements.md) and 10 (10-building-agents.md) had `case "x":` syntax inside `llm if`. The implementation only supports `branch "name" { }`. **This was fixed 2026-06**: all `case:` instances replaced with `branch "name" { }` in the tutorial markdown files.

### Agent Calls: Function-Style Only (call Keyword DEPRECATED 2026-06)

**`call` keyword is DEPRECATED.** Use function-style `Agent(args)` exclusively:

```helen
// ✅ CORRECT — function-style (parses to CallNode directly)
let translated = Translator(text="Hello", target="French")
let result = Summarizer(text)
let t = Translator("Hello", "French")    // positional also works

// ❌ DEPRECATED — call keyword removed from parser
// let translated = call Translator(text="Hello", target="French")

// ❌ DEPRECATED — llm act statement form
// llm act Translator(text="Hello") "translate"
```

**Implementation**: `visit_call` in interpreter checks `self._functions` first, then `self._agents`. Both named and positional arguments are supported. The `call` keyword was removed from the parser in 2026-06 (commit 9cb0d97).

**Bug fixed 2026-06**: `call Agent("hello")` silently ignored positional arguments. The interpreter's `visit_call` built the args dict as `{arg.name: arg.value.accept(self) for arg in node.arguments}` — but positional args have `arg.name == None`, so all values mapped to the `None` key and parameters got `None`.

**Fix**: `visit_call` now iterates with index and binds positional args to the i-th parameter:
```python
agent_args: dict[str, object] = {}
for i, arg in enumerate(node.arguments):
    if arg.name is not None:
        agent_args[arg.name] = arg.value.accept(self)  # named
    elif i < len(agent.params):
        agent_args[agent.params[i].name] = arg.value.accept(self)  # positional
    else:
        self._runtime_error(node.span, f"too many positional arguments...")
```

**All call forms now work**:
```helen
call Agent(text="hello")           // named
call Agent("hello")                // positional → binds to first param
call Agent("hello", "French")      // multiple positional
call Agent("hello", target="Fr")   // mixed
```

**Important**: Variable name must NOT be `args` (shadows outer `args` list used for function calls). Use `agent_args` to avoid type conflict with the `args: list[object]` used in the function-call branch below.

### Agent `prompt` Field Only Accepts String Literals

**Parser limitation**: The `prompt` field in agent declarations only accepts string or triple-quoted-string literals. Expressions like `prompt "text" + variable` fail with `Unexpected token in agent body: PLUS`.

**Correct**: `prompt "翻译以下文字到英文:"`
**Wrong**: `prompt "翻译到" + target + ":"`

If dynamic prompt construction is needed, do it in the `main { }` block instead.

### Implementing New Language Features: Full Pipeline Checklist

When adding a new statement/expression type to Helen, follow this exact sequence:

1. **Token** (`tokens.py`): Add `TokenType.XXX` + keyword map entry + update count comments
2. **AST** (`ast.py`): Add `XxxNode` dataclass + `visit_xxx` abstract method on `Visitor` + `visit_xxx` on `ASTPrinter`
3. **Parser** (`parser.py`): Import new node + add dispatch in `_declaration()`/expression + implement `_xxx()` method
4. **SemanticAnalyzer** (`analyzer.py`): Import new node + implement `visit_xxx()` with validation
5. **Interpreter** (`interpreter.py`): Import new node + import exceptions if needed + implement `visit_xxx()` with execution logic
6. **Tests**: Update `MockVisitor` in `test_ast.py` + update keyword count in `test_tokens.py` + add feature-specific tests
7. **Docs** — update ALL affected files across two locations:
   - **Wiki** (`~/wiki/helen/`, NOT a git repo):
     - `index.md` — test count, phase status, new section links
     - `appendix/changelog.md` — new phase/feature entry + quality metrics table
     - `appendix/hld-compliance.md` — module file list, compliance items, totals
     - `overview/architecture.md` — new modules in Runtime layer, CLI commands, quality metrics
     - `toolchain/cli.md` — new subcommands (e.g. `helen init`)
     - `runtime/llm-runtime.md` — new runtime features (config, tools, skill system)
     - `reference/01-getting-started.md` — setup/config changes
     - `reference/XX-*.md` — the specific reference chapter for the feature
   - **Project** (`~/helen/`, git repo):
     - `README.md` — CLI table, Language Features, Status table, test count, project structure
   - **Rule of thumb**: if you added a CLI command, update cli.md + architecture.md + README. If you added a runtime module, update llm-runtime.md + architecture.md + hld-compliance.md. If you changed test count, update index.md + changelog.md + README.
8. **Run**: `pytest` (all pass) + `run_tutorial_tests.py` (0 fail) + git commit (helen repo) + push. Wiki is NOT a git repo — changes are local only.

**Pitfall**: Forgetting to add `visit_xxx` to `ASTPrinter` causes all AST printer tests to fail with `TypeError: Can't instantiate abstract class MockVisitor with abstract method visit_xxx`. Forgetting to update `MockVisitor` in `test_ast.py` causes the same error. Always update both.

**Pitfall — Statement vs Expression prefix token consumption**: When a token (like `async`, `llm`) is registered as BOTH a statement prefix (dispatched from `_declaration()`) AND an expression prefix (registered in Pratt rules via `_register_pratt_rules()`), the two parse functions handle token consumption differently:
- **Statement prefix** (`_async_call_stmt`): Called from `_declaration()` BEFORE the Pratt framework consumes the token → must call `self._advance()` to consume the keyword
- **Expression prefix** (`_async_call_expr`): Called by Pratt framework AFTER it already consumed the token → must use `self._previous()` to get the already-consumed keyword

Using `self._advance()` in the expression prefix causes the parser to skip the NEXT token (e.g., the agent name), leading to cryptic errors on the line AFTER the expression. Always check whether your parse function is called from `_declaration()` (needs `_advance()`) or from Pratt prefix rules (needs `_previous()`).

**Parser disambiguation**: When adding features that can be both statements and expressions (e.g., `llm act`), use lookahead + backtracking. See `references/parser-disambiguation.md` for the pattern, pitfalls, and testing approach.

### Removing Language Features: Full Pipeline Checklist

When removing a statement/keyword/expression type from Helen (e.g., `llm choose`, `call` keyword), follow this exact sequence:

1. **Interpreter** (`interpreter.py`): Remove `visit_xxx()` methods + remove node imports
2. **SemanticAnalyzer** (`analyzer.py`): Remove `visit_xxx()` methods + remove node imports
3. **Parser** (`parser.py`): Remove `_xxx()` parsing methods + remove dispatch branches + remove node imports + update error messages that reference removed keywords
4. **AST** (`ast.py`): Remove `XxxNode` dataclass + remove `visit_xxx` abstract method from `Visitor` + remove `visit_xxx` from `ASTPrinter`
5. **Tokens** (`tokens.py`): Remove `TokenType.XXX` enum entry + remove keyword map entry (if keyword is fully removed)
6. **Runtime** (`runtime/*.py`): Remove corresponding methods from `LLMRuntime` abstract base + all subclasses (`HttpLLMRuntime`, `HermesCLILLMRuntime`, `MockLLMRuntime`) + remove mock fields (e.g., `choose_return`, `choose_history`)
7. **Tests**: Delete feature-specific test files + remove test classes + update keyword count assertions + update method-presence assertions
8. **Docs** — update ALL affected files (same locations as adding features):
   - `~/wiki/helen/tutorial/*.md` — mirror changes
   - `README.md` — update feature lists if mentioned
9. **Run**: `pytest` (all pass) + git commit + push

**Example (2026-06): Removing `llm choose`**:
- Merged functionality into `llm if` (which now handles both routing and selection)
- Removed: `LlmChooseStmtNode`, `LlmOptionNode`, `visit_llm_choose_stmt`, `visit_llm_option`, `_llm_choose_stmt()`, `_llm_option()`, `TokenType.CHOOSE`, `TokenType.OPTION`, `choose()` from all runtimes
- Updated: keyword count 43→41, test count 901→886, parser dispatch, error messages
- Deleted: `tests/execution/test_llm_choose.py`

**Pitfall**: When removing a `TokenType`, also check `_block_body_list()` and similar methods that use token types as block terminators — they may reference the removed token.

**Pitfall**: When removing from `MockLLMRuntime`, also remove the corresponding `*_history` list field and update `reset()` to not reference it.

**Pitfall**: Test files may import removed AST nodes at the top level — these cause `ImportError` during test collection, not test failures. Delete entire test files for removed features rather than trying to patch them.

### `:undefine` Must Clean Three Registries

**Bug fixed 2026-06**: `:undefine` only removed symbols from `SymbolTable.global_scope` but left entries in `SemanticAnalyzer._agent_names` and `_function_param_types`. This caused "duplicate agent name" errors when redefining an agent after `:undefine`, even though `:list` showed it was gone.

**Fix**: `SemanticAnalyzer.undefine()` now cleans all three:
```python
def undefine(self, name: str) -> bool:
    removed = self.symbols.global_scope.undefine(name) is not None
    if name in self._agent_names:
        del self._agent_names[name]
        removed = True
    if name in self._function_param_types:
        del self._function_param_types[name]
        removed = True
    return removed
```

**Lesson**: When removing a definition, check ALL registries that track it — not just the symbol table. The interpreter has its own `_agents` and `_functions` dicts (cleaned by `undefine_agent`/`undefine_function`), and the analyzer has `_agent_names` and `_function_param_types`.

### Agent `prompt` Field → system_prompt in `llm act` (Implemented 2026-06)

The agent's `prompt` field is rendered as a template (`{{var}}` → environment values) and passed as `system_prompt` to the LLM runtime when `llm act` executes inside that agent's `main` block.

**How it works**:
1. `_get_rendered_agent_prompt()` reads `self._current_agent.prompt.content`
2. `_render_prompt_template()` replaces `{{var}}` via `environment.lookup()` (supports nested `{{a.b}}` for dicts)
3. The rendered string is passed as `system_prompt` to `llm_runtime.act()`
4. `HttpLLMRuntime` injects it as `{"role": "system"}` message; `HermesCLILLMRuntime` prepends it to user prompt

**Key detail**: `environment.lookup()` raises `NameError` for undefined vars — the renderer catches this and keeps the original `{{var}}` text (graceful degradation).

**Usage patterns**:
```helen
// Pattern 1: prompt as system instruction, llm act provides the task
agent Translator(text: str, target: str) {
    prompt """你是一个专业翻译。请将内容翻译成{{target}}。只输出翻译结果。"""
    main { return llm act text }
}

// Pattern 2: bare llm act — fully rely on agent prompt
agent Greeter(name: str) {
    prompt "请用中文向{{name}}打招呼"
    main { return llm act }
}

// Pattern 3: No prompt field — llm act prompt is self-contained (backward compatible)
agent Simple(text: str) {
    main { return llm act "大写：" + text }
}
```

**Key points**:
- `prompt` is optional — agents without it work fine (system_prompt=None)
- Template variables resolve from the agent's Environment (params, local vars)
- Undefined variables in `{{var}}` are kept as-is (no error)
- Single-pass rendering — rendered output is NOT re-rendered
- `description`, `model`, `temperature`, `max-turns`, `tools`, `skills`, `memory`, `sub-agents` are all optional config fields

### Helen Independent Configuration System (Implemented 2026-06)

Helen has its own configuration directory (`~/.helen/`) and no longer requires Hermes for configuration.

**Directory structure**:
```
~/.helen/
├── config.yaml    # LLM API configuration (priority)
├── config.yml     # Alternative YAML config
├── .env           # Alternative .env config
└── skills/        # Helen-native skill directory
```

**Config loading priority** (merge, not first-match — later sources override earlier):
1. `~/.hermes/.env` (base fallback)
2. `~/.helen/.env` (Helen .env)
3. `~/.helen/config.yml` (Helen YAML)
4. `~/.helen/config.yaml` (Helen YAML, highest priority)

**Key module**: `helen/runtime/config.py`
- `load_config()` — loads and merges from all sources
- `get_skill_dirs()` — returns skill dirs in priority order
- `get_helen_home()` — returns `~/.helen/`, creating if needed
- `save_config(config)` — writes `~/.helen/config.yaml`

**Config YAML format**:
```yaml
llm:
  base_url: "https://api.openai.com/v1"
  api_key: "your-key"
  model: "gpt-4"
  temperature: 0.7
  timeout: 60
```

**Config .env format** (supports multiple provider prefixes):
```
HELEN_API_KEY=xxx          # or DASHSCOPE_API_KEY, OPENAI_API_KEY
HELEN_BASE_URL=xxx         # or DASHSCOPE_BASE_URL, OPENAI_BASE_URL
HELEN_MODEL=gpt-4
```

**`helen init` command**: Creates `~/.helen/` with default `config.yaml` and `skills/` directory.

**Skill directory priority**:
1. `~/.helen/skills/` (Helen native)
2. `~/.hermes/skills/` (Hermes fallback)
3. `~/.hermes/hermes-agent/skills/` (Hermes agent skills)

**Pitfall**: Config loading is **merge, not first-match**. All existing sources are loaded and merged. A key in `config.yaml` overrides the same key in `.env`, but keys only in `.env` are still present. This means partial configs work — you can put API key in `.env` and model/temperature in `config.yaml`.

**Pitfall**: `_load_env_config()` recognizes multiple provider prefixes (`HELEN_*`, `DASHSCOPE_*`, `OPENAI_*`) for `API_KEY` and `BASE_URL`. This ensures backward compatibility with existing Hermes `.env` files.

### Built-in Tool Registry + Function Calling (Implemented 2026-06)

Helen has a built-in tool system that allows LLMs to call tools during `llm act` execution via OpenAI-compatible function calling.

**Architecture**:
```
helen/runtime/tools.py           — Tool registry (register_tool, get_tool_schemas, dispatch_tool)
helen/runtime/http_llm.py        — Function calling loop in act()
helen/interpreter/interpreter.py — _build_tools_list() reads agent tools declaration
```

**8 built-in tools**:
- `web_search(query, num_results=3)` — Wikipedia API (reliable, no API key needed). **Pitfall**: DuckDuckGo HTML scraping is blocked from most servers; Wikipedia summary + opensearch endpoints work reliably.
- `web_fetch(url)` — Fetch and extract text content from URL
- `read_file(path)` — Read local file content
- `write_file(path, content)` — Write to local file (creates parent dirs)
- `patch_file(path, old_string, new_string, replace_all=false)` — Precise file edit via fuzzy matching. Uses `helen/runtime/fuzzy_match.py` (9 strategies, copied from Hermes for independent operation). Returns unified diff.
- `shell_exec(command, timeout=30)` — Execute shell command, return stdout/stderr
- `calculate(expression)` — Safe math evaluation (AST-whitelisted, allows math functions)
- `load_skill(name)` — Load full SKILL.md content by name (Tier 2 of two-phase skill disclosure)

**Function calling flow**:
1. `_build_tools_list()` collects tool schemas from the `tools = [...]` allowlist only (two-layer authorization: `functions {}` declares capability, `tools` decides LLM visibility). Always includes `load_skill`. If `tools` is not declared, LLM gets no tools except `load_skill`. Names are resolved first in `functions {}` (Helen functions), then in the Python tool registry.
2. `HttpLLMRuntime.act()` enters loop: sends messages with tools → LLM returns `tool_calls` → `dispatch_tool()` executes → results fed back → repeat until text response or turns exhausted
3. On last tool turn, a "nudge" message is appended + one extra iteration for final text response

**Agent tools declaration**:
```helen
const FILE_TOOLS = ["read_file", "write_file"]

agent Researcher(query: str) {
    tools = FILE_TOOLS                    // 引用模块级 const（静态可审计）
    main { return llm act query }
}

agent SimpleAgent(text: str) {
    // no tools declaration — LLM has NO tools (only load_skill)
    main { return llm act text }
}
```

**Pitfall**: `tools` 严格校验——只接受模块级 const 引用或字面量列表。拒绝可变变量、函数、agent、未定义标识符、重复声明。这是**安全设计**，工具边界必须静态可追踪。

**Pitfall**: `max_turns` defaults to 1, but function calling needs at least 3 turns (tool call + tool result + final response). The interpreter auto-bumps `max_turns` to 3 when tools are present.

**Pitfall**: When the loop exhausts turns on tool calls, a nudge message is appended and one extra iteration runs (`for turn in range(max_turns + 1)`). Without this, the LLM might use all turns on tool calls and never produce text.

**Pitfall**: `calculate` tool's AST safety check must allow `ast.Load`, `ast.Attribute`, `ast.Num` nodes — these are needed for `math.sqrt()`, `2**10`, etc. Initial implementation rejected them as "unsafe".

**Pitfall: Tool Schema ≠ Tool Registration**. Adding a tool schema to `_build_tools_list()` makes it visible to the LLM (the LLM can see it and try to call it), but the tool won't actually work unless it's also registered in `_register_builtin_tools()`. This causes "Unknown tool: xxx" errors at runtime when the LLM tries to call it via function calling. **The fix requires BOTH**: (1) add schema to tool list (so LLM sees it), AND (2) register handler in `_register_builtin_tools()` (so `dispatch_tool()` can find it). Example: `load_skill` was added to `_build_tools_list()` but not registered, causing "Unknown tool: load_skill" errors. Fixed by adding `register_tool(name="load_skill", handler=_load_skill)` in `_register_builtin_tools()`.

### Integrating External Modules: Copy vs Dynamic Import

**Pattern choice**: When Helen needs functionality from Hermes (or other external sources), there are two approaches:

1. **Copy into Helen** (preferred for core functionality): Copy the module into `helen/runtime/` so Helen runs independently. Used for `fuzzy_match.py` (860 lines, 9 strategies) — now at `helen/runtime/fuzzy_match.py`.
2. **Dynamic sys.path import** (for optional/experimental): Add external path to `sys.path` at runtime. **Deprecated** — only use if the module is truly optional and Helen degrades gracefully without it.

**Current state**: `patch_file` tool uses `from helen.runtime.fuzzy_match import fuzzy_find_and_replace` — fully independent, no Hermes dependency.

### Adding Parameters to LLMRuntime Interface: Update ALL Subclasses

**Pitfall (found 2026-06)**: When adding a parameter to `LLMRuntime.act()`, you must update ALL subclasses — `HttpLLMRuntime`, `HermesCLILLMRuntime`, and `MockLLMRuntime`. Missing any one causes Pyright override errors or silent parameter drops.

**Checklist when modifying `LLMRuntime.act()` signature**:
1. `helen/runtime/llm_runtime.py` — abstract base + `MockLLMRuntime`
2. `helen/runtime/http_llm.py` — `HttpLLMRuntime.act()` and `_chat()`
3. `helen/runtime/hermes_cli_llm.py` — `HermesCLILLMRuntime.act()`
4. Run `pytest` — MockLLMRuntime tests may need updating if they assert on `act_history` dict keys

**Pattern**: Each runtime handles `system_prompt` differently:
- HttpLLMRuntime → separate `{"role": "system"}` message
- HermesCLILLMRuntime → prepend to user prompt string
- MockLLMRuntime → record in history dict, ignore for response

### Language Design: Simplify Overlapping Syntax

**Principle**: When two syntaxes achieve the same goal, consolidate to one. The user prefers fewer ways to do things.

**Example 1 (2026-06)**: `llm act Agent(args) "desc"`, `call Agent(args)`, and `Agent(args)` all called agents. Simplified by:
1. First deprecating `llm act` statement form (kept `call` and function-style)
2. Then deprecating `call` keyword entirely (commit 9cb0d97, 2026-06)

**Example 2 (2026-06)**: `llm if` and `llm choose` had overlapping functionality — both let LLM make decisions. `llm if` was more flexible (could execute code AND return values). Merged by removing `llm choose` entirely and keeping `llm if` as the unified construct.

**When to simplify**:
- Two syntaxes with identical semantics → keep the more intuitive one
- User asks "is X necessary?" → if the answer is "it overlaps with Y", deprecate X
- Function-style calls `Agent(args)` are preferred over keyword-style for consistency with function calls

**Deprecation pattern for keywords** (2026-06):
1. **Remove prefix registration**: Delete `self._rules[TokenType.XXX].prefix = self._xxx_kw` from `_register_pratt_rules()`
2. **Remove parsing method**: Delete the `_xxx_kw()` method entirely
3. **Update related syntax**: If other syntax uses the keyword (e.g., `async call`), update to not require it
4. **Update bare form detection**: Remove `TokenType.XXX` from any token lists that detect statement boundaries
5. **Update tests**: Replace all `call Agent(...)` with `Agent(...)` in test code
6. **Update documentation**: `~/wiki/helen/tutorial/*.md`
7. **Commit**: Clear message like "refactor: deprecate 'call' keyword, use function-style AgentName(args)"

**Important**: Don't emit deprecation warnings — just remove the syntax. Helen is pre-1.0, so breaking changes are acceptable. This keeps the parser simple and avoids maintaining deprecated code paths.

### Extending Existing Constructs: Literal-Only → Expression-Capable

**Pattern (2026-06)**: When a language construct currently accepts only literal values but should accept arbitrary expressions (e.g., `llm if "static"` → `llm if text+"dynamic"`).

**Example**: `llm if` description was changed from string-only to expression-capable, enabling dynamic prompt construction like `llm if text+"反映的情绪" { ... }`.

**Five-step upgrade pattern**:

1. **AST**: Change field type from `str` to `ExpressionNode | str` (backward compat) or just `ExpressionNode`:
   ```python
   # Before
   description: str
   # After
   description: object  # ExpressionNode (evaluated at runtime) or str (legacy)
   ```

2. **Parser**: Change from consuming a literal token to parsing a full expression:
   ```python
   # Before
   desc_tok = self._consume(TokenType.STRING, "Expected description after 'llm if'.")
   # After
   desc_expr = self._expression()  # Parse any expression
   ```
   Update the return statement to pass the expression node instead of extracting the literal value.

3. **Interpreter**: Add runtime evaluation with type check for backward compatibility:
   ```python
   if isinstance(node.description, str):
       desc_str = node.description  # Legacy path
   else:
       desc_val = node.description.accept(self)  # Evaluate expression
       desc_str = str(desc_val) if desc_val is not None else ""
   ```

4. **SemanticAnalyzer**: Analyze the expression for variable references and type checking:
   ```python
   if not isinstance(node.description, str):
       node.description.accept(self)  # Analyze expression
   ```

5. **Tests**: Update assertions to check for expression nodes instead of plain values:
   ```python
   # Before
   assert stmt.description == "classify"
   # After
   from helen.core.ast import LiteralNode
   assert isinstance(stmt.description, LiteralNode)
   assert stmt.description.value == "classify"
   ```

**When to use**:
- User requests dynamic values where only literals were allowed
- A construct's parameter should support variables, concatenation, or function calls
- Error messages indicate "expected STRING" but user wants to pass an expression

**Pitfall**: The `isinstance(node.field, str)` check in interpreter/analyzer provides backward compatibility for any code that still constructs AST nodes with plain strings (e.g., test fixtures, programmatic AST construction). Without this check, legacy code breaks.

**Related**: This is different from "Implementing New Language Features" (which adds entirely new syntax). This pattern upgrades existing syntax to be more flexible.

### `llm act` Expression Form

`llm act <expr>` is a first-class expression via `LlmActExprNode`:

```helen
// ✅ Expression form — evaluates prompt, calls LLM, returns response text
import std.core.*
return llm act "translate " + text + " to " + target
let result = llm act prompt
print(llm act "summarize this: " + content)

// ✅ Bare form inside agent — uses rendered prompt
agent Translator {
    prompt "Translate to French:"
    main { return llm act }
}

// ❌ DEPRECATED — use function-style Agent(args) or call Agent(args) instead
// llm act Translator(text="hello") "translate to French"
```

**Implementation**:
- **AST**: `LlmActExprNode(ExpressionNode)` with `prompt: ExpressionNode | None`
- **Parser**: Registered as `TokenType.LLM` prefix in Pratt parser (`_llm_act_expr()` method). Consumes `LLM`, `ACT`, then optionally parses a full expression as the prompt.
- **Interpreter**: `visit_llm_act_expr()` evaluates prompt expression → stringifies → calls `llm_runtime.act()` → returns response text
- **SemanticAnalyzer**: `visit_llm_act_expr()` visits the prompt expression for validation (if not None)

**Pitfall**: The LLM keyword is registered as BOTH a statement-level dispatch (in `_declaration()`) AND an expression prefix (in Pratt rules). The statement path handles `llm if`. The expression path handles `llm act`. The Pratt parser's prefix takes precedence when `llm` appears in expression position (after `return`, `=`, inside `()`, etc.).

### `llm act` Syntax (Simplified 2026-06)

**Statement form `llm act Agent(args) "desc"` is DEPRECATED.** Use `Agent(args)` instead (no `call` keyword needed).

Only two forms remain:

```helen
// 1. Expression form — direct LLM call with prompt
let result = llm act "translate hello to French"
llm act "Analyze sentiment of: " + review

// 2. Bare form — inside agent main, uses rendered prompt automatically
agent Translator(text: str, target: str) {
    prompt """
    Translate to {{target}}:
    {{text}}
    """
    main {
        let result = llm act    // bare form — auto-uses rendered prompt
        return result
    }
}

// Calling agents — function-style only (call keyword deprecated)
let translated = Translator(text="Hello", target="French")
```

**Parser emits deprecation error** for statement form:
```
'llm act Translator(...)' is deprecated. Use 'call Translator(...)' instead.
```

**Implementation**:
- **AST**: `LlmActExprNode(ExpressionNode)` with `prompt: ExpressionNode | None`
- **Parser**: `_llm_act_stmt()` checks for deprecated pattern and emits error. `_llm_act_expr()` handles bare form detection.
- **Interpreter**: `visit_llm_act_expr()` — if prompt is None/empty and in agent context, uses rendered agent prompt as user message.
- **SemanticAnalyzer**: `visit_llm_act_expr()` visits prompt expression (if not None).

**Removed**: `LlmActStmtNode` class, `visit_llm_act_stmt` methods.

### Bare `llm act` Inside Agent Context (Primary Pattern)

**Design**: When an agent's `prompt` template already contains all necessary information (rendered with `{{var}}`), `llm act` without arguments automatically uses the rendered agent prompt as the user message. This avoids redundant repetition.

```helen
agent Translator(text: str, target: str) {
    prompt """
    Translate to {{target}}:
    {{text}}
    """
    main {
        let result = llm act    // bare form — uses rendered prompt as user message
        return result
    }
}
// call Translator(text="Hello", target="French")
// → system: "Translate to French:\nHello"
// → user: "Translate to French:\nHello" (same content, auto-filled)
```

**Also works with empty string**: `llm act ""` is treated the same as bare `llm act` inside an agent context.

**Implementation**:

- **AST**: `LlmActExprNode.prompt` is `ExpressionNode | None`
- **Parser**: `_llm_act_expr()` checks if the next token is a statement terminator, statement keyword, or on a different line:
  ```python
  bare_form_tokens = (
      TokenType.RIGHT_BRACE, TokenType.SEMICOLON, TokenType.EOF,
      TokenType.RETURN, TokenType.LET, TokenType.CONST,
      TokenType.IF, TokenType.FOR, TokenType.WHILE,
      TokenType.BREAK, TokenType.CONTINUE, TokenType.MATCH,
      TokenType.TRY, TokenType.THROW,
      TokenType.LLM, TokenType.CALL, TokenType.ASYNC,
  )
  if self._check(*bare_form_tokens):
      prompt_expr = None
  elif self._current().line > act_token.line:
      prompt_expr = None  # newline = statement boundary
  else:
      prompt_expr = self._expression()
  ```
- **Interpreter**: `visit_llm_act_expr()` checks if prompt is empty and we're in an agent:
  ```python
  if not prompt and self._current_agent is not None:
      rendered = self._get_rendered_agent_prompt()
      if rendered:
          prompt = rendered
  ```

**Parser pitfall**: When making an expression optional after a keyword, check for ALL statement-starting tokens AND use newline boundary detection. Missing `return`, `if`, `let`, etc. causes the parser to consume the next statement as an expression. Newline boundary handles cases like `let result = llm act\nprint(...)` where `print` is an IDENTIFIER on a different line.

**Note**: `TokenType.CALL` was removed from `bare_form_tokens` in 2026-06 when the `call` keyword was deprecated. The list now only includes actual statement keywords.

**Design rationale**: This enables a clean pattern where the agent's `prompt` field is the complete instruction (rendered with parameters), and `llm act` just triggers execution without redundant content. The rendered prompt is sent as BOTH system and user message, which works well with most LLM APIs.

### LLM Runtime Architecture (Updated 2026-06)

**All runtimes now accept `system_prompt` parameter on `act()`:**

| Runtime | File | Speed | Mechanism | system_prompt handling |
|---------|------|-------|-----------|----------------------|
| **HttpLLMRuntime** | `helen/runtime/http_llm.py` | 7-11s/call | Direct HTTP to OpenAI-compatible API | Injected as `{"role": "system"}` first message |
| HermesCLILLMRuntime | `helen/runtime/hermes_cli_llm.py` | 15-17s/call | Spawns `hermes -z` subprocess | Prepended to user prompt |
| MockLLMRuntime | `helen/runtime/llm_runtime.py` | instant | Returns preset values | Recorded in `act_history` |

**HttpLLMRuntime** (REPL + script default):
- Auto-loads config via `helen.runtime.config.load_config()` — checks `~/.helen/config.yaml` first, falls back to `~/.hermes/.env`
- Default model: `qwen3.7-plus`
- Uses `/v1/chat/completions` endpoint (OpenAI-compatible)
- No subprocess overhead — pure HTTP via `urllib.request`
- `system_prompt` → `{"role": "system", "content": system_prompt}` as first message

**HermesCLILLMRuntime** (fallback):
- Uses `hermes -z <prompt>` (oneshot mode) — returns plain text, NOT JSON
- Model override: `-m <model>` not `--model`
- `system_prompt` prepended to user prompt (CLI has no separate system message support)

**Performance lesson**: `hermes -z` takes 15-17s because each call spawns a full Python process that loads hermes config, plugins, and auth. Direct HTTP calls skip all that overhead (~2x faster). For any LLM-heavy Helen program, prefer `HttpLLMRuntime`.

**Config auto-loading pattern** in `HttpLLMRuntime.__post_init__`:
```python
def __post_init__(self):
    if not self.base_url or not self.api_key:
        hermes_env = _load_hermes_env()  # delegates to helen.runtime.config.load_config()
        self.base_url = hermes_env.get("DASHSCOPE_BASE_URL", "https://coding.dashscope.aliyuncs.com/v1")
        self.api_key = hermes_env.get("DASHSCOPE_API_KEY", "")
        self.default_model = "qwen3.7-plus"
```

`_load_hermes_env()` now calls `load_config()` which merges from `~/.helen/config.yaml` → `~/.helen/.env` → `~/.hermes/.env`. The env-style dict keys (`DASHSCOPE_API_KEY`, `DASHSCOPE_BASE_URL`) are preserved for backward compatibility.

**Model name pitfall**: DashScope's coding endpoint only supports specific model names. `gpt-4`, `qwen-plus`, `qwen-max` all return HTTP 400 with `"model 'xxx' is not supported"`. Only `qwen3.7-plus` (the default) is confirmed working. Always check `hermes config show` for the actual model name, or omit the `model` field in agent declarations to use the default.

### `throw` Statement Implementation (Added 2026-06)

The `throw` keyword is fully implemented across the pipeline:
- **Lexer**: `THROW` keyword (43 total)
- **Parser**: `throw Type` or `throw Type(message)` — semicolon optional
- **AST**: `ThrowStmtNode(exception_type: TypeNode, message: ExpressionNode | None)`
- **Semantic**: Validates exception type is in `_PREDEFINED_EXCEPTIONS`
- **Interpreter**: Resolves type → evaluates message → raises corresponding `HelenRuntimeError` subclass

**Predefined exception hierarchy**:
```
AnyError (root)
├── LLMError
│   ├── TimeoutError
│   └── ModelError
├── ToolError
├── RuntimeError
└── AggregateError
```

**Syntax**:
```helen
import std.core.*
throw RuntimeError("something went wrong")  // with message
throw LLMError                               // default message

try {
    throw TimeoutError("timed out")
} catch LLMError err {    // inheritance: TimeoutError → LLMError
    print("LLM error: " + err.message)
} catch {
    print("unknown")
} finally {
    cleanup()
}
```

## Running Tests

```bash
cd ~/helen
~/.hermes/hermes-agent/venv/bin/python -m pytest  # full suite, 933 tests (~2s)
~/.hermes/hermes-agent/venv/bin/python -m pytest tests/runtime/  # runtime tests (memory, hermes CLI)
~/.hermes/hermes-agent/venv/bin/python -m pytest -k "async"  # 81 async-related tests
~/.hermes/hermes-agent/venv/bin/python tests/tutorial/run_tutorial_tests.py  # 49 pass, 0 fail, 31 skip
```

## Consolidated Tutorial

Tutorial source files at `~/wiki/helen/tutorial/*.md` (NOT a git repo).
See `references/tutorial-testing.md` for test runner details, skip categories, and pitfall patterns.

## Implementation Status (HLD v1.2.1)

### ✅ Fully Implemented (aligned with HLD)
| Module | Detail |
|--------|--------|
| M1 Lexer | 41 keywords, 77 tokens, maximal munch, hyphen kw disambiguation, SourceSpan |
| M2 Parser | Pratt parsing + recursive descent, 50+ AST nodes, panic-mode recovery, `llm` context kw disambiguation, `throw` stmt, `llm act` expr (statement form deprecated), bare `llm act` (optional expression), `async` as both statement and expression. Note: `llm choose` removed 2026-06 — use `llm if` for both routing and selection |
| M3 AST | SourceSpan on all nodes, mutable field, Visitor pattern |
| M4 SemanticAnalyzer | Symbol table, scope nesting, type checking, const protection, Agent boundary checks |
| M5 Interpreter (core) | All syntax features executable, Sentinel control flow, environment isolation, `throw` statement, `llm act` expression |
| M9 Type System | 14 types, Optional/Union/Literal, type compatibility checks |
| M10 Error Handler | Unified error format + SourceSpan |
| M11 CLI | `helen run/check/repl/docgen/init` |
| M14 Test Framework | 933 tests pass, pytest integration |
| M15 Stdlib | 25+ builtin functions |
| M16 History Manager | Token budget + trimming + conversation_summary (summary strategy is v2 per HLD) |
| M17 Structured Output | function calling schema + fallback to default |

### ⚠️ Partially Implemented
| Module | Gap |
|--------|-----|
| M6 Prompt Builder | ✅ Template rendering, Tier 1/2 skill injection fully working. Tier 1: `build_skill_index()` scans skill dirs, reads frontmatter, formats `<available_skills>` XML. Tier 2: `load_skill` tool registered in tools.py, dispatches to `_load_skill()` handler that searches skill dirs and returns full SKILL.md content. ⚠️ **Missing LRU cache** for Skill Index |
| M7 Runtime API | ✅ `HttpLLMRuntime` (direct HTTP, REPL+script default). ✅ `system_prompt` parameter on all `act()` methods. ✅ Built-in tool registry (8 tools) + function calling loop. ✅ `load_tool()` delegates to Helen tool registry. ✅ Independent config system (`~/.helen/config.yaml`, `helen init`). ✅ `patch_file` tool with local fuzzy matching engine (`helen/runtime/fuzzy_match.py`, 9 strategies, copied from Hermes). ⚠️ `get_token_count()` uses crude `len//4` estimate; `call_llm` delegates to injected `llm_runtime` |
| M8 Import Resolver | ✅ `.helen`/`.json`/`.md`. ⚠️ **Missing `.yaml`/`.yml`** support; **path security (`../` escape prevention)** not verified |
| M12 LSP Server | ✅ Diagnostics (full Lex→Parse→Analyze), completions (keywords+types+stdlib), go-to-definition. ⚠️ **Completions not position-filtered**; missing hover/signature help/rename |
| M13 VS Code Extension | ⚠️ TextMate syntax highlight only; **no LSP-backed completion/jump integration** |
| M14 Concurrency | ✅ **Replaced by `spawn` + Channel (v1.18, 2026-07)**. `spawn Agent(...)` spawns an agent in a daemon thread and returns a `Channel` (mailbox) for bidirectional communication (`send`/`receive`/`cancel`). `mailbox_select([m1, m2])` for multi-channel select. Old `async`/`await`/`detach` keywords and `AsyncLLMInterpreter`/`Task` classes removed. Environment isolation via full `snapshot()` deep copy (including SharedStore). See `references/spawnagent-proposal.md` for architecture details |

### ❌ Not Implemented (HLD v1.2.1 gaps found 2026-06)
| ❌ Not Implemented (HLD v1.2.1 gaps found 2026-06) |
|---|-----|---------------|--------|
| 1 | **MemoryProvider interface mismatch** | §3.8.2 v1.2.1 | HLD defines `load(path)/save(path,data)/get(path,key)/set(path,key,value)/search(path,query,top_k)`. Implementation has `get(key)/set(key)/delete(key)/list_keys()` — **missing `load`, `save`, `search` methods, and `path` parameter on all methods** |
| 2 | **Missing `search` semantic search** | §3.8.2 | `MemoryProvider.search()` with default fallback (text containment match) not implemented. VectorDB provider example in HLD also missing |
| 3 | **Missing `MarkdownMemoryProvider`** | §3.8.2 default impl table | HLD requires 3 defaults: File/Markdown/InMemory. Only File + InMemory exist |
| 4 | **`_memory_content` template injection missing** | §3.8.2 | Agent init should auto-call `MemoryProvider.load(path)` and inject `{{_memory_content}}`. Runtime has provider registration but **no auto-load/injection in interpreter** |
| 5 | **Language-level memory access syntax** | §3.8.1 supplement | `memory["key"]` and `memory.search("query")` — **no lexer tokens, no AST nodes, no interpreter visit methods** |
| 6 | ~~`load_tool` is still a stub~~ | ~~§3.8.3~~ | ~~✅ Implemented 2026-06: `helen/runtime/tools.py` — built-in tool registry with 8 tools (web_search, web_fetch, read_file, write_file, shell_exec, calculate, patch_file, load_skill). Function calling loop in HttpLLMRuntime. Agent `tools` declaration wired to runtime. Two-phase skill disclosure fully working: Tier 1 (Skill Index in System Prompt) + Tier 2 (load_skill tool for on-demand SKILL.md loading).~~ |
| 7 | ~~No direct LLM API provider~~ | ~~§3.6.5~~ | ~~✅ Implemented 2026-06: `HttpLLMRuntime` in `helen/runtime/http_llm.py` — direct HTTP calls to OpenAI-compatible API, auto-loads from `~/.helen/config.yaml` (fallback `~/.hermes/.env`), REPL+script default. 7-11s/call vs 15-17s for CLI.~~ |
| 8 | ~~Pre-defined exception hierarchy missing~~ | ~~§3.6.4~~ | ~~✅ Implemented 2026-06: `AnyError → LLMError → TimeoutError/ModelError, ToolError, RuntimeError`. `throw` statement also implemented.~~ |
| 9 | **Path security in ImportResolver** | §3.9.2, §7.1 | **No `../` escape prevention** verified in import path resolution |
| 10 | **Agent param validation completeness** | §3.3.6 | Parser supports `agent Name(params)`, but **call-time param name/type semantic validation** needs verification |
| ~~12~~ | ~~`patch_file` tool~~ | ~~§3.8.3~~ | ~~✅ Implemented 2026-06: `patch_file(path, old_string, new_string, replace_all=false)` in `helen/runtime/tools.py`. Uses local `helen/runtime/fuzzy_match.py` (9 strategies, 860 lines, copied from Hermes for independent operation). Returns unified diff.~~ |
| ~~13~~ | ~~`setup.py` for pip < 21.3~~ | ~~install compat~~ | ~~✅ Added 2026-06: minimal `setup.py` that delegates to `pyproject.toml`. Allows `pip install -e .` on pip < 21.3 (PEP 660 not supported).~~ |

The compiler pipeline (Lex → Parse → Analyze → Interpret) is complete for all language syntax. The biggest gaps are in the **Runtime layer**: Memory Provider interface doesn't match HLD v1.2.1, `load_tool` is a stub, no direct LLM API calls, and missing exception hierarchy.

See `references/tutorial-testing.md` for details on skipped features.
See `references/tutorial-sync.md` for the tutorial-implementation sync methodology and known mismatch patterns.
See `references/interpreter-sentinels.md` for sentinel architecture and double-execution pitfall.
See `references/hld-implementation.md` for HLD implementation patterns (Memory Providers, Hermes CLI LLM Runtime, skill scanning).
See `references/helen-config.md` for Helen independent configuration system (config loading, skill dirs, `helen init`).
See `references/fuzzy-match.md` for fuzzy match engine details (9 strategies, API, testing).
See `references/parser-optional-expression.md` for parser pattern: making expressions optional after keywords (bare `llm act`, etc.).
See `references/async-interpreter.md` for async interpreter architecture, hybrid async pattern, contract-first development approach, and performance benchmarks.

### Tutorial Test Runner: Bare Exceptions Fail

The tutorial test runner (`tests/tutorial/run_tutorial_tests.py`) executes each `helen` code block and expects it to complete without uncaught exceptions. Code blocks that raise uncaught exceptions (e.g., bare `throw RuntimeError("msg")` without a surrounding `try-catch`) are reported as test failures.

**Rule**: When writing tutorial examples that demonstrate `throw` or other exception-raising constructs, ALWAYS wrap them in `try-catch`:

```helen
// ❌ FAILS tutorial test — uncaught exception
import std.core.*
throw RuntimeError("something went wrong")

// ✅ PASSES tutorial test — exception is caught
try {
    throw RuntimeError("something went wrong")
} catch RuntimeError err {
    print("Caught: " + err.message)
}
```

This applies to any statement that intentionally raises: `throw`, division by zero examples, etc. Demonstration code must be self-contained and not crash the test runner.

### Helen Semicolon Rules

Helen has inconsistent semicolon usage that trips up test authors:
- **`let`/`const` declarations**: NO trailing semicolon → `let x = 42` (not `let x = 42;`)
- **Statements inside blocks**: semicolons ARE used → `throw RuntimeError("msg");`, `return x;`, `print("hi");`
- **`let` with semicolon causes parse error**: `let x = false;` → `E0301: Expected expression, got SEMICOLON`

When writing test code or tutorial examples, omit semicolons after `let`/`const` declarations but include them after expression statements and control flow statements inside blocks.

## Adding New Language Features

When extending Helen with new syntax (keywords, AST nodes, statements), follow the contract-first + TDD workflow documented in `references/streaming-implementation.md`.

Key steps:
1. Define contracts (Protocol classes) if applicable
2. Write tests FIRST (RED phase)
3. Implement across ALL layers: Lexer → Parser → AST → Semantic Analyzer → Interpreter → Runtime
4. Update test infrastructure: MockVisitor, keyword counts

Critical pitfalls:
- Must add visit methods to ALL visitors (ASTPrinter, SemanticAnalyzer, Interpreter, MockVisitor)
- Import resolver needs special handling for absolute paths in REPL
- Data files (.json/.md) need different duplicate handling than .helen files

## Streaming Output (Phase 1-3)

Helen now supports streaming LLM output through three mechanisms:

1. **Standard library**: `stream_print()`, `stream_clear()`, `progress_bar()`, cursor movement
2. **Syntax**: `llm act "prompt" [on_chunk callback] [on_complete callback]` (v1.14: streaming merged into `llm act`; `for await` and `StreamingResponse` removed in v1.18)

See `references/streaming-implementation.md` for complete implementation guide.

## Git Workflow

- Repo at `~/helen/` with HTTPS remote to `https://github.com/hahalee000000/helen`
- Git user: `Hellen Dev <hellen@dev>` (local config)
- HTTP proxy configured: `git config http.proxy` / `https.proxy` set
- GitHub PAT (Classic token) configured for push access
- **Commit AND push immediately after each fix/change** — do NOT wait for the user to ask "did you commit?" or "did you push?". When the user says "提交git", "提交", or "远程推送", they mean `git commit` + `git push`. Always do both in one step: `git add -A && git commit -m "..." && git push origin master`.
- **Always run `pytest` before committing** — full test suite is fast (~2s)
- Commit message format: `fix: ...`, `refactor: ...`, `chore: ...`
- **After code changes, always update docs** — see step 7 above for the full doc update checklist. At minimum: `README.md` (test count, status). For runtime/CLI changes: also update wiki (`index.md`, `changelog.md`, `hld-compliance.md`, `architecture.md`, relevant topic page). Wiki is NOT a git repo — only helen project gets committed/pushed.
- **Wiki updates must be explicit and visible**: When updating `~/wiki/helen/tutorial/*.md`, list the updated files in your response so the user can verify. Wiki is a separate directory (not git-tracked), so updates are easy to miss. After updating wiki files, say "Updated wiki tutorials: 05-agents.md, 06-llm-statements.md, ..." to make the changes visible.
