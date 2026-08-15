//! Execution Tier C — source-based port of `tests/execution/*` behavioral
//! scenarios. The Python suite builds ASTs programmatically; each scenario is
//! expressed as Helen source (the equivalent program) and asserted via the
//! Rust interpreter's public API.
//!
//! Generated from the M13 Tier-C plan (Task 13.3, execution row): "construct
//! Rust AST → run → assert" — here the Rust parser constructs the AST from
//! source, keeping the suite self-contained and readable.
//!
//! Style notes (verified against the Python reference):
//! - `let`/`const` declarations are NEWLINE-terminated (`;` after them is a
//!   PARSER_ERROR in both implementations).
//! - Expression/assignment statements may be `;`-separated.
//! - Boolean source operators are `!`, `&&`, `||` (not `not`/`and`/`or`).
//! - `catch`/`catch-all` bodies use `return` to yield values (a bare `{ 7 }`
//!   parses as a map literal `{7: None}`).
//! - Out-of-bounds index / missing property RAISE RuntimeError (the pytest
//!   `_run` helper catches it → None, but the interpreter raises).

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

fn err_class(e: &ExceptionValue) -> String {
    e.class_name.clone()
}

// ── Variables (test_variables.py) ────────────────────────────────────────

#[test]
fn let_declare_and_read() {
    let (r, _) = run_src("main {\n let x = 42\n x\n}");
    assert_eq!(r.unwrap(), Some(int(42)));
}

#[test]
fn let_reassign() {
    let (r, _) = run_src("main {\n let x = 1\n x = 2\n x\n}");
    assert_eq!(r.unwrap(), Some(int(2)));
}

#[test]
fn const_declare_and_read() {
    let (r, _) = run_src("main {\n const MAX = 100\n MAX\n}");
    assert_eq!(r.unwrap(), Some(int(100)));
}

#[test]
fn const_assignment_raises() {
    // HLD 3.4.1: assigning to a const raises ConstAssignmentError.
    let e = run_err("main {\n const MAX = 100\n MAX = 200\n}");
    assert_eq!(err_class(&e), "ConstAssignmentError");
}

#[test]
fn shadow_in_nested_scope() {
    // Variable in inner scope shadows outer, but outer is unchanged.
    let (r, _) = run_src(
        "main {\n let x = 1\n if true {\n  let x = 2\n  x\n }\n x\n}",
    );
    assert_eq!(r.unwrap(), Some(int(1)));
}

// ── Control flow (test_control_flow.py) ─────────────────────────────────

#[test]
fn if_true_branch() {
    let (r, _) = run_src("main {\n if true {\n  1\n }\n}");
    assert_eq!(r.unwrap(), Some(int(1)));
}

#[test]
fn if_false_then_else() {
    let (r, _) = run_src("main {\n if false {\n  1\n } else {\n  2\n }\n}");
    assert_eq!(r.unwrap(), Some(int(2)));
}

#[test]
fn if_false_no_else() {
    let (r, _) = run_src("main {\n if false {\n  1\n }\n}");
    assert_eq!(r.unwrap(), None);
}

#[test]
fn for_loop_iterates() {
    let (_, out) =
        run_src("import std.core.*\nmain {\n for x in [1, 2, 3] {\n  print(x)\n }\n}");
    assert_eq!(out, "1\n2\n3\n");
}

#[test]
fn for_loop_sums() {
    let (r, _) = run_src(
        "main {\n let s = 0\n for x in [1, 2, 3, 4] {\n  s = s + x\n }\n s\n}",
    );
    assert_eq!(r.unwrap(), Some(int(10)));
}

#[test]
fn while_loop_counts() {
    let (r, _) = run_src("main {\n let i = 0\n while i < 3 {\n  i = i + 1\n }\n i\n}");
    assert_eq!(r.unwrap(), Some(int(3)));
}

#[test]
fn while_loop_break() {
    let (r, _) = run_src(
        "main {\n let i = 0\n while true {\n  i = i + 1\n  if i >= 2 {\n   break\n  }\n }\n i\n}",
    );
    assert_eq!(r.unwrap(), Some(int(2)));
}

#[test]
fn for_loop_continue_skips() {
    let (_, out) = run_src(
        "import std.core.*\nmain {\n for x in [1, 2, 3, 4] {\n  if x % 2 == 0 {\n   continue\n  }\n  print(x)\n }\n}",
    );
    assert_eq!(out, "1\n3\n");
}

// ── Functions (test_functions.py) ────────────────────────────────────────

#[test]
fn fn_declare_and_call() {
    let (r, _) = run_src("fn add(a, b) {\n return a + b\n}\nmain {\n add(2, 3)\n}");
    assert_eq!(r.unwrap(), Some(int(5)));
}

#[test]
fn fn_implicit_last_expr() {
    let (r, _) = run_src("fn double(x) {\n x * 2\n}\nmain {\n double(21)\n}");
    assert_eq!(r.unwrap(), Some(int(42)));
}

#[test]
fn fn_recursion() {
    let (r, _) = run_src(
        "fn fact(n) {\n if n <= 1 {\n  return 1\n }\n n * fact(n - 1)\n}\nmain {\n fact(5)\n}",
    );
    assert_eq!(r.unwrap(), Some(int(120)));
}

#[test]
fn closure_captures_outer() {
    let (r, _) = run_src(
        "main {\n let base = 10\n fn add(x) {\n  return base + x\n }\n add(5)\n}",
    );
    assert_eq!(r.unwrap(), Some(int(15)));
}

// ── Exceptions (test_exceptions.py) ──────────────────────────────────────

#[test]
fn try_catch_type_match() {
    // Python tests use `return` in catch bodies (a bare `{ 7 }` parses as a
    // map literal `{7: None}` — verified against the reference).
    let (r, _) = run_src(
        "main {\n try {\n  throw RuntimeError(\"boom\")\n } catch RuntimeError e {\n  return 7\n }\n}",
    );
    assert_eq!(r.unwrap(), Some(int(7)));
}

#[test]
fn catch_all_fallback() {
    // Python's test throws ToolError (not an LLMError subtype) so the typed
    // catch misses and the catch-all runs. ModelError WOULD match LLMError.
    let (r, _) = run_src(
        "main {\n try {\n  throw ToolError(\"x\")\n } catch LLMError e {\n  return 1\n } catch AnyError e {\n  return 2\n }\n}",
    );
    assert_eq!(r.unwrap(), Some(int(2)));
}

#[test]
fn finally_always_executes() {
    let (_, out) = run_src(
        "import std.core.*\nmain {\n try {\n  print(\"try\")\n } finally {\n  print(\"finally\")\n }\n}",
    );
    assert_eq!(out, "try\nfinally\n");
}

#[test]
fn finally_executes_on_exception() {
    let (_, out) = run_src(
        "import std.core.*\nmain {\n try {\n  throw RuntimeError(\"x\")\n } catch AnyError e {\n  print(\"caught\")\n } finally {\n  print(\"finally\")\n }\n}",
    );
    assert_eq!(out, "caught\nfinally\n");
}

#[test]
fn uncaught_exception_rethrows() {
    // ValueError is NOT a predefined Helen exception — Python's
    // `resolve_exception` returns None and the throw falls back to
    // RuntimeError (verified: `throw ValueError("bad")` raises RuntimeError
    // in the Python reference). Use a native class for the rethrow test.
    let e = run_err("main {\n throw ToolError(\"bad\")\n}");
    assert_eq!(err_class(&e), "ToolError");
}

#[test]
fn nested_try_catch() {
    let (r, out) = run_src(
        "import std.core.*\nmain {\n try {\n  try {\n   throw RuntimeError(\"inner\")\n  } finally {\n   print(\"inner-finally\")\n  }\n } catch AnyError e {\n  return 9\n }\n}",
    );
    assert_eq!(out, "inner-finally\n");
    assert_eq!(r.unwrap(), Some(int(9)));
}

#[test]
fn division_by_zero_caught() {
    let (r, _) = run_src(
        "main {\n try {\n  1 / 0\n } catch AnyError e {\n  return \"zero\"\n }\n}",
    );
    assert_eq!(r.unwrap(), Some(Value::Str("zero".into())));
}

#[test]
fn error_hierarchy_match() {
    // LLMError is a parent of TimeoutError/ModelError; ToolError is not.
    let (r, _) = run_src(
        "main {\n try {\n  throw TimeoutError(\"t\")\n } catch LLMError e {\n  return \"llm\"\n }\n}",
    );
    assert_eq!(r.unwrap(), Some(Value::Str("llm".into())));
    let e = run_err("main {\n throw ToolError(\"t\")\n}");
    assert_eq!(err_class(&e), "ToolError");
}

// ── Collections (test_collections.py) ────────────────────────────────────

#[test]
fn empty_list() {
    let (r, _) = run_src("main {\n []\n}");
    assert!(matches!(r.unwrap(), Some(Value::List(_))));
}

#[test]
fn list_of_ints() {
    let (r, _) = run_src("main {\n [1, 2, 3]\n}");
    let v = r.unwrap().unwrap();
    match v {
        Value::List(items) => assert_eq!(items.borrow().len(), 3),
        other => panic!("expected list, got {:?}", other),
    }
}

#[test]
fn map_with_entries() {
    let (r, _) = run_src("main {\n { \"a\": 1, \"b\": 2 }\n}");
    let v = r.unwrap().unwrap();
    match v {
        Value::Map(m) => {
            assert_eq!(m.borrow().len(), 2);
            assert_eq!(m.borrow().get(&Value::Str("a".into())), Some(&int(1)));
        }
        other => panic!("expected map, got {:?}", other),
    }
}

#[test]
fn index_list_by_int() {
    let (r, _) = run_src("main {\n [10, 20, 30][1]\n}");
    assert_eq!(r.unwrap(), Some(int(20)));
}

#[test]
fn index_map_by_string() {
    let (r, _) = run_src("main {\n { \"k\": 99 }[\"k\"]\n}");
    assert_eq!(r.unwrap(), Some(int(99)));
}

#[test]
fn index_out_of_bounds_raises() {
    // Python's test `_run` helper catches HelenRuntimeError and returns None,
    // but the interpreter itself raises "list index out of range".
    let e = run_err("main {\n [1, 2][5]\n}");
    assert_eq!(err_class(&e), "RuntimeError");
}

#[test]
fn access_dict_property() {
    let (r, _) = run_src("main {\n let d = { \"name\": \"x\" }\n d.name\n}");
    assert_eq!(r.unwrap(), Some(Value::Str("x".into())));
}

#[test]
fn access_missing_property_raises() {
    // Python interpreter raises `RuntimeError: Property 'missing' not found`
    // (the pytest helper catches it → None; the interpreter raises).
    let e = run_err("main {\n let d = { \"name\": \"x\" }\n d.missing\n}");
    assert_eq!(err_class(&e), "RuntimeError");
}

#[test]
fn list_in_variable() {
    let (_, out) =
        run_src("import std.core.*\nmain {\n let xs = [1, 2]\n for x in xs {\n  print(x)\n }\n}");
    assert_eq!(out, "1\n2\n");
}

// ── Expressions (test_interpreter_expressions.py) ────────────────────────

#[test]
fn expr_int() {
    let (r, _) = run_src("main {\n 42\n}");
    assert_eq!(r.unwrap(), Some(int(42)));
}

#[test]
fn expr_float() {
    let (r, _) = run_src("main {\n 3.5\n}");
    assert_eq!(r.unwrap(), Some(Value::Float(3.5)));
}

#[test]
fn expr_string() {
    let (r, _) = run_src("main {\n \"hi\"\n}");
    assert_eq!(r.unwrap(), Some(Value::Str("hi".into())));
}

#[test]
fn expr_true_false_null() {
    assert_eq!(run_src("main {\n true\n}").0.unwrap(), Some(Value::Bool(true)));
    assert_eq!(
        run_src("main {\n false\n}").0.unwrap(),
        Some(Value::Bool(false))
    );
    // `null` evaluates to None (Python `null` → None, interpret returns None).
    assert_eq!(run_src("main {\n null\n}").0.unwrap(), None);
}

#[test]
fn expr_arithmetic() {
    assert_eq!(run_src("main {\n 2 + 3\n}").0.unwrap(), Some(int(5)));
    assert_eq!(run_src("main {\n 5 - 3\n}").0.unwrap(), Some(int(2)));
    assert_eq!(run_src("main {\n 4 * 3\n}").0.unwrap(), Some(int(12)));
    assert_eq!(run_src("main {\n 10 / 2\n}").0.unwrap(), Some(int(5)));
    assert_eq!(run_src("main {\n 10 % 3\n}").0.unwrap(), Some(int(1)));
}

#[test]
fn expr_string_concat() {
    let (r, _) = run_src("main {\n \"a\" + \"b\"\n}");
    assert_eq!(r.unwrap(), Some(Value::Str("ab".into())));
}

#[test]
fn expr_string_number_concat() {
    let (r, _) = run_src("main {\n \"n=\" + 5\n}");
    assert_eq!(r.unwrap(), Some(Value::Str("n=5".into())));
}

#[test]
fn expr_comparison() {
    assert_eq!(run_src("main {\n 1 == 1\n}").0.unwrap(), Some(Value::Bool(true)));
    assert_eq!(run_src("main {\n 1 != 2\n}").0.unwrap(), Some(Value::Bool(true)));
    assert_eq!(run_src("main {\n 2 > 1\n}").0.unwrap(), Some(Value::Bool(true)));
    assert_eq!(run_src("main {\n 2 >= 2\n}").0.unwrap(), Some(Value::Bool(true)));
    assert_eq!(run_src("main {\n 1 < 2\n}").0.unwrap(), Some(Value::Bool(true)));
    assert_eq!(run_src("main {\n 1 <= 1\n}").0.unwrap(), Some(Value::Bool(true)));
}

#[test]
fn expr_logic() {
    // Source operators: `!`, `&&`, `||` (lexer maps `!` → BANG, `&&` → AND,
    // `||` → OR; `not`/`and`/`or` are NOT keywords in Helen).
    assert_eq!(
        run_src("main {\n true && false\n}").0.unwrap(),
        Some(Value::Bool(false))
    );
    assert_eq!(
        run_src("main {\n true || false\n}").0.unwrap(),
        Some(Value::Bool(true))
    );
    assert_eq!(
        run_src("main {\n !true\n}").0.unwrap(),
        Some(Value::Bool(false))
    );
}

#[test]
fn expr_grouped() {
    let (r, _) = run_src("main {\n (1 + 2) * 3\n}");
    assert_eq!(r.unwrap(), Some(int(9)));
}

#[test]
fn expr_lookup_undefined_is_error() {
    // Python's `visit_variable` records UNDECLARED_VARIABLE and returns None
    // (test: `assert errors.has_errors`); the CLI exits 2 with E0332. The
    // Rust interpreter raises a RuntimeError with the same message — CLI
    // parity verified (exit 2, "undeclared variable"). Assert the error class.
    let e = run_err("main {\n undefined_var\n}");
    assert_eq!(err_class(&e), "RuntimeError");
    assert!(e.message.contains("undefined_var"), "msg: {}", e.message);
}

#[test]
fn expr_list_literal() {
    let (_, out) = run_src("import std.core.*\nmain {\n print([1, 2, 3])\n}");
    assert_eq!(out, "[1, 2, 3]\n");
}

#[test]
fn expr_call_undefined_is_error() {
    let e = run_err("main {\n no_such_fn()\n}");
    // Either an UndefinedFunction error or an internal error class — the
    // important assertion is that it raises (parity: Python raises too).
    assert!(!e.class_name.is_empty());
}

#[test]
fn expr_call_defined_function() {
    let (r, _) = run_src("fn sq(x) {\n x * x\n}\nmain {\n sq(6)\n}");
    assert_eq!(r.unwrap(), Some(int(36)));
}
