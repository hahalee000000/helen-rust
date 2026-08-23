//! Helen runtime bridge — spawns Helen processes and streams output
//!
//! This module handles spawning Helen agent processes and capturing their output
//! in real-time for streaming to the frontend via WebSocket.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;

/// Represents an active Helen stream
pub struct HelenStream {
    pub id: String,
    pub session_id: String,
    pub child: Child,
    pub sender: broadcast::Sender<StreamMessage>,
}

/// Message types for streaming
#[derive(Debug, Clone)]
pub enum StreamMessage {
    /// Text output from Helen
    Output(String),
    /// Stream completed
    Complete,
    /// Stream error
    Error(String),
}

/// Manages active Helen streams
pub struct HelenBridge {
    streams: Arc<Mutex<HashMap<String, HelenStream>>>,
    helen_path: PathBuf,
}

impl HelenBridge {
    /// Create a new Helen bridge
    pub fn new(helen_path: PathBuf) -> Self {
        Self {
            streams: Arc::new(Mutex::new(HashMap::new())),
            helen_path,
        }
    }

    /// Start a new Helen stream
    ///
    /// Spawns a Helen process with the given input and returns a stream ID
    /// and a receiver for streaming output.
    pub async fn start_stream(
        &self,
        session_id: &str,
        input: &str,
        cwd: &str,
    ) -> Result<(String, broadcast::Receiver<StreamMessage>), String> {
        let stream_id = Uuid::new_v4().to_string();

        // Spawn Helen process
        let mut child = Command::new(&self.helen_path)
            .arg("--session")
            .arg(session_id)
            .arg("--input")
            .arg(input)
            .current_dir(cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn Helen: {}", e))?;

        // Create broadcast channel for streaming
        let (sender, receiver) = broadcast::channel(100);

        // Capture stdout
        let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
        let sender_clone = sender.clone();
        let stream_id_clone = stream_id.clone();
        let streams_clone = self.streams.clone();

        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                if sender_clone.send(StreamMessage::Output(line)).is_err() {
                    break;
                }
            }

            // Stream completed
            let _ = sender_clone.send(StreamMessage::Complete);

            // Clean up stream
            let mut streams = streams_clone.lock().await;
            streams.remove(&stream_id_clone);
        });

        // Capture stderr for errors
        let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;
        let sender_err = sender.clone();

        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                let _ = sender_err.send(StreamMessage::Error(line));
            }
        });

        // Store stream
        let stream = HelenStream {
            id: stream_id.clone(),
            session_id: session_id.to_string(),
            child,
            sender: sender.clone(),
        };

        let mut streams = self.streams.lock().await;
        streams.insert(stream_id.clone(), stream);

        Ok((stream_id, receiver))
    }

    /// Stop an active stream
    pub async fn stop_stream(&self, stream_id: &str) -> Result<(), String> {
        let mut streams = self.streams.lock().await;

        if let Some(mut stream) = streams.remove(stream_id) {
            // Kill the child process
            let _ = stream.child.kill().await;
            let _ = stream.sender.send(StreamMessage::Complete);
            Ok(())
        } else {
            Err(format!("Stream {} not found", stream_id))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_helen_bridge_creation() {
        let temp = TempDir::new().unwrap();
        let bridge = HelenBridge::new(temp.path().to_path_buf());
        assert!(bridge.helen_path.exists() || bridge.helen_path.to_str().unwrap().contains("tmp"));
    }

    #[tokio::test]
    async fn test_stop_nonexistent_stream() {
        let temp = TempDir::new().unwrap();
        let bridge = HelenBridge::new(temp.path().to_path_buf());
        let result = bridge.stop_stream("nonexistent").await;
        assert!(result.is_err());
    }
}
