//! MediaPart dataclass for multimodal content in Helen.
//!
//! Byte-faithful port of `helen/runtime/media.py` (v1.44.0).

use std::collections::HashMap;

/// Represents a piece of multimodal content.
///
/// MediaPart is a first-class citizen in Helen - it can be assigned to variables,
/// passed as function arguments, stored in lists, etc.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaPart {
    /// Where the content comes from - "file", "url", or "base64"
    pub source: String,
    /// The actual content - file path, URL, or base64-encoded string
    pub content: String,
    /// MIME type - "image/png", "video/mp4", "audio/mp3", etc.
    pub mime: String,
    /// High-level type - "image", "video", or "audio"
    pub media_type: String,
    /// Additional parameters (detail, alt, etc.)
    pub metadata: HashMap<String, String>,
}

impl MediaPart {
    /// Create a new MediaPart with validation.
    pub fn new(
        source: String,
        content: String,
        mime: String,
        media_type: String,
        metadata: HashMap<String, String>,
    ) -> Result<Self, String> {
        // Validate source
        let valid_sources = ["file", "url", "base64"];
        if !valid_sources.contains(&source.as_str()) {
            return Err(format!(
                "Invalid source: {}. Must be one of {:?}",
                source, valid_sources
            ));
        }

        // Validate media_type
        let valid_media_types = ["image", "video", "audio"];
        if !valid_media_types.contains(&media_type.as_str()) {
            return Err(format!(
                "Invalid media_type: {}. Must be one of {:?}",
                media_type, valid_media_types
            ));
        }

        // Validate mime
        if mime.is_empty() {
            return Err("mime type cannot be empty".to_string());
        }

        Ok(MediaPart {
            source,
            content,
            mime,
            media_type,
            metadata,
        })
    }

    /// Create a MediaPart from a file path.
    pub fn from_file(path: String, mime: String, media_type: String) -> Result<Self, String> {
        Self::new("file".to_string(), path, mime, media_type, HashMap::new())
    }

    /// Create a MediaPart from a URL.
    pub fn from_url(url: String, mime: String, media_type: String) -> Result<Self, String> {
        Self::new("url".to_string(), url, mime, media_type, HashMap::new())
    }

    /// Create a MediaPart from base64 data.
    pub fn from_base64(data: String, mime: String, media_type: String) -> Result<Self, String> {
        Self::new("base64".to_string(), data, mime, media_type, HashMap::new())
    }

    /// Check if this is an image.
    pub fn is_image(&self) -> bool {
        self.media_type == "image"
    }

    /// Check if this is a video.
    pub fn is_video(&self) -> bool {
        self.media_type == "video"
    }

    /// Check if this is audio.
    pub fn is_audio(&self) -> bool {
        self.media_type == "audio"
    }
}

impl std::fmt::Display for MediaPart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let content_preview = if self.content.len() > 50 {
            let end = (0..=50)
                .rev()
                .find(|&i| self.content.is_char_boundary(i))
                .unwrap_or(0);
            format!("{}...", &self.content[..end])
        } else {
            self.content.clone()
        };
        write!(
            f,
            "<Media:{} {}:{}>",
            self.media_type, self.source, content_preview
        )
    }
}
