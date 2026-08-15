---
name: helen-custom-provider
description: "Write custom LLM provider adapters — PlatformProtocol subclasses auto-loaded from ~/.helen/providers/ to support OpenAI-compatible providers with quirks"
version: 1.41.0
author: Helen Team
license: MIT
metadata:
  hermes:
    tags: [helen, provider, llm, protocol, custom-adapter, openai-compatible, v1.41]
---
<!-- helen-rust edition: custom providers loaded via python-ffi (M10) — Python subclasses detected through PyType::is_subclass and instantiated in-process; requires the python-ffi feature. -->


# Helen Custom Provider Guide

Write a `PlatformProtocol` subclass so Helen can talk to an **OpenAI-compatible provider with quirks**. Custom provider files are auto-discovered from `~/.helen/providers/*.py` and registered at runtime — no source edits required.

## 🎯 Scope — When This Skill Applies

A custom provider is the right tool when:

✅ The target provider exposes `/chat/completions` (or equivalent) with **OpenAI-compatible JSON**
✅ There are small **protocol quirks**: non-standard thinking fields, unusual error format, streaming usage at a different path, restricted `tool_choice`, etc.
✅ You want users to `helen init` (or hand-edit `~/.helen/config.yaml`) with `protocol: "your_name"` and have it Just Work

This skill does **not** cover:

❌ Providers with entirely different HTTP endpoints (Anthropic Messages API at `/v1/messages`, Google Gemini native API, etc.) — those need a different extension point
❌ Modifying the HTTP transport layer itself (auth header schemes, base path rewrites) — `PlatformProtocol` transforms payloads/responses, not requests-at-the-wire level

> 💡 Rule of thumb: if you can `curl` the provider with a payload shaped like OpenAI's `chat/completions` and get back a response shaped like OpenAI's, a custom provider works. Otherwise you need deeper surgery.

See `references/platform-protocol-api.md` for the full method reference.

---

## 📁 File Convention

| Item | Convention |
|------|------------|
| Directory | `~/.helen/providers/` |
| File name | `<name>.py` (snake_case, no leading `_` or `.`) |
| Class name | `<Name>Protocol` (PascalCase, optional) |
| Class attribute | **`name = "<identifier>"`** — **required**, must be unique, used in `config.yaml`'s `protocol:` field |

The file is a normal Python module. It will be loaded with `importlib` into a synthetic module namespace. Top-level imports from `helen.runtime.provider_protocol` work.

```bash
# Create the directory (one-time)
mkdir -p ~/.helen/providers

# List installed providers
helen provider list
```

---

## 🧬 Minimal Skeleton (OpenAI-compatible, no quirks)

Use this when the target provider is **100% OpenAI-compatible** and you just want a stable `protocol:` name in `config.yaml`:

```python
# ~/.helen/providers/my_llm.py
from helen.runtime.provider_protocol import PlatformProtocol


class MyLLMProtocol(PlatformProtocol):
    name = "my_llm"
    # Every method inherits the default OpenAI implementation.
```

That's it. Save, restart the REPL (or call `detect_protocol()` again), and use:

```yaml
# ~/.helen/config.yaml
llm:
  base_url: "https://my-llm.example.com/v1"
  api_key: "sk-..."
  model: "my-model"
  protocol: "my_llm"   # <-- matches the `name` above
```

---

## 🌳 Decision Tree — Which Methods to Override

Most custom providers only need **1–3 overrides**. Walk the tree:

```
Does the provider use a non-standard thinking / reasoning field?
  └── YES → override build_request_payload()
       (see: DashScope's enable_thinking, DeepSeek's thinking.type, etc.)

Does the provider restrict tool_choice values (e.g., only "auto")?
  └── YES → override supports_tool_choice()

Does the provider return reasoning content in a different field name?
  └── YES → override parse_response() and/or parse_streaming_delta()

Is streaming usage at choices[0].usage instead of chunk["usage"]?
  └── YES → override extract_streaming_usage()

Does the provider use a custom error format?
  └── YES → override parse_error()
            + override is_context_overflow_error() / is_auth_error() if needed

None of the above?
  └── Use the minimal skeleton. No overrides needed.
```

Each override corresponds to one row in the API table below. Override only what's actually different — the defaults follow standard OpenAI behavior.

---

## 📋 PlatformProtocol API Summary

| Method | Default behavior | Override when |
|--------|------------------|---------------|
| `name` (class attr) | `"openai"` | **Always** — required for every custom provider |
| `build_request_payload(base_payload, *, model_id, thinking_enabled, reasoning_effort)` | Returns payload as-is | Provider uses non-standard thinking / reasoning fields |
| `supports_tool_choice(value)` | Returns `True` for all | Provider restricts tool_choice (e.g., Zhipu: only `"auto"`) |
| `sanitize_messages(messages)` | Returns messages as-is | Provider rejects certain message fields |
| `parse_response(response_data)` | Extracts from `choices[0].message` | Provider puts content/reasoning/tools in non-standard paths |
| `parse_streaming_delta(delta, context)` | Extracts from delta dict | Provider's SSE chunk format differs |
| `extract_streaming_usage(chunk)` | Reads `chunk["usage"]` | Usage lives at `choices[0].usage` or elsewhere |
| `parse_error(status_code, response_body)` | Reads `error.message` | Provider uses a different error envelope |
| `is_context_overflow_error(error_msg)` | Matches 6 common markers | Provider uses unique wording for context overflow |
| `is_auth_error(error_msg)` | (inherited) | Provider uses unique wording for auth failures |

**Full signatures + examples:** `references/platform-protocol-api.md`

---

## 🔄 End-to-End Workflow

From user need to working provider, in 5 steps:

### Step 1 — Verify OpenAI compatibility

Ask the user for (or look up) a working `curl` example:

```bash
curl -X POST "https://provider.example.com/v1/chat/completions" \
  -H "Authorization: Bearer $KEY" \
  -H "Content-Type: application/json" \
  -d '{"model": "x", "messages": [{"role": "user", "content": "hi"}]}'
```

If this returns `choices[0].message.content` in standard shape → proceed.
Otherwise → out of scope for this skill.

### Step 2 — Identify quirks

Compare provider docs against OpenAI. Common quirks to ask about:

- **Thinking format**: `enable_thinking`? `thinking: {type: "enabled"}`? `thinking.type`?
- **Tool choice**: any value besides `"auto"`, `"none"`, `"required"` supported?
- **Streaming usage**: top-level `usage`, or inside `choices[0]`?
- **Error format**: `error.message`, or something else?
- **Reasoning content**: `reasoning_content`, `thinking`, `encrypted_content`?

### Step 3 — Write the provider file

Create `~/.helen/providers/<name>.py` with the overrides from the decision tree.

See `references/examples/` for three worked examples:
- `minimal.py` — zero overrides (pure OpenAI-compatible)
- `private_llm.py` — thinking field + error parsing overrides (most common case)
- `streaming_usage.py` — streaming usage location override

### Step 4 — Update config.yaml

```yaml
llm:
  base_url: "https://provider.example.com/v1"
  api_key: "sk-..."
  model: "model-name"
  protocol: "<name>"   # matches the `name` in your Python file
```

Or run `helen init` and answer the wizard prompts. If `helen init` doesn't recognize the URL, it will probe and ask to save with a custom protocol name.

### Step 5 — Verify

```bash
# 1. Confirm the provider file is visible
helen provider list
# Expected: "• <name>  (/home/you/.helen/providers/<name>.py)"

# 2. Start a REPL and test detection
helen repl
>>> from helen.runtime.provider_protocol import detect_protocol
>>> p = detect_protocol("https://provider.example.com/v1", protocol_name="<name>")
>>> p.name
'<name>'

# 3. Try a real LLM call
>>> llm_call("hello")
```

If step 2 returns your custom protocol's name but step 3 still acts as OpenAI, you forgot to set `protocol:` in `config.yaml` (or passed the wrong `protocol_name`).

---

## ⚠️ Common Pitfalls

**Forgetting `name` on the subclass.** The base `PlatformProtocol` has `name = "openai"` — a subclass without its own `name` silently inherits it and gets skipped as a built-in conflict. Always write `name = "your_name"` explicitly.

**Naming collision with built-ins.** Built-in names (`openai`, `dashscope`, `volcengine`, `zhipu`, `deepseek`, `minimax`, `kimi`) cannot be overridden. Pick a unique `name`.

**Forgetting to restart / re-detect.** Custom providers are loaded lazily on the next `detect_protocol()` call. For a running REPL, you may need to restart or create a new `HttpLLMRuntime`. The file mtime is checked — editing a file in-place triggers reload.

**Importing from `helen.runtime.provider_protocol` at top-level.** This is fine — the dynamic loader makes the base class available. You can also inherit from built-in protocols like `OpenAIProtocol` when you only need to tweak a few things.

**Testing in isolation.** Use `pytest` with `monkeypatch` to redirect `_get_providers_dir` to `tmp_path` — don't test against real `~/.helen/`. See `tests/runtime/test_provider_protocol.py::TestCustomProviderLoading` for patterns.

---

## 📚 References

- `references/platform-protocol-api.md` — full method signatures with real-world examples from all 7 built-in protocols
- `references/examples/minimal.py` — zero-override template
- `references/examples/private_llm.py` — thinking + error overrides
- `references/examples/streaming_usage.py` — streaming usage override
- Source of truth: `helen/runtime/provider_protocol.py`
- Loader source: `helen/runtime/provider_protocol.py::_load_custom_providers`
- Wiki: `wiki/runtime/llm-provider-protocol-reference.md` (protocol comparison across 6 providers)
