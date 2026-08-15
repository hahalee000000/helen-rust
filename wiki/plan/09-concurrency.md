# M7 — Concurrency: `spawn`, Channel, Shared Store, Mailbox

**Objective:** Port `runtime/channel.py`, `interpreter/shared_store.py`, `interpreter/readonly_view.py`, the `spawn` expression, and `mailbox_select`. Exit criterion: `tests/interpreter/test_spawn*` (Tier A/C) and concurrency corpus pass deterministically.

> **Scope note (verified):** the Helen *language* has **no `async`/`await`/`for await`** — no AWAIT token, no ForAwait AST node. Task-level async exists **only on the bridge's Python side** (`agent_wrapper.async_call`, M11). LLM streaming is callback-based (`on_chunk`/`on_complete`, M5/M6), not iterable. There is therefore **no async interpreter core** to build; `spawn` uses OS threads exactly like Python's daemon threads.

## Files

```
crates/helen-runtime/src/channel.rs        // Channel + ChannelEndpoint + mailbox_select
crates/helen-interpreter/src/shared_store.rs
crates/helen-interpreter/src/spawn.rs      // spawn expr + resume("<id>") + thread runner
```

## Task 7.1: Channel (port `runtime/channel.py`)

```rust
pub struct Channel<T = Value> { pub name: String, pub queue: Mutex<VecDeque<T>>,
                                pub cond: Condvar, pub closed: AtomicBool }
pub struct ChannelEndpoint { pub channel: Arc<Channel>, pub is_main: bool }
impl ChannelEndpoint {
  pub fn send(&self, v: Value) -> Result<(), ChannelClosed>;
  pub fn recv(&self, timeout: Option<Duration>) -> Result<Option<Value>, ChannelClosed>; // port timeout/None semantics
}
```
Port exact behavior: FIFO, close semantics, `is_closed`, recv-with-timeout returning None vs raising (verify against tests).

## Task 7.2: `spawn` expression (port `interpreter.py:visit_spawn_expr`)

Semantics to mirror exactly:
1. Only valid on an agent call; evaluate args; optional `resume("<session_id>")` clause.
2. Create `Channel(name="spawn_<agent>")`; auto-append the spawned-side endpoint as the **last argument** (agent's last param typed `Channel`).
3. **Snapshot the environment** *before* spawn evaluation (M3 snapshot rule), then run the agent in a **new `std::thread`** with a fresh `Interpreter` + that snapshot (mirrors Python daemon threads + fresh Interpreter per spawn).
4. Return the main-side endpoint.
5. Session tracking: pass `parent_session_id`; support `get_spawned_sessions`/`get_spawn_tree` (M8).

Thread-safety: the spawned interpreter is fully owned by the thread (no `Arc` sharing of environments — matches Python); only `Channel`/`SharedStore` are shared (`Arc`). **Note this as a deviation risk**: Python's GIL makes some racy behaviors "safe"; Rust must document where it is intentionally stricter.

## Task 7.3: Shared store + shared let (port `shared_store.py`)

`shared store` declarations: typed slots (`store MyStore { counter: int, items: list }`), only **value types** allowed (semantic rule already in M2). Rust: `Arc<RwLock<IndexMap<String, Value>>>` shared across spawned interpreters. Port read/write/`reset` semantics and the value-type enforcement.

## Task 7.4: ReadOnlyView (port `readonly_view.py`)

Reference-typed agent params get a read-only wrapper: reads delegate, writes raise (exception class identical to Python's — one of the 11 Helen-native names). Also wrap params for `shared let` in agent main where applicable.

## Task 7.5: Mailbox (port `stdlib/mailbox.py`)

Poll multiple channel endpoints; return first available; timeout → None; port the exact waiting strategy (naive polling vs Condvar wait-all) to match Python behavior under load.

## Definition of Done — M7

- [ ] spawn → channel round-trip programs from `tests/interpreter/test_spawn*` pass byte-identical, N times in a row (deterministic stress).
- [ ] `resume("<session_id>")` reconnects to an existing spawned session.
- [ ] shared-store value-type rules enforced; isolation tests pass.
- [ ] `mailbox_select` tests pass.
- [ ] No `for await` / async interpreter surface exists (confirmed — nothing to port).
