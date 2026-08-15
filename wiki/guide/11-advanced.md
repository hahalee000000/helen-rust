# Chapter 11: Advanced Topics

## Scope Isolation

### Why Isolation?

When an agent executes, it creates a completely new environment. This environment **cannot see** the caller's local variables:

```helen
import std.core.*

let module_var = "hello"     // ❌ Not visible in agent (模块变量)
const read_only_const = 42   // ✅ Automatically visible in agent (只读常量)
shared let shared_count = 0  // ✅ Visible and modifiable in agent (共享计数)

agent MyAgent {              // (我的智能体)
    main {
        // print(module_var)     // ❌ Compile error! Not visible
        print(read_only_const)  // ✅ 42
        shared_count = shared_count + 1  // ✅ Can modify
        return llm act "task"
    }
}

main {
    MyAgent()
    print(shared_count)  // 1
}
```

### Variable Visibility Rules

| Variable Type | Visible in Agent? | Description |
|---------------|-------------------|-------------|
| Module-level `let` | ❌ Not visible | Compile error |
| Module-level `const` | ✅ Auto-visible | Read-only |
| `shared let` | ✅ Visible & modifiable | Value types only |
| `shared store` | ✅ Visible & modifiable | Supports complex types |
| Function parameters | ✅ Visible | Passed in |
| Local variables | ✅ Visible | Defined inside functions |

### Isolation Levels

Helen provides three isolation levels:

```helen
// L0: Open (for debugging)
@open agent DebugAgent() {
    main { return module_var }  // Can see module-level let
}

// L1: Standard (default)
agent StandardAgent() {
    main { ... }  // Cannot see module-level let
}

// L2: Strict (deep-copies parameters and return values)
@strict agent StrictAgent(data: list) {
    main { data.append(4); return data }  // Won't affect caller's original list
}

// L3: Sandbox (forces empty tool list)
@sandbox agent SandboxAgent() {
    tools = ["read_file"]  // Will be ignored, actual tools are empty
    main { ... }
}
```

| Level | Decorator | Characteristics |
|-------|-----------|-----------------|
| L0 | `@open` | Module-level let visible (debugging only) |
| L1 | (default) | Standard isolation |
| L2 | `@strict` | Deep-copies parameters and return values |
| L3 | `@sandbox` | Forces empty tools, no tools available |

### Parameter Read-Only Protection

If agent parameters are reference types (lists, maps), they are automatically wrapped as read-only views:

```helen
agent Processor(data: list) {
    main {
        let first_item = data[0]    // ✅ Can read
        // data.append(4)           // ❌ Cannot modify original list
        let copy = list(data)       // ✅ Create mutable copy
        copy.append(4)              // ✅ Modify copy
        return copy
    }
}
```

## Multimodal

Helen supports sending images, videos, and audio to the LLM:

```helen
import std.core.*
import std.media.*

agent ImageAnalyst(image_path: str) {  // (图片分析员, 图片路径)
    description "Image analysis agent"
    prompt "You are an image analysis expert. Carefully observe the image and describe what you see."

    main {
        let img = media(image_path)        // Load media file (图片)
        return llm act "Please describe this image" media(img)
    }
}

main {
    let desc = ImageAnalyst("photo.jpg")
    print(desc)
}
```

### Multimodal Callbacks (Advanced)

Different LLM providers have different multimodal formats. Helen uses callbacks to adapt:

```helen
import std.media.*

agent CustomMultimodal {
    main {
        let img = media("photo.jpg")
        return llm act "Analyze image" media(img) on_media fn(parts, provider) {
            // Custom media format conversion (自定义媒体格式转换)
            return [{"type": "image_url", "image_url": {"url": parts[0].source}}]
        }
    }
}
```

When `on_media` is not specified, Helen uses the default OpenAI-compatible format.

## Protocols

Protocols define interface specifications, and implementations (Impl) provide concrete implementations:

```helen
import std.core.*

// Define protocol: specifies which methods exist (定义协议：规定有哪些方法)
protocol Printable {
    fn to_string(self): str
}

// Define data structure (定义数据结构)
struct Point {
    x: int
    y: int
}

// Implement "Printable" protocol for "Point" (为"点"实现"可打印"协议)
impl Printable for Point {
    fn to_string(self): str {
        return "Point(" + str(self.x) + ", " + str(self.y) + ")"
    }
}

main {
    let p = Point{x: 3, y: 4}
    print(p.to_string())  // Point(3, 4)
}
```

Protocols let you define "what types should have what methods" without worrying about specific implementations.

## Module Import Details

### Importing Helen Modules

```helen
// Import another Helen file (导入另一个 Helen 文件)
import "utils.helen"

// Import with alias (导入并起别名)
import "utils.helen" as Utils
```

### Importing Different File Formats

```helen
import "config.json" as Config    // Import JSON (导入 JSON)
import "data.yaml" as Data        // Import YAML (导入 YAML)
import "settings.toml" as Settings  // Import TOML (导入 TOML)
import "notes.md" as Notes        // Import Markdown (导入 Markdown)
import "text.txt" as Text         // Import plain text (导入纯文本)
```

### Importing Python Modules

```helen
import "./python_module" as py  // Import Python module (导入 Python 模块)
```

### Standard Library Module Imports

```helen
// Import all (全部导入)
import std.core.*

// Selective import (选择性导入)
import std.str.{upper, lower, split}

// Namespace import (命名空间导入)
import std.dict as Dict

main {
    let s = upper("hello")          // Selectively imported function
    let d = {"a": 1}
    let v = Dict.get(d, "a")        // Namespace access
}
```

## Transcript Control

Control whether agent conversations are saved to disk:

```helen
// Don't save (default, zero overhead) (不保存，零开销)
agent QuickTask {
    transcript "none"
    main { llm act "quick task" }
}

// In-memory only (for debugging) (仅内存保存，调试用)
agent DebugAgent {
    transcript "memory"
    main { llm act "debug" }
}

// Persistent save (for auditing, session recovery) (持久化保存，审计、会话恢复)
agent AuditAgent {
    transcript "persistent"
    main { llm act "audit" }
}
```

| Level | English | Chinese | Use Case |
|-------|---------|---------|----------|
| Don't save | `none` | `无` | Most scripts, batch processing |
| In-memory | `memory` | `内存` | Development debugging |
| Persistent | `persistent` | `持久` | Auditing, long-running services |

## Command-Line Arguments

```helen
import std.core.*
import std.system.*

main {
    // argv[0] is program name, user arguments start from argv[1]
    print(argv)          // ["tool.helen", "--verbose", "input.txt"]

    // Skip first (program name)
    let user_args = []
    let skip_first = true
    for arg in argv {
        if skip_first {
            skip_first = false
        } else {
            user_args.append(arg)
        }
    }

    // Parse command-line arguments (解析命令行参数)
    let config = parse_cli_args(user_args)
    print(config)  // {verbose: true, _positional: ["input.txt"]}
}
```

## Error Codes

Helen compile-time error codes:

| Error Code | Name | Description |
|------------|------|-------------|
| E0350 | SCOPE_VIOLATION | Module-level let not visible in agent |
| E0351 | SHARED_NOT_MODULE_LEVEL | shared let must be declared at module top level |
| E0352 | IMMUTABLE_ASSIGNMENT | Attempting to modify a constant |
| E0355 | TOP_LEVEL_STATEMENT | Executable statements not allowed at top level |

### Top-Level Restrictions

Only declarations and `main` blocks are allowed at module top level; executable statements are not allowed:

```helen
// ✅ Correct: declarations at top level, executable code in main
const LIMIT = 100
fn helper(): int { return 42 }
agent Worker { main { ... } }
main {
    let x = helper()
    if x > LIMIT { print("over") }
}

// ❌ Error: top level cannot have executable statements
let x = 42          // E0355
print("hello")      // E0355
if true { ... }     // E0355
```

## MCP Tool Integration

MCP (Model Context Protocol) allows Helen to access external tool ecosystems:

### Configuration

Create `.mcp.json` in the project root:

```json
{
  "mcpServers": {
    "database": {
      "command": "npx",
      "args": ["-y", "@example/db-mcp"],
      "tool_timeout_sec": 60
    }
  }
}
```

### Usage

MCP tools are automatically discovered and can be used like built-in tools:

```helen
agent DatabaseAssistant {
    tools = ["query_database", "read_file"]
    main { return llm act "Query user table" }
}
```

Tool priority: built-in tools > agent functions > MCP tools.

## Built-in Templates

Helen provides code templates for quickly creating common structures:

```bash
# View all templates (查看所有模板)
helen template --list

# View template content (查看模板内容)
helen template simple_agent

# Copy template to current directory (复制模板到当前目录)
helen template spawn_channel --copy my_worker.helen
```

Available templates:
- `simple_agent` - Simple agent
- `spawn_channel` - spawn + Channel pattern
- `shared_store` - Shared store pattern
- `context_object` - Context object pattern
- `pipeline` - Pipeline pattern

## Chapter Summary

- Agent scope isolation: `let` not visible, `const` visible, `shared` visible and modifiable
- Four isolation levels: `@open` > default > `@strict` > `@sandbox`
- `media()` loads images/videos/audio for multimodal input
- Protocols (`protocol`/`impl`) define interface specifications
- Multi-format imports: `.helen`, `.json`, `.yaml`, `.toml`, Python
- `transcript` controls conversation history save level
- MCP extends tool ecosystem
- Top level only allows declarations; executable code must be in `main` or functions

## Further Reading

- [[reference/07-spawn|Concurrent Programming]] - `spawn` + Channel, `spawn resume`, `mailbox_select`
- [[reference/17-multimodal|Multimodal Support]] - `media()`, `MediaPart`, `on_media`/`on_generate` callbacks (callbacks-as-adapters design)
- [[reference/15-python-bridge|Python Bridge]] - Bidirectional Helen ↔ Python integration (FFI + Bridge)
- [[reference/16-quality-assessment|Quality Assessment]] - 7-dimension quality framework, security scoring

## Next Chapter

[Appendix: Keywords and Quick Reference](appendix.md) ->
