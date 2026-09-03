//! Tests for frontend embedding and static file serving

#[tokio::test]
async fn test_serves_index_html() {
    let server = helen_agent::server::start_server("127.0.0.1:0")
        .await
        .unwrap();
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

/// Regression test: static images must be served with correct MIME type,
/// not as HTML (bug: SPA fallback was returning index.html for all routes).
#[tokio::test]
async fn test_serves_favicon_with_correct_mime() {
    let server = helen_agent::server::start_server("127.0.0.1:0")
        .await
        .unwrap();
    let port = server.local_addr().port();

    let resp = reqwest::get(format!("http://127.0.0.1:{}/favicon.png", port))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let content_type = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(content_type, "image/png");
    // favicon.png should be > 1KB (not the 212-byte placeholder)
    let bytes = resp.bytes().await.unwrap();
    assert!(
        bytes.len() > 1000,
        "favicon.png too small: {} bytes",
        bytes.len()
    );

    server.shutdown().await;
}

/// Regression test: helen-logo-64.png must be served as PNG, not HTML.
#[tokio::test]
async fn test_serves_helen_logo_with_correct_mime() {
    let server = helen_agent::server::start_server("127.0.0.1:0")
        .await
        .unwrap();
    let port = server.local_addr().port();

    let resp = reqwest::get(format!("http://127.0.0.1:{}/helen-logo-64.png", port))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let content_type = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(content_type, "image/png");
    let bytes = resp.bytes().await.unwrap();
    assert!(
        bytes.len() > 1000,
        "helen-logo-64.png too small: {} bytes",
        bytes.len()
    );

    server.shutdown().await;
}

/// Regression test: SPA fallback must still work for client-side routing.
#[tokio::test]
async fn test_spa_fallback_for_unknown_routes() {
    let server = helen_agent::server::start_server("127.0.0.1:0")
        .await
        .unwrap();
    let port = server.local_addr().port();

    // Unknown routes should return index.html for SPA client-side routing
    let resp = reqwest::get(format!("http://127.0.0.1:{}/chat", port))
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let content_type = resp
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(
        content_type.contains("text/html"),
        "SPA fallback should return HTML, got: {}",
        content_type
    );

    server.shutdown().await;
}

/// Test MIME type detection for various file extensions.
#[test]
fn test_mime_from_path() {
    assert_eq!(
        helen_agent::server::mime_from_path("favicon.png"),
        "image/png"
    );
    assert_eq!(
        helen_agent::server::mime_from_path("logo.svg"),
        "image/svg+xml"
    );
    assert_eq!(
        helen_agent::server::mime_from_path("photo.jpg"),
        "image/jpeg"
    );
    assert_eq!(
        helen_agent::server::mime_from_path("photo.jpeg"),
        "image/jpeg"
    );
    assert_eq!(
        helen_agent::server::mime_from_path("icon.ico"),
        "image/x-icon"
    );
    assert_eq!(
        helen_agent::server::mime_from_path("app.js"),
        "application/javascript"
    );
    assert_eq!(helen_agent::server::mime_from_path("style.css"), "text/css");
    assert_eq!(
        helen_agent::server::mime_from_path("index.html"),
        "text/html"
    );
    assert_eq!(
        helen_agent::server::mime_from_path("font.woff2"),
        "font/woff2"
    );
    assert_eq!(
        helen_agent::server::mime_from_path("unknown.xyz"),
        "application/octet-stream"
    );
}
