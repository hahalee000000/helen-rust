//! Tests for WebSocket functionality

use futures::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn test_websocket_connects() {
    let server = helen_agent::server::start_server("127.0.0.1:0")
        .await
        .unwrap();
    let port = server.local_addr().port();

    let url = format!("ws://127.0.0.1:{}/api/chat/ws", port);
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();

    // Send a test message
    ws.send(Message::Text(r#"{"type":"ping"}"#.to_string()))
        .await
        .unwrap();

    // Should receive a response
    let msg = ws.next().await.unwrap().unwrap();
    assert!(msg.is_text());

    server.shutdown().await;
}
