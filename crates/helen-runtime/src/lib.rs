//! helen-runtime — M8: context management (transcript, session).
//!
//! Byte-faithful port of `helen/runtime/{transcript_store,session_manager}.py`
//! core. M8 continues the M5 runtime crate with transcript storage (Task 8.1)
//! and session management (Task 8.5).

pub mod calc;
pub mod call_tracking;
pub mod channel;
pub mod compression;
pub mod config;
pub mod constants;
pub mod context_awareness;
pub mod context_recovery;
pub mod coverage;
pub mod data_lineage;
pub mod diagnostics;
pub mod fuzzy_match;
pub mod history;
pub mod http_llm;
pub mod llm;
pub mod mcp;
pub mod media;
pub mod memory;
pub mod model_caps;
pub mod observability;
pub mod prompt;
pub mod provider;
pub mod recording;
pub mod session;
pub mod skills;
pub mod sqlite_backend;
pub mod token;
pub mod tools;
pub mod transcript;
pub mod transcript_replay;
pub mod validator;
pub mod working_memory;

pub use session::SessionManager;
pub use tools::{
    dispatch_mcp_tool, dispatch_tool, ensure_mcp_initialized, get_mcp_tool_schemas,
    get_tool_schemas, initialize_mcp, shutdown_mcp, tools_dispatch,
};
pub use transcript::{BoundaryMarker, Item, JsonlBackend, Message, SessionMeta, TranscriptStore};
