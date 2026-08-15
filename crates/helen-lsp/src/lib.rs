//! helen-lsp — Helen Language Server Protocol implementation (M12).
//!
//! Port of `helen/lsp/server.py` (1,375 lines): JSON-RPC 2.0 over stdio,
//! diagnostics, completion, go-to-definition, references, hover, document
//! symbols.

pub mod constants;
pub mod server;

pub use constants::*;
pub use server::*;

/// Version reported in `serverInfo` (mirrors `helen.__version__`).
pub const VERSION: &str = "1.45.0";
