# M26: /clear Command Fix — Complete Solution

## Problem Description

When users executed the `/clear` command in the Helen WebUI:
1. ✅ Frontend display was cleared immediately (M26.10 fix)
2. ✅ SessionStats were reset (M26.11 fix)
3. ✅ BoundaryMarker was inserted into transcript (M26.14 fix)
4. ❌ **BUT**: Refreshing the page caused all messages to reappear

## Root Cause

The `TranscriptReader::entries_to_messages()` function in Rust ignored `boundary_marker` entries. When the page refreshed:
1. Frontend called `GET /api/chat/dir/messages`
2. Backend read transcript.jsonl via `TranscriptReader`
3. `entries_to_messages()` skipped boundary_marker entries but returned ALL message entries
4. Messages that should have been hidden by the boundary reappeared

The Python reference implementation's `read_view()` method correctly applies boundary markers to hide messages in the `[head_uuid, tail_uuid]` range, but the Rust port did not implement this logic.

## Solution

### Code Changes

**File**: `crates/helen-agent/src/transcript.rs`

1. **Added boundary_marker fields to `TranscriptEntry`**:
   ```rust
   pub head_uuid: Option<String>,
   pub tail_uuid: Option<String>,
   pub anchor_uuid: Option<String>,
   pub summary: Option<String>,
   pub layer: Option<String>,
   ```

2. **Modified `entries_to_messages()` to respect boundary markers**:
   - Build UUID → index map for all entries
   - Collect all compressed (hidden) UUIDs from boundary_marker entries
   - For each boundary, mark messages in `[head_idx, tail_idx]` range as compressed
   - Skip messages whose UUID is in the compressed set

3. **Updated `extract_preview()` to respect boundary markers**:
   - Same logic as `entries_to_messages()` to ensure session previews don't show cleared messages

### Algorithm

```rust
// 1. Build UUID index
let uuid_index: HashMap<&str, usize> = entries
    .iter()
    .enumerate()
    .filter_map(|(i, e)| e.uuid.as_deref().map(|uuid| (uuid, i)))
    .collect();

// 2. Collect compressed UUIDs from all boundary_markers
let mut compressed_uuids: HashSet<String> = HashSet::new();
for entry in entries {
    if entry.entry_type != "boundary_marker" { continue; }
    
    let head_idx = uuid_index.get(entry.head_uuid.as_deref()?)?;
    let tail_idx = uuid_index.get(entry.tail_uuid.as_deref()?)?;
    
    // Mark all messages in range as compressed
    let (lo, hi) = if head_idx <= tail_idx {
        (head_idx, tail_idx)
    } else {
        (tail_idx, head_idx)
    };
    
    for i in lo..=hi {
        if let Some(ref uuid) = entries[i].uuid {
            compressed_uuids.insert(uuid.clone());
        }
    }
}

// 3. Skip compressed messages when building result
for entry in entries {
    if entry.entry_type != "message" { continue; }
    if compressed_uuids.contains(entry.uuid.as_deref()?) { continue; }
    // ... process message
}
```

## Regression Tests

### Unit Tests (in `transcript.rs`)

Added 6 unit tests covering:
- `test_entries_to_messages_no_boundary`: Baseline - all messages returned without boundaries
- `test_entries_to_messages_boundary_clears_all`: Boundary covering all messages hides them
- `test_entries_to_messages_boundary_partial`: Boundary covering subset hides only those
- `test_entries_to_messages_multiple_boundaries`: Multiple boundaries work correctly
- `test_entries_to_messages_boundary_with_missing_uuids`: Graceful handling of invalid boundaries
- `test_entries_to_messages_boundary_empty_head_tail`: Empty head/tail ignored

### Integration Tests (in `tests/test_transcript.rs`)

Added 3 integration tests simulating real-world scenarios:

1. **`test_clear_command_hides_messages_after_refresh`**:
   - Simulates transcript after `/clear` command
   - Verifies all messages are hidden by boundary_marker
   - **This is the primary regression test for the reported issue**

2. **`test_clear_then_new_messages_only_shows_new`**:
   - Simulates: conversation → /clear → new conversation
   - Verifies only new messages (after boundary) are visible

3. **`test_multiple_clear_commands`**:
   - Simulates multiple `/clear` operations
   - Verifies all boundaries are applied correctly

## Test Results

```
running 11 tests
test test_read_transcript_with_entries ... ok
test test_read_transcript_skips_invalid_json ... ok
test test_multiple_clear_commands ... ok
test test_session_info_structure ... ok
test test_transcript_to_messages_extracts_timestamp ... ok
test test_clear_command_hides_messages_after_refresh ... ok
test test_read_empty_transcript ... ok
test test_list_sessions ... ok
test test_transcript_to_messages_filters_non_messages ... ok
test test_transcript_to_messages_filters_test_messages ... ok
test test_clear_then_new_messages_only_shows_new ... ok

test result: ok. 11 passed; 0 failed
```

## Why This Wasn't Caught Earlier

The previous fixes (M26.10, M26.11, M26.14) focused on the **runtime behavior**:
- M26.10: Send `clear_messages` WS event to clear frontend
- M26.11: Reset SessionStats snapshot
- M26.14: Actually insert BoundaryMarker (was stub)

But they didn't test the **persistence layer**: what happens when the page is refreshed and messages are reloaded from the transcript file.

The existing test `test_transcript_to_messages_filters_non_messages` only verified that boundary_marker entries themselves are not returned as messages, but didn't verify that they should **hide** the messages they cover.

## Lessons Learned

1. **Test the full user journey**: Don't just test individual components in isolation. Test the complete flow: user action → runtime behavior → persistence → reload.

2. **Regression tests should cover the exact failure scenario**: The test should simulate "execute /clear → refresh page → verify messages don't reappear".

3. **Porting requires conformance testing**: The Python reference implementation had this logic correct. Differential testing against the reference would have caught this issue earlier.

## Related Commits

- M26.10 (ae57d9ca): Fix /clear not sending clear_messages WS event
- M26.11 (f07e2a52): Fix /clear not resetting SessionStats
- M26.14 (439b184e): Fix clear_context() to actually insert BoundaryMarker
- **This fix**: Make TranscriptReader respect boundary_marker entries

## Files Modified

- `crates/helen-agent/src/transcript.rs`: Core fix + unit tests
- `crates/helen-agent/tests/test_transcript.rs`: Integration regression tests
