//! Tests for frontend embedding

#[tokio::test]
async fn test_serves_index_html() {
    let server = helen_agent::server::start_server("127.0.0.1:0").await.unwrap();
    let port = server.local_addr().port();

    let resp = reqwest::get(format!("http://127.0.0.1:{}/", port))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body = resp.text().await.unwrap();
    // Should contain HTML content
    assert!(body.contains("<!DOCTYPE html>") || body.contains("<html"));

    server.shutdown().await;
}
