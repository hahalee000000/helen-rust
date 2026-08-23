//! Integration tests for the complete agent workflow

use futures::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn test_full_agent_workflow() {
    // Start server
    let server = helen_agent::server::start_server("127.0.0.1:0")
        .await
        .unwrap();
    let addr = server.local_addr();
    let base_url = format!("http://{}", addr);

    // Give server time to fully start
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // 1. Check health
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/health", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // 2. Connect to WebSocket
    let ws_url = format!("ws://{}/api/chat/ws", addr);
    let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();

    // 4. Send a chat message
    let msg = serde_json::json!({
        "type": "send",
        "input": "Hello",
        "session_id": "test_session"
    });
    ws.send(Message::Text(msg.to_string())).await.unwrap();

    // 5. Receive response
    let response = ws.next().await.unwrap().unwrap();
    match response {
        Message::Text(text) => {
            let data: serde_json::Value = serde_json::from_str(&text).unwrap();
            // Should receive stream_started or error (if helen not found)
            assert!(data["type"] == "stream_started" || data["type"] == "error");
        }
        _ => panic!("Expected text message"),
    }

    // 6. Close WebSocket
    ws.close(None).await.ok();
}
