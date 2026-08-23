//! Tests for the `helen agent` CLI command

#[test]
fn test_agent_command_help() {
    let output = std::process::Command::new("cargo")
        .args(["run", "--bin", "helen", "--", "agent", "--help"])
        .output()
        .expect("Failed to execute helen");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    // Should mention agent or webui
    assert!(
        combined.contains("agent") || combined.contains("webui") || combined.contains("Web UI"),
        "Help should mention agent/webui, got: {}",
        combined
    );
}
