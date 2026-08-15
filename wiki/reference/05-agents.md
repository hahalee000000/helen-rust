# Tutorial 05: Agent Programming

> agent declarations / description / prompt / configuration

## What Is an Agent?

In Helen, agents are **first-class citizens** — not library objects, but language-level constructs.

Traditional approach (Python):

```python
class Translator:
    def __init__(self):
        self.description = "Translate text"
        self.prompt = "You are a translator..."
```

Helen approach:

```helen
agent Translator {
    description "Translate text"
    prompt "You are a translator..."
}
```

The compiler understands agent semantics and can provide completion in LSP and auto-extract them in documentation.

## Core Design Principle: Caller Decides Context

> **"Before calling an agent, ask: what does it need to know?"**

Helen agents are **strictly isolated** — each agent call (whether synchronous `call Agent(...)` or `spawn Agent(...)`) creates a brand-new, independent execution environment. **Agents do not automatically inherit any variables, history, or context from the caller.**

This is the core expression of Helen's "explicit over implicit" philosophy, fundamentally different from the "functions naturally see outer scope" behavior in languages like Python/JS.

### Why Design It This Way?

1. **Predictability**: What an agent sees is entirely determined by parameters; it cannot be polluted by outer state
2. **Reusability**: The same agent can work with different contexts at different call sites
3. **Testability**: Testing an agent does not require constructing a complete outer environment
4. **Security**: Sensitive data is not accidentally leaked to agents that don't need it

### Pre-call Checklist

Before each agent call, explicitly answer:

- [ ] **What information does this agent need to complete its task?** (input parameters)
- [ ] **Is all this information passed explicitly via parameters?**
- [ ] **Does the agent need access to cross-agent shared state?** (If so, use `shared store` or `shared let`)
- [ ] **How will the agent's output be used by the caller or other agents?** (return value / Channel / SharedStore)

### ❌ Wrong Example: Assuming Context Is Automatically Inherited

```helen
// ❌ Wrong: the agent tries to use user_name and user_id, but they are
// not passed in as parameters. The compiler will report "undefined variable"
// because agents are strictly isolated from the caller's scope.
import std.core.*
agent Greeter {
    main {
        print("Hello " + user_name + ", your id is " + str(user_id))
    }
}
```

### ✅ Correct Example: Explicitly Passing via Parameters

```helen
import std.core.*
agent Greeter(user_name: str, user_id: int) {
    main {
        // ✅ All information enters the agent via parameters
        print("Hello " + user_name + ", your id is " + str(user_id))
    }
}

main {
    let user_name = "Alice"
    let user_id = 42
    // ✅ Explicitly pass the required context at call time
    Greeter(user_name, user_id)
}
```

### Context Passing Methods for Different Scenarios

| Scenario | Recommended Method | Example |
|----------|-------------------|---------|
| One-time input | Parameter passing | `Agent(data, config)` |
| Read-only configuration | `const` module constants | Automatically visible |
| Cross-agent mutable shared state | `shared store` | `Store.field = value` |
| Output from a spawned agent | Channel messages | `ch.send(result)` |
| Resuming a conversation across processes | `resume_session(sid)` | Explicitly inherit transcript |
| Context seen by the LLM | Agent's `prompt` template | `{{var}}` placeholders |

> 💡 For detailed examples, see `helen-programming-methodology` §5 "Context Relay Pattern"

## Basic Agent

```helen
agent Translator {
    description "Translate text between languages"
    prompt """
    You are a professional translator.
    Translate the given text accurately.
    """
}
```

**Note**: Triple-quoted strings (`"""..."""`) automatically strip common leading whitespace (auto-dedent), so that multi-line strings indented in code remain clean at runtime. For example, the prompt above will not contain leading spaces at runtime.

## Agent Configuration

### Configuration Syntax: Optional `=`

Agent configuration elements support **two equivalent syntaxes** — with or without `=`:

```helen
agent Assistant {
    // Without = (traditional)
    description "Helpful assistant"
    model "gpt-4"
    temperature 0.7
    max-turns 5
    
    // With = (also valid)
    description = "Helpful assistant"
    model = "gpt-4"
    temperature = 0.7
    max-turns = 5
    tools = ["web_search", "read_file"]
}
```

**Both forms are equivalent** — the parser accepts either style. The `=` is optional for all agent configuration elements except `prompt`, which requires a string literal and does not accept `=`.

**Supported elements with optional `=`:**
- `description` / `description =`
- `model` / `model =`
- `temperature` / `temperature =`
- `max-turns` / `max-turns =`
- `max-tokens` / `max-tokens =`
- `tools` / `tools =`
- `thinking-mode` / `thinking-mode =`
- `reasoning-effort` / `reasoning-effort =`

**Note:** `prompt` always uses the form `prompt "..."` (no `=`), because it requires a literal string template for `{{variable}}` interpolation.

### description — Agent Description

The `description` field provides context to the LLM about the agent's role and purpose. It's included in the system prompt sent to the LLM.

```helen
agent CodeReviewer {
    description "You are a senior software engineer with 10 years of experience in code review."
    main { return llm act "Review this code..." }
}
```

**Best practices**:
- Be specific about the agent's role and expertise
- Include relevant context that helps the LLM understand its purpose
- Keep it concise but informative

### streaming — Enable Streaming Response (v1.21)

The `streaming` configuration enables streaming response from the agent. When enabled, the agent returns a `StreamingResponse` object instead of a complete string.

```helen
agent StreamAgent {
    description "Agent with streaming output"
    streaming true    // Enable streaming
    main { return llm act "Generate a long response" }
}

main {
    let agent = StreamAgent()
    let response = call agent
    // response is a StreamingResponse object
    for chunk in response {
        print(chunk, end="")
    }
}
```

**When to use**:
- Long-form content generation (articles, stories)
- Real-time feedback to users
- Progress indicators for long operations

### transcript — Control Conversation Persistence (v1.29)

The `transcript` configuration controls whether agent conversations are persisted to disk. This is crucial for session management and debugging.

**Three levels**:

```helen
// Level 1: "none" (default) — No persistence, zero overhead
agent QuickTask {
    transcript "none"
    main { llm act "Quick calculation" }
}

// Level 2: "memory" — In-memory only, no disk files
agent DebugAgent {
    transcript "memory"
    main { 
        let session_id = get_session_id()
        llm act "Debug issue"
    }
}

// Level 3: "persistent" — Full disk persistence
agent AuditAgent {
    transcript "persistent"
    main { llm act "Perform security audit" }
}
```

**When to use each level**:
- `none`: Batch processing, performance-critical tasks, one-shot operations
- `memory`: Development, debugging, when you need `get_session_id()` but don't want disk files
- `persistent`: Long-running services, audit requirements, session resumption, conversation history

**Chinese aliases**: `记录 "无"` / `记录 "内存"` / `记录 "持久"`

### model — Specify Model

```helen
agent SmartTranslator {
    description "High-quality translation"
    model "gpt-4"
    prompt "Translate carefully..."
}
```

### temperature — Control Randomness

```helen
agent CreativeWriter {
    description "Write creative stories"
    temperature 0.9    // High creativity
    prompt "Write a story..."
}

agent DataExtractor {
    description "Extract structured data"
    temperature 0.1    // Low randomness, precise output
    prompt "Extract data..."
}
```

### max-turns — Multi-turn Conversation

```helen
agent Interviewer {
    description "Conduct an interview"
    max-turns 5    // Maximum 5 turns of conversation
    prompt "Ask follow-up questions..."
}
```

### max-tokens - Output Length Limit (v1.31.2)

Limits the maximum output tokens for LLM response.

```helen
agent Concise {
    description "Concise responder"
    max-tokens 100    // Limit to 100 output tokens
    main { return llm act "Summarize in one sentence" }
}
```

### thinking-mode - Enable Reasoning (v1.36)

Enables thinking/reasoning mode for the LLM. When enabled, the model produces a chain-of-thought reasoning before the final answer. The reasoning content is captured separately in `reasoning_content` field and not streamed to the frontend.

```helen
agent Thinker {
    description "Deep reasoning agent"
    model "deepseek-v4-pro"
    thinking-mode true    // Enable thinking mode
    main { return llm act "Solve this complex problem..." }
}
```

### reasoning-effort - Control Reasoning Depth (v1.36)

Controls the reasoning effort level. Values: `"low"`, `"medium"`, `"high"`, `"max"`. Higher effort produces more thorough reasoning at the cost of latency and tokens.

```helen
agent DeepThinker {
    description "Maximum reasoning effort"
    model "deepseek-v4-pro"
    thinking-mode true
    reasoning-effort "max"    // Use maximum reasoning effort
    main { return llm act "Analyze this thoroughly" }
}
```

**Provider mapping** (auto-detected from `base_url`):

| Platform | `thinking-mode true` maps to | `reasoning-effort` maps to |
|----------|------------------------------|---------------------------|
| DashScope | `enable_thinking: true` + `thinking_budget` | `thinking_budget` (1024/4096/16384/32768) |
| Zhipu (GLM) | `thinking: {type: "enabled"}` | `reasoning_effort` string |
| DeepSeek | `thinking: {type: "enabled"}` | `reasoning_effort` string |
| Minimax | `reasoning_split: true` + `thinking: {type: "adaptive"}` | - |
| Kimi | - | `reasoning_effort` string |
| Volcengine (Doubao) | `thinking: {type: "enabled"}` | - |

> **Thinking + Tool Calls (v1.37):** When using `thinking-mode` together with `tools`, the `reasoning_content` is automatically preserved across tool-call rounds. This is a correctness requirement for DeepSeek (returns 400 error if missing) and Minimax (loses context). Helen handles this automatically - no user action needed.

### provider - Explicit Provider Override (v1.36)

Optionally overrides the auto-detected platform protocol. Useful when using API aggregators or custom endpoints.

```helen
agent Assistant {
    description "Custom provider"
    model "deepseek-v4-flash"
    provider "deepseek"    // Force DeepSeek protocol (overrides base_url detection)
    main { return llm act "Hello" }
}
```

> **Note**: `provider` is a context keyword (not reserved), so it can still be used as a variable name. Auto-detection from `base_url` is the default and works for all standard providers.

### tools — LLM-Visible Tool Whitelist

`tools = [...]` is the **sole whitelist for LLM visibility** (two-layer authorization model).

**Two-layer authorization:**

- The `functions {}` block declares the agent's **full capabilities** — Helen code in `main {}` can call any of these functions, but the LLM cannot see them by default.
- `tools = [...]` selects the subset **allowed for the LLM to call autonomously**.
- **Omitting `tools`** means the LLM has no tools available (except the built-in `load_skill`).

```helen
import std.core.*
agent Assistant {
    description "Helpful assistant"
    tools = ["web_search", "read_file"]   // LLM can autonomously call these two
    functions {
        fn fetch_summary(url: str): str {  // Declared in functions
            let content = read_file(url)
            return summarize(content)
        }
        fn dangerous_op() { ... }          // LLM cannot see this
    }
    main {
        // main can call any function in functions (not limited by tools)
        let summary = fetch_summary("http://example.com")
        dangerous_op()                      // ✅ main can call it
        return llm act "..."                // LLM can only call web_search/read_file/fetch_summary
    }
}
```

Names in `tools` are first looked up in the `functions {}` block (Helen functions), then in the Python tool registry (`web_search`, `read_file`, etc.). Helen functions take precedence on name conflicts.

### context {} — Context Management Configuration (v1.15+)

The `context {}` block allows customizing context management strategies per agent, including compression algorithms, working memory, etc.

#### Basic Syntax

```helen
agent SmartAssistant {
    description "Smart assistant with custom context config"
    
    context {
        compression "graduated"      // Compression strategy
        cache-aware true             // Cache-aware
        working-memory true          // Working memory
        working-memory-tokens 5000   // Working memory token budget
    }
    
    tools ["read_file", "web_search"]
    prompt "You are a helpful assistant."
    
    main {
        return llm act "..."
    }
}
```

#### Configuration Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `compression` | str | `"graduated"` | Compression strategy: `"none"` / `"graduated"` / `"traditional"` |
| `cache-aware` | bool | `true` | Enable cache-aware compression (improves cache hit rate) |
| `working-memory` | bool | `true` | Enable working memory (tracks active files, decisions, errors) |
| `working-memory-tokens` | int | `5000` | Working memory token budget |

#### Compression Strategies Explained

**1. `"none"` — No compression**

Suitable for short conversations or scenarios requiring complete history.

```helen
context {
    compression "none"
}
```

**2. `"graduated"` — Graduated compression (default)**

Multi-layer progressive strategy that automatically escalates compression intensity as context usage increases. The defaults work well for most scenarios.

```helen
context {
    compression "graduated"  // Recommended for long conversations
}
```

**3. `"traditional"` — Traditional compression**

Simple truncation strategy, suitable for quick scenarios.

```helen
context {
    compression "traditional"
}
```

#### Cache-Aware Compression

When `cache-aware` is enabled, the compression algorithm works with the LLM provider's prompt cache to reduce the cost and latency of repeated tokens:

```helen
context {
    compression "graduated"
    cache-aware true  // Works with provider caching to reduce costs
}
```

#### Working Memory

When `working-memory` is enabled, the agent automatically tracks:

- **Active files**: Recently read/written file paths
- **Recent decisions**: Key decisions from the assistant
- **TODO items**: TODOs extracted from comments
- **Error history**: Error records from tool calls

```helen
context {
    working-memory true
    working-memory-tokens 5000  // Working memory budget
}
```

#### Chinese Keywords

Chinese keyword configuration is supported:

```helen
agent 智能助手 {
    描述 "智能助手"
    
    上下文 {
        压缩 "graduated"
        缓存感知 true
        工作记忆 true
        工作记忆词元 5000
    }
    
    主逻辑 {
        返回 llm act "..."
    }
}
```

#### Complete Example: High-Performance Research Agent

```helen
agent Researcher(topic: str) {
    description "Research assistant with optimized context"
    
    // Optimize context management
    context {
        compression "graduated"      // Graduated compression
        cache-aware true             // Cache-aware
        working-memory true          // Track research files
        working-memory-tokens 8000   // Larger working memory
    }
    
    tools ["web_search", "web_fetch", "read_file", "write_file"]
    
    prompt """
    You are a research assistant.
    Research topic: {{topic}}
    
    Use tools to search and organize information.
    """
    
    main {
        let result = llm act "Start research"
        return result
    }
}
```

#### Default Behavior

If `context {}` is not specified, the agent uses default configuration:

```helen
// Equivalent to:
agent DefaultAgent {
    context {
        compression "graduated"
        cache-aware true
        working-memory true
        working-memory-tokens 5000
    }
}
```

#### Transcript Session Recording (v1.16+)

Helen automatically saves all conversation history. You can access and manage sessions via stdlib functions in an agent:

```helen
import std.core.*
import std.transcript.*
agent ChatBot {
    description "Chat bot with transcript management"
    prompt "You are a helpful chat assistant."
    
    main {
        // Get current session ID
        let session_id = get_session_id()
        print("Current session: " + session_id)
        
        // List all sessions
        let sessions = list_sessions()
        for s in sessions {
            print("{s.session_id}: {s.message_count} messages")
        }
        
        // Replay the current session
        let messages = replay_transcript()
        for msg in messages {
            print("{msg.role}: {msg.content}")
        }
        
        // Export session to file
        export_transcript("chat_log.json", "json")
        
        // Get compression audit (analyze compression efficiency)
        let audit = get_compression_audit()
        for event in audit {
            print("{event.layer}: {event.original_token_count} -> {event.compressed_token_count}")
        }
        
        // Resume a previous session
        let success = resume_session("session_1783492628_d9d9c0aa")
        if success {
            print("Session resumed")
        }
        
        return llm act "Hello!"
    }
}
```

**Use cases**:
- **Session resumption**: Use `resume_session(session_id)` to resume a previous conversation
- **Audit trail**: Use `get_compression_audit()` to analyze compression efficiency
- **Session export**: Use `export_transcript()` to save conversation records
- **Multi-session management**: Use `list_sessions()` to manage multiple sessions

**Configuration**: Configure transcript in `~/.helen/config.yaml`:

```yaml
transcript:
  enabled: true              # Enabled by default
  backend: "jsonl"           // or "sqlite"
  session_dir: "~/.helen/sessions"
```

**CLI arguments**: Use `--transcript-log` to customize the output path:

```bash
$ helen chat.helen --transcript-log=/tmp/my_chat.jsonl
```

**REPL commands**: Use transcript commands in the REPL:

```
>>> :sessions              # List all sessions
>>> :session_id            # Show current session ID
>>> :transcript            # Show current transcript
>>> :resume <session_id>   # Resume a specific session
```

See [TranscriptStore documentation](../runtime/transcript-store.md) and [stdlib reference](10-stdlib.md#transcript-functions-6-v116).

#### Transcript Control (v1.29+)

By default, agent transcripts are **not persisted** to avoid cluttering the working directory with session files. You can explicitly control transcript behavior per agent:

```helen
import std.core.*
agent 工作Agent {
    description "Simple task agent"
    model "qwen3.7-plus"
    transcript "none"  // Default: no transcript recording
    
    main {
        llm act "Process data"
        print("Done")
    }
}

agent 调试Agent {
    description "Debug agent with memory transcript"
    model "qwen3.7-plus"
    transcript "memory"  // In-memory only, no disk persistence
    
    main {
        llm act "Debug issue"
    }
}

agent 审计Agent {
    description "Audit agent with persistent transcript"
    model "qwen3.7-plus"
    transcript "persistent"  // Full persistence to disk
    
    main {
        llm act "Perform audit"
    }
}
```

**Three transcript levels**:

| Level | Behavior | Use Case |
|-------|----------|----------|
| `none` | No transcript recording (default) | Simple scripts, batch processing, performance-critical |
| `memory` | In-memory transcript, no disk persistence | Debugging, temporary analysis, session tracking |
| `persistent` | Full persistence to `.helen/sessions/` | Long-running agents, audit trails, session resumption |

**Chinese aliases**: `transcript "无"`, `transcript "内存"`, `transcript "持久"`

**Design rationale**: Most Helen programs don't need transcript persistence. The default `none` level provides zero overhead and keeps the working directory clean. Only enable `memory` or `persistent` when you explicitly need conversation tracking or audit trails.

#### tools = CONST_NAME (Reusing Tool Sets)

`tools` can reference **module-level const** values to reduce repetitive declarations and keep tool sets **statically auditable** (clear security boundaries):

```helen
// Define once at the top of the project
const FILE_TOOLS = ["read_file", "write_file", "path_exists"]
const RESEARCH_TOOLS = ["web_search", "web_fetch", "read_file"]

agent Contractor {
    tools = FILE_TOOLS                // ✅ Reuse const
    ...
}

agent Researcher {
    tools = RESEARCH_TOOLS            // ✅ Reuse const
    ...
}
```

**Strict validation** (compile-time):

| Syntax | Allowed? | Reason |
|--------|----------|--------|
| `tools = CONST_NAME` | ✅ | Module-level const, statically traceable |
| `tools = ["...", ...]` | ✅ | Literal list, static |
| `tools = my_var` | ❌ | Mutable variable, dynamic |
| `tools = my_fn` | ❌ | Function, not a list |
| `tools = OtherAgent` | ❌ | Agent, not a list |
| `tools = UNKNOWN` | ❌ | Undefined |
| Two `tools = ...` | ❌ | Duplicate declaration, ambiguous |

> ⚠️ Agent-internal const and expression concatenation (e.g., `A + B`) are not supported — this is a **security design choice**, not a limitation. Tools define the LLM's capability boundary and must be statically auditable.

**Available Built-in Tools (10):**

| Tool | Function | Parameters |
|------|----------|------------|
| `web_search` | Search the web (Bing) | `query: str` |
| `web_fetch` | Fetch web page content | `url: str` |
| `read_file` | Read a file | `path: str` |
| `write_file` | Write to a file | `path: str, content: str` |
| `patch_file` | Precisely modify files (9 fuzzy matching strategies) | `path: str, old_string: str, new_string: str` |
| `shell_exec` | Execute shell commands | `command: str` |
| `calculate` | Math calculations | `expression: str` |
| `find_files` | Find files by glob pattern (`**` for recursion) | `path: str, pattern: str = "**/*", max_results: int = 200` |
| `search_files` | Search files by content (text/regex) | `path: str, pattern: str, regex: bool = false, case_sensitive: bool = true, max_results: int = 100` |
| `load_skill` | Load skill documentation | `name: str` |

> **Note**: `load_skill` is always available (even when not listed in `tools`), used for loading skill documentation.

### File Search Tool Usage Examples (v1.15+)

`find_files` and `search_files` enable the LLM to explore codebase structure:

```helen
agent CodeExplorer {
    description "Explore and understand codebases"
    tools = ["find_files", "search_files", "read_file"]
    prompt "Explore the codebase to answer the user's question."
}

// The LLM can autonomously decide to:
// 1. find_files("src/", "**/*.py")  → List all Python files
// 2. search_files("src/", "def process", regex=false)  → Search for function definitions
// 3. read_file("src/processor.py")  → Read relevant files
```

**`find_files` — Find files by pattern** (inside an agent `main` or `functions` block)

```helen
main {
    // Find all Python files
    find_files("src/", "**/*.py")

    // Find all test files
    find_files("tests/", "**/test_*.py")

    // Find configuration files
    find_files(".", "**/*.{json,yaml,toml}")
}
```

**`search_files` — Search by content** (inside an agent `main` or `functions` block)

```helen
main {
    // Text search (default)
    search_files("src/", "TODO")

    // Regex search
    search_files("src/", "def \\w+Handler", regex=true)

    // Case-insensitive
    search_files("docs/", "warning", case_sensitive=false)
}
```

**Chinese aliases**: The corresponding stdlib functions are `查找文件()` and `搜索内容()`.

## Agent Prompt Structure (v1.15+)

Helen automatically places the agent's `description` and `prompt` in the correct positions in LLM messages:

- **`description`** → System-level behavioral rules (role, capability boundaries)
- **`prompt`** → Task-level context (specific instructions, rendered `{{}}` content)
- **`llm act "..."`** → Actual query (the user's current question)

```helen
agent CodingAgent {
    description "A coding assistant"
    prompt "You are a Python expert. Help me with coding."
    tools ["read_file", "write_file"]

    main {
        llm act "How do I sort a list?"
    }
}
```

The messages received by the LLM are roughly:

```
System: <auto-injected framework instructions> + description ("A coding assistant")
User:   prompt ("You are a Python expert...") + llm act query
```

You don't need to worry about the specifics of the framework instructions — they are automatically applied to all agents, ensuring correct behavior for tool usage, skill loading, etc. You just need to write good `description` and `prompt`.

### Further Reading

For how to **write well-crafted** `prompt` and `description` — structure layout, writing principles, anti-patterns, token budget allocation, cache-friendly design, mid-conversation injection mechanisms — see [[../reference/agent-system-prompt-guide|Agent Prompt Engineering Complete Guide]]. That guide was reverse-engineered from Claude Code's system prompt and is the key knowledge for elevating agent quality from "it runs" to "it's reliable."

---

## Agent main Block

Agents can include a `main` block as the execution entry point, invoked with `call`:

```helen
import std.core.*
import std.str.*
agent Translator(text: str, target: str) {
    description "Translate text"
    model "gpt-4"
    temperature 0.3
    prompt """
    Translate to {{target}}:
    {{text}}
    """
    
    functions {
        let default_format = "formal"
        const MAX_LENGTH = 1000
        
        fn validate_input(s: str): bool {
            return len(s) > 0
        }
        
        fn format_output(text: str): str {
            if default_format == "formal" {
                return text.upper()
            }
            return text
        }
    }
    
    main {
        if validate_input(text) {
            let result = llm act    // bare form: automatically uses the rendered prompt
            return format_output(result)
        }
        return "Input is empty"
    }
}

main {
    // Invocation (recommended function-style call):
    let translated = Translator(text="Hello", target="French")
}
```

**Variable definitions in the functions block**:

The `functions {}` block now supports `let` and `const` declarations; these variables are visible to all functions in the agent:

```helen
import std.core.*
agent MyAgent {
    description "Example agent"
    prompt "..."
    
    functions {
        let config = "default"
        const MAX_RETRIES = 3
        
        fn get_config(): str {
            return config  // ✅ Can access
        }
        
        fn retry() {
            for i in range(MAX_RETRIES) {
                print("Retry " + str(i))
            }
        }
    }
}
```

## Agent Parameters

```helen
agent Translator {
    description "Translate text"

    // Parameter declarations (type checking supported in future versions)
    // text: str — Text to translate
    // target_lang: str — Target language

    prompt """
    Translate: {{text}}
    Target language: {{target_lang}}
    """
}

main {
    let result = Translator("Hello", "French")
}
```

## Calling Agents

```helen
import std.core.*
agent Summarizer {
    description "Summarize text"
    prompt "Summarize the following:"
}

main {
    let text = "Long article content here..."
    let summary = Summarizer(text)
    print(summary)
}
```

## Complete Example: Email Classification System

```helen
import std.core.*
agent EmailClassifier {
    description "Classify emails into categories"
    model "gpt-4"
    temperature 0.1
    prompt """
    Classify the email into one of:
    - urgent: Requires immediate attention
    - meeting: Calendar-related
    - informational: FYI only
    - spam: Unwanted email
    """
}

agent UrgentResponder {
    description "Draft response to urgent emails"
    prompt "Draft a professional response..."
}

agent EmailClassifier {
    description "Classify emails"
    prompt "Classify this email..."
    main {
        let email = "URGENT: Server down in production!"

        llm if "Classify this email" {
            branch "urgent" {
                print("🚨 URGENT email detected!")
                UrgentResponder(email)
            }
            branch "meeting" {
                print("📅 Meeting request")
            }
            branch "informational" {
                print("📧 FYI email")
            }
            branch "spam" {
                print("🗑️ Spam, ignoring")
            }
            default {
                print("📬 Uncategorized")
            }
        }
    }
}
```

## Exercises

1. Create an agent with the description "Determine text sentiment" and test with different inputs
2. Create an agent with temperature set to 0 and observe output stability
3. Create a multi-agent system: classifier + responder + summarizer

---

## Shared State and Communication (v1.12 / v1.13)

Multi-agent systems often need to share state or communicate with each other. Helen provides two mechanisms: **shared store** and **channel**.

### Shared Store: Structured Shared State

`shared store` is used to share **mutable state** across agents, especially reference types (list, dict).

```helen
import std.core.*
import std.str.*
shared store TaskRegistry {
    let tasks: list = []
    let counter: int = 0
    
    fn register(task_name: str) {
        counter = counter + 1
        tasks.append(task_name)
    }
    
    fn count(): int { return counter }
    
    fn get_task(index: int): str { return tasks[index] }
}

// All agents can access it
agent Worker() {
    main {
        TaskRegistry.register("my-task")
        print("Total tasks: " + str(TaskRegistry.count()))
    }
}
```

**Key features**:
- ✅ Thread-safe: All method calls are automatically locked (RLock)
- ✅ Visible to all agents by default
- ✅ Supports reference types like list, dict
- ✅ Supports default parameter values on methods (since v1.29.15)
- ❌ Cannot directly access fields with `_` prefix (private fields)

**Default parameter values** (since v1.29.15):

Shared store methods support default values just like regular functions:

```helen
import std.math.*
shared store Logger {
    let logs: list = []
    
    fn log(message: str, level: str = "info") {
        logs.append("[" + level + "] " + message)
    }
}

main {
    Logger.log("starting up")             // level defaults to "info"
    Logger.log("disk full", "error")      // explicit level
}
```

Before v1.29.15, calling a method without passing a defaulted parameter left the parameter `undefined`, causing runtime errors. The fix in `helen/interpreter/shared_store.py` evaluates the default expression at call time (not at declaration time), matching regular function semantics.

**Private fields** (`_` prefix):

```helen
import std.core.*
shared store BankAccount {
    let balance: int = 1000
    _transactionLog: list = []  // Private: not visible externally
    
    fn withdraw(amount: int) {
        balance -= amount
        _transactionLog.append("withdraw: " + str(amount))
    }
    
    fn getHistory(): list {
        return _transactionLog  // Accessible within methods
    }
}

main {
    // ✅ Public interface
    BankAccount.withdraw(100)
    print(BankAccount.balance)  // Output: 900

    // ❌ Private field
    print(BankAccount._transactionLog)  // Error!
}
```

### Channel: Inter-Agent Message Communication (v1.18+)

`spawn` returns a **Channel** (mailbox) for bidirectional communication with the spawned agent. Channel provides `send`/`receive`/`try_receive`/`cancel`/`close` methods.

```helen
// Worker agent receives a Channel parameter to reply with results
import std.core.*
agent Worker(task: str, reply: Channel) {
    main {
        let result = "Done: " + task
        reply.send(result)
    }
}

main {
    // spawn returns a Channel, automatically injected as the agent's last parameter
    let mailbox = spawn Worker("Task A")
    print(mailbox.receive())  // "Done: Task A"
}
```

**Channel API:**

| Method | Description |
|--------|-------------|
| `channel.send(value)` | Send a message to the Channel |
| `channel.receive()` | Blocking receive |
| `channel.try_receive()` | Non-blocking receive; returns null if no message |
| `channel.cancel()` | Cancel the agent associated with the Channel |
| `channel.close()` | Close the Channel |

**Chinese aliases**: `发送()`, `接收()`, `尝试接收()`, `取消()`, `关闭()`.

#### Multi-Channel Selection: mailbox_select

When listening on multiple Channels simultaneously, use `mailbox_select` for multiplexing:

```helen
import std.concurrency.*
import std.core.*
import std.tools.*
agent Fetcher(url: str, reply: Channel) {
    main {
        let data = web_fetch(url)
        reply.send(data)
    }
}

main {
    let mb1 = spawn Fetcher("https://api.example.com/a")
    let mb2 = spawn Fetcher("https://api.example.com/b")

    // Wait for any Channel to return a result
    let result = mailbox_select([mb1, mb2])
    print("First to return: " + result)
}
```

**Chinese alias**: `邮箱选择([mb1, mb2])`.

#### Concurrency Pattern Example

```helen
// Producer agent: sends multiple messages to a Channel
import std.core.*
agent Producer(items: list, reply: Channel) {
    main {
        for item in items {
            reply.send("Processing: " + item)
        }
        reply.send("done")  // Completion signal
    }
}

main {
    // Consumer: receives messages from the Channel
    let mailbox = spawn Producer(["apple", "banana", "cherry"])
    let msg = mailbox.receive()
    while (msg != "done") {
        print(msg)
        msg = mailbox.receive()
    }
    mailbox.close()
}
```

### spawn and Shared State (v1.18+)

`spawn` can start agents in the background and communicate via Channels. Multiple spawned agents can concurrently access a shared store:

```helen
import std.core.*
shared store Counter {
    let count: int = 0
    fn increment() { count = count + 1 }
}

agent Worker(reply: Channel) {
    main {
        Counter.increment()
        reply.send("done")
    }
}

main {
    // Start 3 concurrent agents sharing the same Counter
    let mb1 = spawn Worker()
    let mb2 = spawn Worker()
    let mb3 = spawn Worker()

    // Wait for all agents to finish
    print(mb1.receive())  // "done"
    print(mb2.receive())  // "done"
    print(mb3.receive())  // "done"

    print(Counter.count)  // Output: 3
}
```

**Thread safety guarantees**:
- SharedStore uses RLock internally to protect all field access
- When multiple spawns call methods concurrently, execution is automatically serialized
- The main thread and spawned agents can access the same SharedStore simultaneously
- Channel `send`/`receive` operations are also thread-safe

---

## 📦 Built-in Template Library

Helen provides a set of **built-in templates** covering common agent patterns. Each template is a complete, runnable example with detailed comments.

### Viewing Templates

```bash
# List all templates
helen template --list

# View template content
helen template simple_agent
helen template spawn_channel

# Copy a template to the current directory
helen template spawn_channel --copy my_worker.helen
```

### Available Templates

| Template | Purpose |
|----------|---------|
| `simple_agent` | Simple agent invocation |
| `spawn_channel` | spawn + Channel concurrency |
| `spawn_with_transcript` | spawn + transcript inheritance |
| `shared_store` | SharedStore data exchange |
| `context_object` | Context object for aggregating parameters |
| `pipeline` | Agent pipeline (sequential processing) |

### 💡 Usage Recommendations

1. **Beginners**: Start with `simple_agent` to understand basic call patterns
2. **Concurrency**: Look at `spawn_channel` to understand isolation and communication
3. **Multi-agent collaboration**: Look at `shared_store` and `context_object`
4. **Complex workflows**: Look at `pipeline` to compose multiple agents

All templates follow the **"caller decides context"** principle — all information for agents is passed explicitly via parameters.

---

> **Next**: [[reference/06-llm-statements|LLM Statements in Practice]]
