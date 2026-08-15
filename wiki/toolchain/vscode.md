# VS Code Extension

> Module M13 | `extensions/vscode/` | Tests: `tests/extension/test_vscode_extension.py`

---

## Overview

The Helen VS Code extension provides full IDE support:
- 🎨 Syntax highlighting
- 🔍 Language Server Protocol (LSP) integration
- ⚡ Real-time diagnostics
- 💡 Code completion
- 🚀 Go to definition

---

## File Structure

```
extensions/vscode/
├── package.json                      # Extension manifest
├── language-configuration.json       # Language configuration
├── tsconfig.json                     # TypeScript configuration
├── src/
│   └── extension.ts                  # LSP client entry point
└── syntaxes/
    └── helen.tmLanguage.json        # TextMate grammar
```

---

## Installation

### Prerequisites

1. **Install Helen**:
```bash
# Recommended: Install from PyPI
pip install helen-lang

# Or install from source (developers)
git clone https://github.com/hahalee000000/helen.git
cd helen
pip install -e .
```

2. **Verify installation**:
```bash
helen --version   # Helen 1.20.0
helen lsp         # Should start the LSP server (press Ctrl+C to exit)
```

### Install Extension from Source

```bash
cd extensions/vscode
npm install
npm run compile
npx vsce package
# Install the generated .vsix file in VS Code
```

### Install from Source Directory (Development Mode)

```bash
# Copy the extension directory to the VS Code extensions directory
# Linux/macOS:
cp -r extensions/vscode ~/.vscode/extensions/helen-language

# Windows:
xcopy /E /I extensions/vscode %USERPROFILE%\.vscode\extensions\helen-language
```

---

## Features

### Syntax Highlighting

Automatic syntax highlighting for `.helen` files:

| Scope | Matches | Color Theme Suggestion |
|---|---|---|
| `keyword.control.helen` | if/else/for/while/break/continue/return/match/case/default/try/catch/finally/in/as | Keyword color |
| `keyword.declaration.helen` | let/const/fn/agent/protocol/impl/functions/main | Declaration color |
| `keyword.other.helen` | import/as/call/await/async/llm/act/stream/is | Special keyword color |
| `keyword.agent.property.helen` | description/model/tools/sub-agents/temperature/max-turns/streaming/prompt/memory | Modifier color |
| `support.type.helen` | string/int/float/bool/list/dict/any/number/void | Type color |
| `string.quoted.double.helen` | `"..."` | String color |
| `comment.line.double-slash.helen` | `// ...` | Comment color |
| `comment.block.helen` | `/* ... */` | Block comment color |
| `constant.language.boolean.helen` | true/false | Boolean color |
| `constant.language.null.helen` | null | Null color |
| `constant.numeric.integer.helen` | `42` | Number color |
| `constant.numeric.float.helen` | `3.14` | Float color |
| `keyword.operator.*.helen` | `+ - * / % == != > < >= <= && \|\| ! = ..` | Operator color |
| `entity.name.function.helen` | `fn_name(` | Function name color |
| `entity.name.type.agent.helen` | `agent Name` | Agent name color |

### Language Server (LSP)

#### Real-Time Diagnostics

- Syntax errors shown instantly
- Semantic error checking
- Type error reporting

#### Code Completion

- Keyword completion (40+ keywords)
- Type completion (string, int, float, bool, list, dict, any)
- Stdlib function completion (print, len, str_upper, regex_match, etc.)

#### Go to Definition

Supports jumping to:
- Agent declarations
- Function declarations
- Variable declarations (let/const)

Use `Ctrl+Click` (or `Cmd+Click` on macOS) to jump to definition.

---

## Configuration

### Extension Settings

In VS Code settings (`Ctrl+,`), search for `helen`:

| Setting | Description | Default |
|---------|-------------|---------|
| `helen.lsp.path` | LSP server executable path | `"helen"` |
| `helen.lsp.args` | LSP server arguments | `["lsp"]` |
| `helen.lsp.enabled` | Enable/disable Language Server | `true` |

### Example Configuration

If Helen is installed in a custom location:

```json
{
  "helen.lsp.path": "/home/user/helen/venv/bin/helen",
  "helen.lsp.args": ["lsp"]
}
```

---

## Commands

### Helen: Restart Language Server

Restarts the Language Server.

**Usage**:
1. Press `Ctrl+Shift+P` to open the Command Palette
2. Type `Helen: Restart Language Server`
3. Press Enter

---

## Status Bar

The extension shows a "Helen" indicator on the right side of the status bar:
- Click to restart the Language Server
- Hover to show "Helen Language Server"

---

## Language Configuration

### Bracket Pairing

```json
"brackets": [["{", "}"], ["[", "]"], ["(", ")"]]
```

### Auto-Closing Pairs

```json
"autoClosingPairs": [
    { "open": "{", "close": "}" },
    { "open": "\"", "close": "\"" },
    { "open": "'", "close": "'" },
    { "open": "/*", "close": "*/" }
]
```

### Comments

```json
"comments": {
    "lineComment": "//",
    "blockComment": ["/*", "*/"]
}
```

### Indentation

```json
"indentationRules": {
    "increaseIndentPattern": "^.*\\{[^}\"']*$",
    "decreaseIndentPattern": "^\\s*[\\}\\]\\)].*$"
}
```

---

## Troubleshooting

### Language Server Not Starting

1. **Check Helen installation**:
   ```bash
   which helen
   helen help
   ```

2. **Check VS Code settings**:
   - Open settings (`Ctrl+,`)
   - Search for "helen"
   - Verify `helen.lsp.path` is correct

3. **Check the Output panel**:
   - View → Output
   - Select "Helen Language Server" from the dropdown
   - Check for error messages

### Syntax Highlighting Not Working

1. Make sure the file extension is `.helen`
2. Check the language mode (bottom-right corner)
3. Manually set the language: `Ctrl+Shift+P` → "Change Language Mode" → "Helen"

### Completion Not Working

1. Wait for the Language Server to initialize (check the status bar)
2. Check the Output panel for errors
3. Try restarting the Language Server

---

## Development

### Build

```bash
cd extensions/vscode
npm install
npm run compile
```

### Package

```bash
npx vsce package
# Generates helen-language-1.8.0.vsix
```

### Test

```bash
# Press F5 in VS Code to launch the Extension Development Host
# Open a .helen file in the new window
```

---

## Sample Code

```helen
// Define an AI agent
agent code_reviewer {
    description = "Reviews code for quality"
    model = "gpt-4"
    temperature = 0.3
    
    functions {
        fn review(code: string) -> dict {
            let issues = []
            return {"issues": issues, "score": 85}
        }
    }
}

// Pattern matching
fn categorize(error: dict) -> string {
    let code = error["code"] ?? 0
    return match code {
        case 1..100 { "error-patterns" }
        case 101..200 { "code-quality" }
        default { "general" }
    }
}

// Protocol (interface)
protocol Validator {
    fn validate(data: any) -> bool
}

// Main entry point
fn main() {
    let result = code_reviewer.review("print('hello')")
    print(result)
}
```

---

## Language Reference

Complete Helen language documentation:
- [Helen GitHub Repository](https://github.com/hahalee00000/helen)
- [Helen High Level Design Document](https://github.com/hahalee00000/helen/blob/main/documents/Helen_High_Level_Design_v1.2.md)

### Key Features

- **Agent declarations** — AI agents with LLM configuration
- **Pattern matching** — `match/case` expressions
- **Protocols** — `protocol/impl` interfaces
- **Error handling** — `try/catch/finally`
- **Concurrency** — `spawn` / Channel message queues
- **Standard library** — Common utility functions

---

## Contributing

Contributions welcome! Please submit issues or PRs on [GitHub](https://github.com/hahalee00000/helen).

---

**Helen** - The Agent Programming Language 🚀
