//! Tests for Helen execution

#[tokio::test]
async fn test_execute_simple_helen_program() {
    let code = r#"
import std.core.*
main { print("Hello from Helen!") }
"#;
    let output = helen_agent::executor::execute_helen(code).await.unwrap();
    assert!(
        output.contains("Hello from Helen!"),
        "Output should contain the print statement, got: {}",
        output
    );
}
