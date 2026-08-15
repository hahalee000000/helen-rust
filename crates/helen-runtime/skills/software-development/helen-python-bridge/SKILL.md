---
name: helen-python-bridge
description: Helen Python Bridge Usage Guide — Let Python directly use Helen Agents and functions, enabling bidirectional FFI
version: 1.1.0
author: Helen Team
tags: [python, ffi, bridge, integration, agent, interoperability, function]
---
<!-- helen-rust edition: the Rust port's bridge is the `helen_rust` wheel (M11, maturin cdylib): `import helen_rust; from translator import TranslatorAgent`. Loads the RUST interpreter, not the Python one. -->


# Helen Python Bridge

Helen Python Bridge allows Python developers to directly import and use Helen Agents and functions, just like using regular Python classes and functions. This is Helen's deep integration with the Python ecosystem.

## Overview

Python Bridge implements bidirectional FFI between Helen and Python:

1. **Helen → Python (FFI)**: Use Python libraries in Helen (existing)
2. **Python → Helen (Bridge)**: Use Helen Agents and functions in Python (new)

This makes Helen a "native extension" of the Python ecosystem — Python developers can use Helen Agents and functions just like `numpy`, `pandas`, etc.

> 📘 **Want the full picture of bidirectional integration?** See `wiki/reference/python-integration.md` — architecture diagram + mixed usage examples

## Helen → Python (FFI) Quick Reference

This document focuses on the **Python → Helen** direction. If you need **Helen → Python** (calling Python libraries from Helen), the core syntax is:

```helen
// Import Python modules (no extension = Python module)
import "math" as math
import "json" as json
import "mylib.renderer" as PyRenderer

// Call functions / access constants
let s = json.dumps({"k": "v"})
let pi = math.pi

// Instantiate Python classes + call methods
let encoder = json.JSONEncoder()
let result = encoder.encode({"x": 1})        // Natural method call (recommended)
let result2 = encoder.call("encode", {"x": 1})  // Call by method name (dynamic scenarios)
```

**Key points:**
- Class instantiation: `PyModule.ClassName()` — classes are callable
- Method calls: prefer `obj.method()`; use `obj.call("method")` for dynamic method names
- Nested imports: Python imports in imported `.helen` modules are fully available

→ Detailed tutorial: `wiki/reference/09-python-ffi.md`

## Quick Start

### 1. Create a Helen Agent

```helen
// translator.helen
agent TranslatorAgent(text: str, target: str) {
    description "Translate text to target language"
    prompt "Translate '{{text}}' to {{target}}"

    main {
        return llm act "Translate '{{text}}' to {{target}}"
    }
}
```

### 2. Use It in Python

```python
from translator import TranslatorAgent

# Create agent instance
agent = TranslatorAgent()

# Call agent
result = agent("Hello", "French")
print(result)  # "Bonjour"
```

## Core Features

### Automatic Import

Python Bridge uses Import Hook to automatically recognize `.helen` files (function import supported since v1.23.6+):

```python
# Install import hook (one-time)
from helen.python_bridge.import_hook import install_import_hook
install_import_hook()

# Auto-load agents and functions from translator.helen
from translator import TranslatorAgent, SummarizerAgent, format_text
```

**Helen file example:**

```helen
// translator.helen
import std.str.*
const default_lang = "English"

// Regular function (pure computation, no LLM calls)
fn format_text(text: str): str {
    返回 text.strip().capitalize()
}

// Agent (requires LLM reasoning)
agent TranslatorAgent(text: str, target: str) {
    description "Translate text to target language"
    prompt "Translate '{{text}}' to {{target}}"

    main {
        return llm act "Translate '{{text}}' to {{target}}"
    }
}
```

**Python usage:**

```python
from translator import TranslatorAgent, format_text

# Call function directly
formatted = format_text("  hello world  ")  # "Hello world"

# Call agent
agent = TranslatorAgent()
result = agent("Hello", "French")  # "Bonjour"
```

### Parameter Validation

```python
agent = TranslatorAgent()

# ✅ Correct call
result = agent("Hello", target="French")

# ❌ Missing required argument
result = agent("Hello")  # TypeError: missing required argument

# ❌ Unknown argument
result = agent("Hello", target="French", extra="value")  # TypeError
```

### Type Conversion

Automatically converts between Python and Helen types:

```python
# Python → Helen
agent(42, "text", [1, 2, 3], {"key": "value"})

# Helen → Python
result = agent(...)  # Automatically converted to Python types
```

Supported types:
- Primitives: `int`, `float`, `str`, `bool`
- Collections: `list`, `dict`
- Null: `None`

### Async Calls

```python
import asyncio

async def main():
    agent = TranslatorAgent()
    result = await agent.async_call("Hello", "Spanish")
    print(result)

asyncio.run(main())
```

### Keyword Arguments

```python
agent = TranslatorAgent()

# Positional arguments
result = agent("Hello", "French")

# Keyword arguments
result = agent(text="Hello", target="French")

# Mixed usage
result = agent("Hello", target="French")
```

## Advanced Usage

### Decorator Pattern

Use the `@helen_agent` decorator to simplify calls:

```python
from helen.python_bridge import helen_agent

@helen_agent("translator.helen", "TranslatorAgent")
def translate(text: str, target: str) -> str:
    pass

result = translate("Hello", "French")
```

### Shared Interpreter

Multiple agents can share the same interpreter instance:

```python
from helen.interpreter import Interpreter
from helen.python_bridge import HelenAgentWrapper

# Create shared interpreter
interpreter = Interpreter()

# Multiple agents sharing it
agent1 = HelenAgentWrapper("Agent1", "agents.helen", interpreter)
agent2 = HelenAgentWrapper("Agent2", "agents.helen", interpreter)
```

### Session Management (v1.24+)

Interpreter supports the `session_id` parameter to resume historical sessions, enabling cross-process conversation persistence:

```python
from helen.interpreter import Interpreter

# Approach 1: Resume a specific session
interp = Interpreter(session_id="session_xxx")

# Approach 2: Resume the most recent session
from helen.runtime.session_manager import SessionManager
manager = SessionManager()
sessions = manager.list_sessions()
if sessions:
    latest_sid = sessions[0]["session_id"]
    interp = Interpreter(session_id=latest_sid)

# Approach 3: Create new session by default (backward compatible)
interp = Interpreter()
```

**Typical usage in web services**:

```python
from helen.interpreter import Interpreter
from helen.python_bridge import HelenAgentWrapper

class ChatService:
    """Chat service with cross-request conversation persistence"""

    def __init__(self, user_id: str, session_id: str | None = None):
        self.user_id = user_id
        # Resume previous conversation or create a new one
        self.interp = Interpreter(session_id=session_id)
        self.agent = HelenAgentWrapper("ChatBot", "chat.helen", self.interp)

    def chat(self, message: str) -> str:
        return self.agent(message)

    @property
    def session_id(self) -> str:
        """Return current session_id; client can save it for next-time resumption"""
        return self.interp._agent_context.session_id

# Flask/FastAPI web service example
from flask import Flask, request, jsonify

app = Flask(__name__)

@app.post("/chat")
def chat():
    data = request.json
    service = ChatService(
        user_id=data["user_id"],
        session_id=data.get("session_id"),  # Optional: resume historical conversation
    )
    response = service.chat(data["message"])
    return jsonify({
        "response": response,
        "session_id": service.session_id,  # Client saves this and passes it next time
    })
```

**`Interpreter(session_id=...)` vs `resume_session()`**:

| Feature | `Interpreter(session_id=...)` | `resume_session()` |
|---------|------------------------------|-------------------|
| Timing | At interpreter creation | Called at runtime |
| Behavior | Directly reuses specified session | Imports history into current new session |
| Transcript files | One | Two |
| Use case | Python service persistent conversation | Switching context within code |

### Import Hook Session Reuse (v1.24.1+, Issue #16)

Explicitly constructing `Interpreter(session_id=...)` requires managing interpreter instances yourself. But the import hook scenario
(`from chat_tui import TUIChatAgent`) creates interpreters **implicitly** — you cannot pass arguments in an import statement.

v1.24.1 adds a session_id detection chain to the import hook, resolved by priority:

```
1. set_session_id() explicit setting         (highest priority, in-process dynamic control)
2. Environment variable HELEN_SESSION_ID      (cross-process restart recovery)
3. Memento file .helen/current_session_id     (relative to cwd, auto-persisted)
4. None                                       (default, create new session)
```

```python
# Approach 1: Explicit API (multi-session process, must call before import)
from helen.python_bridge import set_session_id
set_session_id("session_user_alice")
from chat_tui import TUIChatAgent   # Reuses alice's session

# Approach 2: Environment variable (cross-process restart)
#   export HELEN_SESSION_ID=session_xxx && python app.py
from chat_tui import TUIChatAgent   # Automatically reuses session from env var

# Approach 3: Memento file (auto-persisted)
#   echo "session_xxx" > .helen/current_session_id
from chat_tui import TUIChatAgent   # Automatically reads memento to reuse session

# Check currently effective session_id
from helen.python_bridge import get_session_id
print(get_session_id())
```

**Applicable scenarios**:

| Scenario | Recommended Approach |
|----------|---------------------|
| Web service multi-user (multiple sessions in same process) | `set_session_id()` |
| Cross-process restart recovery | Environment variable `HELEN_SESSION_ID` |
| Local development auto-persistence | Memento file |
| One-off scripts | Don't set (default new session) |

### Calling Helen Functions (v1.23.6+)

Besides calling agents, Python Bridge also supports directly calling Helen's regular functions (`fn`):

```python
from helen.python_bridge.function_wrapper import HelenFunctionWrapper, load_helen_functions

# Method 1: Call a single function
add = HelenFunctionWrapper("add", "utils.helen")
result = add(10, 32)  # 42

greet = HelenFunctionWrapper("greet", "utils.helen")
result = greet("Python")  # "Hello, Python!"

# Method 2: Load all functions
functions = load_helen_functions("utils.helen")
# {'add': <HelenFunctionWrapper>, 'greet': <HelenFunctionWrapper>, ...}

result = functions['add'](100, 200)  # 300
result = functions['greet']("World")  # "Hello, World!"
```

**When to use functions vs agents:**
- **Functions (fn)**: pure computation, utility functions, data processing (no LLM calls)
- **Agents**: require LLM reasoning, tool calls, context management

**Mixed usage:**
```python
from helen.python_bridge.agent_wrapper import HelenAgentWrapper
from helen.python_bridge.function_wrapper import HelenFunctionWrapper

# Agent and function from the same Helen file
agent = HelenAgentWrapper("translator", "app.helen")
utils = HelenFunctionWrapper("format_text", "app.helen")

# Process data with function first, then reason with agent
processed = utils("raw text")
result = agent(processed)
```

### Batch Processing

```python
from agents import TranslatorAgent

agent = TranslatorAgent()
texts = ["Hello", "World", "AI"]

results = [agent(text, target="French") for text in texts]
print(results)  # ["Bonjour", "Monde", "IA"]
```

### Error Handling

```python
from agents import TranslatorAgent

agent = TranslatorAgent()

try:
    result = agent("Hello", target="French")
except TypeError as e:
    print(f"Parameter error: {e}")
except Exception as e:
    print(f"Execution error: {e}")
```

## Use Cases

### AI Agent Development

```python
from agents import ResearchAgent, AnalysisAgent

# Research phase
researcher = ResearchAgent()
findings = researcher("quantum computing", depth="deep")

# Analysis phase
analyzer = AnalysisAgent()
insights = analyzer(findings)
```

### Multi-Agent Collaboration

```python
from workflow import PlannerAgent, ExecutorAgent, ReviewerAgent

planner = PlannerAgent()
plan = planner("Build a web app")

executor = ExecutorAgent()
result = executor(plan)

reviewer = ReviewerAgent()
feedback = reviewer(result)
```

### LLM Applications

```python
from llm_agents import ChatBot, Summarizer, Translator

chatbot = ChatBot()
response = chatbot("What is AI?")

summarizer = Summarizer()
summary = summarizer(long_text)

translator = Translator()
translated = translator(summary, target="Chinese")
```

## API Reference

### HelenAgentWrapper

```python
class HelenAgentWrapper:
    def __init__(self, agent_name: str, helen_file: str, interpreter=None):
        """
        Initialize wrapper

        Args:
            agent_name: Agent name
            helen_file: Helen file path
            interpreter: Optional interpreter instance (for sharing)
        """

    def __call__(self, *args, **kwargs) -> Any:
        """Call agent"""

    async def async_call(self, *args, **kwargs) -> Any:
        """Async call agent"""
```

### Decorators

```python
@helen_agent(helen_file: str, agent_name: str = None)
def my_function(...):
    """Wrap function as Helen agent call"""

@helen_module(helen_file: str)
class MyModule:
    """Wrap class as Helen agents collection"""
```

### Import Hook

```python
from helen.python_bridge import install_import_hook

# Auto-install (default)
install_import_hook()

# Manual uninstall
from helen.python_bridge import uninstall_import_hook
uninstall_import_hook()
```

## Implementation Principles

1. **Import Hook**: Uses Python's `sys.meta_path` to intercept module imports
2. **Dynamic class generation**: Parses Helen files and dynamically creates Python classes for each agent
3. **Type conversion**: Automatically converts between Python and Helen types
4. **Parameter validation**: Checks argument types and required parameters
5. **Async support**: Provides `async_call` method for async invocations

## Limitations

- Requires Python 3.10+ (because Helen uses match statements)
- Currently only supports agent calls, not other Helen features
- Type conversion currently only supports basic types (int, float, str, bool, list, dict)

## Future Plans

- Support more Helen features (functions, classes, etc.)
- Improve type conversion (support custom types)
- Add type hint generation
- Support Helen module system

## Related Resources

- Full tutorial: [[reference/15-python-bridge]]
- Example code: `examples/python_bridge/`
- Python FFI tutorial: [[reference/09-python-ffi]]

## Best Practices

### 1. Use a Shared Interpreter

When you need to create multiple agent instances, using a shared interpreter improves performance:

```python
from helen.interpreter import Interpreter
from helen.python_bridge import HelenAgentWrapper

interpreter = Interpreter()
agent1 = HelenAgentWrapper("Agent1", "agents.helen", interpreter)
agent2 = HelenAgentWrapper("Agent2", "agents.helen", interpreter)
```

### 2. Batch Processing

For large-scale data processing, use list comprehensions for batch calls:

```python
results = [agent(item) for item in items]
```

### 3. Error Handling

Always use try-except to catch potential errors:

```python
try:
    result = agent(data)
except TypeError as e:
    print(f"Parameter error: {e}")
except Exception as e:
    print(f"Execution error: {e}")
```

### 4. Async Calls

For time-consuming agent calls, use async to avoid blocking:

```python
result = await agent.async_call(data)
```

### 5. Type Hints

Use type hints to improve code readability:

```python
def process_data(data: list, agent: TranslatorAgent) -> str:
    return agent(data)
```
