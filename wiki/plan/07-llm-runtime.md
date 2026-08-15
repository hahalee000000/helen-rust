# M5 — LLM Runtime: Providers, HTTP, Prompt Building

**Objective:** Port `runtime/{llm_runtime,http_llm,provider_protocol,config,prompt_builder,model_capabilities,token_utils,llm_summarizer,probe}.py`. Exit criterion: `llm act/if` works end-to-end against the same providers/config the Python version supports, with identical request payloads and response parsing.

## Files

```
crates/helen-runtime/src/llm.rs         // LlmRuntime trait + MockLlmRuntime
crates/helen-runtime/src/http_llm.rs    // blocking HTTP + SSE streaming
crates/helen-runtime/src/provider.rs    // PlatformProtocol + 6 providers + custom loader
crates/helen-runtime/src/config.rs      // settings, env, API keys
crates/helen-runtime/src/prompt.rs      // prompt builder + {{var}} renderer
crates/helen-runtime/src/model_caps.rs  // model capabilities
crates/helen-runtime/src/token.rs       // token counting (heuristic + tiktoken-style)
crates/helen-runtime/src/probe.rs       // provider connectivity probe
crates/helen-runtime/src/summarize.rs   // conversation summarizer
```

## Task 5.1: Trait + Mock

```rust
pub trait LlmRuntime: Send + Sync {
  fn route(&self, description: &str, branches: &[String], context: Option<&str>) -> Result<Option<String>, ExceptionValue>;
  fn act(&self, prompt: &str, tools: Option<&[ToolSchema]>, model: Option<&str>,
         temperature: Option<f64>, max_tokens: Option<u64>) -> Result<LlmResponse, ExceptionValue>;
  fn act_stream(&self, prompt: &str, model: Option<&str>) -> Result<Box<dyn Iterator<Item=StreamChunk>>, ExceptionValue>;
  fn reset(&self) -> ();
}
```

`MockLlmRuntime`: deterministic canned text/route results (identical to Python's mock strings) — drives all deterministic tests. `LlmResponse { text, tool_calls, usage, finish_reason, thinking }`.

## Task 5.2: HTTP LLM client (port `http_llm.py`, 1,689 lines)

Blocking client: `ureq` (keep-it-simple) or `reqwest::blocking`. Must port:
- Request building from messages + tools (OpenAI-compatible shape).
- **SSE streaming** (`data: {...}\n\n`), incremental parse into chunks, `usage` extraction, `finish_reason` detection (port `test_finish_reason_detection.py`).
- Timeouts, retry/backoff (port `runtime/resilience.py` policy), cancellation tokens.
- Provider-specific auth headers and endpoint URL patterns.

## Task 5.3: PlatformProtocol (6 providers + openai fallback)

Port `provider_protocol.py` — trait with: `build_request_payload, supports_tool_choice, sanitize_messages, parse_response, parse_streaming_delta, extract_streaming_usage, parse_error, is_context_overflow_error`. Implement for: **DashScope, Volcengine, Zhipu, DeepSeek, Minimax, Kimi, OpenAI** (base). Auto-detection from base_url patterns + protocol_name config.

**Custom provider loading (D8):** Python scripts in `~/.helen/providers/*.py` (subclasses of `PlatformProtocol`) can only execute with Python embedded. Implement a PyO3-gated loader in `helen-ffi` (M10) that: (1) loads the module, (2) finds `PlatformProtocol` subclasses with a unique `name`, (3) registers them in the Rust `PROTOCOL_NAME_MAP`. Without the FFI feature: only built-ins; log a warning for custom providers.

## Task 5.4: Config + prompt builder + model capabilities

- `config.rs`: port `runtime/config.py` (751 lines) — TOML config file, env vars, CLI overrides, API keys, provider defaults, transcript settings. Same lookup precedence.
- `prompt.rs`: two-layer progressive disclosure + template `{{var}}` rendering (port `prompt_builder.py` semantics: system prompt assembly, context injection, escaping).
- `model_caps.rs`: port the model feature matrix (function-calling support, streaming, vision, thinking modes per model).

## Task 5.5: Token counting + summarizer + probe

- `token.rs`: heuristic counter matching Python's fallback (port token_utils); optional accurate counting when `tiktoken` is available via PyO3 (feature-gated, M10).
- `summarize.rs`: LLM-driven summarization flow (port `llm_summarizer.py`).
- `probe.rs`: connectivity probing (port `probe.py`) — used by config/CLI.

## Definition of Done — M5

- [ ] Mock LLM drives all deterministic tests — via the in-process Python reference driver (`tests/conformance/reference.py`, M0.4, D12) on the reference side and `MockLlmRuntime` on the Rust side. No env-var hook is needed.
- [ ] Real `act` + `act_stream` round-trip against at least one configured provider (dev key) with payload/SSE parity verified by recorded fixtures.
- [ ] `--provider-detect` CLI probe output matches Python's.
- [ ] Custom-provider loader works when `helen-ffi` is enabled (checked in M10).
