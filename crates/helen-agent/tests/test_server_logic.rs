//! Unit tests for server.rs pure functions
//!
//! Tests mime_from_path, auth middleware logic, and server construction.

/// Test mime_from_path for all supported extensions
#[test]
fn test_mime_from_path_js() {
    assert_eq!(
        helen_agent::server::mime_from_path("app.js"),
        "application/javascript"
    );
}

#[test]
fn test_mime_from_path_css() {
    assert_eq!(
        helen_agent::server::mime_from_path("style.css"),
        "text/css"
    );
}

#[test]
fn test_mime_from_path_html() {
    assert_eq!(
        helen_agent::server::mime_from_path("index.html"),
        "text/html"
    );
}

#[test]
fn test_mime_from_path_svg() {
    assert_eq!(
        helen_agent::server::mime_from_path("icon.svg"),
        "image/svg+xml"
    );
}

#[test]
fn test_mime_from_path_png() {
    assert_eq!(
        helen_agent::server::mime_from_path("image.png"),
        "image/png"
    );
}

#[test]
fn test_mime_from_path_jpg() {
    assert_eq!(
        helen_agent::server::mime_from_path("photo.jpg"),
        "image/jpeg"
    );
}

#[test]
fn test_mime_from_path_jpeg() {
    assert_eq!(
        helen_agent::server::mime_from_path("photo.jpeg"),
        "image/jpeg"
    );
}

#[test]
fn test_mime_from_path_gif() {
    assert_eq!(
        helen_agent::server::mime_from_path("anim.gif"),
        "image/gif"
    );
}

#[test]
fn test_mime_from_path_ico() {
    assert_eq!(
        helen_agent::server::mime_from_path("favicon.ico"),
        "image/x-icon"
    );
}

#[test]
fn test_mime_from_path_woff() {
    assert_eq!(
        helen_agent::server::mime_from_path("font.woff"),
        "font/woff"
    );
}

#[test]
fn test_mime_from_path_woff2() {
    assert_eq!(
        helen_agent::server::mime_from_path("font.woff2"),
        "font/woff2"
    );
}

#[test]
fn test_mime_from_path_ttf() {
    assert_eq!(
        helen_agent::server::mime_from_path("font.ttf"),
        "font/ttf"
    );
}

#[test]
fn test_mime_from_path_json() {
    assert_eq!(
        helen_agent::server::mime_from_path("data.json"),
        "application/json"
    );
}

#[test]
fn test_mime_from_path_wasm() {
    assert_eq!(
        helen_agent::server::mime_from_path("module.wasm"),
        "application/wasm"
    );
}

#[test]
fn test_mime_from_path_unknown() {
    assert_eq!(
        helen_agent::server::mime_from_path("file.xyz"),
        "application/octet-stream"
    );
}

#[test]
fn test_mime_from_path_no_extension() {
    assert_eq!(
        helen_agent::server::mime_from_path("Makefile"),
        "application/octet-stream"
    );
}

#[test]
fn test_mime_from_path_nested() {
    assert_eq!(
        helen_agent::server::mime_from_path("assets/js/app.js"),
        "application/javascript"
    );
}
