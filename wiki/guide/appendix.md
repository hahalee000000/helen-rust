# Appendix: Keywords & Quick Reference

## Keyword Reference Table

Helen has 99 keywords (48 English + 51 Chinese), in one-to-one correspondence. They can be mixed freely.

### Variables & Functions

| English | Chinese | Description |
|---------|---------|-------------|
| `let` | `设` / `定义` | Mutable variable |
| `const` | `常量` | Constant |
| `shared` | `共享` | Cross-agent shared variable |
| `store` | `仓库` | Shared store |
| `fn` | `函数` | Function declaration |
| `return` | `返回` | Function return |
| `alias` | `别名` | Function/variable alias |
| `import` | `导入` | Module import |
| `as` | `作为` | Import alias |

### Control Flow

| English | Chinese | Description |
|---------|---------|-------------|
| `if` | `如果` | Conditional branch |
| `else` | `否则` | Else branch |
| `for` | `对于` | Loop |
| `in` | `属于` | Iteration |
| `while` | `当` | Conditional loop |
| `break` | `中断` | Exit loop |
| `continue` | `继续` | Skip current iteration |
| `match` | `匹配` | Pattern matching |
| `case` | `情况` | Match branch |
| `default` | `默认` | Default branch |

### Exception Handling

| English | Chinese | Description |
|---------|---------|-------------|
| `try` | `尝试` | Try block |
| `catch` | `捕获` | Catch exception |
| `finally` | `最终` | Finalizer block |
| `throw` | `抛出` | Raise exception |
| `assert` | `断言` | Runtime assertion |

### Logical Operators

| English | Chinese | Description |
|---------|---------|-------------|
| `and` | `且` | Logical AND (keyword form) |
| `or` | `或` | Logical OR (keyword form) |
| `true` | `真` | Boolean true |
| `false` | `假` | Boolean false |
| `null` | `空` | Null value |
| `is` | `是` | Type check |

### Agent-Related

| English | Chinese | Description |
|---------|---------|-------------|
| `agent` | `智能体` | Agent declaration |
| `llm` | `大模型` | LLM operation |
| `act` | `执行` | Autonomous execution |
| `spawn` | `分生` | Launch concurrent agent |
| `prompt` | `提示` / `提示词` | System prompt |
| `description` | `描述` | Agent description |
| `model` | `模型` | Specify model |
| `tools` | `工具` | Tool list |
| `streaming` | `流式输出` | Enable streaming |
| `temperature` | `温度` | Temperature parameter |
| `max-turns` | `最大轮次` | Max tool-calling turns |
| `max-tokens` | `最大tokens` | Max output tokens |
| `thinking-mode` | `思考模式` | Enable reasoning mode |
| `reasoning-effort` | `推理强度` | Reasoning effort level |
| `functions` | `函数区` | Agent function definition block |
| `main` | `主函` | Entry block |
| `transcript` | `记录` | Conversation transcript control |

### Other

| English | Chinese | Description |
|---------|---------|-------------|
| `protocol` | `协议` | Protocol declaration |
| `impl` | `实现` | Protocol implementation |
| `branch` | `分支` | Branch |

## Data Types Quick Reference

| Type | English | Chinese | Examples |
|------|---------|---------|----------|
| Integer | `int` | `整数` | `42`, `-7` |
| Float | `float` | `浮点` | `3.14` |
| String | `str` | `字符串` | `"hello"` |
| Boolean | `bool` | `布尔` | `true`, `false` |
| Null | `null` | `空` | `null` |
| List | `list` | `列表` | `[1, 2, 3]` |
| Map | `map` | `映射` | `{"key": "value"}` |
| Optional | `str?` | - | `null` or `"x"` |
| Union | `int \| str` | - | `42` or `"x"` |

## Operators Quick Reference

### Arithmetic

| Operator | Description | Example |
|----------|-------------|---------|
| `+` | Addition | `1 + 2` → `3` |
| `-` | Subtraction | `5 - 3` → `2` |
| `*` | Multiplication | `2 * 3` → `6` |
| `/` | Division | `10 / 3` → `3.333...` |
| `%` | Modulo | `10 % 3` → `1` |
| `**` | Exponentiation | `2 ** 3` → `8` |

### Comparison

| Operator | Description |
|----------|-------------|
| `==` | Equal |
| `!=` | Not equal |
| `<` | Less than |
| `<=` | Less than or equal |
| `>` | Greater than |
| `>=` | Greater than or equal |

### Logical

| Operator | Description | Note |
|----------|-------------|------|
| `&&` | Logical AND (short-circuit) | Not `and` |
| `\|\|` | Logical OR (short-circuit) | Not `or` |
| `!` | Logical NOT | Not `not` |

### Other

| Operator | Description |
|----------|-------------|
| `..` | Inclusive range |
| `\|>` | Pipe operator |
| `{{}}` | Template variable / string interpolation |

## Fullwidth Symbol Reference

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
| `<` | `＜` | | `\|` | `｜` |

## Agent Configuration Quick Reference

```helen
agent FullConfig {
    description "Description"        // description
    prompt "System prompt"           // prompt
    model "model-name"               // model
    temperature 0.7                  // temperature (0.0-1.0)
    max-turns 10                     // max tool-calling turns
    max-tokens 4096                  // max output tokens
    streaming true                   // streaming output
    thinking-mode true               // thinking mode
    reasoning-effort "high"          // reasoning effort
    tools = ["tool1", "tool2"]       // tool list
    transcript "none"                // transcript level
}
```

## Standard Library Modules Quick Reference

| Module | # Functions | Common Functions |
|--------|-------------|------------------|
| `std.core` | 17 | `print`, `len`, `str`, `int`, `range`, `type` |
| `std.str` | 43 | `upper`, `lower`, `split`, `join`, `replace`, `regex_match` |
| `std.list` | 11 | `sort`, `map`, `filter`, `reduce`, `unique`, `flatten` |
| `std.dict` | 10 | `keys`, `values`, `get`, `set_key`, `has_key`, `merge` |
| `std.math` | 27 | `round`, `sqrt`, `pow`, `sum`, `mean`, `random` |
| `std.time` | 16 | `now`, `date`, `date_format`, `sleep` |
| `std.file` | 12 | `read_file`, `write_file`, `list_dir`, `glob_files` |
| `std.data` | 28 | `json_parse`, `json_stringify`, `yaml_parse`, `csv_parse` |
| `std.network` | 9 | `http_get`, `http_post`, `url_parse` |
| `std.system` | 24 | `env_get`, `shell_exec`, `platform`, `get_cli_args` |
| `std.crypto` | 17 | `md5`, `sha256`, `random`, `uuid_generate` |
| `std.media` | 12 | `media`, `is_image`, `save_media` |
| `std.test` | 23 | `assert_equal`, `assert_true`, `run_tests`, `expect` |
| `std.debug` | 11 | `debug`, `trace_on`, `coverage_report` |
| `std.path` | 6 | `path_join`, `path_exists`, `path_basename` |

## Common Error Codes

| Code | Name | Description | Fix |
|------|------|-------------|-----|
| E0350 | SCOPE_VIOLATION | Module-level `let` not visible inside agent | Use `const` or `shared let`, or pass as parameter |
| E0351 | SHARED_NOT_MODULE_LEVEL | `shared let` not at module top level | Move `shared let` to file top level |
| E0352 | IMMUTABLE_ASSIGNMENT | Attempt to modify a constant | Use `let` instead, or don't mutate it |
| E0355 | TOP_LEVEL_STATEMENT | Executable statement at top level | Move code into `main {}` or inside a function |

## Common Pitfalls

### 1. Logical Operators

```helen
// ✅ Correct
if a && b { }
if a || b { }
if !a { }

// ❌ Wrong — and/or/not are not operators
if a and b { }
if a or b { }
if not a { }
```

### 2. String Slicing

```helen
// ✅ Correct: use the substring function
let sub = substring(str, 0, 10)

// ❌ Wrong: slice syntax is not supported
let sub = str[0:10]
```

### 3. Agent Invocation

```helen
// ✅ Correct: call an agent like a function
let result = MyAgent(arg)

// ❌ Wrong: no 'call' keyword
let result = call MyAgent(arg)
```

### 4. Function Return Values

```helen
// ✅ Correct: must have explicit return
fn process(): map {
    return {"status": "ok"}
}

// ❌ Wrong: implicit return is not supported
fn process(): map {
    {"status": "ok"}
}
```

### 5. Keywords Cannot Be Variable Names

```helen
// ❌ Wrong: description, model, tools are reserved keywords
let description = "hello"    // Error!
let model = "qwen"           // Error!

// ✅ Correct: use other names
let image_description = "hello"
let model_name = "qwen"
```

### 6. Module Top-Level Restrictions

```helen
// ❌ Wrong: no executable statements at top level
let x = 42           // E0355
print("hello")       // E0355

// ✅ Correct: put executable code in main
main {
    let x = 42
    print("hello")
}
```

## Environment Setup

### Configuring the LLM API

Configure in `~/.helen/config.yaml`:

```yaml
# Example configuration
api_key: "your-api-key"
base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1"
model: "qwen-plus"
```

### Running Helen Programs

```bash
# Run a program
helen my_program.helen

# Syntax check
helen check my_program.helen

# Run tests
helen test my_test.helen

# Start the REPL
helen repl

# List templates
helen template --list
```

## Suggested Learning Path

```
Beginner
├── Chapter 1: Create Your First Agent
├── Chapter 2: Understand Prompts and Template Variables
├── Chapter 3: Learn llm act and llm if
└── Chapter 4: Give Your Agent Tools

Intermediate
├── Chapter 5: Master Variables and Types
├── Chapter 6: Control Flow
├── Chapter 7: Functions and Closures
└── Chapter 9: Get Familiar with the Standard Library

Practical
├── Chapter 8: Multi-Agent Collaboration
├── Chapter 10: Writing Tests
└── Chapter 11: Advanced Topics like Scope Isolation
```

---

> 🎉 Congratulations! You have finished the Helen language tutorial. Now go create your first AI agent!
>
> Questions? File an issue at https://github.com/hahalee000000/helen/issues
