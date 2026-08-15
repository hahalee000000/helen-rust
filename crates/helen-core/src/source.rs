//! Source position tracking.
//!
//! Mirrors the Python `helen.core.source.SourceSpan` exactly: an immutable
//! (file, start_line, start_col, end_line, end_col) region, 1-based lines
//! and columns, `end_col` exclusive. `Display` is byte-identical to the
//! Python `__str__` so error messages normalize across implementations.

use std::fmt;

/// A region within a source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSpan {
    /// The source filename.
    pub file: String,
    /// 1-based starting line number.
    pub start_line: u32,
    /// 1-based starting column number.
    pub start_col: u32,
    /// 1-based ending line number (inclusive).
    pub end_line: u32,
    /// 1-based ending column number (exclusive).
    pub end_col: u32,
}

impl SourceSpan {
    /// Construct a span from the same positional arguments as Python.
    pub fn new(
        file: impl Into<String>,
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
    ) -> Self {
        SourceSpan {
            file: file.into(),
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    /// Whether the (line, col) position falls within this span
    /// (inclusive start, exclusive end) — mirrors `SourceSpan.contains`.
    pub fn contains(&self, line: u32, col: u32) -> bool {
        if line < self.start_line || line > self.end_line {
            return false;
        }
        if line == self.start_line && col < self.start_col {
            return false;
        }
        if line == self.end_line && col >= self.end_col {
            return false;
        }
        true
    }
}

impl fmt::Display for SourceSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.start_line == self.end_line {
            write!(
                f,
                "{}:{}:{}-{}",
                self.file, self.start_line, self.start_col, self.end_col
            )
        } else {
            write!(
                f,
                "{}:{}:{}-{}:{}",
                self.file, self.start_line, self.start_col, self.end_line, self.end_col
            )
        }
    }
}
