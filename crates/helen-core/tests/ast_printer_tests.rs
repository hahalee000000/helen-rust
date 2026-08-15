//! Contract tests for `ast_printer.rs` (M1 Task 1.3).
//!
//! Pins the Python `ASTPrinter` / `repr()` output byte-for-byte:
//! - `py_str_float` matches Python `str(float)` (shortest round-trip, Py3
//!   formatting rules: scientific outside `[-4, 16)`, `e+20`/`e-05` style)
//! - `py_str_repr` matches Python `repr(str)`
//! - `py_repr_span` / `py_repr_token` match Python dataclass reprs
//! - `py_repr_map_entry` matches the dataclass repr used by
//!   `visit_map_literal` (Python prints `str(MapEntryNode)`, not an
//!   S-expression — including spans and nested node reprs)

use helen_core::ast::*;
use helen_core::ast_printer::*;
use helen_core::source::SourceSpan;
use helen_core::tokens::{LiteralValue, Token, TokenType};

fn span(file: &str, sl: u32, sc: u32, el: u32, ec: u32) -> SourceSpan {
    SourceSpan::new(file.to_string(), sl, sc, el, ec)
}

// ---------------------------------------------------------------------------
// py_str_float — Python str(float)
// ---------------------------------------------------------------------------

#[test]
fn float_matches_python_str() {
    let cases: &[(f64, &str)] = &[
        (2.5, "2.5"),
        (1e20, "1e+20"),
        (1.5e-05, "1.5e-05"),
        (1.0, "1.0"),
        (100.0, "100.0"),
        (0.0001, "0.0001"),
        (1e-05, "1e-05"),
        (1e16, "1e+16"),
        (1000000000000000.0, "1000000000000000.0"),
        (123456789.0, "123456789.0"),
        (7.0, "7.0"),
        (-7.5, "-7.5"),
        (0.5, "0.5"),
        (1.2345678901234567e-8, "1.2345678901234567e-08"),
        (5e-324, "5e-324"),
        (1.7976931348623157e308, "1.7976931348623157e+308"),
        (-1e20, "-1e+20"),
        (0.1, "0.1"),
        (1e-6, "1e-06"),
        (123.456, "123.456"),
        (2.2250738585072014e-308, "2.2250738585072014e-308"),
        (42.0, "42.0"),
        (0.0, "0.0"),
        (-0.0, "-0.0"),
    ];
    for (v, expected) in cases {
        assert_eq!(&py_str_float(*v), expected, "py_str_float({v})");
    }
}

#[test]
fn float_approx_pi_round_trip() {
    // 3.14 is an approximate PI constant; clippy flags the bare literal, so
    // compute it from parts to keep the exact parity string.
    let pi_minus: f64 = 3.0 + 0.14;
    assert_eq!(py_str_float(pi_minus), "3.14");
}

#[test]
fn float_special_values() {
    assert_eq!(py_str_float(f64::NAN), "nan");
    assert_eq!(py_str_float(f64::INFINITY), "inf");
    assert_eq!(py_str_float(f64::NEG_INFINITY), "-inf");
}

// ---------------------------------------------------------------------------
// py_str_repr — Python repr(str)
// ---------------------------------------------------------------------------

#[test]
fn str_repr_matches_python() {
    assert_eq!(py_str_repr("hello"), "'hello'");
    assert_eq!(py_str_repr("a'b"), "'a\\'b'");
    assert_eq!(py_str_repr("a\\b"), "'a\\\\b'");
    assert_eq!(py_str_repr("line1\nline2"), "'line1\\nline2'");
    assert_eq!(py_str_repr("tab\there"), "'tab\\there'");
    assert_eq!(py_str_repr(""), "''");
}

// ---------------------------------------------------------------------------
// py_repr_span / py_repr_token
// ---------------------------------------------------------------------------

#[test]
fn span_repr_matches_python() {
    assert_eq!(
        py_repr_span(&span("m.helen", 3, 14, 3, 17)),
        "SourceSpan(file='m.helen', start_line=3, start_col=14, end_line=3, end_col=17)"
    );
}

#[test]
fn token_repr_matches_python() {
    let t = Token {
        kind: TokenType::Plus,
        lexeme: "+".to_string(),
        literal: LiteralValue::Null,
        line: 4,
        col: 19,
        end_line: 4,
        end_col: 20,
        file: "m.helen".to_string(),
    };
    assert_eq!(
        py_repr_token(&t),
        "Token(type=PLUS, lexeme='+', literal=None, line=4, col=19, end_line=4, end_col=20, file='m.helen')"
    );
}

// ---------------------------------------------------------------------------
// py_repr_map_entry — the dataclass repr Python prints inside `(map ...)`
// ---------------------------------------------------------------------------

#[test]
fn map_entry_repr_matches_python() {
    // From Python ground truth: {"a": 1, "b": 2} at m.helen:3
    let entry = MapEntry {
        key: Expr::Literal(Lit {
            value: LiteralValue::Str("a".to_string()),
            span: span("m.helen", 3, 14, 3, 17),
        }),
        value: Expr::Literal(Lit {
            value: LiteralValue::Int(num_bigint::BigInt::from(1)),
            span: span("m.helen", 3, 19, 3, 20),
        }),
        span: span("m.helen", 3, 14, 3, 20),
    };
    assert_eq!(
        py_repr_map_entry(&entry),
        "MapEntryNode(key=LiteralNode(value='a', span=SourceSpan(file='m.helen', start_line=3, start_col=14, end_line=3, end_col=17)), value=LiteralNode(value=1, span=SourceSpan(file='m.helen', start_line=3, start_col=19, end_line=3, end_col=20)), span=SourceSpan(file='m.helen', start_line=3, start_col=14, end_line=3, end_col=20))"
    );
}

#[test]
fn map_entry_with_list_value_repr_matches_python() {
    // From Python ground truth: {"a": [1, 2]}
    let entry = MapEntry {
        key: Expr::Literal(Lit {
            value: LiteralValue::Str("a".to_string()),
            span: span("m.helen", 3, 14, 3, 17),
        }),
        value: Expr::List(ListLit {
            elements: vec![
                Expr::Literal(Lit {
                    value: LiteralValue::Int(num_bigint::BigInt::from(1)),
                    span: span("m.helen", 3, 20, 3, 21),
                }),
                Expr::Literal(Lit {
                    value: LiteralValue::Int(num_bigint::BigInt::from(2)),
                    span: span("m.helen", 3, 23, 3, 24),
                }),
            ],
            span: span("m.helen", 3, 19, 3, 25),
        }),
        span: span("m.helen", 3, 14, 3, 25),
    };
    let expected = "MapEntryNode(key=LiteralNode(value='a', span=SourceSpan(file='m.helen', start_line=3, start_col=14, end_line=3, end_col=17)), value=ListLiteralNode(elements=[LiteralNode(value=1, span=SourceSpan(file='m.helen', start_line=3, start_col=20, end_line=3, end_col=21)), LiteralNode(value=2, span=SourceSpan(file='m.helen', start_line=3, start_col=23, end_line=3, end_col=24))], span=SourceSpan(file='m.helen', start_line=3, start_col=19, end_line=3, end_col=25)), span=SourceSpan(file='m.helen', start_line=3, start_col=14, end_line=3, end_col=25))";
    assert_eq!(py_repr_map_entry(&entry), expected);
}

#[test]
fn map_entry_with_binary_value_repr_matches_python() {
    // From Python ground truth: {x: y + 1}
    let entry = MapEntry {
        key: Expr::Variable(Variable {
            name: "x".to_string(),
            span: span("m.helen", 4, 14, 4, 15),
        }),
        value: Expr::Binary(Binary {
            left: Box::new(Expr::Variable(Variable {
                name: "y".to_string(),
                span: span("m.helen", 4, 17, 4, 18),
            })),
            operator: Token {
                kind: TokenType::Plus,
                lexeme: "+".to_string(),
                literal: LiteralValue::Null,
                line: 4,
                col: 19,
                end_line: 4,
                end_col: 20,
                file: "m.helen".to_string(),
            },
            right: Box::new(Expr::Literal(Lit {
                value: LiteralValue::Int(num_bigint::BigInt::from(1)),
                span: span("m.helen", 4, 21, 4, 22),
            })),
            span: span("m.helen", 4, 19, 4, 22),
        }),
        span: span("m.helen", 4, 14, 4, 22),
    };
    let expected = "MapEntryNode(key=VariableNode(name='x', span=SourceSpan(file='m.helen', start_line=4, start_col=14, end_line=4, end_col=15)), value=BinaryOpNode(left=VariableNode(name='y', span=SourceSpan(file='m.helen', start_line=4, start_col=17, end_line=4, end_col=18)), operator=Token(type=PLUS, lexeme='+', literal=None, line=4, col=19, end_line=4, end_col=20, file='m.helen'), right=LiteralNode(value=1, span=SourceSpan(file='m.helen', start_line=4, start_col=21, end_line=4, end_col=22)), span=SourceSpan(file='m.helen', start_line=4, start_col=19, end_line=4, end_col=22)), span=SourceSpan(file='m.helen', start_line=4, start_col=14, end_line=4, end_col=22))";
    assert_eq!(py_repr_map_entry(&entry), expected);
}
