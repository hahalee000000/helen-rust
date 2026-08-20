//! More parser tests — targeting pratt.rs coverage.

use helen_core::lexer::Scanner;
use helen_parser::Parser;

fn parse_ok(src: &str) {
    let tokens = Scanner::new(src, "t.helen").scan_all();
    let mut parser = Parser::new(tokens);
    let _program = parser.parse();
    assert!(
        parser.errors().is_empty(),
        "parse errors for {}: {:?}",
        src,
        parser.errors()
    );
}

// ── Binary operators ────────────────────────────────────────────────────

#[test]
fn parse_addition() {
    parse_ok("main {\n 1 + 2\n}");
}

#[test]
fn parse_subtraction() {
    parse_ok("main {\n 5 - 3\n}");
}

#[test]
fn parse_multiplication() {
    parse_ok("main {\n 4 * 7\n}");
}

#[test]
fn parse_division() {
    parse_ok("main {\n 20 / 4\n}");
}

#[test]
fn parse_modulo() {
    parse_ok("main {\n 17 % 5\n}");
}

#[test]
fn parse_power() {
    parse_ok("main {\n 2 ^ 3\n}");
}

// ── Comparison operators ────────────────────────────────────────────────

#[test]
fn parse_equal() {
    parse_ok("main {\n 5 == 5\n}");
}

#[test]
fn parse_not_equal() {
    parse_ok("main {\n 5 != 3\n}");
}

#[test]
fn parse_less_than() {
    parse_ok("main {\n 3 < 5\n}");
}

#[test]
fn parse_greater_than() {
    parse_ok("main {\n 5 > 3\n}");
}

#[test]
fn parse_less_equal() {
    parse_ok("main {\n 3 <= 5\n}");
}

#[test]
fn parse_greater_equal() {
    parse_ok("main {\n 5 >= 3\n}");
}

// ── Logical operators ───────────────────────────────────────────────────

#[test]
fn parse_and() {
    parse_ok("main {\n true && false\n}");
}

#[test]
fn parse_or() {
    parse_ok("main {\n true || false\n}");
}

#[test]
fn parse_not() {
    parse_ok("main {\n !true\n}");
}

// ── Unary operators ─────────────────────────────────────────────────────

#[test]
fn parse_unary_minus() {
    parse_ok("main {\n -5\n}");
}

// ── Complex expressions ─────────────────────────────────────────────────

#[test]
fn parse_nested_arithmetic() {
    parse_ok("main {\n (1 + 2) * (3 - 4)\n}");
}

#[test]
fn parse_chained_comparison() {
    parse_ok("main {\n 1 < 2 && 2 < 3\n}");
}

#[test]
fn parse_ternary_like() {
    parse_ok("main {\n if true { 1 } else { 2 }\n}");
}

// ── List expressions ────────────────────────────────────────────────────

#[test]
fn parse_list_literal() {
    parse_ok("main {\n [1, 2, 3]\n}");
}

#[test]
fn parse_list_index() {
    parse_ok("main {\n let xs = [1, 2, 3]\n xs[0]\n}");
}

#[test]
fn parse_list_negative_index() {
    parse_ok("main {\n let xs = [1, 2, 3]\n xs[-1]\n}");
}

// ── Map expressions ─────────────────────────────────────────────────────

#[test]
fn parse_map_literal() {
    parse_ok(
        r#"main {
 {"a": 1, "b": 2}
}"#,
    );
}

#[test]
fn parse_map_access() {
    parse_ok(
        r#"main {
 let m = {"a": 1}
 m["a"]
}"#,
    );
}

#[test]
fn parse_map_dot_access() {
    parse_ok(
        r#"main {
 let m = {"x": 42}
 m.x
}"#,
    );
}

// ── String expressions ──────────────────────────────────────────────────

#[test]
fn parse_string_concat() {
    parse_ok(
        r#"main {
 "hello" + " " + "world"
}"#,
    );
}

#[test]
fn parse_string_multiply() {
    parse_ok(
        r#"main {
 "ab" * 3
}"#,
    );
}

#[test]
fn parse_string_index() {
    parse_ok(
        r#"main {
 let s = "hello"
 s[0]
}"#,
    );
}

// ── Function calls ──────────────────────────────────────────────────────

#[test]
fn parse_function_call_no_args() {
    parse_ok("fn foo() { return 1 }\nmain {\n foo()\n}");
}

#[test]
fn parse_function_call_one_arg() {
    parse_ok("fn foo(x) { return x }\nmain {\n foo(1)\n}");
}

#[test]
fn parse_function_call_multiple_args() {
    parse_ok("fn foo(a, b, c) { return a + b + c }\nmain {\n foo(1, 2, 3)\n}");
}

#[test]
fn parse_method_call() {
    parse_ok("main {\n let xs = [1, 2, 3]\n xs.pop()\n}");
}

// ── Lambda expressions ──────────────────────────────────────────────────

#[test]
fn parse_lambda_no_params() {
    parse_ok("main {\n let f = fn() { return 42 }\n f()\n}");
}

#[test]
fn parse_lambda_one_param() {
    parse_ok("main {\n let f = fn(x) { return x * 2 }\n f(5)\n}");
}

#[test]
fn parse_lambda_multiple_params() {
    parse_ok("main {\n let f = fn(a, b) { return a + b }\n f(3, 4)\n}");
}

// ── Control flow ────────────────────────────────────────────────────────

#[test]
fn parse_if_else() {
    parse_ok("main {\n if true { 1 } else { 2 }\n}");
}

#[test]
fn parse_if_elif_else() {
    parse_ok("main {\n let x = 5\n if x > 10 { 1 } else if x > 5 { 2 } else { 3 }\n}");
}

#[test]
fn parse_for_loop() {
    parse_ok("main {\n for i in [1, 2, 3] { print(i) }\n}");
}

#[test]
fn parse_while_loop() {
    parse_ok("main {\n let i = 0\n while i < 5 { i = i + 1 }\n}");
}

#[test]
fn parse_break() {
    parse_ok("main {\n for i in [1, 2, 3] { if i == 2 { break } }\n}");
}

#[test]
fn parse_continue() {
    parse_ok("main {\n for i in [1, 2, 3] { if i == 2 { continue } }\n}");
}

// ── Match statements ────────────────────────────────────────────────────

#[test]
fn parse_match_int() {
    parse_ok("main {\n let x = 5\n match x {\n  case 5 { 10 }\n  default { 20 }\n }\n}");
}

#[test]
fn parse_match_string() {
    parse_ok(
        r#"main {
 let x = "hello"
 match x {
  case "hello" { 1 }
  default { 2 }
 }
}"#,
    );
}

#[test]
fn parse_match_multiple_cases() {
    parse_ok("main {\n let x = 3\n match x {\n  case 1 { 10 }\n  case 2 { 20 }\n  case 3 { 30 }\n  default { 40 }\n }\n}");
}

// ── Try-catch ───────────────────────────────────────────────────────────

#[test]
fn parse_try_catch() {
    parse_ok(
        r#"main {
 try {
  throw RuntimeError("boom")
 } catch RuntimeError e {
  42
 }
}"#,
    );
}

#[test]
fn parse_try_catch_finally() {
    parse_ok(
        r#"main {
 try {
  1
 } catch RuntimeError e {
  2
 } finally {
  3
 }
}"#,
    );
}

// ── Assert ──────────────────────────────────────────────────────────────

#[test]
fn parse_assert_true() {
    parse_ok("main {\n assert true\n}");
}

#[test]
fn parse_assert_with_message() {
    parse_ok(
        r#"main {
 assert true, "message"
}"#,
    );
}

// ── Agent declarations ──────────────────────────────────────────────────

#[test]
fn parse_agent_basic() {
    parse_ok(
        r#"
agent MyAgent(x: int) {
    description "test"
    prompt "test"
    main {
        return x
    }
}
main { "done" }
"#,
    );
}

#[test]
fn parse_agent_with_tools() {
    parse_ok(
        r#"
agent MyAgent(x: int) {
    description "test"
    prompt "test"
    tools ["read_file", "write_file"]
    main {
        return x
    }
}
main { "done" }
"#,
    );
}

// ── Import statements ───────────────────────────────────────────────────

#[test]
fn parse_import_star() {
    parse_ok("import std.core.*\nmain {\n 1\n}");
}

// ── Complex nested expressions ──────────────────────────────────────────

#[test]
fn parse_nested_function_calls() {
    parse_ok("fn add(a, b) { return a + b }\nfn mul(a, b) { return a * b }\nmain {\n add(mul(2, 3), 4)\n}");
}

#[test]
fn parse_nested_list_access() {
    parse_ok("main {\n let xs = [[1, 2], [3, 4]]\n xs[0][1]\n}");
}

#[test]
fn parse_complex_expression() {
    parse_ok("main {\n let x = 5\n let y = 10\n (x + y) * 2 - 3\n}");
}
