# Rust migration notes — known divergences

## llm act streaming (Task 3.6, session committed 3.7b)
Python's `_visit_llm_act_streaming` is **broken in the reference version**:
`LLMRuntime.act_stream()`'s base signature lacks `max_tokens`/callback kwargs,
so every streaming call raises `TypeError`; the outer `except Exception`
then references `HelenRuntimeError` — a local imported only inside the
error-event branch — raising `UnboundLocalError` instead of the intended
`Streaming LLM call failed: ...` runtime error.

Rust implements the **intended** HLD semantics instead:
- `act_stream` default (wraps `act()`) yields one `content` event.
- `on_chunk(text)` called per event; literal `false` return interrupts
  (Python checks `is False` identity, not truthiness).
- `on_complete()` called with **no args**, only when not interrupted.
- Returns the joined text (`""` when empty — not Null).
The corpus fixture `llm_act_streaming.helen` never spawns its agent, so the
run differential (29/29) is unaffected either way.
