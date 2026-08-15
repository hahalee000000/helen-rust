//! Source position tracking.

use std::fmt;

/// A byte-offset span within a source file, plus the 1-based line/column
/// of the span start. Mirrors the Python `SourceSpan` used by the reference
/// interpreter so diagnostics can be normalized across both implementations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpan {
    /// Byte offset of the first character of the span.
    pub start: usize,
    /// Byte offset one past the last character of the span.
    pub end: usize,
    /// 1-based line number of the span start.
    pub line: u32,
    /// 1-based column number (in characters) of the span start.
    pub col: u32,
}

impl SourceSpan {
    pub fn new(start: usize, end: usize, line: u32, col: u32) -> Self {
        SourceSpan {
            start,
            end,
            line,
            col,
        }
    }
}

impl fmt::Display for SourceSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.col)
    }
}
