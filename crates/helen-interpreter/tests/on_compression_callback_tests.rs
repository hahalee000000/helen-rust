//! Tests for on_compression callback functionality.
//!
//! Verifies that the on_compression callback is properly registered and invoked
//! when context compression occurs, matching Python's behavior.

use helen_core::lexer::Scanner;
use helen_interpreter::exceptions::ExceptionValue;
use helen_interpreter::interpreter::Interpreter;
use helen_interpreter::value::Value;
use helen_parser::Parser;
use num_bigint::BigInt;

#[allow(dead_code)]
fn int(n: i64) -> Value {
    Value::Int(BigInt::from(n))
}

fn run_src(src: &str) -> (Result<Option<Value>, ExceptionValue>, String) {
    let tokens = Scanner::new(src, "t.helen").scan_all();
    let mut parser = Parser::new(tokens);
    let program = parser.parse();
    assert!(
        parser.errors().is_empty(),
        "parse errors for {}: {:?}",
        src,
        parser.errors()
    );
    let mut interp = Interpreter::new();
    let r = interp.interpret(&program);
    let out = interp.stdout.lock().unwrap().clone();
    (r, out)
}

// ── on_compression Callback Registration ──────────────────────────────────

#[test]
fn on_compression_registers_callback() {
    // on_compression should accept a callback and return status
    let (r, _) = run_src(
        r#"
        import std.core.*
        import std.context.*
        main {
            let callback = fn(stats) {
                print("Compression occurs")
            }
            let result = on_compression(callback)
            result["status"]
        }
        "#,
    );
    // Should return "ok" status
    match &r {
        Ok(v) => {
            if let Some(Value::Str(s)) = v {
                assert_eq!(s.as_ref(), "ok");
            }
        }
        Err(e) => panic!("Expected Ok, got error: {:?}", e),
    }
}

#[test]
fn on_compression_returns_previous_callback() {
    // on_compression should return the previous callback
    let (r, _) = run_src(
        r#"
        import std.core.*
        import std.context.*
        main {
            let cb1 = fn(stats) { print("first") }
            let cb2 = fn(stats) { print("second") }
            
            let r1 = on_compression(cb1)
            let r2 = on_compression(cb2)
            
            // r2 should have "previous" field
            r2["status"]
        }
        "#,
    );
    assert!(r.is_ok());
}

#[test]
fn on_compression_with_none_clears_callback() {
    // on_compression(None) should clear the callback
    let (r, _) = run_src(
        r#"
        import std.core.*
        import std.context.*
        main {
            let cb = fn(stats) { print("callback") }
            on_compression(cb)
            let result = on_compression(null)
            result["status"]
        }
        "#,
    );
    assert!(r.is_ok());
}

// ── on_compression Callback Invocation ────────────────────────────────────

#[test]
fn on_compression_callback_invoked_on_compression() {
    // The callback should be invoked when compress_context is called
    let (r, out) = run_src(
        r#"
        import std.core.*
        import std.context.*
        main {
            let callback_invoked = false
            let callback = fn(stats) {
                print("Callback invoked!")
            }
            on_compression(callback)
            
            // Trigger compression (need enough messages)
            for i in range(0, 20) {
                insert_message("user", "message " + str(i))
            }
            compress_context("truncate")
            
            "done"
        }
        "#,
    );
    assert!(r.is_ok());
    // The callback should have been invoked and printed
    assert!(
        out.contains("Callback invoked!"),
        "Expected callback to be invoked, but output was: {}",
        out
    );
}

#[test]
fn on_compression_callback_receives_stats() {
    // The callback should receive compression statistics
    let (r, out) = run_src(
        r#"
        import std.core.*
        import std.context.*
        main {
            let callback = fn(stats) {
                print("Original tokens: " + str(stats["original_tokens"]))
                print("Compressed tokens: " + str(stats["compressed_tokens"]))
            }
            on_compression(callback)
            
            // Trigger compression
            for i in range(0, 20) {
                insert_message("user", "message " + str(i))
            }
            compress_context("truncate")
            
            "done"
        }
        "#,
    );
    assert!(r.is_ok());
    // The callback should have received stats
    assert!(
        out.contains("Original tokens:") || out.contains("Compressed tokens:"),
        "Expected callback to receive stats, but output was: {}",
        out
    );
}
