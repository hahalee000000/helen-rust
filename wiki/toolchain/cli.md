<!-- helen-rust edition: `helen` CLI — crates/helen-rust (M12). All subcommands (run/check/test/quality/repl/doc/init/provider/lsp) verified via Tier B subprocess harness (cli+ffi 128+1skip). -->

# Command-Line Tools (CLI)

> Crate: `crates/helen-rust/src/main.rs` + `cli_utils.rs` | **helen-rust 0.1.0** (ported from Python Helen v1.45.0)

---

## Subcommands

```bash
$ helen <file> [args...]  # Compile + execute (args passed to the program as argv)
$ helen check <file>       # Validate only (Lex + Parse + Analyze)
$ helen repl               # Interactive interpreter
$ helen agent              # Launch the Helen programming assistant (Web UI)
$ helen doc <files...>     # Generate documentation
$ helen init               # Initialize config directory
$ helen lsp                # Start Language Server (LSP)
$ helen test <file>        # Run tests
$ helen coverage <file>    # Run tests with coverage measurement
$ helen quality <file>     # 7-dimension quality assessment
```

---

## helen lsp

```bash
$ helen lsp
```

Starts the Helen Language Server, communicating via JSON-RPC 2.0 over stdin/stdout.

### Usage

- **VS Code integration**: Automatically started after installing the [Helen VS Code extension](vscode.md)
- **Manual testing**: `echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | helen lsp`
- **Custom IDEs**: Provides LSP support for other editors

### Features

| Feature | Description |
|---------|-------------|
| Real-time diagnostics | Instant syntax and semantic error reporting |
| Code completion | Keywords, types, stdlib functions |
| Go to definition | Jump to agent/fn/let declarations |

See also [LSP Documentation](lsp.md) and [VS Code Extension Documentation](vscode.md).

---

## helen init

```bash
$ helen init
Helen home: /home/user/.helen
Skills directory: /home/user/.helen/skills

============================================================
🚀 Helen Setup Wizard
============================================================

Configure your LLM API settings:

API Base URL [https://api.openai.com/v1]: 
Your API key will be masked (input not visible):
API Key: ********
Model [gpt-4]: 

✅ Detected provider: dashscope
✅ Configuration saved to: /home/user/.helen/config.yaml
```

Initializes the Helen standalone config directory `~/.helen/`:

| Created | Description |
|---------|-------------|
| `~/.helen/` | Helen home directory |
| `~/.helen/skills/` | Skill directory |
| `~/.helen/config.yaml` | LLM API config (via interactive wizard) |

If already configured, `helen init` will show the current config path and exit.

### Provider Auto-Detection (v1.40.1)

The setup wizard automatically validates connectivity and detects the provider:

1. **Known provider match**: If `base_url` matches a known provider pattern (DashScope, Volcengine, Zhipu, DeepSeek, Minimax, Kimi, OpenAI), the protocol is saved immediately.
2. **Connectivity probe**: For unknown URLs, a minimal chat completion request is sent to verify connectivity.
3. **Deep probe** (optional): If basic connectivity succeeds but protocol doesn't match, the wizard offers to try known protocol variants (thinking format, streaming format, etc.) to detect the right protocol.
6. **Error classification**: Connection failures, auth errors, and model-not-found errors are reported with bilingual (Chinese/English) messages — config is NOT saved for hard errors.

```
# Example: Known provider → auto-detected
API Base URL: https://dashscope.aliyuncs.com/compatible-mode/v1
✅ Detected provider: dashscope
✅ Configuration saved

# Example: Unknown provider → probe
API Base URL: https://custom-proxy.example.com/v1
⏳ Testing connectivity...
✅ Connectivity OK
✅ Configuration saved

# Example: Connection failure → error
API Base URL: https://bad-url.com/v1
❌ Cannot connect to https://bad-url.com/v1
   (config not saved)

# Example: Protocol mismatch → deep probe option
⚠️ Provider protocol not fully compatible
Deep probe for protocol variants? (y/N): y
⏳ Deep probing...
✅ Detected provider: deepseek
```

### Custom Provider Support (v1.40.1)

For providers not in the built-in list, use `helen agent` to create a custom adapter:

```bash
# Prerequisites: A working Helen environment (configured with any known provider)
$ helen agent
```

Ask the agent to generate a `PlatformProtocol` subclass and save it to `~/.helen/providers/<name>.py`:

```
Please generate a Helen provider adapter for the Anthropic API.
Inherit from OpenAIProtocol and override methods that differ.
Save to ~/.helen/providers/anthropic.py
```

The agent has `web_search`, `web_fetch`, `write_file` and other tools to research the API and generate the adapter interactively.

List installed custom providers:

```bash
$ helen provider list
Installed providers (1):
  • anthropic  (~/.helen/providers/anthropic.py)
```

### Configuration File Format

YAML format (`~/.helen/config.yaml`):

```yaml
llm:
  base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1"
  api_key: "your-api-key-here"
  model: "qwen3.7-plus"
  temperature: 0.7
  timeout: 60
  protocol: "dashscope"           # optional — detected during init (v1.40.1)
  capabilities:                   # optional — detected during deep probe (v1.40.1)
    thinking: true
    streaming: true
    vision: false
```

The `protocol` and `capabilities` fields are optional. When present, they allow `detect_protocol()` to skip URL pattern matching and use the saved protocol directly.

### Environment Variables

You can also use environment variables (these override config.yaml):

```bash
export HELEN_BASE_URL=https://dashscope.aliyuncs.com/compatible-mode/v1
export HELEN_API_KEY=your-api-key-here
export HELEN_MODEL=qwen3.7-plus
```

### Automatic Configuration Check

Helen automatically checks for configuration before running commands:

- **Interactive terminal (TTY)**: If not configured, runs the setup wizard automatically
- **Non-interactive mode**: Shows error message. Use environment variables instead
- **Commands that skip config check**: `--version`, `--help`, `init`, `check`, `doc`, `quality`, `lsp`, `template`

### Skill Directory Priority

| Priority | Directory | Description |
|----------|-----------|-------------|
| 1 (highest) | `~/.helen/skills/` | Helen native |
| 2 | `~/.hermes/skills/` | Hermes fallback |
| 3 | `~/.hermes/hermes-agent/skills/` | Hermes agent |

---

## helen <file>

```
$ helen main.helen
$ helen main.helen --verbose --output=json --port=8080 input.txt
$ helen main.helen --transcript-log=/tmp/my_transcript.jsonl
```

Executes the full compilation pipeline:
1. Lexer → Lexical analysis
2. Parser → Syntax analysis
3. SemanticAnalyzer → Semantic analysis
4. Interpreter → Interpretation and execution

Exit codes: `0` = success, `1` = lexical error, `2` = syntax error, `3` = semantic/runtime error

### Transcript Log (v1.16)

Use the `--transcript-log` option to save conversation records to a specified file:

```bash
$ helen chat.helen --transcript-log=/tmp/chat_session.jsonl
$ helen agent.helen --transcript-log=/var/log/helen/agent.db
```

**Parameter format**:
- `--transcript-log <path>` — Specify transcript output path
- `--transcript-log=<path>` — Equals-sign format also supported

**File types**:
- `.jsonl` extension — Uses JSONL backend (human-readable)
- `.db` extension — Uses SQLite backend (high-performance)

**Use cases**:
- Debugging conversation history
- Exporting session records
- Custom storage location
- Production environment auditing

**Configuration priority**:
1. `--transcript-log` CLI argument (highest)
2. `transcript.session_dir` in `~/.helen/config.yaml`
3. Default `~/.helen/sessions/`

See also [TranscriptStore Documentation](../runtime/transcript-store.md).

### Program Arguments (argv)

All arguments after the filename are passed to the Helen program and can be accessed in three ways:

| Access Method | Type | Description |
|---------------|------|-------------|
| `argv` | `const list<str>` | Predefined constant containing all command-line arguments |
| `get_cli_args()` | `list<str>` | Stdlib function, returns the same list as argv |
| `parse_cli_args(spec?)` | `map` | Structured parsing (supports flags, key=value, positional arguments) |

**Example**:

```bash
$ helen my_tool.helen --verbose --output=json --port=8080 input.txt
```

```helen
// my_tool.helen

// 1. Direct access to argv
print(argv)  // ["--verbose", "--output=json", "--port=8080", "input.txt"]

// 2. Auto-parse
let parsed = parse_cli_args()
// {verbose: true, output: "json", port: "8080", _positional: ["input.txt"]}

// 3. Structured parsing (with types and defaults)
let spec = {
    "verbose": {"type": "flag", "default": false},
    "output": {"type": "string", "default": "text"},
    "port": {"type": "int", "default": 3000}
}
let config = parse_cli_args(spec)
// {verbose: true, output: "json", port: 8080, _positional: ["input.txt"]}
```

> **Note**: `argv` is `const` and cannot be reassigned. It is auto-visible inside agent scope (via the const read-only sharing mechanism).

> **Note**: In nested map literals, `}}` is recognized by the lexer as a template reference closer (`TEMPLATE_CLOSE`). You need to add a space between the two braces: `} }`.

---

## helen check

```
$ helen check main.helen
✓ main.helen: OK
```

Executes frontend validation (no execution):
1. Lexer → Lexical analysis
2. Parser → Syntax analysis
3. SemanticAnalyzer → Semantic analysis

`check` also supports passing program arguments (for validating programs that use `argv`):

```
$ helen check main.helen --verbose --output=json
✓ main.helen: OK
```

Useful for code quality checks in CI/CD.

---

## helen repl

```
$ helen repl
Helen REPL v1.2
Type 'exit' or Ctrl+D to quit, ':help' for commands
In multi-line mode (...), press Enter on empty line or Ctrl+C to cancel

>>> let x = 42
>>> x
42
>>>
```

### Interactive Features

| Feature | Description |
|---------|-------------|
| **Cursor movement** | Arrow keys ← → to move cursor, ↑ ↓ to browse history |
| **Command history** | Input history automatically saved, browse with ↑ ↓ |
| **Tab completion** | Press Tab to trigger completion (e.g., keywords) |

### REPL Commands

```
:help               Show help
:reset              Clear all definitions (functions, agents)
:list               List all defined functions and agents
:undefine <name>    Remove a specific function or agent definition
:ask <question>     Ask the AI assistant (uses LLM to answer Helen language questions)
:trace on|off       Enable/disable execution tracing
:trace show [n]     Show last n trace entries (default 50)
:last_error [-v]    Show structured context of last error (-v shows execution trace)
:llm_log [n] [-v]   Show last n LLM call audit logs (-v shows details)
:stats              Show context window usage statistics (Phase 4)
:transcript         Show current transcript (SSOT effective view)
:transcript --full  Show full transcript (including compressed messages)
:transcript --audit Show compression audit trail
:sessions           List all transcript sessions
:session_id         Show current session ID
:resume <id>        Resume a specific transcript session
exit                Exit REPL
```

> **Note**: Stack traces and execution tracing are enabled by default in the REPL — no need for manual `:trace on`.

#### Transcript Commands (v1.16)

Transcript commands are used to manage and view conversation history:

```
>>> :session_id
Current session: session_1783503886_67a17b79

>>> :sessions
Transcript sessions (3 total):
  [1] session_1783503886_67a17b79
       Modified: 2026-07-08 17:30:00, Size: 2.5 KB, Messages: ~50
  [2] session_1783503800_abc12345
       Modified: 2026-07-08 16:00:00, Size: 1.2 KB, Messages: ~20
  ...

>>> :transcript
Current transcript view (15 messages):
  [1] [user] Hello
  [2] [assistant] Hi there
  ...
Stats: 20 total items, 15 messages, 5 compression boundaries

>>> :transcript --audit
Compression audit (3 events):
  [1] Layer: auto_compact
      UUID: a1b2c3d4e5f6
      Range: abc123..def456
      Anchor: ghi789
      Tokens: 500 -> 100
      Summary: Compressed conversation...
```

**Resuming a session**:
```
>>> :resume session_1783503800_abc12345
Session resumed: session_1783503800_abc12345
Transcript loaded. Use :transcript to view.
```

See also [TranscriptStore Documentation](../runtime/transcript-store.md).

#### :ask — AI Assistant

The `:ask` command launches a built-in Helen language expert Agent that can answer questions about Helen syntax, standard library, usage, etc.:

```
>>> :ask 标准库有哪些字符串函数？
🤔 Thinking...

Helen 标准库提供 36 个字符串函数，包括：
- upper/lower/strip — Case and whitespace handling
- split/join — Splitting and joining
- replace/find — Replacing and finding
- regex_match/regex_replace — Regular expressions
...
```

`:ask` uses the `HelenAssistant` agent (defined in `stdlib/_helen_assistant.helen`), which has:
- Complete Helen language knowledge (syntax, type system, standard library)
- Access to tools such as `read_file`, `write_file`, `web_search`
- Conversation history context (maintained within the same REPL session)

### Multi-Line Input

When brackets are unclosed, the REPL enters multi-line mode (`...` prompt):

```
>>> agent Trans(text) {
...   main {
...     return llm act "translate " + text
...   }
... }
```

**Ways to exit multi-line mode:**

| Method | Description |
|--------|-------------|
| **Empty line** | Press Enter at the `...` prompt (enter an empty line) |
| **Ctrl+C** | Cancel current multi-line input, return to `>>>` prompt |
| **Ctrl+D** | Exit the entire REPL |

### Multi-Line Input Detection

The REPL uses a bracket/quote counting state machine to determine whether to continue input. It tracks:
- Brace count `{}`
- Parenthesis count `()`
- Bracket count `[]`
- String literal state (handles escape sequences)

When any count is non-zero (unclosed delimiters), the `...` prompt is shown waiting for more input.

### Error Formatting

The REPL uses `format_error()` to output structured errors:

```
Error: [E0311] at <repl>:2:5
  2 | let x = y
    |         ^
Undefined variable 'y'
```

---

## helen agent

```bash
$ helen agent
Starting Helen Programming Agent...
Web UI: http://localhost:5173
```

Launches the **Helen Programming Assistant** — an interactive, self-evolving coding agent built entirely in Helen. Unlike `helen repl` (which executes Helen code line by line), `helen agent` starts a *long-lived* agent that edits files, runs tests, checks quality, and learns skills on your behalf.

### What It Does

| Capability | Description |
|---|---|
| Code editing | Read, write, patch files in your project |
| Quality checks | Runs `helen check`, reports scores, suggests fixes |
| Test execution | Runs `pytest` / `helen test`, iterates until green |
| Skill learning | Loads methodology skills on demand; can save new ones |
| Memory | Keeps working memory + long-term memory across turns |
| Persistent transcripts | Every session is recorded in `.helen/sessions/` |

### Architecture

```
Web UI (React + Tailwind)
  ↕ WebSocket
Helen agent backend (Rust, built into helen binary)
  ↕
chat_tui.helen (actor lifecycle)
  ↕ Channel mailbox
ChatSessionActor (long-lived agent, single main {} loop)
  ├── File tools: read_file / write_file / patch_file
  ├── Quality tools: run_helen_check / get_scores / run_helen_tests
  ├── Meta tools: load_skill / save_new_skill / update_memory
  └── Shell tools: shell_exec / web_search / web_fetch
```

The entire agent behavior is defined in `.helen` files. The Rust binary provides the I/O bridge, tool implementations, and Web UI server.

### Startup Sequence

1. The `helen` binary parses the `agent` subcommand
2. Resolves the current working directory, sets environment variables
3. Starts the Web UI server (WebSocket + HTTP)
4. `chat_tui.helen` imports `ChatSessionActor`, spawns it as a long-lived actor
5. The actor's `main {}` loop pulls requests from a Channel mailbox and dispatches to the LLM

### Cross-Platform Support

The agent works on **Windows, macOS, and Linux**:

- **No bash dependency**: Works natively on Windows without Git Bash or WSL
- **Cross-platform path handling**: Uses Rust stdlib path operations
- **stdlib over shell**: Agent code uses `time()`, `date()`, `env_get()`, `delete_file()` etc. instead of Unix shell commands

To enable debug output (hidden by default):

```bash
HELEN_DEBUG=1 helen agent          # Unix
set HELEN_DEBUG=1 && helen agent   # Windows
```

### Session Memento

The active session ID is saved to `.helen/current_session_id` so subsequent `helen agent` runs can resume. Since v1.29.15, the memento is a JSON object:

```json
{"main": "session_...", "child": "session_..."}
```

Older plain-string mementos are also accepted. See [Session Scoping](../runtime/session-scoping.md) for details.

### Web UI

The Web UI is served by the built-in HTTP server. It is started automatically by `helen agent`:

```bash
helen agent    # starts the programming assistant (launches Web UI)
```

Features:
- Real-time streaming of LLM tokens via WebSocket
- Session management (create / switch / delete sessions)
- Slash commands (sent as regular messages starting with `/`)
- Agent status indicator (thinking / tool call / etc.)

### Slash Commands

While the agent is running, you can type slash commands:

| Command | Effect |
|---|---|
| `/help` | Show available commands |
| `/clear` | Clear conversation context (inserts `BoundaryMarker`) |
| `/clear-session [<sid>]` | Delete the entire session (cascades to spawn transcripts) |
| `/compress` | LLM-driven semantic context compression |
| `/stats` | Session statistics (turns, tokens, tool calls) |
| `/memory` | Show long-term memory state |
| `/working-memory` | Show `<working_memory>` block |

### See Also

- [Tutorial 18: Helen Programming Agent](../tutorial/18-helen-agent.md) — full guide
- [Session Scoping](../runtime/session-scoping.md) — how `.helen/` markers work
- [TranscriptStore SSOT](../runtime/transcript-store.md) — persistent sessions

---

## helen coverage

```bash
$ helen coverage test_file.helen [options]
$ helen coverage tests/ --html coverage_html/
$ helen coverage test_math.helen --source math_utils.helen
```

Runs tests and measures code coverage. Reports which functions, lines, and branches were executed during testing.

### Options

| Option | Description |
|--------|-------------|
| `--format <fmt>` | Output format: `text` (default), `json`, or `html` |
| `--output <path>` | Save report to file |
| `--html <dir>` | Generate HTML report in directory |
| `--source <dir>` | Source directory to measure coverage for |

### Coverage Types

| Type | Description |
|------|-------------|
| **Function Coverage** | Which functions were called during tests |
| **Line Coverage** | Which code lines were executed |
| **Branch Coverage** | Which if/else branches were taken |

### Example Output

```
============================================================
HELEN COVERAGE REPORT
============================================================

  Lines:     22/46  (47.8%)
  Functions: 7/7  (100.0%)
  Branches:  6/6  (100.0%)

Files:
  File                                          Lines      Funcs
  ---------------------------------------- ---------- ----------
  calculator.helen                            15/20      3/4    
  calculator_test.helen                       7/7        4/4    

============================================================
```

### Programmatic Coverage Control

Coverage can also be controlled programmatically in Helen code:

```helen
main {
    coverage_on()           // Enable coverage tracking
    // ... run code ...
    let summary = coverage_summary()
    let report = coverage_report("text")
    coverage_off()          // Disable coverage tracking
}
```

### Design Features

- **Zero overhead by default**: No performance impact when not enabled
- **Minimal logging**: Only records file/line/function names, never values
- **Resource-bounded**: 1M counter limit prevents memory exhaustion
- **Thread-safe**: Uses locks for counter updates

---

## helen doc

```
$ helen doc main.helen
# Helen Program Documentation

## Agents

### Translator
- **Description**: Translate text between languages
- **Model**: gpt-4
- **Parameters**: text (str)

## Functions
...

## Built-in Functions
...
```

Supports `--format markdown|json` and `-o output_file`.

---

## Error Formatter

Follows HLD 3.11.2 format:

```
Error: [E0301] at main.helen:5:10
  5 | let x = "hello
    |           ^^^^^
Unterminated string

Code: E0301 — UNTERMINATED_STRING
```

Output includes:
1. Error header: `Error: [E{code}] at {file}:{line}:{col}`
2. Source code line
3. Caret indicator `^^^^`
4. Error message
5. Error code description
