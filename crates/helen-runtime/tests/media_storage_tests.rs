//! MediaStorage tests (v1.17 Phase 3 parity)
//!
//! Tests for large media external storage functionality.

use helen_runtime::media_storage::MediaStorage;
use helen_runtime::transcript::{Message, TranscriptStore};
use serde_json::json;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// MediaStorage basic functionality
// ---------------------------------------------------------------------------

#[test]
fn media_storage_extract_large_content() {
    let temp_dir = TempDir::new().unwrap();
    let storage = MediaStorage::new(temp_dir.path().to_path_buf(), 1.0).unwrap();

    // Create a message with large content (> 1MB)
    let large_content = "x".repeat(1_500_000); // ~1.5MB
    let message = Message {
        role: "user".to_string(),
        content: json!({
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": "image/png",
                "data": large_content.clone()
            }
        }),
        uuid: "test-uuid-123".to_string(),
        ..Default::default()
    };

    // Extract media
    let extracted = storage.extract_media_from_message(&message).unwrap();

    // Should have media_ref instead of inline data
    assert!(extracted.content.get("media_ref").is_some());
    let media_ref = extracted.content.get("media_ref").unwrap();
    assert_eq!(media_ref["media_type"], "image");
    assert_eq!(media_ref["mime_type"], "image/png");
    assert!(media_ref["path"].as_str().unwrap().ends_with(".bin"));
}

#[test]
fn media_storage_skip_small_content() {
    let temp_dir = TempDir::new().unwrap();
    let storage = MediaStorage::new(temp_dir.path().to_path_buf(), 1.0).unwrap();

    // Create a message with small content (< 1MB)
    let small_content = "x".repeat(1000); // ~1KB
    let message = Message {
        role: "user".to_string(),
        content: json!({
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": "image/png",
                "data": small_content
            }
        }),
        ..Default::default()
    };

    // Extract media
    let extracted = storage.extract_media_from_message(&message).unwrap();

    // Should keep inline data (no media_ref)
    assert!(extracted.content.get("media_ref").is_none());
    assert!(extracted.content.get("source").is_some());
}

#[test]
fn media_storage_restore_media() {
    let temp_dir = TempDir::new().unwrap();
    let storage = MediaStorage::new(temp_dir.path().to_path_buf(), 1.0).unwrap();

    // Create and extract a large message
    let large_content = "x".repeat(1_500_000);
    let original = Message {
        role: "user".to_string(),
        content: json!({
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": "image/png",
                "data": large_content.clone()
            }
        }),
        uuid: "test-uuid-456".to_string(),
        ..Default::default()
    };

    let extracted = storage.extract_media_from_message(&original).unwrap();

    // Restore media
    let restored = storage.restore_media_to_message(&extracted).unwrap();

    // Should have original inline data back
    assert!(restored.content.get("source").is_some());
    let source = restored.content.get("source").unwrap();
    assert_eq!(source["data"], large_content);
}

#[test]
fn media_storage_integrity_check() {
    let temp_dir = TempDir::new().unwrap();
    let storage = MediaStorage::new(temp_dir.path().to_path_buf(), 1.0).unwrap();

    // Create and extract a large message
    let large_content = "x".repeat(1_500_000);
    let original = Message {
        role: "user".to_string(),
        content: json!({
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": "image/png",
                "data": large_content
            }
        }),
        uuid: "test-uuid-789".to_string(),
        ..Default::default()
    };

    let extracted = storage.extract_media_from_message(&original).unwrap();
    let media_ref = extracted.content.get("media_ref").unwrap();

    // Verify integrity metadata
    assert!(media_ref.get("binary_size").is_some());
    assert!(media_ref.get("checksum").is_some());
}

#[test]
fn media_storage_degradation_missing_file() {
    let temp_dir = TempDir::new().unwrap();
    let storage = MediaStorage::new(temp_dir.path().to_path_buf(), 1.0).unwrap();

    // Create a message with media_ref pointing to non-existent file
    let message = Message {
        role: "user".to_string(),
        content: json!({
            "media_ref": {
                "path": "nonexistent.bin",
                "media_type": "image",
                "mime_type": "image/png"
            }
        }),
        ..Default::default()
    };

    // Restore should degrade gracefully
    let restored = storage.restore_media_to_message(&message).unwrap();

    // Should have degradation marker
    assert!(restored.content.get("degraded").is_some());
    assert_eq!(restored.content["degraded"], true);
}

// ---------------------------------------------------------------------------
// Session resume support
// ---------------------------------------------------------------------------

#[test]
fn transcript_store_track_resumed_messages() {
    let mut store = TranscriptStore::new(None, 1000);

    // Add some messages
    let mut msg1 = Message {
        role: "user".to_string(),
        content: json!("Hello"),
        uuid: "uuid-1".to_string(),
        ..Default::default()
    };
    let mut msg2 = Message {
        role: "assistant".to_string(),
        content: json!("Hi there"),
        uuid: "uuid-2".to_string(),
        ..Default::default()
    };

    store.append(&mut msg1, false);
    store.append(&mut msg2, false);

    // Simulate loading from backend (session resume)
    store.mark_as_resumed(&["uuid-1".to_string(), "uuid-2".to_string()]);

    // Verify resumed UUIDs are tracked
    assert!(store.is_resumed_message("uuid-1"));
    assert!(store.is_resumed_message("uuid-2"));
    assert!(!store.is_resumed_message("uuid-3"));
}

#[test]
fn transcript_store_expose_resumed_messages() {
    let mut store = TranscriptStore::new(None, 1000);

    // Add messages with limited visibility
    let mut msg1 = Message {
        role: "user".to_string(),
        content: json!("Hello"),
        uuid: "uuid-1".to_string(),
        visible_to_invocation_ids: vec!["inv-1".to_string()],
        ..Default::default()
    };

    store.append(&mut msg1, false);
    store.mark_as_resumed(&["uuid-1".to_string()]);

    // Expose to new invocation
    let count = store.expose_resumed_messages_to("inv-2");
    assert_eq!(count, 1);

    // Verify message is now visible (read_view returns all messages)
    let view = store.read_view();
    assert_eq!(view.len(), 1);
    // Verify the message now has inv-2 in its visibility list
    assert!(view[0].visible_to_invocation_ids.contains(&"inv-2".to_string()));
}

#[test]
fn transcript_store_expose_only_resumed_messages() {
    let mut store = TranscriptStore::new(None, 1000);

    // Add resumed message
    let mut msg1 = Message {
        role: "user".to_string(),
        content: json!("Resumed"),
        uuid: "uuid-1".to_string(),
        visible_to_invocation_ids: vec!["inv-1".to_string()],
        ..Default::default()
    };

    // Add new message (not resumed)
    let mut msg2 = Message {
        role: "user".to_string(),
        content: json!("New"),
        uuid: "uuid-2".to_string(),
        visible_to_invocation_ids: vec!["inv-1".to_string()],
        ..Default::default()
    };

    store.append(&mut msg1, false);
    store.append(&mut msg2, false);
    store.mark_as_resumed(&["uuid-1".to_string()]);

    // Expose to new invocation
    let count = store.expose_resumed_messages_to("inv-2");
    assert_eq!(count, 1); // Only msg1 should be exposed

    // Verify only resumed message has inv-2 in visibility
    let view = store.read_view();
    assert_eq!(view.len(), 2);
    
    // Find msg1 and verify it has inv-2
    let msg1_view = view.iter().find(|m| m.uuid == "uuid-1").unwrap();
    assert!(msg1_view.visible_to_invocation_ids.contains(&"inv-2".to_string()));
    
    // Find msg2 and verify it does NOT have inv-2
    let msg2_view = view.iter().find(|m| m.uuid == "uuid-2").unwrap();
    assert!(!msg2_view.visible_to_invocation_ids.contains(&"inv-2".to_string()));
}
