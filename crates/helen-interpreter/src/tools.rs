//! Tool stdlib functions (web_search, web_fetch, shell_exec, etc.).
//!
//! Byte-faithful port of `helen/stdlib/tools.py` (v1.44.0): delegates to
//! `helen_runtime::tools::dispatch_tool` for real implementations.

use std::rc::Rc;

use crate::exceptions::ExceptionValue;
use crate::interpreter::Interpreter;
use crate::stdlib::json_to_value;
use crate::value::Value;

// ---------------------------------------------------------------------------
// Helpers
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

fn arg_int_or(args: &[Value], i: usize, default: i64) -> i64 {
    match args.get(i) {
        Some(Value::Int(n)) => {
            let s = n.to_string();
            s.parse::<i64>().unwrap_or(default)
        }
        Some(Value::Bool(b)) => if *b { 1 } else { 0 },
        _ => default,
    }
}

fn arg_bool_or(args: &[Value], i: usize, default: bool) -> bool {
    match args.get(i) {
        Some(Value::Bool(b)) => *b,
        Some(Value::Int(n)) => {
            let s = n.to_string();
            s.parse::<i64>().unwrap_or(0) != 0
        }
        _ => default,
    }
}

/// Call `helen_runtime::tools::dispatch_tool(name, args_json)` and return
/// the result as a Helen `Value::Str` (byte-faithful to Python which returns
/// the raw string from the tool handler).
fn dispatch(name: &str, args_json: serde_json::Value) -> Result<Value, ExceptionValue> {
    let result = helen_runtime::tools::dispatch_tool(name, &args_json);
    Ok(Value::Str(Rc::from(result.as_str())))
}

// ---------------------------------------------------------------------------
// Tool wrappers — byte-faithful port of helen/stdlib/tools.py
// ---------------------------------------------------------------------------

/// Search the web.
/// Python: `_web_search(query, num_results=3)` → `helen.runtime.tools._web_search`
pub fn tools_web_search(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let query = arg_str(args, 0)?;
    let num_results = arg_int_or(args, 1, 3);
    dispatch(
        "web_search",
        serde_json::json!({
            "query": query,
            "num_results": num_results,
        }),
    )
}

/// Fetch a web page.
/// Python: `_web_fetch(url)` → `helen.runtime.tools._web_fetch`
pub fn tools_web_fetch(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let url = arg_str(args, 0)?;
    dispatch(
        "web_fetch",
        serde_json::json!({
            "url": url,
        }),
    )
}

/// Execute a shell command.
/// Python: `_shell_exec(command, timeout=30, shell=True)` → raw stdout string
pub fn tools_shell_exec(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let command = arg_str(args, 0)?;
    let timeout = arg_int_or(args, 1, 30);
    let shell = arg_bool_or(args, 2, true);
    dispatch(
        "shell_exec",
        serde_json::json!({
            "command": command,
            "timeout": timeout,
            "shell": shell,
        }),
    )
}

/// Calculate a mathematical expression.
/// Python: `_calculate(expression)` → JSON string with result
pub fn tools_calculate(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let expression = arg_str(args, 0)?;
    dispatch(
        "calculate",
        serde_json::json!({
            "expression": expression,
        }),
    )
}

/// Patch a file using fuzzy matching.
/// Python: `_patch_file(path, old_string, new_string, replace_all=False)`
pub fn tools_patch_file(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    let old_string = arg_str(args, 1)?;
    let new_string = arg_str(args, 2)?;
    let replace_all = arg_bool_or(args, 3, false);
    dispatch(
        "patch_file",
        serde_json::json!({
            "path": path,
            "old_string": old_string,
            "new_string": new_string,
            "replace_all": replace_all,
        }),
    )
}

/// Load a skill by name.
/// Python: `_load_skill(name, include_references=False)`
pub fn tools_load_skill(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let name = arg_str(args, 0)?;
    let include_references = arg_bool_or(args, 1, false);
    dispatch(
        "load_skill",
        serde_json::json!({
            "name": name,
            "include_references": include_references,
        }),
    )
}

/// List skill references.
/// Python: `_list_skill_references(name)`
pub fn tools_list_skill_references(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let name = arg_str(args, 0)?;
    let result = helen_runtime::tools::dispatch_tool(
        "list_skill_references",
        &serde_json::json!({ "name": name }),
    );
    // Parse JSON result and convert to Helen Value
    match serde_json::from_str::<serde_json::Value>(&result) {
        Ok(json) => Ok(json_to_value(&json)),
        Err(_) => Ok(Value::Str(Rc::from(result.as_str()))),
    }
}

/// Read the content of a local file.
/// Python: `_read_file(path)`
pub fn tools_read_file(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    dispatch("read_file", serde_json::json!({ "path": path }))
}

/// Write content to a local file.
/// Python: `_write_file(path, content)`
pub fn tools_write_file(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    let content = arg_str(args, 1)?;
    dispatch("write_file", serde_json::json!({ "path": path, "content": content }))
}

/// Execute a shell command and return full result as JSON.
/// Python: `_shell_exec_full(command, timeout=30, shell=True)`
pub fn tools_shell_exec_full(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let command = arg_str(args, 0)?;
    let timeout = arg_int_or(args, 1, 30);
    let shell = arg_bool_or(args, 2, true);
    dispatch(
        "shell_exec_full",
        serde_json::json!({
            "command": command,
            "timeout": timeout,
            "shell": shell,
        }),
    )
}
