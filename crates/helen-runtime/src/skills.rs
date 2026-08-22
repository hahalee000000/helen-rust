//! Skills system (Task 6.4) — port of `helen/runtime/config.py#get_skill_dirs`
//! plus the two-layer skill disclosure used by the `load_skill` and
//! `list_skill_references` tools.
//!
//! Search layers (priority order):
//!   1. `<project>/.helen/skills/` (closest ancestor of cwd)
//!   2. `~/.helen/skills/` (user-level)
//!   3. **Embedded** skills (compiled into the binary via build.rs)

// Include the auto-generated embedded skills map (from build.rs).
include!(concat!(env!("OUT_DIR"), "/embedded_skills.rs"));

use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Skill directories in priority order (Python `get_skill_dirs`).
///
/// Only returns **disk** directories (project → user).  Bundled skills
/// are served from the compile-time embedded filesystem and are not
/// listed here.
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

    dirs
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

// ---------------------------------------------------------------------------
// Embedded skill helpers
// ---------------------------------------------------------------------------

/// Find a skill by name in the embedded filesystem.
///
/// Returns the "virtual root prefix" (e.g. `"devops/github"`) if found.
fn find_embedded_skill_root(name: &str) -> Option<String> {
    let suffix = format!("/{}/SKILL.md", name);
    for path in embedded_skill_paths() {
        if path.ends_with(&suffix) || *path == format!("{}/SKILL.md", name) {
            // Strip the trailing "/SKILL.md" to get the root prefix.
            let root = &path[..path.len() - "/SKILL.md".len()];
            return Some(root.to_string());
        }
    }
    None
}

/// List all embedded files under a given virtual prefix.
fn embedded_files_under(prefix: &str) -> Vec<String> {
    let p = if prefix.is_empty() {
        String::new()
    } else {
        format!("{}/", prefix)
    };
    embedded_skill_paths()
        .iter()
        .filter(|path| path.starts_with(&p))
        .map(|path| path.to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// Disk-based search (unchanged logic, minus the broken bundled fallback)
// ---------------------------------------------------------------------------

/// Find the skill root directory that contains `name/SKILL.md` on disk.
fn find_skill_root(name: &str, dirs: &[PathBuf]) -> Option<PathBuf> {
    for base in dirs {
        if !base.is_dir() {
            continue;
        }
        let mut stack = vec![base.clone()];
        while let Some(dir) = stack.pop() {
            if dir.file_name().map(|n| n == name).unwrap_or(false) && dir.join("SKILL.md").is_file()
            {
                return Some(dir);
            }
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        stack.push(path);
                    }
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

    // 1. Try disk-based search first (project → user).
    if let Some(root) = find_skill_root(name, &dirs) {
        return load_skill_from_disk(name, &root, include_references);
    }

    // 2. Fall back to embedded skills.
    if let Some(virtual_root) = find_embedded_skill_root(name) {
        return load_skill_from_embedded(name, &virtual_root, include_references);
    }

    json!({"error": format!("Skill '{name}' not found in any skill directory")})
}

/// Load a skill from a disk directory.
fn load_skill_from_disk(name: &str, root: &Path, include_references: bool) -> Value {
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
                p.is_file() && is_ref_file(&p.file_name().unwrap_or_default().to_string_lossy())
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

/// Load a skill from the embedded filesystem.
fn load_skill_from_embedded(name: &str, virtual_root: &str, include_references: bool) -> Value {
    let skill_rel = format!("{}/SKILL.md", virtual_root);
    let content = embedded_skill_file(&skill_rel).unwrap_or_default();
    let mut result = json!({
        "name": name,
        "path": format!("(embedded)/{}", skill_rel),
        "content": content,
    });
    if include_references {
        let refs_prefix = format!("{}/references", virtual_root);
        let refs: Vec<Value> = embedded_files_under(&refs_prefix)
            .into_iter()
            .filter(|p| is_ref_file(p.rsplit('/').next().unwrap_or(p)))
            .map(|p| {
                let fname = p.rsplit('/').next().unwrap_or(&p).to_string();
                let size = embedded_skill_file(&p).map(|s| s.len()).unwrap_or(0);
                json!({
                    "name": fname,
                    "path": format!("(embedded)/{}", p),
                    "size": size as u64,
                })
            })
            .collect();
        result["references"] = Value::Array(refs);
    }
    result
}

/// `_list_skill_references` tool — reference docs with 3-line previews.
pub fn list_skill_references(name: &str) -> Value {
    let dirs = get_skill_dirs();

    // 1. Try disk-based search first.
    if let Some(root) = find_skill_root(name, &dirs) {
        return list_references_from_disk(name, &root);
    }

    // 2. Fall back to embedded skills.
    if let Some(virtual_root) = find_embedded_skill_root(name) {
        return list_references_from_embedded(name, &virtual_root);
    }

    json!({"error": format!("Skill '{name}' not found in any skill directory")})
}

/// List references from a disk skill directory.
fn list_references_from_disk(name: &str, root: &Path) -> Value {
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

/// List references from an embedded skill.
fn list_references_from_embedded(name: &str, virtual_root: &str) -> Value {
    let skill_rel = format!("{}/SKILL.md", virtual_root);
    let refs_prefix = format!("{}/references", virtual_root);
    let ref_files: Vec<String> = embedded_files_under(&refs_prefix)
        .into_iter()
        .filter(|p| is_ref_file(p.rsplit('/').next().unwrap_or(p)))
        .collect();

    if ref_files.is_empty() {
        return json!({
            "name": name,
            "skill_path": format!("(embedded)/{}", skill_rel),
            "references": [],
            "message": format!("Skill '{name}' has no references/ directory"),
        });
    }

    let refs: Vec<Value> = ref_files
        .iter()
        .map(|p| {
            let fname = p.rsplit('/').next().unwrap_or(p).to_string();
            let content = embedded_skill_file(p).unwrap_or_default();
            let size = content.len() as u64;
            let preview = content
                .lines()
                .take(3)
                .map(|l| l.trim_end().to_string())
                .collect::<Vec<_>>()
                .join("\n");
            json!({
                "name": fname,
                "path": format!("(embedded)/{}", p),
                "size": size,
                "preview": preview,
            })
        })
        .collect();

    json!({
        "name": name,
        "skill_path": format!("(embedded)/{}", skill_rel),
        "references": refs,
        "total": refs.len(),
    })
}

/// Check whether a path is inside the project directory (security helper).
pub fn is_within_project(path: &Path) -> bool {
    let Ok(cwd) = std::env::current_dir() else {
        return true;
    };
    let Ok(rel) = path.strip_prefix(&cwd) else {
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

    #[test]
    fn embedded_skills_are_accessible() {
        // Verify that at least one known skill is embedded.
        let v = load_skill("helen-syntax", false);
        assert!(
            v.get("content").is_some(),
            "helen-syntax should be found in embedded skills"
        );
        let content = v["content"].as_str().unwrap_or_default();
        assert!(
            !content.is_empty(),
            "helen-syntax SKILL.md should have content"
        );
    }

    #[test]
    fn embedded_skill_with_references() {
        let v = load_skill("helen-language-development", true);
        assert!(v.get("content").is_some());
        let refs = v.get("references").and_then(|r| r.as_array());
        assert!(refs.is_some(), "should have references array");
        assert!(
            !refs.unwrap().is_empty(),
            "helen-language-development should have reference files"
        );
    }

    #[test]
    fn embedded_paths_not_empty() {
        let paths = embedded_skill_paths();
        assert!(
            paths.len() >= 50,
            "should have at least 50 embedded skill files, got {}",
            paths.len()
        );
    }
}
