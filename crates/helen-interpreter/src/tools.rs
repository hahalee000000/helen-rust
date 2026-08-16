//! Tool stdlib functions (web_search, web_fetch, shell_exec, etc.).
//!
//! Byte-faithful port of `helen/stdlib/tools.py` (v1.44.0): provides
//! agent tool functions for web access, shell execution, etc.

use std::cell::RefCell;
use std::rc::Rc;

use num_bigint::BigInt;

use crate::exceptions::ExceptionValue;
use crate::interpreter::Interpreter;
use crate::value::Value;

/// Search the web.
pub fn tools_web_search(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let _query = arg_str(args, 0)?;
    // Stub: web search requires external API integration
    Err(ExceptionValue::new(
        "RuntimeError",
        "web_search is not available in this runtime build".to_string(),
        None,
    ))
}

/// Fetch a web page.
pub fn tools_web_fetch(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let _url = arg_str(args, 0)?;
    // Stub: web fetch requires external API integration
    Err(ExceptionValue::new(
        "RuntimeError",
        "web_fetch is not available in this runtime build".to_string(),
        None,
    ))
}

/// Execute a shell command.
pub fn tools_shell_exec(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let _command = arg_str(args, 0)?;
    // Stub: shell execution requires security considerations
    Err(ExceptionValue::new(
        "RuntimeError",
        "shell_exec is not available in this runtime build".to_string(),
        None,
    ))
}

/// Calculate a mathematical expression.
pub fn tools_calculate(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let _expression = arg_str(args, 0)?;
    // Stub: expression evaluation requires safe parsing
    Err(ExceptionValue::new(
        "RuntimeError",
        "calculate is not available in this runtime build".to_string(),
        None,
    ))
}

/// Patch a file.
pub fn tools_patch_file(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    // Stub: file patching requires careful implementation
    Err(ExceptionValue::new(
        "RuntimeError",
        "patch_file is not available in this runtime build".to_string(),
        None,
    ))
}

/// Load a skill.
pub fn tools_load_skill(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let _skill_name = arg_str(args, 0)?;
    // Stub: skill loading requires skill registry
    Err(ExceptionValue::new(
        "RuntimeError",
        "load_skill is not available in this runtime build".to_string(),
        None,
    ))
}

/// List skill references.
pub fn tools_list_skill_references(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let _skill_name = arg_str(args, 0)?;
    // Stub: skill registry not yet implemented
    Ok(Value::List(Rc::new(RefCell::new(vec![]))))
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
