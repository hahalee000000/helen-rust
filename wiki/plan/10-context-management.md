# M8 — Context Management: Transcript, History, Compression, Memory, Observability

**Objective:** Port `runtime/{transcript_store,session_manager,history,graduated_compression,cache_aware_compression,reactive_compaction,context_awareness,context_recovery,working_memory,memory,observability,recording,transcript_replay,data_lineage,error_diagnostics,coverage,output_validator}.py` (≈ 12k lines total). Exit criterion: `tests/runtime/` ported suites pass; long-conversation agents compress identically on the corpus.

## Files

```
crates/helen-runtime/src/transcript.rs   // TranscriptStore: JSONL/SQLite, LRU, BoundaryMarker, replay
crates/helen-runtime/src/session.rs      // SessionManager, session scoping, memento
crates/helen-runtime/src/history.rs      // token budget, truncation, conversation_summary
crates/helen-runtime/src/compression.rs  // graduated (5 layers) + cache-aware + reactive
crates/helen-runtime/src/working_memory.rs
crates/helen-runtime/src/memory.rs       // FileMemoryProvider / InMemoryProvider
crates/helen-runtime/src/observability.rs // trace, metrics, llm_log
crates/helen-runtime/src/recording.rs    // record/replay sessions
crates/helen-runtime/src/diagnostics.rs  // AI-native error diagnostics
crates/helen-runtime/src/coverage.rs     // helen coverage
crates/helen-runtime/src/validator.rs    // output contract validation
```

## Task 8.1: TranscriptStore SSOT (port `transcript_store.py`, 1,822 lines — highest-risk port)

Port: append-only store; **JSONL and SQLite backends** (`rusqlite`); `BoundaryMarker` compression-event records; **LRU cache** (boundary-aware eviction); **view cache** (dirty flag, O(1) reads); UUID message addressing; `session_meta` (argv, startup time, version); `get_session_id`, `get_session_meta`, `list_sessions`, search (`search_transcript`). `Interpreter._history` becomes a read-only view from the store (SSOT). Match Python's file formats exactly so the **same session files are readable by both implementations** — add a round-trip test with a fixture JSONL written by the Python version.

## Task 8.2: History (port `history.py`, 1,069 lines)

Token budget computation (port `token_utils`), truncation strategy (oldest-first, boundary-aware), `conversation_summary` integration. Keep the exact thresholds from `runtime/constants.py`.

## Task 8.3: Graduated compression (port `graduated_compression.py`, 810 lines)

5-layer graduated compression with per-layer triggers (message counts / token ratios) and summary prompts. Port `cache_aware_compression.py` (cache-aware decisions) and `reactive_compaction.py`. Record non-destructive compression via `BoundaryMarker` (SSOT). Triggers and thresholds must match `runtime/constants.py`.

## Task 8.4: Working memory (v1.25) + memory providers

Port `working_memory.py` (system-prompt `<working_memory>` block maintenance) and `memory.py` (`file://` provider, in-memory provider). Agent `memory "file://mem.json"` config wiring (M6).

## Task 8.5: Sessions

`session.rs`: project vs global scoping, `.helen/` marker auto-creation, `HELEN_SESSION_DIR` override, memento format (port `session_manager.py`). Session lifecycle: create/resume/cleanup, spawn session tree.

## Task 8.6: Observability, diagnostics, recording, replay, coverage

- `observability.rs`: `trace_on`, event log, `llm_log` (port `observability.py`); expose to `debug` stdlib (M4).
- `recording.rs` + `transcript.rs::replay`: `record_session`/`replay_session` (port `recording.py` + `transcript_replay.py`).
- `diagnostics.rs`: error categorization + suggestions + data-flow origin/consumers (port `error_diagnostics.py` + `data_lineage.py`) — drives `debug` stdlib functions.
- `coverage.rs`: statement/tool coverage collection + `helen coverage` report (port `coverage.py`).
- `validator.rs`: output-contract validation for `llm act` results (port `output_validator.py`).

## Task 8.7: Constants parity

Create `crates/helen-runtime/src/constants.rs` mirroring `runtime/constants.py` — URL list, size limits, thresholds, compression layer config. **Test**: a script asserts every constant matches Python's (generate from source; fail CI on drift).

## Definition of Done — M8

- [ ] JSONL/SQLite stores written by Rust are byte-compatible with Python (round-trip fixtures).
- [ ] `tests/runtime/` ported suites pass (TranscriptStore, persistence, session manager, phase4 features).
- [ ] 5-layer compression triggers identical outputs on long-conversation corpus.
- [ ] Observability/recording/replay/coverage parity on fixture sessions.
- [ ] Constants parity test green.
