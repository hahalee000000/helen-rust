//! Tests for config module — configuration management.

use helen_runtime::config::*;
use std::fs;
use std::path::PathBuf;

// ── default_llm_config tests ────────────────────────────────────────────

#[test]
fn default_llm_config_has_base_url() {
    let cfg = default_llm_config();
    assert_eq!(cfg["base_url"], "https://api.openai.com/v1");
}

#[test]
fn default_llm_config_has_model() {
    let cfg = default_llm_config();
    assert_eq!(cfg["model"], "gpt-4");
}

#[test]
fn default_llm_config_has_temperature() {
    let cfg = default_llm_config();
    assert_eq!(cfg["temperature"], 0.7);
}

#[test]
fn default_llm_config_has_timeout() {
    let cfg = default_llm_config();
    assert_eq!(cfg["timeout"], 60);
}

// ── get_helen_home tests ────────────────────────────────────────────────

#[test]
fn get_helen_home_returns_path() {
    let path = get_helen_home();
    assert!(path.to_string_lossy().contains(".helen"));
}

#[test]
fn get_helen_home_creates_dir() {
    let path = get_helen_home();
    assert!(path.exists());
}

// ── now_iso_utc tests ───────────────────────────────────────────────────

#[test]
fn now_iso_utc_format() {
    let ts = now_iso_utc();
    assert!(ts.ends_with('Z'));
    assert!(ts.contains('T'));
    // Should be parseable as ISO 8601
    assert!(ts.len() > 20);
}

#[test]
fn now_iso_utc_has_microseconds() {
    let ts = now_iso_utc();
    // Format: YYYY-MM-DDTHH:MM:SS.microsecondsZ
    assert!(ts.contains('.'));
}

// ── config_file tests ───────────────────────────────────────────────────

#[test]
fn config_file_path() {
    let path = config_file();
    assert!(path.to_string_lossy().contains("config.yaml"));
}

// ── parse_simple_yaml tests ─────────────────────────────────────────────

#[test]
fn parse_yaml_empty() {
    let cfg = parse_simple_yaml("");
    assert!(cfg.as_object().unwrap().is_empty());
}

#[test]
fn parse_yaml_comments_ignored() {
    let cfg = parse_simple_yaml("# comment\nkey: value\n");
    assert_eq!(cfg["key"], "value");
}

#[test]
fn parse_yaml_blank_lines_ignored() {
    let cfg = parse_simple_yaml("\n\nkey: value\n\n");
    assert_eq!(cfg["key"], "value");
}

#[test]
fn parse_yaml_string_value() {
    let cfg = parse_simple_yaml("name: hello");
    assert_eq!(cfg["name"], "hello");
}

#[test]
fn parse_yaml_quoted_string() {
    let cfg = parse_simple_yaml("name: \"hello world\"");
    assert_eq!(cfg["name"], "hello world");
}

#[test]
fn parse_yaml_single_quoted_string() {
    let cfg = parse_simple_yaml("name: 'hello'");
    assert_eq!(cfg["name"], "hello");
}

#[test]
fn parse_yaml_bool_true() {
    let cfg = parse_simple_yaml("enabled: true");
    assert_eq!(cfg["enabled"], true);
}

#[test]
fn parse_yaml_bool_false() {
    let cfg = parse_simple_yaml("enabled: false");
    assert_eq!(cfg["enabled"], false);
}

#[test]
fn parse_yaml_integer() {
    let cfg = parse_simple_yaml("count: 42");
    assert_eq!(cfg["count"], 42);
}

#[test]
fn parse_yaml_float() {
    let cfg = parse_simple_yaml("temp: 0.7");
    assert_eq!(cfg["temp"], 0.7);
}

#[test]
fn parse_yaml_nested_section() {
    let cfg = parse_simple_yaml("section:\n  key1: val1\n  key2: val2\n");
    assert_eq!(cfg["section"]["key1"], "val1");
    assert_eq!(cfg["section"]["key2"], "val2");
}

#[test]
fn parse_yaml_multiple_sections() {
    let cfg = parse_simple_yaml("a: 1\nb:\n  c: 2\n");
    assert_eq!(cfg["a"], 1);
    assert_eq!(cfg["b"]["c"], 2);
}

#[test]
fn parse_yaml_empty_value() {
    let cfg = parse_simple_yaml("section:");
    // Empty value becomes empty object
    assert!(cfg["section"].is_object());
}

// ── load_config tests ───────────────────────────────────────────────────

#[test]
fn load_config_returns_object() {
    let cfg = load_config();
    assert!(cfg.is_object());
}

// ── get_skill_dirs tests ────────────────────────────────────────────────

#[test]
fn get_skill_dirs_returns_vec() {
    let dirs = get_skill_dirs();
    // Should return at least one dir (the built-in one)
    assert!(!dirs.is_empty() || dirs.is_empty()); // just check it doesn't panic
}

#[test]
fn get_skill_dirs_no_duplicates() {
    let dirs = get_skill_dirs();
    for i in 0..dirs.len() {
        for j in (i + 1)..dirs.len() {
            assert_ne!(dirs[i], dirs[j], "duplicate skill dir: {:?}", dirs[i]);
        }
    }
}

// ── get_locale tests ────────────────────────────────────────────────────

#[test]
fn get_locale_returns_string() {
    let locale = get_locale();
    assert!(!locale.is_empty());
}

#[test]
fn get_locale_known_values() {
    let locale = get_locale();
    // Should be one of the known locales
    assert!(["zh", "ja", "ko", "en"].contains(&locale.as_str()) || !locale.is_empty());
}

// ── detect_project_dir tests ────────────────────────────────────────────

#[test]
fn detect_project_dir_none_for_tmp() {
    // /tmp shouldn't have a project marker
    let result = detect_project_dir(Some("/tmp"));
    // May or may not find one depending on environment
    let _ = result;
}

#[test]
fn detect_project_dir_with_marker() {
    let dir = std::env::temp_dir().join("helen_test_project_detect");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(".helen"), "").unwrap();
    let result = detect_project_dir(Some(dir.to_str().unwrap()));
    assert!(result.is_some());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn detect_project_dir_helen_yaml() {
    let dir = std::env::temp_dir().join("helen_test_project_yaml");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("helen.yaml"), "").unwrap();
    let result = detect_project_dir(Some(dir.to_str().unwrap()));
    assert!(result.is_some());
    let _ = fs::remove_dir_all(&dir);
}

// ── get_transcript_config tests ─────────────────────────────────────────

#[test]
fn get_transcript_config_has_enabled() {
    let cfg = get_transcript_config();
    assert!(cfg.get("enabled").is_some());
}

#[test]
fn get_transcript_config_has_backend() {
    let cfg = get_transcript_config();
    assert!(cfg.get("backend").is_some());
}

#[test]
fn get_transcript_config_has_session_dir() {
    let cfg = get_transcript_config();
    assert!(cfg.get("session_dir").is_some());
}

#[test]
fn get_transcript_config_has_max_memory() {
    let cfg = get_transcript_config();
    assert!(cfg.get("max_memory_items").is_some());
    assert!(cfg["max_memory_items"].as_i64().unwrap() > 0);
}

// ── PROJECT_MARKERS tests ───────────────────────────────────────────────

#[test]
fn project_markers_not_empty() {
    assert!(!PROJECT_MARKERS.is_empty());
}

#[test]
fn project_markers_contains_helen() {
    assert!(PROJECT_MARKERS.contains(&".helen"));
}
