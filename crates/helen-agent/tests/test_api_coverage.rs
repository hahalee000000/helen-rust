//! Comprehensive API coverage tests
//!
//! Starts a real server and exercises every endpoint to maximize code coverage.
//! Covers: chat API, server routes, upload API, session management.

/// Helper: start a test server and return (base_url, temp_dir_handle)
async fn start_test_server() -> (String, tempfile::TempDir) {
    let temp = tempfile::TempDir::new().unwrap();

    // Create .helen/sessions structure
    let sessions_dir = temp.path().join(".helen").join("sessions");
    std::fs::create_dir_all(&sessions_dir).unwrap();

    // Create .helen/uploads structure
    let uploads_dir = temp.path().join(".helen").join("uploads");
    std::fs::create_dir_all(&uploads_dir).unwrap();

    // Create a test session with transcript
    let test_session_dir = sessions_dir.join("session_test_001");
    std::fs::create_dir_all(&test_session_dir).unwrap();
    let transcript = r#"{"type":"session_meta","session_id":"session_test_001","timestamp":"2026-08-23T10:00:00Z","cwd":"/tmp/test"}
{"type":"message","role":"user","content":"Hello world","timestamp":"2026-08-23T10:00:01Z"}
{"type":"message","role":"assistant","content":"Hi there!","timestamp":"2026-08-23T10:00:02Z"}
"#;
    std::fs::write(test_session_dir.join("transcript.jsonl"), transcript).unwrap();

    // Start server
    let server = helen_agent::server::start_server("127.0.0.1:0")
        .await
        .unwrap();
    let addr = server.local_addr();
    let base_url = format!("http://{}", addr);

    // Give server time to start
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Leak the server handle so it stays alive for the test
    std::mem::forget(server);

    (base_url, temp)
}

// ============================================================
// Chat API: GET /api/chat/status
// ============================================================

#[tokio::test]
async fn test_chat_status() {
    let (base_url, _temp) = start_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/api/chat/status", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("is_processing").is_some());
    assert!(body.get("version").is_some());
}

// ============================================================
// Chat API: GET /api/chat/cwd
// ============================================================

#[tokio::test]
async fn test_chat_get_cwd() {
    let (base_url, _temp) = start_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/api/chat/cwd", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("cwd").is_some());
    assert!(body.get("display_name").is_some());
}

// ============================================================
// Chat API: POST /api/chat/cwd
// ============================================================

#[tokio::test]
async fn test_chat_set_cwd_valid() {
    let (base_url, _temp) = start_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/chat/cwd", base_url))
        .json(&serde_json::json!({"cwd": "/tmp"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn test_chat_set_cwd_nonexistent() {
    let (base_url, _temp) = start_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{}/api/chat/cwd", base_url))
        .json(&serde_json::json!({"cwd": "/nonexistent/path/xyz"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    // Should return error status
    assert_eq!(body["status"], "error");
}

// ============================================================
// Chat API: GET /api/chat/sessions
// ============================================================

#[tokio::test]
async fn test_chat_list_sessions() {
    let (base_url, _temp) = start_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/api/chat/sessions", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("sessions").is_some());
    assert!(body["sessions"].is_array());
}

// ============================================================
// Chat API: GET /api/chat/sessions/:id/messages
// ============================================================

#[tokio::test]
async fn test_chat_get_messages_existing_session() {
    let (base_url, temp) = start_test_server().await;
    let client = reqwest::Client::new();

    // Set CWD to temp dir so the server can see our test sessions
    let resp = client
        .post(format!("{}/api/chat/cwd", base_url))
        .json(&serde_json::json!({"cwd": temp.path().to_string_lossy()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let resp = client
        .get(format!(
            "{}/api/chat/sessions/session_test_001/messages",
            base_url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("messages").is_some());
    let messages = body["messages"].as_array().unwrap();
    assert!(!messages.is_empty());
}

#[tokio::test]
async fn test_chat_get_messages_nonexistent_session() {
    let (base_url, _temp) = start_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!(
            "{}/api/chat/sessions/nonexistent_session/messages",
            base_url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let messages = body["messages"].as_array().unwrap();
    assert!(messages.is_empty());
}

// ============================================================
// Chat API: DELETE /api/chat/sessions/:id
// ============================================================

#[tokio::test]
async fn test_chat_delete_session_success() {
    let (base_url, temp) = start_test_server().await;
    let client = reqwest::Client::new();

    // Set CWD to temp dir so the server can see our test sessions
    let resp = client
        .post(format!("{}/api/chat/cwd", base_url))
        .json(&serde_json::json!({"cwd": temp.path().to_string_lossy()}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // Create a session to delete
    let session_dir = temp
        .path()
        .join(".helen")
        .join("sessions")
        .join("session_to_delete");
    std::fs::create_dir_all(&session_dir).unwrap();
    std::fs::write(
        session_dir.join("transcript.jsonl"),
        r#"{"type":"session_meta","session_id":"session_to_delete"}"#,
    )
    .unwrap();

    let resp = client
        .delete(format!("{}/api/chat/sessions/session_to_delete", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");

    // Verify it's gone
    assert!(!session_dir.exists());
}

#[tokio::test]
async fn test_chat_delete_session_not_found() {
    let (base_url, _temp) = start_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .delete(format!(
            "{}/api/chat/sessions/nonexistent_session",
            base_url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_chat_delete_session_path_traversal() {
    let (base_url, _temp) = start_test_server().await;
    let client = reqwest::Client::new();

    // Test path traversal with ../
    let resp = client
        .delete(format!("{}/api/chat/sessions/..%2F..%2Fetc", base_url))
        .send()
        .await
        .unwrap();
    // Should be rejected (either 400 from our validation or 404 from routing)
    assert!(resp.status() == 400 || resp.status() == 404);

    // Test with explicit .. in the ID
    let resp = client
        .delete(format!("{}/api/chat/sessions/..", base_url))
        .send()
        .await
        .unwrap();
    assert!(resp.status() == 400 || resp.status() == 404);
}

// ============================================================
// Chat API: POST /api/chat/upload
// ============================================================

#[tokio::test]
async fn test_chat_upload_file_success() {
    let (base_url, _temp) = start_test_server().await;
    let client = reqwest::Client::new();

    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(b"hello world".to_vec())
            .file_name("test.txt")
            .mime_str("text/plain")
            .unwrap(),
    );

    let resp = client
        .post(format!("{}/api/chat/upload", base_url))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body.get("upload_id").is_some());
    assert!(body.get("filename").is_some());
}

#[tokio::test]
async fn test_chat_upload_unsupported_mime() {
    let (base_url, _temp) = start_test_server().await;
    let client = reqwest::Client::new();

    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(b"fake exe".to_vec())
            .file_name("malware.exe")
            .mime_str("application/x-executable")
            .unwrap(),
    );

    let resp = client
        .post(format!("{}/api/chat/upload", base_url))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

// ============================================================
// Chat API: GET /api/chat/uploads/:id/file
// ============================================================

#[tokio::test]
async fn test_chat_get_upload_file_invalid_id() {
    let (base_url, _temp) = start_test_server().await;
    let client = reqwest::Client::new();

    // "invalid-id" passes validation (no /, \\, ..) but file doesn't exist → 404
    let resp = client
        .get(format!("{}/api/chat/uploads/invalid-id/file", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_chat_get_upload_file_path_traversal() {
    let (base_url, _temp) = start_test_server().await;
    let client = reqwest::Client::new();

    // Path traversal should be rejected with 400
    let resp = client
        .get(format!("{}/api/chat/uploads/..%2F..%2Fetc/file", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 400);
}

#[tokio::test]
async fn test_chat_get_upload_file_not_found() {
    let (base_url, _temp) = start_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!(
            "{}/api/chat/uploads/00000000-0000-0000-0000-000000000000/file",
            base_url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_chat_upload_then_download() {
    let (base_url, _temp) = start_test_server().await;
    let client = reqwest::Client::new();

    // Upload
    let content = b"test file content for download";
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(content.to_vec())
            .file_name("download_test.txt")
            .mime_str("text/plain")
            .unwrap(),
    );

    let resp = client
        .post(format!("{}/api/chat/upload", base_url))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    let upload_id = body["upload_id"].as_str().unwrap();

    // Download
    let resp = client
        .get(format!(
            "{}/api/chat/uploads/{}/file",
            base_url, upload_id
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(
        resp.headers().get("content-type").unwrap().to_str().unwrap(),
        "text/plain"
    );
    let downloaded = resp.bytes().await.unwrap();
    assert_eq!(downloaded.as_ref(), content);
}

// ============================================================
// Server: health, SPA fallback, static assets
// ============================================================

#[tokio::test]
async fn test_health_endpoint() {
    let (base_url, _temp) = start_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/health", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn test_spa_fallback_non_api_route() {
    let (base_url, _temp) = start_test_server().await;
    let client = reqwest::Client::new();

    // Non-API route should return HTML (SPA fallback)
    let resp = client
        .get(format!("{}/some/random/page", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    let content_type = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert!(content_type.contains("text/html"));
}

#[tokio::test]
async fn test_api_404_for_unknown_api_route() {
    let (base_url, _temp) = start_test_server().await;
    let client = reqwest::Client::new();

    // Unknown API route should return 404
    let resp = client
        .get(format!("{}/api/nonexistent", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}

#[tokio::test]
async fn test_static_asset_not_found() {
    let (base_url, _temp) = start_test_server().await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("{}/assets/nonexistent.js", base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);
}
