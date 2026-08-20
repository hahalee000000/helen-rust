//! Extended tests for ast_printer module — AST node printing.

use helen_core::ast::*;
use helen_core::ast_printer::AstPrinter;
use helen_core::source::SourceSpan;
use helen_core::tokens::{LiteralValue, Token, TokenType};

fn dummy_span() -> SourceSpan {
    SourceSpan::new("t", 1, 1, 1, 2)
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

fn make_var(name: &str) -> Expr {
    Expr::Variable(Variable {
        name: name.into(),
        span: dummy_span(),
    })
}

fn make_lit(value: LiteralValue) -> Expr {
    Expr::Literal(Lit {
        value,
        span: dummy_span(),
    })
}

fn make_expr_stmt(expr: Expr) -> Stmt {
    Stmt::Expr(ExprStmt {
        expression: expr,
        span: dummy_span(),
    })
}

// ── Literals ────────────────────────────────────────────────────────────

#[test]
fn test_print_literal_int() {
    let printer = AstPrinter::new();
    let expr = make_lit(LiteralValue::Int(42.into()));
    let result = printer.print_expr(&expr);
    assert!(result.contains("42"));
}

#[test]
fn test_print_literal_float() {
    let printer = AstPrinter::new();
    // Use 2.718 instead of 3.14 to avoid clippy::approx_constant
    let expr = make_lit(LiteralValue::Float(1.234));
    let result = printer.print_expr(&expr);
    assert!(result.contains("1.234"));
}

#[test]
fn test_print_literal_str() {
    let printer = AstPrinter::new();
    let expr = make_lit(LiteralValue::Str("hello".into()));
    let result = printer.print_expr(&expr);
    assert!(result.contains("hello"));
}

#[test]
fn test_print_literal_bool() {
    let printer = AstPrinter::new();
    let expr = make_lit(LiteralValue::Bool(true));
    let result = printer.print_expr(&expr);
    assert!(result.contains("true") || result.contains("True"));
}

#[test]
fn test_print_literal_null() {
    let printer = AstPrinter::new();
    let expr = make_lit(LiteralValue::Null);
    let result = printer.print_expr(&expr);
    assert!(result.contains("null") || result.contains("None"));
}

// ── Variables ───────────────────────────────────────────────────────────

#[test]
fn test_print_variable() {
    let printer = AstPrinter::new();
    let expr = make_var("x");
    let result = printer.print_expr(&expr);
    assert_eq!(result, "x");
}

// ── Binary operations ───────────────────────────────────────────────────

#[test]
fn test_print_binary() {
    let printer = AstPrinter::new();
    let expr = Expr::Binary(Binary {
        left: Box::new(make_var("x")),
        operator: dummy_token(),
        right: Box::new(make_var("y")),
        span: dummy_span(),
    });
    let result = printer.print_expr(&expr);
    assert!(result.contains("+"));
    assert!(result.contains("x"));
    assert!(result.contains("y"));
}

// ── Unary operations ────────────────────────────────────────────────────

#[test]
fn test_print_unary() {
    let printer = AstPrinter::new();
    let expr = Expr::Unary(Unary {
        operator: dummy_token(),
        operand: Box::new(make_var("x")),
        span: dummy_span(),
    });
    let result = printer.print_expr(&expr);
    assert!(result.contains("x"));
}

// ── Grouping ────────────────────────────────────────────────────────────

#[test]
fn test_print_grouping() {
    let printer = AstPrinter::new();
    let expr = Expr::Grouping(Grouping {
        expression: Box::new(make_var("x")),
        span: dummy_span(),
    });
    let result = printer.print_expr(&expr);
    assert!(result.contains("x"));
}

// ── Call ────────────────────────────────────────────────────────────────

#[test]
fn test_print_call() {
    let printer = AstPrinter::new();
    let expr = Expr::Call(Call {
        callee: Box::new(make_var("f")),
        arguments: vec![
            CallArg {
                name: None,
                value: make_var("x"),
            },
            CallArg {
                name: None,
                value: make_var("y"),
            },
        ],
        span: dummy_span(),
    });
    let result = printer.print_expr(&expr);
    assert!(result.contains("f"));
    assert!(result.contains("x"));
    assert!(result.contains("y"));
}

// ── Index ───────────────────────────────────────────────────────────────

#[test]
fn test_print_index() {
    let printer = AstPrinter::new();
    let expr = Expr::Index(Index {
        target: Box::new(make_var("arr")),
        index: Box::new(make_lit(LiteralValue::Int(0.into()))),
        span: dummy_span(),
    });
    let result = printer.print_expr(&expr);
    assert!(result.contains("arr"));
}

// ── Access ──────────────────────────────────────────────────────────────

#[test]
fn test_print_access() {
    let printer = AstPrinter::new();
    let expr = Expr::Access(Access {
        target: Box::new(make_var("obj")),
        property: "prop".into(),
        span: dummy_span(),
    });
    let result = printer.print_expr(&expr);
    assert!(result.contains("obj"));
    assert!(result.contains("prop"));
}

// ── List literal ────────────────────────────────────────────────────────

#[test]
fn test_print_list() {
    let printer = AstPrinter::new();
    let expr = Expr::List(ListLit {
        elements: vec![
            make_lit(LiteralValue::Int(1.into())),
            make_lit(LiteralValue::Int(2.into())),
        ],
        span: dummy_span(),
    });
    let result = printer.print_expr(&expr);
    assert!(result.contains("list"));
}

// ── Map literal ─────────────────────────────────────────────────────────

#[test]
fn test_print_map() {
    let printer = AstPrinter::new();
    let expr = Expr::Map(MapLit {
        entries: vec![MapEntry {
            key: make_lit(LiteralValue::Str("k".into())),
            value: make_lit(LiteralValue::Int(1.into())),
            span: dummy_span(),
        }],
        span: dummy_span(),
    });
    let result = printer.print_expr(&expr);
    assert!(result.contains("map"));
}

// ── Statements ──────────────────────────────────────────────────────────

#[test]
fn test_print_var_decl() {
    let printer = AstPrinter::new();
    let stmt = Stmt::VarDecl(VarDecl {
        name: "x".into(),
        type_annotation: None,
        initializer: Some(Box::new(make_lit(LiteralValue::Int(42.into())))),
        mutable: true,
        span: dummy_span(),
        shared: false,
    });
    let result = printer.print_stmt(&stmt);
    assert!(result.contains("x"));
}

#[test]
fn test_print_if_stmt() {
    let printer = AstPrinter::new();
    let stmt = Stmt::If(IfStmt {
        condition: Box::new(make_lit(LiteralValue::Bool(true))),
        then_branch: Box::new(make_expr_stmt(make_var("x"))),
        else_branch: None,
        span: dummy_span(),
    });
    let result = printer.print_stmt(&stmt);
    assert!(result.contains("if"));
}

#[test]
fn test_print_for_stmt() {
    let printer = AstPrinter::new();
    let stmt = Stmt::For(ForStmt {
        iterator: Some(Variable {
            name: "i".into(),
            span: dummy_span(),
        }),
        iterable: Box::new(make_var("items")),
        body: Box::new(make_expr_stmt(make_var("i"))),
        span: dummy_span(),
    });
    let result = printer.print_stmt(&stmt);
    assert!(result.contains("for"));
}

#[test]
fn test_print_while_stmt() {
    let printer = AstPrinter::new();
    let stmt = Stmt::While(WhileStmt {
        condition: Box::new(make_lit(LiteralValue::Bool(true))),
        body: Box::new(make_expr_stmt(make_var("x"))),
        span: dummy_span(),
    });
    let result = printer.print_stmt(&stmt);
    assert!(result.contains("while"));
}

#[test]
fn test_print_return_stmt() {
    let printer = AstPrinter::new();
    let stmt = Stmt::Return(ReturnStmt {
        value: Some(Box::new(make_var("x"))),
        span: dummy_span(),
    });
    let result = printer.print_stmt(&stmt);
    assert!(result.contains("return"));
}

#[test]
fn test_print_break() {
    let printer = AstPrinter::new();
    let stmt = Stmt::Break(BreakStmt { span: dummy_span() });
    let result = printer.print_stmt(&stmt);
    assert_eq!(result, "(break)");
}

#[test]
fn test_print_continue() {
    let printer = AstPrinter::new();
    let stmt = Stmt::Continue(ContinueStmt { span: dummy_span() });
    let result = printer.print_stmt(&stmt);
    assert_eq!(result, "(continue)");
}

// ── Program ─────────────────────────────────────────────────────────────

#[test]
fn test_print_program() {
    let printer = AstPrinter::new();
    let program = Program {
        statements: vec![
            make_expr_stmt(make_lit(LiteralValue::Int(1.into()))),
            make_expr_stmt(make_lit(LiteralValue::Int(2.into()))),
        ],
        span: dummy_span(),
    };
    let result = printer.print_program(&program);
    assert!(result.contains("program"));
}
