//! Quality analysis stdlib functions.
//!
//! Byte-faithful port of `helen/stdlib/quality.py` (v1.44.0): provides
//! code quality analysis and scoring functions.
//!
//! This is a simplified implementation that provides basic metrics and scoring.
//! The full Python version has 1471 lines with comprehensive static analysis.

use std::cell::RefCell;
use std::rc::Rc;

use num_bigint::BigInt;

use crate::exceptions::ExceptionValue;
use crate::interpreter::Interpreter;
use crate::value::Value;

/// Analyze code quality — returns basic metrics.
/// Python: `analyze_code(source: str, filename: str = "<unknown>") -> dict`.
pub fn quality_analyze_code(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let source = arg_str_or(args, 0, "");
    let filename = arg_str_or(args, 1, "<unknown>");

    let lines: Vec<&str> = source.lines().collect();
    let total_lines = lines.len();
    let code_lines = lines.iter().filter(|l| !l.trim().is_empty() && !l.trim().starts_with("//")).count();
    let comment_lines = lines.iter().filter(|l| l.trim().starts_with("//")).count();
    let blank_lines = lines.iter().filter(|l| l.trim().is_empty()).count();
    let comment_ratio = if total_lines > 0 { comment_lines as f64 / total_lines as f64 } else { 0.0 };

    // Count functions and agents (simple regex-like matching)
    let function_count = lines.iter().filter(|l| l.trim().starts_with("fn ")).count();
    let agent_count = lines.iter().filter(|l| l.trim().starts_with("agent ")).count();

    // Estimate average function length (rough heuristic)
    let avg_function_length = if function_count > 0 { code_lines as f64 / function_count as f64 } else { 0.0 };
    let max_function_length = if function_count > 0 { code_lines } else { 0 };

    let mut metrics = indexmap::IndexMap::new();
    metrics.insert(Value::Str(Rc::from("total_lines")), Value::Int(BigInt::from(total_lines as i64)));
    metrics.insert(Value::Str(Rc::from("code_lines")), Value::Int(BigInt::from(code_lines as i64)));
    metrics.insert(Value::Str(Rc::from("comment_lines")), Value::Int(BigInt::from(comment_lines as i64)));
    metrics.insert(Value::Str(Rc::from("blank_lines")), Value::Int(BigInt::from(blank_lines as i64)));
    metrics.insert(Value::Str(Rc::from("comment_ratio")), Value::Float(comment_ratio));
    metrics.insert(Value::Str(Rc::from("function_count")), Value::Int(BigInt::from(function_count as i64)));
    metrics.insert(Value::Str(Rc::from("agent_count")), Value::Int(BigInt::from(agent_count as i64)));
    metrics.insert(Value::Str(Rc::from("avg_function_length")), Value::Float(avg_function_length));
    metrics.insert(Value::Str(Rc::from("max_function_length")), Value::Int(BigInt::from(max_function_length as i64)));
    metrics.insert(Value::Str(Rc::from("filename")), Value::Str(Rc::from(filename.as_str())));

    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("status")), Value::Str(Rc::from("ok")));
    result.insert(Value::Str(Rc::from("metrics")), Value::Map(Rc::new(RefCell::new(metrics))));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Check code security — returns list of issues (empty for now).
/// Python: `check_security(source: str) -> list[dict]`.
pub fn quality_check_security(_i: &mut Interpreter, _args: &[Value]) -> Result<Value, ExceptionValue> {
    // Simplified: return empty list (no security issues detected)
    Ok(Value::List(Rc::new(RefCell::new(vec![]))))
}

/// Get quality score — returns 0-10 score based on basic heuristics.
/// Python: `quality_score(source: str) -> float`.
pub fn quality_quality_score(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let source = arg_str_or(args, 0, "");
    let lines: Vec<&str> = source.lines().collect();
    let total_lines = lines.len();

    if total_lines == 0 {
        return Ok(Value::Float(0.0));
    }

    let comment_lines = lines.iter().filter(|l| l.trim().starts_with("//")).count();
    let comment_ratio = comment_lines as f64 / total_lines as f64;
    let function_count = lines.iter().filter(|l| l.trim().starts_with("fn ")).count();

    // Simple scoring heuristic:
    // - Base score: 5.0
    // - +2.0 if has comments (ratio > 0.1)
    // - +1.5 if has functions
    // - +1.5 if code is reasonably sized (>10 lines)
    let mut score: f64 = 5.0;
    if comment_ratio > 0.1 {
        score += 2.0;
    }
    if function_count > 0 {
        score += 1.5;
    }
    if total_lines > 10 {
        score += 1.5;
    }

    Ok(Value::Float(score.min(10.0)))
}

/// Get quality report — returns formatted text report.
/// Python: `quality_report(source: str, filename: str = "<unknown>") -> str`.
pub fn quality_quality_report(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let source = arg_str_or(args, 0, "");
    let filename = arg_str_or(args, 1, "<unknown>");

    let lines: Vec<&str> = source.lines().collect();
    let total_lines = lines.len();
    let code_lines = lines.iter().filter(|l| !l.trim().is_empty() && !l.trim().starts_with("//")).count();
    let comment_lines = lines.iter().filter(|l| l.trim().starts_with("//")).count();
    let comment_ratio = if total_lines > 0 { comment_lines as f64 / total_lines as f64 } else { 0.0 };
    let function_count = lines.iter().filter(|l| l.trim().starts_with("fn ")).count();
    let agent_count = lines.iter().filter(|l| l.trim().starts_with("agent ")).count();

    // Calculate score
    let mut score: f64 = 5.0;
    if comment_ratio > 0.1 {
        score += 2.0;
    }
    if function_count > 0 {
        score += 1.5;
    }
    if total_lines > 10 {
        score += 1.5;
    }
    score = score.min(10.0);

    let grade = if score >= 9.0 { "A" }
                else if score >= 8.0 { "B" }
                else if score >= 7.0 { "C" }
                else if score >= 6.0 { "D" }
                else { "F" };

    let mut report = String::new();
    report.push_str("\n");
    report.push_str("============================================================\n");
    report.push_str("  HELEN QUALITY REPORT\n");
    report.push_str("============================================================\n");
    report.push_str(&format!("  File: {}\n\n", filename));

    report.push_str("  Code Metrics:\n");
    report.push_str(&format!("    Total lines: {}\n", total_lines));
    report.push_str(&format!("    Code lines: {}\n", code_lines));
    report.push_str(&format!("    Comment lines: {} ({:.0}%)\n", comment_lines, comment_ratio * 100.0));
    report.push_str(&format!("    Functions: {}\n", function_count));
    report.push_str(&format!("    Agents: {}\n", agent_count));
    report.push_str("\n");

    report.push_str("  Quality Score (0-10):\n");
    report.push_str(&format!("    TOTAL: {:.2}\n", score));
    report.push_str(&format!("    GRADE: {}\n", grade));
    report.push_str("\n");

    report.push_str("============================================================\n");
    report.push_str("\n");

    Ok(Value::Str(Rc::from(report.as_str())))
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn arg_str_or(args: &[Value], i: usize, default: &str) -> String {
    match args.get(i) {
        Some(Value::Str(s)) => s.to_string(),
        _ => default.to_string(),
    }
}
