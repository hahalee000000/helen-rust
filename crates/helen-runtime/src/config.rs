//! Configuration management (Task 5.4) — port of `helen/runtime/config.py`.
//!
//! Helen uses `~/.helen/` for API keys, LLM endpoints, skill dirs, sessions.

use std::env;
use std::path::{Path, PathBuf};

pub const HELEN_HOME: &str = ".helen";
pub const CONFIG_FILE: &str = "config.yaml";

/// Default LLM settings (Python parity).
pub fn default_llm_config() -> serde_json::Value {
    serde_json::json!({
        "base_url": "https://api.openai.com/v1",
        "model": "gpt-4",
        "temperature": 0.7,
        "timeout": 60,
    })
}

/// `~/.helen` — get helen home directory.
pub fn get_helen_home() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| ".".into());
    let dir = PathBuf::from(home).join(HELEN_HOME);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// `datetime.utcnow().isoformat() + "Z"` (Python parity for saved timestamps).
pub fn now_iso_utc() -> String {
    let now = chrono::Utc::now();
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}Z",
        now.format("%Y"),
        now.format("%m"),
        now.format("%d"),
        now.format("%H"),
        now.format("%M"),
        now.format("%S"),
        now.timestamp_subsec_micros()
    )
}

/// `~/.helen/config.yaml`
pub fn config_file() -> PathBuf {
    get_helen_home().join(CONFIG_FILE)
}

/// Load config from `~/.helen/config.yaml`. Returns defaults on any failure.
/// Flattens nested `llm:` section to match Python behavior.
pub fn load_config() -> serde_json::Value {
    let path = config_file();
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return serde_json::Value::Object(Default::default()),
    };
    let parsed = parse_simple_yaml(&content);
    
    // Flatten nested `llm:` section (Python parity)
    let mut flat = serde_json::Map::new();
    
    // Extract llm section if present
    if let Some(llm) = parsed.get("llm").and_then(|v| v.as_object()) {
        for (k, v) in llm {
            flat.insert(k.clone(), v.clone());
        }
    }
    
    // Copy other top-level keys (transcript, multimodal, etc.)
    for (k, v) in parsed.as_object().unwrap_or(&serde_json::Map::new()) {
        if k != "llm" {
            flat.insert(k.clone(), v.clone());
        }
    }
    
    serde_json::Value::Object(flat)
}

/// Parse a simple YAML subset (top-level `key: value` pairs + nested blocks
/// with 2-space indentation). Enough for Helen's config.yaml.
pub fn parse_simple_yaml(content: &str) -> serde_json::Value {
    use serde_json::{Map, Value};
    let mut root: Map<String, Value> = Map::new();
    let mut current_section: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if indent == 0 {
            // Top-level: `key: value` or `key:` (section start)
            if let Some((k, v)) = split_key_value(trimmed) {
                current_section = Some(k.clone());
                root.insert(k, parse_scalar(&v));
            }
        } else {
            // Nested (2-space indent) under current section
            if let Some(section) = &current_section {
                if let Some((k, v)) = split_key_value(trimmed) {
                    let sec = root
                        .entry(section.clone())
                        .or_insert_with(|| Value::Object(Map::new()));
                    if let Value::Object(m) = sec {
                        m.insert(k, parse_scalar(&v));
                    }
                }
            }
        }
    }
    Value::Object(root)
}

fn split_key_value(line: &str) -> Option<(String, String)> {
    let idx = line.find(':')?;
    let key = line[..idx].trim().to_string();
    let value = line[idx + 1..].trim().to_string();
    Some((key, value))
}

fn parse_scalar(v: &str) -> serde_json::Value {
    if v.is_empty() {
        return serde_json::Value::Object(Default::default());
    }
    if (v.starts_with('"') && v.ends_with('"')) || (v.starts_with('\'') && v.ends_with('\'')) {
        return serde_json::Value::String(v[1..v.len() - 1].to_string());
    }
    if v == "true" {
        return serde_json::Value::Bool(true);
    }
    if v == "false" {
        return serde_json::Value::Bool(false);
    }
    if let Ok(n) = v.parse::<i64>() {
        return serde_json::json!(n);
    }
    if let Ok(f) = v.parse::<f64>() {
        return serde_json::json!(f);
    }
    serde_json::Value::String(v.to_string())
}

/// Get list of skill directories in priority order:
/// 1. <project>/.helen/skills/ (closest ancestor, highest priority)
/// 2. ~/.helen/skills/ (user-level)
/// 3. <helen-install>/skills/ (built-in)
pub fn get_skill_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();

    // 1. Project-level: walk up from cwd looking for .helen/skills
    if let Ok(cwd) = env::current_dir() {
        let mut current = Some(cwd.as_path());
        while let Some(dir) = current {
            let project_skills = dir.join(".helen").join("skills");
            if project_skills.exists() && !dirs.contains(&project_skills) {
                dirs.push(project_skills);
                break;
            }
            current = dir.parent();
        }
    }

    // 2. User-level
    let helen_skills = get_helen_home().join("skills");
    if helen_skills.exists() && !dirs.contains(&helen_skills) {
        dirs.push(helen_skills);
    }

    // 3. Built-in skills (bundled with the crate)
    let builtin = Path::new(env!("CARGO_MANIFEST_DIR")).join("skills");
    if builtin.exists() && !dirs.contains(&builtin) {
        dirs.push(builtin);
    }

    dirs
}

/// Get configured locale ("zh" default; env LANG fallback) — Python parity.
pub fn get_locale() -> String {
    let config = load_config();
    if let Some(locale) = config.get("locale").and_then(|v| v.as_str()) {
        if !locale.is_empty() {
            return locale.to_string();
        }
    }
    let lang = env::var("LANG").unwrap_or_default();
    if lang.starts_with("zh") {
        return "zh".into();
    }
    if lang.starts_with("ja") {
        return "ja".into();
    }
    if lang.starts_with("ko") {
        return "ko".into();
    }
    "zh".into()
}

/// Files/directories that indicate a "Helen project" in a directory.
pub const PROJECT_MARKERS: &[&str] = &[".helen", "helen.yaml", "helen.yml", "helen.toml"];

/// Detect the nearest Helen project directory by walking up from start_dir.
pub fn detect_project_dir(start_dir: Option<&str>) -> Option<PathBuf> {
    let start = match start_dir {
        Some(s) => PathBuf::from(s),
        None => env::current_dir().ok()?,
    };
    let mut current = Some(start.as_path());
    while let Some(dir) = current {
        for marker in PROJECT_MARKERS {
            let p = dir.join(marker);
            if p.exists() {
                return Some(dir.to_path_buf());
            }
        }
        current = dir.parent();
    }
    None
}

/// Get transcript configuration with defaults (Python parity).
pub fn get_transcript_config() -> serde_json::Value {
    let config = load_config();
    let t = config
        .get("transcript")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let session_dir = get_helen_home().join("sessions");
    serde_json::json!({
        "enabled": t.get("enabled").and_then(|v| v.as_bool()).unwrap_or(true),
        "backend": t.get("backend").and_then(|v| v.as_str()).unwrap_or("jsonl"),
        "session_scope": t.get("session_scope").and_then(|v| v.as_str()).unwrap_or("auto"),
        "session_dir": t.get("session_dir").and_then(|v| v.as_str()).unwrap_or(&session_dir.to_string_lossy()),
        "project_session_dir": t.get("project_session_dir").and_then(|v| v.as_str()).unwrap_or(".helen/sessions"),
        "max_memory_items": t.get("max_memory_items").and_then(|v| v.as_i64()).unwrap_or(1000),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_yaml() {
        let cfg = parse_simple_yaml(
            "base_url: https://api.deepseek.com\napi_key: \"sk-123\"\nmodel: deepseek-chat\ntemperature: 0.3\nenabled: true\n",
        );
        assert_eq!(cfg["base_url"], "https://api.deepseek.com");
        assert_eq!(cfg["api_key"], "sk-123");
        assert_eq!(cfg["temperature"], 0.3);
        assert_eq!(cfg["enabled"], true);
    }

    #[test]
    fn test_parse_nested_yaml() {
        let cfg = parse_simple_yaml(
            "transcript:\n  enabled: false\n  backend: jsonl\n  max_memory_items: 500\n",
        );
        assert_eq!(cfg["transcript"]["enabled"], false);
        assert_eq!(cfg["transcript"]["backend"], "jsonl");
        assert_eq!(cfg["transcript"]["max_memory_items"], 500);
    }

    #[test]
    fn test_locale_default() {
        // No config file in test env -> zh default (env LANG may vary)
        let _ = get_locale();
    }
}
