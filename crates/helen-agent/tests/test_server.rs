//! Tests for the Helen Agent web server

#[tokio::test]
async fn test_server_starts_and_responds_to_health() {
    let server = helen_agent::server::start_server("127.0.0.1:0")
        .await
        .unwrap();
    let port = server.local_addr().port();

    let resp = reqwest::get(format!("http://127.0.0.1:{}/health", port))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");

    server.shutdown().await;
}
