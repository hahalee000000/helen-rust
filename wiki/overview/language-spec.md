# Helen Language Specification

> Version: v1.18 | Keywords: 89 (44.5 English + 44.5 Chinese) | Token Types: 83 | AST Nodes: 60 | Visitor Methods: 54

---

## Keyword Overview (89)

### Chinese Keywords (v1.10)

Helen supports bilingual keywords. Chinese keywords map to the same TokenTypes as English keywords, requiring no changes to the parser or interpreter. Chinese identifiers (variable names, function names) are also fully supported.

| English | Chinese | Description |
|---------|---------|-------------|
| `let` | `让` | Mutable variable |
| `const` | `常量` | Constant |
| `shared` | `共享` | Cross-agent visible variable (v1.10) |
| `fn` | `函数` | Function declaration |
| `return` | `返回` | Function return |
| `if` / `else` | `如果` / `否则` | Conditional branch |
| `for` / `in` | `对于` / `属于` | Loop |
| `while` | `当` | Conditional loop |
| `break` / `continue` | `中断` / `继续` | Loop control |
| `match` / `case` / `default` | `匹配` / `情况` / `默认` | Pattern matching |
| `try` / `catch` / `finally` | `尝试` / `捕获` / `最终` | Exception handling |
| `throw` | `抛出` | Throw exception |
| `assert` | `断言` | Runtime assertion |
| `true` / `false` | `真` / `假` | Boolean values |
| `null` | `空` | Null value |
| `is` | `是` | Type check |
| `agent` | `智能体` | Declare Agent |
| `llm` | `大模型` | LLM operations |
| `act` | `执行` | Autonomous execution (supports on_chunk/on_complete streaming callbacks) |
| `spawn` | `分生` | Concurrent agent spawning (v1.18) |
| `prompt` | `提示` | System prompt |
| `description` | `描述` | Agent description |
| `model` | `模型` | Specify model |
| `tools` | `工具` | Available tools |
| `streaming` | `流式输出` | Enable streaming |
| `temperature` | `温度` | Temperature parameter |
| `max-turns` | `最大轮次` | Maximum turns |
| `functions` | `函数区` | Function definition block |
| `main` | `主` | Entry point |
| `import` / `as` | `导入` / `作为` | Module import |
| `protocol` / `impl` | `协议` / `实现` | Protocol declaration |
| `call` | `调用` | Call |
| `branch` | `分支` | Branch |
| `alias` | `别名` | Function/variable alias (v1.10) |
| `store` | `仓库` | Shared Store declaration (v1.12) |

### Agent Declarations (11)

| Keyword | Purpose | Example |
|---------|---------|---------|
| `agent` | Declare Agent | `agent Translator { ... }` |
| `description` | Agent description | `description "Translate text"` |
| `model` | Specify model | `model "gpt-4"` |
| `tools` | LLM-visible tool whitelist (two-tier authorization) | `tools = ["web_search", "my_fn"]` |
| `skills` | Agent skill index | `skills ["web-research"]` |
| `sub-agents` | Sub-agent declaration | `sub-agents { ... }` |
| `memory` | Persistent memory | `memory "file://mem.json"` |
| `temperature` | Sampling temperature | `temperature 0.7` |
| `max-turns` | Maximum interaction turns | `max-turns 3` |
| `prompt` | Prompt definition | `prompt """..."""` |
| `context` | Context configuration block (v1.15) | `context { compression "graduated" }` |

### LLM Statements (5)

| Keyword | Purpose | Example |
|---------|---------|---------|
| `llm` | LLM context keyword | `llm if / llm act` |
| `act` | LLM autonomous execution | `llm act target "desc"` |
| `branch` | Conditional branch | `branch "urgent": ...` |

### Control Flow (15)

| Keyword | Purpose | Example |
|---------|---------|---------|
| `if` / `else` | Conditional branch | `if x > 0 { ... } else { ... }` |
| `for` / `in` | Iteration loop | `for item in list { ... }` |
| `while` | Conditional loop | `while condition { ... }` |
| `break` | Exit loop | `break` |
| `continue` | Skip current iteration | `continue` |
| `match` / `case` | Pattern matching | `match x { case 1: ... }` |
| `default` | Default branch | `default: { ... }` |
| `try` / `catch` | Exception handling | `try { ... } catch RuntimeError { ... }` |
| `finally` | Final block | `finally { cleanup() }` |
| `assert` | Runtime assertion | `assert x > 0, "must be positive"` |

### Functions and Calls (3)

| Keyword | Purpose | Example |
|---------|---------|---------|
| `fn` | Function declaration | `fn add(a, b) { return a + b }` |
| `call` | Call Agent | `call Translator(text)` |
| `spawn` | Concurrent agent spawn (v1.18) | `let m = spawn Worker("task")` |

### Variables and Types (5)

| Keyword | Purpose | Example |
|---------|---------|---------|
| `let` | Mutable variable | `let x = 42` |
| `const` | Immutable constant | `const PI = 3.14` |
| `shared` | Cross-agent visible variable (v1.10) | `shared let counter = 0` |
| `true` | Boolean true | `let ok = true` |
| `false` | Boolean false | `let done = false` |

### Other (5)

| Keyword | Purpose | Example |
|---------|---------|---------|
| `import` | Module import | `import "./utils.helen"` |
| `return` | Return value | `return result` |
| `as` | Alias | `import "./lib" as utils` |
| `functions` | Agent capability block (available to main; requires tools authorization for LLM) | `functions { fn x() {} }` |
| `main` | Agent main block | `agent A { main { ... } }` |
| `null` | Null value | `let x = null` |

**Note**: The `main` block is only valid within an `agent` declaration body, or as a single top-level entry point. Top-level programs consist of **declaration sequences** (`fn`/`agent`/`const`/`import`/`alias`/`shared let`/`shared store`/`protocol`/`impl`) plus at most one top-level `main {}` block. As of v1.30, module-level `let` is forbidden (E0355 TOP_LEVEL_STATEMENT) — all executable code must be inside `main {}` or inside a function/agent body.

---

## Token Types (86)

### Literals (7)
`NUMBER` `STRING` `TRIPLE_QUOTE_STRING` `TRUE` `FALSE` `NULL_KW` `IDENTIFIER`

### Identifiers and Symbols (5)
`IDENTIFIER` `DOT` `DOTDOT` `COMMA` `SEMICOLON`

### Brackets (6)
`LEFT_PAREN` `RIGHT_PAREN` `LEFT_BRACE` `RIGHT_BRACE` `LEFT_BRACKET` `RIGHT_BRACKET`

### Operators (18)
`PLUS` `MINUS` `STAR` `SLASH` `PERCENT` `BANG` `BANG_EQUAL` `ASSIGN` `EQUAL_EQUAL` `GREATER` `GREATER_EQUAL` `LESS` `LESS_EQUAL` `ARROW` `PIPE` `PIPE_RIGHT` `QUESTION` `AT`

**v1.12**: `AT` (`@`) used for isolation level annotations (`@open`/`@strict`/`@sandbox`)

### Keyword Tokens (43)
Each keyword corresponds to a TokenType: `AGENT` `DESCRIPTION` `MODEL` `TOOLS` `STREAMING` `TEMPERATURE` `MAX_TURNS` `MEMORY` `PROMPT` `LLM` `IMPORT` `LET` `CONST` `IF` `ELSE` `FOR` `WHILE` `BREAK` `CONTINUE` `RETURN` `MATCH` `CASE` `BRANCH` `DEFAULT` `ACT` `TRY` `CATCH` `FINALLY` `THROW` `ASSERT` `FN` `AS` `IN` `FUNCTIONS` `MAIN` `PROTOCOL` `IMPL` `IS` `WILDCARD` `SHARED` `NULL_KW` `TRUE` `FALSE` `ALIAS` `STORE` `SPAWN`

**v1.12**: Added `STORE`. **v1.14**: Removed `STREAM`. **v1.18**: Removed `ASYNC`/`AWAIT`/`DETACH`/`CHANNEL`; added `SPAWN`.

### Templates (2)
`TEMPLATE_OPEN` `TEMPLATE_CLOSE`

### Special (1)
`EOF`

---

## AST Nodes (60)

### Expression Nodes (15)
`LiteralNode` `VariableNode` `BinaryOpNode` `UnaryOpNode` `GroupingNode` `CallNode` `CallArgNode` `AccessNode` `IndexNode` `ListLiteralNode` `MapLiteralNode` `MapEntryNode` `PipeExprNode` `MatchExprNode` `SpawnExprNode`

**v1.18**: Added `SpawnExprNode`. Removed `AsyncCallExprNode`.

### Statement Nodes (22)
`ExprStmtNode` `VarDeclNode` `DeclarationNode` `IfStmtNode` `ForStmtNode` `WhileStmtNode` `BreakStmtNode` `ContinueStmtNode` `ReturnStmtNode` `MatchStmtNode` `CaseNode` `TryStmtNode` `CatchClauseNode` `CatchAllNode` `FinallyBlockNode` `FunctionDeclNode` `FnBlockNode` `ImportStmtNode` `TemplateRefNode` `AgentParamNode` `MainBlockNode` `LambdaNode` `AliasStmtNode`

**v1.18**: Removed `AsyncCallStmtNode`, `DetachStmtNode`, `ForAwaitStmtNode`.

### Protocol/Implementation Nodes (2)
`ProtocolDeclNode` `ImplDeclNode`

### LLM Nodes (3)
`LlmActExprNode` `LlmIfStmtNode` `LlmBranchNode`

**v1.14**: `LlmActStmtNode` and `LlmStreamStmtNode` removed; unified into `LlmActExprNode` (added `on_chunk`/`on_complete` optional fields).

### Declaration Nodes (5)
`AgentDeclNode` `ContextConfigNode` `SharedStoreDeclNode` `TypeNode` `OptionalTypeNode` `UnionTypeNode` `LiteralTypeNode`

**v1.12**: Added `SharedStoreDeclNode`. **v1.15**: Added `ContextConfigNode`. **v1.18**: Removed `ChannelDeclNode`. `AgentDeclNode` gained `isolation_level` and `context_config` fields.

### Program Node (1)
`ProgramNode`

### Base Nodes (3)
`StatementNode` (ABC) `ExpressionNode` (ABC) `ASTNode` (ABC)

---

## Type System (14)

| Type | Description | Helen Syntax |
|------|-------------|-------------|
| `AnyType` | Dynamic type | Default inferred |
| `BoolType` | Boolean | `bool` |
| `IntType` | Integer | `int` |
| `FloatType` | Float | `float` |
| `NumberType` | Number base class | — |
| `StringType` | String | `str` |
| `NullType` | Null value | `null` |
| `OptionalType` | Optional `T?` | `str?` |
| `ListType` | List | `list<int>` |
| `MapType` | Map | `map<str, int>` |
| `UnionType` | Union `A\|B` | `int \| str` |
| `LiteralType` | Literal | `"hello"` |
| `AgentType` | Agent type | `Agent<Translator>` |

---

## Architecture Overview

```
                    ┌──────────────────────────────────────┐
                    │          helen main.helen           │
                    └──────────────────┬───────────────────┘
                                       │
                    ┌──────────────────▼───────────────────┐
                    │              CLI (M11)               │
                    │   run → Lex→Parse→Analyze→Interpret  │
                    │   check → Lex→Parse→Analyze          │
                    │   repl → Interactive loop            │
                    └──────────────────┬───────────────────┘
                                       │
         ┌─────────────────────────────┼─────────────────────────────┐
         │                             │                             │
    ┌────▼────┐                   ┌────▼────┐                   ┌────▼────┐
    │  M1:    │                   │  M2:    │                   │  M3:    │
    │ Lexer   │──Token[83]──────▶│ Parser  │──AST[60]────────▶│  AST    │
    │         │                   │Pratt×10 │                   │Visitor×54│
    └─────────┘                   └─────────┘                   └────┬────┘
                                                                     │
         ┌─────────────────────────────┬──────────────────────┬──────▼──────┐
         │                             │                      │             │
    ┌────▼────┐                   ┌────▼────┐           ┌─────▼─────┐ ┌────▼─────┐
    │  M4:    │                   │  M9:    │           │  M5:      │ │  M10:    │
    │Semantic │                   │ Types   │           │Interpreter│ │ Errors   │
    │Analyzer │                   │ ×14     │           │ +LLM     │ │ ×42 codes│
    └─────────┘                   └─────────┘           └─────┬─────┘ └──────────┘
                                                              │
         ┌─────────────────────────────┬──────────────────────┼──────────────┐
         │                             │                      │              │
    ┌────▼────┐                   ┌────▼────┐           ┌─────▼─────┐  ┌────▼────┐
    │  M6:    │                   │  M7:    │           │  M8:      │  │  M16:   │
    │Prompt   │                   │ Runtime │           │Import     │  │History  │
    │Builder  │                   │ABC×12   │           │Resolver   │  │Manager  │
    └─────────┘                   └─────────┘           └───────────┘  └─────────┘
```

---

## v1.10 New Features

### 1. shared let — Cross-Agent Visible Variables (v1.10)

```helen
shared let counter = 0

agent Worker {
  main {
    counter += 1  // can access and modify shared let
  }
}
```

**Keywords**: `shared` / `共享`

**Rules** (updated in v1.30):
- Module-level `let` is **forbidden** (E0355 TOP_LEVEL_STATEMENT); use `const`, `shared let`, or `shared store` instead
- Module-level `const` is automatically visible (read-only sharing)
- `shared let` explicitly declares cross-agent visible mutable variables
- Imported `shared let` is tracked correctly

### 2. Agent Scope Isolation (v1.10)

```helen
// ❌ let moduleVar = "module-level"  // E0355: module-level let is forbidden (v1.30)

agent MyAgent {
  let agentVar = "agent-level"  // agent scope
  
  main {
    // moduleVar  // ❌ Compile error: module-level let not visible
    // agentVar   // ❌ Compile error: agent-scoped variable not visible
    let localVar = "local"  // ✅ Local variable
  }
}
```

**Rules** (updated in v1.30):
- `agent main {}` runs in a fully isolated environment
- Module-level `let` is forbidden entirely (E0355)
- Module-level `const` is automatically visible (read-only)
- Use `shared let` for cross-agent visibility
- Closures in agent main can capture local variables

### 3. Subscript/Field Assignment (v1.10)

```helen
main {
    let arr = [1, 2, 3]
    arr[0] = 10  // ✅ Array index assignment

    let obj = { name: "Alice", age: 30 }
    obj.age = 31  // ✅ Object field assignment
}
```

### 4. Short-Circuit Evaluation (v1.10)

```helen
main {
    let x = false && expensiveCall()  // expensiveCall() is not executed
    let y = true || expensiveCall()   // expensiveCall() is not executed
}
```

**Operators**: `&&` and `||` short-circuit

### 5. Return Type Annotation Syntax (v1.10)

```helen
// ✅ New syntax (only supported form)
fn add(a: int, b: int): int {
  return a + b
}

// ❌ Old syntax (removed)
// fn add(a: int, b: int) -> int { ... }
```

### 6. Enhanced Exception Handling (v1.10)

```helen
main {
    try {
        // Python stdlib exceptions are wrapped as RuntimeError
        let result = int("not a number")
    } catch RuntimeError as e {
        print(e.message)  // Includes original exception info
    }
}
```

### 7. Async HTTP Support (v1.10, removed in v1.18)

As of v1.18, `act_async()` / `act_stream_async()` are removed, replaced by `spawn` + Channel.

```helen
// v1.18 approach: concurrent agent calls
main {
    let m1 = spawn AgentA("task1")
    let m2 = spawn AgentB("task2")
    let r1 = m1.receive()
    let r2 = m2.receive()
}
```

---

## v1.11-v1.14 New Features

### v1.11: shared let Write-Back + Exception Hierarchy Fix

- After modifying `shared let` inside an agent, the new value is automatically written back to the global environment
- Exception hierarchy fix: `HelenRuntimeError` hierarchy made more precise

### v1.12: Agent Isolation Enhancement + Shared Store

#### 1. Isolation Level Annotations

```helen
@open agent Permissive { main { ... } }      // Can access module let
@strict agent Strict { main { ... } }        // Deep-copies shared let
@sandbox agent Sandbox { main { ... } }      // Disables external tools; prohibits shared let
```

| Isolation Level | Module let | Module const | shared let | External Tools |
|-----------------|------------|--------------|------------|----------------|
| Standard (default) | ❌ | ✅ | ✅ | ✅ |
| `@open` | ✅ | ✅ | ✅ | ✅ |
| `@strict` | ❌ | ✅ | ✅ (deep copy) | ✅ |
| `@sandbox` | ❌ | ✅ | ❌ | ❌ |

#### 2. Shared Store

```helen
shared store Cache {
    data: dict = {}
    _lock_count: int = 0  // Private field (_ prefix)

    fn get(key): any { return data[key] }
    fn set(key, value) { data[key] = value }
}
```

#### 3. Key Isolation Fixes

- Parameter defaults and `functions{}` variables are evaluated in the agent environment
- Reference-type parameters are automatically wrapped in `ReadOnlyView`
- Closures use value capture (instead of environment reference capture)
- `shared let` restricted to value types (int/float/str/bool)
- Compound assignment (`arr[i]=x`, `obj.field=x`) isolation checks fixed

### v1.13: Channel + Chinese Keyword Completion (updated in v1.18)

As of v1.18, the `channel X { fields }` declaration syntax is removed. Channels are now created via the `Channel()` constructor or `spawn`, and serve as message queues (send/receive).

```helen
// v1.18 new syntax: create channel via spawn
main {
    let mailbox = spawn Worker("task")
    let result = mailbox.receive()

    // Or create via constructor
    let pipe = Channel()
}
```

- `Channel` type supports `send`/`receive`/`try_receive`/`cancel`/`close`/`is_closed` methods
- Chinese method aliases: `发送`/`接收`/`尝试接收`/`取消`/`关闭`/`已关闭`

### v1.14: Merged llm stream into llm act

**Breaking change**: The `llm stream` keyword is removed.

```helen
// Old syntax (v1.13 and earlier) — would be inside main {}
// llm stream "write a long essay" on_chunk handle_chunk

// New syntax (v1.14+)
main {
    llm act "write a long essay" on_chunk handle_chunk
}
```

- `llm act` supports optional `on_chunk`/`on_complete` callbacks
- No callbacks → synchronous execution (`act()`)
- With callbacks → streaming execution (`act_stream()`)
- Keyword count 94 → 92; `stream` is no longer a keyword
- `STREAM` TokenType removed; `LlmStreamStmtNode` removed

### v1.18: spawn Concurrency Primitive

**Breaking change**: The `async`/`await`/`detach` keywords and `channel X { fields }` declaration syntax are removed, replaced by `spawn` + Channel message queue.

#### 1. spawn Syntax

```helen
agent Worker(task: str, reply: Channel) {
    main {
        reply.send("Result: " + task)
    }
}

main {
    let mailbox = spawn Worker("data analysis")
    let result = mailbox.receive()
}
```

#### 2. Channel Message Queue

- `Channel` type (no longer uses `channel X { fields }` declaration syntax)
- Methods: `send`/`receive`/`try_receive`/`cancel`/`close`/`is_closed`
- Chinese aliases: `发送`/`接收`/`尝试接收`/`取消`/`关闭`/`已关闭`
- `mailbox_select([m1, m2, ...])` stdlib function for multiplexing

#### 3. Snapshot Semantic Change

- **Full deep copy, no exceptions** (including SharedStore)
- Sharing is done explicitly by passing references through channels

#### 4. Removal List

- Keywords: `async`/`await`/`detach`/`channel` (declaration) + `异步`/`等待`/`分离`/`通道` (declaration)
- AST nodes: `AsyncCallStmtNode`/`AsyncCallExprNode`/`DetachStmtNode`/`ChannelDeclNode`/`ForAwaitStmtNode`
- Files: `async_interpreter.py`/`task.py` removed
- Added: `SpawnExprNode`, `channel.py`, `mailbox.py`
- Keyword count 97 → 89 (44.5 English + 44.5 Chinese)

---

## Related Pages

- [[syntax/keywords|Keyword Reference]]
- [[syntax/grammar|Grammar Specification]]
- [[compiler/ast|AST Node Definitions]]
- [[interpreter/execution|Execution Engine]]
- [[interpreter/spawn|Concurrency and spawn]]
- [[reference/05-agents|Agent Programming Tutorial]]
- [[reference/07-spawn|Concurrency Programming Tutorial]]

---

**Last updated**: 2026-07-13  
**Version**: v1.18
