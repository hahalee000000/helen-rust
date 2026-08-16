//! Media stdlib functions for multimodal support.
//!
//! Byte-faithful port of `helen/stdlib/media.py` (v1.44.0): provides
//! functions for creating and inspecting MediaPart objects.

use std::collections::HashMap;
use std::io::Read;
use std::rc::Rc;

use crate::exceptions::ExceptionValue;
use crate::interpreter::Interpreter;
use crate::value::Value;
use helen_runtime::media::MediaPart;

/// Check if a value is a MediaPart.
pub fn media_is_media(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let value = args.get(0).cloned().unwrap_or(Value::Null);
    Ok(Value::Bool(matches!(value, Value::MediaPart(_))))
}

/// Get the media type of a MediaPart.
pub fn media_media_type(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let value = args.get(0).cloned().unwrap_or(Value::Null);
    match value {
        Value::MediaPart(mp) => Ok(Value::Str(Rc::from(mp.media_type.as_str()))),
        _ => Ok(Value::Null),
    }
}

/// Check if a value is an image MediaPart.
pub fn media_is_image(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let value = args.get(0).cloned().unwrap_or(Value::Null);
    match value {
        Value::MediaPart(mp) => Ok(Value::Bool(mp.is_image())),
        _ => Ok(Value::Bool(false)),
    }
}

/// Check if a value is a video MediaPart.
pub fn media_is_video(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let value = args.get(0).cloned().unwrap_or(Value::Null);
    match value {
        Value::MediaPart(mp) => Ok(Value::Bool(mp.is_video())),
        _ => Ok(Value::Bool(false)),
    }
}

/// Check if a value is an audio MediaPart.
pub fn media_is_audio(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let value = args.get(0).cloned().unwrap_or(Value::Null);
    match value {
        Value::MediaPart(mp) => Ok(Value::Bool(mp.is_audio())),
        _ => Ok(Value::Bool(false)),
    }
}

/// Create a MediaPart from a file path or URL.
pub fn media_media(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    if args.is_empty() {
        return Err(ExceptionValue::new(
            "TypeError",
            "media() requires at least one argument".to_string(),
            None,
        ));
    }

    let source = match args.get(0) {
        Some(Value::Str(s)) => s.to_string(),
        Some(Value::MediaPart(mp)) => {
            // Passthrough existing MediaPart
            return Ok(Value::MediaPart(mp.clone()));
        }
        Some(other) => {
            return Err(ExceptionValue::new(
                "TypeError",
                format!(
                    "media() argument must be str or MediaPart, got {:?}",
                    other.type_name()
                ),
                None,
            ));
        }
        None => {
            return Err(ExceptionValue::new(
                "TypeError",
                "media() requires at least one argument".to_string(),
                None,
            ));
        }
    };

    let media_type = match args.get(1) {
        Some(Value::Str(s)) => Some(s.to_string()),
        _ => None,
    };

    let part = create_media_part(&source, media_type.as_deref())?;
    Ok(Value::MediaPart(Rc::new(part)))
}

/// Create a MediaPart from base64 data.
pub fn media_media_base64(
    _i: &mut Interpreter,
    args: &[Value],
) -> Result<Value, ExceptionValue> {
    let data = match args.get(0) {
        Some(Value::Str(s)) => s.to_string(),
        _ => {
            return Err(ExceptionValue::new(
                "TypeError",
                "media_base64() requires a base64 string as first argument".to_string(),
                None,
            ));
        }
    };

    let mime = match args.get(1) {
        Some(Value::Str(s)) => s.to_string(),
        _ => {
            return Err(ExceptionValue::new(
                "TypeError",
                "media_base64() requires a MIME type as second argument".to_string(),
                None,
            ));
        }
    };

    let media_type = match args.get(2) {
        Some(Value::Str(s)) => Some(s.to_string()),
        _ => None,
    };

    let media_type = media_type.unwrap_or_else(|| guess_media_type(&mime));

    let part = MediaPart::from_base64(data, mime, media_type)
        .map_err(|e| ExceptionValue::new("ValueError", e, None))?;

    Ok(Value::MediaPart(Rc::new(part)))
}

/// Convert MediaPart to OpenAI format.
pub fn media_to_openai_parts(
    _i: &mut Interpreter,
    args: &[Value],
) -> Result<Value, ExceptionValue> {
    let parts = match args.get(0) {
        Some(Value::List(l)) => l.borrow().clone(),
        _ => {
            return Err(ExceptionValue::new(
                "TypeError",
                "to_openai_parts() requires a list of MediaParts".to_string(),
                None,
            ));
        }
    };

    let mut result = Vec::new();
    for part in parts {
        if let Value::MediaPart(mp) = part {
            let openai_part = convert_to_openai(&mp)?;
            if let Some(p) = openai_part {
                result.push(p);
            }
        }
    }

    Ok(Value::List(Rc::new(std::cell::RefCell::new(result))))
}

/// Convert MediaPart to Claude format.
pub fn media_to_claude_parts(
    _i: &mut Interpreter,
    args: &[Value],
) -> Result<Value, ExceptionValue> {
    let parts = match args.get(0) {
        Some(Value::List(l)) => l.borrow().clone(),
        _ => {
            return Err(ExceptionValue::new(
                "TypeError",
                "to_claude_parts() requires a list of MediaParts".to_string(),
                None,
            ));
        }
    };

    let mut result = Vec::new();
    for part in parts {
        if let Value::MediaPart(mp) = part {
            let claude_part = convert_to_claude(&mp)?;
            if let Some(p) = claude_part {
                result.push(p);
            }
        }
    }

    Ok(Value::List(Rc::new(std::cell::RefCell::new(result))))
}

/// Convert MediaPart to Gemini format.
pub fn media_to_gemini_parts(
    _i: &mut Interpreter,
    args: &[Value],
) -> Result<Value, ExceptionValue> {
    let parts = match args.get(0) {
        Some(Value::List(l)) => l.borrow().clone(),
        _ => {
            return Err(ExceptionValue::new(
                "TypeError",
                "to_gemini_parts() requires a list of MediaParts".to_string(),
                None,
            ));
        }
    };

    let mut result = Vec::new();
    for part in parts {
        if let Value::MediaPart(mp) = part {
            let gemini_part = convert_to_gemini(&mp)?;
            if let Some(p) = gemini_part {
                result.push(p);
            }
        }
    }

    Ok(Value::List(Rc::new(std::cell::RefCell::new(result))))
}

/// Convert MediaPart to base64 string.
pub fn media_media_to_base64(
    _i: &mut Interpreter,
    args: &[Value],
) -> Result<Value, ExceptionValue> {
    let part = match args.get(0) {
        Some(Value::MediaPart(mp)) => mp.clone(),
        _ => {
            return Err(ExceptionValue::new(
                "TypeError",
                "media_to_base64() requires a MediaPart".to_string(),
                None,
            ));
        }
    };

    let base64_data = read_media_as_base64(&part)?;
    Ok(Value::Str(Rc::from(base64_data.as_str())))
}

/// Save MediaPart to a file.
pub fn media_save_media(
    _i: &mut Interpreter,
    args: &[Value],
) -> Result<Value, ExceptionValue> {
    let part = match args.get(0) {
        Some(Value::MediaPart(mp)) => mp.clone(),
        _ => {
            return Err(ExceptionValue::new(
                "TypeError",
                "save_media() requires a MediaPart".to_string(),
                None,
            ));
        }
    };

    let path = match args.get(1) {
        Some(Value::Str(s)) => s.to_string(),
        _ => {
            // Generate a default path
            let ext = mime_to_extension(&part.mime);
            format!("media_{}.{}", uuid::Uuid::new_v4(), ext)
        }
    };

    let base64_data = read_media_as_base64(&part)?;
    let bytes = base64::decode(&base64_data)
        .map_err(|e| ExceptionValue::new("ValueError", format!("Failed to decode base64: {}", e), None))?;

    std::fs::write(&path, &bytes)
        .map_err(|e| ExceptionValue::new("IOError", format!("Failed to write file: {}", e), None))?;

    Ok(Value::Str(Rc::from(path.as_str())))
}

// ── Helper functions ──────────────────────────────────────────────────────────

fn create_media_part(source: &str, media_type: Option<&str>) -> Result<MediaPart, ExceptionValue> {
    let (source_type, content, mime) = if source.starts_with("http://")
        || source.starts_with("https://")
        || source.starts_with("//")
    {
        // URL
        let mime = guess_mime_from_url(source);
        ("url".to_string(), source.to_string(), mime)
    } else {
        // File path
        if !std::path::Path::new(source).exists() {
            return Err(ExceptionValue::new(
                "ValueError",
                format!("Media file not found: {}", source),
                None,
            ));
        }
        if !std::path::Path::new(source).is_file() {
            return Err(ExceptionValue::new(
                "ValueError",
                format!("Media path is not a file: {}", source),
                None,
            ));
        }
        let mime = mime_guess::from_path(source)
            .first_or_octet_stream()
            .to_string();
        ("file".to_string(), source.to_string(), Some(mime))
    };

    let mime = mime.unwrap_or_else(|| {
        match media_type {
            Some("video") => "video/mp4".to_string(),
            Some("audio") => "audio/mp3".to_string(),
            _ => "image/png".to_string(),
        }
    });

    let media_type = match media_type {
        Some(mt) => mt.to_string(),
        None => guess_media_type(&mime),
    };

    MediaPart::new(source_type, content, mime, media_type, HashMap::new())
        .map_err(|e| ExceptionValue::new("ValueError", e, None))
}

fn guess_media_type(mime: &str) -> String {
    if mime.starts_with("image/") {
        "image".to_string()
    } else if mime.starts_with("video/") {
        "video".to_string()
    } else if mime.starts_with("audio/") {
        "audio".to_string()
    } else {
        "image".to_string() // Default to image
    }
}

fn guess_mime_from_url(url: &str) -> Option<String> {
    let path = url.split('?').next().unwrap_or(url);
    let path = path.split('#').next().unwrap_or(path);
    mime_guess::from_path(path).first().map(|m| m.to_string())
}

fn read_media_as_base64(part: &MediaPart) -> Result<String, ExceptionValue> {
    match part.source.as_str() {
        "base64" => Ok(part.content.clone()),
        "file" => {
            let mut file = std::fs::File::open(&part.content)
                .map_err(|e| ExceptionValue::new("IOError", format!("Failed to open file: {}", e), None))?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)
                .map_err(|e| ExceptionValue::new("IOError", format!("Failed to read file: {}", e), None))?;
            Ok(base64::encode(&buffer))
        }
        "url" => {
            // For URLs, we'd need to download the content
            // For now, return an error as this requires async HTTP
            Err(ExceptionValue::new(
                "NotImplementedError",
                "URL downloading not yet implemented in Rust runtime".to_string(),
                None,
            ))
        }
        _ => Err(ExceptionValue::new(
            "ValueError",
            format!("Unknown MediaPart source: {}", part.source),
            None,
        )),
    }
}

fn convert_to_openai(part: &MediaPart) -> Result<Option<Value>, ExceptionValue> {
    match part.media_type.as_str() {
        "image" => {
            let url = match part.source.as_str() {
                "url" => part.content.clone(),
                "base64" => format!("data:{};base64,{}", part.mime, part.content),
                "file" => {
                    let b64 = read_media_as_base64(part)?;
                    format!("data:{};base64,{}", part.mime, b64)
                }
                _ => return Ok(None),
            };

            let mut map = HashMap::new();
            map.insert(
                Value::Str(Rc::from("type")),
                Value::Str(Rc::from("image_url")),
            );

            let mut image_url_map = HashMap::new();
            image_url_map.insert(Value::Str(Rc::from("url")), Value::Str(Rc::from(url.as_str())));
            map.insert(
                Value::Str(Rc::from("image_url")),
                Value::Map(Rc::new(std::cell::RefCell::new(image_url_map.into_iter().collect()))),
            );

            Ok(Some(Value::Map(Rc::new(std::cell::RefCell::new(
                map.into_iter().collect(),
            )))))
        }
        "video" => {
            // Video falls back to text placeholder
            let text = format!("[视频: {}]", if part.source == "url" { &part.content } else { &part.media_type });
            let mut map = HashMap::new();
            map.insert(Value::Str(Rc::from("type")), Value::Str(Rc::from("text")));
            map.insert(Value::Str(Rc::from("text")), Value::Str(Rc::from(text.as_str())));
            Ok(Some(Value::Map(Rc::new(std::cell::RefCell::new(
                map.into_iter().collect(),
            )))))
        }
        "audio" => {
            let (audio_type, data) = match part.source.as_str() {
                "url" => ("audio_url".to_string(), part.content.clone()),
                "base64" | "file" => {
                    let data = if part.source == "file" {
                        read_media_as_base64(part)?
                    } else {
                        part.content.clone()
                    };
                    ("input_audio".to_string(), data)
                }
                _ => return Ok(None),
            };

            let mut map = HashMap::new();
            map.insert(Value::Str(Rc::from("type")), Value::Str(Rc::from(audio_type.as_str())));

            if audio_type == "audio_url" {
                let mut audio_url_map = HashMap::new();
                audio_url_map.insert(Value::Str(Rc::from("url")), Value::Str(Rc::from(data.as_str())));
                map.insert(
                    Value::Str(Rc::from("audio_url")),
                    Value::Map(Rc::new(std::cell::RefCell::new(audio_url_map.into_iter().collect()))),
                );
            } else {
                let mut input_audio_map = HashMap::new();
                input_audio_map.insert(Value::Str(Rc::from("data")), Value::Str(Rc::from(data.as_str())));
                input_audio_map.insert(Value::Str(Rc::from("format")), Value::Str(Rc::from(part.mime.as_str())));
                map.insert(
                    Value::Str(Rc::from("input_audio")),
                    Value::Map(Rc::new(std::cell::RefCell::new(input_audio_map.into_iter().collect()))),
                );
            }

            Ok(Some(Value::Map(Rc::new(std::cell::RefCell::new(
                map.into_iter().collect(),
            )))))
        }
        _ => Ok(None),
    }
}

fn convert_to_claude(part: &MediaPart) -> Result<Option<Value>, ExceptionValue> {
    // Claude doesn't support video or audio
    if part.media_type == "video" {
        return Err(ExceptionValue::new(
            "ValueError",
            "Claude Messages API does not support video input. Consider extracting key frames as images instead.".to_string(),
            None,
        ));
    }
    if part.media_type == "audio" {
        return Err(ExceptionValue::new(
            "ValueError",
            "Claude Messages API does not support audio input. Consider transcribing the audio first.".to_string(),
            None,
        ));
    }

    if part.media_type != "image" {
        return Ok(None);
    }

    let mut map = HashMap::new();
    map.insert(Value::Str(Rc::from("type")), Value::Str(Rc::from("image")));

    let source = if part.source == "url" {
        let mut source_map = HashMap::new();
        source_map.insert(Value::Str(Rc::from("type")), Value::Str(Rc::from("url")));
        source_map.insert(Value::Str(Rc::from("url")), Value::Str(Rc::from(part.content.as_str())));
        Value::Map(Rc::new(std::cell::RefCell::new(source_map.into_iter().collect())))
    } else {
        // base64 or file
        let b64 = read_media_as_base64(part)?;
        let mut source_map = HashMap::new();
        source_map.insert(Value::Str(Rc::from("type")), Value::Str(Rc::from("base64")));
        source_map.insert(Value::Str(Rc::from("media_type")), Value::Str(Rc::from(part.mime.as_str())));
        source_map.insert(Value::Str(Rc::from("data")), Value::Str(Rc::from(b64.as_str())));
        Value::Map(Rc::new(std::cell::RefCell::new(source_map.into_iter().collect())))
    };

    map.insert(Value::Str(Rc::from("source")), source);

    Ok(Some(Value::Map(Rc::new(std::cell::RefCell::new(
        map.into_iter().collect(),
    )))))
}

fn convert_to_gemini(part: &MediaPart) -> Result<Option<Value>, ExceptionValue> {
    let b64 = read_media_as_base64(part)?;

    let mut inline_data = HashMap::new();
    inline_data.insert(Value::Str(Rc::from("mime_type")), Value::Str(Rc::from(part.mime.as_str())));
    inline_data.insert(Value::Str(Rc::from("data")), Value::Str(Rc::from(b64.as_str())));

    let mut map = HashMap::new();
    map.insert(
        Value::Str(Rc::from("inline_data")),
        Value::Map(Rc::new(std::cell::RefCell::new(inline_data.into_iter().collect()))),
    );

    Ok(Some(Value::Map(Rc::new(std::cell::RefCell::new(
        map.into_iter().collect(),
    )))))
}

fn mime_to_extension(mime: &str) -> String {
    match mime {
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "audio/mp3" | "audio/mpeg" => "mp3",
        "audio/wav" => "wav",
        "audio/ogg" => "ogg",
        _ => "bin",
    }
    .to_string()
}
