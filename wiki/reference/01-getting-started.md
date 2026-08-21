# Tutorial 01: Getting Started

> Install Helen, configure the environment, write your first program, use the REPL

---

## System Requirements

- **Rust toolchain** (for building from source) or pre-built binary
- OS: Linux, macOS, Windows
- Disk space: ~50 MB binary + ~100 MB (including 17 built-in skills)

---

## Installation

### Method 1: Cargo Install (Recommended)

```bash
# Install from crates.io
cargo install helen-rust

# Verify
helen --version
# helen-rust 0.1.0 — Helen Agent Programming Language (Rust)
> Ported from Python Helen v1.45.0

helen --help
```

### Method 2: Installer Script

```bash
# Download and run the installer
curl -fsSL https://raw.githubusercontent.com/hahalee000000/helen-rust/main/scripts/install.sh | bash

# Or clone and install locally
git clone https://github.com/hahalee000000/helen-rust.git
cd helen-rust
bash scripts/install.sh
```

### Method 3: Build from Source (Developers)

```bash
# Clone the repository
git clone https://github.com/hahalee000000/helen-rust.git
cd helen-rust

# Build release binary
cargo build --release

# Binary is at target/release/helen
./target/release/helen --version

# Optional: install to ~/.cargo/bin
bash scripts/install.sh --from-build

# Run workspace tests (verify the installation is correct)
cargo test --workspace
```

### Method 4: Python Bridge (Optional)

```bash
# Install the Python bridge wheel (for Python → Helen integration)
pip install helen-rust

# Use Helen agents from Python
python3 -c "import helen_rust; from translator import TranslatorAgent"
```

### Post-Installation Configuration

Helen automatically checks for LLM configuration on first run. If not configured, it will prompt you with an interactive setup wizard that validates connectivity and detects the provider:

```bash
$ helen
⚠️  Helen is not configured

============================================================
🚀 Helen Setup Wizard
============================================================

Configure your LLM API settings:

API Base URL [https://api.openai.com/v1]: https://dashscope.aliyuncs.com/compatible-mode/v1

Your API key will be masked (input not visible):
API Key: ********

Model [gpt-4]: qwen3.7-plus

✅ Detected provider: dashscope
✅ Configuration saved to: /home/user/.helen/config.yaml

You can now run Helen programs:
  helen <file.helen>
```

The wizard automatically:
1. **Matches known providers** by URL pattern (DashScope, Volcengine, Zhipu, DeepSeek, Minimax, Kimi, OpenAI)
2. **Probes connectivity** for unknown URLs (sends a minimal API request)
3. **Classifies errors** — connection failures, invalid API keys, and unknown models are reported separately (config is NOT saved for hard errors)
4. **Offers deep probe** for protocol variant detection when basic connectivity succeeds but protocol doesn't match standard OpenAI format

Alternatively, you can run the setup wizard explicitly:

```bash
$ helen init
Helen home: /home/user/.helen
Skills directory: /home/user/.helen/skills

[Interactive wizard with auto-detection starts...]
```

Or configure manually by editing `~/.helen/config.yaml` directly.

### Custom Provider Support (v1.40.1)

If your provider is not in the built-in list and connectivity probing fails to match a known protocol, use `helen agent` to create a custom adapter:

1. First configure Helen with any supported provider (e.g., DashScope or DeepSeek)
2. Run `helen agent` and ask the agent to generate a `PlatformProtocol` subclass
3. The agent will save the adapter to `~/.helen/providers/<name>.py`

```bash
$ helen agent
# Example prompt:
# "Please generate a Helen provider adapter for the Anthropic API.
#  Inherit from OpenAIProtocol and override methods that differ.
#  Save to ~/.helen/providers/anthropic.py"
```

### Directory Structure

```
~/.helen/
├── config.yaml    # LLM API configuration
├── skills/        # Helen native skill directory
└── providers/     # Custom provider adapters (v1.40.1)
```

### Configuration File

Edit `~/.helen/config.yaml`:

```yaml
# Helen configuration

llm:
  base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1"
  api_key: "your-api-key-here"
  model: "qwen3.7-plus"
  temperature: 0.7
  timeout: 60
  protocol: "dashscope"           # optional — auto-detected during init (v1.40.1)
  capabilities:                   # optional — detected via deep probe (v1.40.1)
    thinking: true
    streaming: true

transcript:
  enabled: true
  backend: "sqlite"
  session_scope: "auto"                  # "auto" | "global" | "project"
  max_memory_items: 1000
```

### Environment Variables

You can also configure Helen using environment variables (these override config.yaml):

```bash
export HELEN_BASE_URL=https://dashscope.aliyuncs.com/compatible-mode/v1
export HELEN_API_KEY=your-api-key-here
export HELEN_MODEL=qwen3.7-plus
```

Environment variables are useful for CI/CD pipelines and temporary overrides.

### Configuration Behavior

Helen checks for configuration before running commands that require LLM access:

- **TTY mode** (interactive terminal): If not configured, automatically runs the setup wizard
- **Non-TTY mode** (scripts, CI/CD): Shows an error and exits. Set `HELEN_API_KEY` environment variable instead
- **Commands that skip config check**: `--version`, `--help`, `init`, `check`, `doc`, `quality`, `lsp`, `template`

### Skill Directory Priority

| Priority | Directory | Description |
|----------|-----------|-------------|
| 1 (highest) | `~/.helen/skills/` | Helen native skills |
| 2 | `~/.hermes/skills/` | Hermes fallback |
| 3 | `~/.hermes/hermes-agent/skills/` | Hermes agent skills |

### Transcript Configuration (v1.16)

Helen v1.16 introduced TranscriptStore, which automatically saves all conversation history. Enabled by default, no configuration needed:

```yaml
# ~/.helen/config.yaml

llm:
  base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1"
  api_key: "your-api-key-here"
  model: "qwen3.7-plus"

# Transcript configuration (optional; defaults shown below)
transcript:
  enabled: true              # Enable session recording (default: true)
  backend: "jsonl"           # Backend type: "jsonl" or "sqlite"
  session_dir: "~/.helen/sessions"  # Session storage directory
```

**Default behavior**:
- Session recording is enabled by default; all conversations are automatically saved to `~/.helen/sessions/`
- Uses the JSONL backend (human-readable, crash-safe)

**Custom configuration**:
- Set `enabled: false` to disable session recording
- Set `backend: "sqlite"` to use the SQLite backend (suitable for large sessions)
- Set `session_dir` to customize the storage location

**CLI arguments**:
```bash
# Custom transcript output path
$ helen chat.helen --transcript-log=/tmp/my_chat.jsonl
```

**REPL commands**:
```
>>> :sessions              # List all sessions
>>> :session_id            # Show current session ID
>>> :transcript            # Show current transcript
>>> :resume <session_id>   # Resume a specific session
```

See [TranscriptStore documentation](../runtime/transcript-store.md) and [stdlib reference](10-stdlib.md#transcript-functions-6-v116).

---

## Hello, World!

Create `hello.helen`:

```helen
import std.core.*
main {
    print("Hello, World!")
}
```

Run:

```bash
$ helen hello.helen
Hello, World!
```

### Passing Command-Line Arguments

Arguments after the filename are passed to the Helen program, accessed via the predefined constant `argv`:

```bash
$ helen greet.helen Alice Bob
```

```helen
// greet.helen
import std.core.*
main {
    for name in argv {
        print("Hello, " + name + "!")
    }
}
```

```
Hello, Alice!
Hello, Bob!
```

You can also use `parse_cli_args()` for structured parsing:

```helen
// tool.helen — Run: helen tool.helen --verbose --output=json file.txt
import std.core.*
import std.system.*
main {
    let config = parse_cli_args({
        "verbose": {"type": "flag", "default": false},
        "output": {"type": "string", "default": "text"}
    })

    if config["verbose"] {
        print("Verbose mode on, output=" + config["output"])
    }
}
```

See [[toolchain/cli|CLI documentation]] and [[toolchain/stdlib|stdlib System section]].

---

## VS Code Integration

Helen provides full VS Code support including syntax highlighting, code completion, and real-time error checking.

### Step 1: Install the Extension

**Method A: Install from VSIX (Recommended)**

```bash
cd ~/helen-rust/extensions/vscode
npm install
npm run compile
npx vsce package
```

Then in VS Code:
1. Press `Ctrl+Shift+P` to open the command palette
2. Type `Extensions: Install from VSIX...`
3. Select the generated `.vsix` file

**Method B: Development Mode**

```bash
# Copy the extension directory to the VS Code extensions directory
cp -r extensions/vscode ~/.vscode/extensions/helen-language
```

### Step 2: Ensure Helen Is Installed

```bash
# Confirm the helen command is available
which helen
helen help
```

### Step 3: Start Using

1. **Open a .helen file** — Syntax highlighting is applied automatically
2. **LSP starts automatically** — Providing real-time error checking, code completion, and go-to-definition

### Feature Overview

| Feature | How |
|---------|-----|
| Syntax highlighting | Automatic |
| Real-time diagnostics | Automatic (red/yellow squiggly lines) |
| Code completion | Pops up automatically while typing, or press `Ctrl+Space` |
| Go to definition | `Ctrl+Click` or `F12` |
| Restart LSP | `Ctrl+Shift+P` → `Helen: Restart Language Server` |

### Quick Test

Create `test.helen`:

```helen
import std.core.*
fn greet(name: string): string {
    return "Hello, " + name + "!"
}

main {
    let msg = greet("Helen")
    print(msg)
}
```

After opening, you should see:
- ✅ Keyword highlighting (`fn`, `let`, `main`, `return`)
- ✅ Type highlighting (`string`)
- ✅ Function name highlighting (`greet`, `print`)
- ✅ Completion pops up when typing `pri` (`print`)
- ✅ `Ctrl+Click` on a function name jumps to its definition

### Configuration Options

In VS Code settings (`Ctrl+,`), search for `helen`:

| Setting | Description | Default |
|---------|-------------|---------|
| `helen.lsp.path` | LSP server path | `"helen"` |
| `helen.lsp.args` | LSP arguments | `["lsp"]` |
| `helen.lsp.enabled` | Enable/disable LSP | `true` |

If helen is not in PATH, configure a custom path:

```json
{
  "helen.lsp.path": "/home/user/.local/bin/helen"
}
```

### Troubleshooting

**LSP not starting?**

1. Check the VS Code Output panel: `View` → `Output` → select `Helen Language Server`
2. Confirm helen is available: `which helen`
3. If the path is wrong, modify the `helen.lsp.path` setting
4. Restart the LSP: `Ctrl+Shift+P` → `Helen: Restart Language Server`

**Syntax highlighting not working?**

1. Make sure the file extension is `.helen`
2. Check the language mode in the bottom-right corner and set it manually to "Helen"

See [VS Code Extension documentation](../toolchain/vscode.md).

---

## Code Validation

Check syntax and semantics without executing:

```bash
$ helen check hello.helen
✓ hello.helen: OK
```

If there are errors:

```bash
$ helen check broken.helen
Error: [E0311] at broken.helen:2:9
  2 | let x = y
    |         ^
Undefined variable 'y'

Code: E0311 — UNDEFINED_VARIABLE

1 error found.
```

---

## Using the REPL

```bash
$ helen repl
Helen REPL v1.2
Type 'exit' or Ctrl+D to quit, ':help' for commands

>>> print("Hello!")
Hello!
>>> let x = 42
>>> x
42
>>> let y = x * 2
>>> y
84
>>>
```

### Interactive Features

The REPL supports the following interactive features:

| Feature | Description |
|---------|-------------|
| **Cursor movement** | ← → arrow keys move the cursor |
| **Command history** | ↑ ↓ arrow keys to browse history |
| **Tab completion** | Press Tab to trigger keyword completion |

### REPL Commands

```
:help             Show help
:reset            Clear all definitions
:list             List defined functions and agents
:undefine <name>  Remove a specific definition
:ask <question>   Ask the Helen language assistant
exit              Exit the REPL
```

### Helen Language Assistant

The REPL includes a built-in AI language assistant (located at `helen/agent/helen_assistant.helen`) that can answer Helen language questions, help write code, and debug programs.

The assistant loads:
- **Helen documentation** (`wiki/reference/` and `wiki/syntax/`) — syntax, semantics, examples
- **Helen source code** (`helen/` directory) — parser, interpreter, AST, lexer

This means the assistant can not only answer syntax questions but also explain implementation details and internal mechanisms.

Use the `:ask` command to ask questions:

```
>>> :ask How do I define an agent?

🤔 Thinking...

# Defining an Agent in Helen

An `agent` is a first-class language construct...
[Detailed answer and code examples]
```

**Streaming output**: The assistant uses `llm act` (with `on_chunk` callback) to stream answers in real time, displaying content chunk by chunk without waiting for the complete response.

The assistant loads Helen documentation and generates detailed answers with code examples.

### Multi-line Input

When brackets are not closed, the REPL enters multi-line mode (`...` prompt):

```
>>> for i in [1, 2, 3] {
...     print(i)
... }
1
2
3
```

**Ways to exit multi-line mode:**

| Method | Description |
|--------|-------------|
| **Empty line** | Press Enter at the `...` prompt (enter a blank line) |
| **Ctrl+C** | Cancel the current multi-line input |
| **Ctrl+D** | Exit the entire REPL |

Example: If you make a typo and get stuck in multi-line mode:

```
>>> agent Bad(x) {
...   main {
...     return x *
... 
(multi-line input cancelled)
>>>
```

### Exiting the REPL

Press `Ctrl+D` or type `exit`.

---

## Generating Documentation

```bash
$ helen doc hello.helen
# Helen Program Documentation

## Agents

(No agents defined)

## Functions

(No functions defined)

## Built-in Functions
- print(*args) → str — Print values
- len(value) → int — Length
...
```

JSON output:

```bash
$ helen doc hello.helen --format json
{"agents": [], "functions": [], "builtins": [...]}
```

---

## Bilingual Programming (v1.9)

Helen supports both English and Chinese keywords. Chinese keywords map to the same TokenType as English ones, and you can freely mix them.

```helen
// Pure Chinese Hello World
import std.core.*
主函 {
    print("你好，世界！")
}

// Chinese variables and functions
函数 打招呼(名字: str) {
    print("你好, " + 名字)
}

main {
    let 姓名 = "张三"
    打招呼(姓名)
}

// Mixed Chinese and English
main {
    let 年龄 = 30
    如果 年龄 >= 18 {
        print("成年")
    } 否则 {
        print("未成年")
    }
}
```

**Chinese keyword mapping**: `定义`=let, `函数`=fn, `如果`=if, `否则`=else, `返回`=return, `真`=true, `假`=false, `空`=null, etc. (44 keywords). See [[syntax/keywords|Keyword Reference]].

---

## Exercises

1. Create a Helen program that prints your name (try using Chinese keywords)
2. Calculate `1 + 2 * 3` in the REPL
3. Deliberately write a program with a syntax error and observe the error output
