//! helen-semantic — semantic analysis (M2).
//!
//! Byte-faithful port of `helen/semantic/{types,symbols,type_utils,analyzer}.py`
//! (v1.44.0). Produces the same `ErrorCode` diagnostics as the Python
//! reference `SemanticAnalyzer`.

pub mod analyzer;
pub mod diagnostics;
pub mod stdlib;
pub mod symbols;
pub mod type_utils;
pub mod types;

pub use analyzer::{analyze_codes, analyze_messages, SemanticAnalyzer};
pub use diagnostics::{Diagnostic, ErrorReporter};
