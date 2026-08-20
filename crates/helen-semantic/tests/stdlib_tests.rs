//! Tests for semantic stdlib module — stdlib registry for import/alias analysis.

use helen_semantic::stdlib::*;

// ── known_modules tests ─────────────────────────────────────────────────

#[test]
fn known_modules_not_empty() {
    let modules = known_modules();
    assert!(!modules.is_empty());
}

#[test]
fn known_modules_contains_std_core() {
    let modules = known_modules();
    assert!(modules.iter().any(|m| m.contains("std")));
}

#[test]
fn known_modules_no_duplicates() {
    let modules = known_modules();
    let mut seen = std::collections::HashSet::new();
    for m in &modules {
        assert!(seen.insert(m.clone()), "duplicate module: {m}");
    }
}

// ── module_exports tests ────────────────────────────────────────────────

#[test]
fn module_exports_unknown() {
    let exports = module_exports("std.nonexistent_module");
    assert!(exports.is_none());
}

#[test]
fn module_exports_known() {
    // Try a few common modules
    let modules = known_modules();
    if let Some(first) = modules.first() {
        let exports = module_exports(first);
        assert!(exports.is_some());
    }
}

#[test]
fn module_exports_non_empty() {
    let modules = known_modules();
    for m in modules.iter().take(5) {
        if let Some(exports) = module_exports(m) {
            assert!(!exports.is_empty(), "module {m} has no exports");
        }
    }
}

// ── is_canonical_builtin tests ──────────────────────────────────────────

#[test]
fn is_canonical_builtin_unknown() {
    assert!(!is_canonical_builtin("__nonexistent_builtin__"));
}

#[test]
fn is_canonical_builtin_common() {
    // Common builtins should be canonical
    let common = [
        "len", "print", "type", "str", "int", "float", "bool", "list", "map",
    ];
    let mut found = 0;
    for name in &common {
        if is_canonical_builtin(name) {
            found += 1;
        }
    }
    assert!(
        found > 0,
        "at least some common builtins should be canonical"
    );
}

// ── canonical_name tests ────────────────────────────────────────────────

#[test]
fn canonical_name_unknown() {
    let result = canonical_name("__nonexistent_alias__");
    assert!(result.is_none());
}

#[test]
fn canonical_name_returns_str() {
    let aliases = all_aliases();
    if let Some((alias, _)) = aliases.first() {
        let result = canonical_name(alias);
        assert!(result.is_some());
    }
}

// ── is_alias tests ──────────────────────────────────────────────────────

#[test]
fn is_alias_unknown() {
    assert!(!is_alias("__nonexistent_alias__"));
}

#[test]
fn is_alias_true_for_known() {
    let aliases = all_aliases();
    if let Some((alias, _)) = aliases.first() {
        assert!(is_alias(alias));
    }
}

// ── all_aliases tests ───────────────────────────────────────────────────

#[test]
fn all_aliases_not_empty() {
    let aliases = all_aliases();
    assert!(!aliases.is_empty());
}

#[test]
fn all_aliases_pairs() {
    let aliases = all_aliases();
    for (alias, canonical) in &aliases {
        assert!(!alias.is_empty());
        assert!(!canonical.is_empty());
    }
}

#[test]
fn all_aliases_no_duplicate_keys() {
    let aliases = all_aliases();
    let mut seen = std::collections::HashSet::new();
    for (alias, _) in &aliases {
        assert!(seen.insert(alias.clone()), "duplicate alias: {alias}");
    }
}

#[test]
fn all_aliases_canonical_is_builtin() {
    let aliases = all_aliases();
    for (_, canonical) in aliases.iter().take(10) {
        assert!(
            is_canonical_builtin(canonical),
            "alias canonical name '{canonical}' should be a canonical builtin"
        );
    }
}

// ── Round-trip tests ────────────────────────────────────────────────────

#[test]
fn alias_to_canonical_roundtrip() {
    let aliases = all_aliases();
    for (alias, expected_canonical) in aliases.iter().take(10) {
        let result = canonical_name(alias);
        assert_eq!(result, Some(expected_canonical.as_str()));
    }
}

#[test]
fn is_alias_consistent_with_all_aliases() {
    let aliases = all_aliases();
    for (alias, _) in &aliases {
        assert!(
            is_alias(alias),
            "is_alias should be true for alias from all_aliases"
        );
    }
}
