//! M13 Task 13.8 — stdlib surface driver for coverage.
//!
//! Calls every exported stdlib builtin with generic argument patterns to
//! drive argument-matching, coercion, and error paths in `stdlib.rs`
//! (line coverage gate: ≥85% core+parser+interpreter).
//!
//! This is a coverage driver, not a behavioral conformance suite — the
//! behavioral parity lives in `execution_tests.rs`, the Tier-A diff, and
//! the corpus tests. Here we only require: no panics, and each builtin
//! accepts at least the arity-0 or arity-1 generic patterns without
//! unwinding the process.

use std::cell::RefCell;
use std::rc::Rc;

use helen_core::ast::TypeRef;
use helen_core::source::SourceSpan;
use helen_interpreter::exceptions::ExceptionValue;
use helen_interpreter::interpreter::Interpreter;
use helen_interpreter::stdlib::{module_exports, module_tag};
use helen_interpreter::value::Value;

/// Generic argument patterns fed to every builtin.
fn generic_args() -> Vec<Vec<Value>> {
    let list_val = Value::List(Rc::new(RefCell::new(vec![
        Value::Int(1.into()),
        Value::Str(Rc::from("two")),
    ])));
    let map_val = Value::Map(Rc::new(RefCell::new(
        vec![(
            Value::Str(Rc::from("k")),
            Value::Int(42.into()),
        )]
        .into_iter()
        .collect(),
    )));
    vec![
        vec![],                                     // arity 0
        vec![Value::Int(7.into())],                 // int
        vec![Value::Float(1.5)],                    // float
        vec![Value::Str(Rc::from("hello"))],        // str
        vec![Value::Bool(true)],                    // bool
        vec![Value::Null],                          // null
        vec![list_val.clone()],                     // list
        vec![map_val.clone()],                      // map
        vec![Value::Int(0.into()), Value::Str(Rc::from("x"))], // pair
    ]
}

/// Builtins that must NOT be driven with generic args — they run real
/// subprocesses/signals on generic inputs (e.g. `kill 0` would SIGTERM the
/// test runner's own process group; `exec`/`exec_async` spawn arbitrary
/// programs), or block for a long time on generic inputs (`sleep` with an
/// int arg sleeps seconds; `mailbox_select` polls channels).
/// These are covered behaviorally in execution/corpus tests.
const DANGEROUS: &[&str] = &["kill", "exec", "exec_async", "sleep", "mailbox_select"];

/// A single module's exports driven with generic args. Returns
/// (name, panicked, errored_count, ok_count).
fn drive_exports(module: &str) -> Vec<(String, bool, usize, usize)> {
    let mut out = Vec::new();
    if let Some(exports) = module_exports(module) {
        for e in exports {
            if DANGEROUS.contains(&e.name) {
                continue;
            }
            let mut panicked = false;
            let mut ok = 0usize;
            let mut err = 0usize;
            for args in generic_args() {
                let mut interp = Interpreter::new();
                #[allow(clippy::result_large_err)]
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    (e.func)(&mut interp, &args)
                }));
                match result {
                    Ok(Ok(_)) => ok += 1,
                    Ok(Err(_)) => err += 1,
                    Err(_) => panicked = true,
                }
            }
            out.push((e.name.to_string(), panicked, err, ok));
        }
    }
    out
}

#[test]
fn stdlib_surface_no_panics() {
    // std.core is special: its exports live in the interpreter's builtins
    // map (CORE_EXPORTS), not in module_exports. All other modules are
    // table-driven via module_exports.
    //
    // std.network / std.llm / std.media / std.transcript are excluded:
    // their builtins make real HTTP/LLM/media/transcript calls and would
    // block (or require a live backend) on generic arguments. They are
    // covered behaviorally by the Tier-A corpus and execution tests.
    let modules: Vec<&str> = [
        "std.str",
        "std.list",
        "std.dict",
        "std.math",
        "std.data",
        "std.time",
        "std.crypto",
        "std.path",
        "std.io",
        "std.file",
        "std.system",
        "std.debug",
        "std.context",
        "std.quality",
        "std.test",
        "std.tools",
        "std.concurrency",
    ]
    .to_vec();

    let mut panicked: Vec<String> = Vec::new();
    let mut total_ok = 0usize;
    let mut total_err = 0usize;
    let mut total_exports = 0usize;

    for m in &modules {
        for (name, p, errs, oks) in drive_exports(m) {
            total_exports += 1;
            total_ok += oks;
            total_err += errs;
            if p {
                panicked.push(format!("{m}.{name}"));
            }
            // Progress log (only for the few modules that may side-effect).
            if matches!(*m, "std.system" | "std.io" | "std.network" | "std.file") {
                eprintln!("surface {m}.{name}: ok={oks} err={errs} panicked={p}");
            }
        }
    }

    // Coverage driver sanity: we must have driven a meaningful surface.
    assert!(total_exports >= 50, "only {total_exports} exports driven");
    assert!(
        total_ok + total_err > 0,
        "no builtin calls executed (surface driver is dead)"
    );

    // The hard guarantee: no builtin may unwind the process.
    assert!(
        panicked.is_empty(),
        "panicking builtins: {}",
        panicked.join(", ")
    );
    eprintln!(
        "stdlib surface: {total_exports} exports, {total_ok} ok calls, {total_err} error calls"
    );
}

#[test]
fn module_tags_resolve() {
    // module_tag must round-trip for every module module_exports knows
    // (std.core is handled specially via CORE_EXPORTS/builtins; the
    // blocking modules network/llm/media/transcript are still listed here
    // since module_tag must resolve them regardless of driving).
    let modules = [
        "std.str", "std.list", "std.dict", "std.math",
        "std.data", "std.time", "std.crypto", "std.path", "std.io",
        "std.file", "std.system", "std.network", "std.debug", "std.context",
        "std.quality", "std.test", "std.tools", "std.llm", "std.media",
        "std.transcript", "std.concurrency",
    ];
    for m in modules {
        let tag = module_tag(m);
        assert!(!tag.is_empty(), "module_tag({m}) empty");
        assert!(
            module_exports(m).is_some(),
            "module_exports({m}) unexpectedly absent"
        );
    }
}

/// Helper to keep imports used (TypeRef/SourceSpan/ExceptionValue appear
/// only in coverage-relevant signatures on the interpreter side).
#[allow(dead_code)]
fn _keep_imports(_t: Option<&TypeRef>, _s: &SourceSpan, _e: &ExceptionValue) {}
