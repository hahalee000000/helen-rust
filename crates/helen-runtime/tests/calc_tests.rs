//! Extended tests for calc module — arithmetic evaluator.

use helen_runtime::calc::eval_simple;

// ── Basic arithmetic ────────────────────────────────────────────────────

#[test]
fn eval_integer_addition() {
    assert_eq!(eval_simple("1 + 2").unwrap(), "3");
}

#[test]
fn eval_integer_subtraction() {
    assert_eq!(eval_simple("10 - 3").unwrap(), "7");
}

#[test]
fn eval_integer_multiplication() {
    assert_eq!(eval_simple("4 * 5").unwrap(), "20");
}

#[test]
fn eval_integer_division() {
    assert_eq!(eval_simple("10 / 4").unwrap(), "2.5");
}

#[test]
fn eval_integer_modulo() {
    assert_eq!(eval_simple("7 % 3").unwrap(), "1");
}

#[test]
fn eval_power() {
    assert_eq!(eval_simple("2 ^ 10").unwrap(), "1024");
}

#[test]
fn eval_negative() {
    assert_eq!(eval_simple("-5 + 3").unwrap(), "-2");
}

#[test]
fn eval_unary_plus() {
    assert_eq!(eval_simple("+5").unwrap(), "5");
}

// ── Operator precedence ─────────────────────────────────────────────────

#[test]
fn eval_precedence_mul_before_add() {
    assert_eq!(eval_simple("1 + 2 * 3").unwrap(), "7");
}

#[test]
fn eval_precedence_parens() {
    assert_eq!(eval_simple("(1 + 2) * 3").unwrap(), "9");
}

#[test]
fn eval_nested_parens() {
    assert_eq!(eval_simple("((2 + 3) * (4 - 1))").unwrap(), "15");
}

#[test]
fn eval_complex_expression() {
    assert_eq!(eval_simple("2 * 3 + 4 * 5").unwrap(), "26");
}

// ── Float operations ────────────────────────────────────────────────────

#[test]
fn eval_float_addition() {
    assert_eq!(eval_simple("1.5 + 2.5").unwrap(), "4");
}

#[test]
fn eval_float_result() {
    assert_eq!(eval_simple("1 / 3").unwrap(), "0.3333333333333333");
}

#[test]
fn eval_float_power() {
    assert_eq!(eval_simple("2.5 ^ 2").unwrap(), "6.25");
}

// ── Math functions ──────────────────────────────────────────────────────

#[test]
fn eval_sqrt() {
    assert_eq!(eval_simple("sqrt(16)").unwrap(), "4");
}

#[test]
fn eval_sqrt_non_perfect() {
    let result = eval_simple("sqrt(2)").unwrap();
    let v: f64 = result.parse().unwrap();
    assert!((v - std::f64::consts::SQRT_2).abs() < 0.001);
}

#[test]
fn eval_abs_positive() {
    assert_eq!(eval_simple("abs(5)").unwrap(), "5");
}

#[test]
fn eval_abs_negative() {
    assert_eq!(eval_simple("abs(-3)").unwrap(), "3");
}

#[test]
fn eval_abs_zero() {
    assert_eq!(eval_simple("abs(0)").unwrap(), "0");
}

#[test]
fn eval_sin_zero() {
    let result = eval_simple("sin(0)").unwrap();
    let v: f64 = result.parse().unwrap();
    assert!(v.abs() < 0.001);
}

#[test]
fn eval_cos_zero() {
    let result = eval_simple("cos(0)").unwrap();
    let v: f64 = result.parse().unwrap();
    assert!((v - 1.0).abs() < 0.001);
}

#[test]
fn eval_tan_zero() {
    let result = eval_simple("tan(0)").unwrap();
    let v: f64 = result.parse().unwrap();
    assert!(v.abs() < 0.001);
}

#[test]
fn eval_log_e() {
    let result = eval_simple("log(2.718281828)").unwrap();
    let v: f64 = result.parse().unwrap();
    assert!((v - 1.0).abs() < 0.01);
}

#[test]
fn eval_log10() {
    let result = eval_simple("log10(100)").unwrap();
    let v: f64 = result.parse().unwrap();
    assert!((v - 2.0).abs() < 0.001);
}

#[test]
fn eval_exp_zero() {
    let result = eval_simple("exp(0)").unwrap();
    let v: f64 = result.parse().unwrap();
    assert!((v - 1.0).abs() < 0.001);
}

#[test]
fn eval_exp_one() {
    let result = eval_simple("exp(1)").unwrap();
    let v: f64 = result.parse().unwrap();
    assert!((v - std::f64::consts::E).abs() < 0.01);
}

#[test]
fn eval_floor() {
    assert_eq!(eval_simple("floor(2.7)").unwrap(), "2");
}

#[test]
fn eval_floor_negative() {
    assert_eq!(eval_simple("floor(-2.3)").unwrap(), "-3");
}

#[test]
fn eval_ceil() {
    assert_eq!(eval_simple("ceil(2.3)").unwrap(), "3");
}

#[test]
fn eval_ceil_negative() {
    assert_eq!(eval_simple("ceil(-2.7)").unwrap(), "-2");
}

#[test]
fn eval_round() {
    assert_eq!(eval_simple("round(2.5)").unwrap(), "3");
}

#[test]
fn eval_round_down() {
    assert_eq!(eval_simple("round(2.4)").unwrap(), "2");
}

#[test]
fn eval_min_single() {
    assert_eq!(eval_simple("min(5)").unwrap(), "5");
}

#[test]
fn eval_min_multiple() {
    assert_eq!(eval_simple("min(1, 5, 3)").unwrap(), "1");
}

#[test]
fn eval_max_single() {
    assert_eq!(eval_simple("max(5)").unwrap(), "5");
}

#[test]
fn eval_max_multiple() {
    assert_eq!(eval_simple("max(1, 5, 3)").unwrap(), "5");
}

#[test]
fn eval_pow() {
    assert_eq!(eval_simple("pow(2, 3)").unwrap(), "8");
}

// ── Error cases ─────────────────────────────────────────────────────────

#[test]
fn eval_division_by_zero() {
    assert!(eval_simple("1/0").is_err());
}

#[test]
fn eval_incomplete_expression() {
    assert!(eval_simple("2 +").is_err());
}

#[test]
fn eval_unknown_function() {
    assert!(eval_simple("unknown_fn(1)").is_err());
}

#[test]
fn eval_unmatched_paren() {
    assert!(eval_simple("(1 + 2").is_err());
}

#[test]
fn eval_extra_close_paren() {
    assert!(eval_simple("1 + 2)").is_err());
}

#[test]
fn eval_empty_string() {
    assert!(eval_simple("").is_err());
}

#[test]
fn eval_invalid_number() {
    assert!(eval_simple("1.2.3").is_err());
}

#[test]
fn eval_wrong_arg_count_sin() {
    assert!(eval_simple("sin(1, 2)").is_err());
}

#[test]
fn eval_wrong_arg_count_pow() {
    assert!(eval_simple("pow(2)").is_err());
}

#[test]
fn eval_min_no_args() {
    assert!(eval_simple("min()").is_err());
}

#[test]
fn eval_max_no_args() {
    assert!(eval_simple("max()").is_err());
}

// ── Whitespace handling ─────────────────────────────────────────────────

#[test]
fn eval_whitespace() {
    assert_eq!(eval_simple("  1  +  2  ").unwrap(), "3");
}

#[test]
fn eval_newlines() {
    assert_eq!(eval_simple("1 +\n2").unwrap(), "3");
}

#[test]
fn eval_tabs() {
    assert_eq!(eval_simple("1\t+\t2").unwrap(), "3");
}

// ── Large numbers ───────────────────────────────────────────────────────

#[test]
fn eval_large_number() {
    assert_eq!(eval_simple("1000000 * 1000000").unwrap(), "1000000000000");
}

#[test]
fn eval_zero() {
    assert_eq!(eval_simple("0").unwrap(), "0");
}

#[test]
fn eval_single_number() {
    assert_eq!(eval_simple("42").unwrap(), "42");
}
