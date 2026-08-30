# True Streaming vs Simulated Streaming: Technical Deep Dive

## Executive Summary

The current V2 architecture implements **simulated streaming**, not true streaming. This document explains the difference, why it matters, and what would be required to implement true streaming.

---

## 1. What is True Streaming?

### Definition

**True streaming** (also called "real-time streaming" or "token-by-token streaming") means:
- The LLM generates tokens one at a time
- Each token is **immediately forwarded** to the client as it's generated
- The client sees output appearing in **real-time** as the LLM generates it
- Total latency = **time to first token** (TTFT), typically 100-500ms

### How True Streaming Works

```
┌─────────────┐
│   Browser   │
└──────┬──────┘
       │ WebSocket
       ↓
┌─────────────────────────────────────────────────────────┐
│  Rust WebUI (Axum)                                      │
│                                                         │
│  WebSocket Handler                                      │
│  ├─ Receives streaming chunks from bridge               │
│  └─ Forwards to browser immediately                     │
└──────┬──────────────────────────────────────────────────┘
       │ broadcast channel
       ↓
┌─────────────────────────────────────────────────────────┐
│  HelenActorBridge                                       │
│                                                         │
│  Interpreter Thread                                     │
│  ├─ Calls Helen function with streaming callback        │
│  ├─ Callback receives each token as it arrives          │
│  └─ Forwards token to broadcast channel immediately     │
└──────┬──────────────────────────────────────────────────┘
       │ streaming callback
       ↓
┌─────────────────────────────────────────────────────────┐
│  Helen LLM Runtime                                      │
│                                                         │
│  HttpLlmRuntime.act_stream()                            │
│  ├─ Sends request to LLM API with stream=true           │
│  ├─ Receives SSE (Server-Sent Events) stream            │
│  ├─ Parses each token from SSE                          │
│  └─ Calls on_event callback for each token              │
└──────┬──────────────────────────────────────────────────┘
       │ HTTP SSE
       ↓
┌─────────────────────────────────────────────────────────┐
│  LLM API (OpenAI, Anthropic, etc.)                      │
│                                                         │
│  Generates tokens one at a time                         │
│  Sends each token via SSE as it's generated             │
└─────────────────────────────────────────────────────────┘
```

### Timeline (True Streaming)

```
Time 0ms:     User sends message
Time 100ms:   First token arrives from LLM → forwarded to browser
Time 150ms:   Second token arrives → forwarded to browser
Time 200ms:   Third token arrives → forwarded to browser
...
Time 3000ms:  Last token arrives → forwarded to browser
Time 3000ms:  Complete response sent

User sees: Text appearing character-by-character in real-time
Total wait time: 100ms (time to first token)
```

---

## 2. What is Simulated Streaming?

### Definition

**Simulated streaming** (also called "fake streaming" or "chunked response") means:
- The LLM generates the **full response** first
- The response is then **split into chunks**
- Chunks are sent to the client with or without delay
- Total latency = **time to complete response** (e.g., 3-5 seconds)

### How Simulated Streaming Works (Current Implementation)

```
┌─────────────┐
│   Browser   │
└──────┬──────┘
       │ WebSocket
       ↓
┌─────────────────────────────────────────────────────────┐
│  Rust WebUI (Axum)                                      │
│                                                         │
│  WebSocket Handler                                      │
│  ├─ Receives streaming chunks from bridge               │
│  └─ Forwards to browser                                 │
└──────┬──────────────────────────────────────────────────┘
       │ broadcast channel
       ↓
┌─────────────────────────────────────────────────────────┐
│  HelenActorBridge                                       │
│                                                         │
│  Interpreter Thread                                     │
│  ├─ Calls Helen function (BLOCKS until complete)        │
│  ├─ Receives FULL response                              │
│  ├─ Splits response into 50-char chunks                 │
│  └─ Sends all chunks immediately (no delay)             │
└──────┬──────────────────────────────────────────────────┘
       │ blocking call
       ↓
┌─────────────────────────────────────────────────────────┐
│  Helen LLM Runtime                                      │
│                                                         │
│  HttpLlmRuntime.act()                                   │
│  ├─ Sends request to LLM API                            │
│  ├─ Waits for COMPLETE response                         │
│  └─ Returns full response                               │
└──────┬──────────────────────────────────────────────────┘
       │ HTTP request/response
       ↓
┌─────────────────────────────────────────────────────────┐
│  LLM API (OpenAI, Anthropic, etc.)                      │
│                                                         │
│  Generates full response                                │
│  Returns complete response                              │
└─────────────────────────────────────────────────────────┘
```

### Timeline (Simulated Streaming - Current)

```
Time 0ms:     User sends message
Time 100ms:   Request sent to LLM
Time 3000ms:  LLM returns COMPLETE response
Time 3000ms:  Bridge receives full response
Time 3000ms:  Bridge splits into chunks
Time 3000ms:  All chunks sent to browser at once
Time 3000ms:  Complete response sent

User sees: Nothing for 3 seconds, then all text appears at once
Total wait time: 3000ms (time to complete response)
```

---

## 3. Why the Difference Matters

### User Experience

| Aspect | True Streaming | Simulated Streaming |
|--------|----------------|---------------------|
| **Time to first token** | 100-500ms | 3000-5000ms |
| **Perceived latency** | Very low | High |
| **User feedback** | Immediate | Delayed |
| **Streaming appearance** | Real-time | All at once |

### Example

**True Streaming:**
```
[0.1s]  Hello
[0.2s]  Hello, how
[0.3s]  Hello, how are
[0.4s]  Hello, how are you
[0.5s]  Hello, how are you today?
```

**Simulated Streaming (Current):**
```
[3.0s]  (nothing)
[3.0s]  Hello, how are you today?
```

### Technical Implications

1. **Latency**: True streaming reduces perceived latency by 90%+
2. **User engagement**: Users see progress immediately
3. **Interruption**: Users can stop generation early with true streaming
4. **Token counting**: True streaming allows real-time token counting

---

## 4. Current Implementation Analysis

### Code Location

`crates/helen-agent/src/actor_bridge/bridge.rs` lines 182-217

### Current Code

```rust
match interp.call_function(&func, vec![user_input_val, file_paths_val], None, &span) {
    Ok(result) => {
        // Convert Helen Value to string
        let response_str = format!("{:?}", result);
        
        // NOTE: This is NOT true streaming.
        // The full response is received before any chunk is sent.
        
        let chunk_size = 50; // characters per chunk
        let chars: Vec<char> = response_str.chars().collect();
        
        for (sequence, chunk_start) in (0..chars.len()).step_by(chunk_size).enumerate() {
            let chunk_end = (chunk_start + chunk_size).min(chars.len());
            let chunk_content: String = chars[chunk_start..chunk_end].iter().collect();
            
            let chunk = StreamChunk {
                sequence: sequence as u64,
                content: chunk_content,
            };
            
            // Send chunk via broadcast channel (no artificial delay)
            if stream_tx_clone.send(chunk).is_err() {
                eprintln!("Failed to send streaming chunk");
                break;
            }
        }
        
        // Send complete response via output channel
        let output = AgentOutput::ResponseComplete {
            request_id: input.request_id.clone(),
            content: response_str,
        };
        if let Err(e) = output_tx.send(output) {
            eprintln!("Failed to send response to output channel: {:?}", e);
        }
    }
}
```

### Problems

1. **Blocking call**: `interp.call_function()` blocks until the full response is ready
2. **No streaming callback**: The function doesn't accept a callback for streaming
3. **Post-processing**: Chunks are created after the full response is received
4. **No real-time forwarding**: All chunks are sent at once, not as they're generated

---

## 5. What Would True Streaming Require?

### Option 1: Modify Helen Functions (Recommended)

**Changes needed:**

1. **Add streaming callback parameter to Helen functions**

```helen
// chat_actor.helen
fn tui_chat_handler_actor(user_input: str, file_paths: list, streaming_callback: fn) {
    // Call LLM with streaming
    let response = llm act stream {
        prompt = user_input
        on_chunk = |chunk| {
            streaming_callback(chunk)  // Forward each chunk
            return true  // Continue streaming
        }
    }
    return response
}
```

2. **Modify bridge to pass callback**

```rust
// bridge.rs
let callback = |chunk: &str| {
    let stream_chunk = StreamChunk {
        sequence: sequence_counter.fetch_add(1, Ordering::Relaxed),
        content: chunk.to_string(),
    };
    stream_tx_clone.send(stream_chunk).ok();
};

let callback_val = Value::Callable(callback);
interp.call_function(&func, vec![user_input_val, file_paths_val, callback_val], None, &span);
```

**Pros:**
- Clean separation of concerns
- Helen code controls streaming logic
- Reusable across different contexts

**Cons:**
- Requires modifying Helen code (outside this project)
- Complex callback passing between Rust and Helen

### Option 2: Use Helen's Streaming API Directly

**Changes needed:**

1. **Bypass Helen functions, call LLM directly**

```rust
// bridge.rs
let mut interp = Interpreter::new();
let runtime = interp.get_llm_runtime();

let mut sequence = 0u64;
let on_event = |event: Value| -> bool {
    if let Value::Str(chunk) = event {
        let stream_chunk = StreamChunk {
            sequence,
            content: chunk.to_string(),
        };
        stream_tx_clone.send(stream_chunk).ok();
        sequence += 1;
    }
    true  // Continue streaming
};

runtime.act_stream(
    &user_input,
    None,  // model
    0.7,   // temperature
    Some(&system_prompt),
    None,  // tools
    10,    // max_turns
    None,  // max_tokens
    None,  // history
    None,  // dispatch_fn
    &mut on_event,
    false, // thinking_enabled
    None,  // reasoning_effort
)?;
```

**Pros:**
- Direct access to streaming API
- No Helen function modifications needed

**Cons:**
- Bypasses Helen agent logic (context management, tools, etc.)
- Loses ChatSessionActor features
- Duplicates logic

### Option 3: Global Streaming Callback in Interpreter

**Changes needed:**

1. **Add global streaming callback to interpreter**

```rust
// interpreter.rs
pub struct Interpreter {
    // ... existing fields
    streaming_callback: Option<Box<dyn Fn(&str) -> bool + Send>>,
}

impl Interpreter {
    pub fn set_streaming_callback<F>(&mut self, callback: F)
    where
        F: Fn(&str) -> bool + Send + 'static,
    {
        self.streaming_callback = Some(Box::new(callback));
    }
}
```

2. **Call callback in LLM runtime**

```rust
// http_llm.rs
fn act_stream(...) {
    // ... existing code
    for token in tokens {
        if let Some(callback) = &interpreter.streaming_callback {
            if !callback(&token) {
                break;  // Stop streaming
            }
        }
    }
}
```

**Pros:**
- No Helen function modifications
- Works with existing code

**Cons:**
- Global state (not thread-safe)
- Complex interpreter modifications
- Hard to test

---

## 6. Recommendation

**For now**: Keep simulated streaming (current implementation)

**Rationale:**
1. The V2 architecture is a **demonstration** of the bridge pattern
2. True streaming requires **significant architectural changes**
3. The current implementation is **functional** (just not optimal)
4. True streaming can be added in a **future phase**

**Future work**: Implement Option 1 (modify Helen functions)

**Timeline**: Phase 8 (Advanced Features) - estimated 5 days

---

## 7. Phase 7: Production Hardening

### Overview

Phase 7 focuses on making the V2 architecture production-ready with comprehensive error handling, monitoring, and testing.

### Tasks

#### Task 7.1: Comprehensive Error Handling

**Objective**: Handle all error cases gracefully

**Implementation**:
1. Add error types for each failure mode
2. Implement error recovery strategies
3. Add user-friendly error messages
4. Log errors for debugging

**Files**:
- `crates/helen-agent/src/actor_bridge/errors.rs`
- `crates/helen-agent/src/websocket/errors.rs`

**Estimated effort**: 1 day

#### Task 7.2: Metrics and Monitoring

**Objective**: Track performance and health metrics

**Implementation**:
1. Add metrics collection (Prometheus)
2. Track request latency, error rates, active sessions
3. Add health check endpoints
4. Implement logging with structured logs

**Files**:
- `crates/helen-agent/src/metrics.rs`
- `crates/helen-agent/src/health.rs`

**Estimated effort**: 1 day

#### Task 7.3: Load Testing

**Objective**: Verify system can handle production load

**Implementation**:
1. Create load test scenarios
2. Test with 10, 50, 100 concurrent connections
3. Measure latency, throughput, error rates
4. Identify bottlenecks

**Files**:
- `tests/load/load_test.rs`
- `scripts/load-test.sh`

**Estimated effort**: 0.5 days

#### Task 7.4: Security Audit

**Objective**: Ensure system is secure for production

**Implementation**:
1. Review authentication/authorization
2. Check for injection vulnerabilities
3. Verify input validation
4. Test rate limiting

**Files**:
- `crates/helen-agent/src/auth.rs`
- `crates/helen-agent/src/rate_limit.rs`

**Estimated effort**: 0.5 days

### Total Effort: 3 days

---

## 8. Phase 8: Advanced Features

### Overview

Phase 8 adds advanced features that require significant architectural changes, including true streaming.

### Tasks

#### Task 8.1: True Streaming (Option 1)

**Objective**: Implement real-time token-by-token streaming

**Implementation**:
1. Modify Helen functions to accept streaming callbacks
2. Update bridge to pass callbacks
3. Forward chunks immediately as they arrive
4. Test with real LLM APIs

**Files**:
- `~/helen/helen/agent/chat_actor.helen` (modify)
- `crates/helen-agent/src/actor_bridge/bridge.rs` (modify)

**Estimated effort**: 2 days

#### Task 8.2: Interpreter Sharing

**Objective**: Share interpreters across bridges for efficiency

**Implementation**:
1. Create global interpreter pool
2. Implement interpreter checkout/checkin
3. Handle interpreter state isolation
4. Test with multiple concurrent sessions

**Files**:
- `crates/helen-agent/src/actor_bridge/global_pool.rs`

**Estimated effort**: 1.5 days

#### Task 8.3: Distributed Session Management

**Objective**: Support sessions across multiple servers

**Implementation**:
1. Add session storage (Redis)
2. Implement session migration
3. Handle session replication
4. Test with multiple servers

**Files**:
- `crates/helen-agent/src/session/storage.rs`
- `crates/helen-agent/src/session/migration.rs`

**Estimated effort**: 1 day

#### Task 8.4: Advanced Caching

**Objective**: Cache LLM responses for efficiency

**Implementation**:
1. Implement response cache (LRU)
2. Add cache invalidation logic
3. Handle cache hits/misses
4. Test cache effectiveness

**Files**:
- `crates/helen-agent/src/cache.rs`

**Estimated effort**: 0.5 days

### Total Effort: 5 days

---

## 9. Summary

### Current State

- ✅ V2 architecture implemented (Phases 1-6)
- ✅ 205 tests passing
- ⚠️ Simulated streaming (not true streaming)
- ⚠️ No production hardening

### Next Steps

1. **Phase 7** (3 days): Production hardening
   - Error handling
   - Metrics and monitoring
   - Load testing
   - Security audit

2. **Phase 8** (5 days): Advanced features
   - True streaming
   - Interpreter sharing
   - Distributed sessions
   - Advanced caching

### Total Remaining Effort: 8 days

---

## 10. Conclusion

The current V2 architecture is a **solid foundation** but implements **simulated streaming**, not true streaming. The difference is significant for user experience:

- **True streaming**: 100-500ms to first token
- **Simulated streaming**: 3000-5000ms to first token

To implement true streaming, we need to:
1. Modify Helen functions to accept streaming callbacks
2. Hook into Helen's LLM runtime streaming API
3. Forward chunks immediately as they arrive

This is a **significant architectural change** that should be done in Phase 8 after production hardening (Phase 7).

**Recommendation**: 
- Deploy current implementation as-is (simulated streaming is functional)
- Plan Phase 7 for production hardening (3 days)
- Plan Phase 8 for true streaming and advanced features (5 days)
