//! Tests for skills module — skill loading and discovery.

use helen_runtime::skills::*;
use std::path::Path;

// ── get_skill_dirs tests ────────────────────────────────────────────────

#[test]
fn skill_dirs_returns_vec() {
    let dirs = get_skill_dirs();
    // Just check it doesn't panic
    let _ = dirs;
}

#[test]
fn skill_dirs_no_duplicates() {
    let dirs = get_skill_dirs();
    for i in 0..dirs.len() {
        for j in (i + 1)..dirs.len() {
            assert_ne!(dirs[i], dirs[j]);
        }
    }
}

// ── load_skill tests ────────────────────────────────────────────────────

#[test]
fn load_skill_unknown_returns_error() {
    let v = load_skill("__nonexistent_skill__", false);
    assert!(v.get("error").is_some());
}

#[test]
fn load_skill_unknown_error_message() {
    let v = load_skill("__nonexistent_skill__", false);
    let err = v["error"].as_str().unwrap();
    assert!(err.contains("__nonexistent_skill__"));
}

#[test]
fn load_skill_known_has_content() {
    // Try loading a known skill (helen-syntax should exist in bundled skills)
    let v = load_skill("helen-syntax", false);
    if v.get("error").is_none() {
        assert!(v.get("content").is_some());
        assert!(v.get("path").is_some());
        assert!(v.get("name").is_some());
    }
}

#[test]
fn load_skill_with_references() {
    let v = load_skill("helen-syntax", true);
    if v.get("error").is_none() {
        // Should have references field when include_references=true
        assert!(v.get("references").is_some());
    }
}

#[test]
fn load_skill_without_references() {
    let v = load_skill("helen-syntax", false);
    if v.get("error").is_none() {
        // Should NOT have references field when include_references=false
        assert!(v.get("references").is_none());
    }
}

// ── list_skill_references tests ─────────────────────────────────────────

#[test]
fn list_references_unknown_returns_error() {
    let v = list_skill_references("__nonexistent_skill__");
    assert!(v.get("error").is_some());
}

#[test]
fn list_references_known_has_refs() {
    let v = list_skill_references("helen-syntax");
    if v.get("error").is_none() {
        assert!(v.get("references").is_some());
        assert!(v.get("skill_path").is_some());
    }
}

#[test]
fn list_references_has_total_or_message() {
    let v = list_skill_references("helen-syntax");
    if v.get("error").is_none() {
        // Either has "total" (if refs dir exists) or "message" (if no refs dir)
        assert!(v.get("total").is_some() || v.get("message").is_some());
    }
}

#[test]
fn list_references_ref_has_preview() {
    let v = list_skill_references("helen-syntax");
    if v.get("error").is_none() {
        if let Some(refs) = v["references"].as_array() {
            if !refs.is_empty() {
                let first = &refs[0];
                assert!(first.get("name").is_some());
                assert!(first.get("path").is_some());
                assert!(first.get("size").is_some());
                assert!(first.get("preview").is_some());
            }
        }
    }
}

// ── is_within_project tests ─────────────────────────────────────────────

#[test]
fn is_within_project_cwd() {
    let cwd = std::env::current_dir().unwrap();
    assert!(is_within_project(&cwd));
}

#[test]
fn is_within_project_subdir() {
    let cwd = std::env::current_dir().unwrap();
    let sub = cwd.join("subdir");
    assert!(is_within_project(&sub));
}

#[test]
fn is_within_project_absolute_outside() {
    // Absolute path outside cwd should still return true (Python parity)
    let outside = Path::new("/tmp/some/path");
    assert!(is_within_project(outside));
}
