//! Session manager (Task 8.5).
//!
//! Byte-faithful port of `helen/runtime/session_manager.py` (v1.45.0).
//! Sessions are stored in `~/.helen/sessions/<session_id>/transcript.jsonl`.
//!
//! Each session has:
//! - A unique session_id (e.g., "session_1720435200_a1b2c3d4_e5f6g7h8")
//! - A directory under `~/.helen/sessions/<session_id>/`
//! - A transcript.jsonl file containing the message log
//! - A session.lock file (v1.27 cross-process resume locking)

use std::fs;
use std::path::{Path, PathBuf};

/// A session entry as returned by `list_sessions`.
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub session_id: String,
    pub created_at: f64,
    pub modified_at: f64,
    pub size_bytes: u64,
    pub message_count: usize,
}

/// Session manager — creates, lists, locks, and deletes transcript sessions.
pub struct SessionManager {
    /// Base directory for all sessions (`~/.helen/sessions`).
    pub base_dir: PathBuf,
}

impl SessionManager {
    /// Initialize a session manager rooted at `base_dir` (default: `~/.helen/sessions`).
    pub fn new(base_dir: Option<&Path>) -> Self {
        let base_dir = match base_dir {
            Some(p) => p.to_path_buf(),
            None => {
                let home = std::env::var("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from("."));
                home.join(".helen").join("sessions")
            }
        };
        // Ensure base directory exists.
        let _ = fs::create_dir_all(&base_dir);
        SessionManager { base_dir }
    }

    /// Generate a session ID: timestamp + random salt + short UUID.
    ///
    /// Salt (8 hex chars / 32 bit) defeats offline prediction even if the
    /// timestamp is guessable; the trailing uuid4 segment keeps legacy
    /// collision resistance for code that parses the suffix.
    fn generate_session_id() -> String {
        let timestamp = chrono::Utc::now().timestamp();
        let salt = Self::random_hex(4);
        let short_uuid = Self::random_hex(8);
        format!("session_{timestamp}_{salt}_{short_uuid}")
    }

    /// Random hex string of `n` bytes (2n hex chars).
    fn random_hex(n: usize) -> String {
        use rand::RngCore;
        let mut bytes = vec![0u8; n];
        rand::thread_rng().fill_bytes(&mut bytes);
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Create a new transcript session.
    ///
    /// `session_id`: optional custom session ID; if `None`, generates one.
    /// Returns the session_id for the created session.
    pub fn create_session(&self, session_id: Option<&str>) -> String {
        let session_id = match session_id {
            Some(s) => s.to_string(),
            None => Self::generate_session_id(),
        };
        // Create session directory.
        let session_dir = self.base_dir.join(&session_id);
        let _ = fs::create_dir_all(&session_dir);
        session_id
    }

    /// Get transcript file path for a session.
    pub fn get_session_path(&self, session_id: &str) -> PathBuf {
        self.base_dir.join(session_id).join("transcript.jsonl")
    }

    /// Check if a session exists (directory + transcript file).
    pub fn session_exists(&self, session_id: &str) -> bool {
        let session_dir = self.base_dir.join(session_id);
        let transcript_path = session_dir.join("transcript.jsonl");
        session_dir.is_dir() && transcript_path.is_file()
    }

    /// Return True if a process with `pid` is currently running (POSIX).
    fn is_pid_alive(pid: i64) -> bool {
        if pid <= 0 {
            return false;
        }
        // Send signal 0 to probe liveness. EPERM means the process exists but
        // is owned by another user -> treat as alive. ESRCH means dead.
        // SAFETY: kill(pid, 0) has no side effects on the target process.
        // (Rust has no libc dependency in this crate; use the shell-free
        // approach: check /proc/<pid> on Linux, fall back to kill via
        // std::process::Command.)
        if Path::new(&format!("/proc/{pid}")).exists() {
            // /proc/<pid> exists for zombie processes too; treat as alive to
            // match Python's conservative os.kill(pid, 0) semantics (a zombie
            // is still alive until reaped).
            return true;
        }
        // Non-Linux fallback: attempt `kill -0` via shell is avoided; check
        // /proc only. On non-Linux, be conservative and report alive.
        #[cfg(not(target_os = "linux"))]
        {
            true
        }
        #[cfg(target_os = "linux")]
        {
            false
        }
    }

    /// Lock file path for a session.
    fn lock_path(&self, session_id: &str) -> PathBuf {
        self.base_dir.join(session_id).join("session.lock")
    }

    /// Try to acquire a cross-process lock for resuming `session_id`.
    ///
    /// Writes `<session_dir>/session.lock` containing the current PID.
    /// The lock is acquired when: no lock file exists, the lock file is stale
    /// (holder PID is dead), or the lock is already held by the current
    /// process (in-process reuse).
    ///
    /// Returns `(acquired, holder_pid)` where `holder_pid` is the live PID
    /// still holding the lock when `acquired` is false (None otherwise).
    pub fn acquire_session_lock(&self, session_id: &str) -> (bool, Option<i64>) {
        let lock_path = self.lock_path(session_id);
        let _ = fs::create_dir_all(lock_path.parent().unwrap_or(&self.base_dir));
        let current_pid = std::process::id() as i64;

        if lock_path.exists() {
            let holder_pid = fs::read_to_string(&lock_path)
                .ok()
                .and_then(|s| s.trim().parse::<i64>().ok());
            if let Some(holder_pid) = holder_pid {
                if holder_pid != current_pid && Self::is_pid_alive(holder_pid) {
                    // Held by another live process -> refuse.
                    return (false, Some(holder_pid));
                }
            }
            // Stale or self-held -> fall through and reclaim.
        }
        match fs::write(&lock_path, current_pid.to_string()) {
            Ok(()) => (true, None),
            Err(_) => {
                // Non-fatal: proceed without a lock rather than blocking resume.
                (true, None)
            }
        }
    }

    /// Release the session lock if it is held by the current process.
    ///
    /// Safe to call when no lock exists or when the lock is held by another
    /// process (no-op in both cases).
    pub fn release_session_lock(&self, session_id: &str) {
        let lock_path = self.lock_path(session_id);
        if !lock_path.exists() {
            return;
        }
        let holder_pid = fs::read_to_string(&lock_path)
            .ok()
            .and_then(|s| s.trim().parse::<i64>().ok());
        if holder_pid == Some(std::process::id() as i64) {
            let _ = fs::remove_file(&lock_path);
        }
    }

    /// List all sessions with metadata, sorted by modification time (newest first).
    pub fn list_sessions(&self) -> Vec<SessionInfo> {
        let mut sessions = Vec::new();
        if !self.base_dir.is_dir() {
            return sessions;
        }
        let entries = match fs::read_dir(&self.base_dir) {
            Ok(e) => e,
            Err(_) => return sessions,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let transcript_path = path.join("transcript.jsonl");
            if !transcript_path.is_file() {
                continue;
            }
            let meta = match fs::metadata(&transcript_path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            // Count messages (quick estimate by counting lines).
            let message_count = fs::read_to_string(&transcript_path)
                .map(|s| s.lines().count())
                .unwrap_or(0);
            let created_at = meta
                .created()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            let modified_at = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs_f64())
                .unwrap_or(0.0);
            sessions.push(SessionInfo {
                session_id: entry.file_name().to_string_lossy().to_string(),
                created_at,
                modified_at,
                size_bytes: meta.len(),
                message_count,
            });
        }
        // Sort by modification time (newest first).
        sessions.sort_by(|a, b| {
            b.modified_at
                .partial_cmp(&a.modified_at)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sessions
    }

    /// Delete a session and its transcript. Returns true if deleted.
    pub fn delete_session(&self, session_id: &str) -> bool {
        let session_dir = self.base_dir.join(session_id);
        if !session_dir.exists() {
            return false;
        }
        match fs::remove_dir_all(&session_dir) {
            Ok(()) => true,
            Err(_) => false,
        }
    }

    /// Get session directory path.
    pub fn get_session_dir(&self, session_id: &str) -> PathBuf {
        self.base_dir.join(session_id)
    }

    /// Clean up old sessions, keeping only the most recent N.
    /// Returns the number of sessions deleted.
    pub fn cleanup_old_sessions(&self, keep_count: usize) -> usize {
        let sessions = self.list_sessions();
        if sessions.len() <= keep_count {
            return 0;
        }
        // Sessions are already sorted by modified_at (newest first).
        let to_delete = &sessions[keep_count..];
        let mut deleted_count = 0;
        for session in to_delete {
            if self.delete_session(&session.session_id) {
                deleted_count += 1;
            }
        }
        deleted_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("helen_session_test_{}_{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn create_session_generates_unique_ids() {
        let base = tmp_dir("a").join("sessions");
        let m = SessionManager::new(Some(&base));
        let id1 = m.create_session(None);
        let id2 = m.create_session(None);
        assert!(id1.starts_with("session_"), "id format: {id1}");
        assert_ne!(id1, id2);
        // Python semantics: session_exists requires a transcript.jsonl file
        // (create_session only creates the directory).
        assert!(!m.session_exists(&id1));
        assert!(m.get_session_dir(&id1).is_dir());
        fs::write(m.get_session_path(&id1), "x\n").unwrap();
        fs::write(m.get_session_path(&id2), "x\n").unwrap();
        assert!(m.session_exists(&id1));
        assert!(m.session_exists(&id2));
    }

    #[test]
    fn get_session_path_points_at_transcript() {
        let base = tmp_dir("b").join("sessions");
        let m = SessionManager::new(Some(&base));
        let id = m.create_session(None);
        let p = m.get_session_path(&id);
        assert_eq!(p.file_name().unwrap().to_str().unwrap(), "transcript.jsonl");
        assert!(p.parent().unwrap().is_dir());
    }

    #[test]
    fn list_sessions_sorted_by_mtime() {
        let base = tmp_dir("c").join("sessions2");
        let m = SessionManager::new(Some(&base));
        let id_a = m.create_session(None);
        std::thread::sleep(std::time::Duration::from_millis(30));
        let id_b = m.create_session(None);
        // Write a transcript file for both so they appear in listing.
        fs::write(m.get_session_path(&id_a), "line1\n").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(30));
        fs::write(m.get_session_path(&id_b), "l1\nl2\nl3\n").unwrap();
        let sessions = m.list_sessions();
        assert_eq!(sessions.len(), 2);
        // Newest first.
        assert_eq!(sessions[0].session_id, id_b);
        assert_eq!(sessions[1].session_id, id_a);
        // Message counts.
        assert_eq!(sessions[0].message_count, 3);
        assert_eq!(sessions[1].message_count, 1);
    }

    #[test]
    fn delete_and_cleanup() {
        let base = tmp_dir("d").join("sessions3");
        let m = SessionManager::new(Some(&base));
        let id = m.create_session(None);
        fs::write(m.get_session_path(&id), "x\n").unwrap();
        assert!(m.session_exists(&id));
        assert!(m.delete_session(&id));
        assert!(!m.session_exists(&id));
        assert!(!m.delete_session(&id)); // Already gone.
                                         // Cleanup keeps only the most recent N (deterministic mtime order).
        let id_a = m.create_session(None);
        fs::write(m.get_session_path(&id_a), "a\n").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let id_b = m.create_session(None);
        fs::write(m.get_session_path(&id_b), "b\n").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        let id_c = m.create_session(None);
        fs::write(m.get_session_path(&id_c), "c\n").unwrap();
        let deleted = m.cleanup_old_sessions(2);
        assert_eq!(deleted, 1);
        assert!(!m.session_exists(&id_a)); // Oldest deleted.
        assert!(m.session_exists(&id_b));
        assert!(m.session_exists(&id_c));
    }

    #[test]
    fn session_lock_acquire_release() {
        let base = tmp_dir("e").join("sessions4");
        let m = SessionManager::new(Some(&base));
        let id = m.create_session(None);
        let (acquired, holder) = m.acquire_session_lock(&id);
        assert!(acquired);
        assert_eq!(holder, None);
        // Self-held -> reacquire succeeds (in-process reuse).
        let (acquired2, _) = m.acquire_session_lock(&id);
        assert!(acquired2);
        m.release_session_lock(&id);
        // After release, lock file is gone.
        assert!(!m.lock_path(&id).exists());
    }
}
