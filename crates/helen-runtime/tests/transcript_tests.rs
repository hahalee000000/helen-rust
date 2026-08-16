//! Comprehensive tests for transcript module (port of Python test_transcript.py).

use helen_runtime::transcript::{
    generate_uuid, message_from_dict, message_text_parts, message_to_dict, BoundaryMarker, Item,
    Message, SessionMeta,
};
use serde_json::json;

// ── UUID Generation ─────────────────────────────────────────────────────────

#[test]
fn test_generate_uuid_length() {
    let uuid = generate_uuid();
    assert_eq!(uuid.len(), 12);
}

#[test]
fn test_generate_uuid_hex_format() {
    let uuid = generate_uuid();
    assert!(uuid.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_generate_uuid_uniqueness() {
    let uuids: Vec<_> = (0..100).map(|_| generate_uuid()).collect();
    let unique: std::collections::HashSet<_> = uuids.iter().collect();
    assert_eq!(unique.len(), 100);
}

// ── Message Creation ────────────────────────────────────────────────────────

#[test]
fn test_message_creation_basic() {
    let msg = Message::new(
        "user",
        json!("hello"),
        vec![],
        None,
        "abc123".to_string(),
        None,
        90,
        false,
        false,
        None,
        "inv1".to_string(),
        "parent1".to_string(),
        vec![],
    );
    assert_eq!(msg.role, "user");
    assert_eq!(msg.content, json!("hello"));
    assert_eq!(msg.uuid, "abc123");
    assert_eq!(msg.priority, 90);
    assert!(!msg.compressed);
    assert!(!msg.pinned);
}

#[test]
fn test_message_creation_with_tool_calls() {
    let tool_call = json!({"id": "call_1", "function": {"name": "read_file"}});
    let msg = Message::new(
        "assistant",
        json!(""),
        vec![tool_call.clone()],
        None,
        "def456".to_string(),
        Some("assistant_tool_call".to_string()),
        70,
        false,
        false,
        None,
        "inv2".to_string(),
        "parent2".to_string(),
        vec![],
    );
    assert_eq!(msg.tool_calls.len(), 1);
    assert_eq!(msg.message_type, Some("assistant_tool_call".to_string()));
}

#[test]
fn test_message_creation_with_agent_name() {
    let msg = Message::new(
        "assistant",
        json!("response"),
        vec![],
        None,
        "ghi789".to_string(),
        None,
        80,
        false,
        false,
        Some("MyAgent".to_string()),
        "inv3".to_string(),
        "parent3".to_string(),
        vec![],
    );
    assert_eq!(msg.agent_name, Some("MyAgent".to_string()));
}

// ── Message Type Inference ──────────────────────────────────────────────────

#[test]
fn test_infer_message_type_system() {
    let msg = Message::new(
        "system",
        json!("You are a helpful assistant"),
        vec![],
        None,
        "uuid1".to_string(),
        None,
        100,
        false,
        false,
        None,
        "".to_string(),
        "".to_string(),
        vec![],
    );
    assert_eq!(msg.infer_message_type(), "system");
}

#[test]
fn test_infer_message_type_user() {
    let msg = Message::new(
        "user",
        json!("Hello"),
        vec![],
        None,
        "uuid2".to_string(),
        None,
        90,
        false,
        false,
        None,
        "".to_string(),
        "".to_string(),
        vec![],
    );
    assert_eq!(msg.infer_message_type(), "user");
}

#[test]
fn test_infer_message_type_assistant() {
    let msg = Message::new(
        "assistant",
        json!("Hi there"),
        vec![],
        None,
        "uuid3".to_string(),
        None,
        80,
        false,
        false,
        None,
        "".to_string(),
        "".to_string(),
        vec![],
    );
    assert_eq!(msg.infer_message_type(), "assistant");
}

#[test]
fn test_infer_message_type_assistant_tool_call() {
    let msg = Message::new(
        "assistant",
        json!(""),
        vec![json!({"id": "call_1"})],
        None,
        "uuid4".to_string(),
        None,
        70,
        false,
        false,
        None,
        "".to_string(),
        "".to_string(),
        vec![],
    );
    assert_eq!(msg.infer_message_type(), "assistant_tool_call");
}

#[test]
fn test_infer_message_type_tool() {
    let msg = Message::new(
        "tool",
        json!("file content"),
        vec![],
        Some("call_1".to_string()),
        "uuid5".to_string(),
        None,
        20,
        false,
        false,
        None,
        "".to_string(),
        "".to_string(),
        vec![],
    );
    assert_eq!(msg.infer_message_type(), "tool");
}

// ── Message Text Extraction ─────────────────────────────────────────────────

#[test]
fn test_message_text_parts_string() {
    let (text, media) = message_text_parts(&json!("hello world"));
    assert_eq!(text, "hello world");
    assert_eq!(media, 0);
}

#[test]
fn test_message_text_parts_multimodal() {
    let content = json!([
        {"type": "text", "text": "Describe this image"},
        {"type": "image_url", "url": "http://example.com/img.png"}
    ]);
    let (text, media) = message_text_parts(&content);
    assert_eq!(text, "Describe this image");
    assert_eq!(media, 1);
}

#[test]
fn test_message_text_parts_multiple_text() {
    let content = json!([
        {"type": "text", "text": "First part"},
        {"type": "text", "text": "Second part"}
    ]);
    let (text, media) = message_text_parts(&content);
    assert_eq!(text, "First part\nSecond part");
    assert_eq!(media, 0);
}

#[test]
fn test_message_text_parts_empty() {
    let (text, media) = message_text_parts(&json!(null));
    assert_eq!(text, "");
    assert_eq!(media, 0);
}

// ── Message Serialization ───────────────────────────────────────────────────

#[test]
fn test_message_to_dict_basic() {
    let msg = Message::new(
        "user",
        json!("hello"),
        vec![],
        None,
        "abc123".to_string(),
        Some("user".to_string()),
        90,
        false,
        false,
        None,
        "inv1".to_string(),
        "parent1".to_string(),
        vec![],
    );
    let dict = message_to_dict(&msg);
    assert_eq!(dict["type"], "message");
    assert_eq!(dict["role"], "user");
    assert_eq!(dict["content"], "hello");
    assert_eq!(dict["uuid"], "abc123");
    assert_eq!(dict["priority"], 90);
}

#[test]
fn test_message_to_dict_with_optional_fields() {
    let msg = Message::new(
        "assistant",
        json!("response"),
        vec![],
        None,
        "def456".to_string(),
        None,
        80,
        false,
        false,
        Some("Agent1".to_string()),
        "inv2".to_string(),
        "parent2".to_string(),
        vec!["inv3".to_string()],
    );
    let dict = message_to_dict(&msg);
    assert_eq!(dict["agent_name"], "Agent1");
    assert_eq!(dict["invocation_id"], "inv2");
    assert_eq!(dict["parent_invocation_id"], "parent2");
    assert_eq!(dict["visible_to_invocation_ids"], json!(["inv3"]));
}

#[test]
fn test_message_round_trip() {
    let msg = Message::new(
        "user",
        json!("test message"),
        vec![],
        None,
        "xyz789".to_string(),
        Some("user".to_string()),
        90,
        false,
        false,
        None,
        "inv1".to_string(),
        "parent1".to_string(),
        vec![],
    );
    let dict = message_to_dict(&msg);
    let reconstructed = message_from_dict(&dict);
    assert_eq!(reconstructed.role, msg.role);
    assert_eq!(reconstructed.content, msg.content);
    assert_eq!(reconstructed.uuid, msg.uuid);
    assert_eq!(reconstructed.priority, msg.priority);
}

// ── Boundary Marker ─────────────────────────────────────────────────────────

#[test]
fn test_boundary_marker_creation() {
    let marker = BoundaryMarker::new(
        "anchor1",
        "head1",
        "tail1",
        "Compressed summary",
        "layer1",
        1000,
        200,
    );
    assert_eq!(marker.anchor_uuid, "anchor1");
    assert_eq!(marker.head_uuid, "head1");
    assert_eq!(marker.tail_uuid, "tail1");
    assert_eq!(marker.summary, "Compressed summary");
    assert_eq!(marker.layer, "layer1");
    assert_eq!(marker.original_token_count, 1000);
    assert_eq!(marker.compressed_token_count, 200);
    assert!(!marker.uuid.is_empty());
}

#[test]
fn test_boundary_marker_to_dict() {
    let marker = BoundaryMarker::new("a", "h", "t", "summary", "L1", 500, 100);
    let dict = marker.to_dict();
    assert_eq!(dict["type"], "boundary_marker");
    assert_eq!(dict["anchor_uuid"], "a");
    assert_eq!(dict["head_uuid"], "h");
    assert_eq!(dict["tail_uuid"], "t");
    assert_eq!(dict["summary"], "summary");
    assert_eq!(dict["layer"], "L1");
}

#[test]
fn test_boundary_marker_from_dict() {
    let dict = json!({
        "type": "boundary_marker",
        "uuid": "marker1",
        "anchor_uuid": "a",
        "head_uuid": "h",
        "tail_uuid": "t",
        "summary": "test",
        "layer": "L2",
        "timestamp": 1234567890.0,
        "original_token_count": 800,
        "compressed_token_count": 150
    });
    let marker = BoundaryMarker::from_dict(&dict);
    assert_eq!(marker.uuid, "marker1");
    assert_eq!(marker.anchor_uuid, "a");
    assert_eq!(marker.original_token_count, 800);
}

// ── Session Metadata ────────────────────────────────────────────────────────

#[test]
fn test_session_meta_creation() {
    let meta = SessionMeta::new();
    assert!(meta.timestamp > 0.0);
    assert!(meta.argv.is_empty());
}

#[test]
fn test_session_meta_to_dict() {
    let mut meta = SessionMeta::new();
    meta.argv = vec!["helen".to_string(), "test.helen".to_string()];
    meta.helen_version = "1.45.0".to_string();
    meta.session_id = "sess123".to_string();
    let dict = meta.to_dict();
    assert_eq!(dict["type"], "session_meta");
    assert_eq!(dict["helen_version"], "1.45.0");
    assert_eq!(dict["session_id"], "sess123");
    assert_eq!(dict["argv"], json!(["helen", "test.helen"]));
}

#[test]
fn test_session_meta_from_dict() {
    let dict = json!({
        "type": "session_meta",
        "argv": ["helen", "run.helen"],
        "timestamp": 1234567890.0,
        "helen_version": "1.44.0",
        "python_version": "3.10.0",
        "platform": "linux",
        "cwd": "/home/user",
        "session_id": "sess456",
        "session_scope": "global",
        "parent_session_id": ""
    });
    let meta = SessionMeta::from_dict(&dict);
    assert_eq!(meta.argv, vec!["helen", "run.helen"]);
    assert_eq!(meta.helen_version, "1.44.0");
    assert_eq!(meta.session_id, "sess456");
}

// ── Item Enum ───────────────────────────────────────────────────────────────

#[test]
fn test_item_uuid_message() {
    let msg = Message::new(
        "user",
        json!("test"),
        vec![],
        None,
        "msg_uuid".to_string(),
        None,
        90,
        false,
        false,
        None,
        "".to_string(),
        "".to_string(),
        vec![],
    );
    let item = Item::Message(msg);
    assert_eq!(item.uuid(), "msg_uuid");
}

#[test]
fn test_item_uuid_boundary() {
    let marker = BoundaryMarker::new("a", "h", "t", "summary", "L1", 100, 20);
    let item = Item::Boundary(marker);
    assert!(!item.uuid().is_empty());
}

#[test]
fn test_item_from_dict_message() {
    let dict = json!({
        "type": "message",
        "role": "user",
        "content": "hello",
        "tool_calls": [],
        "tool_call_id": null,
        "uuid": "test_uuid",
        "message_type": "user",
        "priority": 90,
        "compressed": false,
        "pinned": false
    });
    let item = Item::from_dict(&dict);
    assert!(item.is_some());
    if let Some(Item::Message(msg)) = item {
        assert_eq!(msg.role, "user");
        assert_eq!(msg.uuid, "test_uuid");
    }
}

#[test]
fn test_item_from_dict_boundary() {
    let dict = json!({
        "type": "boundary_marker",
        "uuid": "marker_uuid",
        "anchor_uuid": "a",
        "head_uuid": "h",
        "tail_uuid": "t",
        "summary": "test",
        "layer": "L1",
        "timestamp": 0.0,
        "original_token_count": 100,
        "compressed_token_count": 20
    });
    let item = Item::from_dict(&dict);
    assert!(item.is_some());
    if let Some(Item::Boundary(marker)) = item {
        assert_eq!(marker.uuid, "marker_uuid");
    }
}

#[test]
fn test_item_from_dict_session_meta_returns_none() {
    let dict = json!({
        "type": "session_meta",
        "argv": [],
        "timestamp": 0.0
    });
    let item = Item::from_dict(&dict);
    assert!(item.is_none());
}

#[test]
fn test_item_from_dict_unknown_type_returns_none() {
    let dict = json!({
        "type": "unknown_type"
    });
    let item = Item::from_dict(&dict);
    assert!(item.is_none());
}
