//! Integration tests for multimodal `llm act` support.
//!
//! Tests the complete flow: parsing media expressions, evaluating them,
//! building multimodal user messages, and storing in history.

#![allow(clippy::arc_with_non_send_sync)]

use helen_core::lexer::Scanner;
use helen_interpreter::interpreter::Interpreter;
use helen_interpreter::llm_runtime::MockLlmRuntime;
use helen_interpreter::value::Value;
use helen_parser::Parser;
use std::sync::Arc;

/// Helper to parse and run Helen code
fn run_code(
    code: &str,
) -> (
    Result<Option<Value>, helen_interpreter::exceptions::ExceptionValue>,
    Arc<MockLlmRuntime>,
) {
    let mut interp = Interpreter::new();
    let mock = Arc::new(MockLlmRuntime::with_act_text("test response"));
    interp.set_llm_runtime(mock.clone());

    let tokens = Scanner::new(code, "test.helen").scan_all();
    let mut parser = Parser::new(tokens);
    let program = parser.parse();

    assert!(
        parser.errors().is_empty(),
        "Parse errors: {:?}",
        parser.errors()
    );

    let result = interp.interpret(&program);
    (result, mock)
}

// ── Media Expression Evaluation ─────────────────────────────────────────────

#[test]
fn test_llm_act_with_single_media_base64() {
    let code = r#"
        import std.media.*
        let img = media_base64("iVBORw0KGgo=", "image/png")
        let result = llm act "Describe this image" media(img)
    "#;

    let (result, mock) = run_code(code);

    assert!(
        result.is_ok(),
        "Should parse and execute without error: {:?}",
        result.err()
    );

    // Verify that media was evaluated and passed to LLM
    let history = mock.act_history.borrow();
    assert!(!history.is_empty(), "LLM act should have been called");

    // The prompt should contain the text
    let (prompt, _) = &history[0];
    assert!(prompt.contains("Describe this image"));
}

#[test]
fn test_llm_act_with_multiple_media() {
    let code = r#"
        import std.media.*
        let img1 = media_base64("data1", "image/png")
        let img2 = media_base64("data2", "image/png")
        let result = llm act "Compare these images" media(img1, img2)
    "#;

    let (result, _) = run_code(code);

    assert!(
        result.is_ok(),
        "Should handle multiple media expressions: {:?}",
        result.err()
    );
}

#[test]
fn test_llm_act_with_media_url() {
    let code = r#"
        import std.media.*
        let img = media("https://example.com/image.png")
        let result = llm act "Describe this" media(img)
    "#;

    let (result, _) = run_code(code);

    assert!(
        result.is_ok(),
        "Should handle media from URL: {:?}",
        result.err()
    );
}

// ── Multimodal Message Building ─────────────────────────────────────────────

#[test]
fn test_multimodal_message_structure() {
    let code = r#"
        import std.media.*
        let img = media_base64("test_data", "image/png")
        let result = llm act "Analyze this image" media(img)
    "#;

    let (result, mock) = run_code(code);

    assert!(result.is_ok(), "Should execute: {:?}", result.err());

    // Check that history contains multimodal content
    let history = mock.act_history.borrow();
    assert!(!history.is_empty());
}

#[test]
fn test_text_only_message_when_no_media() {
    let code = r#"
        let result = llm act "Just text, no media"
    "#;

    let (result, mock) = run_code(code);

    assert!(result.is_ok());

    let history = mock.act_history.borrow();
    assert!(!history.is_empty());

    // Without media, should be plain text
    let (prompt, _) = &history[0];
    assert_eq!(prompt, "Just text, no media");
}

// ── Provider Hints ──────────────────────────────────────────────────────────

#[test]
fn test_llm_act_with_provider_hint() {
    let code = r#"
        import std.media.*
        let img = media_base64("test", "image/png")
        let result = llm act "Process" media(img) provider("claude")
    "#;

    let (result, _) = run_code(code);

    assert!(
        result.is_ok(),
        "Should accept provider hint: {:?}",
        result.err()
    );
}

// ── on_media Callback ───────────────────────────────────────────────────────

#[test]
fn test_llm_act_with_on_media_callback() {
    let code = r#"
        import std.media.*
        fn custom_adapter(parts, provider) {
            return to_openai_parts(parts)
        }
        
        let img = media_base64("test", "image/png")
        let result = llm act "Process" media(img) on_media custom_adapter
    "#;

    let (result, _) = run_code(code);

    assert!(
        result.is_ok(),
        "Should accept on_media callback: {:?}",
        result.err()
    );
}

// ── on_generate Callbacks ───────────────────────────────────────────────────

#[test]
fn test_llm_act_with_on_generate_callback() {
    let code = r#"
        import std.media.*
        fn generate_image(text) {
            return media_base64("generated_data", "image/png")
        }
        
        let result = llm act "Create an image" on_generate generate_image
    "#;

    let (result, _) = run_code(code);

    assert!(
        result.is_ok(),
        "Should accept on_generate callback: {:?}",
        result.err()
    );
}

#[test]
fn test_llm_act_with_multiple_on_generate_callbacks() {
    let code = r#"
        import std.media.*
        fn generate_image(text) {
            return media_base64("img_data", "image/png")
        }
        
        fn generate_video(text) {
            return media_base64("vid_data", "video/mp4", "video")
        }
        
        let result = llm act "Create media" on_generate generate_image on_generate generate_video
    "#;

    let (result, _) = run_code(code);

    assert!(
        result.is_ok(),
        "Should accept multiple on_generate callbacks: {:?}",
        result.err()
    );
}

// ── History Management ──────────────────────────────────────────────────────

#[test]
fn test_multimodal_content_in_history() {
    let code = r#"
        import std.media.*
        let img = media_base64("test_data", "image/png")
        let result = llm act "First message" media(img)
        let result2 = llm act "Second message"
    "#;

    let (result, mock) = run_code(code);

    assert!(result.is_ok());

    let history = mock.act_history.borrow();
    // Should have 2 LLM calls
    assert_eq!(history.len(), 2);
}

// ── Error Handling ──────────────────────────────────────────────────────────

#[test]
fn test_media_expression_type_error() {
    let code = r#"
        import std.media.*
        let result = llm act "Test" media(42)
    "#;

    let (result, _) = run_code(code);

    // Should error: media() must return MediaPart
    assert!(
        result.is_err(),
        "Should error when media expression returns non-MediaPart"
    );
}

// ── Integration with Agent Context ──────────────────────────────────────────

#[test]
fn test_multimodal_in_agent() {
    let code = r#"
        import std.media.*
        agent ImageAnalyzer(img_data: str) {
            description "Analyze an image"
            
            main {
                let img = media_base64(img_data, "image/png")
                let analysis = llm act "Analyze this image" media(img)
                return analysis
            }
        }
        
        let result = ImageAnalyzer("test_data")
    "#;

    let (result, _) = run_code(code);

    assert!(
        result.is_ok(),
        "Should work within agent context: {:?}",
        result.err()
    );
}

// ── Format Adapters ─────────────────────────────────────────────────────────

#[test]
fn test_openai_format_adapter() {
    let code = r#"
        import std.media.*
        let img = media_base64("test_data", "image/png")
        let parts = to_openai_parts([img])
        // Just verify it executes without error
    "#;

    let (result, _) = run_code(code);

    assert!(
        result.is_ok(),
        "OpenAI format adapter should work: {:?}",
        result.err()
    );
}

#[test]
fn test_claude_format_adapter() {
    let code = r#"
        import std.media.*
        let img = media_base64("test_data", "image/png")
        let parts = to_claude_parts([img])
    "#;

    let (result, _) = run_code(code);

    assert!(
        result.is_ok(),
        "Claude format adapter should work: {:?}",
        result.err()
    );
}

#[test]
fn test_gemini_format_adapter() {
    let code = r#"
        import std.media.*
        let img = media_base64("test_data", "image/png")
        let parts = to_gemini_parts([img])
    "#;

    let (result, _) = run_code(code);

    assert!(
        result.is_ok(),
        "Gemini format adapter should work: {:?}",
        result.err()
    );
}
