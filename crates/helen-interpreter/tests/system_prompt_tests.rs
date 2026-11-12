//! Regression tests for system prompt building (skill loading fix)
//!
//! These tests verify that:
//! 1. Working memory instructions are included in system prompt (Python parity)
//! 2. Skill index is properly generated and included
//! 3. System prompt contains all required sections for skill loading

use helen_interpreter::interpreter::Interpreter;

/// Test that build_system_prompt includes working memory instructions
#[test]
fn test_system_prompt_includes_working_memory() {
    let interp = Interpreter::new();

    // Use reflection to call the private method (or make it pub for testing)
    // For now, we'll test through the public interface
    let system_prompt = interp.build_system_prompt();

    // Verify working memory section is present
    assert!(
        system_prompt.contains("## Working Memory Maintenance"),
        "System prompt should contain Working Memory Maintenance section"
    );

    assert!(
        system_prompt.contains("<working_memory>"),
        "System prompt should contain working_memory XML tag example"
    );

    assert!(
        system_prompt.contains("active_files:"),
        "System prompt should contain active_files field example"
    );
}

/// Test that build_system_prompt includes framework instructions with skill loading
#[test]
fn test_system_prompt_includes_skill_loading_instructions() {
    let interp = Interpreter::new();
    let system_prompt = interp.build_system_prompt();

    // Verify framework instructions are present
    assert!(
        system_prompt.contains("<framework_instructions>"),
        "System prompt should contain framework_instructions tag"
    );

    // Verify skill loading instruction is present
    assert!(
        system_prompt.contains("## 2. Skills (CRITICAL)"),
        "System prompt should contain Skills section"
    );

    assert!(
        system_prompt.contains("load_skill"),
        "System prompt should mention load_skill"
    );

    assert!(
        system_prompt.contains("<available_skills>"),
        "System prompt should reference available_skills"
    );
}

/// Test that build_system_prompt includes Helen conventions
#[test]
fn test_system_prompt_includes_helen_conventions() {
    let interp = Interpreter::new();
    let system_prompt = interp.build_system_prompt();

    // Verify Helen conventions are present
    assert!(
        system_prompt.contains("<helen_conventions>"),
        "System prompt should contain helen_conventions tag"
    );

    assert!(
        system_prompt.contains("Skill-Driven Development"),
        "System prompt should mention Skill-Driven Development"
    );

    assert!(
        system_prompt.contains("helen-syntax"),
        "System prompt should reference helen-syntax skill"
    );

    assert!(
        system_prompt.contains("helen-stdlib"),
        "System prompt should reference helen-stdlib skill"
    );
}

/// Test that skill index is generated and included when skills exist
#[test]
fn test_system_prompt_includes_skill_index() {
    let interp = Interpreter::new();
    let system_prompt = interp.build_system_prompt();

    // Verify skill index is present (should have at least embedded skills)
    assert!(
        system_prompt.contains("<available_skills>"),
        "System prompt should contain available_skills XML block"
    );

    assert!(
        system_prompt.contains("</available_skills>"),
        "System prompt should contain closing available_skills tag"
    );

    // Verify the instruction to scan skills is present
    assert!(
        system_prompt.contains("Before replying, scan skills below"),
        "System prompt should instruct to scan skills before replying"
    );
}

/// Test that system prompt sections are in correct order
#[test]
fn test_system_prompt_section_order() {
    let interp = Interpreter::new();
    let system_prompt = interp.build_system_prompt();

    // Find positions of each section
    // Note: FRAMEWORK_INSTRUCTIONS contains a reference to <available_skills>,
    // so we search for the closing tag to find the actual XML block
    let framework_pos = system_prompt
        .find("<framework_instructions>")
        .expect("framework_instructions not found");
    let conventions_pos = system_prompt
        .find("<helen_conventions>")
        .expect("helen_conventions not found");
    let skills_pos = system_prompt
        .find("</available_skills>")
        .expect("available_skills closing tag not found");
    let working_memory_pos = system_prompt
        .find("## Working Memory Maintenance")
        .expect("Working Memory Maintenance not found");

    // Verify order: framework < conventions < skills < working_memory
    assert!(
        framework_pos < conventions_pos,
        "framework_instructions should come before helen_conventions"
    );
    assert!(
        conventions_pos < skills_pos,
        "helen_conventions should come before available_skills"
    );
    assert!(
        skills_pos < working_memory_pos,
        "available_skills should come before Working Memory Maintenance"
    );
}

/// Test that agent description is included when agent context exists
#[test]
fn test_system_prompt_with_agent_description() {
    let interp = Interpreter::new();

    // Note: This test would require setting up an agent context
    // For now, we just verify the method doesn't panic without an agent
    let system_prompt = interp.build_system_prompt();
    assert!(
        !system_prompt.is_empty(),
        "System prompt should not be empty"
    );
}

/// Test working memory instructions content matches Python version
#[test]
fn test_working_memory_instructions_content() {
    let interp = Interpreter::new();
    let wm_instructions = interp.build_working_memory_instructions();

    // Verify key elements from Python version
    assert!(
        wm_instructions
            .contains("IMPORTANT: Always provide your complete response to the user FIRST"),
        "Should emphasize complete response first"
    );

    assert!(
        wm_instructions.contains("active_files:"),
        "Should include active_files field"
    );

    assert!(
        wm_instructions.contains("decisions:"),
        "Should include decisions field"
    );

    assert!(
        wm_instructions.contains("todos:"),
        "Should include todos field"
    );

    assert!(
        wm_instructions.contains("errors:"),
        "Should include errors field"
    );

    assert!(
        wm_instructions.contains("Example response:"),
        "Should include example response"
    );
}
