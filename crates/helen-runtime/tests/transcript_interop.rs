//! M14 D8 — Transcript/JSONL interoperability between the Python reference
//! and the Rust runtime.
//!
//! The JSONL line format is the interchange contract:
//!   {"type":"message","role":...,"content":...,"uuid":..., ...}
//!
//! These tests use fixture files in `tests/fixtures/jsonl/`:
//!   - `python_written.jsonl` — captured from the Python reference's
//!     `JSONLBackend.append()` output (M14, verified byte-identical).
//!   - `rust_written.jsonl` — written by this crate's `JsonlBackend`.
//!
//! Both files must round-trip through the *other* implementation. The
//! Python side of the check lives in `tests/fixtures/jsonl/check_python.py`.

use helen_runtime::transcript::{Item, JsonlBackend, Message};

fn fixture(name: &str) -> String {
    let root = env!("CARGO_MANIFEST_DIR");
    format!("{root}/../../tests/fixtures/jsonl/{name}")
}

#[test]
fn rust_reads_python_written_jsonl() {
    let b = JsonlBackend::new(fixture("python_written.jsonl"));
    let items = b.load_all();
    let msgs: Vec<_> = items
        .iter()
        .filter_map(|i| match i {
            Item::Message(m) => Some(m),
            _ => None,
        })
        .collect();
    assert_eq!(msgs.len(), 2, "both python messages must parse");
    assert_eq!(msgs[0].role, "user");
    assert_eq!(msgs[0].uuid, "u-1");
    assert_eq!(msgs[0].content, serde_json::json!("hello from python"));
    assert_eq!(msgs[1].role, "assistant");
    assert_eq!(msgs[1].uuid, "u-2");
    assert_eq!(msgs[1].content, serde_json::json!("hi there"));
}

#[test]
fn rust_writes_jsonl_python_can_read() {
    // Write a fresh JSONL with the Rust backend, then the accompanying
    // Python fixture check must read it back (see check_python.py). Here we
    // at least assert the written format matches the Python line contract.
    let p = fixture("rust_written.jsonl");
    // The fixture is regenerated on each run: clear stale content first.
    let _ = std::fs::remove_file(&p);
    let b = JsonlBackend::new(p);
    let m = Message::new(
        "user",
        serde_json::json!("rust wrote this"),
        vec![],
        None,
        "r-1".into(),
        None,
        50,
        false,
        false,
        None,
        String::new(),
        String::new(),
        vec![],
    );
    let m2 = Message::new(
        "assistant",
        serde_json::json!("and rust replied"),
        vec![],
        None,
        "r-2".into(),
        None,
        50,
        false,
        false,
        None,
        String::new(),
        String::new(),
        vec![],
    );
    b.append(&Item::Message(m));
    b.append(&Item::Message(m2));

    let content = std::fs::read_to_string(fixture("rust_written.jsonl")).unwrap();
    let lines: Vec<serde_json::Value> = content
        .lines()
        .map(|l| serde_json::from_str(l).expect("each line is valid JSON"))
        .collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0]["type"], "message");
    assert_eq!(lines[0]["role"], "user");
    assert_eq!(lines[0]["uuid"], "r-1");
    assert_eq!(lines[0]["content"], "rust wrote this");
    assert_eq!(lines[1]["role"], "assistant");
    assert_eq!(lines[1]["uuid"], "r-2");
}
