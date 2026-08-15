//! Runtime exceptions and control-flow sentinels.
//!
//! Byte-faithful port of `helen/interpreter/exceptions.py` and
//! `helen/interpreter/exception_mixin.py` (v1.44.0).
//!
//! Reference corrections vs the M3 plan (05-interpreter-core.md):
//! - The plan claims `catch` matches by exact class name. The actual
//!   reference uses Python `isinstance` — a **hierarchy** match:
//!   `catch LLMError` catches `TimeoutError`/`ModelError`/etc.
//!   (verified empirically: `throw TimeoutError` is caught by
//!   `catch LLMError`). Ported faithfully here.
//! - `throw AgentError("name")` passes the message positionally into
//!   `agent_name` (Python quirk), producing the derived message
//!   `Agent 'name' failed` with span=None. Verified empirically.

use indexmap::IndexMap;

use helen_core::source::SourceSpan;

use crate::value::Value;

/// The 11 predefined Helen exception classes (HLD 3.6.4).
pub const PREDEFINED_EXCEPTIONS: [&str; 11] = [
    "AnyError",
    "LLMError",
    "TimeoutError",
    "ModelError",
    "PromptTooLongError",
    "AgentError",
    "LLMOutputContractError",
    "ToolError",
    "RuntimeError",
    "AssertionError",
    "AggregateError",
];

/// `resolve_exception`: is `type_name` a predefined Helen exception?
pub fn resolve_exception(type_name: &str) -> Option<&'static str> {
    PREDEFINED_EXCEPTIONS
        .iter()
        .copied()
        .find(|n| *n == type_name)
        .or_else(|| {
            // Python fallback: case-insensitive match
            PREDEFINED_EXCEPTIONS
                .iter()
                .copied()
                .find(|n| n.eq_ignore_ascii_case(type_name))
        })
}

/// Parent class in the Helen exception hierarchy (HLD 3.6.4).
///
/// AnyError is the root; everything else derives from it. The runtime
/// uses Python `isinstance` semantics for `catch` matching, so a catch
/// of `LLMError` matches all four LLM subclasses.
fn parent_class(class_name: &str) -> Option<&'static str> {
    match class_name {
        "TimeoutError"
        | "ModelError"
        | "PromptTooLongError"
        | "AgentError"
        | "LLMOutputContractError" => Some("LLMError"),
        "LLMError" | "ToolError" | "RuntimeError" | "AssertionError" | "AggregateError" => {
            Some("AnyError")
        }
        _ => None,
    }
}

/// `error_matches(exc, type_name)`: does an exception match a catch type?
/// Walks the exception's class hierarchy (isinstance semantics).
pub fn error_matches(exc: &ExceptionValue, type_name: &str) -> bool {
    if !PREDEFINED_EXCEPTIONS.contains(&type_name) {
        return false;
    }
    let mut current: Option<&str> = Some(&exc.class_name);
    while let Some(cls) = current {
        if cls == type_name {
            return true;
        }
        current = parent_class(cls);
    }
    false
}

/// A Helen runtime exception value (thrown or raised).
#[derive(Clone, Debug)]
pub struct ExceptionValue {
    /// Exact class name, e.g. `"TimeoutError"`.
    pub class_name: String,
    pub message: String,
    pub span: Option<SourceSpan>,
    /// Extra named fields (`err.agent_name`, `err.tokens_used`, ...).
    pub fields: IndexMap<String, Value>,
}

impl ExceptionValue {
    pub fn new(class_name: &str, message: String, span: Option<SourceSpan>) -> Self {
        ExceptionValue {
            class_name: class_name.to_string(),
            message,
            span,
            fields: IndexMap::new(),
        }
    }

    /// Default constructor message (Python `__init__.__defaults__[0]`).
    pub fn default_message(class_name: &str) -> String {
        match class_name {
            "AnyError" => "any error".into(),
            "LLMError" => "LLM error".into(),
            "TimeoutError" => "LLM call timed out".into(),
            "ModelError" => "model error".into(),
            "PromptTooLongError" => "prompt too long".into(),
            // AgentError / LLMOutputContractError defaults[0] is agent_name="".
            "AgentError" => String::new(),
            "LLMOutputContractError" => String::new(),
            "ToolError" => "tool error".into(),
            "RuntimeError" => "runtime error".into(),
            "AssertionError" => "assertion failed".into(),
            "AggregateError" => "aggregate error".into(),
            _ => format!("{class_name} thrown"),
        }
    }

    /// Python `str(exception)` — class-specific `__str__` port.
    pub fn to_display_string(&self) -> String {
        let loc = match &self.span {
            Some(sp) => format!(" at {sp}"),
            None => String::new(),
        };
        match self.class_name.as_str() {
            "AgentError" => format!("AgentError:{loc} {}", self.message),
            "LLMOutputContractError" => format!("LLMOutputContractError:{loc} {}", self.message),
            "AggregateError" => {
                let errs: Vec<String> = self
                    .fields
                    .get("errors")
                    .map(|v| {
                        if let Value::List(l) = v {
                            l.borrow()
                                .iter()
                                .map(|e| {
                                    if let Value::Exception(ex) = e {
                                        ex.to_display_string()
                                    } else {
                                        e.python_str()
                                    }
                                })
                                .collect()
                        } else {
                            vec![]
                        }
                    })
                    .unwrap_or_default();
                if !errs.is_empty() {
                    format!(
                        "AggregateError({} task(s) failed): {}",
                        errs.len(),
                        errs.join(", ")
                    )
                } else {
                    format!("AggregateError: {}", self.message)
                }
            }
            // Base HelenRuntimeError.__str__ hardcodes the "RuntimeError:"
            // prefix for every class that does not override __str__.
            _ => format!("RuntimeError:{loc} {}", self.message),
        }
    }
}

/// Control-flow sentinels (Python BreakSentinel/ContinueSentinel/ReturnSentinel).
#[derive(Clone, Debug)]
pub enum Flow {
    /// Normal statement result (Python returns the value or None).
    Normal(Option<Value>),
    Break,
    Continue,
    Return(Option<Value>),
}

/// Error raised on const reassignment (Python ConstAssignmentError).
#[derive(Clone, Debug)]
pub struct ConstAssignmentError {
    pub name: String,
    pub span: Option<SourceSpan>,
}

/// Error raised on scope-isolation violations (Python ScopeViolationError).
#[derive(Clone, Debug)]
pub struct ScopeViolationError {
    pub message: String,
    pub span: Option<SourceSpan>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predefined_exceptions_are_11() {
        assert_eq!(PREDEFINED_EXCEPTIONS.len(), 11);
        assert!(resolve_exception("RuntimeError").is_some());
        assert!(resolve_exception("ValueError").is_none()); // Python names invalid
                                                            // case-insensitive fallback
        assert!(resolve_exception("timeouterror").is_some());
    }

    #[test]
    fn error_matches_uses_hierarchy() {
        let te = ExceptionValue::new("TimeoutError", "slow".into(), None);
        assert!(error_matches(&te, "TimeoutError"));
        assert!(error_matches(&te, "LLMError")); // isinstance: TimeoutError ⊂ LLMError
        assert!(error_matches(&te, "AnyError"));
        assert!(!error_matches(&te, "ModelError"));
        assert!(!error_matches(&te, "RuntimeError"));
        assert!(!error_matches(&te, "ValueError")); // unknown -> False

        let rt = ExceptionValue::new("RuntimeError", "boom".into(), None);
        assert!(error_matches(&rt, "RuntimeError"));
        assert!(error_matches(&rt, "AnyError"));
        assert!(!error_matches(&rt, "LLMError"));
    }

    #[test]
    fn default_messages_match_python() {
        assert_eq!(
            ExceptionValue::default_message("TimeoutError"),
            "LLM call timed out"
        );
        assert_eq!(
            ExceptionValue::default_message("RuntimeError"),
            "runtime error"
        );
        assert_eq!(
            ExceptionValue::default_message("AssertionError"),
            "assertion failed"
        );
        assert_eq!(ExceptionValue::default_message("ToolError"), "tool error");
        assert_eq!(
            ExceptionValue::default_message("AggregateError"),
            "aggregate error"
        );
        assert_eq!(ExceptionValue::default_message("AgentError"), "");
    }

    #[test]
    fn display_string_matches_python() {
        let e = ExceptionValue::new("RuntimeError", "boom".into(), None);
        assert_eq!(e.to_display_string(), "RuntimeError: boom");
        let e2 = ExceptionValue::new("TimeoutError", "slow".into(), None);
        assert_eq!(e2.to_display_string(), "RuntimeError: slow");
        let e3 = ExceptionValue::new("AgentError", "Agent 'worker' failed".into(), None);
        assert_eq!(e3.to_display_string(), "AgentError: Agent 'worker' failed");
        let e4 = ExceptionValue::new(
            "LLMOutputContractError",
            "Agent 'a' output does not match contract: ".into(),
            None,
        );
        assert_eq!(
            e4.to_display_string(),
            "LLMOutputContractError: Agent 'a' output does not match contract: "
        );
        // span renders " at {span}"
        let sp = SourceSpan {
            file: "t.helen".into(),
            start_line: 4,
            start_col: 5,
            end_line: 4,
            end_col: 25,
        };
        let e5 = ExceptionValue::new("RuntimeError", "LLM call timed out".into(), Some(sp));
        assert_eq!(
            e5.to_display_string(),
            "RuntimeError: at t.helen:4:5-25 LLM call timed out"
        );
    }
}
