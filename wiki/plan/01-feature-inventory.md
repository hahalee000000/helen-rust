# Feature Inventory — Complete Map of Python → Rust

Source: `~/helen/` v1.44.0 (module line counts from `wc -l`, verified). Use this as the
conformance checklist. Every item must have a Rust counterpart with tests.

## 1. Core Frontend → `crates/helen-core` + `crates/helen-parser`

| Python module | Lines | Rust target | Must port |
|---|---|---|---|
| `core/source.py` | 129 | `src/source.rs` | `SourceSpan`, source ranges, line/col tracking |
| `core/tokens.py` | 321 | `src/tokens.rs` | 88 `TokenType` variants; **99 bilingual keywords** (48 EN + 51 CN) incl. `agent/智能体`, `spawn/分生`, `shared/共享`, `transcript/记录`; token structs |
| `core/lexer.py` | 960 | `src/lexer.rs` | maximal munch, string escapes, triple-quoted strings, `{{ }}` template delimiters, CJK identifiers, comments, numbers (int/float), operators incl. `|>` `..` `->` |
| `core/ast.py` | 1459 | `src/ast.rs` | 61 node structs + `AstPrinter` (S-expression) |
| `core/errors.py` | 213 | `src/errors.rs` | `LexError`, `ParseError`, `SyntaxError` with spans, error codes (E0xxx) |
| `core/parser.py` | 2012 | `src/pratt.rs` | Pratt parser, 10 precedence levels, prefix/infix rule tables, right-assoc ternary |

**Precedence table (from parser.py):** NONE=0, ASSIGNMENT=1, PIPE=2, OR=3, AND=4, EQUALITY=5, COMPARISON=6, TERM=7, FACTOR=8, UNARY=9, CALL=11.

## 2. Semantic → `crates/helen-semantic`

| Python module | Lines | Rust target | Must port |
|---|---|---|---|
| `semantic/types.py` | 338 | `src/types.rs` | 14-type hierarchy, assignability rules, operator type constraints, gradual checking (dynamic/annotated/strict), `type_from_typenode()` |
| `semantic/symbols.py` | 195 | `src/symbols.rs` | symbol table, scopes |
| `semantic/analyzer.py` | 1976 | `src/analyzer.rs` | 47+ visitor methods, E0xxx error codes (incl. E0355 TOP_LEVEL_STATEMENT, agent-scope isolation checks, shared-let value-type rule), predefined-exception whitelist |

## 3. Interpreter → `crates/helen-interpreter`

| Python module | Lines | Rust target | Must port |
|---|---|---|---|
| `interpreter/environment.py` | 316 | `src/environment.rs` | scope chain, `snapshot()` (shallow copy for async/spawn isolation) |
| `interpreter/exceptions.py` | 347 | `src/exceptions.rs` | `HelenRuntimeError` base, `_PREDEFINED_EXCEPTIONS` set, `BreakSentinel`/`ContinueSentinel`/`ReturnSentinel`, `ConstAssignmentError` |
| `interpreter/exception_mixin.py` | 183 | (in `interpreter.rs`) | try/catch/finally, `throw` |
| `interpreter/closure.py` | 341 | `src/closure.rs` | closures, value capture (v1.12) |
| `interpreter/readonly_view.py` | 162 | `src/readonly_view.rs` | ReadOnlyView wrapping reference params |
| `interpreter/shared_store.py` | 234 | `src/shared_store.rs` | shared store (v1.12), value types only |
| `interpreter/pattern_mixin.py` | 188 | `src/pattern.rs` | `match`/`case`/`default` |
| `interpreter/import_mixin.py` | 503 | `src/import.rs` | multi-format imports, circular detection, path safety, import resolver cache |
| `interpreter/streaming_mixin.py` | 76 | (in `interpreter.rs`) | `for await` over StreamingResponse |
| `interpreter/agent_context.py` | 1316 | `src/agent.rs` | agent decl, `main{}`, history/compression hooks, session tracking |
| `interpreter/llm_mixin.py` | 1926 | `src/llm.rs` | `llm act` (sync + streaming + tools), `llm if`, `llm branch`, tool-calling loop |
| `interpreter/interpreter.py` | 2259 | `src/interpreter.rs` | expression/statement evaluation, sentinel propagation, `spawn` (daemon threads), `for await`, call agents |

## 4. Runtime → `crates/helen-runtime`

| Python module | Lines | Rust target | Notes |
|---|---|---|---|
| `runtime/tools.py` | 931 | `src/tools.rs` | 11 built-in tools, JSON-schema registration, MCP hook |
| `runtime/llm_runtime.py` | 189 | `src/llm.rs` | `LLMRuntime` trait, `MockLLMRuntime` |
| `runtime/http_llm.py` | 1689 | `src/http_llm.rs` | sync HTTP client, SSE streaming parse, retries, cancellation |
| `runtime/provider_protocol.py` | 611 | `src/provider.rs` | `PlatformProtocol` trait; DashScope, Volcengine, Zhipu, DeepSeek, Minimax, Kimi, OpenAI; custom-provider loader (PyO3-gated, D8) |
| `runtime/config.py` | 751 | `src/config.rs` | settings: TOML/env/CLI, API keys, provider defaults |
| `runtime/prompt_builder.py` | 437 | `src/prompt.rs` | two-layer progressive disclosure, `{{var}}` template render |
| `runtime/history.py` | 1069 | `src/history.rs` | token budget, truncation, `conversation_summary` |
| `runtime/transcript_store.py` | 1822 | `src/transcript.rs` | JSONL/SQLite backends, LRU, BoundaryMarker, UUID addressing, session meta |
| `runtime/graduated_compression.py` | 810 | `src/compression.rs` | 5-layer graduated compression |
| `runtime/cache_aware_compression.py` | 305 | `src/compression.rs` | cache-aware layer |
| `runtime/reactive_compaction.py` | 377 | `src/compression.rs` | reactive compaction |
| `runtime/context_awareness.py` | 158 | `src/context.rs` | context-awareness decisions |
| `runtime/context_recovery.py` | 377 | `src/context.rs` | recovery |
| `runtime/working_memory.py` | 473 | `src/working_memory.rs` | v1.25 working-memory block |
| `runtime/session_manager.py` | 325 | `src/session.rs` | session lifecycle, scoping, memento |
| `runtime/memory.py` | 144 | `src/memory.rs` | FileMemoryProvider, InMemoryProvider |
| `runtime/model_capabilities.py` | 260 | `src/model_caps.rs` | model feature matrix |
| `runtime/observability.py` | 658 | `src/observability.rs` | tracing, metrics, LLM log |
| `runtime/recording.py` | 485 | `src/recording.rs` | transcript recording |
| `runtime/transcript_replay.py` | 246 | `src/transcript.rs` | replay |
| `runtime/data_lineage.py` | 270 | `src/data_lineage.rs` | data lineage |
| `runtime/error_diagnostics.py` | 299 | `src/diagnostics.rs` | AI-native error diagnostics |
| `runtime/coverage.py` | 661 | `src/coverage.rs` | `helen coverage` |
| `runtime/output_validator.py` | 186 | `src/validator.rs` | output contract validation |
| `runtime/resilience.py` | 296 | `src/resilience.rs` | retries/fallback |
| `runtime/import_resolver.py` | 419 | `src/import.rs` | resolver cache + circular detection (see skill note) |
| `runtime/fuzzy_match.py` | 620 | `src/fuzzy.rs` | fuzzy matching for `find*`/skill lookup |
| `runtime/channel.py` | 219 | `src/channel.rs` | `Channel`/`ChannelEndpoint` |
| `runtime/media.py` + `media_storage.py` | 436 | `src/media.rs` | multimodal media handling |
| `runtime/async_iterator_contracts.py`, `stream_contracts.py`, `streaming_response.py` | — | `src/stream.rs` | `for await` types |
| `runtime/mcp/*` | 5 modules | `src/mcp/` | client, config, registry, server_manager, exceptions |
| `runtime/token_utils.py`, `llm_summarizer.py`, `probe.py`, `context_helpers.py` | — | `src/token.rs` | token counting (tiktoken fallback heuristic), summarizer, connectivity probe |
| `runtime/mailbox.py` (stdlib) | 52 | stdlib `mailbox` | `mailbox_select` |

## 5. Stdlib → `crates/helen-stdlib` (378 builtins, ~25 modules)

Port **exactly** these modules (`helen/stdlib/*.py`), each with its `*_contracts.py` doc/signature:

`string`(602) · `collection`(539) · `data`(618) · `time`(376) · `math_stats`(547) · `file_advanced`(352) · `system`(528) · `network`(264) · `crypto`(285) · `data_formats`(373) · `media`(601) · `context`(1990) · `quality`(1471) · `test`(974) · `tools`(148) · `transcript`(2104) · `transcript_query`(167) · `debug`(465) · `llm_control`(173) · `collection_contracts`(311) · `file_advanced_contracts`(172) · `system_contracts`(243) · `time_contracts`(194) · `math_stats_contracts`(173) · `crypto_contracts`(159) · `data_contracts`(235) · `data_formats_contracts`(190) · `network_contracts`(99) · `string_contracts`(350) · `stream_contracts`(102)

Plus `stdlib/locales/zh.py` — **Chinese alias registration** (e.g., `我的函数` → `my_function`).

Core builtins (from `stdlib/__init__.py` head): `print, len, str, int, float, bool, list, dict, abs, min, max, range, type, isinstance, input, multiline_input, read_file, upper, lower, strip, split, join, startswith, endswith, replace, find, find_from, contains, substring, trim_prefix, trim_suffix, interpolate, regex_*, tokenize, levenshtein, similarity, base64_*, html_escape/unescape, …`

## 6. FFI + Bridge → `crates/helen-ffi` + `crates/helen-python-bridge`

| Python module | Rust target | Must port |
|---|---|---|
| `ffi/contracts.py` (170) | `src/contracts.rs` | `PythonObject`, `PythonModule`, `TypeConverter`, `PythonRuntime` trait contracts |
| `ffi/python_runtime.py` (89) | `src/runtime.rs` | GIL lifecycle, module import, call dispatch |
| `ffi/python_module.py` (71) | `src/module.rs` | imported module attribute access |
| `ffi/python_object.py` (140) | `src/object.rs` | attribute/item/call wrappers, `unwrap()` |
| `ffi/type_converter.py` (79) | `src/converter.rs` | Helen ↔ Python type mapping (int, float, str, bool, list, dict, None; list↔PyList, dict↔PyDict) |
| `python_bridge/import_hook.py` (246) | `src/import_hook.rs` + `import_hook.py` shim | meta-path finder for `.helen` files |
| `python_bridge/agent_wrapper.py` (259) | `src/agent_wrapper.rs` | callable wrapper, positional/keyword validation, `async_call` |
| `python_bridge/function_wrapper.py` (185) | `src/function_wrapper.rs` | wrap Helen `fn` for Python |
| `python_bridge/decorators.py` (91) | `src/decorators.rs` | `@helen_agent` |
| `python_bridge/type_converter.py` (86) | `src/converter.rs` | bridge direction conversions |

## 7. CLI / LSP / Agent → `crates/helen-cli` + `crates/helen-lsp`

| Python module | Lines | Rust target |
|---|---|---|
| `cli/__main__.py` | 1586 | `src/main.rs` — subcommands: run `<file>`, `check`, `test`, `repl`, `docgen`, `ask`, `coverage`, `--version` |
| `cli/repl.py` | 704 | `src/repl.rs` — multiline input, history, hints |
| `cli/ask_assistant.py` | 521 | `src/ask.rs` |
| `cli/agent_launcher.py` | 214 | `src/launcher.rs` |
| `cli/docgen.py` | 352 | `src/docgen.rs` |
| `cli/formatter.py` | 108 | `src/formatter.rs` |
| `lsp/server.py` | 1375 | `src/server.rs` — JSON-RPC, text sync, diagnostics, hover, completion |

## 8. Skills system (used by agent runtime, not a Python module)

`helen/skills/software-development/*` — three-layer search, two-layer disclosure, `load_skill`/`list_skill_references` tools. Port the search/disclosure logic to `src/skills.rs`; ship the same skill content as data under `helen-rust/skills/`.

## 9. Test Suites (conformance mapping)

| Python test dir | Files / lines | Conformance strategy |
|---|---|---|
| `tests/lexer/` | 3 / 1638 | Port to Rust unit tests (token streams) |
| `tests/parser/` | 12 / 1430 | Port + AST-printer snapshot comparisons |
| `tests/core/` | 5 / 1997 | AST visitor, source spans |
| `tests/semantic/` | 11 / 2411 | Port error-code tests (E-code diffing) |
| `tests/language/` | 11 / 2672 | Differential corpus |
| `tests/execution/` | 24 / 6685 | Differential corpus (primary) |
| `tests/interpreter/` | 22 / 6358 | Differential corpus + ported unit tests |
| `tests/stdlib/` | 34 / 10207 | Per-module differential + ported data tables |
| `tests/runtime/` | 47 / 11985 | Ported unit tests (MockLLM) + corpus |
| `tests/ffi/` | 4 / 759 | Ported PyO3 tests |
| `tests/integration/` | 2 / 489 | End-to-end |
| `tests/agent/` | 10 / 2096 | Agent-level integration |
| `tests/cli/` | 7 / 899 | CLI golden tests |
| `tests/lsp/` | 1 / 689 | LSP protocol tests |
| `tests/multimodal/` | 3 / 2109 | media pipeline tests |
| `tests/performance/` | 1 / 487 | benchmark port |
| `tests/extension/` | 1 / 217 | extension mechanism |

## 10. Feature-Complete Checklist (high level)

- [ ] 88 token types, 99 bilingual keywords, CJK identifiers
- [ ] 61 AST nodes, AST printer, spans
- [ ] 10-level Pratt parser (all operators incl. `|>`, `..`, `->`, ternary)
- [ ] 14-type gradual semantic analysis, E0xxx codes
- [ ] Interpreter: let/const/shared/alias, fn+closures, if/for/while/match/try, `assert`, `throw`
- [ ] `agent` decls (description/prompt/model/tools/skills/sub-agents/memory/temperature/max-turns/functions/context/main), scope isolation
- [ ] `llm act` (sync/stream/tools), `llm if`, `llm branch`, `for await`
- [ ] `spawn` + Channel + `mailbox_select`, `resume("<id>")`
- [ ] `import` multi-format, resolver cache, circular detection
- [ ] protocols/impl, pipe operator, pattern matching
- [ ] 378 stdlib builtins + Chinese aliases
- [ ] 11 built-in tools + skills system
- [ ] LLM: MockLLMRuntime, HTTP SSE streaming, 6 providers, custom provider loader
- [ ] TranscriptStore (JSONL/SQLite), history, 5-layer compression, working memory, session scoping, observability, coverage, recording/replay
- [ ] MCP client
- [ ] Python FFI (import numpy/requests from Helen)
- [ ] Python Bridge (import .helen agents from Python, sync/async, decorators)
- [ ] CLI subcommands + REPL + LSP
- [ ] docs: `helen docgen` output parity
