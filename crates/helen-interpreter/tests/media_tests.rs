//! Tests for media module — multimodal media functions.

use helen_interpreter::interpreter::Interpreter;
use helen_interpreter::media::*;
use helen_interpreter::value::Value;
use num_bigint::BigInt;
use std::rc::Rc;

fn make_interp() -> Interpreter {
    Interpreter::new()
}

// ── Type checking ─────────────────────────────────────────────────────────────

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

// ── Media creation ────────────────────────────────────────────────────────────

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
fn test_media_base64_no_args_errors() {
    let mut interp = make_interp();
    let result = media_media_base64(&mut interp, &[]);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().class_name, "TypeError");
}

#[test]
fn test_to_openai_parts_no_args_errors() {
    let mut interp = make_interp();
    let result = media_to_openai_parts(&mut interp, &[]);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().class_name, "TypeError");
}

#[test]
fn test_to_claude_parts_no_args_errors() {
    let mut interp = make_interp();
    let result = media_to_claude_parts(&mut interp, &[]);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().class_name, "TypeError");
}

#[test]
fn test_to_gemini_parts_no_args_errors() {
    let mut interp = make_interp();
    let result = media_to_gemini_parts(&mut interp, &[]);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().class_name, "TypeError");
}

#[test]
fn test_media_to_base64_no_args_errors() {
    let mut interp = make_interp();
    let result = media_media_to_base64(&mut interp, &[]);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().class_name, "TypeError");
}

#[test]
fn test_save_media_no_args_errors() {
    let mut interp = make_interp();
    let result = media_save_media(&mut interp, &[]);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().class_name, "TypeError");
}

// ── MediaPart creation and inspection ─────────────────────────────────────────

#[test]
fn test_media_base64_creation() {
    let mut interp = make_interp();
    let result = media_media_base64(
        &mut interp,
        &[
            Value::Str(Rc::from("iVBORw0KGgo=")),
            Value::Str(Rc::from("image/png")),
        ],
    );
    assert!(result.is_ok());
    let value = result.unwrap();
    assert!(matches!(value, Value::MediaPart(_)));
    
    // Check is_media
    let is_media = media_is_media(&mut interp, std::slice::from_ref(&value));
    assert_eq!(is_media.unwrap(), Value::Bool(true));
    
    // Check media_type
    let media_type = media_media_type(&mut interp, std::slice::from_ref(&value));
    assert_eq!(media_type.unwrap(), Value::Str(Rc::from("image")));
    
    // Check is_image
    let is_image = media_is_image(&mut interp, &[value]);
    assert_eq!(is_image.unwrap(), Value::Bool(true));
}

#[test]
fn test_media_base64_with_explicit_type() {
    let mut interp = make_interp();
    let result = media_media_base64(
        &mut interp,
        &[
            Value::Str(Rc::from("base64data")),
            Value::Str(Rc::from("audio/mp3")),
            Value::Str(Rc::from("audio")),
        ],
    );
    assert!(result.is_ok());
    let value = result.unwrap();
    
    let is_audio = media_is_audio(&mut interp, &[value]);
    assert_eq!(is_audio.unwrap(), Value::Bool(true));
}

#[test]
fn test_media_to_base64_roundtrip() {
    let mut interp = make_interp();
    let original_data = "iVBORw0KGgo=";
    
    // Create MediaPart from base64
    let create_result = media_media_base64(
        &mut interp,
        &[
            Value::Str(Rc::from(original_data)),
            Value::Str(Rc::from("image/png")),
        ],
    );
    assert!(create_result.is_ok());
    let media_part = create_result.unwrap();
    
    // Convert back to base64
    let to_base64_result = media_media_to_base64(&mut interp, &[media_part]);
    assert!(to_base64_result.is_ok());
    let base64_str = to_base64_result.unwrap();
    
    assert_eq!(base64_str, Value::Str(Rc::from(original_data)));
}

#[test]
fn test_to_openai_parts_empty_list() {
    let mut interp = make_interp();
    let empty_list = Value::List(Rc::new(std::cell::RefCell::new(vec![])));
    let result = media_to_openai_parts(&mut interp, &[empty_list]);
    assert!(result.is_ok());
    if let Value::List(list) = result.unwrap() {
        assert_eq!(list.borrow().len(), 0);
    } else {
        panic!("Expected list");
    }
}

#[test]
fn test_to_claude_parts_empty_list() {
    let mut interp = make_interp();
    let empty_list = Value::List(Rc::new(std::cell::RefCell::new(vec![])));
    let result = media_to_claude_parts(&mut interp, &[empty_list]);
    assert!(result.is_ok());
    if let Value::List(list) = result.unwrap() {
        assert_eq!(list.borrow().len(), 0);
    } else {
        panic!("Expected list");
    }
}

#[test]
fn test_to_gemini_parts_empty_list() {
    let mut interp = make_interp();
    let empty_list = Value::List(Rc::new(std::cell::RefCell::new(vec![])));
    let result = media_to_gemini_parts(&mut interp, &[empty_list]);
    assert!(result.is_ok());
    if let Value::List(list) = result.unwrap() {
        assert_eq!(list.borrow().len(), 0);
    } else {
        panic!("Expected list");
    }
}
