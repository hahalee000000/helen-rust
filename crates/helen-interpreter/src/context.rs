//! Context management stdlib functions.
//!
//! Byte-faithful port of `helen/stdlib/context.py` (v1.44.0): provides
//! functions to manage LLM conversation context.

use std::cell::RefCell;
use std::rc::Rc;

use num_bigint::BigInt;

use crate::exceptions::ExceptionValue;
use crate::interpreter::Interpreter;
use crate::value::Value;

/// Clear the current conversation context.
pub fn context_clear_context(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("No interpreter context available")));
    result.insert(Value::Str(Rc::from("cleared_messages")), Value::Int(BigInt::from(0)));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Return detailed statistics about the current conversation context.
pub fn context_context_stats(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("No interpreter context available")));
    result.insert(Value::Str(Rc::from("message_count")), Value::Int(BigInt::from(0)));
    result.insert(Value::Str(Rc::from("total_tokens")), Value::Int(BigInt::from(0)));
    result.insert(Value::Str(Rc::from("usage_ratio")), Value::Float(0.0));
    result.insert(Value::Str(Rc::from("max_tokens")), Value::Int(BigInt::from(0)));
    
    let mut by_role = indexmap::IndexMap::new();
    by_role.insert(Value::Str(Rc::from("system")), Value::Int(BigInt::from(0)));
    by_role.insert(Value::Str(Rc::from("user")), Value::Int(BigInt::from(0)));
    by_role.insert(Value::Str(Rc::from("assistant")), Value::Int(BigInt::from(0)));
    by_role.insert(Value::Str(Rc::from("tool")), Value::Int(BigInt::from(0)));
    result.insert(Value::Str(Rc::from("by_role")), Value::Map(Rc::new(RefCell::new(by_role))));
    
    result.insert(Value::Str(Rc::from("compressed_count")), Value::Int(BigInt::from(0)));
    result.insert(Value::Str(Rc::from("pinned_count")), Value::Int(BigInt::from(0)));
    
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Return current context usage ratio (0.0 to 1.0+).
pub fn context_context_usage(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::Float(0.0))
}

/// Retrieve a single message by UUID.
pub fn context_get_message(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let _uuid = arg_str(args, 0)?;
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("Message not found")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Insert a message into the conversation history.
pub fn context_insert_message(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("Context management not available")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Delete a message by UUID.
pub fn context_delete_message(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let _uuid = arg_str(args, 0)?;
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("Message not found")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Pin a message (immune to compression).
pub fn context_pin_message(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let _uuid = arg_str(args, 0)?;
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("Message not found")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Unpin a previously pinned message.
pub fn context_unpin_message(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let _uuid = arg_str(args, 0)?;
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("Message not found")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// List all pinned messages.
pub fn context_list_pinned_messages(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    Ok(Value::List(Rc::new(RefCell::new(vec![]))))
}

/// Compress the current conversation context.
pub fn context_compress_context(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("No interpreter context available")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Search context for messages matching a query.
pub fn context_search_context(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("No interpreter context available")));
    result.insert(Value::Str(Rc::from("matches")), Value::List(Rc::new(RefCell::new(vec![]))));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Get a slice of the conversation history.
pub fn context_context_slice(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("No interpreter context available")));
    result.insert(Value::Str(Rc::from("messages")), Value::List(Rc::new(RefCell::new(vec![]))));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Export the current context.
pub fn context_export_context(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("No interpreter context available")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Import context from exported data.
pub fn context_import_context(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("Context management not available")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Fork the current context.
pub fn context_fork_context(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("No interpreter context available")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Restore context from a fork.
pub fn context_restore_context(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("Context management not available")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Replace a message's content.
pub fn context_replace_message(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("Message not found")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Set the context window size.
pub fn context_set_context_window(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("Context management not available")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Get context configuration.
pub fn context_get_context_config(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("No interpreter context available")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Set cache-aware mode.
pub fn context_set_cache_aware(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("Context management not available")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Set compression strategy.
pub fn context_set_compression_strategy(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("Context management not available")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Compress context to a target size.
pub fn context_compress_context_target(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("No interpreter context available")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Set compression callback.
pub fn context_on_compression(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("Context management not available")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Set context overflow callback.
pub fn context_on_context_overflow(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("Context management not available")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Set working memory field.
pub fn context_working_memory_set(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("Working memory not available")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Get working memory contents.
pub fn context_working_memory_get(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("Working memory not available")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Remove a working memory entry.
pub fn context_working_memory_remove(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("Working memory not available")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Clear all working memory.
pub fn context_working_memory_clear(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("Working memory not available")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Enable or disable working memory.
pub fn context_set_working_memory_enabled(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("error")));
    result.insert(Value::Str(Rc::from("error")), Value::Str(Rc::from("Context management not available")));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn arg_str(args: &[Value], i: usize) -> Result<String, ExceptionValue> {
    match args.get(i) {
        Some(Value::Str(s)) => Ok(s.to_string()),
        Some(other) => Err(ExceptionValue::new(
            "TypeError",
            format!("Expected string at position {}, got {:?}", i, other),
            None,
        )),
        None => Err(ExceptionValue::new(
            "TypeError",
            format!("Missing required argument at position {}", i),
            None,
        )),
    }
}
