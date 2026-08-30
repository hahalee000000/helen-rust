//! HelenActorBridge — connects Rust WebUI to Helen ChatSessionActor
//!
//! This module provides the bridge between the Rust-based web server
//! and the Helen-based ChatSessionActor agent.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::mpsc as std_mpsc;
use std::time::{SystemTime, UNIX_EPOCH};
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
    /// Channel to send user input to the interpreter thread (tokio for async)
    input_tx: mpsc::Sender<UserInput>,
    /// Channel to receive agent output from the interpreter thread
    output_rx: Arc<tokio::sync::Mutex<std_mpsc::Receiver<AgentOutput>>>,
    /// Broadcast channel for streaming chunks
    stream_tx: broadcast::Sender<StreamChunk>,
    /// Last activity timestamp (seconds since UNIX epoch)
    last_activity: Arc<AtomicU64>,
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
        let (input_tx, mut input_rx_std) = mpsc::channel::<UserInput>(32);
        let (input_tx_std, input_rx_std_sync) = std_mpsc::channel::<UserInput>();
        let (output_tx, output_rx) = std_mpsc::channel::<AgentOutput>();
        let (stream_tx, _) = broadcast::channel(100);
        
        // Initialize last activity timestamp
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let last_activity = Arc::new(AtomicU64::new(now));
        let last_activity_clone = last_activity.clone();
        
        // Wrap output_rx in Arc<Mutex> for sharing
        let output_rx = Arc::new(tokio::sync::Mutex::new(output_rx));

        // Wrap the std receiver in Arc for sharing with interpreter thread
        // Note: We don't actually need Arc here, just move the receiver
        let input_rx_for_thread = input_rx_std_sync;

        // Spawn a bridge thread to forward tokio messages to std channel
        let alive_bridge = alive.clone();
        std::thread::spawn(move || {
            // Create a tokio runtime for this thread
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                while let Some(input) = input_rx_std.recv().await {
                    if input_tx_std.send(input).is_err() {
                        break;
                    }
                    if !alive_bridge.load(Ordering::Relaxed) {
                        break;
                    }
                }
            });
        });

        // Spawn Helen interpreter thread
        let alive_clone = alive.clone();
        let stream_tx_clone = stream_tx.clone();
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
            
            // 4. Call spawn_chat_actor() to start the ChatSessionActor
            // This function spawns the actor and stores the mailbox in _chat_actor_mailbox
            if let Some(func) = interp.functions.get("spawn_chat_actor").cloned() {
                let span = helen_core::source::SourceSpan::new("chat_actor.helen".to_string(), 0, 0, 0, 0);
                match interp.call_function(&func, vec![], None, &span) {
                    Ok(result) => {
                        eprintln!("spawn_chat_actor() returned: {:?}", result);
                    }
                    Err(e) => {
                        eprintln!("spawn_chat_actor() failed: {:?}", e);
                    }
                }
            } else {
                eprintln!("spawn_chat_actor() function not found");
            }
            
            // 5. Message loop: receive messages from Rust and call Helen functions
            loop {
                // Check if we should exit
                if !alive_clone.load(Ordering::Relaxed) {
                    break;
                }
                
                // Try to receive a message from Rust (blocking with timeout)
                match input_rx_for_thread.recv_timeout(std::time::Duration::from_millis(100)) {
                    Ok(input) => {
                        // Call tui_chat_handler_actor(user_input, file_paths)
                        if let Some(func) = interp.functions.get("tui_chat_handler_actor").cloned() {
                            let span = helen_core::source::SourceSpan::new("chat_actor.helen".to_string(), 0, 0, 0, 0);
                            
                            // Convert arguments to Helen values
                            let user_input_val = helen_interpreter::value::Value::Str(std::rc::Rc::from(input.content.as_str()));
                            let file_paths_val = helen_interpreter::value::Value::List(std::rc::Rc::new(std::cell::RefCell::new(
                                input.file_paths.iter()
                                    .map(|p| helen_interpreter::value::Value::Str(std::rc::Rc::from(p.as_str())))
                                    .collect()
                            )));
                            
                            match interp.call_function(&func, vec![user_input_val, file_paths_val], None, &span) {
                                Ok(result) => {
                                    // Convert Helen Value to string
                                    let response_str = format!("{:?}", result);
                                    
                                    // NOTE: This is NOT true streaming.
                                    // The full response is received before any chunk is sent.
                                    // True streaming would require:
                                    // 1. Modifying Helen functions to accept streaming callbacks
                                    // 2. Hooking into Helen's LLM runtime streaming API
                                    // 3. Forwarding chunks immediately as they arrive from LLM
                                    //
                                    // Current implementation: "simulated streaming"
                                    // - Splits response into chunks after receiving full response
                                    // - No artificial delay (removed misleading 10ms delay)
                                    // - Chunks are sent immediately, but all at once
                                    
                                    let chunk_size = 50; // characters per chunk
                                    let chars: Vec<char> = response_str.chars().collect();
                                    
                                    for (sequence, chunk_start) in (0..chars.len()).step_by(chunk_size).enumerate() {
                                        let chunk_end = (chunk_start + chunk_size).min(chars.len());
                                        let chunk_content: String = chars[chunk_start..chunk_end].iter().collect();
                                        
                                        let chunk = StreamChunk {
                                            sequence: sequence as u64,
                                            content: chunk_content,
                                        };
                                        
                                        // Send chunk via broadcast channel (no artificial delay)
                                        if stream_tx_clone.send(chunk).is_err() {
                                            eprintln!("Failed to send streaming chunk");
                                            break;
                                        }
                                    }
                                    
                                    // Send complete response via output channel
                                    let output = AgentOutput::ResponseComplete {
                                        request_id: input.request_id.clone(),
                                        content: response_str,
                                    };
                                    if let Err(e) = output_tx.send(output) {
                                        eprintln!("Failed to send response to output channel: {:?}", e);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("tui_chat_handler_actor() failed: {:?}", e);
                                    let output = AgentOutput::Error {
                                        request_id: input.request_id.clone(),
                                        error_msg: format!("{:?}", e),
                                    };
                                    if let Err(e) = output_tx.send(output) {
                                        eprintln!("Failed to send error to output channel: {:?}", e);
                                    }
                                }
                            }
                        } else {
                            eprintln!("tui_chat_handler_actor() function not found");
                        }
                    }
                    Err(std_mpsc::RecvTimeoutError::Timeout) => {
                        // No message, continue loop
                        continue;
                    }
                    Err(std_mpsc::RecvTimeoutError::Disconnected) => {
                        // Channel closed, exit loop
                        break;
                    }
                }
            }
            
            // Clean up interpreter
            drop(interp);
        });

        Self {
            alive,
            input_tx,
            output_rx,
            stream_tx,
            last_activity: last_activity_clone,
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
        // Update last activity timestamp
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.last_activity.store(now, Ordering::Relaxed);
        
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

    /// Receive agent output (non-blocking)
    ///
    /// Returns the next available output, or None if no output is available.
    pub async fn receive_output(&self) -> Option<AgentOutput> {
        let rx = self.output_rx.lock().await;
        rx.try_recv().ok()
    }

    /// Check if bridge is responsive (heartbeat)
    ///
    /// Returns true if the bridge is alive and responsive.
    pub fn heartbeat(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }

    /// Gracefully shutdown the bridge
    ///
    /// Signals the interpreter thread to exit and waits for cleanup.
    pub async fn shutdown(&self) {
        self.alive.store(false, Ordering::Relaxed);
        // Give thread time to exit
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    /// Get the last activity timestamp (seconds since UNIX epoch)
    pub fn last_activity(&self) -> u64 {
        self.last_activity.load(Ordering::Relaxed)
    }

    /// Check if bridge is stale (no activity for too long)
    ///
    /// # Arguments
    /// * `timeout` - Maximum allowed inactivity duration
    pub fn is_stale(&self, timeout: std::time::Duration) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let last = self.last_activity.load(Ordering::Relaxed);
        let elapsed = now.saturating_sub(last);
        elapsed > timeout.as_secs()
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
