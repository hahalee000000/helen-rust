//! HelenActorBridge — connects Rust WebUI to Helen ChatSessionActor
//!
//! This module provides the bridge between the Rust-based web server
//! and the Helen-based ChatSessionActor agent.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};

use helen_core::lexer::Scanner;
use helen_interpreter::interpreter::Interpreter;
use helen_parser::Parser;

use super::messages::{AgentOutput, StreamChunk, UserInput};

/// Bridge between Rust WebUI and Helen ChatSessionActor
///
/// The bridge spawns a Helen interpreter in a dedicated thread and
/// communicates via channels. The interpreter thread runs the
/// ChatSessionActor agent, which handles all agent logic.
pub struct HelenActorBridge {
    /// Whether the bridge is alive
    alive: Arc<AtomicBool>,
    /// Channel to send user input to the interpreter thread
    input_tx: mpsc::Sender<UserInput>,
    /// Channel to receive agent output from the interpreter thread
    _output_rx: mpsc::Receiver<AgentOutput>,
    /// Broadcast channel for streaming chunks
    stream_tx: broadcast::Sender<StreamChunk>,
}

impl HelenActorBridge {
    /// Create a new bridge (spawns Helen interpreter thread)
    ///
    /// # Arguments
    /// * `cwd` - Working directory for the agent
    /// * `session_id` - Session ID for transcript persistence
    /// * `env_context` - Environment context XML to inject into prompt
    pub fn new(_cwd: String, _session_id: String, _env_context: String) -> Self {
        let alive = Arc::new(AtomicBool::new(true));
        let (input_tx, _input_rx) = mpsc::channel(32);
        let (_output_tx, output_rx) = mpsc::channel(32);
        let (stream_tx, _) = broadcast::channel(100);

        // Spawn Helen interpreter thread
        let alive_clone = alive.clone();
        std::thread::spawn(move || {
            // 1. Create interpreter with full runtime
            let mut interp = Interpreter::new();
            
            // 2. Install Python FFI (if feature enabled)
            #[cfg(feature = "python-ffi")]
            {
                if let Err(e) = helen_ffi::install() {
                    eprintln!("Python FFI install failed: {}", e);
                }
            }
            
            // 3. Load Helen agent files
            let agent_dir = std::env::var("HELEN_AGENT_DIR")
                .unwrap_or_else(|_| "/home/rxx/helen/helen/agent".to_string());
            
            let files_to_load = vec![
                "utils.helen",
                "lang.helen",
                "json_utils.helen",
                "output.helen",
                "context.helen",
                "context_manager.helen",
                "memory_utils.helen",
                "session_stats.helen",
                "system_reminders.helen",
                "task_manager.helen",
                "ui_event_queue.helen",
                "commands.helen",
                "chat_session_actor.helen",
                "chat_actor.helen",
            ];
            
            for file in files_to_load {
                let path = format!("{}/{}", agent_dir, file);
                match std::fs::read_to_string(&path) {
                    Ok(source) => {
                        let mut scanner = Scanner::new(&source, file);
                        let tokens = scanner.scan_all();
                        let mut parser = Parser::new(tokens);
                        let program = parser.parse();
                        
                        if !parser.errors().is_empty() {
                            eprintln!("Parse errors in {}: {:?}", file, parser.errors());
                            continue;
                        }
                        
                        if let Err(e) = interp.interpret(&program) {
                            eprintln!("Load error in {}: {:?}", file, e);
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to read {}: {}", path, e);
                    }
                }
            }
            
            // 4. Message loop (will be implemented in Task 2.2)
            // For now, just keep thread alive
            loop {
                std::thread::sleep(std::time::Duration::from_secs(1));
                if !alive_clone.load(Ordering::Relaxed) {
                    break;
                }
            }
            
            // Clean up interpreter
            drop(interp);
        });

        Self {
            alive,
            input_tx,
            _output_rx: output_rx,
            stream_tx,
        }
    }

    /// Check if bridge is alive
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    /// Send user input to ChatSessionActor
    ///
    /// # Arguments
    /// * `content` - The message content from the user
    /// * `file_paths` - Optional file paths attached to the message
    pub async fn send_message(&self, content: String, file_paths: Vec<String>) {
        let input = UserInput {
            content,
            file_paths,
            request_id: uuid::Uuid::new_v4().to_string(),
        };
        let _ = self.input_tx.send(input).await;
    }

    /// Subscribe to streaming chunks
    ///
    /// Returns a receiver that will receive streaming chunks as they
    /// are generated by the ChatSessionActor.
    pub fn subscribe_stream(&self) -> broadcast::Receiver<StreamChunk> {
        self.stream_tx.subscribe()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bridge_creation() {
        let bridge = HelenActorBridge::new(
            "/tmp".to_string(),
            "test-session".to_string(),
            "<context></context>".to_string(),
        );
        assert!(bridge.is_alive());
    }

    #[tokio::test]
    async fn test_bridge_send_message() {
        let bridge = HelenActorBridge::new(
            "/tmp".to_string(),
            "test-session".to_string(),
            "<context></context>".to_string(),
        );
        
        // Should not panic
        bridge.send_message("Hello".to_string(), vec![]).await;
    }
}
