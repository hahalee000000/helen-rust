//! Large media external storage for Helen multimodal support.
//!
//! Phase 3 (v1.17): Handles extraction and restoration of large media content
//! (images, audio) from transcript messages to external files.
//!
//! When media content exceeds a configurable threshold (default 1MB), it is:
//! 1. Extracted to a binary file in the session directory
//! 2. Replaced with a `media_ref` containing path, MIME type, and integrity metadata
//! 3. On read, the original base64 content is restored from the file
//!
//! v1.31: Added integrity verification (binary_size + checksum) to detect
//! truncated or corrupted media files. Degradation handling returns a
//! placeholder text when files are missing or corrupted.

use crate::transcript::Message;
use serde_json::json;
use std::fs;
use std::io;
use std::path::PathBuf;

/// Default threshold for external storage (1MB in bytes).
#[allow(dead_code)]
const DEFAULT_THRESHOLD_BYTES: u64 = 1_048_576;

/// MediaStorage manages extraction and restoration of large media content.
pub struct MediaStorage {
    /// Directory where media files are stored.
    session_dir: PathBuf,
    /// Threshold in bytes: content >= this is extracted to external file.
    threshold_bytes: u64,
}

impl MediaStorage {
    /// Create a new MediaStorage instance.
    ///
    /// # Arguments
    /// * `session_dir` - Directory for storing media files
    /// * `threshold_mb` - Threshold in megabytes (e.g., 1.0 = 1MB)
    pub fn new(session_dir: PathBuf, threshold_mb: f64) -> io::Result<Self> {
        // Ensure session directory exists
        fs::create_dir_all(&session_dir)?;

        let threshold_bytes = (threshold_mb * 1_048_576.0) as u64;

        Ok(Self {
            session_dir,
            threshold_bytes,
        })
    }

    /// Create with default threshold (1MB).
    pub fn with_default_threshold(session_dir: PathBuf) -> io::Result<Self> {
        Self::new(session_dir, 1.0)
    }

    /// Extract large media content from a message to external file.
    ///
    /// If the message contains media content >= threshold, it is extracted
    /// to a binary file and replaced with a `media_ref`.
    ///
    /// Returns a new message with either:
    /// - Original content (if below threshold)
    /// - `media_ref` with path, MIME type, and integrity metadata
    pub fn extract_media_from_message(&self, message: &Message) -> io::Result<Message> {
        // Check if content has media source
        let content = &message.content;

        // Try to extract base64 data from image/audio content
        if let Some(source) = content.get("source") {
            if source.get("type").and_then(|t| t.as_str()) == Some("base64") {
                if let Some(data) = source.get("data").and_then(|d| d.as_str()) {
                    let data_bytes = data.len() as u64;

                    // Check if content exceeds threshold
                    if data_bytes >= self.threshold_bytes {
                        // Extract to external file
                        let media_type = content
                            .get("type")
                            .and_then(|t| t.as_str())
                            .unwrap_or("unknown");
                        let mime_type = source
                            .get("media_type")
                            .and_then(|m| m.as_str())
                            .unwrap_or("application/octet-stream");

                        // Generate unique filename
                        let uuid = if message.uuid.is_empty() {
                            crate::transcript::generate_uuid()
                        } else {
                            message.uuid.clone()
                        };
                        let filename = format!("{}_{}.bin", media_type, &uuid[..8.min(uuid.len())]);
                        let filepath = self.session_dir.join(&filename);

                        // Decode base64 and write to file
                        let decoded = base64_decode(data)?;
                        fs::write(&filepath, &decoded)?;

                        // Calculate integrity metadata
                        let binary_size = decoded.len();
                        let checksum = calculate_checksum(&decoded);

                        // Build media_ref
                        let media_ref = json!({
                            "path": filename,
                            "media_type": media_type,
                            "mime_type": mime_type,
                            "binary_size": binary_size,
                            "checksum": checksum
                        });

                        // Return new message with media_ref
                        let mut new_content = content.clone();
                        new_content.as_object_mut().unwrap().remove("source");
                        new_content
                            .as_object_mut()
                            .unwrap()
                            .insert("media_ref".to_string(), media_ref);

                        return Ok(Message {
                            content: new_content,
                            ..message.clone()
                        });
                    }
                }
            }
        }

        // Content below threshold or not extractable - return as-is
        Ok(message.clone())
    }

    /// Restore media content from external file to message.
    ///
    /// If the message contains a `media_ref`, the original base64 content
    /// is restored from the external file.
    ///
    /// Returns a new message with either:
    /// - Original content (if no media_ref)
    /// - Restored `source` with base64 data
    /// - Degradation marker if file is missing or corrupted
    pub fn restore_media_to_message(&self, message: &Message) -> io::Result<Message> {
        let content = &message.content;

        // Check if content has media_ref
        if let Some(media_ref) = content.get("media_ref") {
            let path = match media_ref.get("path").and_then(|p| p.as_str()) {
                Some(p) => p,
                None => return Ok(message.clone()),
            };

            let filepath = self.session_dir.join(path);

            // Try to read the file
            match fs::read(&filepath) {
                Ok(decoded) => {
                    // Verify integrity if metadata is present
                    if let (Some(expected_size), Some(expected_checksum)) = (
                        media_ref.get("binary_size").and_then(|s| s.as_u64()),
                        media_ref.get("checksum").and_then(|c| c.as_str()),
                    ) {
                        let actual_size = decoded.len() as u64;
                        let actual_checksum = calculate_checksum(&decoded);

                        if actual_size != expected_size || actual_checksum != expected_checksum {
                            // Integrity check failed - degrade
                            return Ok(
                                self.create_degraded_message(message, "Media file corrupted")
                            );
                        }
                    }

                    // Encode to base64
                    let encoded = base64_encode(&decoded);

                    // Restore source
                    let media_type = media_ref
                        .get("media_type")
                        .and_then(|t| t.as_str())
                        .unwrap_or("unknown");
                    let mime_type = media_ref
                        .get("mime_type")
                        .and_then(|m| m.as_str())
                        .unwrap_or("application/octet-stream");

                    let source = json!({
                        "type": "base64",
                        "media_type": mime_type,
                        "data": encoded
                    });

                    let mut new_content = content.clone();
                    new_content.as_object_mut().unwrap().remove("media_ref");
                    new_content
                        .as_object_mut()
                        .unwrap()
                        .insert("source".to_string(), source);

                    // Ensure type field is set
                    if !new_content.get("type").is_some() {
                        new_content
                            .as_object_mut()
                            .unwrap()
                            .insert("type".to_string(), json!(media_type));
                    }

                    Ok(Message {
                        content: new_content,
                        ..message.clone()
                    })
                }
                Err(_) => {
                    // File missing - degrade
                    Ok(self
                        .create_degraded_message(message, &format!("Media file missing: {}", path)))
                }
            }
        } else {
            // No media_ref - return as-is
            Ok(message.clone())
        }
    }

    /// Create a degraded message with placeholder text.
    fn create_degraded_message(&self, original: &Message, reason: &str) -> Message {
        let media_ref = original.content.get("media_ref");
        let media_type = media_ref
            .and_then(|m| m.get("media_type"))
            .and_then(|t| t.as_str())
            .unwrap_or("media");
        let path = media_ref
            .and_then(|m| m.get("path"))
            .and_then(|p| p.as_str())
            .unwrap_or("unknown");

        let placeholder = match media_type {
            "image" => format!("[图片丢失: {}]", path),
            "audio" => format!("[音频丢失: {}]", path),
            _ => format!("[媒体丢失: {}]", path),
        };

        let degraded_content = json!({
            "type": "text",
            "text": placeholder,
            "degraded": true,
            "degradation_reason": reason
        });

        Message {
            content: degraded_content,
            ..original.clone()
        }
    }
}

/// Decode base64 string to bytes.
fn base64_decode(input: &str) -> io::Result<Vec<u8>> {
    // Simple base64 decoder (without padding validation)
    let input = input.trim();
    let mut output = Vec::with_capacity(input.len() * 3 / 4);

    let mut buffer = 0u32;
    let mut bits_collected = 0;

    for c in input.chars() {
        let value = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            '=' => continue, // Skip padding
            _ => continue,   // Skip invalid characters
        };

        buffer = (buffer << 6) | value;
        bits_collected += 6;

        if bits_collected >= 8 {
            bits_collected -= 8;
            output.push((buffer >> bits_collected) as u8);
        }
    }

    Ok(output)
}

/// Encode bytes to base64 string.
fn base64_encode(input: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut output = String::with_capacity(input.len().div_ceil(3) * 4);

    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };

        let triple = (b0 << 16) | (b1 << 8) | b2;

        output.push(ALPHABET[((triple >> 18) & 0x3F) as usize] as char);
        output.push(ALPHABET[((triple >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            output.push(ALPHABET[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            output.push('=');
        }

        if chunk.len() > 2 {
            output.push(ALPHABET[(triple & 0x3F) as usize] as char);
        } else {
            output.push('=');
        }
    }

    output
}

/// Calculate checksum for integrity verification.
fn calculate_checksum(data: &[u8]) -> String {
    // Simple checksum using sum of bytes (mod 2^32)
    let sum: u32 = data.iter().map(|&b| b as u32).sum();
    format!("{:08x}", sum)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_roundtrip() {
        let original = b"Hello, World!";
        let encoded = base64_encode(original);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_checksum() {
        let data = b"test data";
        let checksum = calculate_checksum(data);
        assert_eq!(checksum.len(), 8);
    }
}
