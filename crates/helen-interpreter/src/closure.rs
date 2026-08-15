//! Closures and free-variable analysis.
//!
//! Byte-faithful port of `helen/interpreter/closure.py` (v1.44.0).
//! v1.12 rule: closures capture **values** at creation (deep copies of
//! mutable types), not environment references — snapshot semantics.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use helen_core::ast::{Expr, Lambda, Stmt};

use crate::environment::Environment;

/// A closure: a lambda plus its captured (snapshot) environment.
#[derive(Clone, Debug)]
pub struct Closure {
    pub lambda: Rc<Lambda>,
    pub captured_env: Rc<RefCell<Environment>>,
    /// v1.18: name of the `let`/`const` the closure was assigned to, so
    /// `_call_closure` can inject the closure itself for self-recursion.
    pub self_name: Option<String>,
}

impl Closure {
    pub fn new(lambda: Rc<Lambda>, captured_env: Rc<RefCell<Environment>>) -> Self {
        Closure {
            lambda,
            captured_env,
            self_name: None,
        }
    }
}

/// `_compute_free_variables(lambda_node)` — variables used in the body that
/// are NOT bound by params or local declarations. These get captured.
pub fn compute_free_variables(lambda: &Lambda) -> HashSet<String> {
    let mut bound: HashSet<String> = lambda.params.iter().map(|p| p.name.clone()).collect();
    let mut used: HashSet<String> = HashSet::new();
    for stmt in &lambda.body.body {
        collect_variable_refs_stmt(stmt, &mut bound, &mut used);
    }
    used
}

/// `_collect_variable_refs` for statements.
fn collect_variable_refs_stmt(
    stmt: &Stmt,
    bound: &mut HashSet<String>,
    used: &mut HashSet<String>,
) {
    match stmt {
        Stmt::VarDecl(v) => {
            if let Some(init) = &v.initializer {
                collect_variable_refs_expr(init, bound, used);
            }
            // After this declaration the variable is bound for subsequent code
            bound.insert(v.name.clone());
        }
        Stmt::If(if_stmt) => {
            collect_variable_refs_expr(&if_stmt.condition, bound, used);
            let mut then_bound = bound.clone();
            collect_variable_refs_stmt(&if_stmt.then_branch, &mut then_bound, used);
            if let Some(else_branch) = &if_stmt.else_branch {
                let mut else_bound = bound.clone();
                collect_variable_refs_stmt(else_branch, &mut else_bound, used);
            }
        }
        Stmt::For(f) => {
            collect_variable_refs_expr(&f.iterable, bound, used);
            let mut body_bound = bound.clone();
            if let Some(iter) = &f.iterator {
                body_bound.insert(iter.name.clone());
            }
            collect_variable_refs_stmt(&f.body, &mut body_bound, used);
        }
        Stmt::While(w) => {
            collect_variable_refs_expr(&w.condition, bound, used);
            collect_variable_refs_stmt(&w.body, bound, used);
        }
        Stmt::Return(r) => {
            if let Some(v) = &r.value {
                collect_variable_refs_expr(v, bound, used);
            }
        }
        Stmt::Expr(e) => collect_variable_refs_expr(&e.expression, bound, used),
        Stmt::FnBlock(fb) => {
            for s in &fb.body {
                collect_variable_refs_stmt(s, bound, used);
            }
        }
        Stmt::Try(t) => {
            for s in &t.body {
                collect_variable_refs_stmt(s, bound, used);
            }
            for c in &t.catch_clauses {
                let mut cb = bound.clone();
                cb.insert(c.error_name.clone());
                for s in &c.body {
                    collect_variable_refs_stmt(s, &mut cb, used);
                }
            }
            if let Some(ca) = &t.catch_all {
                for s in &ca.body {
                    collect_variable_refs_stmt(s, bound, used);
                }
            }
            if let Some(fb) = &t.finally_block {
                for s in &fb.body {
                    collect_variable_refs_stmt(s, bound, used);
                }
            }
        }
        Stmt::Throw(t) => {
            if let Some(m) = &t.message {
                collect_variable_refs_expr(m, bound, used);
            }
        }
        Stmt::Assert(a) => {
            collect_variable_refs_expr(&a.condition, bound, used);
            if let Some(m) = &a.message {
                collect_variable_refs_expr(m, bound, used);
            }
        }
        Stmt::Match(m) => {
            collect_variable_refs_expr(&m.subject, bound, used);
            for c in &m.cases {
                collect_variable_refs_expr(&c.pattern, bound, used);
                let mut cb = bound.clone();
                for s in &c.body {
                    collect_variable_refs_stmt(s, &mut cb, used);
                }
                if let Some(g) = &c.guard {
                    collect_variable_refs_expr(g, &mut cb, used);
                }
            }
            for s in &m.default {
                collect_variable_refs_stmt(s, bound, used);
            }
        }
        Stmt::LlmIf(li) => {
            collect_variable_refs_expr(&li.description, bound, used);
            for b in &li.branches {
                if let Some(c) = &b.condition {
                    collect_variable_refs_expr(c, bound, used);
                }
                for s in &b.body {
                    collect_variable_refs_stmt(s, bound, used);
                }
            }
        }
        Stmt::LlmBranch(lb) => {
            if let Some(c) = &lb.condition {
                collect_variable_refs_expr(c, bound, used);
            }
            for s in &lb.body {
                collect_variable_refs_stmt(s, bound, used);
            }
        }
        Stmt::Import(_)
        | Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::PromptDef(_)
        | Stmt::Declaration(_)
        | Stmt::AgentParam(_)
        | Stmt::ContextConfig(_)
        | Stmt::AgentDecl(_)
        | Stmt::MainBlock(_)
        | Stmt::FunctionDecl(_)
        | Stmt::ProtocolDecl(_)
        | Stmt::ImplDecl(_)
        | Stmt::Alias(_)
        | Stmt::Case(_)
        | Stmt::CatchClause(_)
        | Stmt::CatchAll(_)
        | Stmt::FinallyBlock(_)
        | Stmt::SharedStoreDecl(_) => {}
    }
}

/// `_collect_variable_refs` for expressions.
fn collect_variable_refs_expr(
    expr: &Expr,
    bound: &mut HashSet<String>,
    used: &mut HashSet<String>,
) {
    match expr {
        Expr::Variable(v) => {
            if !bound.contains(&v.name) {
                used.insert(v.name.clone());
            }
        }
        Expr::Binary(b) => {
            collect_variable_refs_expr(&b.left, bound, used);
            collect_variable_refs_expr(&b.right, bound, used);
        }
        Expr::Unary(u) => collect_variable_refs_expr(&u.operand, bound, used),
        Expr::Grouping(g) => collect_variable_refs_expr(&g.expression, bound, used),
        Expr::Call(c) => {
            collect_variable_refs_expr(&c.callee, bound, used);
            for a in &c.arguments {
                collect_variable_refs_expr(&a.value, bound, used);
            }
        }
        Expr::Index(i) => {
            collect_variable_refs_expr(&i.target, bound, used);
            collect_variable_refs_expr(&i.index, bound, used);
        }
        Expr::Access(a) => collect_variable_refs_expr(&a.target, bound, used),
        Expr::List(l) => {
            for e in &l.elements {
                collect_variable_refs_expr(e, bound, used);
            }
        }
        Expr::Map(m) => {
            for e in &m.entries {
                collect_variable_refs_expr(&e.key, bound, used);
                collect_variable_refs_expr(&e.value, bound, used);
            }
        }
        Expr::TemplateRef(t) => collect_variable_refs_expr(&t.expression, bound, used),
        Expr::Pipe(p) => {
            collect_variable_refs_expr(&p.value, bound, used);
            collect_variable_refs_expr(&p.function, bound, used);
        }
        Expr::Lambda(l) => {
            let mut lbound = bound.clone();
            for p in &l.params {
                lbound.insert(p.name.clone());
            }
            for s in &l.body.body {
                collect_variable_refs_stmt(s, &mut lbound, used);
            }
        }
        Expr::MatchExpr(m) => {
            collect_variable_refs_expr(&m.subject, bound, used);
            for c in &m.cases {
                collect_variable_refs_expr(&c.pattern, bound, used);
                for s in &c.body {
                    collect_variable_refs_stmt(s, bound, used);
                }
                if let Some(g) = &c.guard {
                    collect_variable_refs_expr(g, bound, used);
                }
            }
            if let Some(d) = &m.default_body {
                collect_variable_refs_expr(d, bound, used);
            }
        }
        Expr::LlmAct(la) => {
            if let Some(p) = &la.prompt {
                collect_variable_refs_expr(p, bound, used);
            }
        }
        Expr::Spawn(sp) => collect_variable_refs_expr(&Expr::Call(*sp.call.clone()), bound, used),
        Expr::Literal(_)
        | Expr::Type(_)
        | Expr::OptionalType(_)
        | Expr::UnionType(_)
        | Expr::LiteralType(_)
        | Expr::RangePattern(_)
        | Expr::WildcardPattern(_)
        | Expr::VariablePattern(_)
        | Expr::TypePattern(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use helen_core::ast::*;
    use helen_core::source::SourceSpan;
    use helen_core::tokens::{LiteralValue, Token, TokenType};

    #[test]
    fn free_vars_captures_outer_only() {
        // fn(x) { y + x }  — y is free, x is a param
        let lambda = Lambda {
            params: vec![AgentParam {
                name: "x".into(),
                type_annotation: None,
                default_value: None,
                span: SourceSpan {
                    file: "t".into(),
                    start_line: 1,
                    start_col: 1,
                    end_line: 1,
                    end_col: 2,
                },
            }],
            return_type: None,
            body: FnBlock {
                body: vec![Stmt::Expr(ExprStmt {
                    expression: Expr::Binary(Binary {
                        left: Box::new(Expr::Variable(Variable {
                            name: "y".into(),
                            span: SourceSpan {
                                file: "t".into(),
                                start_line: 1,
                                start_col: 1,
                                end_line: 1,
                                end_col: 2,
                            },
                        })),
                        operator: Token {
                            kind: TokenType::Plus,
                            lexeme: "+".into(),
                            literal: LiteralValue::Null,
                            line: 1,
                            col: 1,
                            end_line: 1,
                            end_col: 2,
                            file: "t".into(),
                        },
                        right: Box::new(Expr::Variable(Variable {
                            name: "x".into(),
                            span: SourceSpan {
                                file: "t".into(),
                                start_line: 1,
                                start_col: 1,
                                end_line: 1,
                                end_col: 2,
                            },
                        })),
                        span: SourceSpan {
                            file: "t".into(),
                            start_line: 1,
                            start_col: 1,
                            end_line: 1,
                            end_col: 2,
                        },
                    }),
                    span: SourceSpan {
                        file: "t".into(),
                        start_line: 1,
                        start_col: 1,
                        end_line: 1,
                        end_col: 2,
                    },
                })],
                span: SourceSpan {
                    file: "t".into(),
                    start_line: 1,
                    start_col: 1,
                    end_line: 1,
                    end_col: 2,
                },
            },
            span: SourceSpan {
                file: "t".into(),
                start_line: 1,
                start_col: 1,
                end_line: 1,
                end_col: 2,
            },
        };
        let free = compute_free_variables(&lambda);
        assert!(free.contains("y"));
        assert!(!free.contains("x"));
    }
}
