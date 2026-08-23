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

    // 2. Get agents list
    let resp = client
        .get(format!("{}/api/agents/list", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let agents: serde_json::Value = resp.json().await.unwrap();
    assert!(agents.is_array());

    // 3. Connect to WebSocket
    let ws_url = format!("ws://{}/api/chat/ws", addr);
    let (mut ws, _) = tokio_tungstenite::connect_async(&ws_url).await.unwrap();

    // 4. Send a chat message
    let msg = serde_json::json!({
        "type": "chat",
        "message": "Hello",
        "agent": "chat_actor"
    });
    ws.send(Message::Text(msg.to_string())).await.unwrap();

    // 5. Receive response
    let response = ws.next().await.unwrap().unwrap();
    match response {
        Message::Text(text) => {
            let data: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(data["type"], "response");
            assert!(data["message"].is_string());
        }
        _ => panic!("Expected text message"),
    }

    // 6. Close WebSocket
    ws.close(None).await.ok();
}
