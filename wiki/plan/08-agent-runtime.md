# M6 — Agent Runtime: `llm act/if`, Agent Execution, Tools, Skills

**Objective:** Port `interpreter/agent_context.py` + `interpreter/llm_mixin.py` + `runtime/tools.py` + skills system. Exit criterion: `examples/chatbot.helen`, `examples/test_llm_act.helen`, and the `tests/agent` suite pass with Mock and (optionally) real LLM.

## Files

```
crates/helen-interpreter/src/agent.rs     // agent decl evaluation, _call_agent, scope isolation
crates/helen-interpreter/src/llm.rs       // llm act / llm if / llm branch / tool loop / for await
crates/helen-runtime/src/tools.rs         // tool registry + 11 tools
crates/helen-runtime/src/skills.rs        // skill search + two-layer disclosure
crates/helen-runtime/src/agent_session.rs // session tracking for agents (port agent_context pieces)
```

## Task 6.1: Agent declaration + call semantics (port from `agent_context.py` + `interpreter.py`)

Agent struct: `name, params, description, prompt, model, temperature, max_turns, tools: Vec<String>, skills, functions: Vec<FnDecl>, main_body, context_config, memory_config, transcript_config, streaming`.

Call flow (`_call_agent`):
1. Fresh `Environment` (module-level `const` visible read-only; module-level `let` hidden; `shared let` writable — see M3.4).
2. Bind params; **reference-typed params → ReadOnlyView**; last `Channel`-typed param auto-injects endpoint on spawn (M7).
3. Set up agent context (session_id, transcript store, history, compression hooks — wired to M8).
4. Execute `main_body`; return value (converted to Python-compatible when called through the bridge).

## Task 6.2: `llm act` / `llm if` / `llm branch` (port `llm_mixin.py`, 1,926 lines)

- `llm act "prompt"` → `LlmRuntime::act`, return text; on `tools` attribute → **tool-calling loop**: model request with tool schemas → parse `tool_calls` → execute registered tool → append results → re-request until finish or `max_turns`. Port the loop structure and tool-result message format exactly.
- `llm act ... { on_chunk(chunk) { } on_complete(result) { } }` — streaming callbacks (port `visit_llm_act_streaming`).
- `llm if "desc" { branch "a" { } branch "b" { } }` → `route()`; port branch matching semantics.
- `llm act target "..."` (task target) support.
- Cancellation: `llm_control` stdlib (M4) wiring.
- History/context: port message-history accumulation + `conversation_summary` hooks (M8 integration).

## Task 6.3: Tool registry (port `runtime/tools.py`)

```rust
pub struct ToolSchema { pub name: String, pub description: String, pub parameters: serde_json::Value }
pub trait ToolCallable: Send + Sync { fn call(&self, args: serde_json::Value) -> Result<String, ToolError>; }
pub struct ToolRegistry { tools: HashMap<String, RegisteredTool> }
```

Port exactly the **11 built-in tools** (JSON schemas must match Python for LLM parity):
`web_search, web_fetch, read_file, write_file, shell_exec, calculate, patch_file, find_files, search_files, load_skill, list_skill_references`.
Also port: `shell_exec` security defaults (`shell=false`, timeout, PID/signal validation), `web_search` result formatting, `patch_file` fuzzy matching (port `runtime/fuzzy_match.py` — 9 strategies), `find_files` glob (use `globset`/`walkdir`).
MCP tools are appended by M9.

## Task 6.4: Skills system (port three-layer search + two-layer disclosure)

Search layers: (1) workspace `.helen/skills`, (2) project skills dir, (3) user `~/.helen/skills` + bundled `helen-rust/skills/`. Disclosure: `load_skill` returns SKILL.md frontmatter + body summary; `list_skill_references` lists `references/`. Copy the existing skill content from `~/helen/helen/skills/` as data (they are already packaged skill texts).

## Task 6.5: Agent-level tests

Port `tests/agent/` (10 files): scope isolation, tool whitelist authorization (tools list gates which tools the LLM may call vs which functions the program may call), sub-agents, memory, context blocks.

## Definition of Done — M6

- [ ] `examples/chatbot.helen` runs with Mock LLM; tool-calling loop with Mock returns tool results in messages.
- [ ] 11 tools: schemas byte-identical to Python; calls return identical strings.
- [ ] Skills search/disclosure parity on a sample skill.
- [ ] `tests/agent` differential corpus passes.
