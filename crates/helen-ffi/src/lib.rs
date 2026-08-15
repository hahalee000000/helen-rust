//! helen-ffi — Python FFI for the Helen language (M10).
//!
//! Lets Helen programs `import "numpy" as np` / `import "requests" as req`
//! and call Python from the Rust interpreter (D9: embed CPython via PyO3).
//!
//! Port of `helen/ffi/*`:
//! - `python_runtime.py` → [`runtime::PythonRuntime`]
//! - `python_module.py` → [`module::PythonModule`]
//! - `python_object.py` → [`object::PythonObject`]
//! - `type_converter.py` → [`converter`]
//! - `contracts.py` → trait surface (see [`helen_interpreter::native`])
//!
//! The crate is **feature-gated**: without `python-ffi` it compiles to an
//! empty lib (pure-Rust builds need no Python headers). With the feature it
//! embeds CPython. Default workspace builds are unaffected.

#![cfg_attr(not(feature = "python-ffi"), allow(dead_code, unused_imports))]

#[cfg(feature = "python-ffi")]
pub mod converter;
#[cfg(feature = "python-ffi")]
pub mod custom_provider;
#[cfg(feature = "python-ffi")]
pub mod module;
#[cfg(feature = "python-ffi")]
pub mod object;
#[cfg(feature = "python-ffi")]
pub mod runtime;

#[cfg(feature = "python-ffi")]
pub use runtime::PythonRuntime;

/// Install the Python FFI runtime into the interpreter (import hook + custom
/// provider loader). Call once at process startup when the `python-ffi`
/// feature is enabled.
#[cfg(feature = "python-ffi")]
pub fn install() -> Result<(), String> {
    use std::sync::OnceLock;
    static RUNTIME: OnceLock<PythonRuntime> = OnceLock::new();
    let runtime = RUNTIME.get_or_init(|| PythonRuntime::new().expect("Python runtime init"));
    // Load custom LLM providers from ~/.helen/providers/*.py (M5.3).
    custom_provider::load_custom_providers();
    runtime::install_python_hook(runtime)
}

#[cfg(not(feature = "python-ffi"))]
pub fn install() -> Result<(), String> {
    Err("helen-ffi built without the `python-ffi` feature".to_string())
}
