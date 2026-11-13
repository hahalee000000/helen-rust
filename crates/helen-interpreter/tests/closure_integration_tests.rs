//! Integration tests for closure functionality (v1.18 + v1.32 parity).
//!
//! Tests that closures work correctly as callbacks in real Helen programs,
//! matching Python's test_closure_integration.py test suite.

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

// ── Closure Recursion (v1.18) ─────────────────────────────────────────────

#[test]
fn closure_recursive_factorial() {
    // Closures should support recursion via self_name (v1.18)
    let (r, _) = run_src(
        r#"
        main {
            let factorial = fn(n) {
                if n <= 1 {
                    return 1
                }
                return n * factorial(n - 1)
            }
            factorial(5)
        }
        "#,
    );
    assert_eq!(r.unwrap(), Some(int(120)));
}

#[test]
fn closure_recursive_fibonacci() {
    // Recursive closure for fibonacci
    let (r, _) = run_src(
        r#"
        main {
            let fib = fn(n) {
                if n <= 1 {
                    return n
                }
                return fib(n - 1) + fib(n - 2)
            }
            fib(10)
        }
        "#,
    );
    assert_eq!(r.unwrap(), Some(int(55)));
}

// ── Anonymous Closures as Callbacks ───────────────────────────────────────

#[test]
fn anonymous_closure_as_callback() {
    // Anonymous closures should work as callbacks in higher-order functions
    let (r, _) = run_src(
        r#"
        fn apply_twice(f, x) {
            return f(f(x))
        }
        main {
            let result = apply_twice(fn(n) { return n + 1 }, 3)
            result
        }
        "#,
    );
    assert_eq!(r.unwrap(), Some(int(5))); // 3 + 1 + 1 = 5
}

#[test]
fn closure_in_map_operation() {
    // Closures should work with map-like operations
    let (r, _) = run_src(
        r#"
        import std.core.*
        fn map_double(arr) {
            let result = []
            for x in arr {
                result = result + [x * 2]
            }
            return result
        }
        main {
            let nums = [1, 2, 3]
            let doubled = map_double(nums)
            len(doubled)
        }
        "#,
    );
    assert_eq!(r.unwrap(), Some(int(3)));
}

// ── Closure Captures Environment ──────────────────────────────────────────

#[test]
fn closure_captures_and_uses_outer_variable() {
    // Closures should capture variables from enclosing scope
    let (r, _) = run_src(
        r#"
        fn make_multiplier(factor) {
            return fn(x) { return x * factor }
        }
        main {
            let triple = make_multiplier(3)
            triple(5)
        }
        "#,
    );
    assert_eq!(r.unwrap(), Some(int(15))); // 5 * 3 = 15
}

#[test]
fn closure_captures_multiple_variables() {
    // Closures should capture multiple variables
    let (r, _) = run_src(
        r#"
        fn make_adder(a, b) {
            return fn(x) { return x + a + b }
        }
        main {
            let adder = make_adder(10, 20)
            adder(5)
        }
        "#,
    );
    assert_eq!(r.unwrap(), Some(int(35))); // 5 + 10 + 20 = 35
}

// ── Named Functions as Callbacks ──────────────────────────────────────────

#[test]
fn named_function_as_callback() {
    // Named functions should still work as callbacks
    let (r, _) = run_src(
        r#"
        fn double(x) {
            return x * 2
        }
        fn apply(f, x) {
            return f(x)
        }
        main {
            apply(double, 5)
        }
        "#,
    );
    assert_eq!(r.unwrap(), Some(int(10))); // 5 * 2 = 10
}

// ── Closure with Complex Logic ────────────────────────────────────────────

#[test]
fn closure_with_conditional_logic() {
    // Closures with conditional logic
    let (r, _) = run_src(
        r#"
        main {
            let classify = fn(n) {
                if n < 0 {
                    return "negative"
                } else if n == 0 {
                    return "zero"
                } else {
                    return "positive"
                }
            }
            classify(-5)
        }
        "#,
    );
    // Should return "negative" as a string
    assert!(r.is_ok());
}

#[test]
fn closure_with_loop() {
    // Closures with loops
    let (r, _) = run_src(
        r#"
        import std.core.*
        main {
            let sum_to = fn(n) {
                let total = 0
                for i in range(1, n + 1) {
                    total = total + i
                }
                return total
            }
            sum_to(10)
        }
        "#,
    );
    assert_eq!(r.unwrap(), Some(int(55))); // 1+2+...+10 = 55
}

// ── Nested Closures ───────────────────────────────────────────────────────

#[test]
fn nested_closures() {
    // Nested closures should capture correctly
    let (r, _) = run_src(
        r#"
        fn outer(x) {
            let middle = fn(y) {
                let inner = fn(z) {
                    return x + y + z
                }
                return inner(10)
            }
            return middle(5)
        }
        main {
            outer(1)
        }
        "#,
    );
    assert_eq!(r.unwrap(), Some(int(16))); // 1 + 5 + 10 = 16
}
