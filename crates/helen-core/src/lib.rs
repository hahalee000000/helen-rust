//! helen-core — foundational primitives shared by every other crate.
//!
//! M1: source spans, error codes, tokens, and the scanner. All shapes are
//! byte-faithful to the Python reference (`helen/core/{source,errors,
//! tokens,lexer}.py`) so the differential harness can compare token
//! streams and diagnostics across both implementations.

pub mod ast;
pub mod ast_printer;
pub mod errors;
pub mod lexer;
pub mod source;
pub mod tokens;
