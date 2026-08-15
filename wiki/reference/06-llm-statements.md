# Tutorial 06: LLM Statements

> llm act / llm if in practice

---

## LLM Statements Overview

Helen has two keyword-level LLM statements:

| Statement | Purpose | Return Value |
|-----------|---------|--------------|
| `llm act` | Have the LLM execute a task (supports optional streaming callbacks) | Response text |
| `llm if` | Have the LLM classify and route | Executes the matched branch or returns a value |

---

## llm act

### Basic Usage

`llm act` is used to directly call the LLM with a prompt string:

```helen
import std.core.*
main {
    let result = llm act "Translate 'Hello, world!' to French"
    print(result)
    // Bonjour, le monde!
}
```

### Using Inside an Agent

In an agent's `main` block, `llm act` automatically uses the agent's configuration (model, temperature, etc.):

```helen
import std.core.*
agent Translator(text: str, target: str) {
    description "Translate text"
    model "qwen-plus"
    temperature 0.3
    prompt """
    Translate to {{target}}:
    {{text}}
    """

    main {
        // bare form: automatically uses the rendered prompt
        let result = llm act
        return result
    }
}

main {
    let translated = Translator(text="Hello", target="French")
    print(translated)
}
```

### Runtime Parameter Adjustment (v1.39.8)

Agent declarations set default values for operational parameters. Use stdlib functions to override them at runtime without changing the prompt (preserves prompt cache):

```helen
import std.core.*
import std.llm.*

agent AdaptiveAgent {
    temperature 0.5
    max-turns 1

    main {
        // Simple task: low temperature, few turns
        set_temperature(0.2)
        set_max_turns(1)
        llm act "Quick answer"

        // Complex task: high temperature, deep reasoning
        set_temperature(0.8)
        set_max_turns(10)
        set_thinking_mode(true)
        set_reasoning_effort("high")
        set_max_tokens(8000)
        llm act "Deep analysis"
    }
}
```

**Writable parameters** (5 set/get pairs):
- `set_temperature(t)` / `get_temperature()` — output randomness (0.0-2.0)
- `set_max_turns(n)` / `get_max_turns()` — max tool-calling rounds
- `set_max_tokens(n)` / `get_max_tokens()` — max output tokens
- `set_thinking_mode(bool)` / `get_thinking_mode()` — enable reasoning mode
- `set_reasoning_effort(str)` / `get_reasoning_effort()` — "low" / "medium" / "high"

**Read-only identity parameters** (3 get):
- `get_model()` / `get_description()` / `get_provider()` — set at declaration time

**Priority**: `set_*()` override > agent declaration > built-in default.

**Design rationale**: Parameters are passed as API fields, not embedded in the prompt. This preserves prompt prefix cache hit rate across turns — changing `temperature` via `set_temperature()` does not invalidate the prompt cache, while embedding it in the prompt text would.

### With Dynamic Prompts

You can pass expressions after `llm act` to dynamically build prompts:

```helen
import std.core.*
main {
    let review = "This product is amazing!"
    let result = llm act "Analyze sentiment of: " + review
    print(result)
}
```

---

`llm act` can also be used directly as an expression, without agent context:

```helen
fn translate(text, target) {
    return llm act "translate " + text + " to " + target
}

main {
    // Direct call
    llm act "translate hello to chinese."

    // Assign to a variable
    let result = llm act "summarize this article"

    // String concatenation to build prompts
    let topic = "climate change"
    let analysis = llm act "analyze the impact of " + topic
}
```

**Syntax comparison:**

| Form | Syntax | Purpose |
|------|--------|---------|
| Expression form | `llm act <expr>` | Directly call the LLM; the expr value is used as the prompt |
| Bare form | `llm act` | Omit arguments in agent main; automatically uses the rendered prompt |

**Note:** The statement form `llm act Agent(args) "desc"` is deprecated; use `Agent(args)` to call agents instead.

**When to use the expression form:**
- Quick prototyping without defining an agent
- Dynamically building prompts
- Directly calling the LLM from the REPL
- Simple LLM call scenarios

---

## llm if

### Basic Usage

```helen
import std.core.*
main {
    llm if "Classify email priority" {
        branch "urgent" {
            print("🚨 URGENT — notify on-call immediately")
        }
        branch "high" {
            print("🔴 HIGH — address within 1 hour")
        }
        branch "normal" {
            print("🟢 NORMAL — handle in next sprint")
        }
        branch "low" {
            print("⚪ LOW — handle when convenient")
        }
        default {
            print("❓ Unknown priority")
        }
    }
}
```

**Note**: `llm if` uses the `branch` keyword to define branches, not `case`. Each branch is followed by a `{ }` block.

### Nested Usage

```helen
import std.core.*
main {
    let query = "How do I reset my password?"

    llm if "Classify query type" {
        branch "question" {
            llm if "Identify question category" {
                branch "technical" {
                    TechSupport(query)
                }
                branch "billing" {
                    BillingSupport(query)
                }
                default {
                    GeneralSupport(query)
                }
            }
        }
        branch "command" {
            execute_command(query)
        }
        default {
            print("I don't understand")
        }
    }
}
```

---

## llm act Streaming Output (on_chunk / on_complete)

`llm act` supports optional `on_chunk` and `on_complete` callbacks for streaming LLM responses chunk by chunk, useful for long text generation scenarios.

### Basic Usage

Use `on_chunk` to specify a callback function for custom processing of each chunk:

```helen
import std.io.*
fn handle_chunk(chunk) {
    stream_print("[" + chunk + "]")
}

main {
    llm act "Explain recursion in one paragraph" on_chunk handle_chunk
}
```

Use `on_complete` to specify a callback after streaming is finished:

```helen
import std.core.*
fn handle_chunk(chunk) {
    print(chunk, end="")
}

fn on_done() {
    print("\n\n✅ Streaming complete")
}

main {
    llm act "Write a short story" on_chunk handle_chunk on_complete on_done
}
```

The `on_complete` callback is called after streaming finishes, suitable for:
- Displaying completion notifications
- Logging statistics (e.g., total token count)
- Triggering follow-up actions

### Using Inside an Agent

Streaming callbacks for `llm act` automatically use the agent's configuration (model, temperature, prompt) when inside an agent:

```helen
import std.io.*
agent Poet(topic: str) {
    description "Write poetry"
    temperature 0.9
    prompt """
    Write a poem about: {{topic}}
    """

    main {
        fn print_chunk(chunk: str) { stream_print(chunk) }
        llm act on_chunk print_chunk    // bare form: uses the rendered prompt
    }
}
```

### Dynamic Prompts

```helen
import std.io.*
fn print_chunk(chunk: str) {
    stream_print(chunk)
}

main {
    let topic = "the beauty of recursion"
    llm act "Write a haiku about " + topic on_chunk print_chunk
}
```

### Comparison with Other LLM Statements

| Statement | Purpose | Output Method |
|-----------|---------|---------------|
| `llm act` | Get the complete response text (optional streaming callbacks) | Waits for completion then returns, or streams chunk by chunk via on_chunk |
| `llm if` | LLM classification and routing | Waits for completion then executes the matched branch |

### Tool Execution Callback (on_tool_end)

`llm act` supports an `on_tool_end` callback that is called after each tool execution. The callback can return a string or dict, which is injected as a hint into the conversation history for the LLM to see on the next generation. This is very useful for guiding the LLM's direction in the middle of an agentic loop.

**Callback signature**: `fn(tool_name: str, tool_result: str): str | dict | null`

- Returns a string → Automatically injected as a `user` message with a `[System Hint]` prefix
- Returns a dict → `{"role": "user"|"system", "content": "..."}` for full control over message format
- Returns null → Nothing is injected

**Persistence**: All injected hints are automatically saved to TranscriptStore and can be viewed via the REPL's `:transcript` command, supporting session replay and auditing.

```helen
import std.io.*
agent Coder {
    tools ["write_file", "shell_exec", "read_file"]

    main {
        llm act "Create hello.py, then run it"
            on_chunk fn(c) { stream_print(c) }
            on_tool_end fn(name, result) {
                if name == "write_file" {
                    return "File written; next step is to run tests to verify"
                }
                if name == "shell_exec" {
                    return {"role": "system", "content": "Warning: do not execute dangerous commands"}
                }
                return null
            }
    }
}
```

Using Chinese aliases:

```helen
main {
    llm act "Create hello.py" 工具结束 fn(name, result) {
        return "hint content"
    }
}
```

**Typical use cases**:
- Provide next-step suggestions after tool execution to guide the LLM's direction
- Security auditing: inject security warnings after shell_exec
- External state synchronization: query external queues and inject new information into the conversation
- Progress tracking: update TODO lists after file operations

---

## Comparison: When to Use Which?

| Scenario | Use |
|----------|-----|
| Need the LLM to return text | `llm act` |
| Need the LLM to make a classification decision | `llm if` |
| Need the LLM to choose from options and execute code | `llm if` + `branch` |
| Need real-time output of the generation process | `llm act` + `on_chunk` callback |
| Need to guide the LLM after tool execution | `llm act` + `on_tool_end` callback |
| Multi-step decision | Nested `llm if` |
| Need a result variable | `llm if` or `llm act` |

---

## Automatic Conversation History Recording

Every LLM interaction is automatically recorded in the conversation history:

```helen
import std.core.*
main {
    // Automatically recorded: [user] "Classify email priority"
    llm if "Classify email priority" {
        branch "urgent" { print("Urgent!") }
        default { print("Other") }
    }
    // Automatically recorded: [assistant] "[routed to: urgent]"

    // The next LLM call will include the above history as context
    llm act "Draft response for the email"
}
```

### Context Window Protection

The conversation history is automatically trimmed before being passed to the LLM; you don't need to manually manage context length:

- **Automatic trimming**: Before each LLM call, the oldest messages are automatically deleted based on the context window size
- **Automatic compression**: When history is too long, old messages are compressed into summaries
- **Tool result cap**: There is a limit on the number of results per tool loop to avoid context explosion
- **Context overflow recovery**: When the API returns a context-too-large error, it automatically retries

---

## LLM Calls in the REPL

In the REPL, `llm act` expressions call a real LLM (via the HTTP API):

```bash
$ helen repl
>>> llm act "translate hello to chinese"
'hello → 你好 (nǐ hǎo)'
>>> let result = llm act "what is 2+2?"
>>> result
'4'
```

**Notes:**
- Both REPL and script modes call the LLM API directly
- Response time: 7-11 seconds (depending on network and model)
- Automatically reads configuration from `~/.helen/config.yaml` or environment variables (`HELEN_API_KEY`, `HELEN_BASE_URL`, `HELEN_MODEL`)
- If not configured, Helen will prompt you with an interactive setup wizard

**Configuration:**
Ensure `~/.helen/config.yaml` contains:
```yaml
llm:
  base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1"
  api_key: "your-api-key-here"
  model: "qwen3.7-plus"
```

Or use environment variables:
```bash
export HELEN_API_KEY=***
export HELEN_BASE_URL=https://dashscope.aliyuncs.com/compatible-mode/v1
export HELEN_MODEL=qwen3.7-plus
```

---

## Function Calling (Tool Calls)

When an agent is configured with `tools = [...]`, `llm act` automatically enters the function calling loop:

```helen
agent Researcher(topic) {
    description "Research assistant"
    tools = ["web_search", "read_file"]
    main {
        return llm act "Research about: " + topic
    }
}
```

**Execution flow:**

1. The LLM receives the prompt + tool schemas
2. The LLM returns a tool call request → Helen executes the tool → results are returned to the LLM
3. The loop continues until the LLM outputs a final text response
4. When `max_turns - 1` is reached, a nudge prompt is automatically injected to force the LLM to produce a final answer

**Built-in tool list (10):**

| Tool | Function | Parameters |
|------|----------|------------|
| `web_search` | Search the web (Bing) | `query: str` |
| `web_fetch` | Fetch web page content | `url: str` |
| `read_file` | Read a file | `path: str` |
| `write_file` | Write to a file (overwrite) | `path: str, content: str` |
| `patch_file` | Precisely modify files (automatically handles whitespace/indentation differences) | `path: str, old_string: str, new_string: str` |
| `shell_exec` | Execute shell commands | `command: str` |
| `calculate` | Math calculations | `expression: str` |
| `find_files` | Find files by glob pattern | `path: str, pattern: str = "**/*", max_results: int = 200` |
| `search_files` | Search files by content (text/regex) | `path: str, pattern: str, regex: bool = false, case_sensitive: bool = true, max_results: int = 100` |
| `load_skill` | Load skill documentation (always available) | `name: str, include_references: bool = false` |
| `list_skill_references` | List skill reference documents | `name: str` |

### patch_file Fuzzy Matching

`patch_file` uses the `old_string` → `new_string` pattern to precisely modify files, with multiple built-in matching strategies to handle common differences in LLM-generated code (whitespace, indentation, escaping, Unicode, etc.):

```helen
main {
    // Modify a specific function in a file
    llm act "Read /tmp/main.py and change the function name from 'foo' to 'bar'"
}
```

Usually you don't need to worry about matching details — even if the LLM-generated code has subtle differences from the original, `patch_file` can handle it correctly.

---

## Agent prompt vs system_prompt

The agent's `prompt` field is injected as **system_prompt** in LLM calls when using `llm act`:

```helen
agent Translator(text) {
    description "Professional translator"
    prompt """
    Translate the following text to {{target}}:
    {{text}}
    """
    main {
        // The rendered prompt → system_prompt
        // "Translate the following text to French:\nHello"
        // → Injected as {"role": "system"}
        return llm act "Please translate accurately"
        // → Injected as {"role": "user"}
    }
}
```

**Message structure:**
```json
[
  {"role": "system", "content": "<description>\n<skills>\n<rendered prompt>"},
  {"role": "user", "content": "llm act expression value"}
]
```

---

## Exercises

1. Create a three-level nested classification system using llm if
2. Use llm if to have the LLM select an algorithm strategy and return the result
3. Use llm act to implement a translation pipeline
4. Observe the conversation history after multiple LLM calls
