# Tutorial 08: Modules and Imports

> import / multi-format / cross-file reuse / path safety

---

## Basic Import

```helen
// utils.helen
fn double(x) {
    return x * 2
}

agent Helper {
    description "A helper agent"
    prompt "Help the user."
}

// main.helen
import "./utils.helen"

main {
    let result = double(21)    // 42
    Helper()              // Use the imported Agent
}
```

---

## Import Aliases

```helen
import "./math_utils.helen" as math

main {
    let result = math.add(1, 2)
}
```

---

## Multi-Format Import

### Importing .json

```helen
// config.json (separate file - shown for reference)
// {
//     "model": "gpt-4",
//     "temperature": 0.7,
//     "max_turns": 3
// }

// main.helen
import "./config.json" as cfg

main {
    // cfg contains the parsed JSON data
    // (accessed via environment variables or runtime in v1)
}
```

### Importing .md

```helen
// prompt.md
You are a helpful assistant.
Always respond in a friendly tone.
Be concise but thorough.

// main.helen
import "./prompt.md" as system_prompt

main {
    // system_prompt contains the plain text content
}
```

---

## import Does Not Execute main

The imported file's `main` block is **not** automatically executed:

```helen
// lib.helen
import std.core.*
fn utility() {
    return "useful"
}

main {
    print("This will NOT run when imported!")
}

// main.helen
import "./lib.helen"

main {
    utility()    // ✅ Can use the function
    // lib.helen's main does not execute
}
```

---

## Path Safety

### Allowed imports

```helen
import "./utils.helen"          // ✅ Current directory
import "./lib/helpers.helen"    // ✅ Subdirectory
import "../sibling/utils.helen" // ✅ Sibling directory (within safe bounds)
```

### Blocked imports

```helen
import "../../secrets.helen"    // ❌ Path escapes boundary
import "/etc/passwd"             // ❌ Absolute path
```

Path safety checks ensure imported files stay within the project directory.

---

## Circular Import Detection

```helen
// a.helen
import "./b.helen"
fn from_a() { return "A" }

// b.helen
import "./a.helen"    // Circular import, silently skipped
fn from_b() { return "B" }

// main.helen
import "./a.helen"

main {
    from_a()    // ✅
    from_b()    // ✅ (b.helen imported from main)
}
```

---

## Example Project Structure

```
my-project/
├── main.helen
├── agents/
│   ├── translator.helen
│   ├── summarizer.helen
│   └── classifier.helen
├── utils/
│   ├── text.helen
│   └── validation.helen
├── config.json
└── prompts/
    ├── translator.md
    └── summarizer.md
```

```helen
// main.helen
import "./agents/translator.helen"
import "./agents/summarizer.helen"
import "./agents/classifier.helen"
import "./utils/text.helen" as text_utils
import "./config.json" as config

main {
    // Use all imported Agents and utilities
}
```

---

## Exercises

1. Create a utils.helen file with commonly used functions
2. Import and use those functions in main.helen
3. Create a config.json and import it
4. Try circular imports and observe the behavior

---

## ⚠️ Important Development Note: Module Caching

### Problem: Changes to .helen files don't take effect?

Helen's `ImportResolver` uses an **in-memory cache** to speed up repeated imports. This means:

- ✅ **CLI mode** (`helen main.helen`): reloads every time, no worries
- ❌ **REPL / long-running services**: files are not automatically reloaded after modification

### Example Scenario

```python
# Scenario 1: Developing in Python REPL
from helen.interpreter import Interpreter

interp = Interpreter()
interp.execute_file("agent.helen")  # Loads v1

# Modify agent.helen (add new features)...

interp.execute_file("agent.helen")  // ❌ Still v1!
```

### Solutions

#### Solution 1: Use CLI (recommended for development)

```bash
# Each execution is a new process, automatically reloads
helen main.helen
```

#### Solution 2: Create a new instance in code

```python
def run_agent():
    # Create a new Interpreter each time; cache is automatically cleared
    interp = Interpreter()
    return interp.execute_file("agent.helen")
```

#### Solution 3: Manually clear the cache

```python
interp = Interpreter()
interp.execute_file("agent.helen")

# After modifying files, manually clear the cache
interp.import_resolver._cached_results.clear()
interp.import_resolver._loaded.clear()

# Re-execute
interp.execute_file("agent.helen")  // ✅ Uses new code
```

### Deeper Understanding

See [runtime/import.md - Caching Mechanism](../runtime/import.md#caching-mechanism-developer-must-read)

---
