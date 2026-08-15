//! Standard-library registry for import/alias analysis.
//!
//! Data is generated from the Python reference at build time:
//! `tests/conformance/refdata/stdlib_full.json` (v1.44.0) — module export
//! names, canonical builtin names, and Chinese aliases.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

const STDLIB_DATA: &str = include_str!("stdlib_data.json");

/// Lazily parsed stdlib registry.
struct StdlibData {
    /// Module name → exported function names (`module_class.__exports__`).
    exports: HashMap<String, Vec<String>>,
    /// Alias → canonical builtin name (`stdlib.aliases`).
    aliases: HashMap<String, String>,
    /// Set of canonical builtin names (`stdlib.canonicals`).
    canonicals: Vec<String>,
}

fn stdlib_data() -> &'static StdlibData {
    static DATA: OnceLock<StdlibData> = OnceLock::new();
    DATA.get_or_init(|| {
        let root: Value = serde_json::from_str(STDLIB_DATA)
            .expect("stdlib_data.json must parse (generated from Python reference)");
        let exports = root
            .get("exports")
            .and_then(Value::as_object)
            .map(|obj| {
                obj.iter()
                    .map(|(k, v)| {
                        (
                            k.clone(),
                            v.as_array()
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                        .collect()
                                })
                                .unwrap_or_default(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        let aliases = root
            .get("aliases")
            .and_then(Value::as_object)
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        let canonicals = root
            .get("canonicals")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();
        StdlibData {
            exports,
            aliases,
            canonicals,
        }
    })
}

/// Known stdlib module names (`module_map` keys in `_analyze_stdlib_import`).
pub fn known_modules() -> Vec<String> {
    stdlib_data().exports.keys().cloned().collect()
}

/// Export names for a stdlib module (or None if the module is unknown).
pub fn module_exports(module_name: &str) -> Option<&Vec<String>> {
    stdlib_data().exports.get(module_name)
}

/// `stdlib.lookup(name) is not None` — whether `name` is a canonical builtin.
pub fn is_canonical_builtin(name: &str) -> bool {
    stdlib_data().canonicals.iter().any(|c| c == name)
}

/// `stdlib.canonical_name(alias)` — map an alias to its canonical name.
pub fn canonical_name(alias: &str) -> Option<&str> {
    stdlib_data().aliases.get(alias).map(|s| s.as_str())
}

/// `alias in stdlib.aliases` — whether `alias` is a registered stdlib alias.
pub fn is_alias(name: &str) -> bool {
    stdlib_data().aliases.contains_key(name)
}

/// All (alias, canonical) pairs — Python `stdlib.aliases.items()`.
pub fn all_aliases() -> Vec<(String, String)> {
    stdlib_data()
        .aliases
        .iter()
        .map(|(a, c)| (a.clone(), c.clone()))
        .collect()
}
