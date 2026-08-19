//! Advanced interpreter tests — targeting uncovered code paths.
//!
//! Focus: match statements, access/index, lambda closures,
//! agent declarations, and more complex control flow.

use helen_core::lexer::Scanner;
use helen_interpreter::exceptions::ExceptionValue;
use helen_interpreter::interpreter::Interpreter;
use helen_interpreter::value::Value;
use helen_parser::Parser;
use num_bigint::BigInt;

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

fn run_err(src: &str) -> ExceptionValue {
    match run_src(src).0 {
        Err(e) => e,
        Ok(v) => panic!("expected error, got {:?}", v),
    }
}

// ── Match statements ────────────────────────────────────────────────────

#[test]
fn match_literal_int() {
    let (r, _) = run_src("main {\n let x = 5\n match x {\n  case 5 { 10 }\n  default { 20 }\n }\n}");
    assert_eq!(r.unwrap(), Some(int(10)));
}

#[test]
fn match_default_case() {
    let (r, _) = run_src("main {\n let x = 99\n match x {\n  case 1 { 10 }\n  default { 20 }\n }\n}");
    assert_eq!(r.unwrap(), Some(int(20)));
}

#[test]
fn match_string_literal() {
    let (r, _) = run_src(r#"main {
 let x = "hello"
 match x {
  case "hello" { 1 }
  case "world" { 2 }
  default { 3 }
 }
}"#);
    assert_eq!(r.unwrap(), Some(int(1)));
}

#[test]
fn match_bool() {
    let (r, _) = run_src("main {\n let x = true\n match x {\n  case true { 1 }\n  case false { 2 }\n }\n}");
    assert_eq!(r.unwrap(), Some(int(1)));
}

#[test]
fn match_no_match_returns_null() {
    let (r, _) = run_src("main {\n let x = 5\n match x {\n  case 1 { 10 }\n  case 2 { 20 }\n }\n}");
    assert_eq!(r.unwrap(), None);
}

#[test]
fn match_multiple_cases() {
    let (r, _) = run_src("main {\n let x = 3\n match x {\n  case 1 { 10 }\n  case 2 { 20 }\n  case 3 { 30 }\n  default { 40 }\n }\n}");
    assert_eq!(r.unwrap(), Some(int(30)));
}

// ── Index access ────────────────────────────────────────────────────────

#[test]
fn list_index_access() {
    let (r, _) = run_src("main {\n let xs = [10, 20, 30]\n xs[1]\n}");
    assert_eq!(r.unwrap(), Some(int(20)));
}

#[test]
fn list_negative_index() {
    let (r, _) = run_src("main {\n let xs = [10, 20, 30]\n xs[-1]\n}");
    assert_eq!(r.unwrap(), Some(int(30)));
}

#[test]
fn list_index_out_of_bounds() {
    let e = run_err("main {\n let xs = [10, 20]\n xs[5]\n}");
    assert_eq!(e.class_name, "RuntimeError");
}

#[test]
fn map_key_access() {
    let (r, _) = run_src(r#"main {
 let m = {"a": 1, "b": 2}
 m["a"]
}"#);
    assert_eq!(r.unwrap(), Some(int(1)));
}

#[test]
fn map_missing_key() {
    let e = run_err(r#"main {
 let m = {"a": 1}
 m["z"]
}"#);
    assert_eq!(e.class_name, "RuntimeError");
}

// ── Dot access ──────────────────────────────────────────────────────────

#[test]
fn map_dot_access() {
    let (r, _) = run_src(r#"main {
 let m = {"x": 42}
 m.x
}"#);
    assert_eq!(r.unwrap(), Some(int(42)));
}

// ── Lambda / closures ───────────────────────────────────────────────────

#[test]
fn lambda_basic() {
    let (r, _) = run_src("main {\n let f = fn(x) { return x * 2 }\n f(5)\n}");
    assert_eq!(r.unwrap(), Some(int(10)));
}

#[test]
fn lambda_capture() {
    let (r, _) = run_src("main {\n let y = 10\n let f = fn(x) { return x + y }\n f(5)\n}");
    assert_eq!(r.unwrap(), Some(int(15)));
}

#[test]
fn lambda_multiple_params() {
    let (r, _) = run_src("main {\n let f = fn(a, b) { return a + b }\n f(3, 4)\n}");
    assert_eq!(r.unwrap(), Some(int(7)));
}

#[test]
fn lambda_no_params() {
    let (r, _) = run_src("main {\n let f = fn() { return 42 }\n f()\n}");
    assert_eq!(r.unwrap(), Some(int(42)));
}

#[test]
fn lambda_higher_order() {
    let (r, _) = run_src("main {\n fn apply(f, x) { return f(x) }\n fn double(n) { return n * 2 }\n apply(double, 5)\n}");
    assert_eq!(r.unwrap(), Some(int(10)));
}

// ── Agent declarations ──────────────────────────────────────────────────

#[test]
fn agent_basic_declaration() {
    let (r, _) = run_src(r#"
agent Greeter(name: str) {
    description "A simple greeter"
    prompt "Hello!"
    main {
        return "greeted"
    }
}
main {
    "done"
}
"#);
    assert!(r.is_ok());
}

// ── Complex control flow ────────────────────────────────────────────────

#[test]
fn nested_if_else() {
    let (r, _) = run_src("main {\n let x = 5\n if x > 3 {\n  if x > 4 {\n   1\n  } else {\n   2\n  }\n } else {\n  3\n }\n}");
    assert_eq!(r.unwrap(), Some(int(1)));
}

#[test]
fn for_loop_with_break() {
    let (r, _) = run_src("main {\n let sum = 0\n for i in [1, 2, 3, 4, 5] {\n  if i == 4 {\n   break\n  }\n  sum = sum + i\n }\n sum\n}");
    assert_eq!(r.unwrap(), Some(int(6)));
}

#[test]
fn for_loop_with_continue() {
    let (r, _) = run_src("main {\n let sum = 0\n for i in [1, 2, 3, 4, 5] {\n  if i == 3 {\n   continue\n  }\n  sum = sum + i\n }\n sum\n}");
    assert_eq!(r.unwrap(), Some(int(12)));
}

#[test]
fn while_loop() {
    let (r, _) = run_src("main {\n let i = 0\n let sum = 0\n while i < 5 {\n  sum = sum + i\n  i = i + 1\n }\n sum\n}");
    assert_eq!(r.unwrap(), Some(int(10)));
}

#[test]
fn nested_loops() {
    let (r, _) = run_src("main {\n let count = 0\n for i in [1, 2] {\n  for j in [1, 2, 3] {\n   count = count + 1\n  }\n }\n count\n}");
    assert_eq!(r.unwrap(), Some(int(6)));
}

// ── Try-catch ───────────────────────────────────────────────────────────

#[test]
fn try_catch_basic() {
    let (r, _) = run_src(r#"main {
 try {
  throw RuntimeError("boom")
 } catch RuntimeError e {
  42
 }
}"#);
    assert_eq!(r.unwrap(), Some(int(42)));
}

#[test]
fn try_catch_no_error() {
    let (r, _) = run_src("main {\n try {\n  10\n } catch RuntimeError e {\n  20\n }\n}");
    assert_eq!(r.unwrap(), Some(int(10)));
}

// ── Assert ──────────────────────────────────────────────────────────────

#[test]
fn assert_true_passes() {
    let (r, _) = run_src("main {\n assert true\n 42\n}");
    assert_eq!(r.unwrap(), Some(int(42)));
}

#[test]
fn assert_false_fails() {
    let e = run_err("main {\n assert false\n}");
    assert_eq!(e.class_name, "AssertionError");
}

#[test]
fn assert_with_message() {
    let e = run_err(r#"main {
 assert false, "custom message"
}"#);
    assert_eq!(e.class_name, "AssertionError");
}

// ── String operations ───────────────────────────────────────────────────

#[test]
fn string_concat() {
    let (r, _) = run_src(r#"main {
 "hello" + " " + "world"
}"#);
    assert!(r.is_ok());
}

#[test]
fn string_index() {
    let (r, _) = run_src(r#"main {
 let s = "hello"
 s[1]
}"#);
    assert!(r.is_ok());
}

// ── List operations ─────────────────────────────────────────────────────

#[test]
fn list_pop() {
    let (r, _) = run_src("main {\n let xs = [1, 2, 3]\n xs.pop()\n}");
    assert_eq!(r.unwrap(), Some(int(3)));
}

// ── Map operations ──────────────────────────────────────────────────────

#[test]
fn map_insert() {
    let (r, _) = run_src(r#"main {
 let m = {"a": 1}
 m["b"] = 2
 m["b"]
}"#);
    assert_eq!(r.unwrap(), Some(int(2)));
}

// ── Stdlib with import ──────────────────────────────────────────────────

#[test]
fn stdlib_print() {
    let (r, out) = run_src("import std.core.*\nmain {\n print(\"hello\")\n}");
    assert!(r.is_ok());
    assert!(out.contains("hello"));
}

#[test]
fn stdlib_len() {
    let (r, _) = run_src("import std.core.*\nmain {\n len([1, 2, 3])\n}");
    assert!(r.is_ok());
}

#[test]
fn stdlib_range() {
    let (r, _out) = run_src("import std.core.*\nmain {\n print(range(3))\n}");
    assert!(r.is_ok());
}

#[test]
fn stdlib_abs() {
    let (r, _) = run_src("import std.core.*\nmain {\n abs(-5)\n}");
    assert_eq!(r.unwrap(), Some(int(5)));
}

#[test]
fn stdlib_type() {
    let (r, _out) = run_src("import std.core.*\nmain {\n print(type(42))\n}");
    assert!(r.is_ok());
}

// ── Math module ─────────────────────────────────────────────────────────

#[test]
fn math_sqrt() {
    let (r, _) = run_src("import std.math.*\nmain {\n sqrt(16)\n}");
    assert!(r.is_ok());
}

#[test]
fn math_pow() {
    let (r, _) = run_src("import std.math.*\nmain {\n pow(2, 3)\n}");
    assert!(r.is_ok());
}

#[test]
fn math_floor() {
    let (r, _) = run_src("import std.math.*\nmain {\n floor(3.7)\n}");
    assert!(r.is_ok());
}

#[test]
fn math_ceil() {
    let (r, _) = run_src("import std.math.*\nmain {\n ceil(3.2)\n}");
    assert!(r.is_ok());
}

// ── Time module ─────────────────────────────────────────────────────────

#[test]
fn time_now() {
    let (r, _) = run_src("import std.time.*\nmain {\n now()\n}");
    assert!(r.is_ok());
}

#[test]
fn time_sleep() {
    let (r, _) = run_src("import std.time.*\nmain {\n sleep(0)\n 42\n}");
    assert_eq!(r.unwrap(), Some(int(42)));
}
