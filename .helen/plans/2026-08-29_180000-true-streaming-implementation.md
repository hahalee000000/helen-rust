# True Streaming Implementation Plan (Option 1: Modify Helen Functions)

## Overview

Implement true streaming by modifying Helen functions to accept streaming callbacks, allowing real-time token forwarding from LLM to WebSocket.

## Current Architecture (Simulated Streaming)

```
User Input → tui_chat_handler_actor() → ChatSessionActor
                                              ↓
                                        llm act (blocking)
                                              ↓
                                        Full response received
                                              ↓
                                        Split into chunks
                                              ↓
                                        Send all chunks at once
```

**Problem**: User waits 3-5 seconds before seeing any output.

## Target Architecture (True Streaming)

```
User Input → tui_chat_handler_actor_stream(callback) → ChatSessionActor
                                                              ↓
                                                        llm act stream {
                                                            on_chunk = |chunk| {
                                                                callback(chunk)
                                                                return true
                                                            }
                                                        }
                                                              ↓
                                                        Each token immediately forwarded
                                                              ↓
                                                        callback(chunk) → Rust → WebSocket → Browser
```

**Benefit**: User sees output appearing in real-time (100-500ms to first token).

---

## Implementation Steps

### Step 1: Modify chat_actor.helen

**File**: `crates/helen-agent/agent/chat_actor.helen`

**Changes**:
1. Add new function `tui_chat_handler_actor_stream(user_input, file_paths, streaming_callback)`
2. Pass callback to ChatSessionActor via mailbox
3. Wait for response while callback receives chunks

**Code**:
```helen
fn tui_chat_handler_actor_stream(user_input: str, file_paths: list, streaming_callback: fn): str {
    if _chat_actor_mailbox == null {
        spawn_chat_actor()
    }

    _chat_actor_req_counter = _chat_actor_req_counter + 1
    let request_id = "req-" + str(_chat_actor_req_counter)
    let request = {
        "type": "user_input_stream",
        "content": user_input,
        "file_paths": file_paths,
        "request_id": request_id,
        "streaming_callback": streaming_callback
    }

    _chat_actor_mailbox.send(request)

    // Wait for response, callback receives chunks in real-time
    while true {
        let response = _chat_actor_mailbox.receive()
        if response == null {
            let _p = {"i18n_key": "error.actorExited", "params": dict()}
            return json_stringify(_p)
        }

        let resp_type = ""
        try { resp_type = response["type"] } catch RuntimeError err {
            let _params = {"message": err.message}
            let _payload = {"i18n_key": "error.actorException", "params": _params}
            return json_stringify(_payload)
        } catch {
            let _p = {"i18n_key": "error.actorInvalidResponse", "params": dict()}
            return json_stringify(_p)
        }
        let resp_id = ""
        try { resp_id = response["request_id"] } catch {}

        if resp_type == "actor_status" {
            let _params = {"status": response["status"]}
            let _payload = {"i18n_key": "error.actorStatus", "params": _params}
            return json_stringify(_payload)
        }

        if resp_id != request_id {
            debug("tui_chat_handler_actor_stream: 丢弃过时响应 req_id=" + resp_id)
            continue
        }

        if resp_type == "response_complete" {
            return response["content"]
        }
        if resp_type == "error" {
            let _params = {"message": response["error_msg"]}
            let _payload = {"i18n_key": "error.generic", "params": _params}
            return json_stringify(_payload)
        }

        let _params = {"type": resp_type}
        let _payload = {"i18n_key": "error.unknownResponseType", "params": _params}
        return json_stringify(_payload)
    }
}
```

### Step 2: Modify chat_session_actor.helen

**File**: `crates/helen-agent/agent/chat_session_actor.helen`

**Changes**:
1. Handle new message type `user_input_stream`
2. Extract streaming callback from message
3. Use `llm act stream` with callback

**Code** (in main block):
```helen
main {
    _init_actor_output()
    
    while true {
        let msg = reply.receive()
        if msg == null {
            reply.send({"type": "actor_status", "status": "exited"})
            return
        }
        
        let msg_type = ""
        try { msg_type = msg["type"] } catch { continue }
        
        if msg_type == "user_input_stream" {
            // New streaming path
            let user_content = msg["content"]
            let file_paths = msg["file_paths"]
            let request_id = msg["request_id"]
            let streaming_callback = msg["streaming_callback"]
            
            try {
                let media_parts = _build_actor_media_parts(file_paths)
                
                // Use llm act stream with callback
                let response = llm act stream {
                    prompt = user_content
                    media = media_parts
                    on_chunk = |chunk| {
                        // Forward chunk to callback
                        try {
                            streaming_callback(chunk)
                        } catch RuntimeError err {
                            debug("streaming_callback failed: " + err.message)
                        } catch {
                            debug("streaming_callback failed (unknown)")
                        }
                        return true  // Continue streaming
                    }
                    on_complete = || {
                        _actor_complete_cb()
                    }
                }
                
                reply.send({
                    "type": "response_complete",
                    "request_id": request_id,
                    "content": response,
                    "status": "success"
                })
            } catch RuntimeError err {
                reply.send({
                    "type": "error",
                    "request_id": request_id,
                    "error_msg": err.message,
                    "error_type": "RuntimeError"
                })
            } catch {
                reply.send({
                    "type": "error",
                    "request_id": request_id,
                    "error_msg": "Unknown error",
                    "error_type": "Unknown"
                })
            }
        } else if msg_type == "user_input" {
            // Existing non-streaming path (for backward compatibility)
            // ... existing code ...
        } else if msg_type == "exit" {
            reply.send({"type": "actor_status", "status": "exited"})
            return
        } else if msg_type == "heartbeat" {
            // Keep-alive, no response needed
        }
    }
}
```

### Step 3: Modify bridge.rs

**File**: `crates/helen-agent/src/actor_bridge/bridge.rs`

**Changes**:
1. Create streaming callback that forwards chunks to stream_tx
2. Call new function `tui_chat_handler_actor_stream` instead of `tui_chat_handler_actor`
3. Pass callback as Helen Value

**Code**:
```rust
// In interpreter thread, message loop:
if let Some(input) = input_rx.recv().await {
    // Create streaming callback
    let stream_tx_clone = stream_tx_clone.clone();
    let sequence_counter = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    
    let callback = move |chunk: &str| -> bool {
        let seq = sequence_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let stream_chunk = StreamChunk {
            sequence: seq,
            content: chunk.to_string(),
        };
        if stream_tx_clone.send(stream_chunk).is_err() {
            eprintln!("Failed to send streaming chunk");
            return false;  // Stop streaming
        }
        true  // Continue streaming
    };
    
    // Convert callback to Helen Value
    // Note: This requires Helen to support Rust closures as callbacks
    // If not supported, we need a different approach
    let callback_val = Value::Callable(Box::new(callback));
    
    // Call streaming function
    match interp.call_function(
        "tui_chat_handler_actor_stream",
        vec![user_input_val, file_paths_val, callback_val],
        None,
        &span
    ) {
        Ok(result) => {
            // Send complete response
            let output = AgentOutput::ResponseComplete {
                request_id: input.request_id.clone(),
                content: format!("{:?}", result),
            };
            if let Err(e) = output_tx.send(output) {
                eprintln!("Failed to send response: {:?}", e);
            }
        }
        Err(e) => {
            let output = AgentOutput::Error {
                request_id: input.request_id.clone(),
                error_msg: format!("{:?}", e),
            };
            if let Err(e) = output_tx.send(output) {
                eprintln!("Failed to send error: {:?}", e);
            }
        }
    }
}
```

---

## Challenges and Solutions

### Challenge 1: Passing Rust Closures to Helen

**Problem**: Helen may not support Rust closures as callbacks directly.

**Solution Options**:
1. **Use Helen function as callback**: Define callback in Helen, pass to Rust
2. **Use global callback registry**: Register callback in interpreter, Helen calls it
3. **Use Channel for streaming**: Helen sends chunks via Channel, Rust receives

**Recommended**: Option 3 (Channel-based streaming)
- Helen sends chunks via a dedicated streaming Channel
- Rust receives chunks and forwards to WebSocket
- No need to pass closures between languages

### Challenge 2: Thread Safety

**Problem**: Interpreter is not Send, callback must be called from interpreter thread.

**Solution**: 
- Callback is called from interpreter thread (where llm act stream runs)
- Callback forwards to broadcast channel (thread-safe)
- WebSocket handler receives from broadcast channel

### Challenge 3: Backward Compatibility

**Problem**: Existing code uses `tui_chat_handler_actor` without callback.

**Solution**:
- Keep existing function for backward compatibility
- Add new function `tui_chat_handler_actor_stream` with callback
- Bridge uses new function, existing Helen code continues to work

---

## Implementation Order

1. **Phase 1**: Implement Channel-based streaming (simpler, no closure passing)
   - Add streaming Channel to message protocol
   - Helen sends chunks via Channel
   - Rust receives and forwards to WebSocket

2. **Phase 2**: Test with mock LLM
   - Verify chunks arrive in real-time
   - Verify ordering and completeness

3. **Phase 3**: Test with real LLM
   - Verify end-to-end streaming
   - Measure latency improvements

---

## Testing Strategy

### Unit Tests
1. Test Helen function accepts callback
2. Test callback is called for each chunk
3. Test chunks are forwarded immediately

### Integration Tests
1. Test Rust → Helen → LLM → Helen → Rust flow
2. Test WebSocket receives chunks in real-time
3. Test latency < 500ms to first token

### Performance Tests
1. Measure time to first token
2. Measure total streaming time
3. Compare with simulated streaming

---

## Expected Results

### Before (Simulated Streaming)
- Time to first token: 3000-5000ms
- User experience: Nothing for 3s, then all text at once

### After (True Streaming)
- Time to first token: 100-500ms
- User experience: Text appears character-by-character in real-time

---

## Next Steps

1. Review this plan
2. Implement Phase 1 (Channel-based streaming)
3. Write tests
4. Verify with mock LLM
5. Test with real LLM
6. Measure performance improvements
