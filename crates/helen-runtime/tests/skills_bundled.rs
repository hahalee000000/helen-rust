//! Bundled-skills discoverability test (M14 wiki/skills deliverable).
//!
//! Verifies that the 17 bundled skills under `crates/helen-runtime/skills/`
//! are found by the runtime's `get_skill_dirs` + `load_skill` machinery.

use helen_runtime::skills::{get_skill_dirs, load_skill};

#[test]
fn bundled_skills_dir_is_discovered() {
    let dirs = get_skill_dirs();
    assert!(!dirs.is_empty(), "expected at least one skill dir");
    let bundled = dirs
        .iter()
        .any(|d| d.file_name().map(|n| n == "skills").unwrap_or(false));
    assert!(
        bundled,
        "expected bundled skills/ dir in search path, got: {dirs:?}"
    );
}

#[test]
fn load_skill_finds_bundled_skills() {
    // The differential-porting skill is helen-rust specific and only shipped
    // in the bundled dir — a good canary.
    let res = load_skill("differential-porting", false);
    let name = res.get("name").and_then(|v| v.as_str()).unwrap_or("");
    assert_eq!(name, "differential-porting");
    let content = res
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert!(content.contains("Differential Conformance Porting"));
}

#[test]
fn helen_syntax_skill_has_rust_edition_note() {
    let res = load_skill("helen-syntax", false);
    let content = res
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    assert!(
        content.contains("helen-rust edition"),
        "helen-syntax should carry the rust-edition note"
    );
}
