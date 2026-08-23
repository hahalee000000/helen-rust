//! Tests for session API endpoints

#[tokio::test]
async fn test_create_session_api() {
    let server = helen_agent::server::start_server("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr();
    let base_url = format!("http://{}", addr);
    
    let client = reqwest::Client::new();
    let resp = client.post(&format!("{}/api/sessions", base_url))
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["id"].is_string());
}

#[tokio::test]
async fn test_list_sessions_api() {
    let server = helen_agent::server::start_server("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr();
    let base_url = format!("http://{}", addr);
    
    let client = reqwest::Client::new();
    
    // Create a session first
    client.post(&format!("{}/api/sessions", base_url))
        .send()
        .await
        .unwrap();
    
    // List sessions
    let resp = client.get(&format!("{}/api/sessions", base_url))
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 200);
    let body: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(!body.is_empty());
}

#[tokio::test]
async fn test_get_session_api() {
    let server = helen_agent::server::start_server("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr();
    let base_url = format!("http://{}", addr);
    
    let client = reqwest::Client::new();
    
    // Create a session
    let resp = client.post(&format!("{}/api/sessions", base_url))
        .send()
        .await
        .unwrap();
    let session: serde_json::Value = resp.json().await.unwrap();
    let session_id = session["id"].as_str().unwrap();
    
    // Get the session
    let resp = client.get(&format!("{}/api/sessions/{}", base_url, session_id))
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["id"], session_id);
}

#[tokio::test]
async fn test_delete_session_api() {
    let server = helen_agent::server::start_server("127.0.0.1:0").await.unwrap();
    let addr = server.local_addr();
    let base_url = format!("http://{}", addr);
    
    let client = reqwest::Client::new();
    
    // Create a session
    let resp = client.post(&format!("{}/api/sessions", base_url))
        .send()
        .await
        .unwrap();
    let session: serde_json::Value = resp.json().await.unwrap();
    let session_id = session["id"].as_str().unwrap();
    
    // Delete the session
    let resp = client.delete(&format!("{}/api/sessions/{}", base_url, session_id))
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 200);
    
    // Verify it's gone
    let resp = client.get(&format!("{}/api/sessions/{}", base_url, session_id))
        .send()
        .await
        .unwrap();
    
    assert_eq!(resp.status(), 404);
}
