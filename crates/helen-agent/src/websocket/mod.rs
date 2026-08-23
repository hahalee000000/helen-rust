//! WebSocket handler for real-time chat

use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Router,
};

/// Create WebSocket router
pub fn router() -> Router {
    Router::new().route("/ws", get(ws_handler))
}

/// WebSocket upgrade handler
async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

/// Handle WebSocket connection
async fn handle_socket(mut socket: WebSocket) {
    while let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {
            match msg {
                Message::Text(text) => {
                    // Parse incoming message
                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) {
                        // Return proper JSON response
                        let response = serde_json::json!({
                            "type": "response",
                            "message": format!("Echo: {}", data.get("message").and_then(|m| m.as_str()).unwrap_or(&text)),
                            "agent": data.get("agent").and_then(|a| a.as_str()).unwrap_or("default")
                        });
                        if socket.send(Message::Text(response.to_string())).await.is_err() {
                            break;
                        }
                    } else {
                        // Invalid JSON, echo as-is
                        let response = serde_json::json!({
                            "type": "response",
                            "message": format!("Echo: {}", text)
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
