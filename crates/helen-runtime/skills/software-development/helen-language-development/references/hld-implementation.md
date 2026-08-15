# HLD Implementation Reference

Reference for implementing missing HLD features using Contract → Test → Implementation pattern.

## HLD Compliance Workflow

1. **Compare HLD docs** (`~/wiki/helen/`) against actual code to find gaps
2. **List gaps by category**: completely missing vs v1 simplified vs empty shell
3. **For each gap**: define contract (types + signatures) → write tests (RED) → implement (GREEN)

## Memory Provider Architecture (HLD §3.8.2)

### Contract
```python
class MemoryProvider(ABC):
    def get(self, key: str) -> str | None: ...
    def set(self, key: str, value: str) -> None: ...
    def delete(self, key: str) -> None: ...
    def list_keys(self) -> list[str]: ...
```

### Implementations
- **InMemoryProvider**: Pure dict-based, for testing
- **FileMemoryProvider**: JSON persistence, auto-creates dirs, corruption recovery
- **VectorMemoryProvider** (not yet implemented): For vector similarity search

### Integration with HelenHermesRuntime
```python
runtime = HelenHermesRuntime()
runtime.register_memory_provider("file", FileMemoryProvider("mem.json"))
runtime.set_memory("key", "value")  # delegates to registered provider
```

## LLM Runtime Architecture (Updated 2026-06)

### Two Runtimes Available

| Runtime | File | Speed | Mechanism |
|---------|------|-------|-----------|
| **HttpLLMRuntime** | `helen/runtime/http_llm.py` | 7-11s/call | Direct HTTP to OpenAI-compatible API |
| HermesCLILLMRuntime | `helen/runtime/hermes_cli_llm.py` | 15-17s/call | Spawns `hermes -z` subprocess |

### HttpLLMRuntime (REPL default, recommended)

Direct HTTP calls to OpenAI-compatible endpoints. No subprocess overhead.

```python
@dataclass
class HttpLLMRuntime(LLMRuntime):
    base_url: str = ""       # Auto-loaded from ~/.hermes/.env
    api_key: str = ""        # Auto-loaded from ~/.hermes/.env
    default_model: str = "qwen3.7-plus"
    timeout: int = 120
```

**Config auto-loading**: Reads `~/.hermes/.env` for `DASHSCOPE_API_KEY` and `DASHSCOPE_BASE_URL`.

**API endpoint**: `{base_url}/chat/completions` (OpenAI-compatible format)

**Performance**: 7-11s per call (network + LLM inference only, no process startup)

**Model name pitfall**: DashScope coding endpoint uses `qwen3.7-plus`, NOT `qwen-plus`. Wrong name → HTTP 400.

### HermesCLILLMRuntime (fallback)

Delegates to `hermes -z` (oneshot mode). Useful when HTTP API is unavailable.

```python
class HermesCLILLMRuntime(LLMRuntime):
    def _ask(self, prompt: str, model: str | None = None) -> str | None:
        cmd = [self.hermes_path, "-z", prompt]  # oneshot mode
        if model:
            cmd.extend(["-m", model])
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
        return result.stdout.strip()  # plain text, not JSON
```

**Important**: 
- ✅ `hermes -z "prompt"` — oneshot mode, returns plain text
- ❌ `hermes ask --json "prompt"` — `ask` subcommand does not exist
- ✅ `-m` for model override
- ❌ `--model` not recognized

**Performance**: 15-17s per call (Python process startup + config loading + plugins + API call)

### Why HttpLLMRuntime Is Faster

`hermes -z` spawns a full Python process that:
1. Imports hermes CLI modules (~3-5s)
2. Loads config.yaml and .env (~1s)
3. Initializes plugins and skills (~2-3s)
4. Makes the actual API call (~7-10s)

`HttpLLMRuntime` skips steps 1-3 entirely — just HTTP POST to the API endpoint.

### When to Use Each

| Scenario | Runtime |
|----------|---------|
| REPL interactive use | HttpLLMRuntime (fast) |
| Script execution (`helen <file>`) | MockLLMRuntime (deterministic) |
| No HTTP API available | HermesCLILLMRuntime (fallback) |
| Need hermes skills/tools | HermesCLILLMRuntime (has full hermes context) |

## HelenHermesRuntime Completion (HLD §3.8.3)

### Skills Scanning
```python
def _find_skill_directories() -> list[str]:
    """Walk ~/.hermes/skills recursively for SKILL.md files."""
    for base in ["~/.hermes/skills", "~/.hermes/hermes-agent/skills"]:
        for root, dirs, files in os.walk(base):
            if "SKILL.md" in files:
                candidates.append(root)

def _parse_skill_frontmatter(path: str) -> dict[str, str]:
    """Parse YAML frontmatter from SKILL.md without external YAML parser."""
    # Manual parsing: find --- markers, split key:value lines
```

### Important: Skills Can Be Nested
Directories like `mlops/inference/serving-llms-vllm/SKILL.md` require recursive walk, not just listing the top-level directory.

## Test Updates for Implemented Stubs

When implementing a previously-stub method (NotImplementedError → real code):
1. **Update existing tests** that expected NotImplementedError
2. **Add new tests** for the real behavior
3. **Don't delete the old test class** — rename it to reflect the new behavior

Example:
```python
# Before: TestNotImplementedMethods with test_*_raises
# After: TestImplementedMethods with test_*_returns_schema
```

## Current Implementation Status (Updated 2026-06)

| Feature | Status | Notes |
|---|---|---|
| Memory Providers | ❌ Incomplete vs HLD v1.2.1 | Current: `get/set/delete/list_keys` (no path param). HLD requires: `load/save/get/set/search` with `path` param. Missing `search`. |
| HelenHermesRuntime skills | ⚠️ Partial | `list_skills`/`load_skill` complete. `load_tool` returns stub ToolSchema. |
| Hermes CLI LLM Runtime | ✅ Complete | route/act via `hermes -z` (oneshot mode) |
| **HttpLLMRuntime (direct API)** | ✅ **Complete** | Direct HTTP to OpenAI-compatible API, auto-loads from `~/.hermes/.env`, REPL default. 7-11s/call. |
| LSP Smart Completion | ⚠️ Partial | Syntax highlight only |

## Known Bugs (2026-06)

### Import Runtime Error
`helen/interpreter/interpreter.py` line 674: `visit_import_stmt` reads `node.path` but AST field is `node.module_path` → `AttributeError`. One-character fix: `node.path` → `node.module_path`.

### catch Syntax
HLD EBNF: `catch IDENTIFIER ("as" IDENTIFIER)?`. Tutorial uses `catch Type(var)` which parser rejects. **Parser implementation**: `catch Type var` or `catch Type` (no binding). The `as` keyword is defined in HLD but **not implemented** in parser — `catch Type as var` produces `E0301`.

### Tutorial `llm if` Uses Wrong Syntax
Tutorials 06 and 10 use `case "x":` inside `llm if` blocks. Implementation requires `branch "x" { }`. `case:` is `match` syntax only.
