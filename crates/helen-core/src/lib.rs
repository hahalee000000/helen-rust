//! helen-core — foundational primitives shared by every other crate.
//!
//! The Rust side of the differential harness normalizes both interpreters'
//! diagnostics through `SourceSpan` and `HelenCompileError`. Nothing in this
//! crate depends on the Python implementation.

pub mod errors;
pub mod source;
