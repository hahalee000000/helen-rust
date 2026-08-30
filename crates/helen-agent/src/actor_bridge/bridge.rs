//! HelenActorBridge — connects Rust WebUI to Helen ChatSessionActor
//!
//! This module provides the bridge between the Rust-based web server
//! and the Helen-based ChatSessionActor agent.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc as std_mpsc;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{broadcast, mpsc};

use helen_core::lexer::Scanner;
use helen_interpreter::exceptions::ExceptionValue;
use helen_interpreter::interpreter::Interpreter;
use helen_interpreter::llm_runtime::{LlmResponse as InterpreterLlmResponse, LlmRuntime};
use helen_parser::Parser;
use helen_runtime::http_llm::HttpLLMRuntime;
use helen_runtime::llm::LlmRuntime as RuntimeLlmRuntime;
use std::cell::RefCell;

use super::messages::{AgentOutput, StreamChunk, UserInput};

/// Adapter that wraps `HttpLLMRuntime` to implement the interpreter's `LlmRuntime` trait.
/// (Duplicated from helen-rust/src/llm_adapter.rs to avoid circular dependency)
struct HttpLlmAdapter {
    inner: RefCell<HttpLLMRuntime>,
}

impl HttpLlmAdapter {
    fn new(runtime: HttpLLMRuntime) -> Self {
        Self {
            inner: RefCell::new(runtime),
        }
    }
}

impl LlmRuntime for HttpLlmAdapter {
    fn route(
        &self,
        description: &str,
        branches: &[String],
        context: Option<&str>,
    ) -> Result<Option<String>, ExceptionValue> {
        let mut runtime = self.inner.borrow_mut();
        runtime
            .route(description, branches, context)
            .map_err(|e| ExceptionValue::new("RuntimeError", e, None))
    }

    fn act(
        &self,
        prompt: &str,
        tools: &[serde_json::Value],
        model: Option<&str>,
        temperature: f64,
        max_turns: usize,
        max_tokens: Option<u64>,
        history: &[serde_json::Value],
        system_prompt: Option<&str>,
        dispatch_fn: Option<&dyn Fn(&str, &serde_json::Value) -> String>,
        thinking_enabled: bool,
        reasoning_effort: Option<&str>,
    ) -> Result<InterpreterLlmResponse, ExceptionValue> {
        let mut runtime = self.inner.borrow_mut();
        let response = runtime
            .act(
                prompt,
                Some(tools),
                model,
                temperature,
                max_turns,
                max_tokens,
                Some(history),
                system_prompt,
                dispatch_fn,
                thinking_enabled,
                reasoning_effort,
            )
            .map_err(|e| ExceptionValue::new("RuntimeError", e, None))?;
        Ok(InterpreterLlmResponse {
            text: response.text,
            tool_calls: response.tool_calls,
            model: response.model,
        })
    }

    fn act_stream(
        &self,
        prompt: &str,
        model: Option<&str>,
        temperature: f64,
        system_prompt: Option<&str>,
        tools: &[serde_json::Value],
        max_turns: usize,
        history: &[serde_json::Value],
        dispatch_fn: Option<&dyn Fn(&str, &serde_json::Value) -> String>,
        on_event: &mut dyn FnMut(serde_json::Value) -> bool,
        thinking_enabled: bool,
        reasoning_effort: Option<&str>,
    ) -> Result<(), ExceptionValue> {
        let mut runtime = self.inner.borrow_mut();
        runtime
            .act_stream(
                prompt,
                model,
                temperature,
                system_prompt,
                Some(tools),
                max_turns,
                None,
                Some(history),
                dispatch_fn,
                on_event,
                thinking_enabled,
                reasoning_effort,
            )
            .map_err(|e| ExceptionValue::new("RuntimeError", e, None))
    }
}

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

            // 1b. Configure LLM runtime from environment (same as REPL)
            let llm_runtime = HttpLLMRuntime::new(None, None, None);
            if !llm_runtime.api_key.is_empty() && llm_runtime.api_key != "sk-placeholder" {
                let adapter = HttpLlmAdapter::new(llm_runtime);
                interp.set_llm_runtime(Arc::new(adapter));
                eprintln!("HelenActorBridge: LLM runtime configured from environment");
            } else {
                eprintln!(
                    "HelenActorBridge: WARNING — no valid LLM API key found. Tools will not work."
                );
            }

            // 2. Load Helen agent files
            let agent_dir = std::env::var("HELEN_AGENT_DIR")
                .unwrap_or_else(|_| "/home/rxx/helen-rust/crates/helen-agent/agent".to_string());

            // Set the working directory for import resolution
            if let Err(e) = std::env::set_current_dir(&agent_dir) {
                eprintln!("Failed to set working directory to {}: {}", agent_dir, e);
            }

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

            // 3. Call spawn_chat_actor() to start the ChatSessionActor
            // This function spawns the actor and stores the mailbox in _chat_actor_mailbox
            if let Some(func) = interp.functions.get("spawn_chat_actor").cloned() {
                let span =
                    helen_core::source::SourceSpan::new("chat_actor.helen".to_string(), 0, 0, 0, 0);
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

            // 4. Message loop: receive messages from Rust and call Helen functions
            loop {
                // Check if we should exit
                if !alive_clone.load(Ordering::Relaxed) {
                    break;
                }

                // Try to receive a message from Rust (blocking with timeout)
                match input_rx_for_thread.recv_timeout(std::time::Duration::from_millis(100)) {
                    Ok(input) => {
                        // Call tui_chat_handler_actor_stream(user_input, file_paths, streaming_channel)
                        if let Some(func) = interp
                            .functions
                            .get("tui_chat_handler_actor_stream")
                            .cloned()
                        {
                            let span = helen_core::source::SourceSpan::new(
                                "chat_actor.helen".to_string(),
                                0,
                                0,
                                0,
                                0,
                            );

                            // Convert arguments to Helen values
                            let user_input_val = helen_interpreter::value::Value::Str(
                                std::rc::Rc::from(input.content.as_str()),
                            );
                            let file_paths_val = helen_interpreter::value::Value::List(
                                std::rc::Rc::new(std::cell::RefCell::new(
                                    input
                                        .file_paths
                                        .iter()
                                        .map(|p| {
                                            helen_interpreter::value::Value::Str(std::rc::Rc::from(
                                                p.as_str(),
                                            ))
                                        })
                                        .collect(),
                                )),
                            );

                            // Create a Helen Channel for streaming
                            // IMPORTANT: Create TWO endpoints with opposite is_main values
                            // Bridge endpoint (is_main=true): sends to to_spawned, receives from to_main
                            // Helen endpoint (is_main=false): sends to to_main, receives from to_spawned
                            let streaming_channel =
                                std::sync::Arc::new(helen_runtime::channel::Channel::<
                                    helen_interpreter::value::ChannelMsg,
                                >::new(
                                    "streaming_channel"
                                ));
                            let bridge_endpoint =
                                std::sync::Arc::new(helen_runtime::channel::ChannelEndpoint::new(
                                    streaming_channel.clone(),
                                    true, // Bridge is "main"
                                ));
                            let helen_endpoint =
                                std::sync::Arc::new(helen_runtime::channel::ChannelEndpoint::new(
                                    streaming_channel.clone(),
                                    false, // Helen is "spawned"
                                ));
                            let streaming_channel_val =
                                helen_interpreter::value::Value::Channel(helen_endpoint.clone());

                            // Spawn a thread to forward chunks from Helen Channel to broadcast channel
                            let stream_tx_clone2 = stream_tx_clone.clone();
                            std::thread::spawn(move || {
                                let mut sequence = 0u64;
                                eprintln!("[BRIDGE DEBUG] Streaming thread started");
                                loop {
                                    // Receive from Helen Channel (blocking with timeout)
                                    if let Some(msg) = bridge_endpoint
                                        .receive(Some(std::time::Duration::from_millis(100)))
                                    {
                                        eprintln!(
                                            "[BRIDGE DEBUG] Received message from channel: {:?}",
                                            msg.0
                                        );
                                        // Parse the message
                                        if let helen_interpreter::value::Value::Map(map) = msg.0 {
                                            let map_ref = map.borrow();
                                            if let Some(helen_interpreter::value::Value::Str(
                                                type_str,
                                            )) =
                                                map_ref.get(&helen_interpreter::value::Value::Str(
                                                    std::rc::Rc::from("type"),
                                                ))
                                            {
                                                if type_str.as_ref() == "chunk" {
                                                    if let Some(
                                                        helen_interpreter::value::Value::Str(
                                                            content,
                                                        ),
                                                    ) = map_ref.get(
                                                        &helen_interpreter::value::Value::Str(
                                                            std::rc::Rc::from("content"),
                                                        ),
                                                    ) {
                                                        eprintln!("[BRIDGE DEBUG] Sending chunk to broadcast: {:?}", &content.as_ref()[..content.as_ref().len().min(50)]);
                                                        let chunk = StreamChunk {
                                                            sequence,
                                                            content: content.to_string(),
                                                        };
                                                        if stream_tx_clone2.send(chunk).is_err() {
                                                            eprintln!("[BRIDGE DEBUG] Failed to send streaming chunk to broadcast");
                                                            break;
                                                        }
                                                        sequence += 1;
                                                    }
                                                } else if type_str.as_ref() == "complete" {
                                                    eprintln!(
                                                        "[BRIDGE DEBUG] Received complete signal"
                                                    );
                                                    // Streaming complete
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                                eprintln!("[BRIDGE DEBUG] Streaming thread exiting");
                            });

                            // Call the streaming function
                            match interp.call_function(
                                &func,
                                vec![user_input_val, file_paths_val, streaming_channel_val],
                                None,
                                &span,
                            ) {
                                Ok(result) => {
                                    // Convert Helen Value to string
                                    let response_str = format!("{:?}", result);

                                    // Send complete response via output channel
                                    let output = AgentOutput::ResponseComplete {
                                        request_id: input.request_id.clone(),
                                        content: response_str,
                                    };
                                    if let Err(e) = output_tx.send(output) {
                                        eprintln!(
                                            "Failed to send response to output channel: {:?}",
                                            e
                                        );
                                    }
                                }
                                Err(e) => {
                                    eprintln!("tui_chat_handler_actor_stream() failed: {:?}", e);
                                    let output = AgentOutput::Error {
                                        request_id: input.request_id.clone(),
                                        error_msg: format!("{:?}", e),
                                    };
                                    if let Err(e) = output_tx.send(output) {
                                        eprintln!(
                                            "Failed to send error to output channel: {:?}",
                                            e
                                        );
                                    }
                                }
                            }
                        } else {
                            eprintln!("tui_chat_handler_actor_stream() function not found");
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
