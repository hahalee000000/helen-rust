//! Tests for import_resolver module — ImportResolver, FileRegistry, path resolution.

use helen_interpreter::import_resolver::*;
use std::path::{Path, PathBuf};
use std::fs;

fn tmp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("helen_test_import_{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

// ── ImportResolver basic tests ──────────────────────────────────────────

#[test]
fn resolver_new() {
    let dir = tmp_dir("new");
    let r = ImportResolver::new(dir.clone());
    assert!(r.load_order().is_empty());
    let _ = dir;
}

#[test]
fn resolver_python_import() {
    let dir = tmp_dir("py");
    let mut r = ImportResolver::new(dir);
    // Non-helen data file -> Python
    let result = r.resolve("math", None).unwrap();
    match result {
        ResolvedImport::Python => {},
        _ => panic!("expected Python"),
    }
}

#[test]
fn resolver_python_file_import() {
    let dir = tmp_dir("pyfile");
    let mut r = ImportResolver::new(dir);
    let result = r.resolve("foo.py", None).unwrap();
    match result {
        ResolvedImport::Python => {},
        _ => panic!("expected Python"),
    }
}

#[test]
fn resolver_missing_file() {
    let dir = tmp_dir("missing");
    let mut r = ImportResolver::new(dir);
    let result = r.resolve("nonexistent.helen", None);
    match result {
        Err(e) => assert!(e.contains("not found")),
        Ok(_) => panic!("expected error"),
    }
}

#[test]
fn resolver_load_helen_file() {
    let dir = tmp_dir("helen");
    let helen_file = dir.join("utils.helen");
    fs::write(&helen_file, "fn add(a, b) { return a + b }\nconst X = 42\n").unwrap();
    let mut r = ImportResolver::new(dir);
    let result = r.resolve("utils.helen", None).unwrap();
    match result {
        ResolvedImport::Helen { path } => {
            assert!(path.to_string_lossy().contains("utils.helen"));
        },
        _ => panic!("expected Helen"),
    }
    assert_eq!(r.load_order().len(), 1);
    let reg = r.file(&helen_file.canonicalize().unwrap_or(helen_file.clone()));
    // The file registry should have the function
    if let Some(reg) = reg {
        assert!(!reg.functions.is_empty());
    }
}

#[test]
fn resolver_load_json_file() {
    let dir = tmp_dir("json");
    let json_file = dir.join("data.json");
    fs::write(&json_file, r#"{"key": "value", "num": 42}"#).unwrap();
    let mut r = ImportResolver::new(dir);
    let result = r.resolve("data.json", None).unwrap();
    match result {
        ResolvedImport::Data { alias, value } => {
            assert_eq!(alias, "data");
            // Value should be a map
            match value {
                helen_interpreter::value::Value::Map(m) => {
                    assert!(!m.borrow().is_empty());
                },
                _ => panic!("expected Map"),
            }
        },
        _ => panic!("expected Data"),
    }
}

#[test]
fn resolver_load_text_file() {
    let dir = tmp_dir("text");
    let txt_file = dir.join("readme.txt");
    fs::write(&txt_file, "Hello, world!").unwrap();
    let mut r = ImportResolver::new(dir);
    let result = r.resolve("readme.txt", None).unwrap();
    match result {
        ResolvedImport::Data { alias, value } => {
            assert_eq!(alias, "readme");
            match value {
                helen_interpreter::value::Value::Str(s) => {
                    assert_eq!(s.as_ref(), "Hello, world!");
                },
                _ => panic!("expected Str"),
            }
        },
        _ => panic!("expected Data"),
    }
}

#[test]
fn resolver_load_yaml_as_text() {
    let dir = tmp_dir("yaml");
    let yaml_file = dir.join("config.yaml");
    fs::write(&yaml_file, "key: value\n").unwrap();
    let mut r = ImportResolver::new(dir);
    let result = r.resolve("config.yaml", None).unwrap();
    match result {
        ResolvedImport::Data { alias, .. } => {
            assert_eq!(alias, "config");
        },
        _ => panic!("expected Data"),
    }
}

#[test]
fn resolver_circular_import() {
    let dir = tmp_dir("circular");
    let helen_file = dir.join("a.helen");
    fs::write(&helen_file, "fn foo() { return 1 }\n").unwrap();
    let mut r = ImportResolver::new(dir.clone());
    // First load
    let _ = r.resolve("a.helen", None);
    // Second load (circular) should return Helen without re-parsing
    let result = r.resolve("a.helen", None).unwrap();
    match result {
        ResolvedImport::Helen { .. } => {},
        _ => panic!("expected Helen for circular import"),
    }
    // Load order should only have one entry
    assert_eq!(r.load_order().len(), 1);
}

#[test]
fn resolver_from_file_relative() {
    let dir = tmp_dir("relative");
    let sub = dir.join("sub");
    fs::create_dir_all(&sub).unwrap();
    let helen_file = sub.join("utils.helen");
    fs::write(&helen_file, "fn helper() { return 1 }\n").unwrap();
    let mut r = ImportResolver::new(dir);
    let from_file = sub.join("main.helen");
    fs::write(&from_file, "").unwrap();
    let result = r.resolve("utils.helen", Some(&from_file)).unwrap();
    match result {
        ResolvedImport::Helen { .. } => {},
        _ => panic!("expected Helen"),
    }
}

#[test]
fn resolver_nested_imports() {
    let dir = tmp_dir("nested");
    let a_file = dir.join("a.helen");
    let b_file = dir.join("b.helen");
    fs::write(&b_file, "fn bfunc() { return 2 }\n").unwrap();
    fs::write(&a_file, "import \"b.helen\" as b\nfn afunc() { return 1 }\n").unwrap();
    let mut r = ImportResolver::new(dir);
    let _ = r.resolve("a.helen", None);
    // Both files should be in load order
    assert_eq!(r.load_order().len(), 2);
}

#[test]
fn resolver_file_registry() {
    let dir = tmp_dir("registry");
    let helen_file = dir.join("lib.helen");
    fs::write(&helen_file, "fn foo() { return 1 }\nfn bar() { return 2 }\nconst X = 10\n").unwrap();
    let mut r = ImportResolver::new(dir);
    let _ = r.resolve("lib.helen", None);
    let abs = helen_file.canonicalize().unwrap_or(helen_file);
    let reg = r.file(&abs);
    assert!(reg.is_some());
    let reg = reg.unwrap();
    assert_eq!(reg.functions.len(), 2);
    assert!(!reg.data.is_empty()); // const X
}

#[test]
fn resolver_file_not_found() {
    let dir = tmp_dir("notfound");
    let r = ImportResolver::new(dir);
    let abs = Path::new("/nonexistent/path/file.helen");
    assert!(r.file(abs).is_none());
}

#[test]
fn resolver_json_parse_error() {
    let dir = tmp_dir("jsonerr");
    let json_file = dir.join("bad.json");
    fs::write(&json_file, "{invalid json}").unwrap();
    let mut r = ImportResolver::new(dir);
    let result = r.resolve("bad.json", None);
    match result {
        Err(_) => {}, // expected
        Ok(_) => panic!("expected error"),
    }
}

#[test]
fn resolver_md_file_as_text() {
    let dir = tmp_dir("md");
    let md_file = dir.join("doc.md");
    fs::write(&md_file, "# Title\nContent").unwrap();
    let mut r = ImportResolver::new(dir);
    let result = r.resolve("doc.md", None).unwrap();
    match result {
        ResolvedImport::Data { alias, value } => {
            assert_eq!(alias, "doc");
            match value {
                helen_interpreter::value::Value::Str(s) => {
                    assert!(s.as_ref().contains("# Title"));
                },
                _ => panic!("expected Str"),
            }
        },
        _ => panic!("expected Data"),
    }
}
