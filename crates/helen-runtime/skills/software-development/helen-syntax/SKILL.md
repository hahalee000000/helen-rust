---
name: helen-syntax
description: "Helen language syntax quick reference — keywords, types, expressions, statements"
version: 1.30.12
author: Helen Team
license: MIT
tags: [helen, syntax, reference, language, chinese-punctuation, chinese-quotes]
---
<!-- helen-rust edition: the Rust port implements this syntax byte-faithfully (differential-verified, lex 36/36 + parse 47/47). Reserved-keyword and CJK rules identical. -->


# Helen Syntax Reference

## Keywords (99 total: 48 English + 51 Chinese)

Bilingual keywords map to the same TokenType and can be freely mixed. The parser/interpreter requires no changes.

### ⚠️ Reserved Keywords — Cannot Be Used as Identifiers

**All keywords are reserved and CANNOT be used as variable names, function names, or any other identifiers.**

```helen
// ✗ WRONG — 描述/模型/工具 are reserved keywords
设 描述 = "hello"     // Error: Expected variable name after 'let'/'const'
设 模型 = "qwen"      // Error: 模型 is a keyword
函数 工具() {}        // Error: 工具 is a keyword

// ✓ CORRECT — use non-keyword names
设 图片描述 = "hello"
设 模型名称 = "qwen"
函数 工具箱() {}
```

**Common reserved Chinese keywords** (frequently misused):
- Agent declaration: `描述`, `模型`, `工具`, `提示词`, `温度`, `最大轮次`, `流式输出`, `函数区`, `主函`
- Control flow: `如果`, `否则`, `对于`, `属于`, `当`, `中断`, `继续`
- Variable/function: `设`, `常量`, `函数`, `返回`
- Logical operators: `且` (AND), `或` (OR)

**Note**: stdlib function names (like `长度`, `打印`, `排序`) are NOT keywords and technically CAN be used as variable names. However, **shadowing builtins is strongly discouraged** — the interpreter may raise `ShadowBuiltinError`, and it causes confusion. See [Variable Naming Convention](#variable-naming-convention) below.

### Keyword Mapping Table

| 英文 | 中文 | 说明 |
|------|------|------|
| `let` | `设` / `定义` | 可变变量 |
| `const` | `常量` | 常量 |
| `shared` | `共享` | 跨 agent 可见 (v1.10) |
| `store` | `仓库` | 共享仓库声明 (v1.12) |
| `fn` | `函数` | 函数声明 |
| `return` | `返回` | 函数返回 |
| `if` / `else` | `如果` / `否则` | 条件分支 / match 分支 |
| `for` / `in` | `对于` / `属于` | 循环 |
| `while` | `当` | 条件循环 |
| `break` / `continue` | `中断` / `继续` | 循环控制 |
| `match` / `case` / `default` | `匹配` / `情况` / `默认` | 模式匹配 |
| `try` / `catch` / `finally` | `尝试` / `捕获` / `最终` | 异常处理 |
| `throw` | `抛出` | 抛出异常 |
| `assert` | `断言` | 运行时断言 |
| `true` / `false` | `真` / `假` | 布尔值 |
| `null` | `空` | 空值 |
| `is` | `是` | 类型判断 |
| `and` / `or` | `且` / `或` | 逻辑运算符 (v1.30.12) |
| `agent` | `智能体` | Agent 声明 |
| `llm` | `大模型` | LLM 操作关键字 |
| `act` | `执行` | 自主执行（支持 `on_chunk`/`on_complete`/`on_tool_end` 回调） |
| `spawn` | `分生` | 启动并发 Agent，返回 Channel (v1.18) |
| `prompt` | `提示词` | Agent 系统提示 |
| `description` | `描述` | Agent 描述 |
| `model` | `模型` | 指定模型 |
| `tools` | `工具` | 可用工具列表 |
| `streaming` | `流式输出` | 启用流式 |
| `temperature` | `温度` | 温度参数 |
| `max-turns` | `最大轮次` | 最大工具调用轮次 |
| `max-tokens` | `最大tokens` | LLM 响应最大输出 token 数 (v1.31.2) |
| `thinking-mode` | `思考模式` | 启用思考/推理模式 (v1.36) |
| `reasoning-effort` | `推理强度` | 推理强度: low/medium/high/max (v1.36) |
| `functions` | `函数区` | Agent 内函数定义区 |
| `main` | `主函` | 入口块 |
| `import` / `as` | `导入` / `作为` | 模块导入 |
| `protocol` / `impl` | `协议` / `实现` | 协议声明 |
| `branch` | `分支` | 分支 |
| `alias` | `别名` | 函数/变量别名 (v1.10) |
| `transcript` | `记录` | Agent transcript 控制 (v1.29: none/memory/persistent) |

### Chinese Identifiers

CJK Unified Ideographs (U+4E00–U+9FFF, etc.) can be used as identifier characters.

```helen
// 纯中文
函数 斐波那契(n: int): int {
    如果 n <= 1 { 返回 n }
    否则 { 返回 斐波那契(n - 1) + 斐波那契(n - 2) }
}
常量 LIMIT = 100
main {
    定义 结果 = 斐波那契(10)
    如果 结果 < LIMIT { 打印("OK") }
}

// v1.10: 共享变量
共享 定义 counter = 0
```

### Predefined Variables

| Variable | Type | Description |
|----------|------|-------------|
| `argv` | `const list<str>` | Command-line arguments. **`argv[0]` is the program name** (e.g., `"tool.helen"`), user arguments start at `argv[1]`. |

`argv` is `const`, automatically visible (read-only) in agent isolated scope, and cannot be reassigned.

```helen
// Command line: helen tool.helen --verbose --output=json input.txt
import std.core.*
import std.system.*
main {
    print(argv)          // ["tool.helen", "--verbose", "--output=json", "input.txt"]
    print(len(argv))     // 4 (includes program name)
    
    // Skip argv[0] to get user arguments
    let args = []
    let skip_first = true
    for arg in argv {
        if skip_first {
            skip_first = false
        } else {
            args.append(arg)
        }
    }
    print(args)          // ["--verbose", "--output=json", "input.txt"]
    
    let config = parse_cli_args(args)  // {verbose: true, output: "json", _positional: ["input.txt"]}
}
```

## Top-Level Restrictions (v1.30) 顶层限制

Only **declarations** and at most one `main {}` block are allowed at the module level. All executable code MUST be inside `main {}` or a function body.

**顶层限制（v1.30）**：模块层仅允许**声明**和至多一个 `main {}` 块，所有可执行代码必须位于 `main {}` 或函数体内。

| Allowed at top level 允许 | Forbidden at top level 禁止 |
|---|---|
| `fn`, `agent`, `const` | `let`, `if`, `for`, `while`, `match` |
| `import`, `alias`, `shared`, `protocol`, `impl` | `try`, `print()`, function calls |
| `main {}` (at most one 至多一个) | Any other executable statement 其他可执行语句 |

Violation raises **E0355 `TOP_LEVEL_STATEMENT`**.

```helen
// ✅ Correct — declarations at top level, executable code in main
import std.core.*
const LIMIT = 100
fn helper(): int { return 42 }
agent Worker { main { ... } }
main {
    let x = helper()
    if x > LIMIT { print("over") }
}

// ❌ Error — E0355 TOP_LEVEL_STATEMENT
const LIMIT = 100
let x = 42          // E0355: let not allowed at top level
print("hello")      // E0355: function call not allowed at top level
if true { ... }     // E0355: if not allowed at top level
```

## Data Types

| Type | Examples | Description |
|------|----------|-------------|
| `int` | `42`, `-7` | Integer |
| `float` | `3.14`, `-0.5` | Floating-point number |
| `str` | `"hello"`, `'world'` | String |
| `bool` | `true`, `false` | Boolean |
| `null` | `null` | Null value |
| `list` / `列表` | `[1, 2, 3]` | List |
| `map` / `映射` | `{"key": "value"}` | Map |
| `str?` | `null`, `"x"` | Optional type |
| `int \| str` | `42`, `"x"` | Union type |

```helen
main {
    let name: str? = null           // Optional string
    let value: int | str = "hello"  // Union type
}
```

## Expressions

### Arithmetic Operators
```helen
main {
    let sum = a + b      // Addition
    let diff = a - b     // Subtraction
    let prod = a * b     // Multiplication
    let quot = a / b     // Division
    let remainder = a % b // Modulo
    let power = a ** b   // Exponentiation
}
```

### Comparison and Logical Operators
```helen
main {
    let eq = a == b       // Equal to
    let ne = a != b       // Not equal to
    let lt = a < b        // Less than
    let le = a <= b       // Less than or equal to
    let and = a && b      // Logical AND (short-circuit)
    let or = a || b       // Logical OR (short-circuit)
    let not = !a          // Logical NOT
}
```

### Member Access
```helen
import std.core.*
main {
    let item = list[0]         // List index
    let value = map["key"]     // Map lookup
    let length = len(str)      // Function call
}
```

### Map Method Access (v1.44)

Map literals support Python-style method access — `.get()`, `.keys()`, `.values()`, `.items()`. These methods are only recognized when the property name is **not already a key** in the map (key lookup takes precedence):

```helen
import std.core.*
main {
    let m = {"a": 1, "b": 2, "get": "literal"}

    // Method access (property not a key)
    m.get("a")            // 1
    m.keys()              // ["a", "b", "get"]
    m.values()            // [1, 2, "literal"]
    m.items()             // [["a", 1], ["b", 2], ["get", "literal"]]

    // Key lookup wins: "get" is a real key
    m["get"]              // "literal"
    m.get                 // "literal" (treated as key, not method)
}
```

**Precedence rule**: If a Map contains a key named `get`/`keys`/`values`/`items`, that key shadows the method. Use `Dict.get(map, key)` (stdlib) for unambiguous access.

### Chinese Fullwidth Operators (v1.10)

Chinese fullwidth punctuation marks are equivalent alternatives to ASCII operators — no need to switch input methods:

| ASCII | Fullwidth | | ASCII | Fullwidth |
|-------|-----------|-|-------|-----------|
| `()` | `（）` | | `+` | `＋` |
| `{}` | `｛｝` | | `-` | `－` |
| `[]` | `［］` | | `*` | `＊` |
| `,` | `，` | | `/` | `／` |
| `.` | `．` | | `%` | `％` |
| `:` | `：` | | `!` | `！` |
| `;` | `；` | | `=` | `＝` |
| `?` | `？` | | `>` | `＞` |
| | | | `<` | `＜` |
| `!=` | `！＝` | | `\|` | `｜` |
| `==` | `＝＝` | | `\|>` | `｜＞` |
| `>=` | `＞＝` | | `->` | `－＞` |
| `<=` | `＜＝` | | `..` | `．．` |
| `&&` | `＆＆` | | `\|\|` | `｜｜` |

### Chinese Quotes (v1.10)

| Quote | Unicode | Example |
|-------|---------|---------|
| `""` | U+201C / U+201D | `"你好世界"` |
| `''` | U+2018 / U+2019 | `'你好世界'` |
| `「」` | U+300C / U+300D | `「你好世界」` |
| `『』` | U+300E / U+300F | `『你好世界』` |
| `＂` | U+FF02 | `＂你好世界＂` |

Escape sequences (`\n`, `\t`, `\\`, etc.) are supported; unclosed quotes raise an error. Multi-line strings still use ASCII `"""..."""`.

```helen
// 纯中文代码，全程中文输入法
常量 Y ＝ 20
函数 加（甲： int， 乙： int）： int ｛
    返回 甲 ＋ 乙
｝
main ｛
    设 x ＝ 10
    如果 x ＞ 0 ｛
        设 结果 ＝ 加（x， Y）
    ｝ 否则 ｛
        设 结果 ＝ 0
    ｝
    如果 a ＞＝ 0 ＆＆ a ＜＝ 100 ｛ 打印（"在范围内"） ｝
    设 result ＝ 5 ｜＞ double
｝
```

## Statements

### Variable Declarations
```helen
const PI = 3.14159            // Constant (top-level allowed)
main {
    let x = 42                    // Mutable variable
    let name: str = "Helen"       // Type annotation
}
```

### Variable Naming Convention — Avoid Shadowing Builtins

Helen has 364 stdlib functions (e.g. `len`, `find`, `format`, `map`, `list`, `count`, `str`, `print`, `sort`, `keys`, `values`, `contains`, `split`, `join`, `strip`, `replace`, `substring`) and 99 reserved keywords. **Never use these as variable names.** The interpreter may raise `ShadowBuiltinError`, and the code becomes confusing.

**Rule: use suffix-qualified names — short names must carry a role suffix.**

| ❌ Don't | ✅ Do | Why |
|----------|------|-----|
| `let map = {}` | `let config_map = {}` | shadows `map()` builtin |
| `let list = []` | `let item_list = []` | shadows `list` type/builtin |
| `let count = 0` | `let total_count = 0` | shadows `count()` builtin |
| `let str = "hi"` | `let raw_str = "hi"` | shadows `str()` builtin |
| `fn foo(input)` | `fn foo(user_input)` | `input` is a builtin (reads stdin) |
| `let entries = ...` | `let map_entries = ...` | `entries()` builtin (dict pairs) |
| `let result = find()` | `let find_result = find()` | `result` too generic |
| `let data = ...` | `let user_data = ...` | compound name |
| `let file = ...` | `let log_file = ...` | compound name |
| `let key = "x"` | `let api_key = "x"` | compound name |

**Common suffixes:** `_map`, `_list`, `_count`, `_text`, `_str`, `_result`, `_config`, `_data`, `_file`, `_path`, `_name`, `_id`, `_type`, `_value`, `_item`, `_key`, `_index`, `_flag`, `_status`, `_error`

**Compound names are safest:** `user_name`, `scan_result`, `log_file`, `api_key`, `total_count`, `error_message`, `config_map`, `item_list`

**Chinese identifiers naturally avoid conflicts:** `描述文本`, `配置字典`, `结果列表`, `扫描数量` — these rarely collide with English builtin names.

```helen
// ❌ WRONG — shadows builtins
import std.core.*
fn process(items: list): map {
    let map = {}           // shadows map()
    let list = []          // shadows list
    let count = len(items) // shadows count()
    let str = "done"       // shadows str()
    return map
}

// ✅ CORRECT — suffix-qualified
fn process(items: list): map {
    let result_map = {}
    let output_list = []
    let item_count = len(items)
    let status_text = "done"
    return result_map
}

// ✅ CORRECT — Chinese names avoid conflicts naturally
fn 处理(项目列表: list): map {
    设 结果字典 = {}
    设 输出列表 = []
    设 项目数量 = 长度(项目列表)
    设 状态文本 = "完成"
    返回 结果字典
}
```

### Function Declarations
```helen
import std.core.*
fn add(a: int, b: int): int {     // Return type uses : syntax (v1.10, -> removed)
    return a + b
}
fn greet(name: str) {
    print("Hello, " + name)
}
```

### Function Aliases (v1.10)
```helen
alias len as 我的长度          // stdlib alias
alias print as 输出
fn greet(name: str): str { return "Hello, " + name }
alias greet as 打招呼          // User function alias
别名 sort as 排序              // Chinese keyword equivalent
main {
    输出(我的长度([1, 2, 3]))  // Using aliases
    打招呼("Alice")
}
```

The stdlib includes 230+ built-in Chinese aliases, all loaded at startup (unaffected by `locale` configuration):
```helen
fn 数据处理() {
    let 数据 = [3, 1, 4, 1, 5, 9]
    return 长度(排序(去重(数据)))   // 长度=len, 排序=sort, 去重=unique
}
```

### Agent Declarations
```helen
import std.core.*
agent Translator {
    description "Translate text between languages"
    prompt "You are a professional translator."
    model "gpt-4"
    temperature 0.3
    tools = ["web_search"]
    main {
        return llm act "Translate: Hello"
    }
}

// tools references module-level const (statically auditable, clear security boundary)
const FILE_TOOLS = ["read_file", "write_file", "path_exists"]
agent Builder {
    tools = FILE_TOOLS           // Module-level const reference
    main { ... }
}
// Prohibited: mutable variables, fn, agent, undefined identifiers, expression concatenation

// functions block supports variable definitions, accessible by internal fn
agent MyAgent {
    functions {
        let config = "default"
        const MAX_RETRIES = 3
        fn get_config(): str { return config }
    }
    main { print(get_config()) }
}

// Streaming agent (returns StreamingResponse)
agent Streamer(topic: str) {
    description "Stream a long response"
    streaming true
    prompt "Write a detailed essay about: {{topic}}"
}

// Transcript control (v1.29)
agent SimpleTask {
    description "Simple task with no transcript"
    transcript "none"  // Default: no transcript recording
    main { ... }
}

agent DebugAgent {
    description "Debug with memory transcript"
    transcript "memory"  // In-memory only, no disk persistence
    main { ... }
}

agent AuditAgent {
    description "Audit with persistent transcript"
    transcript "persistent"  // Full persistence to disk
    main { ... }
}

// Chinese aliases
agent 工作Agent {
    描述 "简单任务"
    记录 "无"  // 或 "内存", "持久"
    主函 { ... }
}

// Thinking mode (v1.36) - enables chain-of-thought reasoning
agent DeepThinker {
    description "Deep reasoning agent"
    model "deepseek-v4-pro"
    thinking-mode true          // Enable thinking/reasoning
    reasoning-effort "high"     // low/medium/high/max
    main { return llm act "Solve this complex problem" }
}

// Provider override (v1.36, context keyword - not reserved)
agent CustomProvider {
    model "deepseek-v4-flash"
    provider "deepseek"         // Override auto-detected protocol
    main { return llm act "Hello" }
}
```

Agents are first-class citizens — called like functions:
```helen
main {
    let result = Translator("Hello")
    MyAgent("test")                // Statement position
    let x = some_fn(Translator("test"))
}
```

### Agent Scope Isolation

Agent `main {}` runs in an isolated environment and **cannot** directly access module-level `let` (compile-time `SCOPE_VIOLATION` error).

**Scope rules**:
- Module-level `let` — **not visible** in agent main
- Module-level `const` — automatically visible (read-only)
- `shared let` — explicitly visible across agents (v1.12: value types only: int/float/str/bool)
- `shared store` — structured shared state (v1.12, supports reference types)

**Cross-Agent data sharing** (in recommended order):

```helen
// 1. Closure callbacks (best — buffer fully internalized)
import std.core.*
agent Streamer {
    main {
        let buf = ""
        let cb = fn(chunk) { buf = buf + chunk }
        llm act "..." on_chunk cb
    }
}

// 2. shared let (explicit cross-agent, v1.12: value types only)
shared let counter = 0
agent Worker {
    main { counter += 1; let x = counter }
}

// 3. const (read-only shared configuration)
const LIMIT = 100
agent Worker {
    main { let x = LIMIT }  // Automatically shared read-only
}

// 4. Reference types passed via parameters (v1.12: parameters auto-wrapped in read-only view)
agent Worker(items: list) {
    main {
        let copy = list(items)  // Create a copy before modifying
        copy.append(4)
    }
}
```

Module-level `fn` can access module-level `let` normally, and agent main can call module-level `fn` — the isolation boundary only applies to direct variable access.

### Agent Isolation Levels (v1.12)

`@` decorators control agent isolation level:

```helen
agent Normal() { main { ... } }              // L1: Standard isolation (default)
@open agent Debug() { main { ... } }         // L0: Open — can access module-level let
@strict agent Safe(data: list) { main { ... } }  // L2: Strict — deep copy parameters/return values
@sandbox agent Untrusted(input: str) {       // L3: Sandbox — deep copy + restricted tools
    tools []
    main { return process(input) }
}
// Chinese: @开放、@严格、@沙箱
```

| Level | Decorator | Parameters/Returns | Module let | Tools |
|-------|-----------|-------------------|------------|-------|
| L0 | `@open` | Shared reference | ✅ Visible | Unrestricted |
| L1 | Default | Read-only view | ❌ Not visible | Unrestricted |
| L2 | `@strict` | Deep copy | ❌ Not visible | Unrestricted |
| L3 | `@sandbox` | Deep copy | ❌ Not visible | Restricted to empty |

### Shared Store (v1.12)

Structured shared mutable state (fields can be value or reference types, thread-safe):

```helen
import std.dict.*
shared store Counter {
    count: int = 0
    fn increment() { count += 1 }
    fn get(): int { return count }
    fn reset() { count = 0 }
}
// Chinese
共享 store 计数器 { 数量: int = 0; fn 增加() { 数量 += 1 } }

// Usage
agent Worker {
    main { Counter.increment(); let val = Counter.get() }
}
```

Store rules: fields can be value/reference types, methods can modify fields, all agents can access, thread-safe (internal RLock).
Fields with `_` prefix are private (inaccessible from agent code).

**shared let vs shared store**:

| | shared let | shared store |
|--|-----------|--------------|
| Type | Value types only | Value + reference types |
| Structure | Single variable | Fields + methods |
| Use case | Counters, flags | Queues, caches, state machines |

### Channel Message Queue (v1.18)

`spawn Agent(...)` launches a concurrent agent and returns a Channel (mailbox) for message passing:

```helen
import std.concurrency.*
main {
    let ch = spawn Worker("task")

    // Channel methods
    ch.send("message")            // Send message
    let msg = ch.receive()        // Receive (blocking)
    let ok = ch.try_receive()     // Try receive (non-blocking, returns null or message)
    ch.cancel()                   // Cancel (can interrupt streaming)
    ch.close()                    // Close

    // Multi-channel select (first-ready wins)
    let ready = mailbox_select([ch1, ch2, ch3])
    // Chinese: 发送()、接收()、尝试接收()、取消()、关闭()
}
```

Spawned agents run in an isolated environment with a deep-copied snapshot of all variables. Inter-agent data sharing is done explicitly by passing SharedStore references through Channel messages.

> **Note**: The `channel Name { ... }` declaration syntax (equivalent to shared store) was removed in v1.18. In v1.18, Channel specifically refers to the message channel returned by spawn.

#### Resuming a Spawned Session (v1.27)

By default `spawn` starts the child agent in a fresh transcript. The optional `resume("<session_id>")` clause (Chinese alias `恢复会话(...)`) makes the spawned agent **continue a previously saved child-session transcript** instead:

```helen
main {
    // Fresh spawn (default)
    let mb = spawn Worker("task")

    // Resume a saved child session - history is loaded, LLM remembers the prior run
    let mb = spawn Worker("task") resume(saved_child_sid)
    // Chinese: 设 mb = 分生 工作者("task") 恢复会话(已存子会话id)
}
```

- `resume` is an identifier clause (not a keyword token) - the bilingual keyword count stays at 89.
- The argument must evaluate to a `str` session_id. A non-existent id creates a fresh session with that id (graceful fallback, no crash).
- This is **true resumption** (the spawned interpreter continues appending to that session's transcript), distinct from `resume_session(sid)` which imports another session's messages into a new one.
- Resuming restores LLM conversation memory, **not** runtime variable state - persist critical state separately if you need true stateful continuation.
- A cross-process lock guards the resumed session against concurrent writers; stale locks and same-process reuse are auto-reclaimed.

### LLM Statements
```helen
// llm act — autonomous execution (usable as expression since v1.10)
// llm if — routing classification
import std.core.*
fn handle_chunk(chunk) { print(chunk, end="") }
fn done() { print("\n✅ Done") }
fn after_tool(name, result) {
    if name == "read_file" { return "File read, please analyze the content" }
    return null  // No injection
}
main {
    let result = llm act "What is 2+2?"

    llm if input {
        case "positive" { print("Good!") }
        case "negative" { print("Bad!") }
        default { print("Neutral") }
    }

    // llm act with streaming callbacks
    llm act "Write a story" on_chunk handle_chunk on_complete done

    // v1.21: on_tool_end — inject hint after tool execution to guide LLM
    llm act "Analyze the code" on_tool_end after_tool
}
```

### LLM Multimodal (v1.17)

Callbacks as adapters — protocol differences are handled by user callbacks, Helen core does not hardcode provider formats:

```helen
// media() — ordinary stdlib function, returns MediaPart object
import std.media.*
main {
    let img = media("photo.jpg")
    llm act "Describe this image" media(img)

    // on_media — multimodal input adapter (MediaPart → provider format)
    llm act "Analyze" media(img) on_media fn(parts, provider) {
        return [{"type": "image_url", "image_url": {"url": parts[0].source}}]
    }

    // on_generate — register generation capability as a tool (text-to-image/video, etc.)
    llm act "Create" on_generate fn(params) {
        // params: {prompt, size, model, ...}
        return generate_image(params.prompt)
    }

    // provider — specify provider adapter
    llm act "..." provider("claude")
}
```

`MediaPart` is a first-class data type (fields: `source`/`content`/`mime`/`media_type`/`metadata`), assignable, passable as argument, and storable in lists.
When `on_media` is not specified, the default OpenAI-compatible adapter is used. Chinese aliases: `媒体()`, `处理媒体 fn(...)`, `生成 fn(...)`.

### Exception Handling
```helen
import std.core.*
main {
    try {
        risky_operation()
    } catch RuntimeError e {
        print("Runtime error: " + e.message)
    } catch TimeoutError e {
        print("Timeout: " + e.message)
    } finally {
        cleanup()
    }

    // Agent call failure → AgentError (v1.10, carries agent_name/agent_args/cause)
    try {
        let result = Contractor(req, dir)
    } catch AgentError e {
        print("Agent failed: " + e.agent_name + " — " + e.message)
    }
    // AgentError inherits from LLMError; catching LLMError also catches it

    // Stdlib Python exceptions are automatically wrapped as RuntimeError
    try {
        let x = len(42)
    } catch RuntimeError e {
        print(e.message)  // "Python TypeError: object of type 'int' has no len()"
    }
}
```

### Assertions
```helen
import std.core.*
main {
    assert x > 0
    assert x > 0, "x must be positive"
    try { assert false, "test" } catch AssertionError e { print("Caught: " + e.message) }
}
```

### Pattern Matching
```helen
import std.core.*
main {
    // Basic matching
    match status {
        case 200 { print("OK") }
        case 404 { print("Not Found") }
        case 500 { print("Server Error") }
        default { print("Unknown") }
    }

    // Range matching (.. is inclusive)
    match score {
        case 90..100 { print("A") }
        case 80..89 { print("B") }
        case 70..79 { print("C") }
        default { print("F") }
    }

    // Guard conditions
    match x {
        case 1..100 if x == 42 { print("the answer") }
        case 1..100 { print("in range") }
        default { print("out of range") }
    }

    // Wildcards, variable binding, type patterns
    match value {
        case 1 { print("one") }
        case n if n > 0 { print("positive: " + str(n)) }
        case is String s { print("string: " + s) }
        case _ { print("other") }
    }
}
```

### Pipe Operator
```helen
main {
    let result = 5 |> double                          // Equivalent to double(5)
    let result = "hello" |> upper |> strip            // Chained: strip(upper("hello"))
    let len = [1, 2, 3] |> len                        // 3
    let result = 10 |> add_one                        // Custom function
}
```

### Closures and Anonymous Functions
```helen
import std.core.*
import std.list.*
fn make_counter() {                     // Closure (lexical scope, value capture)
    let count = 0
    return fn() { count = count + 1; return count }
}
main {
    let add = fn(x, y) { return x + y }    // Anonymous function
    print(add(1, 2))                        // 3

    let counter = make_counter()
    print(counter())  // 1
    print(counter())  // 2

    // Closures as first-class callable objects (v1.32+)
    let nums = [1, 2, 3]
    let doubled = map(nums, fn(x) { return x * 2 })  // [2, 4, 6]
    
    // Anonymous closures in hooks
    llm act "test" on_chunk fn(c) { print(c) }
}
```

> Closures capture a deep copy of reference-type variables (snapshot semantics, immune to subsequent modifications). Closures are first-class callable objects that can be passed as callbacks to hooks and higher-order functions. They use weak references to avoid circular references.

### Protocols
```helen
import std.core.*
protocol Printable {
    fn to_string(self): String
}
struct Point { x: Int; y: Int }
impl Printable for Point {
    fn to_string(self): String {
        return "Point(" + str(self.x) + ", " + str(self.y) + ")"
    }
}
```

### Imports
```helen
import "utils.helen"              // Import Helen module
import "config.json" as config    // Import JSON
import "data.yaml" as data        // Import YAML
import "./python_module" as py    // Import Python module
```

Multi-format imports (`.helen`/`.json`/`.yaml`/`.md`/`.txt`/Python), with circular dependency detection.

### Subscript/Field Assignment (v1.10)
```helen
main {
    let arr = [1, 2, 3]
    arr[0] = 10                       // [10, 2, 3]
    let obj = {"name": "Alice"}
    obj.name = "Bob"                  // {"name": "Bob"}
    obj["age"] = 30                   // {"name": "Bob", "age": 30}
    let matrix = [[1, 2], [3, 4]]
    matrix[0][1] = 99                 // Nested assignment
}

// const cannot be assigned
const c = [1, 2, 3]
// c[0] = 10                         # E0352 IMMUTABLE_ASSIGNMENT
```

### Short-Circuit Evaluation (v1.10)
```helen
main {
    let x = false && expensive_call()  // expensive_call() not executed
    let y = true || expensive_call()   // expensive_call() not executed
    let name = user != null && user.getName()    // Safe access
    let config = loadConfig() || defaultConfig() // Default value
}
```

Precedence: `||` has precedence 3, `&&` has precedence 4 (higher than `||`).

## Comments
```helen
// Single-line comment (use // for single-line)

import std.core.*
main {
    """
    Multi-line string (can serve as a documentation block)
    can span multiple lines
    """
    print("comments and strings work")
}
```

## String Interpolation
```helen
main {
    let name = "World"
    let greeting = "Hello, {{name}}!"  // Template variable substitution
}
```

## Error Codes (v1.10+)

| Code | Name | Trigger Condition |
|------|------|-------------------|
| E0350 | `SCOPE_VIOLATION` | Module-level let not visible in agent main |
| E0351 | `SHARED_NOT_MODULE_LEVEL` | shared let not declared at module level |
| E0352 | `IMMUTABLE_ASSIGNMENT` | Subscript/field assignment target is immutable |
| E0355 | `TOP_LEVEL_STATEMENT` | Executable statement (let/if/for/while/try/print/call) at module level outside main {} |

---

**Version**: v1.30
**Last updated**: 2026-07-29

## Related Skills

- **helen-stdlib** — Standard library function reference
- **helen-agent-patterns** — Agent design patterns
- **helen-quality** — Code quality assessment
