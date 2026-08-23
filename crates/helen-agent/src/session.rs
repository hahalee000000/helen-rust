//! Session management for chat conversations

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

/// A single message in a chat session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
    pub timestamp: i64,
}

/// A chat session containing conversation history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub messages: Vec<Message>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Session {
    /// Create a new session with a unique ID
    pub fn new() -> Self {
        let now = chrono::Utc::now().timestamp();
        Self {
            id: Uuid::new_v4().to_string(),
            messages: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Add a message to the session
    pub fn add_message(&mut self, role: &str, content: &str) {
        let now = chrono::Utc::now().timestamp();
        self.messages.push(Message {
            role: role.to_string(),
            content: content.to_string(),
            timestamp: now,
        });
        self.updated_at = now;
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

/// Manages session persistence
pub struct SessionManager {
    storage_dir: PathBuf,
}

impl SessionManager {
    /// Create a new session manager with the given storage directory
    pub fn new(storage_dir: PathBuf) -> Self {
        fs::create_dir_all(&storage_dir).ok();
        Self { storage_dir }
    }

    /// Create a new session (not yet saved)
    pub fn create_session(&self) -> Session {
        Session::new()
    }

    /// Save a session to disk
    pub fn save_session(&self, session: &Session) -> Result<(), std::io::Error> {
        let path = self.storage_dir.join(format!("{}.json", session.id));
        let json = serde_json::to_string_pretty(session)?;
        fs::write(path, json)
    }

    /// Load a session from disk
    pub fn load_session(&self, id: &str) -> Result<Session, std::io::Error> {
        let path = self.storage_dir.join(format!("{}.json", id));
        let json = fs::read_to_string(path)?;
        let session: Session = serde_json::from_str(&json)?;
        Ok(session)
    }

    /// List all saved sessions
    pub fn list_sessions(&self) -> Result<Vec<Session>, std::io::Error> {
        let mut sessions = Vec::new();
        
        for entry in fs::read_dir(&self.storage_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Ok(json) = fs::read_to_string(&path) {
                    if let Ok(session) = serde_json::from_str::<Session>(&json) {
                        sessions.push(session);
                    }
                }
            }
        }
        
        // Sort by updated_at descending (most recent first)
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        
        Ok(sessions)
    }

    /// Delete a session from disk
    pub fn delete_session(&self, id: &str) -> Result<(), std::io::Error> {
        let path = self.storage_dir.join(format!("{}.json", id));
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }
}
