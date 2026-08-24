//! Build script: ensure `frontend/dist/` exists for `rust-embed`.
//!
//! `frontend/dist/` is a build artifact (gitignored) produced by `npm run build`.
//! On CI or fresh clones the directory may not exist yet, which causes
//! `#[derive(RustEmbed)] #[folder = "frontend/dist/"]` to fail at compile time.
//!
//! This script creates a minimal placeholder so the crate always compiles.
//! A real frontend build will overwrite the placeholder with production assets.

use std::fs;
use std::path::Path;

fn main() {
    // rust-embed resolves paths relative to CARGO_MANIFEST_DIR
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let dist = Path::new(&manifest_dir).join("frontend/dist");

    if !dist.exists() {
        fs::create_dir_all(&dist).expect("failed to create frontend/dist/");
    }

    let index = dist.join("index.html");
    if !index.exists() {
        let placeholder = r#"<!doctype html>
<html lang="en">
<head><meta charset="UTF-8"><title>Helen</title></head>
<body>
<p>Frontend not built. Run <code>cd crates/helen-agent/frontend &amp;&amp; npm run build</code>.</p>
</body>
</html>"#;
        fs::write(&index, placeholder).expect("failed to write placeholder index.html");
    }
}
