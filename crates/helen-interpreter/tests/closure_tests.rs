//! Tests for closure module — free variable analysis.
//!
//! Tests the `compute_free_variables` function with various AST patterns
//! to ensure correct identification of captured variables.

use helen_core::ast::*;
use helen_core::source::SourceSpan;
use helen_core::tokens::{LiteralValue, Token, TokenType};
use helen_interpreter::closure::compute_free_variables;

fn make_lit(value: LiteralValue) -> Expr {
    Expr::Literal(Lit {
        value,
        span: dummy_span(),
    })
}

fn dummy_span() -> SourceSpan {
    SourceSpan {
        file: "t".into(),
        start_line: 1,
        start_col: 1,
        end_line: 1,
        end_col: 2,
    }
}

fn dummy_token() -> Token {
    Token {
        kind: TokenType::Plus,
        lexeme: "+".into(),
        literal: LiteralValue::Null,
        line: 1,
        col: 1,
        end_line: 1,
        end_col: 2,
        file: "t".into(),
    }
}

fn dummy_type_ref(name: &str) -> TypeRef {
    TypeRef {
        name: name.into(),
        span: dummy_span(),
        kind: TypeRefKind::Simple,
    }
}

fn make_var(name: &str) -> Expr {
    Expr::Variable(Variable {
        name: name.into(),
        span: dummy_span(),
    })
}

fn make_param(name: &str) -> AgentParam {
    AgentParam {
        name: name.into(),
        type_annotation: None,
        default_value: None,
        span: dummy_span(),
    }
}

fn make_lambda(params: Vec<&str>, body: Vec<Stmt>) -> Lambda {
    Lambda {
        params: params.into_iter().map(make_param).collect(),
        return_type: None,
        body: FnBlock {
            body,
            span: dummy_span(),
        },
        span: dummy_span(),
    }
}

fn make_expr_stmt(expr: Expr) -> Stmt {
    Stmt::Expr(ExprStmt {
        expression: expr,
        span: dummy_span(),
    })
}

// ── Basic cases ─────────────────────────────────────────────────────────

#[test]
fn free_vars_simple_capture() {
    // fn(x) { y + x }  — y is free, x is a param
    let lambda = make_lambda(vec!["x"], vec![make_expr_stmt(Expr::Binary(Binary {
        left: Box::new(make_var("y")),
        operator: dummy_token(),
        right: Box::new(make_var("x")),
        span: dummy_span(),
    }))]);
    let free = compute_free_variables(&lambda);
    assert!(free.contains("y"));
    assert!(!free.contains("x"));
}

#[test]
fn free_vars_no_captures() {
    // fn(x, y) { x + y }  — no free vars
    let lambda = make_lambda(vec!["x", "y"], vec![make_expr_stmt(Expr::Binary(Binary {
        left: Box::new(make_var("x")),
        operator: dummy_token(),
        right: Box::new(make_var("y")),
        span: dummy_span(),
    }))]);
    let free = compute_free_variables(&lambda);
    assert!(free.is_empty());
}

#[test]
fn free_vars_multiple_captures() {
    // fn() { a + b + c }  — all three are free
    let lambda = make_lambda(vec![], vec![make_expr_stmt(Expr::Binary(Binary {
        left: Box::new(Expr::Binary(Binary {
            left: Box::new(make_var("a")),
            operator: dummy_token(),
            right: Box::new(make_var("b")),
            span: dummy_span(),
        })),
        operator: dummy_token(),
        right: Box::new(make_var("c")),
        span: dummy_span(),
    }))]);
    let free = compute_free_variables(&lambda);
    assert_eq!(free.len(), 3);
    assert!(free.contains("a"));
    assert!(free.contains("b"));
    assert!(free.contains("c"));
}

// ── Variable declarations ───────────────────────────────────────────────

#[test]
fn free_vars_local_var_not_captured() {
    // fn() { let x = 1; x }  — x is local, not captured
    let lambda = make_lambda(vec![], vec![
        Stmt::VarDecl(VarDecl {
            name: "x".into(),
            type_annotation: None,
            initializer: Some(Box::new(make_lit(LiteralValue::Int(1.into())))),
            mutable: true,
            span: dummy_span(),
            shared: false,
        }),
        make_expr_stmt(make_var("x")),
    ]);
    let free = compute_free_variables(&lambda);
    assert!(free.is_empty());
}

#[test]
fn free_vars_var_with_free_initializer() {
    // fn() { let x = y; x }  — y is captured, x is local
    let lambda = make_lambda(vec![], vec![
        Stmt::VarDecl(VarDecl {
            name: "x".into(),
            type_annotation: None,
            initializer: Some(Box::new(make_var("y"))),
            mutable: true,
            span: dummy_span(),
            shared: false,
        }),
        make_expr_stmt(make_var("x")),
    ]);
    let free = compute_free_variables(&lambda);
    assert!(free.contains("y"));
    assert!(!free.contains("x"));
}

// ── Control flow ────────────────────────────────────────────────────────

#[test]
fn free_vars_if_statement() {
    // fn() { if cond { a } else { b } }  — cond, a, b all captured
    let lambda = make_lambda(vec![], vec![Stmt::If(IfStmt {
        condition: Box::new(make_var("cond")),
        then_branch: Box::new(make_expr_stmt(make_var("a"))),
        else_branch: Some(Box::new(make_expr_stmt(make_var("b")))),
        span: dummy_span(),
    })]);
    let free = compute_free_variables(&lambda);
    assert!(free.contains("cond"));
    assert!(free.contains("a"));
    assert!(free.contains("b"));
}

#[test]
fn free_vars_for_loop() {
    // fn() { for i in items { i + x } }  — items and x captured, i is loop var
    let lambda = make_lambda(vec![], vec![Stmt::For(ForStmt {
        iterator: Some(Variable {
            name: "i".into(),
            span: dummy_span(),
        }),
        iterable: Box::new(make_var("items")),
        body: Box::new(make_expr_stmt(Expr::Binary(Binary {
            left: Box::new(make_var("i")),
            operator: dummy_token(),
            right: Box::new(make_var("x")),
            span: dummy_span(),
        }))),
        span: dummy_span(),
    })]);
    let free = compute_free_variables(&lambda);
    assert!(free.contains("items"));
    assert!(free.contains("x"));
    assert!(!free.contains("i"));
}

#[test]
fn free_vars_while_loop() {
    // fn() { while cond { body } }  — cond and body captured
    let lambda = make_lambda(vec![], vec![Stmt::While(WhileStmt {
        condition: Box::new(make_var("cond")),
        body: Box::new(make_expr_stmt(make_var("body"))),
        span: dummy_span(),
    })]);
    let free = compute_free_variables(&lambda);
    assert!(free.contains("cond"));
    assert!(free.contains("body"));
}

#[test]
fn free_vars_return_statement() {
    // fn() { return x }  — x is captured
    let lambda = make_lambda(vec![], vec![Stmt::Return(ReturnStmt {
        value: Some(Box::new(make_var("x"))),
        span: dummy_span(),
    })]);
    let free = compute_free_variables(&lambda);
    assert!(free.contains("x"));
}

// ── Expressions ─────────────────────────────────────────────────────────

#[test]
fn free_vars_unary_expression() {
    // fn() { -x }  — x is captured
    let lambda = make_lambda(vec![], vec![make_expr_stmt(Expr::Unary(Unary {
        operator: dummy_token(),
        operand: Box::new(make_var("x")),
        span: dummy_span(),
    }))]);
    let free = compute_free_variables(&lambda);
    assert!(free.contains("x"));
}

#[test]
fn free_vars_call_expression() {
    // fn() { f(x, y) }  — f, x, y all captured
    let lambda = make_lambda(vec![], vec![make_expr_stmt(Expr::Call(Call {
        callee: Box::new(make_var("f")),
        arguments: vec![
            CallArg { name: None, value: make_var("x") },
            CallArg { name: None, value: make_var("y") },
        ],
        span: dummy_span(),
    }))]);
    let free = compute_free_variables(&lambda);
    assert!(free.contains("f"));
    assert!(free.contains("x"));
    assert!(free.contains("y"));
}

#[test]
fn free_vars_index_expression() {
    // fn() { arr[idx] }  — arr and idx captured
    let lambda = make_lambda(vec![], vec![make_expr_stmt(Expr::Index(Index {
        target: Box::new(make_var("arr")),
        index: Box::new(make_var("idx")),
        span: dummy_span(),
    }))]);
    let free = compute_free_variables(&lambda);
    assert!(free.contains("arr"));
    assert!(free.contains("idx"));
}

#[test]
fn free_vars_list_literal() {
    // fn() { [a, b, c] }  — all captured
    let lambda = make_lambda(vec![], vec![make_expr_stmt(Expr::List(ListLit {
        elements: vec![make_var("a"), make_var("b"), make_var("c")],
        span: dummy_span(),
    }))]);
    let free = compute_free_variables(&lambda);
    assert_eq!(free.len(), 3);
    assert!(free.contains("a"));
    assert!(free.contains("b"));
    assert!(free.contains("c"));
}

#[test]
fn free_vars_map_literal() {
    // fn() { {k: v} }  — k and v captured
    let lambda = make_lambda(vec![], vec![make_expr_stmt(Expr::Map(MapLit {
        entries: vec![MapEntry {
            key: make_var("k"),
            value: make_var("v"),
            span: dummy_span(),
        }],
        span: dummy_span(),
    }))]);
    let free = compute_free_variables(&lambda);
    assert!(free.contains("k"));
    assert!(free.contains("v"));
}

#[test]
fn free_vars_nested_lambda() {
    // fn(x) { fn(y) { x + y + z } }  — z is captured by inner, x by outer
    let inner_lambda = Expr::Lambda(Lambda {
        params: vec![make_param("y")],
        return_type: None,
        body: FnBlock {
            body: vec![make_expr_stmt(Expr::Binary(Binary {
                left: Box::new(Expr::Binary(Binary {
                    left: Box::new(make_var("x")),
                    operator: dummy_token(),
                    right: Box::new(make_var("y")),
                    span: dummy_span(),
                })),
                operator: dummy_token(),
                right: Box::new(make_var("z")),
                span: dummy_span(),
            }))],
            span: dummy_span(),
        },
        span: dummy_span(),
    });
    let lambda = make_lambda(vec!["x"], vec![make_expr_stmt(inner_lambda)]);
    let free = compute_free_variables(&lambda);
    // z is free in the inner lambda, which is in the outer lambda's body
    assert!(free.contains("z"));
    // x is bound by outer param, y is bound by inner param
    assert!(!free.contains("x"));
    assert!(!free.contains("y"));
}

// ── Try-catch ───────────────────────────────────────────────────────────

#[test]
fn free_vars_try_catch() {
    // fn() { try { x } catch e { e + y } }  — x and y captured, e is bound
    let lambda = make_lambda(vec![], vec![Stmt::Try(TryStmt {
        body: vec![make_expr_stmt(make_var("x"))],
        catch_clauses: vec![CatchClauseNode {
            error_type: dummy_type_ref("Error"),
            error_name: "e".into(),
            body: vec![make_expr_stmt(Expr::Binary(Binary {
                left: Box::new(make_var("e")),
                operator: dummy_token(),
                right: Box::new(make_var("y")),
                span: dummy_span(),
            }))],
            span: dummy_span(),
        }],
        catch_all: None,
        finally_block: None,
        span: dummy_span(),
    })]);
    let free = compute_free_variables(&lambda);
    assert!(free.contains("x"));
    assert!(free.contains("y"));
    assert!(!free.contains("e"));
}

// ── Assert and Throw ────────────────────────────────────────────────────

#[test]
fn free_vars_assert_statement() {
    // fn() { assert cond, "msg" }  — cond captured
    let lambda = make_lambda(vec![], vec![Stmt::Assert(AssertStmt {
        condition: Box::new(make_var("cond")),
        message: Some(Box::new(make_lit(LiteralValue::Str("msg".into())))),
        span: dummy_span(),
    })]);
    let free = compute_free_variables(&lambda);
    assert!(free.contains("cond"));
}

#[test]
fn free_vars_throw_statement() {
    // fn() { throw Error(err) }  — err captured
    let lambda = make_lambda(vec![], vec![Stmt::Throw(ThrowStmt {
        exception_type: dummy_type_ref("Error"),
        message: Some(Box::new(make_var("err"))),
        span: dummy_span(),
    })]);
    let free = compute_free_variables(&lambda);
    assert!(free.contains("err"));
}

// ── Access and Grouping ─────────────────────────────────────────────────

#[test]
fn free_vars_access_expression() {
    // fn() { obj.prop }  — obj captured
    let lambda = make_lambda(vec![], vec![make_expr_stmt(Expr::Access(Access {
        target: Box::new(make_var("obj")),
        property: "prop".into(),
        span: dummy_span(),
    }))]);
    let free = compute_free_variables(&lambda);
    assert!(free.contains("obj"));
}

#[test]
fn free_vars_grouping_expression() {
    // fn() { (x + y) }  — x and y captured
    let lambda = make_lambda(vec![], vec![make_expr_stmt(Expr::Grouping(Grouping {
        expression: Box::new(Expr::Binary(Binary {
            left: Box::new(make_var("x")),
            operator: dummy_token(),
            right: Box::new(make_var("y")),
            span: dummy_span(),
        })),
        span: dummy_span(),
    }))]);
    let free = compute_free_variables(&lambda);
    assert!(free.contains("x"));
    assert!(free.contains("y"));
}

// ── Pipe ────────────────────────────────────────────────────────────────

#[test]
fn free_vars_pipe_expression() {
    // fn() { x |> f }  — x and f captured
    let lambda = make_lambda(vec![], vec![make_expr_stmt(Expr::Pipe(Pipe {
        value: Box::new(make_var("x")),
        function: Box::new(make_var("f")),
        span: dummy_span(),
    }))]);
    let free = compute_free_variables(&lambda);
    assert!(free.contains("x"));
    assert!(free.contains("f"));
}
