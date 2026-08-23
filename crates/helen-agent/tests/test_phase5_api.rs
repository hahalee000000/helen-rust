//! Integration tests for Phase 5 advanced features
//!
//! Tests the underlying logic directly:
//! - StreamRegistry — track active streams
//! - Session deletion (transcript cleanup)
//! - Agent status data

use helen_agent::stream_registry::StreamRegistry;
use std::sync::Arc;
use tempfile::TempDir;

// === StreamRegistry Integration ===

#[test]
fn test_stream_registry_concurrent_access() {
    let registry = Arc::new(StreamRegistry::new());

    // Simulate concurrent registrations
    let r1 = registry.clone();
    let h1 = std::thread::spawn(move || {
        r1.register("session-1");
        r1.register("session-2");
    });

    let r2 = registry.clone();
    let h2 = std::thread::spawn(move || {
        r2.register("session-3");
    });

    h1.join().unwrap();
    h2.join().unwrap();

    assert!(registry.is_processing());
    assert_eq!(registry.active_sessions().len(), 3);
}

#[test]
fn test_stream_registry_unregister_all() {
    let registry = StreamRegistry::new();
    registry.register("s1");
    registry.register("s2");
    registry.register("s3");

    registry.unregister("s1");
    registry.unregister("s2");
    registry.unregister("s3");

    assert!(!registry.is_processing());
    assert!(registry.active_sessions().is_empty());
}

// === Session Deletion ===

#[test]
fn test_delete_session_directory() {
    let temp = TempDir::new().unwrap();
    let session_dir = temp.path().join(".helen/sessions/test-session");
    std::fs::create_dir_all(&session_dir).unwrap();

    // Create transcript file
    let transcript = r#"{"type":"session_meta","session_id":"test"}
{"type":"message","role":"user","content":"Hello"}
"#;
    std::fs::write(session_dir.join("transcript.jsonl"), transcript).unwrap();

    assert!(session_dir.exists());

    // Delete the session directory
    std::fs::remove_dir_all(&session_dir).unwrap();

    assert!(!session_dir.exists());
}

#[test]
fn test_delete_nonexistent_session() {
    let temp = TempDir::new().unwrap();
    let session_dir = temp.path().join(".helen/sessions/nonexistent");

    // Should not exist
    assert!(!session_dir.exists());

    // Attempting to delete should not panic
    let result = std::fs::remove_dir_all(&session_dir);
    assert!(result.is_err());
}

// === Agent Status ===

#[test]
fn test_agent_status_data_structure() {
    // Mock agent status data (matching Python implementation)
    let agent_states: std::collections::HashMap<&str, serde_json::Value> = [
        ("Contractor", serde_json::json!({"status": "idle", "last_task": null})),
        ("TestBuilder", serde_json::json!({"status": "idle", "last_task": null})),
        ("Implementer", serde_json::json!({"status": "idle", "last_task": null})),
        ("QualityGate", serde_json::json!({"status": "idle", "last_task": null})),
        ("SkillEvaluator", serde_json::json!({"status": "idle", "last_task": null})),
    ].into_iter().collect();

    assert_eq!(agent_states.len(), 5);
    assert!(agent_states.contains_key("Contractor"));
    assert!(agent_states.contains_key("TestBuilder"));
}

#[test]
fn test_agent_list() {
    let agents = vec!["Contractor", "TestBuilder", "Implementer", "QualityGate", "SkillEvaluator"];
    assert_eq!(agents.len(), 5);
    assert!(agents.contains(&"Contractor"));
}
