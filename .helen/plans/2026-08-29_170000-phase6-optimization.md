# Phase 6: Optimization Implementation Plan

**Goal:** Optimize V2 architecture with true streaming, interpreter pooling, session persistence, and connection reuse.

**Architecture:** Build on Phase 1-5 foundation. Add streaming callbacks, interpreter pool, session registry, and connection reuse.

---

## Current State (After Phase 5)

- ✅ 183 tests passing
- ✅ Basic bridge: Rust ↔ Helen communication
- ✅ Response forwarding: Full response as single chunk
- ✅ Lifecycle: Heartbeat, shutdown, activity tracking
- ❌ No true streaming (full response sent at once)
- ❌ No interpreter pooling (new interpreter per connection)
- ❌ No session persistence (new bridge per reconnection)
- ❌ No connection reuse

---

## Phase 6 Tasks

### Task 6.1: True Streaming Chunk Forwarding
**Goal:** Forward LLM streaming chunks to WebSocket in real-time

**Approach:**
1. Add streaming callback to HelenActorBridge
2. Hook into Helen's `act_stream` via `on_chunk` callback
3. Forward chunks through `stream_tx` broadcast channel
4. WebSocket handler subscribes and forwards to browser

**Files:**
- Modify: `crates/helen-agent/src/actor_bridge/bridge.rs`
- Modify: `crates/helen-agent/src/websocket/actor_ws.rs`
- Test: `crates/helen-agent/tests/test_streaming_chunks.rs`

### Task 6.2: Interpreter Pooling
**Goal:** Pool interpreters to avoid creating new ones per connection

**Approach:**
1. Create `InterpreterPool` struct
2. Pre-warm pool with N interpreters
3. Checkout/checkin pattern for connections
4. Lazy creation if pool is empty

**Files:**
- Create: `crates/helen-agent/src/actor_bridge/pool.rs`
- Modify: `crates/helen-agent/src/actor_bridge/bridge.rs`
- Test: `crates/helen-agent/tests/test_interpreter_pool.rs`

### Task 6.3: Session Persistence
**Goal:** Reuse bridges across reconnections

**Approach:**
1. Create `SessionRegistry` to track active bridges by session_id
2. On WebSocket connect, check if session exists
3. If exists, reuse bridge; if not, create new one
4. Cleanup stale sessions on timeout

**Files:**
- Create: `crates/helen-agent/src/actor_bridge/session_registry.rs`
- Modify: `crates/helen-agent/src/websocket/actor_ws.rs`
- Test: `crates/helen-agent/tests/test_session_persistence.rs`

### Task 6.4: Connection Reuse
**Goal:** Reuse connections across multiple WebSocket sessions

**Approach:**
1. Track connection state in SessionRegistry
2. Support multiple WebSocket clients per bridge
3. Broadcast messages to all connected clients
4. Handle client disconnect/reconnect gracefully

**Files:**
- Modify: `crates/helen-agent/src/actor_bridge/session_registry.rs`
- Modify: `crates/helen-agent/src/websocket/actor_ws.rs`
- Test: `crates/helen-agent/tests/test_connection_reuse.rs`

---

## Implementation Order

1. **Task 6.1** (Streaming) — Highest impact, user-visible improvement
2. **Task 6.2** (Pooling) — Performance optimization
3. **Task 6.3** (Persistence) — UX improvement
4. **Task 6.4** (Reuse) — Advanced feature

---

## Verification

After each task:
- [ ] All unit tests pass
- [ ] Integration tests pass
- [ ] `cargo build --release` succeeds
- [ ] `cargo clippy` has 0 warnings
- [ ] Manual page testing (Chat page streaming)
