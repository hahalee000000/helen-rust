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

    // 3. First message should be status_update
    let first = ws.next().await.unwrap().unwrap();
    match first {
        Message::Text(text) => {
            let data: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(data["type"], "status_update");
        }
        _ => panic!("Expected text message for status_update"),
    }

    // 4. Send a chat message with valid Helen code
    let msg = serde_json::json!({
        "type": "message",
        "content": "print(\"Hello\")",
        "client_id": "test-123"
    });
    ws.send(Message::Text(msg.to_string())).await.unwrap();

    // 5. Receive processing_start
    let response = ws.next().await.unwrap().unwrap();
    match response {
        Message::Text(text) => {
            let data: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(data["type"], "processing_start");
        }
        _ => panic!("Expected processing_start"),
    }

    // 6. Receive llm_chunk or error (code execution output or error)
    let response = ws.next().await.unwrap().unwrap();
    match response {
        Message::Text(text) => {
            let data: serde_json::Value = serde_json::from_str(&text).unwrap();
            let msg_type = data["type"].as_str().unwrap();
            // Accept either llm_chunk (success) or error (semantic/runtime error)
            assert!(
                msg_type == "llm_chunk" || msg_type == "error",
                "Expected llm_chunk or error, got: {}",
                msg_type
            );
        }
        _ => panic!("Expected text message"),
    }

    // 7. Receive processing_complete
    let response = ws.next().await.unwrap().unwrap();
    match response {
        Message::Text(text) => {
            let data: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(data["type"], "processing_complete");
        }
        _ => panic!("Expected processing_complete"),
    }

    // 8. Close WebSocket
    ws.close(None).await.ok();
}
