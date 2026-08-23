//! File storage for uploads

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

/// File metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub id: String,
    pub filename: String,
    pub size: u64,
    pub created_at: i64,
}

/// File storage manager
pub struct FileStorage {
    storage_dir: PathBuf,
}

impl FileStorage {
    /// Create a new file storage with the given directory
    pub fn new(storage_dir: PathBuf) -> Self {
        fs::create_dir_all(&storage_dir).ok();
        Self { storage_dir }
    }

    /// Store a file and return its ID
    pub fn store_file(&self, filename: &str, content: &[u8]) -> Result<String, std::io::Error> {
        let file_id = Uuid::new_v4().to_string();
        let file_path = self.storage_dir.join(&file_id);
        let meta_path = self.storage_dir.join(format!("{}.meta", file_id));
        
        // Store file content
        fs::write(&file_path, content)?;
        
        // Store metadata
        let metadata = FileMetadata {
            id: file_id.clone(),
            filename: filename.to_string(),
            size: content.len() as u64,
            created_at: chrono::Utc::now().timestamp(),
        };
        let meta_json = serde_json::to_string_pretty(&metadata)?;
        fs::write(&meta_path, meta_json)?;
        
        Ok(file_id)
    }

    /// Retrieve a file by ID
    pub fn retrieve_file(&self, file_id: &str) -> Result<Vec<u8>, std::io::Error> {
        let file_path = self.storage_dir.join(file_id);
        fs::read(file_path)
    }

    /// Get file metadata
    pub fn get_metadata(&self, file_id: &str) -> Result<FileMetadata, std::io::Error> {
        let meta_path = self.storage_dir.join(format!("{}.meta", file_id));
        let meta_json = fs::read_to_string(meta_path)?;
        let metadata: FileMetadata = serde_json::from_str(&meta_json)?;
        Ok(metadata)
    }

    /// List all stored files
    pub fn list_files(&self) -> Result<Vec<FileMetadata>, std::io::Error> {
        let mut files = Vec::new();
        
        for entry in fs::read_dir(&self.storage_dir)? {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().and_then(|s| s.to_str()) == Some("meta") {
                if let Ok(meta_json) = fs::read_to_string(&path) {
                    if let Ok(metadata) = serde_json::from_str::<FileMetadata>(&meta_json) {
                        files.push(metadata);
                    }
                }
            }
        }
        
        // Sort by created_at descending (most recent first)
        files.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        
        Ok(files)
    }

    /// Delete a file by ID
    pub fn delete_file(&self, file_id: &str) -> Result<(), std::io::Error> {
        let file_path = self.storage_dir.join(file_id);
        let meta_path = self.storage_dir.join(format!("{}.meta", file_id));
        
        if file_path.exists() {
            fs::remove_file(file_path)?;
        }
        if meta_path.exists() {
            fs::remove_file(meta_path)?;
        }
        
        Ok(())
    }
}
