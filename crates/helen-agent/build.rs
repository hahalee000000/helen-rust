//! Build script: copy pre-built frontend and agent files from Python source

use std::fs;
use std::path::Path;

fn main() {
    // Copy pre-built frontend from Python source
    let frontend_src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("../helen/helen/agent/webui/frontend/dist");

    let frontend_dest = Path::new(env!("CARGO_MANIFEST_DIR")).join("frontend");

    if frontend_src.exists() {
        // Clean destination
        if frontend_dest.exists() {
            fs::remove_dir_all(&frontend_dest).ok();
        }
        fs::create_dir_all(&frontend_dest).ok();
        copy_dir_all(&frontend_src, &frontend_dest).ok();
        println!("cargo:warning=Copied frontend from {:?}", frontend_src);
    } else {
        println!(
            "cargo:warning=Frontend source not found at {:?}, creating placeholder",
            frontend_src
        );
        // Create placeholder
        fs::create_dir_all(&frontend_dest).ok();
        fs::write(
            frontend_dest.join("index.html"),
            "<!DOCTYPE html><html><body><h1>Helen Agent</h1><p>Frontend not built</p></body></html>",
        )
        .ok();
    }

    // Copy agent .helen files from Python source
    let agent_src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("../helen/helen/agent");

    let agent_dest = Path::new(env!("CARGO_MANIFEST_DIR")).join("agent");

    if agent_src.exists() {
        // Clean destination
        if agent_dest.exists() {
            fs::remove_dir_all(&agent_dest).ok();
        }
        fs::create_dir_all(&agent_dest).ok();

        // Copy only .helen files (not in subdirectories)
        for entry in fs::read_dir(&agent_src).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |e| e == "helen") {
                fs::copy(&path, agent_dest.join(entry.file_name())).ok();
            }
        }
        println!("cargo:warning=Copied agent files from {:?}", agent_src);
    } else {
        println!(
            "cargo:warning=Agent source not found at {:?}, creating placeholder",
            agent_src
        );
        // Create placeholder
        fs::create_dir_all(&agent_dest).ok();
        fs::write(
            agent_dest.join("placeholder.helen"),
            "// Placeholder agent file\nmain { print(\"No agent files found\") }",
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
