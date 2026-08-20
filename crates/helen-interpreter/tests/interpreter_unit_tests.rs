//! Unit tests for interpreter.rs evaluation paths to improve coverage.
//! Targets: expression evaluation, control flow, function calls, error handling.

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

// === Arithmetic expressions ===

#[test]
fn test_eval_integer_literal() {
    let (r, _) = run_src("main { 42 }");
    assert_eq!(r.unwrap(), Some(int(42)));
}

#[test]
fn test_eval_float_literal() {
    let (r, _) = run_src("main { 3.14 }");
    assert!(r.is_ok());
}

#[test]
fn test_eval_string_literal() {
    let (r, _) = run_src("main { \"hello\" }");
    assert!(r.is_ok());
}

#[test]
fn test_eval_boolean_true() {
    let (r, _) = run_src("main { true }");
    assert!(r.is_ok());
}

#[test]
fn test_eval_boolean_false() {
    let (r, _) = run_src("main { false }");
    assert!(r.is_ok());
}

#[test]
fn test_eval_addition() {
    let (r, _) = run_src("main { 2 + 3 }");
    assert_eq!(r.unwrap(), Some(int(5)));
}

#[test]
fn test_eval_subtraction() {
    let (r, _) = run_src("main { 10 - 4 }");
    assert_eq!(r.unwrap(), Some(int(6)));
}

#[test]
fn test_eval_multiplication() {
    let (r, _) = run_src("main { 3 * 7 }");
    assert_eq!(r.unwrap(), Some(int(21)));
}

#[test]
fn test_eval_division() {
    let (r, _) = run_src("main { 15 / 3 }");
    assert_eq!(r.unwrap(), Some(int(5)));
}

#[test]
fn test_eval_modulo() {
    let (r, _) = run_src("main { 17 % 5 }");
    assert_eq!(r.unwrap(), Some(int(2)));
}

#[test]
fn test_eval_unary_minus() {
    let (r, _) = run_src("main { -42 }");
    assert_eq!(r.unwrap(), Some(int(-42)));
}

#[test]
fn test_eval_unary_not() {
    let (r, _) = run_src("main { !true }");
    assert!(r.is_ok());
}

// === Comparison expressions ===

#[test]
fn test_eval_equal() {
    let (r, _) = run_src("main { 5 == 5 }");
    assert!(r.is_ok());
}

#[test]
fn test_eval_not_equal() {
    let (r, _) = run_src("main { 5 != 6 }");
    assert!(r.is_ok());
}

#[test]
fn test_eval_less_than() {
    let (r, _) = run_src("main { 3 < 5 }");
    assert!(r.is_ok());
}

#[test]
fn test_eval_greater_than() {
    let (r, _) = run_src("main { 5 > 3 }");
    assert!(r.is_ok());
}

#[test]
fn test_eval_less_equal() {
    let (r, _) = run_src("main { 3 <= 5 }");
    assert!(r.is_ok());
}

#[test]
fn test_eval_greater_equal() {
    let (r, _) = run_src("main { 5 >= 3 }");
    assert!(r.is_ok());
}

// === Logical expressions ===

#[test]
fn test_eval_and() {
    let (r, _) = run_src("main { true && true }");
    assert!(r.is_ok());
}

#[test]
fn test_eval_or() {
    let (r, _) = run_src("main { true || false }");
    assert!(r.is_ok());
}

// === Variable assignment and access ===

#[test]
fn test_eval_let_assignment() {
    let (r, _) = run_src("main { let x = 42\n x }");
    assert_eq!(r.unwrap(), Some(int(42)));
}

#[test]
fn test_eval_let_reassignment() {
    let (r, _) = run_src("main { let x = 10\n x = 20\n x }");
    assert_eq!(r.unwrap(), Some(int(20)));
}

#[test]
fn test_eval_let_multiple_vars() {
    let (r, _) = run_src("main { let x = 5\n let y = 10\n x + y }");
    assert_eq!(r.unwrap(), Some(int(15)));
}

// === Control flow ===

#[test]
fn test_eval_if_true() {
    let (r, _) = run_src("main { if true { 42 } else { 0 } }");
    assert_eq!(r.unwrap(), Some(int(42)));
}

#[test]
fn test_eval_if_false() {
    let (r, _) = run_src("main { if false { 42 } else { 0 } }");
    assert_eq!(r.unwrap(), Some(int(0)));
}

#[test]
fn test_eval_if_without_else() {
    let (r, _) = run_src("main { if true { 42 } }");
    assert_eq!(r.unwrap(), Some(int(42)));
}

#[test]
fn test_eval_while_loop() {
    let (r, _) = run_src("main { let i = 0\n while i < 5 { i = i + 1 }\n i }");
    assert_eq!(r.unwrap(), Some(int(5)));
}

#[test]
fn test_eval_while_with_break() {
    let (r, _) = run_src("main { let i = 0\n while i < 10 { i = i + 1\n if i == 5 { break } }\n i }");
    assert_eq!(r.unwrap(), Some(int(5)));
}

#[test]
fn test_eval_while_with_continue() {
    let (r, _) = run_src("main { let sum = 0\n let i = 0\n while i < 10 { i = i + 1\n if i % 2 == 0 { continue }\n sum = sum + i }\n sum }");
    assert_eq!(r.unwrap(), Some(int(25)));
}

// === Function definitions and calls ===

#[test]
fn test_eval_function_definition() {
    let (r, _) = run_src("fn add(a, b) { a + b }\nmain { add(3, 4) }");
    assert_eq!(r.unwrap(), Some(int(7)));
}

#[test]
fn test_eval_function_with_return() {
    let (r, _) = run_src("fn double(x) { return x * 2 }\nmain { double(5) }");
    assert_eq!(r.unwrap(), Some(int(10)));
}

#[test]
fn test_eval_function_early_return() {
    let (r, _) = run_src("fn check(x) { if x > 10 { return \"big\" }\n return \"small\" }\nmain { check(15) }");
    assert!(r.is_ok());
}

#[test]
fn test_eval_function_recursive() {
    let (r, _) = run_src("fn factorial(n) { if n <= 1 { return 1 }\n return n * factorial(n - 1) }\nmain { factorial(5) }");
    assert_eq!(r.unwrap(), Some(int(120)));
}

// === Collections ===

#[test]
fn test_eval_list_literal() {
    let (r, _) = run_src("main { [1, 2, 3] }");
    assert!(r.is_ok());
}

#[test]
fn test_eval_list_index() {
    let (r, _) = run_src("main { let arr = [10, 20, 30]\n arr[1] }");
    assert_eq!(r.unwrap(), Some(int(20)));
}

#[test]
fn test_eval_list_append() {
    let (r, _) = run_src("main { let arr = [1, 2]\n arr.append(3)\n arr }");
    assert!(r.is_ok());
}

#[test]
fn test_eval_map_literal() {
    let (r, _) = run_src("main { {\"a\": 1, \"b\": 2} }");
    assert!(r.is_ok());
}

#[test]
fn test_eval_map_access() {
    let (r, _) = run_src("main { let m = {\"x\": 42}\n m[\"x\"] }");
    assert_eq!(r.unwrap(), Some(int(42)));
}

// === String operations ===

#[test]
fn test_eval_string_concat() {
    let (r, _) = run_src("main { \"hello\" + \" \" + \"world\" }");
    assert!(r.is_ok());
}

#[test]
fn test_eval_string_length() {
    // len() requires import std.core.*
    let (r, _) = run_src("import std.core.*\nmain { len(\"hello\") }");
    assert!(r.is_ok());
}

#[test]
fn test_eval_string_index() {
    // Test string indexing
    let (r, _) = run_src("main { let s = \"hello\"\n s[1] }");
    assert!(r.is_ok());
}

// === Error handling ===

#[test]
fn test_eval_division_by_zero() {
    let e = run_err("main { 10 / 0 }");
    assert_eq!(e.class_name, "RuntimeError");
}

#[test]
fn test_eval_undefined_variable() {
    let e = run_err("main { x }");
    assert_eq!(e.class_name, "RuntimeError");
}

#[test]
fn test_eval_type_coercion() {
    // Helen coerces string + int to string concatenation
    let (r, _) = run_src("main { \"hello\" + 42 }");
    assert!(r.is_ok());
}

// === Complex expressions ===

#[test]
fn test_eval_nested_function_calls() {
    let (r, _) = run_src("fn add(a, b) { a + b }\nfn mul(a, b) { a * b }\nmain { mul(add(2, 3), 4) }");
    assert_eq!(r.unwrap(), Some(int(20)));
}

#[test]
fn test_eval_complex_arithmetic() {
    let (r, _) = run_src("main { (2 + 3) * (4 - 1) }");
    assert_eq!(r.unwrap(), Some(int(15)));
}

#[test]
fn test_eval_chained_comparisons() {
    let (r, _) = run_src("main { 1 < 2 && 2 < 3 }");
    assert!(r.is_ok());
}
