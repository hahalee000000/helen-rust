//! helen-interpreter — Helen language runtime (M3).
//!
//! Byte-faithful port of `helen/interpreter/{interpreter,environment,
//! exceptions,closure,pattern_mixin,exception_mixin,readonly_view,
//! shared_store,import_mixin}.py`.

#![allow(clippy::result_large_err)]

pub mod closure;
pub mod data_formats;
pub mod environment;
pub mod exceptions;
pub mod import_resolver;
pub mod interpreter;
pub mod llm_runtime;
pub mod native;
pub mod shared_store;
pub mod stdlib;
pub mod stdlib_helpers;
pub mod stdlib_string;
pub mod stdlib_list;
pub mod test_framework;
pub mod llm_control;
pub mod media;
pub mod context;
pub mod debug;
pub mod observability;
pub mod tools;
pub mod quality;
pub mod transcript;
pub mod value;

/// M12: embedded stdlib catalog (M4 Task 4.1 `stdlib_catalog.json`) for
/// docgen / LSP completion. Returns the raw JSON document.
pub fn stdlib_catalog_json() -> &'static str {
    include_str!("../stdlib_catalog.json")
}
