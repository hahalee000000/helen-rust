//! Tests for REST API endpoints

#[tokio::test]
async fn test_get_chat_status() {
    let server = helen_agent::server::start_server("127.0.0.1:0")
        .await
        .unwrap();
    let port = server.local_addr().port();

    let resp = reqwest::get(format!("http://127.0.0.1:{}/api/chat/status", port))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("is_processing").is_some());

    server.shutdown().await;
}
