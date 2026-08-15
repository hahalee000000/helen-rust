<!-- helen-rust edition: crates/helen-runtime llm (M5) — providers, HTTP+SSE, tool dispatch. -->

# Runtime System

> Module M7 (`helen/runtime/`) | HLD 3.8

---

## Runtime ABC (12 Methods)

```python
class Runtime(ABC):
    # Tool & Skill
    def load_tool(name) -> Any
    def list_skills() -> list[SkillMeta]
    def load_skill(name) -> str

    # LLM
    def call_llm(messages, tools, model, temperature, max_turns) -> Any
    def cancel_llm_call(call_id) -> bool

    # Memory
    def get_memory(key) -> str | None
    def set_memory(key, value)

    # Import
    def resolve_import(path, from_file) -> Any

    # Token & History
    def get_token_count(text) -> int
    def get_conversation_history() -> list[Message]
    def set_conversation_history(history)

    # Provider
    def register_memory_provider(protocol, provider)
```

**Core code never directly imports Hermes** — it only interacts through this interface.

---

## HelenHermesRuntime

Default implementation, inherits from `Runtime` ABC:

```python
class HelenHermesRuntime(Runtime):
    def __init__(self, llm_runtime=None, import_resolver=None):
        self._llm_runtime = llm_runtime
        self._import_resolver = import_resolver
        self._memory: dict[str, str] = {}
        self._conversation_history: list[Message] = []
        self._active_calls: dict[str, _CallHandle] = {}
        self._memory_providers: dict[str, Any] = {}
        self._lock = threading.Lock()
```

---

## Cancellable LLM Calls

### _CallHandle

```python
class _CallHandle:
    cancelled: threading.Event   # Cancellation signal
    result: Any                  # Call result
    exception: Exception | None  # Exception
    done: threading.Event        # Completion signal
```

### cancel_llm_call()

```python
def cancel_llm_call(self, call_id: str) -> bool:
    with self._lock:
        handle = self._active_calls.get(call_id)
    if handle is None:
        return False          # Not found or already completed
    handle.cancelled.set()
    return True               # Cancellation signal sent
```

### CancelledError

```python
class CancelledError(Exception):
    def __init__(self, call_id: str):
        self.call_id = call_id
        super().__init__(f"LLM call {call_id} was cancelled")
```

---

## MockLLMRuntime (for Testing)

```python
@dataclass
class MockLLMRuntime(LLMRuntime):
    route_return: str | None = None       # Preset route() return value
    act_return: LLMResponse | str | None  # Preset act() return value
    route_fail: Exception | None = None   # Preset route() exception
    act_fail: Exception | None = None     # Preset act() exception
    route_history: list[dict]             # Call history
    act_history: list[dict]               # Call history
```

Supports deterministic testing without a real LLM.

---

## HermesCLILLMRuntime (CLI Mode, Slow)

Calls LLM through Hermes CLI (fallback approach):

```python
@dataclass
class HermesCLILLMRuntime(LLMRuntime):
    hermes_path: str = "hermes"      # Hermes CLI path
    default_model: str | None = None # Default model
    timeout: int = 120               # Timeout in seconds
```

**Performance:** 15-17 seconds/call (includes process startup overhead)

**Use cases:**
- Fallback when HTTP API is unavailable
- When Hermes-specific features (skills, tools) are needed

---

## HttpLLMRuntime (HTTP Mode, Fast)

Directly calls OpenAI-compatible API (recommended):

```python
@dataclass
class HttpLLMRuntime(LLMRuntime):
    base_url: str = ""      # API endpoint
    api_key: str = ""       # API key
    default_model: str = "qwen3.7-plus"  # Default model
    timeout: int = 120
```

**Configuration loading:** Via `helen.runtime.config` module, loads from two sources:

1. **Configuration file**: `~/.helen/config.yaml` (YAML format)
2. **Environment variables** (override config file): `HELEN_BASE_URL`, `HELEN_API_KEY`, `HELEN_MODEL`

Environment variables take precedence over config file values. This is useful for CI/CD pipelines and temporary overrides.

**Interactive setup:** When Helen is not configured and running in an interactive terminal, it automatically launches a setup wizard to guide users through configuration.

**Performance:** 7-11 seconds/call (no process startup overhead)

**Implementation:**
```python
def _chat(self, prompt: str, model: str = None, temperature: float = 1.0):
    url = f"{self.base_url}/chat/completions"
    payload = {
        "model": model or self.default_model,
        "messages": [{"role": "user", "content": prompt}],
        "temperature": temperature,
    }
    # HTTP POST request...
```

**Use cases:**
- REPL interaction (default)
- Script mode (`helen <file>`)
- Scenarios requiring fast response
- Production deployment

---

## Platform Protocol Abstraction (v1.35)

HttpLLMRuntime uses a **two-layer protocol abstraction** to support multiple OpenAI-compatible providers:

### Layer 1: PlatformProtocol (base_url)

Protocol is determined by the **platform** (base_url), not the model. This is because aggregator platforms like DashScope and Volcengine Ark normalize the protocol across all models they host.

```python
from helen.runtime.provider_protocol import detect_protocol

# Auto-detect from base_url
protocol = detect_protocol("https://dashscope.aliyuncs.com/compatible-mode/v1")
# -> DashScopeProtocol (ALL models on DashScope use this protocol)
```

**Supported platforms:**

| Platform | Protocol Class | Key Behavior |
|----------|---------------|--------------|
| DashScope | `DashScopeProtocol` | `enable_thinking` + `thinking_budget` |
| Volcengine Ark | `VolcengineProtocol` | `encrypted_content` priority, Endpoint ID |
| Zhipu (GLM) | `ZhipuProtocol` | `tool_choice` only supports `"auto"` |
| DeepSeek | `DeepSeekProtocol` | `reasoning_effort`, mutually exclusive streaming |
| Minimax | `MinimaxProtocol` | `reasoning_split`, cumulative `reasoning_details` |
| Kimi (Moonshot) | `KimiProtocol` | Usage at `choices[0].usage` (not top-level) |
| OpenAI (default) | `OpenAIProtocol` | Standard OpenAI protocol |

### Layer 2: ModelCapabilities (model_id)

While PlatformProtocol handles format differences, ModelCapabilities handles **feature availability** per model:

```python
from helen.runtime.model_capabilities import get_model_capabilities

caps = get_model_capabilities("deepseek-v4-pro")
# -> ModelCapabilities(
#     supports_thinking=True,
#     reasoning_content_streaming="mutually_exclusive",  # Critical!
#   )
```

**Key capability differences:**

| Model | Streaming Behavior | Special |
|-------|-------------------|---------|
| `deepseek-v4-pro` | mutually_exclusive | - |
| `MiniMax-M3` | cumulative | `has_reasoning_details=True` |
| `glm-4.7` | incremental | `forced_thinking=True` (cannot disable) |
| `doubao-seed-1.6-thinking` | incremental | `has_encrypted_content=True` |
| `qwen-max` | incremental | `supports_thinking=False` (not Qwen3) |

### Protocol Methods

```python
class PlatformProtocol:
    def build_request_payload(base_payload, *, model_id, thinking_enabled, reasoning_effort) -> dict
    def parse_response(response_data) -> dict
    def parse_streaming_delta(delta, context) -> dict
    def extract_streaming_usage(chunk) -> dict | None
    def parse_error(status_code, response_body) -> str
    def supports_tool_choice(value) -> bool
```

The runtime delegates provider-specific logic to the protocol, keeping `http_llm.py` clean and extensible.

### Multi-turn Reasoning Preservation (v1.37)

When using `thinking-mode` together with tool calls, the `reasoning_content` must be preserved in the assistant message when feeding tool results back to the LLM. This is a **correctness requirement** for some providers:

| Provider | Behavior if `reasoning_content` is missing in tool-call multi-turn |
|----------|--------------------------------------------------------------------|
| **DeepSeek** | Returns **400 error** (requires `reasoning_content` in all subsequent requests) |
| **Minimax** | Loses chain-of-thought continuity (reasoning breaks across tool calls) |
| Others | Ignores missing `reasoning_content` (no error, but context may degrade) |

**Implementation**: Both `act()` (non-streaming) and `act_stream()` (streaming) preserve `reasoning_content` when building the assistant message for the next tool-call round:

```python
# v1.37: Preserve reasoning_content for multi-turn tool calls
full_reasoning = "".join(reasoning_chunks) if reasoning_chunks else ""
assistant_msg = {"role": "assistant", "content": full_content or None}
if full_reasoning:
    assistant_msg["reasoning_content"] = full_reasoning
```

### Endpoint ID Validation (v1.37)

For Volcengine Ark (Doubao), the protocol validates the Endpoint ID format:

- **Production**: Should use Endpoint IDs (`ep-XXXXX` format) created in the Ark console
- **Preset**: Direct model names (e.g., `doubao-pro-128k`) work but log a debug warning
- The validation is non-blocking (only logs a warning, does not fail)

```python
# VolcengineProtocol._validate_endpoint_id()
if not model_id.startswith("ep-"):
    logger.debug("Using model name instead of Endpoint ID - recommended for production")
```

### Provider Auto-Detection (v1.40.1)

The `detect_protocol()` function supports **config-aware detection** with a three-tier priority:

```python
# Priority: explicit name (from config) > URL pattern match > OpenAI fallback
protocol = detect_protocol(base_url, protocol_name="deepseek")  # → DeepSeekProtocol
protocol = detect_protocol("https://api.deepseek.com")          # → DeepSeekProtocol (URL match)
protocol = detect_protocol("https://unknown.com")               # → OpenAIProtocol (fallback)
```

When `helen init` detects a provider, it saves the `protocol` field in config.yaml:

```yaml
llm:
  protocol: "deepseek"
  capabilities:
    thinking: true
    streaming: true
    vision: false
```

At runtime, `HttpLLMRuntime.__post_init__()` reads this field and passes it to `detect_protocol()`, avoiding re-detection on every startup.

**Connectivity probing** (in `helen/runtime/probe.py`) uses a three-layer architecture:

| Layer | Purpose | Cost | When |
|-------|---------|------|------|
| Layer 1 | Basic connectivity | 1 API call | Always for unknown URLs |
| Layer 2 | Protocol variant detection | 2-6 API calls | Optional (user prompted) |
| Layer 3 | Capability detection (vision, tool_choice) | 1-2 API calls | Optional (user prompted) |

Layer 1 sends a minimal `{"content": "hi"}` request and classifies errors:
- Connection/timeout → `error_type: "connection"`
- HTTP 401/403 → `error_type: "auth"`
- HTTP 404 with "model" → `error_type: "model_not_found"`
- HTTP 200 but unparseable → `error_type: "protocol"` (triggers deep probe offer)

Layer 2 tries each known protocol's `build_request_payload()` with `thinking_enabled=True`, checking if the response contains `reasoning_content`. First match wins.

### Custom Provider Adapters (v1.40.1)

For providers not in the built-in list, use `helen agent` to create a `PlatformProtocol` subclass:

```bash
helen agent
# Ask the agent to generate a PlatformProtocol subclass
# and save it to ~/.helen/providers/<name>.py
```

The agent has `web_search`, `web_fetch`, `write_file` and other tools to research the provider's API and generate the adapter interactively.

Generated adapters are saved to `~/.helen/providers/<name>.py` and loaded dynamically by `detect_protocol()` at runtime.

> **See also**: [[runtime/llm-provider-protocol-reference|LLM Provider Protocol Reference]] for full protocol details across all 6 providers; the `helen-custom-provider` skill (in `helen/skills/software-development/helen-custom-provider/`) for a guided walkthrough with decision tree, API reference, and worked examples.

---

## Built-in Tool System

Helen provides 7 built-in tools that the LLM can call via function calling during `llm act` execution:

| Tool | Function | Parameters |
|------|------|------|
| `web_search` | Search Wikipedia | `query: str` |
| `web_fetch` | Fetch web content | `url: str` |
| `read_file` | Read file | `path: str` |
| `write_file` | Write file (overwrite) | `path: str, content: str` |
| `patch_file` | Precise file modification (fuzzy matching) | `path, old_string, new_string` |
| `shell_exec` | Execute shell command | `command: str` |
| `calculate` | Math calculation | `expression: str` |

### patch_file Fuzzy Matching

`patch_file` uses 9 matching strategies to handle common discrepancies in LLM-generated code:

| # | Strategy | Handles |
|---|------|---------|
| 1 | Exact | Exact match |
| 2 | Line-trimmed | Leading/trailing whitespace differences |
| 3 | Whitespace-normalized | Multiple spaces/tabs normalized |
| 4 | Indentation-flexible | Indentation completely ignored |
| 5 | Escape-normalized | `\n` `\t` escape differences |
| 6 | Trimmed-boundary | First/last line whitespace trimmed |
| 7 | Unicode-normalized | Smart quotes, dashes, etc. |
| 8 | Block-anchor | SequenceMatcher similarity |
| 9 | Context-aware | Line-by-line similarity |

Tool registry is located in `helen/runtime/tools.py`; the fuzzy matching engine is in `helen/runtime/fuzzy_match.py` (integrated from Hermes, runs independently).

```python
@dataclass
class HelenTool:
    name: str
    description: str
    parameters: dict[str, Any]  # JSON Schema
    handler: Callable[..., str]
```

Agents declare available tools through `tools` configuration:

```helen
agent Researcher(topic) {
    description "Research assistant"
    tools = ["web_search", "web_fetch", "read_file"]
    main {
        return llm act "Research: " + topic
    }
}
```

---

## Skill System

Skill directory scan priority:

1. `~/.helen/skills/` — Helen native skills
2. `~/.hermes/skills/` — Hermes fallback
3. `~/.hermes/hermes-agent/skills/` — Hermes agent skills

### Two-Phase Disclosure

Helen implements the two-phase skill disclosure mechanism from HLD §3.7.1:

**Tier 1: Skill Index (lightweight)**

`PromptBuilder.build_skill_index()` scans the skill directory, reads SKILL.md YAML frontmatter (name, description, category), and formats it as an `<available_skills>` XML block injected into the System Prompt:

```xml
<available_skills>
Before replying, scan skills below. If relevant,
use load_skill tool to load full content.

  devops:
    - helen-language: Helen programming language development...
  research:
    - research: Research discovery and monitoring...
</available_skills>
```

**Tier 2: load_skill Tool (on-demand loading)**

The `load_skill` tool is registered in `helen/runtime/tools.py`; the LLM can load full SKILL.md content on demand via function calling:

```python
# LLM calls load_skill tool
dispatch_tool('load_skill', {'name': 'helen-language'})
# Returns full SKILL.md content (67KB+)
```

**Advantages**:
- Tier 1 only consumes ~16KB tokens (all skill names + descriptions)
- Tier 2 loads on demand — full content is loaded only when the LLM needs it
- Avoids wasting tokens by sending large skill content every time

---

## Performance Comparison

| Runtime | Call Time | Overhead Source |
|---------|---------|---------|
| HttpLLMRuntime | 7-11s | Network latency + LLM inference |
| HermesCLILLMRuntime | 15-17s | Process startup + config loading + network + inference |

**REPL uses HttpLLMRuntime by default**, approximately 2× performance improvement.

---

## llm act Expression

`llm act` supports two usage forms:

### 1. Statement Form (within agent context)

```helen
agent Translator(text) {
    prompt "Translate text"
    model "gpt-4"
    main {
        llm act Translator(text=text) "Translate to Chinese"
    }
}
```

Syntax: `llm act target(arg=value, ...) "description"`

### 2. Expression Form (direct LLM call)

```helen
// Top-level direct call
llm act "translate hello to chinese."

// Used in a function
fn translate(text, target) {
    return llm act "translate " + text + " to " + target
}

// Assigned to a variable
let result = llm act "summarize this article"

// Used in an agent
agent Smart(text) {
    main {
        return llm act "analyze: " + text
    }
}
```

Syntax: `llm act <expression>`

The expression form will:
- Evaluate the expression as the prompt
- Call the LLM runtime
- Return the LLM response text (string)

### Parser Disambiguation

The parser determines the form via lookahead:
- If `llm act` is followed by an IDENTIFIER with `(` or STRING after it → statement form
- Otherwise → expression form

---

## Memory System

### MemoryProvider ABC

```python
class MemoryProvider(ABC):
    @abstractmethod
    def get(self, key: str) -> str | None
    @abstractmethod
    def set(self, key: str, value: str) -> None
    @abstractmethod
    def delete(self, key: str) -> None
    @abstractmethod
    def list_keys(self) -> list[str]
```

### FileMemoryProvider

JSON file persistence:

```python
class FileMemoryProvider(MemoryProvider):
    def __init__(self, path: str):
        self._path = path
        self._data = self._load()

    def _load(self) -> dict:
        if os.path.exists(self._path):
            return json.load(open(self._path))
        return {}

    def _save(self):
        json.dump(self._data, open(self._path, 'w'))
```

### InMemoryProvider

Pure in-memory implementation, used for testing.

---

## v1.10 Async HTTP Support

### Overview

v1.10 added async HTTP methods based on `httpx.AsyncClient`, supporting concurrent LLM calls.

### Async Methods

```python
class LLMRuntime:
    # Synchronous methods
    def act(self, target: str, description: str, **kwargs) -> Any
    def act_stream(self, target: str, description: str, **kwargs) -> Iterator[str]
```

**v1.18 change**: `act_async()` / `act_stream_async()` have been removed, replaced by `spawn` + Channel. Concurrent LLM calls are now achieved through spawn:

```helen
// v1.18 concurrent LLM calls
let m1 = spawn AgentA("task1")
let m2 = spawn AgentB("task2")
let [r1, r2] = [m1.receive(), m2.receive()]
```

### httpx.Client

Synchronous methods use `httpx.Client`:

```python
class HttpLLMRuntime(LLMRuntime):
    def __init__(self, base_url: str, api_key: str, model: str):
        self._client = httpx.Client(
            base_url=base_url,
            headers={"Authorization": f"Bearer {api_key}"},
            timeout=60.0
        )
```

**v1.18 change**: `httpx.AsyncClient` has been removed; concurrency is now implemented via `spawn` (threading.Thread).

### Usage Example

```helen
agent MyAgent {
  main {
    // Synchronous call
    let result = llm act Translate "Hello"
    
    // Concurrent call (v1.18 spawn)
    let m1 = spawn Translate("Hello")
    let m2 = spawn Translate("World")
    let r1 = m1.receive()
    let r2 = m2.receive()
  }
}
```

### Connection Pool Management

`httpx.Client` automatically manages connection pooling:

- **Connection reuse**: Multiple requests reuse the same TCP connection
- **Concurrency control**: Concurrency via `spawn` (threading.Thread)
- **Timeout management**: Unified timeout configuration
- **Resource cleanup**: Connections automatically closed on program exit

### Performance Advantages

| Scenario | Serial | Spawn Concurrency | Improvement |
|------|------|----------------|------|
| Single call | 1.5s | 1.5s | 0% |
| 3 concurrent | 4.5s | ~1.6s | 65% |
| 10 concurrent | 15s | ~2.1s | 86% |

**Note**: Since v1.18, concurrency is implemented via `spawn`, with each spawned agent running in an independent daemon thread.

### Error Handling

Async methods use the same error handling mechanism:

```helen
try {
  let result = await llm act_async Task "Complex task"
} catch LLMError as e {
  print("LLM Error: " + e.message)
} catch TimeoutError as e {
  print("Timeout: " + e.message)
}
```

---

**Last Updated**: 2026-07-04  
**Version**: v1.11

---

## P4 History Management Enhancement (v1.11 Addition)

> v1.11 introduced complete history persistence, retrieval, and context visualization features.

### History Persistence

Retain conversation continuity across sessions:

```helen
agent PersistentAgent {
    main {
        // Save current history to JSON file
        save_history("./session.json")
        
        // Load history from file (on next startup)
        let loaded = load_history("./session.json")
        print("Loaded " + str(loaded) + " messages")
    }
}
```

**JSON format**:
```json
{
  "version": 1,
  "model": "qwen3.7-plus",
  "saved_at": "2026-07-04T12:00:00Z",
  "messages": [
    {"role": "user", "content": "..."},
    {"role": "assistant", "content": "..."}
  ]
}
```

### History Retrieval

Agents can query specific information in history:

```helen
agent SmartResearcher {
    tools ["web_search", "load_skill"]
    main {
        // Search previous tool calls
        let past_searches = search_history(tool_name="web_search")
        
        // Filter by role
        let user_questions = search_history(role="user")
        
        // Text search (case-insensitive)
        let mentions = search_history(query="Python")
        
        // Get tool call history
        let tool_log = get_tool_history("web_search")
        
        return llm act "Continue research..."
    }
}
```

### Context Usage Visualization

Use the `:stats` command in REPL to view context usage statistics:

```
> :stats
╔══════════════════════════════════════╗
║       Context Usage Statistics        ║
╠══════════════════════════════════════╣
║ ✅ ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  12.3%            ║
║ Tokens:   15,984 /  131,072              ║
║ Model:  qwen3.7-plus                  ║
║ Messages: 8                           ║
╠──────────────────────────────────────╣
║  User             3,200 tokens        ║
║  Assistant        8,500 tokens        ║
║  System_prompt    2,100 tokens        ║
║  System           2,184 tokens        ║
╚══════════════════════════════════════╝
```

### Token Estimation Enhancement

v1.11 supports optional tiktoken exact counting (install `helen[accurate-tokens]`), otherwise uses character-level heuristics (~15% accuracy):

```bash
# Install exact token counting
pip install "helen[accurate-tokens]"
```

### History Compression Strategies

v1.11 provides three compression modes:

| Mode | Description | Use Case |
|------|------|---------|
| `summarize` (default) | Three-layer compression: recent → middle → oldest | Long conversations maintaining context |
| `truncate` | Directly drops old messages | Simple scenarios |
| `none` | No compression (may exceed context limits) | Short conversations/testing |

```python
# Dynamically switch compression mode
interpreter._history_manager.set_compression_mode("truncate")
```
