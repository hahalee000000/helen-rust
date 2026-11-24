//! WebSocket handler for real-time chat with HelenActorBridge

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::actor_bridge::bridge::HelenActorBridge;
use crate::actor_bridge::messages::AgentOutput;
use crate::api::sessions::AppState;

/// Create WebSocket router with HelenActorBridge
pub fn router() -> Router<AppState> {
    Router::new().route("/ws", get(ws_handler))
}

/// WebSocket upgrade handler
async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

/// Parsed WebSocket input message
#[derive(Debug)]
pub enum WsInput {
    Message {
        content: String,
        client_id: Option<String>,
    },
    Hint {
        content: String,
        client_id: Option<String>,
    },
    Cancel,
    Unknown(String),
    InvalidJson,
}

/// Parse a raw WebSocket text message into a structured WsInput
pub fn parse_ws_message(text: &str) -> WsInput {
    let data = match serde_json::from_str::<serde_json::Value>(text) {
        Ok(d) => d,
        Err(_) => return WsInput::InvalidJson,
    };

    let msg_type = data.get("type").and_then(|t| t.as_str()).unwrap_or("");

    match msg_type {
        "message" => {
            let content = data
                .get("content")
                .and_then(|i| i.as_str())
                .unwrap_or("")
                .to_string();
            let client_id = data
                .get("client_id")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string());
            WsInput::Message { content, client_id }
        }
        "hint" => {
            let content = data
                .get("content")
                .and_then(|i| i.as_str())
                .unwrap_or("")
                .to_string();
            let client_id = data
                .get("client_id")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string());
            WsInput::Hint { content, client_id }
        }
        "cancel" => WsInput::Cancel,
        other => WsInput::Unknown(other.to_string()),
    }
}

/// Build a JSON response string for processing_start
pub fn build_processing_start() -> String {
    json!({
        "type": "processing_start"
    })
    .to_string()
}

/// Build a JSON response string for processing_complete
pub fn build_processing_complete(
    is_slash: bool,
    content: Option<&str>,
    i18n_key: Option<&str>,
) -> String {
    let mut data = json!({
        "type": "processing_complete",
        "data": {
            "is_slash_response": is_slash
        }
    });
    if let Some(c) = content {
        data["data"]["content"] = json!(c);
    }
    if let Some(k) = i18n_key {
        data["data"]["i18n_key"] = json!(k);
    }
    data.to_string()
}

/// Build a JSON response string for llm_chunk
pub fn build_llm_chunk(content: &str) -> String {
    json!({
        "type": "llm_chunk",
        "data": {
            "content": content
        }
    })
    .to_string()
}

/// Build a JSON response string for llm_complete
pub fn build_llm_complete() -> String {
    json!({
        "type": "llm_complete"
    })
    .to_string()
}

/// Build a JSON response string for error
pub fn build_error(data: &str) -> String {
    json!({
        "type": "error",
        "data": {
            "content": data
        }
    })
    .to_string()
}

/// Build a JSON response string for status_update
pub fn build_status_update(cwd: &str, user: &str, hostname: &str) -> String {
    json!({
        "type": "status_update",
        "data": {
            "cwd": cwd,
            "user": user,
            "hostname": hostname
        }
    })
    .to_string()
}

/// Detect if a message is a slash command (starts with /)
pub fn is_slash_command(content: &str) -> bool {
    content.trim().starts_with('/')
}

/// Result of stripping protocol markers from slash command responses
#[derive(Debug, Default)]
pub struct StrippedResponse {
    /// Cleaned content (markers removed)
    pub content: Option<String>,
    /// Whether __HELEN_CLEAR_OK__ was found (frontend should clear messages)
    pub should_clear_messages: bool,
    /// Whether __HELEN_CLEAR_SESSION_OK__ was found (session was deleted)
    pub should_clear_session: bool,
}

/// Strip internal protocol markers from slash command responses.
/// These markers are used for backend coordination and should not be shown to users.
/// Returns StrippedResponse with detected markers for backend action.
pub fn strip_protocol_markers(content: &str) -> Option<String> {
    strip_protocol_markers_detailed(content).content
}

/// Strip internal protocol markers and return detailed marker detection info.
pub fn strip_protocol_markers_detailed(content: &str) -> StrippedResponse {
    let mut result = StrippedResponse::default();
    let mut c = content.to_string();

    // Detect markers before stripping
    if c.contains("__HELEN_CLEAR_OK__") {
        result.should_clear_messages = true;
    }
    if c.contains("__HELEN_CLEAR_SESSION_OK__") {
        result.should_clear_session = true;
        // Session deletion also implies clearing messages
        result.should_clear_messages = true;
    }

    // Strip markers
    c = c.replace("__HELEN_CLEAR_OK__", "").trim().to_string();
    c = c
        .replace("__HELEN_CLEAR_SESSION_OK__", "")
        .trim()
        .to_string();
    c = c.replace("__HELEN_RESTART_ACTOR__", "").trim().to_string();

    result.content = if c.is_empty() { None } else { Some(c) };
    result
}

/// Build a JSON response string for clear_messages event
pub fn build_clear_messages() -> String {
    json!({
        "type": "clear_messages"
    })
    .to_string()
}

/// Build a JSON response string for directory_changed event
pub fn build_directory_changed(
    cwd: &str,
    display_name: &str,
    session_id: &str,
    helen_session_id: Option<&str>,
) -> String {
    let mut data = json!({
        "type": "directory_changed",
        "data": {
            "cwd": cwd,
            "display_name": display_name,
            "session_id": session_id
        }
    });
    if let Some(sid) = helen_session_id {
        data["data"]["helen_session_id"] = json!(sid);
    }
    data.to_string()
}

/// Check if a message is a /dir command and extract the path argument
pub fn parse_dir_command(content: &str) -> Option<Option<String>> {
    let trimmed = content.trim();
    if !trimmed.starts_with("/dir") {
        return None;
    }

    let rest = trimmed[4..].trim();
    if rest.is_empty() {
        // /dir with no arguments - query current directory
        Some(None)
    } else {
        // /dir <path> - switch to new directory
        Some(Some(rest.to_string()))
    }
}

/// Handle WebSocket connection
async fn handle_socket(mut socket: WebSocket, state: AppState) {
    // Send initial status update
    {
        let state_guard = state.lock().await;
        let cwd = state_guard.directory_manager.get_cwd();
        let user = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
        let hostname = std::fs::read_to_string("/etc/hostname")
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());
        let _ = socket
            .send(Message::Text(build_status_update(&cwd, &user, &hostname)))
            .await;
    }

    // Create HelenActorBridge for this connection
    let bridge = Arc::new(Mutex::new(None::<HelenActorBridge>));

    // v1.46.28: Track whether we're currently processing a message
    // This allows us to use tokio::select! to handle cancel/hint immediately
    let mut is_processing = false;
    let mut pending_content: String;
    let mut pending_is_slash = false;
    let mut stream_rx: Option<
        tokio::sync::broadcast::Receiver<crate::actor_bridge::messages::StreamChunk>,
    > = None;

    loop {
        // Use tokio::select! to listen for both WebSocket messages AND response events
        // This allows cancel/hint to be processed immediately even while waiting for response
        tokio::select! {
            // Branch 1: Receive WebSocket messages (cancel, hint, new messages)
            ws_msg = socket.recv() => {
                match ws_msg {
                    Some(Ok(Message::Text(text))) => {
                        match parse_ws_message(&text) {
                            WsInput::Cancel => {
                                // Cancel current processing immediately
                                let bridge_guard = bridge.lock().await;
                                if let Some(bridge_ref) = bridge_guard.as_ref() {
                                    bridge_ref.cancel();
                                    // Send cancellation confirmation to frontend
                                    if socket
                                        .send(Message::Text(
                                            serde_json::json!({
                                                "type": "cancelled",
                                                "data": {"content": "Processing cancelled"}
                                            })
                                            .to_string(),
                                        ))
                                        .await
                                        .is_err()
                                    {
                                        break;
                                    }
                                    // Reset processing state
                                    is_processing = false;
                                    stream_rx = None;
                                }
                            }
                            WsInput::Hint { content, client_id } => {
                                // Hint injection — enqueue hint for mid-processing injection
                                eprintln!("[WS] Received hint: content='{}', client_id={:?}", content, client_id);
                                if !content.trim().is_empty() {
                                    let queue = crate::hint_queue::get_hint_queue();
                                    let client_id_str = client_id.clone().unwrap_or_default();
                                    eprintln!("[WS] Adding hint to queue: '{}'", content);
                                    queue.add_hint(content.clone(), client_id_str.clone());
                                    eprintln!("[WS] Hint added to queue successfully");
                                    // Send hint_queued confirmation to frontend
                                    if socket
                                        .send(Message::Text(
                                            serde_json::json!({
                                                "type": "hint_queued",
                                                "data": {
                                                    "content": content,
                                                    "client_id": client_id_str
                                                }
                                            })
                                            .to_string(),
                                        ))
                                        .await
                                        .is_err()
                                    {
                                        break;
                                    }
                                    eprintln!("[WS] hint_queued confirmation sent to frontend");
                                } else {
                                    eprintln!("[WS] Hint content is empty, skipping");
                                }
                            }
                            WsInput::Message { content, .. } => {
                                // If already processing, queue this message (not supported yet)
                                // For now, just ignore or handle sequentially
                                if is_processing {
                                    // Could send an error or queue it
                                    continue;
                                }

                                // Start processing new message
                                is_processing = true;
                                pending_content = content.clone();

                                // Signal processing start
                                if socket
                                    .send(Message::Text(build_processing_start()))
                                    .await
                                    .is_err()
                                {
                                    break;
                                }

                                // Get state
                                let state_guard = state.lock().await;
                                let cwd = state_guard.directory_manager.get_cwd();
                                let session_id = state_guard.directory_manager.cwd_to_session_id(&cwd);
                                drop(state_guard);

                                // Initialize bridge if not already done
                                {
                                    let mut bridge_guard = bridge.lock().await;
                                    if bridge_guard.is_none() {
                                        *bridge_guard = Some(HelenActorBridge::new(
                                            cwd.clone(),
                                            session_id,
                                            "<context></context>".to_string(),
                                        ));
                                    }
                                }

                                // Detect slash commands
                                pending_is_slash = is_slash_command(&content);

                                // Handle /dir command specially (don't send to Helen)
                                if let Some(path_arg) = parse_dir_command(&content) {
                                    let state_guard = state.lock().await;
                                    let current_cwd = state_guard.directory_manager.get_cwd();

                                    match path_arg {
                                        None => {
                                            let display_name = state_guard
                                                .directory_manager
                                                .get_display_name(Some(current_cwd.clone()));
                                            drop(state_guard);

                                            let response = json!({
                                                "type": "processing_complete",
                                                "data": {
                                                    "i18n_key": "dir.currentDir",
                                                    "params": {"path": current_cwd, "name": display_name},
                                                    "is_slash_response": true
                                                }
                                            }).to_string();

                                            if socket.send(Message::Text(response)).await.is_err() {
                                                break;
                                            }
                                            is_processing = false;
                                            continue;
                                        }
                                        Some(path) => {
                                            let result = state_guard.directory_manager.set_cwd(&path);

                                            if result.status == "ok" {
                                                let new_cwd = result.cwd.unwrap_or_else(|| current_cwd.clone());
                                                let display_name = result.display_name.unwrap_or_else(|| path.clone());

                                                if new_cwd != current_cwd {
                                                    let new_session_id = state_guard
                                                        .directory_manager
                                                        .cwd_to_session_id(&new_cwd);

                                                    let helen_sid = {
                                                        let bridge_guard = bridge.lock().await;
                                                        if bridge_guard.is_some() {
                                                            Some(new_session_id.clone())
                                                        } else {
                                                            None
                                                        }
                                                    };

                                                    drop(state_guard);

                                                    {
                                                        let mut bridge_guard = bridge.lock().await;
                                                        *bridge_guard = Some(HelenActorBridge::new(
                                                            new_cwd.clone(),
                                                            new_session_id.clone(),
                                                            "<context></context>".to_string(),
                                                        ));
                                                    }

                                                    let dir_changed = build_directory_changed(
                                                        &new_cwd,
                                                        &display_name,
                                                        &new_session_id,
                                                        helen_sid.as_deref(),
                                                    );

                                                    if socket.send(Message::Text(dir_changed)).await.is_err() {
                                                        break;
                                                    }

                                                    if socket.send(Message::Text(build_clear_messages())).await.is_err() {
                                                        break;
                                                    }
                                                } else {
                                                    drop(state_guard);
                                                }

                                                let response = json!({
                                                    "type": "processing_complete",
                                                    "data": {
                                                        "i18n_key": "dir.switched",
                                                        "params": {"path": new_cwd, "name": display_name},
                                                        "is_slash_response": true
                                                    }
                                                }).to_string();

                                                if socket.send(Message::Text(response)).await.is_err() {
                                                    break;
                                                }
                                            } else {
                                                drop(state_guard);
                                                let response = json!({
                                                    "type": "processing_complete",
                                                    "data": {
                                                        "i18n_key": "dir.failed",
                                                        "params": {"path": path, "error": result.status},
                                                        "is_slash_response": true
                                                    }
                                                }).to_string();

                                                if socket.send(Message::Text(response)).await.is_err() {
                                                    break;
                                                }
                                            }
                                            is_processing = false;
                                            continue;
                                        }
                                    }
                                }

                                // Send message to bridge for processing
                                let bridge_guard = bridge.lock().await;
                                if let Some(bridge_ref) = bridge_guard.as_ref() {
                                    // Clear cancel flag before processing
                                    bridge_ref.clear_cancel();

                                    // M25.44: Drain any stale output from previous cancelled request
                                    // This prevents old ResponseComplete messages from interfering with new requests
                                    while bridge_ref.try_receive_output().await.is_some() {
                                        // Discard stale output
                                    }

                                    // Subscribe to streaming chunks
                                    stream_rx = Some(bridge_ref.subscribe_stream());

                                    // Send user input to bridge
                                    bridge_ref.send_message(pending_content.clone(), vec![]).await;
                                } else {
                                    if socket.send(Message::Text(build_error("Bridge not initialized"))).await.is_err() {
                                        break;
                                    }
                                    is_processing = false;
                                }
                            }
                            WsInput::Unknown(msg_type) => {
                                if socket
                                    .send(Message::Text(build_error(&format!(
                                        "Unknown message type: {}",
                                        msg_type
                                    ))))
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            WsInput::InvalidJson => {
                                if socket
                                    .send(Message::Text(build_error("Invalid JSON")))
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) => break,
                    Some(Err(_)) => break,
                    None => break,
                    _ => {}
                }
            }

            // Branch 2: Receive response events from bridge (only when processing)
            response_event = async {
                if !is_processing {
                    // Not processing, wait forever (this branch will never be selected)
                    std::future::pending::<Option<AgentOutput>>().await
                } else {
                    let bridge_guard = bridge.lock().await;
                    if let Some(bridge_ref) = bridge_guard.as_ref() {
                        bridge_ref.receive_output().await
                    } else {
                        None
                    }
                }
            } => {
                if let Some(output) = response_event {
                    match output {
                        AgentOutput::ResponseComplete { content, .. } => {
                            // Send final complete message
                            if socket.send(Message::Text(build_llm_complete())).await.is_err() {
                                break;
                            }

                            // Send processing_complete
                            let stripped = if pending_is_slash && !content.is_empty() {
                                strip_protocol_markers_detailed(&content)
                            } else {
                                StrippedResponse::default()
                            };

                            if stripped.should_clear_messages
                                && socket.send(Message::Text(build_clear_messages())).await.is_err()
                            {
                                break;
                            }

                            if socket
                                .send(Message::Text(build_processing_complete(
                                    stripped.content.is_some(),
                                    stripped.content.as_deref(),
                                    None,
                                )))
                                .await
                                .is_err()
                            {
                                break;
                            }

                            // v1.46.30: Clear cancel flag after processing completes
                            // This ensures clean state for next message
                            let bridge_guard = bridge.lock().await;
                            if let Some(bridge_ref) = bridge_guard.as_ref() {
                                bridge_ref.clear_cancel();
                            }
                            drop(bridge_guard);

                            // Reset processing state
                            is_processing = false;
                            stream_rx = None;
                        }
                        AgentOutput::Error { error_msg, .. } => {
                            if socket.send(Message::Text(build_error(&error_msg))).await.is_err() {
                                break;
                            }
                            // v1.46.30: Clear cancel flag after error
                            let bridge_guard = bridge.lock().await;
                            if let Some(bridge_ref) = bridge_guard.as_ref() {
                                bridge_ref.clear_cancel();
                            }
                            drop(bridge_guard);
                            is_processing = false;
                            stream_rx = None;
                        }
                        _ => {}
                    }
                }
            }

            // Branch 3: Receive streaming chunks (only when processing and subscribed)
            stream_chunk = async {
                if let Some(ref mut rx) = stream_rx {
                    rx.recv().await.ok()
                } else {
                    std::future::pending::<Option<crate::actor_bridge::messages::StreamChunk>>().await
                }
            } => {
                if let Some(chunk) = stream_chunk {
                    if socket.send(Message::Text(build_llm_chunk(&chunk.content))).await.is_err() {
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_message() {
        let input = r#"{"type":"message","content":"Hello","client_id":"123"}"#;
        match parse_ws_message(input) {
            WsInput::Message { content, client_id } => {
                assert_eq!(content, "Hello");
                assert_eq!(client_id, Some("123".to_string()));
            }
            other => panic!("Expected Message, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_message_defaults() {
        let input = r#"{"type":"message"}"#;
        match parse_ws_message(input) {
            WsInput::Message { content, client_id } => {
                assert_eq!(content, "");
                assert_eq!(client_id, None);
            }
            other => panic!("Expected Message, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_cancel() {
        let input = r#"{"type":"cancel"}"#;
        match parse_ws_message(input) {
            WsInput::Cancel => {}
            other => panic!("Expected Cancel, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_hint() {
        let input = r#"{"type":"hint","content":"additional context","client_id":"456"}"#;
        match parse_ws_message(input) {
            WsInput::Hint { content, client_id } => {
                assert_eq!(content, "additional context");
                assert_eq!(client_id, Some("456".to_string()));
            }
            other => panic!("Expected Hint, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_hint_defaults() {
        let input = r#"{"type":"hint"}"#;
        match parse_ws_message(input) {
            WsInput::Hint { content, client_id } => {
                assert_eq!(content, "");
                assert_eq!(client_id, None);
            }
            other => panic!("Expected Hint, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_unknown_type() {
        let input = r#"{"type":"foobar"}"#;
        match parse_ws_message(input) {
            WsInput::Unknown(t) => assert_eq!(t, "foobar"),
            other => panic!("Expected Unknown, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_missing_type() {
        let input = r#"{"data":"hello"}"#;
        match parse_ws_message(input) {
            WsInput::Unknown(t) => assert_eq!(t, ""),
            other => panic!("Expected Unknown, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_invalid_json() {
        let input = "not json at all";
        match parse_ws_message(input) {
            WsInput::InvalidJson => {}
            other => panic!("Expected InvalidJson, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_empty_object() {
        let input = r#"{}"#;
        match parse_ws_message(input) {
            WsInput::Unknown(t) => assert_eq!(t, ""),
            other => panic!("Expected Unknown, got {:?}", other),
        }
    }

    // ---- Response builder tests ----

    #[test]
    fn test_build_processing_start() {
        let result = build_processing_start();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["type"], "processing_start");
    }

    #[test]
    fn test_build_processing_complete() {
        let result = build_processing_complete(true, Some("done"), None);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["type"], "processing_complete");
        assert_eq!(parsed["data"]["is_slash_response"], true);
        assert_eq!(parsed["data"]["content"], "done");
    }

    #[test]
    fn test_build_llm_chunk() {
        let result = build_llm_chunk("hello world");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["type"], "llm_chunk");
        assert_eq!(parsed["data"]["content"], "hello world");
    }

    #[test]
    fn test_build_llm_complete() {
        let result = build_llm_complete();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["type"], "llm_complete");
    }

    #[test]
    fn test_build_error() {
        let result = build_error("something went wrong");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["type"], "error");
        assert_eq!(parsed["data"]["content"], "something went wrong");
    }

    #[test]
    fn test_build_status_update() {
        let result = build_status_update("/home/user", "rxx", "localhost");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["type"], "status_update");
        assert_eq!(parsed["data"]["cwd"], "/home/user");
        assert_eq!(parsed["data"]["user"], "rxx");
        assert_eq!(parsed["data"]["hostname"], "localhost");
    }

    // ── Slash command detection tests ──────────────────────────────────────

    #[test]
    fn test_is_slash_command_basic() {
        assert!(is_slash_command("/session"));
        assert!(is_slash_command("/help"));
        assert!(is_slash_command("/clear"));
        assert!(is_slash_command("/dir /tmp"));
    }

    #[test]
    fn test_is_slash_command_with_whitespace() {
        assert!(is_slash_command("  /session"));
        assert!(is_slash_command("\t/help"));
        assert!(is_slash_command("\n/clear"));
        assert!(is_slash_command("  /dir /tmp  "));
    }

    #[test]
    fn test_is_slash_command_negative() {
        assert!(!is_slash_command("hello"));
        assert!(!is_slash_command("hello /world"));
        assert!(!is_slash_command(""));
        assert!(!is_slash_command("   "));
    }

    #[test]
    fn test_is_slash_command_bare_slash() {
        // A bare "/" is detected as a slash command (will be "unknown command" in Helen)
        assert!(is_slash_command("/"));
    }

    #[test]
    fn test_is_slash_command_edge_cases() {
        // Full-width slash (Chinese input) should NOT be detected as slash command
        // (normalization happens in Helen's parse_command, not here)
        assert!(!is_slash_command("／session"));
        // Unicode slash
        assert!(!is_slash_command("⁄session"));
    }

    // ── Protocol marker stripping tests ────────────────────────────────────

    #[test]
    fn test_strip_protocol_markers_clear_ok() {
        let input = "Session cleared successfully\n__HELEN_CLEAR_OK__";
        let result = strip_protocol_markers(input);
        assert_eq!(result, Some("Session cleared successfully".to_string()));
    }

    #[test]
    fn test_strip_protocol_markers_detailed_clear_ok() {
        let input = "Session cleared successfully\n__HELEN_CLEAR_OK__";
        let result = strip_protocol_markers_detailed(input);
        assert_eq!(
            result.content,
            Some("Session cleared successfully".to_string())
        );
        assert!(
            result.should_clear_messages,
            "should_clear_messages must be true for __HELEN_CLEAR_OK__"
        );
        assert!(!result.should_clear_session);
    }

    #[test]
    fn test_strip_protocol_markers_clear_session_ok() {
        let input = "Session deleted\n__HELEN_CLEAR_SESSION_OK__";
        let result = strip_protocol_markers(input);
        assert_eq!(result, Some("Session deleted".to_string()));
    }

    #[test]
    fn test_strip_protocol_markers_detailed_clear_session_ok() {
        let input = "Session deleted\n__HELEN_CLEAR_SESSION_OK__";
        let result = strip_protocol_markers_detailed(input);
        assert_eq!(result.content, Some("Session deleted".to_string()));
        assert!(
            result.should_clear_messages,
            "session deletion implies clearing messages"
        );
        assert!(
            result.should_clear_session,
            "should_clear_session must be true for __HELEN_CLEAR_SESSION_OK__"
        );
    }

    #[test]
    fn test_strip_protocol_markers_detailed_no_markers() {
        let input = "Hello world";
        let result = strip_protocol_markers_detailed(input);
        assert_eq!(result.content, Some("Hello world".to_string()));
        assert!(!result.should_clear_messages);
        assert!(!result.should_clear_session);
    }

    #[test]
    fn test_strip_protocol_markers_restart_actor() {
        let input = "Restarting actor\n__HELEN_RESTART_ACTOR__";
        let result = strip_protocol_markers(input);
        assert_eq!(result, Some("Restarting actor".to_string()));
    }

    #[test]
    fn test_strip_protocol_markers_multiple() {
        let input = "Done\n__HELEN_CLEAR_OK__\n__HELEN_RESTART_ACTOR__";
        let result = strip_protocol_markers(input);
        assert_eq!(result, Some("Done".to_string()));
    }

    #[test]
    fn test_strip_protocol_markers_empty_after_strip() {
        let input = "__HELEN_CLEAR_OK__";
        let result = strip_protocol_markers(input);
        assert_eq!(result, None);
    }

    #[test]
    fn test_strip_protocol_markers_only_whitespace_after_strip() {
        let input = "  \n__HELEN_CLEAR_OK__\n  ";
        let result = strip_protocol_markers(input);
        assert_eq!(result, None);
    }

    #[test]
    fn test_strip_protocol_markers_no_markers() {
        let input = "## Session Info\n\n- Session ID: abc123";
        let result = strip_protocol_markers(input);
        assert_eq!(
            result,
            Some("## Session Info\n\n- Session ID: abc123".to_string())
        );
    }

    #[test]
    fn test_strip_protocol_markers_preserves_content() {
        let input = "## Help\n\n| Command | Description |\n|---------|-------------|\n| /help | Show help |\n__HELEN_CLEAR_OK__";
        let result = strip_protocol_markers(input);
        assert!(result.is_some());
        let content = result.unwrap();
        assert!(content.contains("## Help"));
        assert!(content.contains("/help"));
        assert!(!content.contains("__HELEN_CLEAR_OK__"));
    }

    // ── Integration: slash command response building ───────────────────────

    #[test]
    fn test_build_processing_complete_slash_response() {
        let result = build_processing_complete(true, Some("## Session Info\n\n- ID: abc"), None);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["type"], "processing_complete");
        assert_eq!(parsed["data"]["is_slash_response"], true);
        assert_eq!(parsed["data"]["content"], "## Session Info\n\n- ID: abc");
    }

    #[test]
    fn test_build_processing_complete_normal_response() {
        let result = build_processing_complete(false, None, None);
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["type"], "processing_complete");
        assert_eq!(parsed["data"]["is_slash_response"], false);
        assert!(parsed["data"].get("content").is_none());
    }

    #[test]
    fn test_build_processing_complete_with_i18n() {
        let result = build_processing_complete(true, None, Some("dir.currentDir"));
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["type"], "processing_complete");
        assert_eq!(parsed["data"]["is_slash_response"], true);
        assert_eq!(parsed["data"]["i18n_key"], "dir.currentDir");
    }
}
