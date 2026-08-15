<!-- helen-rust edition: 99 keywords (48 EN + 51 CN) identical; reserved-keyword rules enforced in parser + analyzer. -->

# Keyword Reference

> 89 keywords (44.5 English + 44.5 Chinese) | Grouped by function | See also `@` isolation annotation operator (v1.12)

---

## Chinese Keywords (v1.10)

Helen supports bilingual keywords. Chinese keywords and English keywords map to the same TokenType and can be freely mixed.

| English | Chinese | Description |
|---------|---------|-------------|
| `let` | `设` | Mutable variable (legacy: 让、定义) |
| `const` | `常量` | Constant |
| `shared` | `共享` | Cross-agent visible variable (v1.10) |
| `store` | `仓库` | Shared Store declaration (v1.12) |
| `fn` | `函数` | Function declaration |
| `return` | `返回` | Function return |
| `if` / `else` | `如果` / `否则` | Conditional branching |
| `for` / `in` | `对于` / `属于` | Loop |
| `while` | `当` | Conditional loop |
| `break` / `continue` | `中断` / `继续` | Loop control |
| `match` / `case` / `default` | `匹配` / `情况` / `默认` | Pattern matching |
| `try` / `catch` / `finally` | `尝试` / `捕获` / `最终` | Exception handling |
| `throw` | `抛出` | Throw exception |
| `assert` | `断言` | Runtime assertion |
| `true` / `false` | `真` / `假` | Boolean literals |
| `null` | `空` | Null value |
| `is` | `是` | Type check |
| `agent` | `智能体` | Declare Agent |
| `llm` | `大模型` | LLM operations |
| `act` | `执行` | Autonomous execution (supports on_chunk/on_complete streaming callbacks) |
| `spawn` | `分生` | Concurrent agent spawn (v1.18) |
| `prompt` | `提示` | System prompt |
| `description` | `描述` | Agent description |
| `model` | `模型` | Specify model |
| `tools` | `工具` | Available tools |
| `streaming` | `流式输出` | Enable streaming |
| `temperature` | `温度` | Temperature parameter |
| `max-turns` | `最大轮次` | Maximum turns |
| `max-tokens` | `最大tokens` | Max output tokens (v1.31.2) |
| `thinking-mode` | `思考模式` | Enable thinking mode (v1.36) |
| `reasoning-effort` | `推理强度` | Reasoning effort level (v1.36) |
| `provider` | `提供商` | Explicit provider override (v1.36) |
| `functions` | `函数区` | Function definition block |
| `main` | `主函` | Entry point |
| `import` / `as` | `导入` / `作为` | Module import |
| `protocol` / `impl` | `协议` / `实现` | Protocol declaration |
| `call` | `调用` | Call |
| `branch` | `分支` | Branch |
| `alias` | `别名` | Function/variable alias (v1.10) |
| `context` | `上下文` | Context configuration block (v1.15) |
| `compression` | `压缩` | Compression strategy (v1.15) |
| `cache-aware` | `缓存感知` | Cache-aware toggle (v1.15) |
| `working-memory` | `工作记忆` | Working memory toggle (v1.15) |
| `working-memory-tokens` | `工作记忆词元` | Working memory budget (v1.15) |

Chinese identifiers (variable names, function names) are also fully supported — all CJK unified ideographs can be used as identifier characters.

```helen
// Pure Chinese programming
函数 斐波那契(n: int): int {
    如果 n <= 1 {
        返回 n
    } 否则 {
        返回 斐波那契(n - 1) + 斐波那契(n - 2)
    }
}

设 结果 = 斐波那契(10)

// Mixed Chinese-English
常量 LIMIT = 100
如果 结果 < LIMIT {
    print("OK")
}

// v1.10: Shared variable
共享 let counter = 0
```

---

## Agent Declarations (10)

### `agent`
Declares an Agent.

```helen
agent Translator {
    description "Translate text"
    prompt "You are a translator."
}

// v1.12: Isolation level annotations
@open agent Permissive { main { ... } }      // Can access module let
@strict agent Strict { main { ... } }        // Deep-copies shared let
@sandbox agent Sandbox { main { ... } }      // Disables external tools
```

**`@` operator (v1.12)**: `@` is a single-character token (`TokenType.AT`) used for isolation level annotations before agent declarations. Supports three isolation levels: `@open`, `@strict`, `@sandbox`.

### `description`
Human-readable description of the Agent, injected into the system prompt.

```helen
description "Translate between English and French"
```

### `model`
Specifies the LLM model.

```helen
model "gpt-4"
```

### `tools`
List of tools available to the Agent.

```helen
tools [web_search, calculator]
```

### `sub-agents`
Sub-agent declarations.

```helen
sub-agents {
    Translator: translate text
    Summarizer: summarize content
}
```

### `memory`
Persistent memory configuration.

```helen
memory "file://memories/translator.json"
```

### `temperature`
LLM sampling temperature (0.0–2.0).

```helen
temperature 0.7
```

### `max-turns`
Maximum interaction turns.

```helen
max-turns 3
```

### `max-tokens`
Maximum output tokens for LLM response (v1.31.2).

```helen
max-tokens 4096
```

### `thinking-mode`
Enable thinking/reasoning mode for LLM (v1.36). When enabled, the model produces a chain-of-thought reasoning before the final answer.

```helen
thinking-mode true
```

### `reasoning-effort`
Control reasoning effort level (v1.36). Values: `low`, `medium`, `high`, `max`.

```helen
reasoning-effort "high"
```

### `provider`
Explicit provider override (v1.36, context keyword). Used to override auto-detected platform protocol.

```helen
provider "zhipu"
```

### `prompt`
Agent prompt.

```helen
prompt """
You are a helpful assistant.
Follow these instructions carefully.
"""
```

---

## LLM Statements (3)

### `llm`
LLM context keyword, used in combination with `act`/`if`.

### `act`
Lets the LLM autonomously execute a task (v1.14+ supports streaming callbacks).

```helen
// Synchronous execution
llm act Translator(text) "Translate to French"

// Streaming execution (v1.14+, replaces old llm stream syntax)
fn handle_chunk(chunk) { print(chunk) }
fn handle_complete(final) { print("Done: " + final) }
llm act "Write a long essay" on_chunk handle_chunk on_complete handle_complete
```

**v1.14 change**: `llm stream` has been removed; streaming functionality is integrated into `llm act` via `on_chunk`/`on_complete` callbacks.

---

## Context Configuration (v1.15+)

### `context`

Configures the agent's context management strategy.

```helen
agent SmartAssistant {
    context {
        compression "graduated"      // Compression strategy
        cache-aware true             // Cache-aware
        working-memory true          // Working memory
        working-memory-tokens 5000   // Working memory budget
    }
}
```

### Configuration Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `compression` | str | `"graduated"` | `"none"` / `"graduated"` / `"traditional"` |
| `cache-aware` | bool | `true` | Enable cache-aware compression |
| `working-memory` | bool | `true` | Enable working memory |
| `working-memory-tokens` | int | `5000` | Working memory token budget |

### Chinese Keywords

```helen
agent 智能助手 {
    上下文 {
        压缩 "graduated"
        缓存感知 true
        工作记忆 true
        工作记忆词元 5000
    }
}
```

---

## Control Flow (16)

### `if` / `else`
Conditional branching.

```helen
if x > 0 {
    print("positive")
} else {
    print("non-positive")
}
```

### `for` / `in`
Iteration loop.

```helen
for item in [1, 2, 3] {
    print(item)
}
```

### `while`
Conditional loop.

```helen
while x < 10 {
    let x = x + 1
}
```

### `break`
Exits the current loop.

### `continue`
Skips the current iteration.

### `match` / `case` / `default`
Pattern matching.

```helen
// Basic pattern matching
match status {
    case "ok": print("Success")
    case "error": print("Failed")
    default: print("Unknown")
}

// v1.5: Range matching
match score {
    case 90..100 { print("A") }
    case 80..89 { print("B") }
    default { print("F") }
}

// v1.5: Guard conditions
match x {
    case 1..100 if x == 42 { print("the answer") }
    case 1..100 { print("in range") }
    default { print("out of range") }
}

// v1.8: Wildcard pattern (can serve as default branch)
match value {
    case 1 { print("one") }
    case _ { print("other") }  // Matches any value
}

// v1.8: Variable binding
match value {
    case n if n > 0 { print("positive: " + str(n)) }
    case n if n < 0 { print("negative: " + str(n)) }
    case _ { print("zero") }
}

// v1.8: Type patterns
match value {
    case is String { print("it's a string") }
    case is Int { print("it's an int") }
    case _ { print("unknown type") }
}

// v1.8: Type pattern with binding
match value {
    case is String s { print("string: " + s) }
    case _ { print("not a string") }
}
```

### `try` / `catch` / `finally` / `throw`
Exception handling and throwing.

```helen
// Throwing exceptions
throw RuntimeError("something went wrong")
throw LLMError  // Uses default message

// Catching exceptions
try {
    risky()
} catch RuntimeError err {
    print("Error: " + err.message)
} catch LLMError err {
    print("LLM Error: " + err.message)
} catch {
    print("Unknown error")
} finally {
    cleanup()
}
```

**Predefined exception types**:
- `RuntimeError` — Runtime error
- `LLMError` — LLM-related error (base class)
  - `TimeoutError` — LLM call timeout
  - `ModelError` — Model unavailable or quota exhausted
- `ToolError` — Tool call failed
- `AssertionError` — `assert` statement failed (Phase 10)
- `AnyError` — Any error (used internally for catch-all)

### `assert`
Runtime assertion. Throws `AssertionError` when the condition is false.

```helen
assert x > 0
assert x > 0, "x must be positive"

// Can be caught
try {
    assert false, "test"
} catch AssertionError e {
    print("Caught: " + e.message)
}
```

---

## Functions and Calls (3)

### `fn`
Function declaration.

```helen
fn add(a, b) {
    return a + b
}
```

### `call`
Call an Agent.

```helen
call Translator(text)
```

### `spawn` (v1.18)
Concurrently spawns an agent, returns a Channel (mailbox).

```helen
let mailbox = spawn Worker("task")
let result = mailbox.receive()
```

Chinese: `分生`

---

## Variables and Types (4)

### `let`
Mutable variable declaration.

```helen
let x = 42
x = 100  // ✅ Can be modified
```

### `const`
Immutable constant declaration.

```helen
const PI = 3.14
PI = 3   // ❌ E0346 CONST_ASSIGNMENT
```

### `shared` (v1.10)
Cross-agent visible variable declaration.

```helen
shared let counter = 0

agent Worker {
  main {
    counter += 1  // ✅ Can access and modify shared let
  }
}
```

**Scope rules** (v1.10):
- Module-level `let` is **not visible** inside agent main (compile-time error)
- Module-level `const` is auto-visible (read-only sharing)
- `shared let` explicitly declares cross-agent visible mutable variables
- Imported `shared let` is correctly tracked
- Functions of imported modules can access their own module's `const` and `shared let` (whether aliased or non-aliased import)

```helen
// Example: module-level variable scope
let moduleVar = "module-level"
const MODULE_CONST = "constant"
shared let sharedVar = "shared"

agent MyAgent {
  main {
    // moduleVar    // ❌ Compile error: module-level let not visible
    MODULE_CONST   // ✅ Read-only access
    sharedVar = "new value"  // ✅ Can read and write
  }
}
```

**Cross-module access** (v1.10):
```helen
// output.helen
const LEVEL = 1
shared let _use_colors = true
fn colorize(t: str): str {
    if _use_colors { return "[C]" + t }
    return t
}

// main.helen — Aliased import
import "output.helen" as output
main {
    output.LEVEL            // ✅ const accessed via alias
    output._use_colors      // ✅ shared let accessed via alias
    output.colorize("hi")   // ✅ Function can see module's const and shared let
}

// main2.helen — Non-aliased import
import "output.helen"
main {
    LEVEL              // ✅ const directly visible
    _use_colors        // ✅ shared let directly visible
    colorize("hi")     // ✅ Function can see module variables
}
```

### `store` (v1.12)

`shared store` declares controlled shared mutable state.

```helen
shared store Counter {
    count: int = 0
    fn increment() { count += 1 }
    fn get(): int { return count }
}
```

- Fields and methods form structured state
- Thread-safe at runtime (RLock)
- `_` prefixed fields are private — not directly accessible from agent code
- Chinese keyword: `仓库`

**v1.18 change**: `channel X { fields }` declaration syntax has been removed. Channels are now created via `Channel()` constructor or `spawn`, and are message queues (send/receive), no longer locked structs. Use `shared store` when you need a locked struct.

**Note**: `通道` can still be used as a Chinese alias for `SharedStore`, but there is no longer keyword-level `channel` declaration syntax.

### `true` / `false`
Boolean literals.

---

## Miscellaneous (5)

### `import`
Module import.

```helen
import "./utils.helen"
import "./config.json" as cfg
```

### `return`
Return a value from a function.

```helen
fn double(x) {
    return x * 2
}
```

### `as`
Import alias.

```helen
import "./lib" as utils
```

### `alias` (v1.10)
Function/variable alias. Creates additional names for existing functions or stdlib functions.

```helen
// Alias for stdlib functions
alias len as 长度
alias print as 打印

// Alias for user functions
函数 greet(name: str): str { 返回 "Hello, " + name }
alias greet as 打招呼
```

Chinese keyword `别名` is equivalent:

```helen
别名 len as 我的长度
主函 { 我的长度([1, 2, 3]) }
```

Helen's stdlib comes with 255 built-in Chinese aliases, automatically loaded at startup — no manual `alias` needed:

```helen
// Use Chinese stdlib function names directly
函数 测试() {
    设 数据 = [3, 1, 4, 1, 5]
    返回 长度(排序(数据))   // 长度 = len, 排序 = sort
}
```

All locale alias tables are loaded in full at startup, regardless of the locale setting. The locale configuration only affects the display language of docs/LSP/error messages, not the available names.

### `functions`
Function block inside an agent.

```helen
agent MyAgent {
    functions {
        fn helper() { ... }
    }
}
```

### `main`
Main program block.

```helen
main {
    print("Hello, Helen!")
}
```

### `null`
Null literal.

```helen
let x: str? = null
```

### `is` (v1.8)
Type pattern matching keyword, used in `match` statements to check a value's type.

```helen
match value {
    case is String { print("it's a string") }
    case is Int { print("it's an int") }
    case is String s { print("string: " + s) }  // Type check and bind
    case _ { print("unknown") }
}
```

Supported types: `String`, `Int`, `Float`, `Bool`, `List`, `Map`, `Null`

---

## Operators (v1.8)

### `|>` (Pipe Operator)
Passes the left-hand value as an argument to the right-hand function.

```helen
// Basic usage
let result = 5 |> double  // Equivalent to double(5)

// Chained calls
let result = "hello" |> upper |> strip  // Equivalent to strip(upper("hello"))

// With built-in functions
let len = [1, 2, 3] |> len  // 3

// With custom functions
fn add_one(x) { return x + 1 }
let result = 10 |> add_one  // 11
```

**Precedence**: Low precedence (level 2), left-associative
