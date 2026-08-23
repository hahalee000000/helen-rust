//! Tests for file upload/download API

#[tokio::test]
async fn test_upload_file() {
    let server = helen_agent::server::start_server("127.0.0.1:0")
        .await
        .unwrap();
    let addr = server.local_addr();
    let base_url = format!("http://{}", addr);

    let client = reqwest::Client::new();
    let form = reqwest::multipart::Form::new()
        .text("filename", "test.txt")
        .text("content", "Hello, World!");

    let resp = client
        .post(format!("{}/api/files", base_url))
        .multipart(form)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert!(body["id"].is_string());
}

#[tokio::test]
async fn test_download_file() {
    let server = helen_agent::server::start_server("127.0.0.1:0")
        .await
        .unwrap();
    let addr = server.local_addr();
    let base_url = format!("http://{}", addr);

    let client = reqwest::Client::new();

    // Upload a file first
    let form = reqwest::multipart::Form::new()
        .text("filename", "test.txt")
        .text("content", "Test content");

    let resp = client
        .post(format!("{}/api/files", base_url))
        .multipart(form)
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    let file_id = body["id"].as_str().unwrap();

    // Download the file
    let resp = client
        .get(format!("{}/api/files/{}", base_url, file_id))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let content = resp.text().await.unwrap();
    assert_eq!(content, "Test content");
}

#[tokio::test]
async fn test_list_files_api() {
    let server = helen_agent::server::start_server("127.0.0.1:0")
        .await
        .unwrap();
    let addr = server.local_addr();
    let base_url = format!("http://{}", addr);

    let client = reqwest::Client::new();

    // Upload two files
    let form1 = reqwest::multipart::Form::new()
        .text("filename", "file1.txt")
        .text("content", "content1");
    client
        .post(format!("{}/api/files", base_url))
        .multipart(form1)
        .send()
        .await
        .unwrap();

    let form2 = reqwest::multipart::Form::new()
        .text("filename", "file2.txt")
        .text("content", "content2");
    client
        .post(format!("{}/api/files", base_url))
        .multipart(form2)
        .send()
        .await
        .unwrap();

    // List files
    let resp = client
        .get(format!("{}/api/files", base_url))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Vec<serde_json::Value> = resp.json().await.unwrap();
    assert!(
        body.len() >= 2,
        "Expected at least 2 files, got {}",
        body.len()
    );
}

#[tokio::test]
async fn test_delete_file_api() {
    let server = helen_agent::server::start_server("127.0.0.1:0")
        .await
        .unwrap();
    let addr = server.local_addr();
    let base_url = format!("http://{}", addr);

    let client = reqwest::Client::new();

    // Upload a file
    let form = reqwest::multipart::Form::new()
        .text("filename", "test.txt")
        .text("content", "content");
    let resp = client
        .post(format!("{}/api/files", base_url))
        .multipart(form)
        .send()
        .await
        .unwrap();
    let body: serde_json::Value = resp.json().await.unwrap();
    let file_id = body["id"].as_str().unwrap();

    // Delete the file
    let resp = client
        .delete(format!("{}/api/files/{}", base_url, file_id))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);

    // Verify it's gone
    let resp = client
        .get(format!("{}/api/files/{}", base_url, file_id))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 404);
}
