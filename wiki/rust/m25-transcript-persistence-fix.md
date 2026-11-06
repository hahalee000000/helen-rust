# M25: Transcript Persistence Fix — TDD Implementation

## Summary

Fixed the transcript persistence issue where agents with `transcript "persistent"` were not writing messages to `transcript.jsonl` files, causing messages to disappear on page refresh in the WebUI.

## Root Cause

The `call_agent` function in `crates/helen-interpreter/src/interpreter.rs` had transcript writing code only in the branch for agents **without explicit logic** (agents with only a `prompt` field). Agents **with explicit logic** (agents with a `main` block) took a different code path that never wrote to the transcript.

## Solution

Added transcript writing to **both** code paths in `call_agent`:

1. **Agents with explicit logic (main block)**: Write a summary of the call before execution, and the return value after execution.
2. **Agents with prompt only**: Write the rendered prompt before LLM call, and the LLM response after.

## Changes Made

### 1. `crates/helen-interpreter/src/interpreter.rs`

- Made `write_transcript_message` method public (line 301)
- Added transcript writing to the `if let Some(logic) = &agent.logic` branch (lines 2762-2792)
- Kept existing transcript writing in the `else` branch for prompt-only agents (lines 2795-2830)

### 2. `crates/helen-interpreter/tests/transcript_tests.rs`

Added 3 unit tests for `write_transcript_message`:
- `test_write_transcript_message_creates_file`: Verifies file creation
- `test_write_transcript_message_appends_multiple_messages`: Verifies message appending
- `test_write_transcript_message_skips_when_no_session`: Verifies early return when no session

### 3. `crates/helen-interpreter/tests/agent_transcript_tests.rs` (NEW)

Added 2 integration tests:
- `test_agent_with_persistent_transcript_writes_messages`: End-to-end test for agents with `transcript "persistent"`
- `test_agent_without_persistent_transcript_does_not_write`: Verifies agents without persistent transcript don't write

## Test Results

All tests pass:
```
test result: ok. 24 passed; 0 failed (transcript_tests)
test result: ok. 2 passed; 0 failed (agent_transcript_tests)
```

## Verification Steps

1. **Restart helen agent**:
   ```bash
   ~/helen/helen-rust/target/release/helen agent
   ```

2. **Send a message in WebUI**

3. **Check transcript file**:
   ```bash
   ls -la /home/user/work/.helen/sessions/ba58499e4e376a4b/
   cat /home/user/work/.helen/sessions/ba58499e4e376a4b/transcript.jsonl
   ```

4. **Refresh page** — messages should persist

5. **Switch pages** (Chat → Settings → Chat) — messages should persist

## Technical Details

### Transcript Message Format

For agents with explicit logic:
```json
{"role":"user","content":"Calling TestAgent(msg=\"Hello\")","agent_name":"TestAgent",...}
{"role":"assistant","content":"TestAgent returned: Str(\"Response: Hello\")","agent_name":"TestAgent",...}
```

For agents with prompt only:
```json
{"role":"user","content":"<rendered prompt>","agent_name":"TestAgent",...}
{"role":"assistant","content":"<LLM response>","agent_name":"TestAgent",...}
```

### Code Path Analysis

```rust
let r = if let Some(logic) = &agent.logic {
    // NEW: Write transcript BEFORE executing logic
    if should_write_transcript {
        write_transcript_message("user", &call_summary, &agent.name);
    }
    
    let result = execute(logic);
    
    // NEW: Write transcript AFTER executing logic
    if should_write_transcript {
        write_transcript_message("assistant", &response_summary, &agent.name);
    }
    
    result
} else {
    // EXISTING: Write transcript for prompt-only agents
    if should_write_transcript {
        write_transcript_message("user", &rendered_prompt, &agent.name);
    }
    
    let response = llm_act(prompt);
    
    if should_write_transcript {
        write_transcript_message("assistant", &response.text, &agent.name);
    }
    
    response
}
```

## Regression Testing

The fix is covered by:
- Unit tests in `transcript_tests.rs` (3 new tests)
- Integration tests in `agent_transcript_tests.rs` (2 new tests)
- All existing tests continue to pass

## Related Issues

- Fixes page refresh message loss in WebUI
- Fixes page switching message loss in WebUI
- Enables `/search` to find messages from agents with explicit logic
- Enables `memento` resume mechanism for all agent types
