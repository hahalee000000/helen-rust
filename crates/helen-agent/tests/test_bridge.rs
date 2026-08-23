//! Tests for Python bridge validation mode

#[tokio::test]
async fn test_bridge_validation_endpoint() {
    // Start server with bridge validation enabled
    let server = helen_agent::server::start_server_with_bridge(
        "127.0.0.1:0",
        None,
        true, // Enable bridge validation
    ).await.unwrap();
    let addr = server.local_addr();
    let base_url = format!("http://{}", addr);
    
    let client = reqwest::Client::new();
    
    // Test bridge validation endpoint
    let resp = client.post(&format!("{}/api/bridge/validate", base_url))
        .json(&serde_json::json!({
            "code": "import std.core.*\nmain { print(\"Hello from bridge!\") }"
        }))
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["success"].as_bool().unwrap());
    assert!(body["output"].as_str().unwrap().contains("Hello from bridge!"));
}

#[tokio::test]
async fn test_bridge_validation_with_error() {
    let server = helen_agent::server::start_server_with_bridge(
        "127.0.0.1:0",
        None,
        true,
    ).await.unwrap();
    let addr = server.local_addr();
    let base_url = format!("http://{}", addr);
    
    let client = reqwest::Client::new();
    
    // Test with invalid code
    let resp = client.post(&format!("{}/api/bridge/validate", base_url))
        .json(&serde_json::json!({
            "code": "import std.core.*\nmain { invalid syntax here }"
        }))
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(!body["success"].as_bool().unwrap());
    assert!(body["error"].is_string());
}

#[tokio::test]
async fn test_bridge_disabled_by_default() {
    let server = helen_agent::server::start_server_with_bridge(
        "127.0.0.1:0",
        None,
        false, // Bridge disabled
    ).await.unwrap();
    let addr = server.local_addr();
    let base_url = format!("http://{}", addr);
    
    let client = reqwest::Client::new();
    
    // Bridge endpoint should return 404 when disabled
    let resp = client.post(&format!("{}/api/bridge/validate", base_url))
        .json(&serde_json::json!({
            "code": "import std.core.*\nmain { print(\"test\") }"
        }))
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 404);
}
