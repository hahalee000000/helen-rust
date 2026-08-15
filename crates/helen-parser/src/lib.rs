//! helen-parser — Helen Pratt precedence parser + recursive descent.
//!
//! Byte-faithful port of `helen/core/parser.py` (v1.44.0). The parser
//! consumes the token stream produced by `helen-core::lexer::Scanner`
//! and produces a `helen-core::ast::Program`.

pub mod pratt;

pub use pratt::Parser;
