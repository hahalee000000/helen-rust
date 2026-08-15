//! Tool dispatch (Task 6.3 subset) — port of `helen/runtime/tools.py`
//! default `dispatch_tool` for the 11 built-in tools.
//!
//! M5 ships the dispatch entry point used by the LLM tool-calling loop; the
//! full tool implementations (fuzzy patch matching etc.) land with M6.

use serde_json::Value;

/// Dispatch a tool call by name. Returns the tool result string.
/// Mirrors Python `dispatch_tool(name, args) -> str`.
pub fn tools_dispatch(name: &str, args: &Value) -> Result<String, String> {
    match name {
        "calculate" => dispatch_calculate(args),
        _ => Err(format!(
            "Tool '{name}' is not available in this runtime build (requires M6+ tool registry)"
        )),
    }
}

/// `calculate` — safe arithmetic evaluation via the workspace `calculate`
/// helper (reuses the interpreter's own evaluation when available).
fn dispatch_calculate(args: &Value) -> Result<String, String> {
    let expression = args
        .get("expression")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "calculate: missing 'expression' string argument".to_string())?;
    // Minimal safe evaluator: 4 basic ops + parens + common math functions.
    crate::calc::eval_simple(expression)
}
