# Concurrency and spawn — helen-rust

> Rust port of the spawn/Channel machinery. Source: `crates/helen-runtime/src/`
> and `helen-interpreter`.

---

## `spawn`

- OS threads (not green threads). Each spawned call receives a **deep-owned
  snapshot** of the caller environment — fresh `Rc`s via `clone_owned()`, not
  `clone_deep()` (shared-let values must stay shared).
- The payload is `unsafe impl Send` under a documented single-owner
  discipline (clippy's `arc_with_non_send_sync` is intentionally allowed).
- stdout is `Arc<Mutex<String>>` so spawned threads append output safely.

### Spawn param injection (parity detail)

`spawn` injects the channel endpoint as the **LAST** parameter; user arguments
bind positionally to the non-Channel params.

## Channel

- Bidirectional message queue; `close()` pushes a sentinel → blocked receiver
  wakes, next `receive()` returns `None`; send-after-close is ignored.
- `mailbox_select` for multiplexing.

## SharedStore (M7)

- Runtime fields+methods with **deep-copy isolation** between agents.
- `shared let` must be module-level; the analyzer enforces this.
- `ReadOnlyView` guards agent params (no `__getattr__` delegation — parity).

## Registering shared stores (gotcha)

Top-level `shared store` can silently no-op — stores are only registered for
imported `.helen` files. A `register_shared_store()` helper is factored so
both the import path and direct declarations register consistently.
