//! Python ↔ Helen value conversion for the bridge direction (Task 11.5).
//!
//! Reuses `helen_ffi::converter` (M10, 22 tests green): Python → Helen maps
//! primitives, list/tuple → list, dict → map, and wraps complex objects as
//! `PythonObject` natives; Helen → Python unwraps `PythonObject` natives back
//! to the original `PyObject` (identity-preserving).
//!
//! Divergence from `helen/python_bridge/type_converter.py` (documented):
//! the reference stringifies unknown objects (`str(value)`) and sorts sets;
//! the M10 converter wraps them as natives instead (strictly more capable —
//! attribute access and method calls work through the FFI wrapper).

pub use helen_ffi::converter::{helen_to_python, python_to_helen};
