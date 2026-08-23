//! Tests for embedded agent files

#[test]
fn test_embedded_agent_files_accessible() {
    let files = helen_agent::agent_files::list_embedded_agents();
    assert!(
        files.contains(&"chat_actor.helen".to_string()),
        "Should contain chat_actor.helen, got: {:?}",
        files
    );

    let content = helen_agent::agent_files::get_embedded_agent("chat_actor.helen");
    assert!(content.is_some(), "Should be able to get chat_actor.helen");
    let content = content.unwrap();
    assert!(
        content.contains("actor") || content.contains("agent") || content.contains("main"),
        "Content should contain 'actor', 'agent', or 'main'"
    );
}
