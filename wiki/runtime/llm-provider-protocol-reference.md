# LLM Provider OpenAI-Compatible Protocol Reference

> Comprehensive reference for the full LLM conversation lifecycle protocol across major Chinese LLM providers.
> Covers: Qwen/DashScope, Zhipu/GLM, DeepSeek, Minimax, Kimi/Moonshot, Doubao/Volcengine Ark.
> Last updated: 2026-08-05

---

## Overview

All major Chinese LLM providers offer **OpenAI-compatible Chat Completions APIs**, allowing drop-in usage with the OpenAI SDK by changing `base_url` and `api_key`. However, each provider has unique extensions, deviations, and quirks. This document covers the **complete conversation lifecycle**: authentication → request → response → streaming → tool calling → reasoning → multi-turn → error handling.

### Quick Comparison Table

| Provider | Base URL | Auth | Thinking Field | Tool Choice | Unique Feature |
|----------|----------|------|----------------|-------------|----------------|
| **Qwen** | `dashscope.aliyuncs.com/compatible-mode/v1` | Bearer token | `enable_thinking` + `reasoning_content` | auto/none/required | `thinking_budget`, native DashScope API |
| **Zhipu** | `open.bigmodel.cn/api/paas/v4` | Bearer token | `thinking.type` + `reasoning_content` | **auto only** | `tool_stream`, forced thinking (GLM-4.7) |
| **DeepSeek** | `api.deepseek.com` | Bearer token | `thinking.type` + `reasoning_content` | auto/none/required | `reasoning_effort`, Responses API, Anthropic compat |
| **Minimax** | `api.minimaxi.com/v1` | Bearer token | `thinking.type` + `reasoning_split` | auto/none | Dual protocol (OpenAI + Anthropic), `service_tier` |
| **Kimi** | `api.moonshot.ai/v1` | Bearer token | `thinking.type` + `reasoning_content` | auto/none/required | `$web_search` builtin, Partial Mode, `prompt_cache_key` |
| **Doubao** | `ark.cn-beijing.volces.com/api/v3` | Bearer token | `thinking.type` + `reasoning_content` | auto/none/required | Endpoint ID (`ep-XXXXX`), Managed Agents API |

---

## 1. Authentication & Base URLs

### 1.1 Standard OpenAI-Compatible Authentication

All providers use the same HTTP header format:

```
Authorization: Bearer <API_KEY>
Content-Type: application/json
```

### 1.2 Base URLs

| Provider | OpenAI-Compatible Base URL | Notes |
|----------|---------------------------|-------|
| **Qwen (DashScope)** | `https://dashscope.aliyuncs.com/compatible-mode/v1` | China endpoint |
| | `https://dashscope-us.aliyuncs.com/compatible-mode/v1` | US/International |
| **Zhipu (GLM)** | `https://open.bigmodel.cn/api/paas/v4` | Also supports Anthropic/Gemini protocol |
| **DeepSeek** | `https://api.deepseek.com` | Also `/anthropic` and `/beta` paths |
| **Minimax** | `https://api.minimaxi.com/v1` (China) | Also `api.minimax.io` (International) |
| | `https://api.minimaxi.com/anthropic` | Anthropic-compatible endpoint |
| **Kimi (Moonshot)** | `https://api.moonshot.ai/v1` | Regional platforms have isolated keys |
| **Doubao (Volcengine)** | `https://ark.cn-beijing.volces.com/api/v3` | Uses endpoint IDs instead of model names |

### 1.3 OpenAI SDK Drop-In Usage

```python
from openai import OpenAI

# All providers work with the same pattern:
client = OpenAI(
    api_key="<YOUR_API_KEY>",
    base_url="<BASE_URL>"
)

response = client.chat.completions.create(
    model="<MODEL_NAME>",  # or endpoint ID for Doubao
    messages=[{"role": "user", "content": "Hello"}]
)
```

---

## 2. Chat Completions — Request Parameters

### 2.1 Common Parameters (All Providers)

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `model` | string | ✅ | Model identifier (or endpoint ID for Doubao) |
| `messages` | array | ✅ | Conversation history `[{role, content}]` |
| `temperature` | number | ❌ | Sampling temperature (range varies by provider) |
| `top_p` | number | ❌ | Nucleus sampling threshold |
| `max_tokens` | integer | ❌ | Max output tokens (deprecated → use `max_completion_tokens`) |
| `max_completion_tokens` | integer | ❌ | New standard for output token limit |
| `stream` | boolean | ❌ | Enable SSE streaming (default: false) |
| `stop` | string\|array | ❌ | Stop sequences |
| `tools` | array | ❌ | Tool/function definitions |
| `tool_choice` | string\|object | ❌ | Tool calling mode |
| `response_format` | object | ❌ | Output format (text/json_object/json_schema) |
| `seed` | integer | ❌ | Random seed for reproducibility |
| `stream_options` | object | ❌ | `{"include_usage": true}` for usage in stream |

### 2.2 Provider-Specific Parameters

| Parameter | Qwen | Zhipu | DeepSeek | Minimax | Kimi | Doubao |
|-----------|------|-------|----------|---------|------|--------|
| `enable_thinking` | ✅ | — | — | — | — | — |
| `thinking.type` | — | ✅ | ✅ | ✅ | ✅ | ✅ |
| `thinking_budget` | ✅ | — | — | — | — | ✅ |
| `reasoning_effort` | — | ✅ | ✅ | — | ✅ (K3) | ✅ |
| `reasoning_split` | — | — | — | ✅ | — | — |
| `tool_stream` | — | ✅ | — | — | — | — |
| `service_tier` | — | — | — | ✅ | — | — |
| `prompt_cache_key` | — | — | — | — | ✅ | — |
| `user_id` | — | — | ✅ | — | — | — |
| `presence_penalty` | ✅ | ✅ | ❌ (deprecated) | ❌ (ignored) | ✅ (v1 only) | ✅ |
| `frequency_penalty` | ✅ | ✅ | ❌ (deprecated) | ❌ (ignored) | ✅ (v1 only) | ✅ |
| `logprobs` | ✅ | — | ✅ | — | ✅ | ✅ |
| `n` | ✅ | — | — | ✅ (=1 only) | ✅ (=1 only) | — |

### 2.3 Temperature Ranges

| Provider | Range | Default | Notes |
|----------|-------|---------|-------|
| Qwen | [0.0, 2.0] | 0.7 | |
| Zhipu | [0.0, 1.0] | model-dependent | GLM-5.x: 1.0, GLM-4.5: 0.6, GLM-4: 0.75 |
| DeepSeek | [0.0, 2.0] | 1 | Ignored in thinking mode |
| Minimax | [0.0, 2.0] | 1 | |
| Kimi (v1) | [0.0, 1.0] | 0.0 | Fixed for K3/K2.x models |
| Doubao | [0.0, 1.0] | ~0.8 | |

---

## 3. Message Format

### 3.1 Standard Roles (All Providers)

| Role | Description |
|------|-------------|
| `system` | System instructions (optional) |
| `user` | User input |
| `assistant` | Model response (may include `tool_calls`) |
| `tool` | Tool execution result (with `tool_call_id`) |

### 3.2 Content Formats

**Plain text:**
```json
{"role": "user", "content": "Hello!"}
```

**Multimodal array (vision):**
```json
{
  "role": "user",
  "content": [
    {"type": "text", "text": "Describe this image"},
    {"type": "image_url", "image_url": {"url": "https://example.com/img.jpg"}}
  ]
}
```

**Image URL formats supported:**
- Public URL: `"url": "https://example.com/image.jpg"`
- Base64 data URI: `"url": "data:image/jpeg;base64,/9j/4AAQ..."`
- File reference (Kimi): `"url": "ms://<file_id>"`
- File reference (Minimax): `"url": "mm_file://<file_id>"`

### 3.3 Tool Call Message (assistant)

```json
{
  "role": "assistant",
  "content": null,
  "tool_calls": [{
    "id": "call_abc123",
    "type": "function",
    "function": {
      "name": "get_weather",
      "arguments": "{\"location\": \"Beijing\"}"
    }
  }]
}
```

> **Important**: `function.arguments` is always a **JSON string** (not a JSON object). Must be parsed.

### 3.4 Tool Result Message

```json
{
  "role": "tool",
  "tool_call_id": "call_abc123",
  "content": "{\"temperature\": \"22°C\", \"condition\": \"sunny\"}"
}
```

---

## 4. Response Format (Non-Streaming)

### 4.1 Standard Response Structure

```json
{
  "id": "chatcmpl-xxx",
  "object": "chat.completion",
  "created": 1699999999,
  "model": "model-name",
  "choices": [{
    "index": 0,
    "message": {
      "role": "assistant",
      "content": "Response text",
      "reasoning_content": "Thinking process...",
      "tool_calls": [...]
    },
    "finish_reason": "stop"
  }],
  "usage": {
    "prompt_tokens": 50,
    "completion_tokens": 100,
    "total_tokens": 150
  }
}
```

### 4.2 finish_reason Values

| Value | Qwen | Zhipu | DeepSeek | Minimax | Kimi | Doubao |
|-------|------|-------|----------|---------|------|--------|
| `stop` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `length` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `tool_calls` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `content_filter` | ✅ | — | ✅ | ✅ | — | ✅ |
| `sensitive` | — | ✅ | — | — | — | — |
| `null` (streaming) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `insufficient_system_resource` | — | — | ✅ | — | — | — |
| `model_context_window_exceeded` | — | ✅ | — | — | — | — |

### 4.3 Provider-Specific Response Fields

| Field | Provider | Description |
|-------|----------|-------------|
| `request_id` | Zhipu | Unique request trace ID |
| `reasoning_content` | Zhipu, DeepSeek, Kimi, Doubao | Chain-of-thought reasoning text |
| `reasoning_details` | Minimax | Structured reasoning array |
| `input_sensitive` / `output_sensitive` | Minimax | Content safety flags |
| `base_resp` | Minimax | Error status object |
| `system_fingerprint` | DeepSeek | Model version fingerprint |
| `logprobs` | Qwen, DeepSeek, Kimi, Doubao | Log probability data |

---

## 5. Streaming Protocol (SSE)

### 5.1 Standard SSE Format

All providers use Server-Sent Events with `text/event-stream` content type.

**Enable streaming:**
```json
{"stream": true, "stream_options": {"include_usage": true}}
```

**SSE chunk format:**
```
data: {"id":"chatcmpl-xxx","object":"chat.completion.chunk","created":1699999999,"model":"model-name","choices":[{"index":0,"delta":{"role":"assistant","content":""},"finish_reason":null}]}

data: {"id":"chatcmpl-xxx","object":"chat.completion.chunk","created":1699999999,"model":"model-name","choices":[{"index":0,"delta":{"content":"Hello"},"finish_reason":null}]}

...

data: {"id":"chatcmpl-xxx","object":"chat.completion.chunk","created":1699999999,"model":"model-name","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":50,"completion_tokens":100,"total_tokens":150}}

data: [DONE]
```

### 5.2 Key Streaming Details

| Aspect | Details |
|--------|---------|
| **Termination** | Always `data: [DONE]` (all providers) |
| **First chunk** | Contains `delta.role: "assistant"` |
| **Delta field** | `delta` (not `message`) — contains incremental content |
| **Usage in stream** | Last chunk before `[DONE]` (requires `stream_options.include_usage: true`) |
| **Intermediate chunks** | `finish_reason: null`, `delta.content` with text fragments |

### 5.3 Streaming with Thinking/Reasoning

All providers that support thinking follow the same pattern:
1. `delta.reasoning_content` streams **first** (chain-of-thought tokens)
2. `delta.content` streams **after** (final answer tokens)
3. They are **mutually exclusive** in each chunk

```
data: {"choices":[{"delta":{"reasoning_content":"Let me think..."}}]}
data: {"choices":[{"delta":{"reasoning_content":"about this..."}}]}
data: {"choices":[{"delta":{"reasoning_content":""}}]}      ← thinking ends
data: {"choices":[{"delta":{"content":"The answer is..."}}]} ← answer begins
data: {"choices":[{"delta":{},"finish_reason":"stop"}]}
data: [DONE]
```

### 5.4 Provider-Specific Streaming Details

| Provider | Notes |
|----------|-------|
| **Qwen** | DashScope native API uses cumulative (non-incremental) mode by default; OpenAI-compatible mode is incremental. Native uses `id:N`, `event:result` format |
| **Zhipu** | `tool_stream: true` streams tool arguments incrementally (GLM-5/5.1/5.2) |
| **DeepSeek** | Usage chunk has `choices: []` (empty array) before `[DONE]` |
| **Minimax** | When `reasoning_split=true`, delta contains `reasoning_details` array |
| **Kimi** | Usage at `choices[0].usage` (NOT top-level `chunk.usage`) — OpenAI SDK quirk |
| **Doubao** | Standard incremental streaming, same as OpenAI format |

---

## 6. Function Calling / Tool Use

### 6.1 Tool Definition Format (Standard — All Providers)

```json
{
  "tools": [{
    "type": "function",
    "function": {
      "name": "get_weather",
      "description": "Get weather for a location",
      "parameters": {
        "type": "object",
        "properties": {
          "location": {
            "type": "string",
            "description": "City name"
          },
          "unit": {
            "type": "string",
            "enum": ["celsius", "fahrenheit"]
          }
        },
        "required": ["location"]
      }
    }
  }]
}
```

### 6.2 tool_choice Values

| Value | Qwen | Zhipu | DeepSeek | Minimax | Kimi | Doubao |
|-------|------|-------|----------|---------|------|--------|
| `"auto"` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| `"none"` | ✅ | ❌ | ✅ | ✅ | ✅ | ✅ |
| `"required"` | ✅ | ❌ | ✅ | ❌ | ✅ | ✅ |
| Specific function | ✅ | ❌ | ✅ | ❌ | ✅ | ✅ |

> **Zhipu limitation**: Only `"auto"` is supported. No `"none"`, `"required"`, or specific function forcing.

### 6.3 Tool Calling Flow

```
1. Request with tools → Model responds with tool_calls (finish_reason: "tool_calls")
2. Execute tool locally
3. Submit result as {role: "tool", tool_call_id: "...", content: "..."}
4. Model generates final response using tool results
```

### 6.4 Provider-Specific Tool Features

| Feature | Provider | Description |
|---------|----------|-------------|
| `$web_search` builtin | Kimi | `type: "builtin_function"` — Kimi executes search internally |
| `tool_stream` | Zhipu | Stream tool arguments incrementally |
| Dynamic tool loading | Kimi (K3) | Insert `{role: "system", tools: [...]}` mid-conversation |
| Parallel tool calls | All | Multiple tool_calls in single response |
| Strict mode | DeepSeek (Beta) | `strict: true` enforces JSON Schema conformance |
| Built-in web search | Doubao | Web search, image processing, knowledge search, MCP tools |
| MFJS schema | Kimi | "Moonshot Flavored JSON Schema" — extended JSON Schema variant |

---

## 7. Reasoning / Thinking Content

### 7.1 Enabling Thinking Mode

| Provider | Parameter | Values |
|----------|-----------|--------|
| **Qwen** | `enable_thinking: true` + `thinking_budget: N` | Boolean + token budget |
| **Zhipu** | `thinking: {type: "enabled"/"disabled"}` + `reasoning_effort` | GLM-5.2+: max/xhigh/high/medium/low/minimal/none |
| **DeepSeek** | `thinking: {type: "enabled"/"disabled"}` + `reasoning_effort` | low/high/max (xhigh also accepted) |
| **Minimax** | `thinking: {type: "adaptive"/"disabled"}` | M3 only; M2.x always thinks |
| **Kimi** | K3: `reasoning_effort`; K2.x: `thinking: {type, keep}` | K3: low/high/max; K2.x: enabled/disabled |
| **Doubao** | `thinking: {type: "enabled"/"disabled"}` + `thinking_budget` | Also `reasoning_effort: low/medium/high` |

### 7.2 Response Fields

| Provider | Non-Streaming | Streaming |
|----------|---------------|-----------|
| **Qwen** | `message.reasoning_content` | `delta.reasoning_content` |
| **Zhipu** | `message.reasoning_content` | `delta.reasoning_content` |
| **DeepSeek** | `message.reasoning_content` | `delta.reasoning_content` |
| **Minimax** | `message.reasoning_content` (if `reasoning_split=true`) | `delta.reasoning_details` |
| **Kimi** | `message.reasoning_content` | `delta.reasoning_content` |
| **Doubao** | `message.reasoning_content` | `delta.reasoning_content` |

### 7.3 Multi-turn Rules for reasoning_content

| Provider | Without Tool Calls | With Tool Calls |
|----------|-------------------|-----------------|
| **Qwen** | Optional to pass back | Recommended |
| **Zhipu** | ⚠️ **NOT visible to model** — reasoning chain breaks | Same issue |
| **DeepSeek** | Ignored if passed | ⚠️ **MUST** pass back or get 400 error |
| **Minimax** | **MUST** preserve full response | **MUST** preserve including reasoning_details |
| **Kimi** | **MUST** preserve for K3 | **MUST** preserve |
| **Doubao** | Recommended | Recommended |

> ⚠️ **Critical**: DeepSeek and Minimax will error or lose context if reasoning_content is not preserved in tool-call multi-turn conversations.

### 7.4 Thinking Token Billing

| Provider | Reasoning Tokens |
|----------|-----------------|
| **Qwen** | Included in `completion_tokens` |
| **Zhipu** | Included in `completion_tokens`, reported in `completion_tokens_details.reasoning_tokens` |
| **DeepSeek** | Reported in `completion_tokens_details.reasoning_tokens` |
| **Minimax** | Reported in `completion_tokens_details.reasoning_tokens` |
| **Kimi** | Count toward `max_tokens` budget |
| **Doubao** | Counted separately from `max_tokens` |

---

## 8. Token Usage Reporting

### 8.1 Standard Usage Object

```json
{
  "usage": {
    "prompt_tokens": 50,
    "completion_tokens": 100,
    "total_tokens": 150
  }
}
```

### 8.2 Extended Usage Fields by Provider

| Field | Provider | Description |
|-------|----------|-------------|
| `cached_tokens` / `prompt_tokens_details.cached_tokens` | Kimi, Doubao, Zhipu | Prompt cache hit tokens |
| `prompt_cache_hit_tokens` / `prompt_cache_miss_tokens` | DeepSeek | Detailed cache breakdown |
| `completion_tokens_details.reasoning_tokens` | DeepSeek, Zhipu, Minimax | Reasoning token count |
| `total_characters` | Minimax | Always 0 for text models |
| `input_tokens` / `output_tokens` | Qwen (native API) | DashScope native format |

### 8.3 Prompt Caching

| Provider | Caching Method | Notes |
|----------|---------------|-------|
| **Qwen** | Automatic | No explicit API needed |
| **Zhipu** | Automatic | `cached_tokens` in usage |
| **DeepSeek** | Automatic | `user_id` for KV cache isolation |
| **Minimax** | Automatic | 512+ token minimum threshold, ~80% discount |
| **Kimi** | Automatic | `prompt_cache_key` for explicit control |
| **Doubao** | Context Cache API | Explicit `ContextChatCompletions` endpoint |

---

## 9. Structured Output

### 9.1 Support Matrix

| Provider | `json_object` | `json_schema` | Notes |
|----------|---------------|---------------|-------|
| **Qwen** | ✅ | ✅ | `json_object` requires "JSON" in prompt |
| **Zhipu** | ✅ | — | |
| **DeepSeek** | ✅ | — | Must instruct model to produce JSON |
| **Minimax** | ✅ (Text-01 only) | ✅ (Text-01 only) | Limited model support |
| **Kimi** | ✅ | ✅ | Uses MFJS schema variant |
| **Doubao** | ✅ | ✅ | |

---

## 10. Multimodal / Vision Support

### 10.1 Vision Support Matrix

| Provider | Vision Models | Content Types | Video |
|----------|--------------|---------------|-------|
| **Qwen** | qwen-vl-max, qwen-vl-plus, qwen3-vl, qvq | text, image_url | Via qvq |
| **Zhipu** | glm-4v-flash (free), glm-4v, glm-5v-turbo | text, image_url | — |
| **DeepSeek** | ❌ (not yet available in API) | — | — |
| **Minimax** | MiniMax-M3 | text, image_url, video_url | ✅ native |
| **Kimi** | kimi-k3, kimi-k2.x | text, image_url, video_url | ✅ native |
| **Doubao** | Doubao-Seed-1.6, Doubao-1.5-thinking-vision-pro | text, image_url | ✅ |

### 10.2 Image Input Formats

| Format | Support |
|--------|---------|
| Public HTTP/HTTPS URL | All providers |
| Base64 data URI (`data:image/png;base64,...`) | All providers |
| File reference (provider-specific) | Kimi (`ms://`), Minimax (`mm_file://`) |

---

## 11. Error Handling

### 11.1 Standard Error Format

```json
{
  "error": {
    "message": "Error description",
    "type": "error_type",
    "param": null,
    "code": "error_code"
  }
}
```

### 11.2 Common HTTP Status Codes

| HTTP Code | Meaning | All Providers |
|-----------|---------|---------------|
| 400 | Invalid request / parameters | ✅ |
| 401 | Authentication failed | ✅ |
| 402 | Insufficient balance | DeepSeek |
| 403 | Permission denied | ✅ |
| 404 | Model/endpoint not found | ✅ |
| 422 | Invalid parameter values | DeepSeek |
| 429 | Rate limit exceeded | ✅ |
| 499 | Client closed request | Kimi |
| 500 | Internal server error | ✅ |
| 503 | Service unavailable | ✅ |
| 504 | Gateway timeout | Kimi (900s) |

### 11.3 Provider-Specific Error Formats

| Provider | Error Format | Notes |
|----------|-------------|-------|
| **Qwen** | Standard + `request_id` | DashScope native uses different format |
| **Zhipu** | `{error: {code, message}}` + `request_id` | Two-layer: HTTP status + business code |
| **DeepSeek** | Standard OpenAI format | 422 for missing reasoning_content in tool calls |
| **Minimax** | `{base_resp: {status_code, status_msg}}` | Non-standard; also `input_sensitive`/`output_sensitive` flags |
| **Kimi** | Standard + many specific error types | content_filter, engine_overloaded, exceeded_current_quota |
| **Doubao** | Standard + `code` field | MissingParameter, InvalidParameter, AuthenticationError |

---

## 12. Model Names & Context Windows

### 12.1 Quick Reference

| Provider | Flagship Models | Max Context |
|----------|----------------|-------------|
| **Qwen** | qwen3.8-max, qwen3-max, qwen-max | 10M (qwen-long) |
| **Zhipu** | glm-5.2, glm-4.7, glm-4.6 | 1M (glm-5.2) |
| **DeepSeek** | deepseek-v4-pro, deepseek-v4-flash | 1M |
| **Minimax** | MiniMax-M3, M2.7, M2.5 | 1M (M3) |
| **Kimi** | kimi-k3, kimi-k2.7-code, kimi-k2.6 | 1M (K3) |
| **Doubao** | Doubao-Seed-2.1-pro, Doubao-pro-256k | 256K |

### 12.2 Legacy Model Names

| Provider | Retired Names | Notes |
|----------|--------------|-------|
| **DeepSeek** | `deepseek-chat`, `deepseek-reasoner` | Retired 2026-07-24, use `deepseek-v4-flash`/`deepseek-v4-pro` |
| **Kimi** | `moonshot-v1-8k/32k/128k` | Legacy, use `kimi-k3`/`kimi-k2.x` |
| **Minimax** | `MiniMax-Text-01`, `abab6.5` | Legacy, use M-series |

---

## 13. Provider-Specific Extensions Summary

### 13.1 Qwen (DashScope)

| Extension | Description |
|-----------|-------------|
| `enable_thinking` | Boolean to enable reasoning mode |
| `thinking_budget` | Max tokens for reasoning chain |
| `incremental_output` | Native API: incremental vs cumulative streaming |
| `X-DashScope-SSE` | Native API: streaming header |
| Native DashScope API | Different request/response format (`input.messages`, `parameters`) |
| `developer` role | ❌ Not supported |

### 13.2 Zhipu (GLM)

| Extension | Description |
|-----------|-------------|
| `thinking.type` | `"enabled"` / `"disabled"` |
| `reasoning_effort` | 7 levels: max/xhigh/high/medium/low/minimal/none |
| `tool_stream` | Stream tool arguments incrementally |
| `tool_choice` limitation | Only `"auto"` supported |
| Forced thinking | GLM-4.7 always thinks (cannot disable) |
| `reasoning_content` bug | ⚠️ Not visible to model in multi-turn — reasoning chain breaks |
| `request_id` | Extra tracing field in response |
| `finish_reason: "sensitive"` | Content safety trigger |
| Temperature range | [0.0, 1.0] only (narrower than OpenAI) |

### 13.3 DeepSeek

| Extension | Description |
|-----------|-------------|
| `thinking.type` | `"enabled"` / `"disabled"` |
| `reasoning_effort` | low/high/max/xhigh |
| `prompt_cache_hit_tokens` | Cache transparency in usage |
| `user_id` | KVCache isolation, content safety |
| `prefix: true` (Beta) | Force model to start with specified content |
| `finish_reason: "insufficient_system_resource"` | Resource interruption |
| Responses API | Full OpenAI Responses format support |
| Anthropic compatibility | `/anthropic` endpoint, Claude model names auto-mapped |
| Legacy retirement | `deepseek-chat`/`deepseek-reasoner` retired 2026-07-24 |

### 13.4 Minimax

| Extension | Description |
|-----------|-------------|
| `reasoning_split` | Separate thinking into `reasoning_content`/`reasoning_details` |
| `reasoning_details` | Structured reasoning array with type, id, format, text |
| `<think>` tags | When `reasoning_split=false`, thinking wrapped in content |
| `thinking.type: "adaptive"` | Minimax-specific thinking mode |
| `service_tier` | `"standard"` / `"priority"` (1.5× cost, faster) |
| `video_url` content type | Native video input |
| `input_sensitive`/`output_sensitive` | Content safety detection |
| `base_resp` | Non-standard error format |
| Dual protocol | Both OpenAI and Anthropic API formats |
| `total_characters` | Always 0 for text models |
| Several params ignored | `presence_penalty`, `frequency_penalty`, `logit_bias` silently ignored |

### 13.5 Kimi (Moonshot)

| Extension | Description |
|-----------|-------------|
| `$web_search` builtin | `type: "builtin_function"` — Kimi executes search internally |
| `partial: true` | Prefill mode — force assistant output prefix |
| `prompt_cache_key` | Explicit cache key for prefix caching |
| `safety_identifier` | User tracking for policy detection |
| Dynamic tool loading (K3) | Insert `{role: "system", tools: [...]}` mid-conversation |
| `thinking.keep: "all"` | Preserved Thinking — keeps historical reasoning_content |
| `reasoning_effort` (K3) | low/high/max controls thinking depth |
| MFJS schema | Moonshot Flavored JSON Schema |
| Usage in stream location | At `choices[0].usage` (NOT top-level) |
| Official tools (Formula API) | `GET /v1/formulas/{uri}/tools` for tool declarations |
| File upload API | `POST /v1/files` with `purpose: "file-extract"` |
| Regional key isolation | Keys from different regional platforms are not interchangeable |

### 13.6 Doubao (Volcengine Ark)

| Extension | Description |
|-----------|-------------|
| Endpoint ID (`ep-XXXXX`) | **Unique**: `model` field uses endpoint ID, not model name |
| `thinking.type` | `"enabled"` / `"disabled"` |
| `thinking_budget` | Max thinking tokens |
| `reasoning_effort` | low/medium/high |
| Managed Agents API | Full agent orchestration: Sessions, Vaults, Memory, Skills |
| Context Cache API | Explicit caching endpoint |
| Responses API | Newer alternative to Chat Completions |
| Built-in tools | Web search, image processing, knowledge search, MCP |
| AK/SK auth (management) | Endpoint management uses different auth than inference |
| Third-party models | Hosts DeepSeek, GLM, Kimi etc. via same endpoint system |

---

## 14. Helen Implementation Notes

Helen's HTTP LLM runtime (`helen/runtime/http_llm.py`) must handle the following provider differences:

### 14.1 Current Handling

| Aspect | Helen's Approach |
|--------|-----------------|
| `reasoning_content` | Separated from `content` in streaming; fallback if content empty |
| `finish_reason: "length"` | Warning logged, `_truncated` flag set |
| Empty responses | Nudge retry with error message, then raise RuntimeError |
| Tool calls | Standard OpenAI format parsing |
| Streaming | SSE with `data: [DONE]` termination |
| **Multi-turn reasoning** (v1.37) | `reasoning_content` preserved in tool-call multi-turn (both `act()` and `act_stream()`) |
| **Endpoint ID validation** (v1.37) | `VolcengineProtocol._validate_endpoint_id()` logs debug warning for non-`ep-` model IDs |
| **Platform protocol** (v1.35) | Auto-detected from `base_url` via `detect_protocol()` |
| **Model capabilities** (v1.35) | Feature detection from `model_id` via `get_model_capabilities()` |
| **Thinking mode** (v1.36) | `thinking-mode`/`reasoning-effort` agent declarations mapped to provider params |

### 14.2 Known Issues & Resolved

| Issue | Provider | Status | Description |
|-------|----------|--------|-------------|
| Zhipu `reasoning_content` bug | Zhipu | Open | Multi-turn reasoning chain breaks (provider-side issue, cannot fix in Helen) |
| Kimi usage location | Kimi | Resolved (v1.35) | `KimiProtocol.extract_streaming_usage()` handles `choices[0].usage` |
| Minimax `reasoning_split` | Minimax | Resolved (v1.35) | `MinimaxProtocol` sets `reasoning_split=true`, parses `reasoning_details` |
| DeepSeek tool-call reasoning | DeepSeek | Resolved (v1.37) | `reasoning_content` now preserved in tool-call multi-turn |
| Doubao endpoint ID | Doubao | Resolved (v1.37) | `VolcengineProtocol._validate_endpoint_id()` validates and warns |
| Zhipu `tool_choice` | Zhipu | Resolved (v1.35) | `ZhipuProtocol.supports_tool_choice()` detects `"auto"` only |
| Doubao `encrypted_content` | Doubao | Resolved (v1.35) | `VolcengineProtocol.parse_response()` prioritizes `encrypted_content` |

### 14.3 Configuration Example

```yaml
# ~/.helen/config.yaml
llm:
  # Qwen (DashScope)
  base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1"
  api_key: "sk-xxx"
  model: "qwen3.7-plus"

  # DeepSeek
  # base_url: "https://api.deepseek.com"
  # model: "deepseek-v4-flash"

  # Kimi (Moonshot)
  # base_url: "https://api.moonshot.ai/v1"
  # model: "kimi-k3"

  # Zhipu (GLM)
  # base_url: "https://open.bigmodel.cn/api/paas/v4"
  # model: "glm-5.2"

  # Minimax
  # base_url: "https://api.minimaxi.com/v1"
  # model: "MiniMax-M3"

  # Doubao (Volcengine)
  # base_url: "https://ark.cn-beijing.volces.com/api/v3"
  # model: "ep-20240918xxxxx-xxxxx"  # Endpoint ID!
```

---

## 15. Provider Auto-Detection & Custom Adapters (v1.40.1)

### Automatic Protocol Detection

Helen automatically detects the correct protocol for your provider during `helen init`:

| Step | What Happens | When |
|------|-------------|------|
| 1. URL match | Check base_url against `_PLATFORM_PATTERNS` (substring match) | Always |
| 2. Layer 1 probe | Send minimal `chat/completions` request | Unknown URL |
| 3. Layer 2 probe | Try each protocol's thinking format | User opts in |
| 4. Layer 3 probe | Test vision (1×1 PNG) + tool_choice | User opts in |

The detected protocol is saved in `config.yaml` as the `protocol` field, so subsequent runs use it directly without re-detection.

### Error Classification

| Error Type | HTTP Status | Message |
|-----------|-------------|---------|
| Connection failure | (no response) | ❌ Cannot connect to {url} |
| Authentication | 401, 403 | ❌ Invalid API Key |
| Model not found | 404 (model in message) | ❌ Model not found |
| Protocol mismatch | 200 (unparseable) | ⚠️ Protocol not compatible |

Hard errors (connection, auth, model) prevent config save. Protocol mismatches trigger the deep probe option.

### Custom Provider Generation

For providers not in the built-in list, use `helen agent` to create a custom adapter:

```bash
# Prerequisites: A working Helen environment (configured with any known provider)
helen agent
```

Ask the agent to analyze the provider's API documentation and generate a `PlatformProtocol` subclass. The agent will:
1. Use `web_fetch` to read the provider's API docs
2. Use `web_search` to find additional information
3. Generate a Python adapter class
4. Save it to `~/.helen/providers/<name>.py`

```
# Example agent prompt:
Please generate a Helen provider adapter for the Anthropic API.
Inherit from OpenAIProtocol and override methods that differ.
Save to ~/.helen/providers/anthropic.py
```

Generated adapters inherit from `OpenAIProtocol` and override only the methods that differ. Always review generated code before production use.

```bash
# List installed custom providers
helen provider list
```

---

## 16. Sources

### Official Documentation Links

| Provider | Documentation |
|----------|--------------|
| **Qwen** | [help.aliyun.com/zh/model-studio](https://help.aliyun.com/zh/model-studio/compatibility-of-openai-with-dashscope) |
| **Zhipu** | [docs.bigmodel.cn](https://docs.bigmodel.cn/cn/guide/develop/openai/introduction) |
| **DeepSeek** | [api-docs.deepseek.com](https://api-docs.deepseek.com/) |
| **Minimax** | [platform.minimaxi.com](https://platform.minimaxi.com/docs/api-reference/text-openai-api) |
| **Kimi** | [platform.kimi.ai/docs](https://platform.kimi.ai/docs/api/chat) |
| **Doubao** | [volcengine.com/docs/82379](https://www.volcengine.com/docs/82379/1494384) |
