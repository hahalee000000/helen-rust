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
    Message { content: String, client_id: Option<String> },
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
pub fn build_processing_complete(is_slash: bool, content: Option<&str>, i18n_key: Option<&str>) -> String {
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
        let _ = socket.send(Message::Text(build_status_update(&cwd, &user, &hostname))).await;
    }

    // Create HelenActorBridge for this connection
    let bridge = Arc::new(Mutex::new(None::<HelenActorBridge>));

    while let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {
            match msg {
                Message::Text(text) => {
                    match parse_ws_message(&text) {
                        WsInput::Message { content, .. } => {
                            // Signal processing start
                            if socket.send(Message::Text(build_processing_start())).await.is_err() {
                                break;
                            }

                            // Get state
                            let state_guard = state.lock().await;
                            let cwd = state_guard.directory_manager.get_cwd();
                            drop(state_guard);

                            // Generate session ID (in production, this would come from auth/session management)
                            let session_id = format!("session-{}", uuid::Uuid::new_v4());

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

                            // Process message using HelenActorBridge
                            let bridge_guard = bridge.lock().await;
                            if let Some(bridge_ref) = bridge_guard.as_ref() {
                                // Send message to Helen
                                bridge_ref.send_message(content.clone(), vec![]).await;

                                // Wait for response with timeout
                                let mut response_received = false;
                                for _ in 0..50 {
                                    // 5 second timeout (50 * 100ms)
                                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                                    
                                    if let Some(output) = bridge_ref.receive_output().await {
                                        match output {
                                            AgentOutput::ResponseComplete { content: resp_content, .. } => {
                                                if socket.send(Message::Text(build_llm_chunk(&resp_content))).await.is_err() {
                                                    break;
                                                }
                                                response_received = true;
                                                break;
                                            }
                                            AgentOutput::Error { error_msg, .. } => {
                                                if socket.send(Message::Text(build_error(&error_msg))).await.is_err() {
                                                    break;
                                                }
                                                response_received = true;
                                                break;
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                
                                if !response_received {
                                    if socket.send(Message::Text(build_error("Response timeout"))).await.is_err() {
                                        break;
                                    }
                                }
                            } else {
                                if socket.send(Message::Text(build_error("Bridge not initialized"))).await.is_err() {
                                    break;
                                }
                            }

                            // Signal processing complete
                            if socket.send(Message::Text(build_processing_complete(false, None, None))).await.is_err() {
                                break;
                            }
                        }
                        WsInput::Cancel => {
                            // Cancel current processing (no-op for now)
                        }
                        WsInput::Unknown(msg_type) => {
                            if socket
                                .send(Message::Text(build_error(&format!("Unknown message type: {}", msg_type))))
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
                Message::Close(_) => break,
                _ => {}
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
}
