# PlatformProtocol API Reference

> Full method reference for `helen.runtime.provider_protocol.PlatformProtocol`.
> For each method: signature, default behavior, return type, when to override, and real-world examples from the 7 built-in protocols.
>
> Last updated: v1.40.1

---

## Class Declaration

```python
from helen.runtime.provider_protocol import PlatformProtocol


class MyProtocol(PlatformProtocol):
    name = "my_protocol"   # REQUIRED: unique string, used in config.yaml's `protocol:` field
```

The `name` attribute **must be defined on the subclass itself** (not inherited). A subclass without an explicit `name` is skipped with a warning.

---

## `build_request_payload`

Transform the base OpenAI-compatible payload into the provider's specific format before sending.

### Signature

```python
def build_request_payload(
    self,
    base_payload: dict[str, Any],
    *,
    model_id: str,
    thinking_enabled: bool = False,
    reasoning_effort: str | None = None,
) -> dict[str, Any]:
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `base_payload` | `dict` | The OpenAI-compatible payload (already contains `model`, `messages`, `tools`, `temperature`, `max_tokens`, etc.). **Mutate in-place or copy** — either works. |
| `model_id` | `str` | The model identifier, useful for model-specific logic (e.g., endpoint ID validation). |
| `thinking_enabled` | `bool` | Whether the user/agent has requested thinking mode. |
| `reasoning_effort` | `str \| None` | `"low"` / `"medium"` / `"high"` / `"max"` / `None`. |

### Returns

The payload dict (possibly modified in-place). Must remain a valid JSON-serializable dict.

### Default Behavior

Returns `base_payload` unchanged. Equivalent to standard OpenAI.

### When to Override

- Provider uses a non-standard thinking/reasoning field name
- Provider needs extra fields added unconditionally (e.g., `reasoning_split: true`)
- Provider needs model-specific transformations

### Real-World Examples

**DashScope (Qwen)** — `enable_thinking` + `thinking_budget`:

```python
def build_request_payload(self, base_payload, *, model_id,
                          thinking_enabled=False, reasoning_effort=None):
    if thinking_enabled:
        base_payload["enable_thinking"] = True
        if reasoning_effort:
            budget_map = {"low": 1024, "medium": 4096, "high": 16384, "max": 32768}
            base_payload["thinking_budget"] = budget_map.get(reasoning_effort, 4096)
    return base_payload
```

**DeepSeek** — `thinking.type` + top-level `reasoning_effort`:

```python
def build_request_payload(self, base_payload, *, model_id,
                          thinking_enabled=False, reasoning_effort=None):
    if thinking_enabled:
        base_payload["thinking"] = {"type": "enabled"}
    if reasoning_effort:
        base_payload["reasoning_effort"] = reasoning_effort
    return base_payload
```

**Volcengine (Doubao)** — endpoint ID validation + `thinking.type`:

```python
def build_request_payload(self, base_payload, *, model_id,
                          thinking_enabled=False, reasoning_effort=None):
    self._validate_endpoint_id(model_id)   # custom helper
    if thinking_enabled:
        base_payload["thinking"] = {"type": "enabled"}
    return base_payload
```

**MiniMax** — unconditional `reasoning_split: true`:

```python
def build_request_payload(self, base_payload, *, model_id,
                          thinking_enabled=False, reasoning_effort=None):
    base_payload["reasoning_split"] = True   # always required
    if thinking_enabled:
        base_payload["thinking"] = {"type": "adaptive"}
    return base_payload
```

---

## `supports_tool_choice`

Check if the provider accepts a given `tool_choice` value.

### Signature

```python
def supports_tool_choice(self, value: str) -> bool:
```

### Default Behavior

Returns `True` for all values. OpenAI supports `"auto"`, `"none"`, `"required"`, and specific function names.

### When to Override

Provider restricts `tool_choice` to a subset.

### Real-World Example

**Zhipu (GLM)** — only `"auto"` is accepted:

```python
def supports_tool_choice(self, value: str) -> bool:
    return value == "auto"
```

---

## `sanitize_messages`

Transform the messages array into the provider's expected shape.

### Signature

```python
def sanitize_messages(self, messages: list[dict[str, Any]]) -> list[dict[str, Any]]:
```

### Default Behavior

Returns messages unchanged.

### When to Override

- Provider rejects certain fields that OpenAI accepts (e.g., extra metadata)
- Provider requires field renames (rare — most OpenAI-compatible providers accept the standard shape)

---

## `parse_response`

Parse a non-streaming chat completion response into Helen's standard shape.

### Signature

```python
def parse_response(self, response_data: dict[str, Any]) -> dict[str, Any]:
```

### Returns

```python
{
    "content": str,                   # the assistant's reply
    "reasoning_content": str,         # chain-of-thought text (empty if not a thinking response)
    "tool_calls": list[dict],         # list of tool call objects (OpenAI format)
    "finish_reason": str,             # "stop" | "length" | "tool_calls" | ...
    "usage": dict,                    # {"prompt_tokens", "completion_tokens", "total_tokens"}
}
```

Additional keys (e.g., `reasoning_details`) may be added for provider-specific downstream consumers.

### Default Behavior

```python
choice = response_data.get("choices", [{}])[0]
message = choice.get("message", {})
return {
    "content": message.get("content", ""),
    "reasoning_content": message.get("reasoning_content", ""),
    "tool_calls": message.get("tool_calls", []),
    "finish_reason": choice.get("finish_reason", "stop"),
    "usage": response_data.get("usage", {}),
}
```

### When to Override

- Reasoning content is in a different field (e.g., `encrypted_content`, `thinking`)
- Usage is at an unusual location
- Response includes structured `reasoning_details` the user wants preserved

### Real-World Examples

**Volcengine** — `encrypted_content` takes priority:

```python
def parse_response(self, response_data):
    result = super().parse_response(response_data)
    choice = response_data.get("choices", [{}])[0]
    message = choice.get("message", {})
    encrypted = message.get("encrypted_content")
    if encrypted:
        result["reasoning_content"] = encrypted
    return result
```

**MiniMax** — preserve `reasoning_details`:

```python
def parse_response(self, response_data):
    result = super().parse_response(response_data)
    choice = response_data.get("choices", [{}])[0]
    message = choice.get("message", {})
    if "reasoning_details" in message:
        result["reasoning_details"] = message["reasoning_details"]
    return result
```

---

## `parse_streaming_delta`

Parse a single SSE chunk's `delta` into Helen's standard shape.

### Signature

```python
def parse_streaming_delta(
    self,
    delta: dict[str, Any],
    context: dict[str, Any],
) -> dict[str, Any]:
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `delta` | `dict` | The `choices[0].delta` from one SSE chunk. |
| `context` | `dict` | **Mutable** state dict that persists across chunks in the same stream. Use it to track cumulative values, partial tool calls, etc. |

### Returns

```python
{
    "content": str,             # incremental content delta
    "reasoning_content": str,   # incremental reasoning delta
    "tool_calls": list[dict],   # incremental tool call deltas
    "finish_reason": str | None,
}
```

### Default Behavior

Directly extracts the four fields from `delta`.

### When to Override

- Provider sends **cumulative** reasoning content (must compute the incremental delta)
- Provider's reasoning and content fields are mutually exclusive per chunk and need merging

### Real-World Example

**MiniMax** — `reasoning_details` is cumulative, must compute delta:

```python
def parse_streaming_delta(self, delta, context):
    result = {
        "content": delta.get("content", ""),
        "reasoning_content": delta.get("reasoning_content", ""),
        "tool_calls": delta.get("tool_calls", []),
        "finish_reason": delta.get("finish_reason"),
    }
    if "reasoning_details" in delta:
        prev_total = context.get("reasoning_details_total", "")
        current_total = delta["reasoning_details"]
        if isinstance(current_total, str) and current_total.startswith(prev_total):
            result["reasoning_content"] = current_total[len(prev_total):]
        context["reasoning_details_total"] = current_total
    return result
```

---

## `extract_streaming_usage`

Extract usage information from a streaming chunk.

### Signature

```python
def extract_streaming_usage(self, chunk: dict[str, Any]) -> dict[str, Any] | None:
```

### Default Behavior

```python
return chunk.get("usage")
```

### When to Override

Provider places `usage` at a non-standard location in the SSE chunk.

### Real-World Example

**Kimi (Moonshot)** — usage at `choices[0].usage`, not top-level:

```python
def extract_streaming_usage(self, chunk):
    choices = chunk.get("choices", [])
    if choices:
        usage = choices[0].get("usage")
        if usage:
            return usage
    return chunk.get("usage")
```

---

## `parse_error`

Transform a provider-specific error response into a human-readable string.

### Signature

```python
def parse_error(self, status_code: int, response_body: dict[str, Any]) -> str:
```

### Default Behavior

```python
error = response_body.get("error", {})
if isinstance(error, dict):
    return error.get("message", str(response_body))
return str(response_body)
```

### When to Override

Provider uses a different error envelope (e.g., nested `error.detail`, top-level `message`, etc.).

---

## `is_context_overflow_error`

Classify whether an error message indicates context-window overflow.

### Signature

```python
def is_context_overflow_error(self, error_msg: str) -> bool:
```

### Default Behavior

Matches six common markers (case-insensitive):
- `"context length"`, `"maximum context"`, `"too many tokens"`, `"reduce your prompt"`, `"context overflow"`, `"max_tokens"`

### When to Override

Provider uses unique wording for context overflow.

---

## `is_auth_error`

Classify whether an error message indicates authentication failure.

### Signature

```python
def is_auth_error(self, error_msg: str) -> bool:
```

### Default Behavior

Matches common auth markers (inherited from base).

### When to Override

Provider uses unique wording for auth failures.

---

## Class Inheritance Tips

### Inherit from `PlatformProtocol` for full customization

```python
class MyProtocol(PlatformProtocol):
    name = "my_protocol"
    # Override any subset of the methods above.
```

### Inherit from `OpenAIProtocol` as stylistic sugar

`OpenAIProtocol` is an empty subclass of `PlatformProtocol` — same behavior, different name. Use it when you want to signal "this is OpenAI-compatible with tweaks":

```python
from helen.runtime.provider_protocol import OpenAIProtocol


class MyProtocol(OpenAIProtocol):
    name = "my_protocol"
    def build_request_payload(self, ...): ...
```

### Inherit from another built-in protocol to share behavior

If your provider is close to an existing one, inherit and tweak:

```python
from helen.runtime.provider_protocol import DeepSeekProtocol


class MyDeepSeekFork(DeepSeekProtocol):
    name = "my_ds_fork"
    # Inherits DeepSeek's thinking.type handling, overrides something else
```

---

## Detection Priority

`detect_protocol(base_url, protocol_name=None)` resolves in this order:

1. **Custom providers** loaded from `~/.helen/providers/*.py` (by `protocol_name` match)
2. **Built-in by name** (if `protocol_name` matches a built-in: `dashscope`, `volcengine`, `zhipu`, `deepseek`, `minimax`, `kimi`, `openai`)
3. **URL pattern match** against `_PLATFORM_PATTERNS`
4. **Fallback** to `OpenAIProtocol`

Custom providers and built-in names share the same `_PROTOCOL_NAME_MAP`, so a custom provider with `name = "my_protocol"` is resolved exactly like a built-in one.

---

## Loader Behavior

- Files are loaded via `importlib.util.spec_from_file_location`
- Errors in user files are **logged and skipped** — they don't crash the runtime
- Built-in names (`openai`, `dashscope`, etc.) **cannot be overridden** — conflicts are skipped with a debug log
- Cache is mtime-based: editing/adding/removing a file triggers re-scan on next `detect_protocol()` call
- Files starting with `_` or `.`, and `__init__.py`, are ignored
