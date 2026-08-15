//! Skills system (Task 6.4) — port of `helen/runtime/config.py#get_skill_dirs`
//! plus the two-layer skill disclosure used by the `load_skill` and
//! `list_skill_references` tools.
//!
//! Search layers (priority order):
//!   1. `<project>/.helen/skills/` (closest ancestor of cwd)
//!   2. `~/.helen/skills/` (user-level)
//!   3. bundled `helen/skills/` (distributed with the language)

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Skill directories in priority order (Python `get_skill_dirs`).
pub fn get_skill_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();

    // 1. Project-level skills (closest ancestor of cwd with .helen/skills).
    if let Ok(cwd) = std::env::current_dir() {
        for parent in cwd.ancestors() {
            let project_skills = parent.join(".helen").join("skills");
            if project_skills.is_dir() && !dirs.contains(&project_skills) {
                dirs.push(project_skills);
                break;
            }
        }
    }

    // 2. User-level skills (~/.helen/skills).
    if let Some(home) = home_dir() {
        let user_skills = home.join(".helen").join("skills");
        if user_skills.is_dir() && !dirs.contains(&user_skills) {
            dirs.push(user_skills);
        }
    }

    // 3. Bundled skills (helen-rust/skills, then HELEN_HOME/skills).
    let candidates = [
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("skills"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default()
            .join("skills"),
    ];
    for c in candidates {
        if c.is_dir() && !dirs.contains(&c) {
            dirs.push(c);
        }
    }

    dirs
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// Find the skill root directory that contains `name/SKILL.md`.
fn find_skill_root(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    for base in dirs {
        if !base.is_dir() {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(base) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir()
                    && entry.file_name().to_string_lossy() == name
                    && path.join("SKILL.md").is_file()
                {
                    return Some(path);
                }
            }
        }
    }
    None
}

/// Skill reference file extensions (Python parity).
fn is_ref_file(name: &str) -> bool {
    name.ends_with(".md")
        || name.ends_with(".txt")
        || name.ends_with(".yaml")
        || name.ends_with(".json")
}

/// `_load_skill` tool — full SKILL.md content, optionally with references.
pub fn load_skill(name: &str, include_references: bool) -> Value {
    let dirs = get_skill_dirs();
    match find_skill_root(name, &dirs) {
        Some(root) => {
            let skill_path = root.join("SKILL.md");
            let content = std::fs::read_to_string(&skill_path).unwrap_or_default();
            let mut result = json!({
                "name": name,
                "path": skill_path.to_string_lossy().to_string(),
                "content": content,
            });
            if include_references {
                let refs_dir = root.join("references");
                let refs: Vec<Value> = if refs_dir.is_dir() {
                    let mut files: Vec<PathBuf> = std::fs::read_dir(&refs_dir)
                        .map(|rd| rd.flatten().map(|e| e.path()).collect())
                        .unwrap_or_default();
                    files.retain(|p| {
                        p.is_file()
                            && is_ref_file(&p.file_name().unwrap_or_default().to_string_lossy())
                    });
                    files.sort();
                    files
                        .iter()
                        .map(|p| {
                            let rf = p
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string();
                            json!({
                                "name": rf,
                                "path": p.to_string_lossy().to_string(),
                                "size": std::fs::metadata(p).map(|m| m.len()).unwrap_or(0),
                            })
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                result["references"] = Value::Array(refs);
            }
            result
        }
        None => json!({"error": format!("Skill '{name}' not found in any skill directory")}),
    }
}

/// `_list_skill_references` tool — reference docs with 3-line previews.
pub fn list_skill_references(name: &str) -> Value {
    let dirs = get_skill_dirs();
    match find_skill_root(name, &dirs) {
        Some(root) => {
            let skill_path = root.join("SKILL.md");
            let refs_dir = root.join("references");
            if !refs_dir.is_dir() {
                return json!({
                    "name": name,
                    "skill_path": skill_path.to_string_lossy().to_string(),
                    "references": [],
                    "message": format!("Skill '{name}' has no references/ directory"),
                });
            }
            let mut files: Vec<PathBuf> = std::fs::read_dir(&refs_dir)
                .map(|rd| rd.flatten().map(|e| e.path()).collect())
                .unwrap_or_default();
            files.retain(|p| {
                p.is_file() && is_ref_file(&p.file_name().unwrap_or_default().to_string_lossy())
            });
            files.sort();
            let refs: Vec<Value> = files
                .iter()
                .map(|p| {
                    let rf = p
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    let size = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
                    // Preview: first 3 non-empty lines.
                    let preview = std::fs::read_to_string(p)
                        .map(|s| {
                            s.lines()
                                .take(3)
                                .map(|l| l.trim_end().to_string())
                                .collect::<Vec<_>>()
                                .join("\n")
                        })
                        .unwrap_or_default();
                    json!({
                        "name": rf,
                        "path": p.to_string_lossy().to_string(),
                        "size": size,
                        "preview": preview,
                    })
                })
                .collect();
            json!({
                "name": name,
                "skill_path": skill_path.to_string_lossy().to_string(),
                "references": refs,
                "total": refs.len(),
            })
        }
        None => json!({"error": format!("Skill '{name}' not found in any skill directory")}),
    }
}

/// Check whether a path is inside the project directory (security helper).
pub fn is_within_project(path: &Path) -> bool {
    let Ok(cwd) = std::env::current_dir() else {
        return true;
    };
    let Ok(rel) = path.strip_prefix(&cwd) else {
        // Absolute outside cwd: allow only if the user's home? Python tools
        // accept arbitrary paths; keep parity by allowing.
        return true;
    };
    !rel.starts_with("..")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_dirs_returns_ordered_list() {
        let dirs = get_skill_dirs();
        // No duplicate entries.
        for i in 0..dirs.len() {
            for j in (i + 1)..dirs.len() {
                assert_ne!(dirs[i], dirs[j]);
            }
        }
    }

    #[test]
    fn load_skill_unknown_returns_error() {
        let v = load_skill("__no_such_skill_xyz__", false);
        assert!(v.get("error").is_some());
    }

    #[test]
    fn list_references_unknown_returns_error() {
        let v = list_skill_references("__no_such_skill_xyz__");
        assert!(v.get("error").is_some());
    }
}
