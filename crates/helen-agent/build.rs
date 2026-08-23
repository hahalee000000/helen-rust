//! Build script: copy pre-built frontend from Python source

use std::fs;
use std::path::Path;

fn main() {
    // Copy pre-built frontend from Python source
    let src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("../helen/helen/agent/webui/frontend/dist");

    let dest = Path::new(env!("CARGO_MANIFEST_DIR")).join("frontend");

    if src.exists() {
        // Clean destination
        if dest.exists() {
            fs::remove_dir_all(&dest).ok();
        }
        fs::create_dir_all(&dest).ok();
        copy_dir_all(&src, &dest).ok();
        println!("cargo:warning=Copied frontend from {:?}", src);
    } else {
        println!(
            "cargo:warning=Frontend source not found at {:?}, creating placeholder",
            src
        );
        // Create placeholder
        fs::create_dir_all(&dest).ok();
        fs::write(
            dest.join("index.html"),
            "<!DOCTYPE html><html><body><h1>Helen Agent</h1><p>Frontend not built</p></body></html>",
        )
        .ok();
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}
