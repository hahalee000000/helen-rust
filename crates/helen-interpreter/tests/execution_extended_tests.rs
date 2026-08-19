//! Extended execution tests — more language features and stdlib coverage.

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

// ── Arithmetic ──────────────────────────────────────────────────────────

#[test]
fn arithmetic_addition() {
    let (r, _) = run_src("main {\n 2 + 3\n}");
    assert_eq!(r.unwrap(), Some(int(5)));
}

#[test]
fn arithmetic_subtraction() {
    let (r, _) = run_src("main {\n 10 - 4\n}");
    assert_eq!(r.unwrap(), Some(int(6)));
}

#[test]
fn arithmetic_multiplication() {
    let (r, _) = run_src("main {\n 3 * 7\n}");
    assert_eq!(r.unwrap(), Some(int(21)));
}

#[test]
fn arithmetic_division() {
    let (r, _) = run_src("main {\n 20 / 4\n}");
    assert_eq!(r.unwrap(), Some(int(5)));
}

#[test]
fn arithmetic_modulo() {
    let (r, _) = run_src("main {\n 17 % 5\n}");
    assert_eq!(r.unwrap(), Some(int(2)));
}

// ── Comparison ──────────────────────────────────────────────────────────

#[test]
fn comparison_equal() {
    let (r, _) = run_src("main {\n 5 == 5\n}");
    assert_eq!(r.unwrap(), Some(Value::Bool(true)));
}

#[test]
fn comparison_not_equal() {
    let (r, _) = run_src("main {\n 5 != 3\n}");
    assert_eq!(r.unwrap(), Some(Value::Bool(true)));
}

#[test]
fn comparison_less_than() {
    let (r, _) = run_src("main {\n 3 < 5\n}");
    assert_eq!(r.unwrap(), Some(Value::Bool(true)));
}

#[test]
fn comparison_greater_than() {
    let (r, _) = run_src("main {\n 5 > 3\n}");
    assert_eq!(r.unwrap(), Some(Value::Bool(true)));
}

// ── Boolean logic ───────────────────────────────────────────────────────

#[test]
fn boolean_and() {
    let (r, _) = run_src("main {\n true && true\n}");
    assert_eq!(r.unwrap(), Some(Value::Bool(true)));
}

#[test]
fn boolean_or() {
    let (r, _) = run_src("main {\n false || true\n}");
    assert_eq!(r.unwrap(), Some(Value::Bool(true)));
}

#[test]
fn boolean_not() {
    let (r, _) = run_src("main {\n !false\n}");
    assert_eq!(r.unwrap(), Some(Value::Bool(true)));
}

// ── Lists ───────────────────────────────────────────────────────────────

#[test]
fn list_literal() {
    let (r, _) = run_src("main {\n [1, 2, 3]\n}");
    assert!(r.is_ok());
}

#[test]
fn list_index() {
    let (r, _) = run_src("main {\n let arr = [10, 20, 30]\n arr[1]\n}");
    assert_eq!(r.unwrap(), Some(int(20)));
}

// ── Maps ────────────────────────────────────────────────────────────────

#[test]
fn map_literal() {
    let (r, _) = run_src("main {\n {\"a\": 1, \"b\": 2}\n}");
    assert!(r.is_ok());
}

#[test]
fn map_access() {
    let (r, _) = run_src("main {\n let m = {\"x\": 42}\n m[\"x\"]\n}");
    assert_eq!(r.unwrap(), Some(int(42)));
}

// ── Strings ─────────────────────────────────────────────────────────────

#[test]
fn string_concat() {
    let (r, _) = run_src("main {\n \"hello\" + \" \" + \"world\"\n}");
    assert!(r.is_ok());
}

// ── Functions ───────────────────────────────────────────────────────────

#[test]
fn function_definition_and_call() {
    let (r, _) = run_src("fn add(a, b) {\n return a + b\n}\nmain {\n add(3, 4)\n}");
    assert_eq!(r.unwrap(), Some(int(7)));
}

#[test]
fn function_recursion() {
    let (r, _) = run_src("fn factorial(n) {\n if n <= 1 {\n  return 1\n }\n return n * factorial(n - 1)\n}\nmain {\n factorial(5)\n}");
    assert_eq!(r.unwrap(), Some(int(120)));
}

// ── Loops ───────────────────────────────────────────────────────────────

#[test]
fn for_loop() {
    let (r, _) = run_src("main {\n let sum = 0\n for i in [1, 2, 3, 4, 5] {\n  sum = sum + i\n }\n sum\n}");
    assert_eq!(r.unwrap(), Some(int(15)));
}

#[test]
fn while_loop() {
    let (r, _) = run_src("main {\n let i = 0\n let sum = 0\n while i < 5 {\n  sum = sum + i\n  i = i + 1\n }\n sum\n}");
    assert_eq!(r.unwrap(), Some(int(10)));
}

// ── Conditionals ────────────────────────────────────────────────────────

#[test]
fn if_else() {
    let (r, _) = run_src("main {\n let x = 10\n if x > 5 {\n  1\n } else {\n  0\n }\n}");
    assert_eq!(r.unwrap(), Some(int(1)));
}

// ── Error handling ──────────────────────────────────────────────────────

#[test]
fn try_catch() {
    let (r, _) = run_src("main {\n try {\n  throw RuntimeError(\"test\")\n } catch RuntimeError e {\n  42\n }\n}");
    assert_eq!(r.unwrap(), Some(int(42)));
}

// ── Closures ────────────────────────────────────────────────────────────

#[test]
fn closure_capture() {
    let (r, _) = run_src("main {\n let x = 10\n let f = fn() { x }\n f()\n}");
    assert_eq!(r.unwrap(), Some(int(10)));
}
