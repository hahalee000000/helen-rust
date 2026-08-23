//! Upload manager — saves files to `.helen/uploads/<upload_id>/`
//!
//! Each upload creates a directory with:
//! - `file` — the raw file content
//! - `metadata.json` — upload metadata (id, filename, mime_type, size, created_at)
//!
//! Security:
//! - Path traversal prevention on upload_id
//! - Realpath validation on file serving
//! - MIME type validation
//! - Size limit (50MB)

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

/// Maximum file size: 50MB
pub const MAX_FILE_SIZE: usize = 50 * 1024 * 1024;

/// Allowed MIME types for upload
pub const ALLOWED_MIME_TYPES: &[&str] = &[
    // Images
    "image/jpeg",
    "image/png",
    "image/gif",
    "image/webp",
    // Audio
    "audio/mpeg",
    "audio/wav",
    "audio/ogg",
    "audio/mp4",
    "audio/m4a",
    "audio/x-m4a",
    // Video
    "video/mp4",
    "video/webm",
    "video/quicktime",
    // Text (for code files)
    "text/plain",
    "text/markdown",
    "text/csv",
    // Application
    "application/pdf",
    "application/json",
];

/// Metadata for an uploaded file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadMetadata {
    pub upload_id: String,
    pub filename: String,
    pub mime_type: String,
    pub size: usize,
    pub created_at: String,
}

/// Result of a successful upload
#[derive(Debug, Serialize)]
pub struct UploadResult {
    pub upload_id: String,
    pub filename: String,
    pub mime_type: String,
    pub size: usize,
    pub url: String,
}

/// Error types for upload operations
#[derive(Debug)]
pub enum UploadError {
    UnsupportedMimeType(String),
    FileTooLarge { size: usize, max: usize },
    InvalidUploadId(String),
    FileNotFound,
    MetadataNotFound,
    AccessDenied,
    IoError(std::io::Error),
}

impl std::fmt::Display for UploadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UploadError::UnsupportedMimeType(t) => write!(f, "Unsupported file type: {}", t),
            UploadError::FileTooLarge { size, max } => {
                write!(
                    f,
                    "File too large ({} bytes). Max: {} bytes (50MB)",
                    size, max
                )
            }
            UploadError::InvalidUploadId(id) => write!(f, "Invalid upload_id: {}", id),
            UploadError::FileNotFound => write!(f, "File not found"),
            UploadError::MetadataNotFound => write!(f, "File metadata not found"),
            UploadError::AccessDenied => write!(f, "Access denied"),
            UploadError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl From<std::io::Error> for UploadError {
    fn from(e: std::io::Error) -> Self {
        UploadError::IoError(e)
    }
}

/// Upload manager — handles file uploads to `.helen/uploads/`
pub struct UploadManager {
    base_dir: PathBuf,
}

impl UploadManager {
    /// Create a new UploadManager with the given base directory (CWD)
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Get the uploads directory for the current CWD
    pub fn uploads_dir(&self) -> PathBuf {
        self.base_dir.join(".helen").join("uploads")
    }

    /// Get the upload directory for a specific upload_id
    pub fn upload_dir(&self, upload_id: &str) -> PathBuf {
        self.uploads_dir().join(upload_id)
    }

    /// Validate an upload_id to prevent path traversal
    pub fn validate_upload_id(upload_id: &str) -> Result<(), UploadError> {
        if upload_id.is_empty()
            || upload_id.contains('/')
            || upload_id.contains('\\')
            || upload_id.contains("..")
        {
            return Err(UploadError::InvalidUploadId(upload_id.to_string()));
        }
        Ok(())
    }

    /// Validate MIME type
    pub fn validate_mime_type(mime_type: &str) -> Result<(), UploadError> {
        if ALLOWED_MIME_TYPES.contains(&mime_type) {
            Ok(())
        } else {
            Err(UploadError::UnsupportedMimeType(mime_type.to_string()))
        }
    }

    /// Validate file size
    pub fn validate_file_size(size: usize) -> Result<(), UploadError> {
        if size > MAX_FILE_SIZE {
            Err(UploadError::FileTooLarge {
                size,
                max: MAX_FILE_SIZE,
            })
        } else {
            Ok(())
        }
    }

    /// Save an uploaded file
    pub fn save_upload(
        &self,
        filename: &str,
        mime_type: &str,
        content: &[u8],
    ) -> Result<UploadResult, UploadError> {
        // Validate
        Self::validate_mime_type(mime_type)?;
        Self::validate_file_size(content.len())?;

        // Generate upload_id
        let upload_id = Uuid::new_v4().to_string();

        // Create upload directory
        let upload_dir = self.upload_dir(&upload_id);
        fs::create_dir_all(&upload_dir)?;

        // Save metadata
        let metadata = UploadMetadata {
            upload_id: upload_id.clone(),
            filename: filename.to_string(),
            mime_type: mime_type.to_string(),
            size: content.len(),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let metadata_json = serde_json::to_string_pretty(&metadata)
            .map_err(|e| UploadError::IoError(std::io::Error::other(e)))?;
        fs::write(upload_dir.join("metadata.json"), metadata_json)?;

        // Save file content
        fs::write(upload_dir.join("file"), content)?;

        let url = format!("/api/chat/uploads/{}/file", upload_id);

        Ok(UploadResult {
            upload_id,
            filename: filename.to_string(),
            mime_type: mime_type.to_string(),
            size: content.len(),
            url,
        })
    }

    /// Get the file path and MIME type for a given upload_id
    pub fn get_upload_file(&self, upload_id: &str) -> Result<(PathBuf, String), UploadError> {
        // Validate upload_id
        Self::validate_upload_id(upload_id)?;

        let upload_dir = self.upload_dir(upload_id);
        let file_path = upload_dir.join("file");

        // Realpath validation — prevent symlink escape
        let real_file = fs::canonicalize(&file_path).map_err(|_| UploadError::FileNotFound)?;
        let real_upload_dir =
            fs::canonicalize(&upload_dir).map_err(|_| UploadError::FileNotFound)?;
        let real_file_str = real_file.to_string_lossy();
        let real_dir_str = real_upload_dir.to_string_lossy();
        if !real_file_str.starts_with(&format!("{}/", real_dir_str))
            && real_file_str != real_dir_str
        {
            return Err(UploadError::AccessDenied);
        }

        if !file_path.exists() {
            return Err(UploadError::FileNotFound);
        }

        // Read metadata for MIME type
        let metadata_path = upload_dir.join("metadata.json");
        if !metadata_path.exists() {
            return Err(UploadError::MetadataNotFound);
        }

        let metadata_str = fs::read_to_string(&metadata_path)?;
        let metadata: UploadMetadata = serde_json::from_str(&metadata_str)
            .map_err(|e| UploadError::IoError(std::io::Error::other(e)))?;

        Ok((file_path, metadata.mime_type))
    }

    /// Read the file content for a given upload_id
    pub fn read_upload_file(&self, upload_id: &str) -> Result<(Vec<u8>, String), UploadError> {
        let (file_path, mime_type) = self.get_upload_file(upload_id)?;
        let content = fs::read(&file_path)?;
        Ok((content, mime_type))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, UploadManager) {
        let tmp = TempDir::new().unwrap();
        let mgr = UploadManager::new(tmp.path().to_path_buf());
        (tmp, mgr)
    }

    #[test]
    fn test_uploads_dir() {
        let (_tmp, mgr) = setup();
        let expected = mgr.base_dir.join(".helen").join("uploads");
        assert_eq!(mgr.uploads_dir(), expected);
    }

    #[test]
    fn test_upload_dir() {
        let (_tmp, mgr) = setup();
        let dir = mgr.upload_dir("test-uuid");
        assert!(dir.to_string_lossy().contains("test-uuid"));
    }

    #[test]
    fn test_validate_upload_id_valid() {
        assert!(UploadManager::validate_upload_id("abc-123-def").is_ok());
        assert!(UploadManager::validate_upload_id(&Uuid::new_v4().to_string()).is_ok());
    }

    #[test]
    fn test_validate_upload_id_empty() {
        assert!(UploadManager::validate_upload_id("").is_err());
    }

    #[test]
    fn test_validate_upload_id_slash() {
        assert!(UploadManager::validate_upload_id("foo/bar").is_err());
    }

    #[test]
    fn test_validate_upload_id_backslash() {
        assert!(UploadManager::validate_upload_id("foo\\bar").is_err());
    }

    #[test]
    fn test_validate_upload_id_dotdot() {
        assert!(UploadManager::validate_upload_id("foo..bar").is_err());
    }

    #[test]
    fn test_validate_mime_type_allowed() {
        assert!(UploadManager::validate_mime_type("image/png").is_ok());
        assert!(UploadManager::validate_mime_type("application/pdf").is_ok());
        assert!(UploadManager::validate_mime_type("text/plain").is_ok());
    }

    #[test]
    fn test_validate_mime_type_disallowed() {
        assert!(UploadManager::validate_mime_type("application/exe").is_err());
        assert!(UploadManager::validate_mime_type("text/html").is_err());
    }

    #[test]
    fn test_validate_file_size_ok() {
        assert!(UploadManager::validate_file_size(1024).is_ok());
        assert!(UploadManager::validate_file_size(MAX_FILE_SIZE).is_ok());
    }

    #[test]
    fn test_validate_file_size_too_large() {
        assert!(UploadManager::validate_file_size(MAX_FILE_SIZE + 1).is_err());
    }

    #[test]
    fn test_save_upload_success() {
        let (_tmp, mgr) = setup();
        let content = b"hello world";
        let result = mgr.save_upload("test.txt", "text/plain", content).unwrap();

        assert!(!result.upload_id.is_empty());
        assert_eq!(result.filename, "test.txt");
        assert_eq!(result.mime_type, "text/plain");
        assert_eq!(result.size, 11);
        assert!(result.url.contains(&result.upload_id));

        // Verify files exist
        let upload_dir = mgr.upload_dir(&result.upload_id);
        assert!(upload_dir.join("file").exists());
        assert!(upload_dir.join("metadata.json").exists());

        // Verify content
        let saved = fs::read(upload_dir.join("file")).unwrap();
        assert_eq!(saved, content);
    }

    #[test]
    fn test_save_upload_bad_mime_type() {
        let (_tmp, mgr) = setup();
        let result = mgr.save_upload("test.exe", "application/exe", b"data");
        assert!(result.is_err());
    }

    #[test]
    fn test_save_upload_too_large() {
        let (_tmp, mgr) = setup();
        let content = vec![0u8; MAX_FILE_SIZE + 1];
        let result = mgr.save_upload("big.txt", "text/plain", &content);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_upload_file() {
        let (_tmp, mgr) = setup();
        let content = b"test content";
        let result = mgr.save_upload("test.txt", "text/plain", content).unwrap();

        let (read_content, mime_type) = mgr.read_upload_file(&result.upload_id).unwrap();
        assert_eq!(read_content, content);
        assert_eq!(mime_type, "text/plain");
    }

    #[test]
    fn test_read_upload_file_not_found() {
        let (_tmp, mgr) = setup();
        let result = mgr.read_upload_file("nonexistent-uuid");
        assert!(result.is_err());
    }

    #[test]
    fn test_read_upload_file_invalid_id() {
        let (_tmp, mgr) = setup();
        let result = mgr.read_upload_file("../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_upload_file_path() {
        let (_tmp, mgr) = setup();
        let content = b"image data";
        let result = mgr.save_upload("photo.png", "image/png", content).unwrap();

        let (path, mime) = mgr.get_upload_file(&result.upload_id).unwrap();
        assert!(path.exists());
        assert_eq!(mime, "image/png");
    }

    #[test]
    fn test_metadata_json_format() {
        let (_tmp, mgr) = setup();
        let result = mgr
            .save_upload("doc.pdf", "application/pdf", b"pdf data")
            .unwrap();

        let metadata_path = mgr.upload_dir(&result.upload_id).join("metadata.json");
        let metadata_str = fs::read_to_string(metadata_path).unwrap();
        let metadata: UploadMetadata = serde_json::from_str(&metadata_str).unwrap();

        assert_eq!(metadata.upload_id, result.upload_id);
        assert_eq!(metadata.filename, "doc.pdf");
        assert_eq!(metadata.mime_type, "application/pdf");
        assert_eq!(metadata.size, 8);
        assert!(!metadata.created_at.is_empty());
    }
}
