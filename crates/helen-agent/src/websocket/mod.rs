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

/// Handle WebSocket connection
async fn handle_socket(mut socket: WebSocket, state: AppState) {
    while let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {
            match msg {
                Message::Text(text) => {
                    // Parse incoming message
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) {
                        let msg_type = data.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        
                        match msg_type {
                            "send" => {
                                // Handle send message
                                let input = data.get("input").and_then(|i| i.as_str()).unwrap_or("");
                                let session_id = data.get("session_id").and_then(|s| s.as_str()).unwrap_or("default");
                                
                                // Get state
                                let state_guard = state.lock().await;
                                let cwd = state_guard.directory_manager.get_cwd();
                                let helen_bridge = state_guard.helen_bridge.clone();
                                drop(state_guard);
                                
                                // Start stream
                                match helen_bridge.start_stream(session_id, input, &cwd).await {
                                    Ok((stream_id, mut receiver)) => {
                                        // Send stream started
                                        let response = json!({
                                            "type": "stream_started",
                                            "stream_id": stream_id
                                        });
                                        if socket.send(Message::Text(response.to_string())).await.is_err() {
                                            break;
                                        }
                                        
                                        // Stream output
                                        loop {
                                            match receiver.recv().await {
                                                Ok(StreamMessage::Output(line)) => {
                                                    let response = json!({
                                                        "type": "output",
                                                        "stream_id": stream_id,
                                                        "data": line
                                                    });
                                                    if socket.send(Message::Text(response.to_string())).await.is_err() {
                                                        break;
                                                    }
                                                }
                                                Ok(StreamMessage::Complete) => {
                                                    let response = json!({
                                                        "type": "stream_complete",
                                                        "stream_id": stream_id
                                                    });
                                                    if socket.send(Message::Text(response.to_string())).await.is_err() {
                                                        break;
                                                    }
                                                    break;
                                                }
                                                Ok(StreamMessage::Error(err)) => {
                                                    let response = json!({
                                                        "type": "error",
                                                        "stream_id": stream_id,
                                                        "data": err
                                                    });
                                                    if socket.send(Message::Text(response.to_string())).await.is_err() {
                                                        break;
                                                    }
                                                }
                                                Err(_) => break,
                                            }
                                        }
                                    }
                                    Err(err) => {
                                        let response = json!({
                                            "type": "error",
                                            "data": err
                                        });
                                        if socket.send(Message::Text(response.to_string())).await.is_err() {
                                            break;
                                        }
                                    }
                                }
                            }
                            "stop" => {
                                // Handle stop stream
                                let stream_id = data.get("stream_id").and_then(|s| s.as_str()).unwrap_or("");
                                
                                let state_guard = state.lock().await;
                                let helen_bridge = state_guard.helen_bridge.clone();
                                drop(state_guard);
                                
                                match helen_bridge.stop_stream(stream_id).await {
                                    Ok(_) => {
                                        let response = json!({
                                            "type": "stream_stopped",
                                            "stream_id": stream_id
                                        });
                                        if socket.send(Message::Text(response.to_string())).await.is_err() {
                                            break;
                                        }
                                    }
                                    Err(err) => {
                                        let response = json!({
                                            "type": "error",
                                            "data": err
                                        });
                                        if socket.send(Message::Text(response.to_string())).await.is_err() {
                                            break;
                                        }
                                    }
                                }
                            }
                            _ => {
                                // Unknown message type
                                let response = json!({
                                    "type": "error",
                                    "data": format!("Unknown message type: {}", msg_type)
                                });
                                if socket.send(Message::Text(response.to_string())).await.is_err() {
                                    break;
                                }
                            }
                        }
                    } else {
                        // Invalid JSON
                        let response = json!({
                            "type": "error",
                            "data": "Invalid JSON"
                        });
                        if socket.send(Message::Text(response.to_string())).await.is_err() {
                            break;
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    }
}
