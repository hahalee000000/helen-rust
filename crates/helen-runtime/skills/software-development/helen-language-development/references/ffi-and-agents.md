# Helen Python FFI Pitfalls & Agent Development Patterns

## Stdlib vs Python FFI — Prefer Stdlib

**Rule**: Always prefer stdlib built-in functions over Python FFI. Stdlib functions are simpler, more reliable, and don't have FFI quirks.

### File & Path Operations (stdlib — preferred)

```helen
// File I/O — no imports needed
import std.core.*
import std.io.*
import std.path.*
write_file("data/output.txt", "Hello!")     // auto-creates parent dirs
append_file("data/output.txt", "\nMore")    // auto-creates if missing
let content = read_file("data/output.txt")  // returns "" if not found

// Directory creation
mkdir_p("logs/2024/01")   // creates full directory tree
mkdir("logs/2024")         // single level (parent must exist)

// Path operations
path_exists("data/output.txt")              // true/false
path_is_file("data/output.txt")             // true/false
path_is_dir("data")                         // true/false
path_dirname("/home/user/file.txt")         // "/home/user"
path_basename("/home/user/file.txt")        // "file.txt"
path_join("/home", "user", "file.txt")      // "/home/user/file.txt"
```

### String Operations (stdlib — preferred)

```helen
import std.str.*
let sub = substring("Hello, World!", 0, 5)   // "Hello"
let rest = substring("Hello, World!", 7)     // "World!"
let cmd = trim_prefix("/remember fact", "/remember ")  // "fact"
let name = trim_suffix("file.txt", ".txt")   // "file"
```

### Python FFI Pitfalls (when stdlib is not enough)

When you MUST use Python FFI (`import "module" as alias`), watch for these quirks:

1. **Keyword arguments often fail** — `pathlib.Path.write_text(content, encoding="utf-8")` fails; use positional args only or use stdlib `write_file()` instead
2. **`os.makedirs(path, exist_ok=true)` fails** — keyword args unreliable; use stdlib `mkdir_p()` instead
3. **`open()` is not a Helen builtin** — must use `import "io" as io` then `io.open()`; but prefer stdlib `write_file()`/`append_file()`
4. **FFI callbacks run synchronously in Helen's thread (no asyncio loop)** — when Helen calls a Python function (e.g. an `on_chunk` / streaming-emit callback registered from Python), it executes synchronously in Helen's execution thread, which has **no running asyncio event loop**. Inside such a callback you cannot `await`, `asyncio.create_task(...)`, or push to an `asyncio.Queue` directly (you'll get `RuntimeError: no running event loop`). Marshal work back to your event loop with `loop.call_soon_threadsafe(...)`. `async_call` does **not** fix this direction — it only makes the *Python->Helen* call asynchronous via a thread pool. Full example and the `spawn`/cancel caveats: see `reports/python_bridge.md` -> "FFI 回调与 asyncio（线程安全）".

---

## Agent Development Patterns

Helen's agent system enables building autonomous AI agents with tool use and persistent memory.

### Agent Declaration Structure

```helen
agent MyAgent(input: str) {
    description "Agent purpose"
    prompt """System prompt with instructions..."""
    
    tools = ["web_search", "read_file", "write_file", "shell_exec"]
    model "qwen3.7-plus"
    max-turns 10
    temperature 0.7
    
    main {
        // Use llm act with on_chunk callback for real-time output (supports tool calling!)
        llm act input
    }
}
```

**Important**: Use `llm act` with `on_chunk` callback for agent responses. `llm act` supports tool calling AND provides real-time streaming output via callbacks.

### Key Patterns

- **Tool declaration**: `tools = [...]` is the LLM's only visibility allowlist (two-layer authorization: `functions {}` declares capability, `tools` decides what the LLM can invoke). Listed tools are included in `llm act` calls.
- **Prefer `llm act` with `on_chunk`**: Supports tool calling AND provides real-time streaming output via callbacks
- **Memory via markdown files**: Store knowledge in `memory/*.md` files, read at start of each task
- **Memory injection**: Load memory content and prepend to user prompt before `llm act`
- **Interactive loops**: Use `while (true)` with `input()` for continuous conversation
- **Special commands**: Handle `/quit`, `/memory`, `/remember`, `/plan` in main loop before calling agent
- **Skill loading**: `load_skill` tool is auto-included for Tier 2 skill disclosure
- **Stdlib over FFI**: Always prefer stdlib functions (write_file, mkdir_p, path_*, substring, trim_prefix) over Python FFI

### Memory-Augmented Agent Example

```helen
import std.core.*
agent SmartAgent(user_input: str) {
    prompt "You are an assistant with persistent memory..."
    tools = ["read_file", "write_file", "web_search"]
    
    main {
        // Use stdlib functions — no Python FFI needed
        let memory = read_file("memory/MEMORY.md")
        let full_prompt = ""
        if (memory != "") {
            let full_prompt = "## Memory\n" + memory + "\n\n## Request\n" + user_input
        } else {
            let full_prompt = user_input
        }
        llm act full_prompt  // streaming output with tool support via on_chunk callback
    }
}
```

### Built-in Tools Available to Agents

| Tool | Description |
|------|-------------|
| `web_search(query)` | Search via Wikipedia API |
| `web_fetch(url)` | Fetch and extract text from URL |
| `read_file(path)` | Read file content |
| `write_file(path, content)` | Write content to file |
| `shell_exec(command, timeout?)` | Execute shell commands |
| `calculate(expression)` | Safe math evaluation |
| `patch_file(path, old, new)` | Fuzzy file editing |
| `load_skill(name)` | Load skill documentation |

### Interactive Agent with Commands

```helen
import std.core.*
import std.io.*
import std.path.*
import std.str.*
main {
    print("Agent ready. Type /quit to exit.")
    
    // Initialize memory directory
    mkdir_p("memory")
    if (!path_exists("memory/MEMORY.md")) {
        write_file("memory/MEMORY.md", "# Memory\n")
    }
    
    while (true) {
        let user_input = input("You > ")
        
        if (user_input == "/quit") { break }
        
        if (startswith(user_input, "/remember ")) {
            let fact = trim_prefix(user_input, "/remember ")
            append_file("memory/MEMORY.md", "\n- " + fact)
            print("Saved: " + fact)
            continue
        }
        
        // Normal: call agent
        MyAgent(user_input)
    }
}
```

### Reference Implementation

See `~/helen/examples/helenchat/helenchat.helen` for a complete working example of a Hermes-like agent with:
- Interactive REPL loop
- Memory persistence via `memory/MEMORY.md`
- Commands: `/quit`, `/memory`, `/remember`, `/plan`, `/clear`
- Full tool suite (web_search, file I/O, shell_exec, etc.)
- Skill loading support
