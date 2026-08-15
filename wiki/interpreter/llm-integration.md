# LLM Integration — helen-rust

> Rust port of `helen/llm/` and the interpreter's `llm act/if` statements.
> Source: `crates/helen-runtime/src/` (llm runtime), `helen-interpreter`.

---

## Architecture

- **`LlmRuntime` trait** — the route/act interface; `Arc<dyn LlmRuntime>`
  forces `&self` methods → implementors use interior mutability
  (`RefCell`/`Mutex`).
- **`MockLlmRuntime`** — deterministic mock used by the differential harness
  (`--mock-llm` is reference-only; the Rust test harness builds the mock
  in-process).
- **Providers** — HTTP + SSE streaming client, config, prompt builder, model
  caps, token counting, tool dispatch.
- **Custom provider adapters** (M10) — Python-side subclass detection
  (`PyType::is_subclass`) + instance instantiation, mirroring the reference's
  `~/.helen/providers/` auto-load.

## `llm act` / `llm if`

- `llm act` — single LLM call with conversation history; tool loop
  (allowlist → schemas → dispatch) in agent context.
- `llm if` — LLM-routed branching; requires a default branch (E0344
  LlmIfNoDefault).
- Streaming — `on_chunk`/`on_complete` callbacks (M3 3.6b).

## Agent settings passthrough

`llm act` in an agent uses the agent's `declarations` settings (model,
temperature, tools) via an `agent_setting(name)` helper — Python
`_get_agent_setting` parity.

## Prompt Building

Two-layer progressive disclosure + template rendering in
`crates/helen-runtime/src/`.
