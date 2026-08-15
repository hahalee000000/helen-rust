---
name: helen-agent-patterns
description: "Helen Agent Design Patterns — Single Agent core patterns, scope isolation, routing, streaming, tool callbacks, best practices"
version: 1.22.0
author: Helen Team
license: MIT
tags: [helen, agent, patterns, design, llm, scope-isolation, shared-let, v1.12, closure, spawn, channel, v1.18, on-tool-end, v1.21, invocation, v1.22]
---
<!-- helen-rust edition: agent runtime in crates/helen-runtime (M6). Tier B agent suite 170/172 vs reference. -->


# Helen Agent Design Patterns

Helen treats Agent as a **first-class language construct**, providing declarative syntax and powerful LLM integration. This guide focuses on **single Agent core patterns**; for multi-Agent collaboration, see `helen-agent-collaboration`.

## 🎯 First Principle: Caller Decides Context

> **"Before calling an agent, ask: what does it need to know?"**

- Agents are **strictly isolated** — each invocation creates an independent execution environment
- They do **not automatically inherit** the caller's variables, history, or LLM context
- **All** context must be passed **explicitly** via arguments, `shared store`, `const`, or Channel

Regardless of the collaboration pattern chosen, **the first step is always to draw the context flow diagram**:

```
Caller ──arguments──► Agent input
       ──SharedStore──► Shared state
       ◄──return value/Channel── Agent output
```

> 💡 Full explanation in `helen-agent-collaboration` § "Design Principle: Caller Decides Context"

---

## Agent Basics

### Minimal Agent

```helen
agent SimpleAgent {
    description "A simple agent"
    prompt "You are a helpful assistant."

    main {
        return llm act "Hello, world!"
    }
}

let result = SimpleAgent()
```

### Parameterized Agent

```helen
agent Translator(text: str, target_lang: str) {
    description "Translate text to target language"
    prompt "You are a professional translator. Translate to {{target_lang}}."
    model "gpt-4"
    temperature 0.3

    main {
        return llm act "Translate: " + text
    }
}

let result = Translator("Hello", "Chinese")
```

### Agent Configuration Options

**Configuration syntax:** Agent configuration elements support **two equivalent syntaxes** — with or without `=`. Both forms are valid and equivalent:

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
    tools = ["web_search", "read_file"]
}
```

**Note:** `prompt` always uses `prompt "..."` (no `=`), because it requires a literal string template for `{{variable}}` interpolation.

**Full configuration example:**

```helen
agent ConfiguredAgent {
    // === Identity ===
    description "Agent with full configuration"
    prompt "You are an expert assistant."
    
    // === Model & Behavior ===
    model "gpt-4"              # LLM model
    temperature 0.7            # Creativity (0.0-1.0)
    max-turns 10               # Maximum tool call rounds
    max-tokens 4096            # Max output tokens (v1.31.2)
    
    // === Advanced Features ===
    streaming true             # Enable streaming response
    thinking-mode true         # Enable thinking/reasoning (v1.36)
    reasoning-effort "high"    # low/medium/high/max (v1.36)
    provider "openai"          # Explicit provider override (v1.36)
    
    // === Tools & Authorization ===
    tools = ["web_search", "read_file", "write_file"]
    
    // === Transcript Control (v1.29) ===
    transcript "persistent"    # "none" (default), "memory", or "persistent"
    
    main {
        return llm act "Do something complex"
    }
}
```

**Configuration elements reference:**

| Element | Type | Purpose | Example |
|---------|------|---------|---------|
| `description` | string | Agent role description for LLM | "Senior code reviewer" |
| `model` | string | LLM model selection | "gpt-4", "qwen3.7-plus" |
| `temperature` | float | Control creativity (0.0-2.0) | 0.1 (precise), 0.7 (balanced), 1.2 (creative) |
| `max-turns` | int | Limit tool-calling iterations | 1, 10, 50 |
| `max-tokens` | int | Max output tokens | 100, 4096, 131072 |
| `streaming` | bool | Enable streaming response | true, false |
| `transcript` | string | Conversation persistence | "none", "memory", "persistent" |
| `tools` | list | LLM-visible tool whitelist | ["read_file", "web_search"] |
| `thinking-mode` | bool | Enable reasoning mode (v1.36) | true, false |
| `reasoning-effort` | string | Reasoning depth (v1.36) | "low", "medium", "high", "max" |
| `provider` | string | Explicit provider (v1.36) | "openai", "anthropic" |

**Note**: All elements support optional `=` syntax except `prompt` (requires literal string for template interpolation).

**Thinking mode (v1.36):** When enabled, the model produces chain-of-thought reasoning before the final answer. The reasoning is captured separately (not streamed to frontend) and mapped to provider-specific parameters automatically:

| Platform | `thinking-mode true` maps to |
|----------|------------------------------|
| DashScope | `enable_thinking: true` + `thinking_budget` |
| Zhipu / DeepSeek / Volcengine | `thinking: {type: "enabled"}` |
| Minimax | `reasoning_split: true` + `thinking: {type: "adaptive"}` |

> **Thinking + Tool Calls (v1.37):** When using `thinking-mode` together with `tools`, the `reasoning_content` is automatically preserved across tool-call rounds. This is required by DeepSeek (returns 400 error if missing) and Minimax (loses context). Helen handles this automatically in both `act()` and `act_stream()` - no user action needed.

**Provider override (v1.36):** The `provider` context keyword optionally overrides auto-detection from `base_url`:

```helen
agent CustomProvider {
    model "deepseek-v4-flash"
    provider "deepseek"         # Force DeepSeek protocol
    main { return llm act "Hello" }
}
```

`tools` can reference module-level const to reduce repetition and keep tool sets statically auditable:

```helen
const FILE_TOOLS = ["read_file", "write_file", "path_exists"]
agent FileWorker {
    tools = FILE_TOOLS
    main { ... }
}
```

### Transcript Control (v1.29+)

Control whether agent conversations are persisted to disk. Default is `none` to avoid cluttering the working directory:

```helen
// No transcript (default, zero overhead)
agent SimpleTask {
    transcript "none"
    main { llm act "Quick task" }
}

// In-memory only (debugging, no disk files)
agent DebugAgent {
    transcript "memory"
    main { llm act "Debug issue" }
}

// Persistent (audit trails, session resumption)
agent AuditAgent {
    transcript "persistent"
    main { llm act "Perform audit" }
}
```

**When to use each level**:
- `none`: Most scripts, batch processing, performance-critical tasks
- `memory`: Development, debugging, when you need `get_session_id()` but don't want disk files
- `persistent`: Long-running services, audit requirements, when you need to resume sessions

**Chinese**: `记录 "无"` / `记录 "内存"` / `记录 "持久"`

### Agent vs Skill

| Dimension | Agent | Skill |
|-----------|-------|-------|
| Nature | Runtime entity | Static document |
| Callable | ✅ `Agent()` | ❌ Not callable |
| Stateful | ✅ Maintains conversation/tool state | ❌ Stateless |
| Purpose | **Execute** tasks | **Guide** how to execute |

Agents can load Skills as knowledge sources: `load_skill("helen-testing")`.

---

## Agent Scope Isolation (v1.10/v1.12)

### Core Rules

**Agent main runs in a fully isolated environment**:

| Variable Type | Visible in agent main | Description |
|--------------|----------------------|-------------|
| Module-level `let` | ❌ Not visible | Compile-time error (except @open) |
| Module-level `const` | ✅ Auto-visible | Read-only sharing |
| `shared let` (value types) | ✅ Visible | Writable across agents |
| `shared store` | ✅ Visible | Accessed via methods |
| Local variables | ✅ Visible | Closure value capture |

**v1.12 Enhancements**:
- `@open` / `@strict` / `@sandbox` — three isolation decorators
- `shared let` only allows value types (int/float/str/bool)
- Reference-type arguments are automatically wrapped in a read-only view (ReadOnlyView)
- Closures use value capture (deep-copy snapshot)
- `arr[i] = x` and `obj.field = x` are also subject to isolation checks

### Isolation Levels

```helen
@open agent DebugAgent() {         // L0: Module-level let visible (for debugging)
    main { return module_let }
}
agent NormalAgent() { ... }        // L1: Standard isolation (default)
@strict agent StrictAgent(data: list) {  // L2: Deep-copies arguments and return values
    main { data.append(4); return data }
}
@sandbox agent SafeAgent() { ... } // L3: Forces tools=[]
```

### Example: Scope Isolation

```helen
import std.core.*
let module_counter = 0           // ❌ Not visible in agent main
const MAX_RETRIES = 3            // ✅ Auto-visible in agent main
shared let shared_count = 0      // ✅ Visible and writable in agent main

agent Worker(task: str) {
    functions {
        fn process(): str {
            module_counter = module_counter + 1  // ✅ Visible in functions block
            return "processed: " + task
        }
    }

    main {
        // ❌ Compile error: module_counter is not visible in agent main
        print("Max retries: " + MAX_RETRIES)     // ✅ const auto-visible
        shared_count = shared_count + 1           // ✅ shared let visible
        return llm act "Process: " + task
    }
}
```

### Parameter Read-Only + Closure Capture

```helen
import std.core.*
import std.list.*
agent ProcessItems(items: list<int>) {
    main {
        let first = items[0]         // ✅ Readable
        // items[0] = 999            // ❌ ScopeViolationError
        let my_items = list(items)   // Create a copy to modify
        return my_items
    }
}

agent DataProcessor(data: list) {
    main {
        let threshold = 10
        fn filter(items: list): list {  // Closure captures local variable
            let result = []
            for item in items {
                if item > threshold { result.append(item) }
            }
            return result
        }
        return llm act "Filtered: " + str(filter(data))
    }
}
```

### shared let Cross-Agent Collaboration

```helen
import std.core.*
shared let request_count = 0
shared let last_request_time = ""

agent RequestCounter() {
    main {
        request_count = request_count + 1
        last_request_time = "2024-01-01"  // str is a value type
        return request_count
    }
}

// Reference types are passed as arguments, auto-wrapped as read-only
agent CacheWriter(cache: map, key: str, value: any) {
    main {
        let my_cache = dict(cache)  // Create a copy
        my_cache[key] = value
        return my_cache
    }
}
```

### Shared Store Collaboration (v1.12)

```helen
import std.core.*
shared store TaskManager {
    let pending = 0
    let completed = 0
    fn submit()  { pending = pending + 1 }
    fn finish()  { pending = pending - 1; completed = completed + 1 }
    fn get_status(): str {
        return str(pending) + " pending, " + str(completed) + " completed"
    }
}

agent TaskProducer() { main { TaskManager.submit(); TaskManager.submit() } }
agent TaskWorker()   { main { TaskManager.finish() } }

main {
    TaskProducer()
    TaskWorker()
    print(TaskManager.get_status())  // "1 pending, 1 completed"
}
```

### v1.12 Isolation Fix Summary

| Fix Item | After Fix |
|----------|-----------|
| ReadOnlyView | Read operations work correctly; iterated items also wrapped |
| Closure capture | Deep-copy snapshot, immune to subsequent modifications |
| @sandbox | LLM tool list forced to empty |
| SharedStore | RLock protection; `_`-prefixed properties inaccessible |

---

## Agent Context Isolation (v1.22/v1.23)

In addition to variable scope isolation, v1.22 introduces **invocation-level context isolation** — each entry into an agent `main {}` creates a new `invocation_id`, so the LLM only sees messages from the current invocation.

```helen
agent AgentA { main { return llm act "I am Alice" } }
agent AgentB { main { return llm act "What is my name?" } }

let a = AgentA()  // invocation_id: inv_abc123
let b = AgentB()  // invocation_id: inv_def456
// AgentB's LLM cannot see AgentA's conversation — each main {} is fresh context
```

| Isolation Dimension | Scope Isolation (v1.10/v1.12) | Context Isolation (v1.22/v1.23) |
|--------------------|-------------------------------|---------------------------------|
| What is isolated | Variables | LLM conversation history |
| Mechanism | Compile-time checks | Runtime invocation_id filtering |
| Purpose | Prevent variable pollution | Prevent context leakage |

Nested calls form an invocation tree (`parent_invocation_id`), queryable via `list_invocations()` / `get_invocation_tree()`. `restore_context()` supports filtering by invocation. v1.23 fixed a bug where `_prepare_history_for_llm()` bypassed invocation filtering.

---

## Design Patterns

### Pattern 1: Expert Agent

**Scenario**: Create expert Agents for specific domains

```helen
agent CodeExpert {
    description "Programming expert"
    prompt """
    You are a senior software engineer with 20 years of experience.
    You provide clear, concise, and correct code solutions.
    Always explain your reasoning.
    """
    tools = ["read_file", "write_file", "shell_exec"]

    main {
        return llm act "Review this code and suggest improvements"
    }
}

agent MathExpert {
    description "Mathematics expert"
    prompt "You are a mathematics professor. Provide rigorous proofs."
    temperature 0.2  // Low temperature, more deterministic

    main {
        return llm act "Solve this math problem step by step"
    }
}
```

### Pattern 2: Routing Agent (llm if)

**Scenario**: Route to different expert Agents based on input content

```helen
agent TechSupport(query: str) {
    description "Technical support specialist"
    prompt "You are a technical support expert."
    main { return llm act "Help with: " + query }
}

agent BillingSupport(query: str) {
    description "Billing support specialist"
    prompt "You are a billing support expert."
    main { return llm act "Help with: " + query }
}

agent GeneralSupport(query: str) {
    description "General support specialist"
    main { return llm act "Help with: " + query }
}

// Routing Agent
agent SupportRouter(query: str) {
    description "Route support queries to specialists"

    main {
        llm if query {
            case "technical issue, bug, error, crash, not working" {
                return TechSupport(query)
            }
            case "billing, payment, invoice, subscription, refund" {
                return BillingSupport(query)
            }
            default {
                return GeneralSupport(query)
            }
        }
    }
}

let response = SupportRouter("I can't login to my account")
// Routes to TechSupport
```

### Pattern 3: Pipeline Agent

**Scenario**: Multiple Agents process sequentially, each stage handling one aspect

```helen
agent Researcher(topic: str) {
    description "Research specialist"
    tools = ["web_search", "web_fetch"]

    main {
        return llm act "Research this topic and provide key findings: " + topic
    }
}

agent Writer(topic: str, research: str) {
    description "Content writer"
    prompt "You are a professional content writer."

    main {
        return llm act "Write an article about " + topic +
                       " based on this research: " + research
    }
}

agent Editor(content: str) {
    description "Content editor"
    prompt "You are a meticulous editor. Fix grammar, improve clarity."
    temperature 0.3

    main {
        return llm act "Edit and improve this content: " + content
    }
}

// Pipeline
agent ContentPipeline(topic: str) {
    description "Research → Write → Edit pipeline"

    main {
        let research = Researcher(topic)
        let draft = Writer(topic, research)
        let final = Editor(draft)
        return final
    }
}

let article = ContentPipeline("Helen programming language")
```

### Pattern 4: Concurrent Agent (spawn + Channel)

**Scenario**: Multiple Agents execute concurrently to improve throughput

```helen
import std.core.*
agent DataFetcher(source: str) {
    description "Fetch data from a source"
    tools = ["http_get"]

    main {
        return llm act "Fetch data from: " + source
    }
}

agent DataAggregator {
    description "Aggregate data from multiple sources"

    main {
        let m1 = spawn DataFetcher("https://api.source1.com/data")
        let m2 = spawn DataFetcher("https://api.source2.com/data")
        let m3 = spawn DataFetcher("https://api.source3.com/data")

        let r1 = m1.receive()
        let r2 = m2.receive()
        let r3 = m3.receive()
        let results = [r1, r2, r3]

        return llm act "Aggregate these results: " + str(results)
    }
}
```

**Multiplexing**: `mailbox_select([m1, m2, m3])` returns the first-ready Channel result.

**⚠️ Transcript Runtime Isolation** (key design principle):

Each agent created by `spawn` runs in an **independent Interpreter instance** with its own `session_id` and transcript. This is **intentional**:

- Same process, multiple `get_session_id()` calls → same ID
- Restart program → new session_id (`session_{timestamp}_{uuid8}`)
- `spawn` → new session_id + new transcript
- Regular agent call (same process) → shared session_id, distinguished by `invocation_id`

#### ❌ Anti-pattern: Assuming Automatic Inheritance

```helen
import std.transcript.*
agent Worker(task: str, ch: Channel) {
    main {
        let sid = get_session_id()     // ❌ This is the worker's own new session
        ch.send("done in " + sid)
    }
}
```

#### ✅ Relay Template: Explicitly Pass session_id

```helen
import std.transcript.*
main {
    let parent_sid = get_session_id()
    let m = spawn Worker("task", parent_sid)  // Pass explicitly
}

agent Worker(task: str, parent_sid: str, ch: Channel) {
    main {
        resume_session(parent_sid)  // Explicitly inherit parent transcript
        ch.send("done")
    }
}
```

#### 📋 Three Relay Methods

| Scenario | Recommended Approach |
|----------|---------------------|
| Spawned child agent needs parent transcript | Pass parent_sid + `resume_session` (import parent messages into child's new session) |
| Agent output needs to be visible to other agents | `working_memory_set` + pass via Channel |
| Continue a saved **child** session across restarts (v1.27) | Persist child_sid + `spawn A(...) resume(child_sid)` (true resumption) |
| Continue the **main** session across restarts | `helen --session=sid` / `Interpreter(session_id=sid)` |

> 🔑 **Mnemonic**: "spawn means isolation, relay requires explicit passing"
>
> **v1.27 distinction**: `spawn A(...) resume("sid")` is **true resumption** - the spawned interpreter continues appending to session `sid`'s transcript. `resume_session(sid)` is an **import** - it copies another session's messages into the current new session. Use `resume(...)` to continue a saved child session; use `resume_session()` to reference another session's history from a fresh one. Resuming restores LLM conversation memory, **not** runtime variable state.

### Pattern 5: Streaming Agent (llm act + on_chunk callback)

**Scenario**: Real-time output of LLM responses

**v1.18**: `llm stream` was removed (v1.14), `for await` was removed (v1.18). Streaming is unified under `llm act` + `on_chunk`.

```helen
import std.io.*
fn print_chunk(chunk: str) {
    stream_print(chunk)
}

agent StreamingWriter(topic: str) {
    description "Write content with streaming output"

    main {
        llm act "Write a detailed article about " + topic on_chunk print_chunk
    }
}

StreamingWriter("The future of AI")
```

Streaming with full callbacks:

```helen
import std.core.*
import std.io.*
fn on_chunk(chunk: str) { stream_print(chunk) }
fn on_complete() { print("\n\n✅ Done") }

agent StreamingWriter(topic: str) {
    main {
        llm act "Write article about " + topic on_chunk on_chunk on_complete on_complete
    }
}
```

**v1.32**: Closures are first-class callable objects, so you can use anonymous closures directly:

```helen
import std.core.*
import std.io.*
agent StreamingWriter(topic: str) {
    main {
        llm act "Write article about " + topic
            on_chunk fn(c) { stream_print(c) }
            on_complete fn() { print("\n\n✅ Done") }
    }
}
```

#### Streaming Interrupt (v1.18, v1.39.7)

`on_chunk` returning `false` terminates streaming early. `spawn` + `Channel.cancel()` interrupts background agent streaming:

```helen
import std.io.*
import std.llm.*
fn conditional_chunk(chunk: str) {
    stream_print(chunk)
    if should_stop() { return false }  // Terminate streaming
}

let mailbox = spawn StreamingAgent("long task")
mailbox.cancel()  // Interrupt background streaming

cancel_llm_call(call_id)
取消大模型调用(call_id)  // Chinese alias
```

**Cancel during tool execution (v1.39.7)**: The stop button in the Web UI now works during tool execution, not just during LLM text streaming. Cancel checks are placed before each sequential tool dispatch, between concurrent tool completions, before yielding tool results, and in the `on_tool_end` callback. This ensures the agent responds to stop requests throughout the entire agentic loop.

### Pattern 5B: Injecting Hints After Tool Execution (on_tool_end, v1.21)

**Scenario**: Guide LLM direction after tool execution in an agentic loop.

**Signature**: `fn(tool_name: str, tool_result: str): str | dict | null`
- Returns str → injected as `user` message (with `[System Hint]` prefix)
- Returns dict → `{"role": "user"|"system", "content": "..."}`
- Returns null → no injection

Injected hints are automatically saved to TranscriptStore.

```helen
import std.io.*
agent Coder {
    tools ["write_file", "shell_exec", "read_file"]

    main {
        llm act "Create hello.py and run it"
            on_chunk fn(c) { stream_print(c) }
            on_tool_end fn(name, result) {
                if name == "write_file" {
                    return "File written, next step: run tests to verify"
                }
                if name == "shell_exec" {
                    return {"role": "system", "content": "Dangerous commands like rm -rf are forbidden"}
                }
                return null
            }
    }
}
```

**External queue integration**:

```helen
agent Worker {
    tools ["read_file", "write_file"]
    main {
        llm act "Complete the assigned task"
            on_tool_end fn(name, result) {
                let hint = get_hint_from_queue()
                return hint  // Not injected when null
            }
    }
}
```

`on_tool_end` can be combined with `on_chunk` / `on_complete`:

```helen
import std.core.*
import std.io.*
llm act "task"
    逐块处理 fn(c) { stream_print(c) }
    完成 fn() { print("\n✅ Done") }
    工具结束 fn(name, result) { return "hint" }
```

### Pattern 5C: Adaptive Parameters (v1.39.8)

**Scenario**: Adjust LLM parameters at runtime based on task characteristics.

Agent declarations set defaults; stdlib functions override per-invocation without affecting prompt cache:

```helen
import std.core.*
import std.llm.*

agent AdaptiveAgent {
    temperature 0.5
    max-turns 3

    main {
        // Quick factual answer
        set_temperature(0.1)
        set_max_turns(1)
        let answer = llm act "What is 2+2?"

        // Creative exploration
        set_temperature(0.9)
        set_max_turns(10)
        set_thinking_mode(true)
        let ideas = llm act "Brainstorm solutions"

        // Code generation
        set_temperature(0.3)
        set_reasoning_effort("high")
        set_max_tokens(8000)
        let code = llm act "Generate implementation"
    }
}
```

**Key principle**: Parameters are API fields, not prompt text. `set_temperature(0.3)` does NOT change the prompt content, so prefix cache hit rate is preserved across turns.

### Pattern 6: Tool-Using Agent

**Scenario**: Agent uses tools to accomplish complex tasks

```helen
agent CodeAssistant {
    description "AI coding assistant with file access"
    prompt """
    You are an expert coding assistant. You can:
    - Read and write files
    - Execute shell commands
    - Search the web for documentation

    Always explain your changes before making them.
    """
    tools = ["read_file", "write_file", "patch_file", "shell_exec", "web_search"]
    max-turns 15  // Allow multiple tool call rounds

    main {
        return llm act "Help me implement a REST API in Python"
    }
}
```

### Pattern 7: Conversational Agent

**Scenario**: Multi-turn conversation maintaining conversational context

```helen
agent ConversationalAssistant {
    description "Multi-turn conversational assistant"
    prompt "You are a helpful assistant. Remember context from previous messages."

    main {
        let response = llm act "Remember this context"
        let followup = llm act "Based on what I said before, what do you think?"
        return followup
    }
}

// In the REPL, conversation history is maintained automatically;
// each :ask remembers previous exchanges
```

---

## Advanced Patterns

### Dynamic Agent Selection

```helen
agent DynamicRouter(input: str) {
    description "Dynamically select agent based on input"

    main {
        let decision = llm act "Classify: tech, billing, or general. Input: " + input

        if decision == "tech" {
            return TechSupport(input)
        } else if decision == "billing" {
            return BillingSupport(input)
        } else {
            return GeneralSupport(input)
        }
    }
}
```

### Configuration-Driven Agent (avoid repetitive definitions)

```helen
agent RoleAgent(topic: str, config: map) {
    description "Configurable role Agent"
    prompt """
    You are "{{config["name"]}}". Role: {{config["description"]}}, Style: {{config["style"]}}
    Analyze: {{topic}}
    """
    main { return llm act }
}

let configs = [
    {"name": "Optimist", "description": "Sees the best in everything", "style": "Positive and upbeat"},
    {"name": "Pessimist", "description": "Focuses on risks", "style": "Cautious and conservative"}
]
for config in configs {
    let result = RoleAgent("AI trends", config)
}
```

### Error Handling and Retry

```helen
import std.core.*
import std.time.*
agent RobustAgent(task: str) {
    main {
        let max_retries = 3
        let attempt = 0

        while attempt < max_retries {
            try {
                return llm act task
            } catch LLMError e {
                attempt = attempt + 1
                if attempt >= max_retries {
                    throw RuntimeError("Failed after " + str(max_retries) + " attempts: " + e.message)
                }
                sleep(2)
            }
        }
    }
}
```

#### Agent Call Failure (AgentError)

```helen
try {
    let result = Contractor(req, dir)
} catch AgentError err {
    // err.agent_name — "Contractor"
    // err.agent_args — {req: "...", dir: "..."}
    // err.cause      — underlying exception
    error("Failed: " + err.message)
}
```

`AgentError` inherits from `LLMError` (`catch LLMError` captures both). During nested calls, inner AgentError propagates transparently without double-wrapping.

---

## Best Practices

| # | Practice | ✅ Recommended | ❌ Avoid |
|---|----------|----------------|----------|
| 1 | **description** | `"Review code for bugs, security, and best practices"` | `"Helps with stuff"` |
| 2 | **prompt** | Specific role + steps + output format | `"You analyze things."` |
| 3 | **temperature** | Creative 0.9 / Precise 0.2 / Balanced 0.7 | Always 0.5 |
| 4 | **max-turns** | Simple Q&A 3 / Complex tasks 15 | Unlimited |
| 5 | **max-tokens** | Short 100 / Default 4096 / Long-form 131072 | Omitting when long responses needed |
| 6 | **tools** | Least privilege: `["read_file"]` | `["read_file","write_file","shell_exec","web_search"]` |
| 7 | **model** | Omit (use default) or verify API support | Hardcoding unsupported models |
| 8 | **Scope** | `shared let` for cross-agent value sharing | Expecting module `let` to be visible in agent |
| 9 | **ground truth** | Inject environment facts via `{{}}` | Letting the LLM guess cwd/time |
| 10 | **validation** | Auto-run `helen check` after writing code | Assuming code is correct without validation |

### Writing Helen Code: Always Validate with `helen check`

> **When an agent writes Helen code, it MUST run `helen check` immediately after writing to catch syntax errors.**

This is non-negotiable. Writing code without validation leads to runtime errors that could have been caught at parse time.

**Correct workflow:**

```helen
import std.core.*
import std.io.*
import std.tools.*
agent HelenCoder(task: str) {
    description "Write and validate Helen code"
    tools ["write_file", "shell_exec", "read_file"]
    
    main {
        // Step 1: Write the code
        let code = llm act "Write Helen code for: " + task
        write_file("output.helen", code)
        
        // Step 2: IMMEDIATELY validate with helen check
        let check_result = shell_exec("helen check output.helen")
        
        // Step 3: If errors, fix them
        if check_result["exit_code"] != 0 {
            print("❌ Syntax errors found:")
            print(check_result["output"])
            // Fix errors and re-check
            let fixed_code = llm act "Fix these errors:\n" + check_result["output"] + "\n\nOriginal code:\n" + code
            write_file("output.helen", fixed_code)
            // Re-validate
            let recheck = shell_exec("helen check output.helen")
            if recheck["exit_code"] != 0 {
                throw RuntimeError("Failed to fix syntax errors after retry")
            }
        }
        
        // Step 4: Code is validated, safe to run
        print("✅ Code validated successfully")
        return "Code written and validated: output.helen"
    }
}
```

**Why this matters:**
- `helen check` catches syntax errors, type mismatches, and semantic issues
- It runs in milliseconds, much faster than runtime debugging
- Prevents cascading errors from invalid code
- Provides clear error messages with line numbers

**Tools needed:** `shell_exec` (to run `helen check`), `write_file` (to write code), `read_file` (to read errors)

**Anti-pattern:**
```helen
// ❌ Writing code without validation
import std.io.*
import std.tools.*
main {
    let code = llm act "Write Helen code"
    write_file("output.helen", code)
    // No helen check! Errors only discovered at runtime
    shell_exec("helen output.helen")  // Might fail with cryptic errors
}
```

### Model Selection: Prefer Default, Verify Before Specifying

> **Unless you have a specific reason, don't specify `model` — use the default from `~/.helen/config.yaml`.**

When you do specify a model, **verify it's supported by your current API provider first**. Different providers (OpenAI, Anthropic, DashScope, etc.) support different models. Hardcoding an unsupported model causes runtime errors.

```helen
// ✅ Recommended: Use default model from config
import std.media.*
agent CodeReviewer {
    description "Review code for quality"
    prompt "You are a senior code reviewer."
    // No model specified — uses default from ~/.helen/config.yaml
    main { return llm act "Review this code" }
}

// ✅ Acceptable: Specify model when needed (e.g., vision tasks)
agent ImageAnalyzer {
    description "Analyze images"
    model "qwen3.7-plus"  // Verified: supports vision on DashScope
    main { 
        let img = media("photo.png")
        return llm act "Describe this image" media(img)
    }
}

// ❌ Avoid: Hardcoding models without checking API support
agent BrokenAgent {
    model "gpt-4o"  // Error if using DashScope (doesn't support gpt-4o)
    main { ... }
}
```

**When to specify a model:**
- **Vision tasks**: Need models with image support (e.g., `qwen3.7-plus`, `gpt-4-vision`)
- **Specialized capabilities**: Code generation, reasoning, or domain-specific models
- **Performance tuning**: Faster/cheaper models for simple tasks, more capable for complex ones

**Before specifying, check your provider:**
```bash
# Check supported models (example for DashScope)
curl -H "Authorization: Bearer $API_KEY" \
  https://dashscope.aliyuncs.com/compatible-mode/v1/models
```

### Key Principle: Inject Ground Truth (`{{}}`)

> **An Agent doesn't know what you haven't told it. Runtime facts must be injected — never let the LLM guess.**

The LLM has no access to the current environment (clock, cwd, OS, git branch). When these facts are missing, the model will **confidently fabricate incorrect values**.

Fix: **Resolve facts in Helen, inject them into the prompt via `{{}}`**.

```helen
// ✅ Ground truth injection — LLM sees real values
agent DevAgent(cwd: str) {
    prompt """
    You are a senior engineer in {{cwd}}.
    Time: {{now()}}  OS: {{os_name()}}
    Answer only based on these facts; if not provided, say so.
    """
    main { return llm act "Review the project" }
}

// ❌ Ground truth missing — LLM will fabricate
agent VagueAgent {
    prompt "You are a senior engineer. Help with code."
}
```

**Inject by domain:**

| Domain | Inject via `{{}}` |
|--------|-------------------|
| Programming | `cwd`, `os_name()`, `shell_exec("git branch --show-current")` |
| Scheduling | `now()`, `timezone()` |
| File operations | Directory listings, absolute paths |
| Database | Schema summary, connection target |
| Data analysis | Row count, column names, sample rows |
| Multi-agent pipeline | Upstream output, shared state snapshot |

**Anti-patterns:**
```helen
// ❌ Letting the LLM "assume" environment facts — breaks on drift
prompt "Assume you are in /home/user/project on Linux."

// ❌ Putting dynamic facts in description — fixed at parse time, not runtime
description "Agent for /home/rxx/helen"  // Static! Use prompt + {{}}
```

---

## Debugging Tips

```helen
import std.core.*
import std.debug.*
main {
    trace_on()
    let result = MyAgent("test")
    let trace = get_trace()
    print("Trace: " + str(trace))
    trace_off()
}
```

REPL commands: `:stats` (context statistics), `:transcript` (message log), `:last_error` (last error).

---

## Related Skills

- **helen-agent-collaboration** — Multi-Agent collaboration patterns in detail
- **helen-syntax** — Helen syntax reference (shared let, agent main, etc.)
- **helen-stdlib** — Complete context management API reference (`context_stats`/`compress_context`/`pin_message`/`list_pinned_messages`, etc.)
- **helen-testing** — Agent testing strategies

## Summary

Core principles of Helen Agent design patterns:

1. 🎯 **Caller Decides Context** — all information passed explicitly
2. 🔒 **Scope Isolation** — agent main is isolated by default, adjustable via decorators
3. 📦 **Invocation Isolation** — each agent execution gets an independent LLM context
4. 🛠 **Pattern Selection** — Expert / Router / Pipeline / Concurrent / Streaming / Tool-Using / Conversational
5. 📋 **Best Practices** — clear description, least-privilege tools, inject ground truth

For the context management API (`compress_context`, `working_memory`, `pin_message`, `list_pinned_messages`, and 25+ other functions), see `helen-stdlib`.

---

**Last Updated**: 2026-07-24
**Version**: v1.22
