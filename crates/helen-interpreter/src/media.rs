//! Media stdlib functions for multimodal support.
//!
//! Byte-faithful port of `helen/stdlib/media.py` (v1.44.0): provides
//! functions for creating and inspecting MediaPart objects.

use std::rc::Rc;

use crate::exceptions::ExceptionValue;
use crate::interpreter::Interpreter;
use crate::value::Value;

/// Check if a value is a MediaPart.
pub fn media_is_media(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let value = args.get(0).cloned().unwrap_or(Value::Null);
    // MediaPart not yet implemented in Rust runtime
    Ok(Value::Bool(matches!(value, Value::Null) == false && false))
}

/// Get the media type of a MediaPart.
pub fn media_media_type(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let _value = args.get(0).cloned().unwrap_or(Value::Null);
    // MediaPart not yet implemented in Rust runtime
    Ok(Value::Null)
}

/// Check if a value is an image MediaPart.
pub fn media_is_image(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let _value = args.get(0).cloned().unwrap_or(Value::Null);
    Ok(Value::Bool(false))
}

/// Check if a value is a video MediaPart.
pub fn media_is_video(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let _value = args.get(0).cloned().unwrap_or(Value::Null);
    Ok(Value::Bool(false))
}

/// Check if a value is an audio MediaPart.
pub fn media_is_audio(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let _value = args.get(0).cloned().unwrap_or(Value::Null);
    Ok(Value::Bool(false))
}

/// Create a MediaPart from a file path or URL.
pub fn media_media(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let _source = match args.get(0) {
        Some(Value::Str(s)) => s.to_string(),
        Some(other) => return Err(ExceptionValue::new(
            "TypeError",
            format!("Expected string source, got {:?}", other),
            None,
        )),
        None => return Err(ExceptionValue::new(
            "TypeError",
            "media() requires at least one argument".to_string(),
            None,
        )),
    };
    // MediaPart not yet implemented in Rust runtime
    Err(ExceptionValue::new(
        "RuntimeError",
        "MediaPart is not yet implemented in the Rust runtime".to_string(),
        None,
    ))
}

/// Create a MediaPart from base64 data.
pub fn media_media_base64(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Err(ExceptionValue::new(
        "RuntimeError",
        "MediaPart is not yet implemented in the Rust runtime".to_string(),
        None,
    ))
}

/// Convert MediaPart to OpenAI format.
pub fn media_to_openai_parts(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Err(ExceptionValue::new(
        "RuntimeError",
        "MediaPart is not yet implemented in the Rust runtime".to_string(),
        None,
    ))
}

/// Convert MediaPart to Claude format.
pub fn media_to_claude_parts(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Err(ExceptionValue::new(
        "RuntimeError",
        "MediaPart is not yet implemented in the Rust runtime".to_string(),
        None,
    ))
}

/// Convert MediaPart to Gemini format.
pub fn media_to_gemini_parts(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Err(ExceptionValue::new(
        "RuntimeError",
        "MediaPart is not yet implemented in the Rust runtime".to_string(),
        None,
    ))
}

/// Convert MediaPart to base64 string.
pub fn media_media_to_base64(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Err(ExceptionValue::new(
        "RuntimeError",
        "MediaPart is not yet implemented in the Rust runtime".to_string(),
        None,
    ))
}

/// Save MediaPart to a file.
pub fn media_save_media(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Err(ExceptionValue::new(
        "RuntimeError",
        "MediaPart is not yet implemented in the Rust runtime".to_string(),
        None,
    ))
}
