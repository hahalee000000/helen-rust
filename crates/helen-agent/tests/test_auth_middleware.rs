//! Tests for auth middleware

#[tokio::test]
async fn test_auth_required_without_token() {
    // Start server with auth enabled
    let server = helen_agent::server::start_server_with_auth(
        "127.0.0.1:0",
        Some("test-token-123".to_string()),
    ).await.unwrap();
    let addr = server.local_addr();
    let base_url = format!("http://{}", addr);
    
    let client = reqwest::Client::new();
    
    // Try to access without token - should fail
    let resp = client.get(&format!("{}/api/sessions", base_url))
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_auth_with_valid_token() {
    let server = helen_agent::server::start_server_with_auth(
        "127.0.0.1:0",
        Some("test-token-123".to_string()),
    ).await.unwrap();
    let addr = server.local_addr();
    let base_url = format!("http://{}", addr);
    
    let client = reqwest::Client::new();
    
    // Access with valid token - should succeed
    let resp = client.get(&format!("{}/api/sessions", base_url))
        .header("Authorization", "Bearer test-token-123")
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_auth_with_invalid_token() {
    let server = helen_agent::server::start_server_with_auth(
        "127.0.0.1:0",
        Some("test-token-123".to_string()),
    ).await.unwrap();
    let addr = server.local_addr();
    let base_url = format!("http://{}", addr);
    
    let client = reqwest::Client::new();
    
    // Access with invalid token - should fail
    let resp = client.get(&format!("{}/api/sessions", base_url))
        .header("Authorization", "Bearer wrong-token")
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn test_auth_disabled() {
    let server = helen_agent::server::start_server_with_auth(
        "127.0.0.1:0",
        None, // Auth disabled
    ).await.unwrap();
    let addr = server.local_addr();
    let base_url = format!("http://{}", addr);
    
    let client = reqwest::Client::new();
    
    // Access without token when auth is disabled - should succeed
    let resp = client.get(&format!("{}/api/sessions", base_url))
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 200);
}
