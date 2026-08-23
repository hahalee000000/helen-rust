//! WebSocket handler for real-time chat

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

use crate::api::sessions::AppState;
use crate::helen_bridge::StreamMessage;

/// Create WebSocket router
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
    Send {
        input: String,
        session_id: String,
    },
    Stop {
        stream_id: String,
    },
    Unknown(String),
    InvalidJson,
}

/// Parse a raw WebSocket text message into a structured WsInput
pub fn parse_ws_message(text: &str) -> WsInput {
    let data = match serde_json::from_str::<serde_json::Value>(text) {
        Ok(d) => d,
        Err(_) => return WsInput::InvalidJson,
    };

    let msg_type = data
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("");

    match msg_type {
        "send" => {
            let input = data
                .get("input")
                .and_then(|i| i.as_str())
                .unwrap_or("")
                .to_string();
            let session_id = data
                .get("session_id")
                .and_then(|s| s.as_str())
                .unwrap_or("default")
                .to_string();
            WsInput::Send { input, session_id }
        }
        "stop" => {
            let stream_id = data
                .get("stream_id")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            WsInput::Stop { stream_id }
        }
        other => WsInput::Unknown(other.to_string()),
    }
}

/// Build a JSON response string for stream_started
pub fn build_stream_started(stream_id: &str) -> String {
    json!({
        "type": "stream_started",
        "stream_id": stream_id
    })
    .to_string()
}

/// Build a JSON response string for output
pub fn build_output(stream_id: &str, data: &str) -> String {
    json!({
        "type": "output",
        "stream_id": stream_id,
        "data": data
    })
    .to_string()
}

/// Build a JSON response string for stream_complete
pub fn build_stream_complete(stream_id: &str) -> String {
    json!({
        "type": "stream_complete",
        "stream_id": stream_id
    })
    .to_string()
}

/// Build a JSON response string for error
pub fn build_error(data: &str) -> String {
    json!({
        "type": "error",
        "data": data
    })
    .to_string()
}

/// Build a JSON response string for stream_error
pub fn build_stream_error(stream_id: &str, data: &str) -> String {
    json!({
        "type": "error",
        "stream_id": stream_id,
        "data": data
    })
    .to_string()
}

/// Build a JSON response string for stream_stopped
pub fn build_stream_stopped(stream_id: &str) -> String {
    json!({
        "type": "stream_stopped",
        "stream_id": stream_id
    })
    .to_string()
}

/// Build a JSON response string for unknown message type error
pub fn build_unknown_type_error(msg_type: &str) -> String {
    build_error(&format!("Unknown message type: {}", msg_type))
}

/// Handle WebSocket connection
async fn handle_socket(mut socket: WebSocket, state: AppState) {
    while let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {
            match msg {
                Message::Text(text) => {
                    match parse_ws_message(&text) {
                        WsInput::Send { input, session_id } => {
                            // Get state
                            let state_guard = state.lock().await;
                            let cwd = state_guard.directory_manager.get_cwd();
                            let helen_bridge = state_guard.helen_bridge.clone();
                            let stream_registry = state_guard.stream_registry.clone();
                            drop(state_guard);

                            // Start stream
                            match helen_bridge.start_stream(&session_id, &input, &cwd).await {
                                Ok((stream_id, mut receiver)) => {
                                    // Register as actively streaming
                                    stream_registry.register(&session_id);

                                    if socket
                                        .send(Message::Text(build_stream_started(&stream_id)))
                                        .await
                                        .is_err()
                                    {
                                        stream_registry.unregister(&session_id);
                                        break;
                                    }

                                    // Stream output
                                    loop {
                                        match receiver.recv().await {
                                            Ok(StreamMessage::Output(line)) => {
                                                if socket
                                                    .send(Message::Text(build_output(
                                                        &stream_id, &line,
                                                    )))
                                                    .await
                                                    .is_err()
                                                {
                                                    break;
                                                }
                                            }
                                            Ok(StreamMessage::Complete) => {
                                                let _ = socket
                                                    .send(Message::Text(build_stream_complete(
                                                        &stream_id,
                                                    )))
                                                    .await;
                                                break;
                                            }
                                            Ok(StreamMessage::Error(err)) => {
                                                if socket
                                                    .send(Message::Text(build_stream_error(
                                                        &stream_id, &err,
                                                    )))
                                                    .await
                                                    .is_err()
                                                {
                                                    break;
                                                }
                                            }
                                            Err(_) => break,
                                        }
                                    }

                                    // Unregister after stream ends
                                    stream_registry.unregister(&session_id);
                                }
                                Err(err) => {
                                    if socket
                                        .send(Message::Text(build_error(&err)))
                                        .await
                                        .is_err()
                                    {
                                        break;
                                    }
                                }
                            }
                        }
                        WsInput::Stop { stream_id } => {
                            let state_guard = state.lock().await;
                            let helen_bridge = state_guard.helen_bridge.clone();
                            drop(state_guard);

                            match helen_bridge.stop_stream(&stream_id).await {
                                Ok(_) => {
                                    if socket
                                        .send(Message::Text(build_stream_stopped(&stream_id)))
                                        .await
                                        .is_err()
                                    {
                                        break;
                                    }
                                }
                                Err(err) => {
                                    if socket
                                        .send(Message::Text(build_error(&err)))
                                        .await
                                        .is_err()
                                    {
                                        break;
                                    }
                                }
                            }
                        }
                        WsInput::Unknown(msg_type) => {
                            if socket
                                .send(Message::Text(build_unknown_type_error(&msg_type)))
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
    fn test_parse_send_message() {
        let input = r#"{"type":"send","input":"Hello","session_id":"sess1"}"#;
        match parse_ws_message(input) {
            WsInput::Send { input, session_id } => {
                assert_eq!(input, "Hello");
                assert_eq!(session_id, "sess1");
            }
            other => panic!("Expected Send, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_send_message_defaults() {
        let input = r#"{"type":"send"}"#;
        match parse_ws_message(input) {
            WsInput::Send { input, session_id } => {
                assert_eq!(input, "");
                assert_eq!(session_id, "default");
            }
            other => panic!("Expected Send, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_stop_message() {
        let input = r#"{"type":"stop","stream_id":"abc-123"}"#;
        match parse_ws_message(input) {
            WsInput::Stop { stream_id } => {
                assert_eq!(stream_id, "abc-123");
            }
            other => panic!("Expected Stop, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_stop_message_default() {
        let input = r#"{"type":"stop"}"#;
        match parse_ws_message(input) {
            WsInput::Stop { stream_id } => {
                assert_eq!(stream_id, "");
            }
            other => panic!("Expected Stop, got {:?}", other),
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
    fn test_build_stream_started() {
        let result = build_stream_started("stream-1");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["type"], "stream_started");
        assert_eq!(parsed["stream_id"], "stream-1");
    }

    #[test]
    fn test_build_output() {
        let result = build_output("stream-1", "hello world");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["type"], "output");
        assert_eq!(parsed["stream_id"], "stream-1");
        assert_eq!(parsed["data"], "hello world");
    }

    #[test]
    fn test_build_stream_complete() {
        let result = build_stream_complete("stream-1");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["type"], "stream_complete");
        assert_eq!(parsed["stream_id"], "stream-1");
    }

    #[test]
    fn test_build_error() {
        let result = build_error("something went wrong");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["type"], "error");
        assert_eq!(parsed["data"], "something went wrong");
    }

    #[test]
    fn test_build_stream_error() {
        let result = build_stream_error("stream-1", "timeout");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["type"], "error");
        assert_eq!(parsed["stream_id"], "stream-1");
        assert_eq!(parsed["data"], "timeout");
    }

    #[test]
    fn test_build_stream_stopped() {
        let result = build_stream_stopped("stream-1");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["type"], "stream_stopped");
        assert_eq!(parsed["stream_id"], "stream-1");
    }

    #[test]
    fn test_build_unknown_type_error() {
        let result = build_unknown_type_error("foobar");
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["type"], "error");
        assert!(parsed["data"].as_str().unwrap().contains("foobar"));
    }
}
