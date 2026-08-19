# Helen-Rust Unimplemented Features & Stubs Report

**Generated:** 2026-08-16  
**Scanner:** Systematic grep analysis + manual review

---

## Executive Summary

Systematic scan identified **11 categories** of unimplemented/simplified features across the codebase. Most REPL stubs have been addressed, but significant gaps remain in quality analysis, debug recording, and LSP advanced features.

---

## 1. REPL (crates/helen-rust/src/repl.rs)

### Status: ✅ Mostly Complete

The REPL implementation is **substantially complete** compared to Python reference.

**Implemented:**
- ✅ `:help`, `:reset`, `:list`, `:undefine`
- ✅ `:ask` (single/multi-turn, --list, --resume)
- ✅ `:trace` (on/off/show)
- ✅ `:last_error` (-v verbose)
- ✅ `:llm_log` (n, -v verbose)
- ✅ `:stats`
- ✅ `:transcript` (--full, --audit)
- ✅ `:sessions`, `:session_id`, `:resume`

**No major stubs found in REPL.**

---

## 2. Debug/Observability (crates/helen-interpreter/src/debug.rs)

### Status: ⚠️ Partial Implementation

**Implemented:**
- ✅ `trace_on()`, `trace_off()`, `get_trace()`
- ✅ `get_llm_log()`, `get_call_stack()`, `get_last_error()`, `last_error_detail()`
- ✅ `error_category()`, `error_suggestion()`, `error_data_flow()`
- ✅ `record_data_flow()`, `query_data_flow()`, `trace_value_origin()`, `trace_value_consumers()`, `get_data_lineage()`
- ✅ `validate_output()` (basic JSON/text/schema validation)

**Stubs/Not Yet Ported:**
- ❌ `record_session()` — returns error "LLM runtime does not support recording"
- ❌ `stop_recording()` — returns error "LLM runtime does not support recording"
- ❌ `replay_session()` — returns error "ReplayLLMRuntime not yet implemented in Rust port"
- ❌ `start_coverage()`, `stop_coverage()`, `get_coverage_report()` — returns error "Coverage tracking requires coverage.py integration (not yet ported)"

**Root Cause:** Recording/replay requires mutable access to `Arc<dyn LlmRuntime>`, which isn't available. Coverage requires coverage.py integration.

---

## 3. Quality Analysis (crates/helen-interpreter/src/quality.rs)

### Status: ⚠️ Simplified Implementation

**Python Reference:** 1471 lines with comprehensive static analysis  
**Rust Implementation:** ~180 lines with basic heuristics

**Implemented (Simplified):**
- ✅ `analyze_code()` — basic metrics (lines, comments, functions, agents)
- ✅ `quality_score()` — simple heuristic scoring (5.0 base + bonuses)
- ✅ `quality_report()` — formatted text report
- ✅ `check_security()` — **STUB: always returns empty list**

**Missing vs Python:**
- ❌ `FunctionMetrics` class (parameter counting, complexity, nesting)
- ❌ `CodeMetrics` class (comprehensive analysis)
- ❌ `SecurityAnalyzer` class (19 security patterns)
- ❌ `QualityScorer` class (architecture, complexity, documentation scoring)
- ❌ Dead code detection
- ❌ Stub function detection
- ❌ Cyclomatic complexity calculation
- ❌ Nesting depth analysis
- ❌ Docstring detection

**Impact:** Quality scores are **not comparable** to Python reference. Security checks are non-functional.

---

## 4. Media (crates/helen-interpreter/src/media.rs)

### Status: ⚠️ Partial Implementation

**Implemented:**
- ✅ `is_media()`, `media_type()`, `is_image()`, `is_video()`, `is_audio()`
- ✅ `media()` (file/URL/base64)
- ✅ `media_base64()`
- ✅ `to_openai_parts()`, `to_claude_parts()`, `to_gemini_parts()`
- ✅ `media_to_base64()` (file/base64 sources)
- ✅ `save_media()`

**Stubs:**
- ❌ `media_to_base64()` for URL sources — returns "URL downloading not yet implemented in Rust runtime"
- ⚠️ Video conversion to OpenAI format — falls back to text placeholder `[视频: ...]`

**Root Cause:** URL downloading requires async HTTP client.

---

## 5. LLM Control (crates/helen-interpreter/src/llm_control.rs)

### Status: ✅ Functional but Tests Marked as "Stub"

**Implemented:**
- ✅ `set_temperature()`, `get_temperature()`
- ✅ `set_max_turns()`, `get_max_turns()`
- ✅ `set_max_tokens()`, `get_max_tokens()`
- ✅ `set_thinking_mode()`, `get_thinking_mode()`
- ✅ `set_reasoning_effort()`, `get_reasoning_effort()`
- ✅ `get_model()`, `get_description()`, `get_provider()`
- ✅ `cancel_llm_call()`, `current_llm_call_id()`, `cancel_all_llm_calls()`

**Issue:** Test names contain "stub" suffix (e.g., `test_cancel_llm_call_stub`) but implementations are **functional**. Tests verify behavior correctly.

**Recommendation:** Rename tests to remove "stub" suffix to avoid confusion.

---

## 6. Interpreter (crates/helen-interpreter/src/interpreter.rs)

### Status: ⚠️ Minimal Stubs

**Identified Stubs:**

1. **Agent without explicit logic** (line 2610):
   ```rust
   // Agent without explicit logic: stub LLM response for M3.
   let prompt = agent.prompt.as_ref().map(|p| p.content.clone()).unwrap_or_default();
   Ok(Flow::Normal(Some(Value::Str(Rc::from(prompt.as_str())))))
   ```
   **Behavior:** Returns prompt string as response instead of calling LLM.  
   **Impact:** Agents without `main` blocks don't actually invoke LLM.

2. **`format_context_stats()`** (line 226):
   ```rust
   /// Simplified version - returns basic stats since full history management
   /// is not yet integrated in Rust.
   ```
   **Behavior:** Shows basic counts (functions, agents, env vars) but not token usage.  
   **Impact:** `:stats` REPL command shows incomplete information.

3. **Alias/LLM stubs** (line 2862):
   - Comment mentions "Alias / LLM stubs" but `visit_alias()` is implemented.
   - No actual stub code found.

---

## 7. Stdlib (crates/helen-interpreter/src/stdlib.rs)

### Status: ⚠️ Minimal Stubs

**Identified Stubs:**

1. **Stub module functions** (line 3887):
   ```rust
   // Stub module functions (runtime-dependent; documented error until M5+).
   ```
   **Context:** Section header for network/HTTP functions that require runtime support.  
   **Status:** Functions are implemented (http_get, http_post, etc.) but may have limited functionality.

2. **Session metadata** (line 4020):
   ```rust
   // Metadata lives in the transcript's first line; without a transcript
   // store we return a minimal stub (M8 transcript integration fills this).
   ```
   **Behavior:** `get_session_info()` returns minimal `{status: "ok", session_id: ...}` without full metadata.  
   **Impact:** Session metadata (created_at, modified_at, size, message_count) not available in batch mode.

---

## 8. LSP Server (crates/helen-lsp/src/server.rs)

### Status: ⚠️ Functional but Limited

**Implemented:**
- ✅ `completion` (keywords, snippets, functions)
- ✅ `definition` (go to definition)
- ✅ `references` (find all references)
- ✅ `hover` (symbol information)
- ✅ `document_symbol` (outline)
- ✅ `diagnostic` (error reporting)
- ✅ `publish_diagnostics` (push diagnostics)

**Missing LSP Features:**
- ❌ `signature_help` (parameter hints)
- ❌ `code_action` (quick fixes)
- ❌ `formatting` (code formatting)
- ❌ `rename` (symbol renaming)
- ❌ `document_highlight` (highlight same symbol)
- ❌ `folding_range` (code folding)
- ❌ `selection_range` (smart selection)
- ❌ `code_lens` (reference counts)
- ❌ `workspace_symbol` (global symbol search)

**Impact:** LSP provides basic IDE support but lacks advanced features.

---

## 9. Placeholder Crate (crates/helen-stdlib/src/lib.rs)

### Status: ❌ Empty Placeholder

```rust
//! helen-stdlib — placeholder crate. Implemented in a later milestone (M0 scaffold).
```

**Content:** Single comment line, no code.  
**Impact:** None (crate not used).

---

## 10. Compression (crates/helen-runtime/src/compression.rs)

### Status: ⚠️ Partial Implementation

**Implemented:**
- ✅ Graduated compression (5 layers)
- ✅ Cache-aware compression
- ✅ Reactive compaction
- ✅ Token estimation

**Stubs:**
- ❌ Semantic compression (line 893):
  ```rust
  // Semantic threshold first (higher priority). Python requires
  // `llm_client is not None` — the base port has no LLM client, so
  // this path is never armed (parity: falls through to structural).
  let has_llm_client = false;
  ```
  **Behavior:** Semantic compression never triggers (requires LLM client for summarization).  
  **Impact:** Compression is less effective than Python reference.

---

## 11. Context Management (crates/helen-interpreter/src/context.rs)

### Status: ⚠️ Simplified Implementation

**Identified Simplifications:**

1. **Token estimation** (line 427):
   ```rust
   // Estimate compressed tokens (simplified)
   let compressed_tokens = if compressed_count > 0 {
       // Assume compressed messages use 10 tokens each
       ...
   }
   ```
   **Behavior:** Uses heuristic (10 tokens per compressed message) instead of actual token counting.  
   **Impact:** Token usage statistics are approximate.

---

## Summary Table

| Category | Status | Severity | Impact |
|----------|--------|----------|--------|
| REPL | ✅ Complete | Low | None |
| Debug/Observability | ⚠️ Partial | Medium | Recording/replay/coverage non-functional |
| Quality Analysis | ⚠️ Simplified | **High** | Scores not comparable to Python; security checks non-functional |
| Media | ⚠️ Partial | Low | URL downloading not supported |
| LLM Control | ✅ Functional | Low | Test naming misleading |
| Interpreter | ⚠️ Minimal Stubs | Medium | Agents without main don't call LLM; stats incomplete |
| Stdlib | ⚠️ Minimal Stubs | Low | Session metadata incomplete in batch mode |
| LSP Server | ⚠️ Limited | Medium | Missing advanced IDE features |
| Placeholder Crate | ❌ Empty | Low | Not used |
| Compression | ⚠️ Partial | Medium | Semantic compression disabled |
| Context Management | ⚠️ Simplified | Low | Token estimates approximate |

---

## Priority Recommendations

### High Priority
1. **Quality Analysis** — Implement full `HelenCodeAnalyzer`, `SecurityAnalyzer`, `QualityScorer` to match Python reference (1471 lines).
2. **Debug Recording** — Refactor `Arc<dyn LlmRuntime>` to allow mutable access for recording/replay.

### Medium Priority
3. **LSP Advanced Features** — Implement `signature_help`, `code_action`, `formatting`, `rename`.
4. **Semantic Compression** — Integrate LLM client for summarization-based compression.
5. **Agent LLM Invocation** — Implement actual LLM calls for agents without explicit `main` blocks.

### Low Priority
6. **Media URL Downloading** — Add async HTTP client for URL-based media.
7. **Context Token Tracking** — Integrate transcript store for accurate token counting.
8. **Test Naming** — Rename "stub" tests in llm_control_tests.rs.
9. **Placeholder Crate** — Remove or implement helen-stdlib crate.

---

## Files Analyzed
- `crates/helen-rust/src/repl.rs`
- `crates/helen-interpreter/src/debug.rs`
- `crates/helen-interpreter/src/quality.rs`
- `crates/helen-interpreter/src/media.rs`
- `crates/helen-interpreter/src/llm_control.rs`
- `crates/helen-interpreter/src/interpreter.rs`
- `crates/helen-interpreter/src/stdlib.rs`
- `crates/helen-lsp/src/server.rs`
- `crates/helen-stdlib/src/lib.rs`
- `crates/helen-runtime/src/compression.rs`
- `crates/helen-interpreter/src/context.rs`

---

## Next Steps
1. Review with team which stubs are acceptable vs. blocking
2. Create implementation plan for high-priority items
3. Update migration notes with stub locations
4. Consider creating GitHub issues for tracking
