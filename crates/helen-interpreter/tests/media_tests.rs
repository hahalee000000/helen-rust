//! Tests for media module — multimodal media functions.

use helen_interpreter::interpreter::Interpreter;
use helen_interpreter::media::*;
use helen_interpreter::value::Value;
use num_bigint::BigInt;

fn make_interp() -> Interpreter {
    Interpreter::new()
}

// ── Type checking (stubs — always false since MediaPart not implemented) ─

#[test]
fn test_is_media_null() {
    let mut interp = make_interp();
    let result = media_is_media(&mut interp, &[Value::Null]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Bool(false));
}

#[test]
fn test_is_media_int() {
    let mut interp = make_interp();
    let result = media_is_media(&mut interp, &[Value::Int(BigInt::from(42))]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Bool(false));
}

#[test]
fn test_is_media_no_args() {
    let mut interp = make_interp();
    let result = media_is_media(&mut interp, &[]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Bool(false));
}

#[test]
fn test_media_type_null() {
    let mut interp = make_interp();
    let result = media_media_type(&mut interp, &[Value::Null]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Null);
}

#[test]
fn test_is_image_false() {
    let mut interp = make_interp();
    let result = media_is_image(&mut interp, &[Value::Str("test.png".into())]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Bool(false));
}

#[test]
fn test_is_video_false() {
    let mut interp = make_interp();
    let result = media_is_video(&mut interp, &[Value::Str("test.mp4".into())]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Bool(false));
}

#[test]
fn test_is_audio_false() {
    let mut interp = make_interp();
    let result = media_is_audio(&mut interp, &[Value::Str("test.mp3".into())]);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Value::Bool(false));
}

// ── Media creation (stubs — always error since MediaPart not implemented) ─

#[test]
fn test_media_from_string_errors() {
    let mut interp = make_interp();
    let result = media_media(&mut interp, &[Value::Str("test.png".into())]);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().class_name, "RuntimeError");
}

#[test]
fn test_media_wrong_type_errors() {
    let mut interp = make_interp();
    let result = media_media(&mut interp, &[Value::Int(BigInt::from(42))]);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().class_name, "TypeError");
}

#[test]
fn test_media_no_args_errors() {
    let mut interp = make_interp();
    let result = media_media(&mut interp, &[]);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().class_name, "TypeError");
}

#[test]
fn test_media_base64_errors() {
    let mut interp = make_interp();
    let result = media_media_base64(&mut interp, &[]);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().class_name, "RuntimeError");
}

// ── Conversion stubs (all error) ────────────────────────────────────────

#[test]
fn test_to_openai_parts_errors() {
    let mut interp = make_interp();
    let result = media_to_openai_parts(&mut interp, &[]);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().class_name, "RuntimeError");
}

#[test]
fn test_to_claude_parts_errors() {
    let mut interp = make_interp();
    let result = media_to_claude_parts(&mut interp, &[]);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().class_name, "RuntimeError");
}

#[test]
fn test_to_gemini_parts_errors() {
    let mut interp = make_interp();
    let result = media_to_gemini_parts(&mut interp, &[]);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().class_name, "RuntimeError");
}

#[test]
fn test_media_to_base64_errors() {
    let mut interp = make_interp();
    let result = media_media_to_base64(&mut interp, &[]);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().class_name, "RuntimeError");
}

#[test]
fn test_save_media_errors() {
    let mut interp = make_interp();
    let result = media_save_media(&mut interp, &[]);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().class_name, "RuntimeError");
}
