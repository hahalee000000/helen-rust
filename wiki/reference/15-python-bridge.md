# Tutorial 15: Python Bridge

> Use Helen Agents directly from Python

## Overview

The Helen Python Bridge allows Python developers to import and use Helen Agents directly, just like ordinary Python classes. This is Helen's deep integration with the Python ecosystem.

> **Bidirectional Integration Panorama**: See [[reference/python-integration]] (covers FFI + Bridge + mixed usage patterns)
>
> **Reverse direction (Helen -> Python)**: See [[reference/09-python-ffi|Python FFI Tutorial]]

## Quick Start

### Installation

```bash
# Option 1: Install CLI from crates.io (recommended)
cargo install helen-rust

# Option 2: Install Python bridge from PyPI (for Python → Helen integration)
pip install helen-rust

# Verify
helen --version
# helen-rust 0.1.0 — Helen Agent Programming Language (Rust)
```

### 1. Create a Helen Agent

Create a `translator.helen` file:

```helen
agent TranslatorAgent(text: str, target: str) {
    description "Translate text to the target language"
    prompt "Translate '{{text}}' to {{target}}"
    
    main {
        return llm act "Translate '{{text}}' to {{target}}"
    }
}
```

### 2. Use It in Python

```python
from translator import TranslatorAgent

# Create an agent instance
agent = TranslatorAgent()

# Call the agent
result = agent("Hello", "French")
print(result)  # "Bonjour"
```

That simple! Python developers can use Helen Agents like ordinary Python classes without learning Helen syntax.

## Core Features

### Automatic Import

The Python Bridge uses an Import Hook to automatically recognize `.helen` files:

```python
# Automatically loads the translator.helen file
from translator import TranslatorAgent, SummarizerAgent
```

### Parameter Validation

```python
agent = TranslatorAgent()

# Correct call
result = agent("Hello", target="French")

# Missing required argument
result = agent("Hello")  # TypeError: missing required argument

# Unknown argument
result = agent("Hello", target="French", extra="value")  # TypeError
```

### Type Conversion

Automatic conversion between Python and Helen types:

```python
# Python -> Helen
agent(42, "text", [1, 2, 3], {"key": "value"})

# Helen -> Python
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

# Create a shared interpreter
interpreter = Interpreter()

# Multiple agents sharing it
agent1 = HelenAgentWrapper("Agent1", "agents.helen", interpreter)
agent2 = HelenAgentWrapper("Agent2", "agents.helen", interpreter)
```

### Session Management (v1.24+)

The Interpreter supports a `session_id` parameter to resume historical sessions:

```python
from helen.interpreter import Interpreter

# Method 1: Resume a specific session
interp = Interpreter(session_id="session_xxx")

# Method 2: Resume the most recent session
from helen.runtime.session_manager import SessionManager
manager = SessionManager()
sessions = manager.list_sessions()
if sessions:
    latest_sid = sessions[0]["session_id"]
    interp = Interpreter(session_id=latest_sid)

# Method 3: Create a new session by default (backward compatible)
interp = Interpreter()
```

**Typical usage**: Persisting conversations across calls in a Python service

```python
from helen.interpreter import Interpreter
from helen.python_bridge import HelenAgentWrapper

class ChatService:
    def __init__(self, session_id: str | None = None):
        # Can resume a previous conversation
        self.interp = Interpreter(session_id=session_id)
        self.agent = HelenAgentWrapper("ChatBot", "chat.helen", self.interp)

    def chat(self, message: str) -> str:
        return self.agent(message)

    @property
    def session_id(self) -> str:
        return self.interp._agent_context.session_id

# Usage
service = ChatService()
print(service.chat("Hello"))
print(f"Session: {service.session_id}")

# Resume next time
service2 = ChatService(session_id=service.session_id)
```

**Difference from `resume_session()`**:

| Feature | `Interpreter(session_id=...)` | `resume_session()` |
|---------|-------------------------------|-------------------|
| Timing | At interpreter creation | At runtime |
| Behavior | Directly reuses the specified session | Imports history into the current new session |
| Transcript files | One | Two |
| Use case | Python service persistent conversations | Switching context within code |

### Import Hook Session Reuse (v1.24.1+)

Explicitly constructing `Interpreter(session_id=...)` requires managing the interpreter instance yourself. But the import hook scenario (`from chat_tui import TUIChatAgent`) implicitly creates the interpreter, and you cannot pass arguments in the import statement.

v1.24.1 (Issue #16) adds a session_id detection chain to the import hook, resolved by priority:

```
1. set_session_id() explicit setting     (highest priority, dynamic in-process control)
2. Environment variable HELEN_SESSION_ID  (cross-process restart recovery)
3. Memento file .helen/current_session_id (relative to cwd, auto-persisted)
4. None                                  (default, creates a new session)
```

#### Method 1: Explicit API (multi-session process)

When a single process serves multiple users/sessions, each using a different session_id:

```python
from helen.python_bridge import set_session_id

# Must be called before importing .helen files
set_session_id("session_user_alice")
from chat_tui import TUIChatAgent   # Reuses alice's session

# Switch to another session (takes effect on next import)
set_session_id("session_user_bob")
```

#### Method 2: Environment Variable (cross-process restart)

```bash
# Specify session at startup
export HELEN_SESSION_ID=session_1784706227_daa6c8d4
python app.py
```

```python
# app.py
from chat_tui import TUIChatAgent   # Automatically reuses the session from the env var
```

#### Method 3: Memento File (auto-persisted)

Write the session_id to `.helen/current_session_id` (relative to cwd); the import hook reads it automatically:

```python
from pathlib import Path

# First startup: save after creating the session
from chat_tui import TUIChatAgent
agent = TUIChatAgent()
sid = agent.__interpreter__._agent_context.session_id

memento = Path(".helen/current_session_id")
memento.parent.mkdir(exist_ok=True)
memento.write_text(sid, encoding="utf-8")

# After process restart: import hook auto-reads the memento, reuses the same session
from chat_tui import TUIChatAgent   # Automatically reuses the session from the memento
```

#### Checking the Currently Active session_id

```python
from helen.python_bridge import get_session_id

print(get_session_id())  # Resolved session_id by priority, or None
```

**Recommended method by scenario**:

| Scenario | Recommended Method |
|----------|-------------------|
| Web service multi-user (multi-session in one process) | `set_session_id()` |
| Cross-process restart recovery | Environment variable `HELEN_SESSION_ID` |
| Local development auto-persistence | Memento file |
| One-off scripts | Don't set (default new session) |

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
        Initialize the wrapper
        
        Args:
            agent_name: Agent name
            helen_file: Path to the Helen file
            interpreter: Optional interpreter instance (for sharing)
        """
    
    def __call__(self, *args, **kwargs) -> Any:
        """Call the agent"""
    
    async def async_call(self, *args, **kwargs) -> Any:
        """Async call the agent"""
```

### Decorators

```python
@helen_agent(helen_file: str, agent_name: str = None)
def my_function(...):
    """Wrap a function as a Helen agent call"""

@helen_module(helen_file: str)
class MyModule:
    """Wrap a class as a collection of Helen agents"""
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

## Limitations

- Requires Python 3.10+ (because Helen uses match statements)
- Currently only supports agent calls, not other Helen features
- Type conversion currently only supports basic types (int, float, str, bool, list, dict)

## Future Plans

- Support more Helen features (functions, classes, etc.)
- Improve type conversion (support custom types)
- Add type hint generation
- Support the Helen module system

## Example Code

For complete examples, see the `examples/python_bridge/` directory:

- `translator.helen`: Helen agent definition
- `example_usage.py`: Complete usage example
- `test_simple.py`: Simple test

## Summary

The Helen Python Bridge makes Helen a "native extension" of Python. Python developers can use Helen Agents just like `numpy` or `pandas`, which maximizes Helen's adoption in the Python ecosystem.

---

> **Related Documentation**:
> - [[reference/python-integration|Helen <-> Python Bidirectional Integration Panorama]] — mixed usage examples + selection guide
> - [[reference/09-python-ffi|Python FFI]] — the reverse direction: calling Python libraries from Helen
