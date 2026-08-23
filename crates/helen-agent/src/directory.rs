//! Directory-based session management
//!
//! Core concept: directory = session boundary.
//! Each project directory has its own session with:
//! - `.helen/sessions/<sid>/transcript.jsonl` (Helen transcript)
//! - `.helen/MEMORY.md` (project memory)
//! - `.helen/USER.md` (user preferences)

use serde::Serialize;
use std::path::Path;
use std::sync::Mutex;

/// Result of a `set_cwd` operation
#[derive(Debug, Clone, Serialize)]
pub struct SetCwdResult {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Manages the current working directory and derived paths.
///
/// Thread-safe: uses internal Mutex for the mutable CWD state.
pub struct DirectoryManager {
    cwd: Mutex<String>,
}

impl DirectoryManager {
    /// Create a new DirectoryManager.
    ///
    /// If `initial_cwd` is a valid directory, uses it.
    /// Otherwise falls back to `std::env::current_dir()`, then to `$HOME`.
    pub fn new(initial_cwd: String) -> Self {
        let resolved = if Path::new(&initial_cwd).is_dir() {
            Self::resolve_path(&initial_cwd)
        } else {
            eprintln!(
                "⚠️  initial_cwd={} is not a valid directory, falling back to current dir",
                initial_cwd
            );
            Self::fallback_cwd()
        };
        Self {
            cwd: Mutex::new(resolved),
        }
    }

    /// Get the current working directory
    pub fn get_cwd(&self) -> String {
        let cwd = self.cwd.lock().unwrap();
        let cwd_str = cwd.clone();
        drop(cwd);
        
        // Verify it still exists; if not, fall back
        if Path::new(&cwd_str).is_dir() {
            cwd_str
        } else {
            let fallback = Self::fallback_cwd();
            let mut guard = self.cwd.lock().unwrap();
            *guard = fallback.clone();
            fallback
        }
    }

    /// Switch the working directory.
    ///
    /// Creates `.helen/` in the new directory if it doesn't exist.
    pub fn set_cwd(&self, path: &str) -> SetCwdResult {
        let abs_path = match Self::resolve_path_checked(path) {
            Some(p) => p,
            None => {
                return SetCwdResult {
                    status: "error".to_string(),
                    cwd: None,
                    display_name: None,
                    message: Some(format!("目录不存在: {}", path)),
                };
            }
        };

        // Create .helen directory
        let helen_dir = Path::new(&abs_path).join(".helen");
        if let Err(e) = std::fs::create_dir_all(&helen_dir) {
            return SetCwdResult {
                status: "error".to_string(),
                cwd: None,
                display_name: None,
                message: Some(format!("Failed to create .helen directory: {}", e)),
            };
        }

        let display_name = Self::compute_display_name(&abs_path);

        // Update internal state
        let mut guard = self.cwd.lock().unwrap();
        *guard = abs_path.clone();

        SetCwdResult {
            status: "ok".to_string(),
            cwd: Some(abs_path),
            display_name: Some(display_name),
            message: None,
        }
    }

    /// Get the display name for a directory path.
    ///
    /// Returns the last component of the path, or the full path for root directories.
    pub fn get_display_name(&self, path: Option<String>) -> String {
        let p = match path {
            Some(ref s) => s.as_str(),
            None => return Self::compute_display_name(&self.get_cwd()),
        };
        Self::compute_display_name(p)
    }

    /// Get the `.helen/` directory path for the current CWD
    pub fn get_helen_dir(&self) -> String {
        let cwd = self.get_cwd();
        format!("{}/.helen", cwd)
    }

    /// Get the `.helen/sessions/` directory path
    pub fn get_sessions_dir(&self) -> String {
        let cwd = self.get_cwd();
        format!("{}/.helen/sessions", cwd)
    }

    /// Get the `.helen/MEMORY.md` file path
    pub fn get_memory_path(&self) -> String {
        let cwd = self.get_cwd();
        format!("{}/.helen/MEMORY.md", cwd)
    }

    /// Get the `.helen/USER.md` file path
    pub fn get_user_path(&self) -> String {
        let cwd = self.get_cwd();
        format!("{}/.helen/USER.md", cwd)
    }

    /// Map a CWD path to a stable, URL-safe session ID.
    ///
    /// Uses SHA256(cwd)[:16] — short, URL-safe (hex), deterministic.
    pub fn cwd_to_session_id(&self, cwd: &str) -> String {
        use sha2::{Digest, Sha256};
        let hash = Sha256::digest(cwd.as_bytes());
        hex::encode(&hash[..8]) // 8 bytes = 16 hex chars
    }

    // ── Private helpers ──────────────────────────────────────────

    fn resolve_path(path: &str) -> String {
        let expanded = shellexpand::tilde(path).into_owned();
        let p = Path::new(&expanded);
        match std::fs::canonicalize(p) {
            Ok(abs) => abs.to_string_lossy().into_owned(),
            Err(_) => {
                // If canonicalize fails (e.g., path doesn't exist), try to make it absolute
                if p.is_absolute() {
                    expanded
                } else {
                    match std::env::current_dir() {
                        Ok(cwd) => cwd.join(p).to_string_lossy().into_owned(),
                        Err(_) => expanded,
                    }
                }
            }
        }
    }

    fn resolve_path_checked(path: &str) -> Option<String> {
        let expanded = shellexpand::tilde(path).into_owned();
        let p = Path::new(&expanded);

        // Try canonicalize first (resolves symlinks)
        if let Ok(abs) = std::fs::canonicalize(p) {
            if abs.is_dir() {
                return Some(abs.to_string_lossy().into_owned());
            }
        }

        // Check if it's a valid directory
        if p.is_dir() {
            let abs = if p.is_absolute() {
                p.to_path_buf()
            } else {
                match std::env::current_dir() {
                    Ok(cwd) => cwd.join(p),
                    Err(_) => return None,
                }
            };
            return Some(abs.to_string_lossy().into_owned());
        }

        None
    }

    fn fallback_cwd() -> String {
        match std::env::current_dir() {
            Ok(cwd) => cwd.to_string_lossy().into_owned(),
            Err(_) => std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string()),
        }
    }

    fn compute_display_name(path: &str) -> String {
        let p = Path::new(path);
        p.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_display_name() {
        assert_eq!(DirectoryManager::compute_display_name("/home/user/project"), "project");
        assert_eq!(DirectoryManager::compute_display_name("/"), "/");
    }

    #[test]
    fn test_cwd_to_session_id_deterministic() {
        let dm = DirectoryManager::new("/tmp".to_string());
        let id1 = dm.cwd_to_session_id("/home/user/project");
        let id2 = dm.cwd_to_session_id("/home/user/project");
        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 16);
    }

    #[test]
    fn test_cwd_to_session_id_different() {
        let dm = DirectoryManager::new("/tmp".to_string());
        let id1 = dm.cwd_to_session_id("/home/user/project_a");
        let id2 = dm.cwd_to_session_id("/home/user/project_b");
        assert_ne!(id1, id2);
    }
}
