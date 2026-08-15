//! The Helen interpreter — statements, expressions, calls, exceptions.
//!
//! Byte-faithful port of `helen/interpreter/interpreter.py` plus the
//! `exception_mixin` and `pattern_mixin` (v1.44.0). LLM/agent/spawn and
//! import machinery are stubbed or minimal in M3 and completed in later
//! milestones.

// `ExceptionValue` mirrors Python's exception object: message, class name,
// span, cause chain and metadata — necessarily a large `Err` payload.
#![allow(clippy::result_large_err)]

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use num_bigint::BigInt;
use num_traits::{Signed, ToPrimitive, Zero};

use helen_core::ast::{
    Access, AgentDecl, Binary, Call, CallArg, Expr, ForStmt, FunctionDecl, IfStmt, Index, Lambda,
    Pipe, Program, Stmt, ThrowStmt, TypeRef, Unary, VarDecl,
};
use helen_core::source::SourceSpan;
use helen_core::tokens::{LiteralValue, Token, TokenType};
use helen_semantic::types::{type_compatible, Type};

use crate::closure::{compute_free_variables, Closure};
use crate::environment::Environment;
use crate::exceptions::{error_matches, resolve_exception, ExceptionValue, Flow};
use crate::value::{BuiltinFn, Value};

/// Runtime type-check results used by `_call_function`/`_call_closure`.
fn type_of_value(v: &Value) -> Type {
    match v {
        Value::Null => Type::Null,
        Value::Bool(_) => Type::Bool,
        Value::Int(_) => Type::Int,
        Value::Float(_) => Type::Float,
        Value::Str(_) => Type::Str,
        _ => Type::Any,
    }
}

/// The Helen interpreter.
pub struct Interpreter {
    pub environment: Rc<RefCell<Environment>>,
    pub functions: HashMap<String, Rc<FunctionDecl>>,
    pub agents: HashMap<String, Rc<AgentDecl>>,
    pub protocols: HashMap<String, Rc<helen_core::ast::ProtocolDecl>>,
    pub impls: HashMap<(String, String), Rc<helen_core::ast::ImplDecl>>,
    /// v1.10: names of `shared let` variables (cross-agent visibility).
    pub shared_vars: HashSet<String>,
    /// v1.39: maps imported function name -> its defining module env.
    pub function_module_envs: HashMap<String, Rc<RefCell<Environment>>>,
    pub program_args: Vec<String>,
    pub builtins: HashMap<String, Rc<BuiltinFn>>,
    /// v1.22: current agent (None at top level).
    pub current_agent: Option<Rc<AgentDecl>>,
    /// v1.12: shared store instance cache (import reuse).
    pub shared_store_instances: HashMap<String, Value>,
    /// Captured stdout (Python redirects sys.stdout around `interpret`).
    pub stdout: RefCell<String>,
}

impl Interpreter {
    pub fn new() -> Self {
        let mut interp = Interpreter {
            environment: Rc::new(RefCell::new(Environment::new(None))),
            functions: HashMap::new(),
            agents: HashMap::new(),
            protocols: HashMap::new(),
            impls: HashMap::new(),
            shared_vars: HashSet::new(),
            function_module_envs: HashMap::new(),
            program_args: Vec::new(),
            builtins: HashMap::new(),
            current_agent: None,
            shared_store_instances: HashMap::new(),
            stdout: RefCell::new(String::new()),
        };
        interp.register_core_builtins();
        interp
    }

    fn register_builtin(&mut self, name: &str, func: BuiltinImpl) {
        let bf = Rc::new(BuiltinFn {
            name: name.to_string(),
            module: "core",
            func,
        });
        self.builtins.insert(name.to_string(), bf.clone());
    }

    /// Register all exports of a stdlib module into `self.builtins`
    /// (canonical name -> BuiltinFn). Idempotent.
    fn register_stdlib_module(&mut self, module: &str) {
        if module == "std.core" {
            return; // already registered in register_core_builtins
        }
        let Some(exports) = crate::stdlib::module_exports(module) else {
            return;
        };
        let tag = crate::stdlib::module_tag(module);
        for e in exports {
            let bf = Rc::new(BuiltinFn {
                name: e.name.to_string(),
                module: tag,
                func: e.func,
            });
            self.builtins.insert(e.name.to_string(), bf.clone());
        }
    }

    /// Execute a stdlib module import (v1.34/v1.38): the three forms are
    /// `import std.X.*`, `import std.X.{a,b}`, and `import std.X as NS`.
    fn import_stdlib_module(
        &mut self,
        module: &str,
        imported_names: Option<&[String]>,
        namespace: Option<&str>,
        span: &SourceSpan,
    ) -> Result<(), ExceptionValue> {
        // std.core exports live in the `builtins` map; other modules are
        // registered on first use (idempotent).
        if module != "std.core" && crate::stdlib::module_exports(module).is_none() {
            return Err(self.runtime_error(
                Some(span),
                &format!(
                    "Unknown stdlib module '{module}'. Available: std.core, std.str, std.list, std.dict, std.math, std.debug"
                ),
            ));
        }
        self.register_stdlib_module(module);

        let all_names: Vec<String> = if module == "std.core" {
            crate::stdlib::CORE_EXPORTS
                .iter()
                .map(|s| s.to_string())
                .collect()
        } else {
            crate::stdlib::module_exports(module)
                .unwrap()
                .iter()
                .map(|e| e.name.to_string())
                .collect()
        };

        if let Some(ns) = namespace {
            // Namespace import: `import std.X as NS` — a module object (map)
            // of name -> BuiltinFn, defined const (Python: is_const=True).
            let mut m = indexmap::IndexMap::new();
            for name in &all_names {
                if let Some(bf) = self.builtins.get(name) {
                    m.insert(
                        Value::Str(Rc::from(name.as_str())),
                        Value::BuiltinFn(bf.clone()),
                    );
                }
            }
            let module_obj = Value::Map(Rc::new(RefCell::new(m)));
            self.environment.borrow_mut().define(ns, module_obj, true);
            return Ok(());
        }

        let star = imported_names
            .map(|n| n.iter().any(|x| x == "*"))
            .unwrap_or(true);
        if star {
            // Wildcard: bind all exports + Chinese aliases (v1.39).
            for name in &all_names {
                if let Some(bf) = self.builtins.get(name) {
                    self.environment
                        .borrow_mut()
                        .define(name, Value::BuiltinFn(bf.clone()), true);
                }
            }
            let aliases: Vec<(String, String)> = helen_semantic::stdlib::all_aliases();
            for (alias, canonical) in aliases {
                if all_names.contains(&canonical) {
                    if let Some(bf) = self.builtins.get(&canonical) {
                        self.environment.borrow_mut().define(
                            &alias,
                            Value::BuiltinFn(bf.clone()),
                            true,
                        );
                    }
                }
            }
            return Ok(());
        }

        // Selective: `import std.X.{a, b}` — error on unknown names.
        let names = imported_names.unwrap_or_default();
        for name in names {
            if !all_names.contains(name) {
                return Err(self.runtime_error(
                    Some(span),
                    &format!(
                        "Function '{name}' not found in module '{module}'. Available: {}",
                        all_names.join(", ")
                    ),
                ));
            }
        }
        for name in names {
            if let Some(bf) = self.builtins.get(name) {
                self.environment
                    .borrow_mut()
                    .define(name, Value::BuiltinFn(bf.clone()), true);
            }
        }
        Ok(())
    }

    fn register_core_builtins(&mut self) {
        // M3: core subset of std.core — bound into the global environment so
        // `import std.core.*` programs resolve them (import machinery is
        // Task 3.7; this mirrors its observable binding for core names).
        let names = [
            "print",
            "len",
            "str",
            "int",
            "float",
            "bool",
            "type",
            "isinstance",
            "range",
            "abs",
            "min",
            "max",
            "list",
            "dict",
        ];
        for n in names {
            self.register_builtin(
                n,
                match n {
                    "print" => builtin_print,
                    "len" => builtin_len,
                    "str" => builtin_str,
                    "int" => builtin_int,
                    "float" => builtin_float,
                    "bool" => builtin_bool,
                    "type" => builtin_type,
                    "isinstance" => builtin_isinstance,
                    "range" => builtin_range,
                    "abs" => builtin_abs,
                    "min" => builtin_min,
                    "max" => builtin_max,
                    "list" => builtin_list,
                    _ => builtin_dict,
                },
            );
        }
    }

    // ------------------------------------------------------------------
    // Entry points
    // ------------------------------------------------------------------

    /// `interpret(program)` — execute and unwrap top-level sentinels.
    pub fn interpret(&mut self, program: &Program) -> Result<Option<Value>, ExceptionValue> {
        let flow = self.execute_stmts(&program.statements)?;
        Ok(match flow {
            Flow::Return(v) => v,
            Flow::Break | Flow::Continue => None,
            Flow::Normal(v) => v,
        })
    }

    /// Execute a list of statements, propagating sentinels.
    pub fn execute_stmts(&mut self, stmts: &[Stmt]) -> Result<Flow, ExceptionValue> {
        let mut result = Flow::Normal(None);
        for stmt in stmts {
            let step = self.execute_stmt(stmt)?;
            match step {
                Flow::Normal(_) => result = step,
                _ => return Ok(step),
            }
        }
        Ok(result)
    }

    /// Execute a single statement.
    pub fn execute_stmt(&mut self, stmt: &Stmt) -> Result<Flow, ExceptionValue> {
        match stmt {
            Stmt::VarDecl(v) => {
                self.visit_var_decl(v)?;
                Ok(Flow::Normal(None))
            }
            Stmt::If(s) => self.visit_if(s),
            Stmt::For(s) => self.visit_for(s),
            Stmt::While(s) => self.visit_while(s),
            Stmt::Break(_) => Ok(Flow::Break),
            Stmt::Continue(_) => Ok(Flow::Continue),
            Stmt::Return(r) => {
                let value = if let Some(v) = &r.value {
                    Some(self.eval_expr(v)?)
                } else {
                    None
                };
                Ok(Flow::Return(value))
            }
            Stmt::Expr(e) => {
                self.eval_expr(&e.expression)?;
                Ok(Flow::Normal(None))
            }
            Stmt::FunctionDecl(f) => {
                self.visit_function_decl(f);
                Ok(Flow::Normal(None))
            }
            Stmt::FnBlock(fb) => self.execute_stmts(&fb.body),
            Stmt::MainBlock(mb) => self.with_scope(None, |s| s.execute_stmts(&mb.body)),
            Stmt::AgentDecl(a) => {
                let rc = Rc::new(a.clone());
                self.current_agent = Some(rc.clone());
                self.agents.insert(a.name.clone(), rc);
                Ok(Flow::Normal(None))
            }
            Stmt::ProtocolDecl(p) => {
                self.protocols.insert(p.name.clone(), Rc::new(p.clone()));
                Ok(Flow::Normal(None))
            }
            Stmt::ImplDecl(im) => {
                let rc = Rc::new(im.clone());
                self.impls
                    .insert((im.protocol_name.clone(), im.struct_name.clone()), rc);
                for m in &im.methods {
                    self.functions.insert(m.name.clone(), Rc::new(m.clone()));
                }
                Ok(Flow::Normal(None))
            }
            Stmt::Try(t) => self.visit_try(t),
            Stmt::Throw(t) => Err(self.make_throw(t)?),
            Stmt::Assert(a) => {
                self.visit_assert(a)?;
                Ok(Flow::Normal(None))
            }
            Stmt::Match(m) => self.visit_match_stmt(m),
            Stmt::Alias(a) => {
                self.visit_alias(a);
                Ok(Flow::Normal(None))
            }
            Stmt::Import(imp) => {
                // v1.34/v1.38 stdlib module imports: wildcard, selective,
                // and namespace (`as NS`) forms. Non-stdlib file imports are
                // Task 3.7b (file resolver).
                if imp.is_stdlib_module {
                    let module = imp.module_name.clone().unwrap_or_default();
                    let names = imp.imported_names.clone();
                    let ns = imp.namespace.clone();
                    let span = imp.span.clone();
                    self.import_stdlib_module(&module, names.as_deref(), ns.as_deref(), &span)?;
                }
                Ok(Flow::Normal(None))
            }
            Stmt::SharedStoreDecl(_) => {
                // Shared stores are implemented with the import resolver (3.7).
                Ok(Flow::Normal(None))
            }
            Stmt::LlmIf(li) => self.visit_llm_if(li),
            Stmt::LlmBranch(lb) => {
                let b = lb.clone();
                if let Some(c) = &b.condition {
                    let v = self.eval_expr(c)?;
                    if !v.truthy() {
                        return Ok(Flow::Normal(None));
                    }
                }
                self.execute_stmts(&b.body)
            }
            Stmt::Case(_)
            | Stmt::CatchClause(_)
            | Stmt::CatchAll(_)
            | Stmt::FinallyBlock(_)
            | Stmt::PromptDef(_)
            | Stmt::Declaration(_)
            | Stmt::AgentParam(_)
            | Stmt::ContextConfig(_) => Ok(Flow::Normal(None)),
        }
    }

    /// Evaluate an expression.
    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value, ExceptionValue> {
        match expr {
            Expr::Literal(l) => Ok(self.value_from_literal(&l.value)),
            Expr::Variable(v) => self.visit_variable(&v.name, &v.span),
            Expr::Binary(b) => self.visit_binary(b),
            Expr::Pipe(p) => self.visit_pipe(p),
            Expr::Unary(u) => self.visit_unary(u),
            Expr::Grouping(g) => self.eval_expr(&g.expression),
            Expr::Call(c) => self.visit_call(c),
            Expr::Index(i) => self.visit_index(i),
            Expr::Access(a) => self.visit_access(a),
            Expr::List(l) => {
                let mut items = Vec::with_capacity(l.elements.len());
                for e in &l.elements {
                    items.push(self.eval_expr(e)?);
                }
                Ok(Value::List(Rc::new(RefCell::new(items))))
            }
            Expr::Map(m) => {
                let mut map = indexmap::IndexMap::new();
                for e in &m.entries {
                    let k = self.eval_expr(&e.key)?;
                    let v = self.eval_expr(&e.value)?;
                    map.insert(k, v);
                }
                Ok(Value::Map(Rc::new(RefCell::new(map))))
            }
            Expr::TemplateRef(t) => self.eval_expr(&t.expression),
            Expr::Lambda(l) => self.visit_lambda(l),
            Expr::MatchExpr(m) => self.visit_match_expr(m),
            Expr::Type(_) | Expr::OptionalType(_) | Expr::UnionType(_) | Expr::LiteralType(_) => {
                Ok(Value::Null)
            }
            Expr::RangePattern(r) => {
                let start = self.eval_expr(&r.start)?;
                let end = self.eval_expr(&r.end)?;
                Ok(Value::Range(Box::new(start), Box::new(end)))
            }
            Expr::WildcardPattern(_) => Ok(Value::Null),
            Expr::VariablePattern(vp) => {
                // Not reachable via normal expression evaluation; handled in match.
                self.visit_variable(&vp.name, &vp.span)
            }
            Expr::TypePattern(_) => Ok(Value::Null),
            Expr::LlmAct(la) => self.visit_llm_act(la),
            Expr::Spawn(sp) => {
                let call = sp.call.clone();
                let result = self.visit_call(&call)?;
                Ok(result)
            }
        }
    }

    fn value_from_literal(&self, lit: &LiteralValue) -> Value {
        match lit {
            LiteralValue::Null => Value::Null,
            LiteralValue::Bool(b) => Value::Bool(*b),
            LiteralValue::Str(s) => Value::Str(Rc::from(s.as_str())),
            LiteralValue::Int(i) => Value::Int(i.clone()),
            LiteralValue::Float(f) => Value::Float(*f),
        }
    }

    /// `_push_scope` — run `f` with a fresh child scope (or a given env).
    fn with_scope<T>(
        &mut self,
        set_to: Option<Rc<RefCell<Environment>>>,
        f: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let old = self.environment.clone();
        self.environment = set_to.unwrap_or_else(|| Environment::child(old.clone()));
        let r = f(self);
        self.environment = old;
        r
    }

    // ------------------------------------------------------------------
    // Variable / statements
    // ------------------------------------------------------------------

    fn visit_var_decl(&mut self, v: &VarDecl) -> Result<(), ExceptionValue> {
        let is_const = !v.mutable;
        let is_lambda_init =
            matches!(&v.initializer, Some(e) if matches!(e.as_ref(), Expr::Lambda(_)));

        if is_lambda_init {
            self.environment
                .borrow_mut()
                .define(&v.name, Value::Null, false);
        }

        let mut value = Value::Null;
        if let Some(init) = &v.initializer {
            value = self.eval_expr(init)?;
        }

        if is_lambda_init {
            if let Value::Closure(c) = &value {
                let mut c: crate::closure::Closure = c.as_ref().clone();
                c.self_name = Some(v.name.clone());
                value = Value::Closure(Rc::new(c));
            }
        }

        self.environment
            .borrow_mut()
            .define(&v.name, value, is_const);
        if v.shared {
            self.shared_vars.insert(v.name.clone());
        }
        Ok(())
    }

    fn visit_if(&mut self, s: &IfStmt) -> Result<Flow, ExceptionValue> {
        let condition = self.eval_expr(&s.condition)?;
        if condition.truthy() {
            return self.execute_stmt(&s.then_branch);
        }
        if let Some(else_branch) = &s.else_branch {
            return self.execute_stmt(else_branch);
        }
        Ok(Flow::Normal(None))
    }

    fn visit_for(&mut self, s: &ForStmt) -> Result<Flow, ExceptionValue> {
        let iterable = self.eval_expr(&s.iterable)?;
        let items: Vec<Value> = match &iterable {
            Value::List(l) => l.borrow().clone(),
            _ => {
                return Err(self.runtime_error(
                    Some(&s.span),
                    &format!("Cannot iterate over {}", iterable.type_name()),
                ))
            }
        };

        let mut result = Flow::Normal(None);
        for item in items {
            let step = self.with_scope(None, |interp| {
                if let Some(iter) = &s.iterator {
                    interp
                        .environment
                        .borrow_mut()
                        .define(&iter.name, item.clone(), false);
                }
                interp.execute_stmt(&s.body)
            })?;
            match step {
                Flow::Break => return Ok(result),
                Flow::Continue => continue,
                Flow::Return(_) => return Ok(step),
                Flow::Normal(v) => result = Flow::Normal(v),
            }
        }
        Ok(result)
    }

    fn visit_while(&mut self, s: &helen_core::ast::WhileStmt) -> Result<Flow, ExceptionValue> {
        let mut result = Flow::Normal(None);
        loop {
            let condition = self.eval_expr(&s.condition)?;
            if !condition.truthy() {
                break;
            }
            let step = self.execute_stmt(&s.body)?;
            match step {
                Flow::Break => break,
                Flow::Continue => continue,
                Flow::Return(_) => return Ok(step),
                Flow::Normal(v) => result = Flow::Normal(v),
            }
        }
        Ok(result)
    }

    fn visit_function_decl(&mut self, f: &FunctionDecl) {
        self.functions.insert(f.name.clone(), Rc::new(f.clone()));
    }

    // ------------------------------------------------------------------
    // Expressions
    // ------------------------------------------------------------------

    fn visit_variable(&mut self, name: &str, span: &SourceSpan) -> Result<Value, ExceptionValue> {
        if let Some(v) = self.environment.borrow().get(name) {
            return Ok(v);
        }
        if let Some(f) = self.functions.get(name) {
            return Ok(Value::UserFn(f.clone()));
        }
        if let Some(a) = self.agents.get(name) {
            return Ok(Value::Agent(a.clone()));
        }
        Err(self.runtime_error(Some(span), &format!("Undefined variable '{name}'")))
    }

    fn visit_binary(&mut self, b: &Binary) -> Result<Value, ExceptionValue> {
        let op = b.operator.kind;

        if op == TokenType::Assign {
            let right = self.eval_expr(&b.right)?;
            match b.left.as_ref() {
                Expr::Variable(v) => {
                    if self.environment.borrow().is_const(&v.name) {
                        return Err(ExceptionValue::new(
                            "ConstAssignmentError",
                            format!("cannot assign to const variable '{}'", v.name),
                            Some(b.span.clone()),
                        ));
                    }
                    match self.environment.borrow_mut().assign(&v.name, right.clone()) {
                        Ok(_) => Ok(right),
                        Err(_) => Err(self.runtime_error(
                            Some(&b.span),
                            &format!("Undefined variable '{}'", v.name),
                        )),
                    }
                }
                Expr::Index(i) => {
                    let target = self.eval_expr(&i.target)?;
                    let index = self.eval_expr(&i.index)?;
                    self.assign_index(&target, &index, right, &b.span)
                }
                Expr::Access(a) => {
                    let target = self.eval_expr(&a.target)?;
                    self.assign_access(&target, &a.property, right, &b.span)
                }
                _ => Err(self.runtime_error(Some(&b.span), "Invalid assignment target")),
            }
        } else if op == TokenType::And {
            let left = self.eval_expr(&b.left)?;
            if !left.truthy() {
                return Ok(Value::Bool(false));
            }
            let right = self.eval_expr(&b.right)?;
            Ok(Value::Bool(right.truthy()))
        } else if op == TokenType::Or {
            let left = self.eval_expr(&b.left)?;
            if left.truthy() {
                return Ok(Value::Bool(true));
            }
            let right = self.eval_expr(&b.right)?;
            Ok(Value::Bool(right.truthy()))
        } else {
            let left = self.eval_expr(&b.left)?;
            let right = self.eval_expr(&b.right)?;
            self.binary_op(op, left, right, b)
        }
    }

    fn binary_op(
        &self,
        op: TokenType,
        left: Value,
        right: Value,
        node: &Binary,
    ) -> Result<Value, ExceptionValue> {
        use TokenType::*;
        match op {
            Plus => self.add(left, right, &node.operator),
            Minus | Star | Slash | Percent => {
                self.check_number(&node.operator, &[&left, &right])?;
                self.num_binop(op, left, right, node)
            }
            EqualEqual => Ok(Value::Bool(left == right)),
            BangEqual => Ok(Value::Bool(left != right)),
            Greater | GreaterEqual | Less | LessEqual => {
                self.check_number(&node.operator, &[&left, &right])?;
                let ord = cmp_values(&left, &right);
                match ord {
                    None => Ok(Value::Bool(false)), // NaN comparisons are false
                    Some(o) => Ok(Value::Bool(match op {
                        Greater => o == Ordering::Greater,
                        GreaterEqual => o != Ordering::Less,
                        Less => o == Ordering::Less,
                        LessEqual => o != Ordering::Greater,
                        _ => unreachable!(),
                    })),
                }
            }
            _ => Err(self.runtime_error(
                Some(&node.span),
                &format!("Unknown operator '{}'", node.operator.lexeme),
            )),
        }
    }

    /// Python `_add`: string concat if either side is a string; list concat
    /// for two lists; numeric addition otherwise.
    fn add(&self, left: Value, right: Value, _op: &Token) -> Result<Value, ExceptionValue> {
        if let Value::Str(_) = &left {
            let s = format!("{}{}", left.python_str(), right.python_str());
            return Ok(Value::Str(Rc::from(s.as_str())));
        }
        if let Value::Str(_) = &right {
            let s = format!("{}{}", left.python_str(), right.python_str());
            return Ok(Value::Str(Rc::from(s.as_str())));
        }
        if let (Value::Int(a), Value::Int(b)) = (&left, &right) {
            return Ok(Value::Int(a + b));
        }
        if is_number(&left) && is_number(&right) {
            let a = to_f64(&left);
            let b = to_f64(&right);
            return Ok(Value::Float(a + b));
        }
        if let (Value::List(a), Value::List(b)) = (&left, &right) {
            let mut items = a.borrow().clone();
            items.extend(b.borrow().iter().cloned());
            return Ok(Value::List(Rc::new(RefCell::new(items))));
        }
        Err(ExceptionValue::new(
            "RuntimeError",
            format!("Cannot add {} and {}", left.type_name(), right.type_name()),
            None,
        ))
    }

    fn num_binop(
        &self,
        op: TokenType,
        left: Value,
        right: Value,
        node: &Binary,
    ) -> Result<Value, ExceptionValue> {
        let zero: Value = Value::Int(BigInt::from(0));
        let right_is_zero = match &right {
            Value::Int(i) => i.is_zero(),
            Value::Float(f) => *f == 0.0,
            _ => false,
        };
        match op {
            TokenType::Slash => {
                if right_is_zero {
                    return Err(self.runtime_error(Some(&node.span), "Division by zero"));
                }
                // Python int/int always yields float.
                let a = to_f64(&left);
                let b = to_f64(&right);
                Ok(Value::Float(a / b))
            }
            TokenType::Percent => {
                if right_is_zero {
                    return Err(self.runtime_error(Some(&node.span), "Modulo by zero"));
                }
                match (&left, &right) {
                    (Value::Int(a), Value::Int(b)) => Ok(Value::Int(py_mod(a, b))),
                    _ => {
                        let a = to_f64(&left);
                        let b = to_f64(&right);
                        Ok(Value::Float(a.rem_euclid(b)))
                    }
                }
            }
            TokenType::Minus | TokenType::Star => match (&left, &right) {
                (Value::Int(a), Value::Int(b)) => {
                    if op == TokenType::Minus {
                        Ok(Value::Int(a - b))
                    } else {
                        Ok(Value::Int(a * b))
                    }
                }
                _ => {
                    let a = to_f64(&left);
                    let b = to_f64(&right);
                    if op == TokenType::Minus {
                        Ok(Value::Float(a - b))
                    } else {
                        Ok(Value::Float(a * b))
                    }
                }
            },
            _ => {
                let _ = zero;
                Err(self.runtime_error(
                    Some(&node.span),
                    &format!("Unknown operator '{}'", node.operator.lexeme),
                ))
            }
        }
    }

    fn visit_unary(&mut self, u: &Unary) -> Result<Value, ExceptionValue> {
        let operand = self.eval_expr(&u.operand)?;
        match u.operator.kind {
            TokenType::Bang => Ok(Value::Bool(!operand.truthy())),
            TokenType::Minus => {
                self.check_number(&u.operator, &[&operand])?;
                match &operand {
                    Value::Int(i) => Ok(Value::Int(-i)),
                    Value::Float(f) => Ok(Value::Float(-f)),
                    Value::Bool(b) => Ok(Value::Int(if *b {
                        BigInt::from(-1)
                    } else {
                        BigInt::from(0)
                    })),
                    _ => unreachable!(),
                }
            }
            _ => Err(self.runtime_error(
                Some(&u.span),
                &format!("Unknown unary operator '{}'", u.operator.lexeme),
            )),
        }
    }

    /// `value |> fn` — desugars to `fn(value)`.
    fn visit_pipe(&mut self, p: &Pipe) -> Result<Value, ExceptionValue> {
        let value = self.eval_expr(&p.value)?;
        if let Expr::Variable(v) = p.function.as_ref() {
            if let Some(f) = self.functions.get(&v.name).cloned() {
                let parent_env = self.function_module_envs.get(&v.name).cloned();
                return self.call_function(&f, vec![value], parent_env, &p.span);
            }
            if let Some(a) = self.agents.get(&v.name).cloned() {
                let mut args = HashMap::new();
                if let Some(first) = a.params.first() {
                    args.insert(first.name.clone(), value);
                }
                return self.call_agent(&a, args, &p.span);
            }
        }
        let func = self.eval_expr(&p.function)?;
        match func {
            Value::Closure(c) => self.call_closure(&c, vec![value], &p.span),
            Value::BuiltinFn(b) => (b.func)(self, &[value]),
            Value::UserFn(f) => self.call_function(&f, vec![value], None, &p.span),
            Value::Agent(a) => {
                let mut args = HashMap::new();
                if let Some(first) = a.params.first() {
                    args.insert(first.name.clone(), value);
                }
                self.call_agent(&a, args, &p.span)
            }
            _ => {
                let name = match p.function.as_ref() {
                    Expr::Variable(v) => v.name.clone(),
                    other => format!("{:?}", std::mem::discriminant(other)),
                };
                Err(self.runtime_error(Some(&p.span), &format!("'{name}' is not callable")))
            }
        }
    }

    fn visit_call(&mut self, c: &Call) -> Result<Value, ExceptionValue> {
        let callee_name = match c.callee.as_ref() {
            Expr::Variable(v) => Some(v.name.clone()),
            _ => None,
        };

        // 1. Registered function
        if let Some(name) = &callee_name {
            if let Some(f) = self.functions.get(name).cloned() {
                let args = self.eval_args(&c.arguments)?;
                let parent_env = self.function_module_envs.get(name).cloned();
                return self.call_function(&f, args, parent_env, &c.span);
            }
        }
        // 2. Registered agent
        if let Some(name) = &callee_name {
            if let Some(a) = self.agents.get(name).cloned() {
                let mut agent_args = HashMap::new();
                for (i, arg) in c.arguments.iter().enumerate() {
                    let value = self.eval_expr(&arg.value)?;
                    if let Some(n) = &arg.name {
                        agent_args.insert(n.clone(), value);
                    } else if i < a.params.len() {
                        agent_args.insert(a.params[i].name.clone(), value);
                    } else {
                        return Err(self.runtime_error(
                            Some(&c.span),
                            &format!(
                                "too many positional arguments for agent '{name}' \
                                 (expected at most {})",
                                a.params.len()
                            ),
                        ));
                    }
                }
                return self.call_agent(&a, agent_args, &c.span);
            }
        }

        // 3. Evaluate callee and dispatch
        let callee = self.eval_expr(&c.callee)?;
        let args = self.eval_args(&c.arguments)?;
        match callee {
            Value::UserFn(f) => {
                let parent_env = self.function_module_envs.get(&f.name).cloned();
                self.call_function(&f, args, parent_env, &c.span)
            }
            Value::Closure(cl) => self.call_closure(&cl, args, &c.span),
            Value::BuiltinFn(b) => (b.func)(self, &args),
            Value::Agent(a) => {
                let mut agent_args = HashMap::new();
                for (i, arg) in args.into_iter().enumerate() {
                    if i < a.params.len() {
                        agent_args.insert(a.params[i].name.clone(), arg);
                    }
                }
                self.call_agent(&a, agent_args, &c.span)
            }
            Value::MapMethod(m) => m.call(&args),
            Value::ListMethod(m) => m.call(&args),
            other => Err(self.runtime_error(
                Some(&c.span),
                &format!("'{}' is not callable", other.type_name()),
            )),
        }
    }

    fn eval_args(&mut self, arguments: &[CallArg]) -> Result<Vec<Value>, ExceptionValue> {
        let mut out = Vec::with_capacity(arguments.len());
        for a in arguments {
            out.push(self.eval_expr(&a.value)?);
        }
        Ok(out)
    }

    /// Generic call dispatch for a value (used by stdlib higher-order
    /// functions like `map`/`filter`/`reduce`/`sort` that receive closures).
    pub fn call_value(&mut self, callee: Value, args: Vec<Value>) -> Result<Value, ExceptionValue> {
        match callee {
            Value::UserFn(f) => {
                let parent_env = self.function_module_envs.get(&f.name).cloned();
                self.call_function(&f, args, parent_env, &self.fake_span())
            }
            Value::Closure(cl) => self.call_closure(&cl, args, &self.fake_span()),
            Value::BuiltinFn(b) => (b.func)(self, &args),
            Value::Agent(a) => {
                let mut agent_args = HashMap::new();
                for (i, arg) in args.into_iter().enumerate() {
                    if i < a.params.len() {
                        agent_args.insert(a.params[i].name.clone(), arg);
                    }
                }
                self.call_agent(&a, agent_args, &self.fake_span())
            }
            Value::MapMethod(m) => m.call(&args),
            Value::ListMethod(m) => m.call(&args),
            other => {
                Err(self.runtime_error(None, &format!("'{}' is not callable", other.type_name())))
            }
        }
    }

    /// A zero-width span for synthetic calls (stdlib higher-order functions).
    fn fake_span(&self) -> SourceSpan {
        SourceSpan::new("", 0, 0, 0, 0)
    }

    fn visit_index(&mut self, i: &Index) -> Result<Value, ExceptionValue> {
        let target = self.eval_expr(&i.target)?;
        let index = self.eval_expr(&i.index)?;
        match &target {
            Value::List(l) => match &index {
                Value::Int(idx) => {
                    let items = l.borrow();
                    let n = items.len() as i64;
                    let mut real = idx.to_i64().unwrap_or(i64::MAX);
                    if real < 0 {
                        real += n;
                    }
                    if real < 0 || real >= n {
                        return Err(self.runtime_error(Some(&i.span), "list index out of range"));
                    }
                    Ok(items[real as usize].clone())
                }
                other => Err(self.runtime_error(
                    Some(&i.span),
                    &format!("List index must be integer, got {}", other.type_name()),
                )),
            },
            Value::Map(m) => {
                let map = m.borrow();
                if let Some(v) = map.get(&index) {
                    return Ok(v.clone());
                }
                // v1.11 detailed missing-key error
                let available: Vec<Value> = map.keys().cloned().collect();
                let keys_str = format_keys(&available);
                Err(self.runtime_error(
                    Some(&i.span),
                    &format!(
                        "Map key {} not found. Available keys: {}",
                        index.python_repr(),
                        keys_str
                    ),
                ))
            }
            Value::Str(s) => self.index_string(s, &index, &i.span),
            other => Err(self.runtime_error(
                Some(&i.span),
                &format!("Type {} does not support indexing", other.type_name()),
            )),
        }
    }

    /// Byte-based string indexing (D4). ASCII matches Python exactly.
    fn index_string(
        &self,
        s: &str,
        index: &Value,
        span: &SourceSpan,
    ) -> Result<Value, ExceptionValue> {
        let idx = match index {
            Value::Int(i) => i,
            other => {
                return Err(self.runtime_error(
                    Some(span),
                    &format!("String index must be integer, got {}", other.type_name()),
                ))
            }
        };
        let bytes = s.as_bytes();
        let n = bytes.len() as i64;
        let mut real = idx.to_i64().unwrap_or(i64::MAX);
        if real < 0 {
            real += n;
        }
        if real < 0 || real >= n {
            return Err(self.runtime_error(Some(span), "string index out of range"));
        }
        let pos = real as usize;
        // UTF-8 boundary validation (divergence from Python for non-ASCII)
        if !s.is_char_boundary(pos) {
            return Err(self.runtime_error(Some(span), "string index is not a character boundary"));
        }
        let end = pos + 1;
        let ch = s
            .get(pos..end)
            .ok_or_else(|| self.runtime_error(Some(span), "string index out of range"))?;
        Ok(Value::Str(Rc::from(ch)))
    }

    fn assign_index(
        &self,
        target: &Value,
        index: &Value,
        right: Value,
        span: &SourceSpan,
    ) -> Result<Value, ExceptionValue> {
        match target {
            Value::List(l) => {
                let idx = match index {
                    Value::Int(i) => i,
                    _ => {
                        return Err(self.runtime_error(
                            Some(span),
                            &format!("List index must be integer, got {}", index.type_name()),
                        ))
                    }
                };
                let mut items = l.borrow_mut();
                let n = items.len() as i64;
                let mut real = idx.to_i64().unwrap_or(i64::MAX);
                if real < 0 {
                    real += n;
                }
                if real < 0 || real >= n {
                    return Err(self.runtime_error(Some(span), "list index out of range"));
                }
                items[real as usize] = right.clone();
                Ok(right)
            }
            Value::Map(m) => {
                m.borrow_mut().insert(index.clone(), right.clone());
                Ok(right)
            }
            other => Err(self.runtime_error(
                Some(span),
                &format!("Type {} does not support indexing", other.type_name()),
            )),
        }
    }

    fn visit_access(&mut self, a: &Access) -> Result<Value, ExceptionValue> {
        // v1.42: static agent function access: AgentName.fn_name
        if let Expr::Variable(v) = a.target.as_ref() {
            if let Some(agent) = self.agents.get(&v.name) {
                if let Some(f) = agent.functions.iter().find(|f| f.name == a.property) {
                    let wrapper = Rc::new(f.clone());
                    return Ok(Value::UserFn(wrapper));
                }
                return Err(self.runtime_error(
                    Some(&a.span),
                    &format!(
                        "Agent '{}' has no function '{}' in its functions{{}} block",
                        agent.name, a.property
                    ),
                ));
            }
        }

        let target = self.eval_expr(&a.target)?;
        let prop = &a.property;
        match &target {
            Value::Map(m) => {
                let map = m.borrow();
                if (prop == "get" || prop == "keys" || prop == "values" || prop == "items")
                    && !map.contains_key(&Value::Str(Rc::from(prop.as_str())))
                {
                    // dict method access
                    return Ok(Value::MapMethod(Box::new(MapMethodValue {
                        kind: match prop.as_str() {
                            "get" => MapMethodKind::Get,
                            "keys" => MapMethodKind::Keys,
                            "values" => MapMethodKind::Values,
                            _ => MapMethodKind::Items,
                        },
                        map: m.clone(),
                    })));
                }
                if let Some(v) = map.get(&Value::Str(Rc::from(prop.as_str()))) {
                    return Ok(v.clone());
                }
                Err(self.runtime_error(Some(&a.span), &format!("Property '{prop}' not found")))
            }
            Value::Exception(ex) => {
                if prop == "message" {
                    return Ok(Value::Str(Rc::from(ex.message.as_str())));
                }
                if let Some(v) = ex.fields.get(prop) {
                    return Ok(v.clone());
                }
                Err(self.runtime_error(
                    Some(&a.span),
                    &format!("'{}' has no property '{}'", ex.class_name, prop),
                ))
            }
            Value::List(l) => {
                // Native list methods (Python `getattr(list, prop)`): append,
                // insert, pop, remove, count, index, clear, extend, reverse.
                let kind = match prop.as_str() {
                    "append" => Some(ListMethodKind::Append),
                    "insert" => Some(ListMethodKind::Insert),
                    "pop" => Some(ListMethodKind::Pop),
                    "remove" => Some(ListMethodKind::Remove),
                    "count" => Some(ListMethodKind::Count),
                    "index" => Some(ListMethodKind::Index),
                    "clear" => Some(ListMethodKind::Clear),
                    "extend" => Some(ListMethodKind::Extend),
                    "reverse" => Some(ListMethodKind::Reverse),
                    _ => None,
                };
                if let Some(kind) = kind {
                    return Ok(Value::ListMethod(Box::new(ListMethodValue {
                        kind,
                        list: l.clone(),
                    })));
                }
                Err(self.runtime_error(
                    Some(&a.span),
                    &format!("'list' has no property '{}'", a.property),
                ))
            }
            other => Err(self.runtime_error(
                Some(&a.span),
                &format!("'{}' has no property '{}'", other.type_name(), a.property),
            )),
        }
    }

    fn assign_access(
        &self,
        target: &Value,
        prop: &str,
        right: Value,
        span: &SourceSpan,
    ) -> Result<Value, ExceptionValue> {
        if let Value::Map(m) = target {
            m.borrow_mut()
                .insert(Value::Str(Rc::from(prop)), right.clone());
            return Ok(right);
        }
        Err(self.runtime_error(
            Some(span),
            &format!(
                "Type {} does not support attribute assignment",
                target.type_name()
            ),
        ))
    }

    fn visit_lambda(&mut self, l: &Lambda) -> Result<Value, ExceptionValue> {
        let free_vars = compute_free_variables(l);
        let mut captured_env = Environment::new(None);
        for var in &free_vars {
            if let Some(value) = self.environment.borrow().get(var) {
                let value = if is_mutable_type(&value) {
                    value.clone_deep()
                } else {
                    value
                };
                captured_env.define(var, value, false);
            }
        }
        let closure = Closure::new(Rc::new(l.clone()), Rc::new(RefCell::new(captured_env)));
        Ok(Value::Closure(Rc::new(closure)))
    }

    // ------------------------------------------------------------------
    // Try / throw / assert
    // ------------------------------------------------------------------

    fn visit_try(&mut self, t: &helen_core::ast::TryStmt) -> Result<Flow, ExceptionValue> {
        let mut caught = false;
        let mut exc_to_rethrow: Option<ExceptionValue> = None;
        let mut result = Flow::Normal(None);

        let try_result = self.with_scope(None, |s| s.execute_stmts(&t.body));
        match try_result {
            Ok(flow) => result = flow,
            Err(exc) => {
                caught = true;
                exc_to_rethrow = Some(exc.clone());

                for clause in &t.catch_clauses {
                    let type_name = &clause.error_type.name;
                    if error_matches(&exc, type_name) {
                        let catch_result = self.with_scope(None, |s| {
                            s.environment.borrow_mut().define(
                                &clause.error_name,
                                Value::Exception(Box::new(exc.clone())),
                                false,
                            );
                            s.execute_stmts(&clause.body)
                        });
                        match catch_result {
                            Ok(Flow::Return(_)) => return catch_result,
                            Ok(flow) => {
                                result = flow;
                            }
                            Err(_) => return catch_result,
                        }
                        caught = false;
                        break;
                    }
                }

                if caught {
                    if let Some(ca) = &t.catch_all {
                        let catch_result = self.with_scope(None, |s| s.execute_stmts(&ca.body));
                        match catch_result {
                            Ok(Flow::Return(_)) => return catch_result,
                            Ok(flow) => {
                                result = flow;
                            }
                            Err(_) => return catch_result,
                        }
                        caught = false;
                    }
                }
            }
        }

        // finally
        if let Some(fb) = &t.finally_block {
            let finally_result = self.with_scope(None, |s| s.execute_stmts(&fb.body));
            if let Err(e) = &finally_result {
                return Err(e.clone()); // thrown-from-finally replaces prior error
            }
        }

        if caught {
            if let Some(exc) = exc_to_rethrow {
                return Err(exc);
            }
        }
        Ok(result)
    }

    /// `throw ExceptionType` or `throw ExceptionType(message)`.
    fn make_throw(&mut self, t: &ThrowStmt) -> Result<ExceptionValue, ExceptionValue> {
        let type_name = &t.exception_type.name;
        let class_name = match resolve_exception(type_name) {
            Some(n) => n.to_string(),
            None => {
                return Err(self.runtime_error(
                    Some(&t.span),
                    &format!("'{type_name}' is not a valid exception type"),
                ))
            }
        };

        let message_value = if let Some(m) = &t.message {
            let v = self.eval_expr(m)?;
            match v {
                Value::Str(s) => s.to_string(),
                other => other.python_str(),
            }
        } else {
            ExceptionValue::default_message(&class_name)
        };

        Ok(self.build_exception(&class_name, message_value, t.span.clone()))
    }

    /// Port of `exc_class(message, node.span)` — the positional-argument
    /// quirks per class (AgentError receives the message as agent_name).
    fn build_exception(
        &self,
        class_name: &str,
        message: String,
        span: SourceSpan,
    ) -> ExceptionValue {
        match class_name {
            "AgentError" => ExceptionValue {
                class_name: class_name.to_string(),
                message: format!("Agent '{message}' failed"),
                span: None,
                fields: indexmap::IndexMap::new(),
            },
            "LLMOutputContractError" => ExceptionValue {
                class_name: class_name.to_string(),
                message: format!("Agent '{message}' output does not match contract: "),
                span: None,
                fields: indexmap::IndexMap::new(),
            },
            _ => ExceptionValue::new(class_name, message, Some(span)),
        }
    }

    fn visit_assert(&mut self, a: &helen_core::ast::AssertStmt) -> Result<(), ExceptionValue> {
        let condition = self.eval_expr(&a.condition)?;
        if condition.truthy() {
            return Ok(());
        }
        let message = if let Some(m) = &a.message {
            let v = self.eval_expr(m)?;
            match v {
                Value::Str(s) => s.to_string(),
                other => other.python_str(),
            }
        } else {
            "Assertion failed".to_string()
        };
        Err(ExceptionValue::new(
            "AssertionError",
            message,
            Some(a.span.clone()),
        ))
    }

    // ------------------------------------------------------------------
    // Match / patterns
    // ------------------------------------------------------------------

    fn match_case(
        &mut self,
        subject: &Value,
        pattern: &Expr,
        guard: &Option<Box<Expr>>,
    ) -> Result<(bool, Vec<(String, Value)>), ExceptionValue> {
        let mut matched = false;
        let mut bindings: Vec<(String, Value)> = Vec::new();
        match pattern {
            Expr::WildcardPattern(_) => matched = true,
            Expr::VariablePattern(vp) => {
                matched = true;
                bindings.push((vp.name.clone(), subject.clone()));
            }
            Expr::TypePattern(tp) => {
                matched = check_type(subject, &tp.type_name);
                if matched {
                    if let Some(bn) = &tp.binding_name {
                        bindings.push((bn.clone(), subject.clone()));
                    }
                }
            }
            Expr::RangePattern(r) => {
                let start = self.eval_expr(&r.start)?;
                let end = self.eval_expr(&r.end)?;
                if let (Some(a), Some(b), Some(c)) =
                    (num_f64(subject), num_f64(&start), num_f64(&end))
                {
                    matched = a >= b && a <= c;
                }
            }
            other => {
                let pattern_value = self.eval_expr(other)?;
                matched = subject == &pattern_value;
            }
        }

        if matched {
            if let Some(g) = guard {
                let guard_ok = self.with_scope(None, |s| {
                    for (name, value) in &bindings {
                        s.environment
                            .borrow_mut()
                            .define(name, value.clone(), false);
                    }
                    s.eval_expr(g)
                })?;
                matched = guard_ok.truthy();
            }
        }
        Ok((matched, bindings))
    }

    fn visit_match_stmt(
        &mut self,
        m: &helen_core::ast::MatchStmtNode,
    ) -> Result<Flow, ExceptionValue> {
        let subject = self.eval_expr(&m.subject)?;
        for case in &m.cases {
            let (matched, bindings) = self.match_case(&subject, &case.pattern, &case.guard)?;
            if matched {
                let flow = self.with_scope(None, |s| {
                    for (name, value) in &bindings {
                        s.environment
                            .borrow_mut()
                            .define(name, value.clone(), false);
                    }
                    s.execute_stmts(&case.body)
                })?;
                return Ok(flow);
            }
        }
        if !m.default.is_empty() {
            let flow = self.with_scope(None, |s| s.execute_stmts(&m.default))?;
            return Ok(flow);
        }
        Ok(Flow::Normal(None))
    }

    fn visit_match_expr(
        &mut self,
        m: &helen_core::ast::MatchExprNode,
    ) -> Result<Value, ExceptionValue> {
        let subject = self.eval_expr(&m.subject)?;
        for case in &m.cases {
            let (matched, bindings) = self.match_case(&subject, &case.pattern, &case.guard)?;
            if matched {
                return self.with_scope(None, |s| {
                    for (name, value) in &bindings {
                        s.environment
                            .borrow_mut()
                            .define(name, value.clone(), false);
                    }
                    match case.body.first() {
                        Some(Stmt::Expr(e)) => s.eval_expr(&e.expression),
                        _ => {
                            let _ = s.execute_stmts(&case.body)?;
                            Ok(Value::Null)
                        }
                    }
                });
            }
        }
        if let Some(d) = &m.default_body {
            return self.eval_expr(d);
        }
        Ok(Value::Null)
    }

    // ------------------------------------------------------------------
    // Calls
    // ------------------------------------------------------------------

    fn call_function(
        &mut self,
        func: &FunctionDecl,
        args: Vec<Value>,
        parent_env: Option<Rc<RefCell<Environment>>>,
        _span: &SourceSpan,
    ) -> Result<Value, ExceptionValue> {
        // Runtime parameter type checking
        for (i, param) in func.params.iter().enumerate() {
            if i < args.len() {
                if let Some(tn) = &param.type_annotation {
                    let expected = type_from_typenode(tn);
                    let actual = type_of_value(&args[i]);
                    if !matches!(actual, Type::Any) && !type_compatible(&actual, &expected) {
                        return Err(ExceptionValue::new(
                            "RuntimeError",
                            format!(
                                "argument {} type '{}' is not compatible with parameter type '{}'",
                                i + 1,
                                actual.name(),
                                expected.name()
                            ),
                            None,
                        ));
                    }
                }
            }
        }

        let call_env = match &parent_env {
            Some(pe) => Rc::new(RefCell::new(Environment::new(Some(pe.clone())))),
            None => Environment::child(self.environment.clone()),
        };

        // Bind parameters
        {
            let mut env = call_env.borrow_mut();
            for (i, param) in func.params.iter().enumerate() {
                if i < args.len() {
                    env.define(&param.name, args[i].clone(), false);
                } else if let Some(dv) = &param.default_value {
                    let default_val = if parent_env.is_some() {
                        // evaluate default in module env
                        self.with_scope(parent_env.clone(), |s| s.eval_expr(dv))?
                    } else {
                        self.eval_expr(dv)?
                    };
                    env.define(&param.name, default_val, false);
                } else {
                    env.define(&param.name, Value::Null, false);
                }
            }
        }

        let flow = self.with_scope(Some(call_env), |s| s.execute_stmts(&func.body.body))?;
        match flow {
            Flow::Return(v) => Ok(v.unwrap_or(Value::Null)),
            Flow::Normal(v) => Ok(v.unwrap_or(Value::Null)),
            _ => Ok(Value::Null),
        }
    }

    fn call_closure(
        &mut self,
        closure: &Closure,
        args: Vec<Value>,
        _span: &SourceSpan,
    ) -> Result<Value, ExceptionValue> {
        let lambda = closure.lambda.clone();

        // Runtime parameter type checking (same as call_function)
        for (i, param) in lambda.params.iter().enumerate() {
            if i < args.len() {
                if let Some(tn) = &param.type_annotation {
                    let expected = type_from_typenode(tn);
                    let actual = type_of_value(&args[i]);
                    if !matches!(actual, Type::Any) && !type_compatible(&actual, &expected) {
                        return Err(ExceptionValue::new(
                            "RuntimeError",
                            format!(
                                "argument {} type '{}' is not compatible with parameter type '{}'",
                                i + 1,
                                actual.name(),
                                expected.name()
                            ),
                            None,
                        ));
                    }
                }
            }
        }

        // New scope with the CAPTURED environment as parent (closures!)
        let call_env = Environment::child(closure.captured_env.clone());

        // v1.18 recursive closure support
        if let Some(self_name) = &closure.self_name {
            call_env.borrow_mut().define(
                self_name,
                Value::Closure(Rc::new(closure.clone())),
                false,
            );
        }

        // Bind parameters
        {
            let mut env = call_env.borrow_mut();
            for (i, param) in lambda.params.iter().enumerate() {
                if i < args.len() {
                    env.define(&param.name, args[i].clone(), false);
                } else if let Some(dv) = &param.default_value {
                    let default_val = self.eval_expr(dv)?;
                    env.define(&param.name, default_val, false);
                } else {
                    env.define(&param.name, Value::Null, false);
                }
            }
        }

        let flow = self.with_scope(Some(call_env), |s| s.execute_stmts(&lambda.body.body))?;
        match flow {
            Flow::Return(v) => Ok(v.unwrap_or(Value::Null)),
            Flow::Normal(v) => Ok(v.unwrap_or(Value::Null)),
            _ => Ok(Value::Null),
        }
    }

    /// Minimal agent call for M3: fresh isolated environment, bind params,
    /// execute the agent's main block (if any). LLM interaction lands in 3.6.
    fn call_agent(
        &mut self,
        agent: &AgentDecl,
        args: HashMap<String, Value>,
        _span: &SourceSpan,
    ) -> Result<Value, ExceptionValue> {
        let prev_agent = self.current_agent.clone();
        self.current_agent = Some(Rc::new(agent.clone()));

        let call_env = Rc::new(RefCell::new(Environment::new(None)));
        {
            let mut env = call_env.borrow_mut();
            for p in &agent.params {
                let value = args.get(&p.name).cloned().unwrap_or(Value::Null);
                env.define(&p.name, value, false);
            }
        }

        let result = self.with_scope(Some(call_env), |s| {
            if let Some(logic) = &agent.logic {
                match logic.as_ref() {
                    Stmt::MainBlock(mb) => s.execute_stmts(&mb.body),
                    other => s.execute_stmt(other),
                }
            } else {
                // Agent without explicit logic: stub LLM response for M3.
                let prompt = agent
                    .prompt
                    .as_ref()
                    .map(|p| p.content.clone())
                    .unwrap_or_default();
                Ok(Flow::Normal(Some(Value::Str(Rc::from(prompt.as_str())))))
            }
        });

        self.current_agent = prev_agent;
        match result? {
            Flow::Return(v) => Ok(v.unwrap_or(Value::Null)),
            Flow::Normal(v) => Ok(v.unwrap_or(Value::Null)),
            _ => Ok(Value::Null),
        }
    }

    // ------------------------------------------------------------------
    // Alias / LLM stubs
    // ------------------------------------------------------------------

    fn visit_alias(&mut self, a: &helen_core::ast::AliasStmt) {
        if let Some(v) = self.environment.borrow().get(&a.canonical) {
            self.environment
                .borrow_mut()
                .define(&a.alias_name, v, false);
        }
    }

    /// `llm if` stub (Task 3.6): for M3, route to the first branch whose
    /// condition is truthy, else default (last branch with no condition).
    fn visit_llm_if(
        &mut self,
        li: &helen_core::ast::LlmIfStmtNode,
    ) -> Result<Flow, ExceptionValue> {
        for branch in &li.branches {
            if let Some(c) = &branch.condition {
                let v = self.eval_expr(c)?;
                if v.truthy() {
                    return self.execute_stmts(&branch.body);
                }
            } else {
                return self.execute_stmts(&branch.body);
            }
        }
        Ok(Flow::Normal(None))
    }

    /// `llm act` stub (Task 3.6): returns the prompt string for M3.
    fn visit_llm_act(&mut self, la: &helen_core::ast::LlmAct) -> Result<Value, ExceptionValue> {
        if let Some(p) = &la.prompt {
            let v = self.eval_expr(p)?;
            match v {
                Value::Str(s) => Ok(Value::Str(s)),
                other => Ok(Value::Str(Rc::from(other.python_str().as_str()))),
            }
        } else {
            Ok(Value::Str(Rc::from("")))
        }
    }

    // ------------------------------------------------------------------
    // Numeric helpers
    // ------------------------------------------------------------------

    fn runtime_error(&self, span: Option<&SourceSpan>, message: &str) -> ExceptionValue {
        ExceptionValue::new("RuntimeError", message.to_string(), span.cloned())
    }

    fn check_number(&self, op: &Token, values: &[&Value]) -> Result<(), ExceptionValue> {
        for v in values {
            if !is_number(v) {
                return Err(ExceptionValue::new(
                    "RuntimeError",
                    format!(
                        "Operator '{}' requires numbers, got {}",
                        op.lexeme,
                        v.type_name()
                    ),
                    Some(op.span()),
                ));
            }
        }
        Ok(())
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Interpreter::new()
    }
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

/// `_truthy` (static).
fn is_number(v: &Value) -> bool {
    matches!(v, Value::Int(_) | Value::Float(_) | Value::Bool(_))
}

/// Convert a numeric value to f64 (bool counts as int).
fn to_f64(v: &Value) -> f64 {
    match v {
        Value::Int(i) => i.to_f64().unwrap_or(0.0),
        Value::Float(f) => *f,
        Value::Bool(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        _ => f64::NAN,
    }
}

fn num_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Int(i) => i.to_f64(),
        Value::Float(f) => Some(*f),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

/// Python `%` on integers: result takes the sign of the divisor.
fn py_mod(a: &BigInt, b: &BigInt) -> BigInt {
    let r = a % b;
    if r.is_zero() {
        return r;
    }
    let same_sign = r.sign() == b.sign();
    if same_sign {
        r
    } else {
        r + b
    }
}

/// Cross-type numeric comparison (None if NaN involved).
pub fn cmp_values(a: &Value, b: &Value) -> Option<Ordering> {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => Some(x.cmp(y)),
        (Value::Float(x), Value::Float(y)) => x.partial_cmp(y),
        (Value::Int(x), Value::Float(y)) => cmp_int_float(x, *y),
        (Value::Float(x), Value::Int(y)) => cmp_int_float(y, *x).map(Ordering::reverse),
        (Value::Bool(x), Value::Int(y)) => {
            Some(if *x { BigInt::from(1) } else { BigInt::from(0) }.cmp(y))
        }
        (Value::Int(x), Value::Bool(y)) => {
            Some(x.cmp(&if *y { BigInt::from(1) } else { BigInt::from(0) }))
        }
        (Value::Bool(x), Value::Float(y)) => {
            let t = if *x { 1.0 } else { 0.0 };
            t.partial_cmp(y)
        }
        (Value::Float(x), Value::Bool(y)) => {
            let t = if *y { 1.0 } else { 0.0 };
            x.partial_cmp(&t)
        }
        _ => None,
    }
}

/// Exact int-vs-float ordering via mantissa/exponent decomposition.
fn cmp_int_float(i: &BigInt, f: f64) -> Option<Ordering> {
    if f.is_nan() {
        return None;
    }
    if f.is_infinite() {
        return Some(if f > 0.0 {
            Ordering::Less
        } else {
            Ordering::Greater
        });
    }
    if f == 0.0 {
        return Some(i.cmp(&BigInt::zero()));
    }
    let neg = f.is_sign_negative();
    let av = f.abs();
    let bits = av.to_bits();
    let exp_bits = ((bits >> 52) & 0x7ff) as i64;
    let frac = bits & 0x000f_ffff_ffff_ffff;
    let (mant, exp): (u64, i64) = if exp_bits == 0 {
        (frac, -1074)
    } else {
        ((1u64 << 52) | frac, exp_bits - 1023 - 52)
    };
    if exp >= 0 {
        let mut scaled = BigInt::from(mant) << exp;
        if neg {
            scaled = -scaled;
        }
        return Some(i.cmp(&scaled));
    }
    let sh = -exp;
    let lhs = i << sh;
    let rhs = if neg {
        -BigInt::from(mant)
    } else {
        BigInt::from(mant)
    };
    Some(lhs.cmp(&rhs))
}

/// v1.11 missing-key message: `str(available_keys[:10])` with "..." suffix.
fn format_keys(keys: &[Value]) -> String {
    let shown: Vec<Value> = keys.iter().take(10).cloned().collect();
    let mut s = Value::List(Rc::new(RefCell::new(shown))).python_str();
    if keys.len() > 10 {
        // Python: str(list[:10])[:-1] + ", ...]"
        s = s.trim_end_matches(']').to_string();
        s.push_str(", ...]");
    }
    s
}

/// Python `_TYPE_NAME_MAP` for `case is Type` patterns.
fn check_type(value: &Value, type_name: &str) -> bool {
    match type_name {
        "Int" => matches!(value, Value::Int(_) | Value::Bool(_)), // bool is int
        "Float" => matches!(value, Value::Float(_) | Value::Int(_) | Value::Bool(_)),
        "String" => matches!(value, Value::Str(_)),
        "Bool" => matches!(value, Value::Bool(_)),
        "List" => matches!(value, Value::List(_)),
        "Map" => matches!(value, Value::Map(_)),
        "Null" => matches!(value, Value::Null),
        _ => false,
    }
}

/// Port of `type_from_typenode` for simple annotations (runtime type check).
fn type_from_typenode(tn: &TypeRef) -> Type {
    match &tn.kind {
        helen_core::ast::TypeRefKind::Simple => match tn.name.as_str() {
            "int" => Type::Int,
            "float" => Type::Float,
            "str" | "string" => Type::Str,
            "bool" => Type::Bool,
            "null" | "NoneType" => Type::Null,
            "any" | "anytype" => Type::Any,
            _ => Type::Any, // user/agent types: dynamic at runtime
        },
        helen_core::ast::TypeRefKind::Optional(inner) => {
            Type::Optional(Box::new(type_from_typenode(inner)))
        }
        helen_core::ast::TypeRefKind::Union(members) => {
            Type::Union(members.iter().map(type_from_typenode).collect())
        }
    }
}

/// Python `_is_mutable_type` — reference types get deep-copied on capture.
fn is_mutable_type(v: &Value) -> bool {
    matches!(v, Value::List(_) | Value::Map(_))
}

// ---------------------------------------------------------------------------
// Map method values (dict.get / keys / values / items)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum MapMethodKind {
    Get,
    Keys,
    Values,
    Items,
}

#[derive(Clone, Debug)]
pub struct MapMethodValue {
    pub kind: MapMethodKind,
    pub map: Rc<RefCell<indexmap::IndexMap<Value, Value>>>,
}

impl MapMethodValue {
    pub fn call(&self, args: &[Value]) -> Result<Value, ExceptionValue> {
        match self.kind {
            MapMethodKind::Get => {
                let key = args.first().cloned().unwrap_or(Value::Null);
                let default = args.get(1).cloned().unwrap_or(Value::Null);
                let map = self.map.borrow();
                Ok(map.get(&key).cloned().unwrap_or(default))
            }
            MapMethodKind::Keys => {
                let map = self.map.borrow();
                let keys: Vec<Value> = map.keys().cloned().collect();
                Ok(Value::List(Rc::new(RefCell::new(keys))))
            }
            MapMethodKind::Values => {
                let map = self.map.borrow();
                let values: Vec<Value> = map.values().cloned().collect();
                Ok(Value::List(Rc::new(RefCell::new(values))))
            }
            MapMethodKind::Items => {
                let map = self.map.borrow();
                let items: Vec<Value> = map
                    .iter()
                    .map(|(k, v)| Value::List(Rc::new(RefCell::new(vec![k.clone(), v.clone()]))))
                    .collect();
                Ok(Value::List(Rc::new(RefCell::new(items))))
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ListMethodKind {
    Append,
    Insert,
    Pop,
    Remove,
    Count,
    Index,
    Clear,
    Extend,
    Reverse,
}

#[derive(Clone, Debug)]
pub struct ListMethodValue {
    pub kind: ListMethodKind,
    pub list: Rc<RefCell<Vec<Value>>>,
}

impl ListMethodValue {
    pub fn call(&self, args: &[Value]) -> Result<Value, ExceptionValue> {
        let mut list = self.list.borrow_mut();
        match self.kind {
            ListMethodKind::Append => {
                if let Some(v) = args.first() {
                    list.push(v.clone());
                }
                Ok(Value::Null)
            }
            ListMethodKind::Insert => {
                let idx = args
                    .first()
                    .and_then(|v| v.as_bigint())
                    .and_then(|b| b.to_i64())
                    .unwrap_or(0);
                let val = args.get(1).cloned().unwrap_or(Value::Null);
                let n = list.len() as i64;
                let mut real = idx;
                if real < 0 {
                    real += n;
                }
                let pos = real.clamp(0, n) as usize;
                list.insert(pos, val);
                Ok(Value::Null)
            }
            ListMethodKind::Pop => {
                if list.is_empty() {
                    return Err(ExceptionValue::new(
                        "RuntimeError",
                        "pop from empty list".into(),
                        None,
                    ));
                }
                Ok(list.pop().unwrap_or(Value::Null))
            }
            ListMethodKind::Remove => {
                let val = args.first().cloned().unwrap_or(Value::Null);
                if let Some(pos) = list.iter().position(|v| *v == val) {
                    list.remove(pos);
                    Ok(Value::Null)
                } else {
                    Err(ExceptionValue::new(
                        "RuntimeError",
                        format!("{} not in list", val.python_repr()),
                        None,
                    ))
                }
            }
            ListMethodKind::Count => {
                let val = args.first().cloned().unwrap_or(Value::Null);
                let n = list.iter().filter(|v| **v == val).count() as i64;
                Ok(Value::Int(BigInt::from(n)))
            }
            ListMethodKind::Index => {
                let val = args.first().cloned().unwrap_or(Value::Null);
                if let Some(pos) = list.iter().position(|v| *v == val) {
                    Ok(Value::Int(BigInt::from(pos as i64)))
                } else {
                    Err(ExceptionValue::new(
                        "RuntimeError",
                        format!("{} is not in list", val.python_repr()),
                        None,
                    ))
                }
            }
            ListMethodKind::Clear => {
                list.clear();
                Ok(Value::Null)
            }
            ListMethodKind::Extend => {
                if let Some(Value::List(other)) = args.first() {
                    let items = other.borrow().clone();
                    list.extend(items);
                }
                Ok(Value::Null)
            }
            ListMethodKind::Reverse => {
                list.reverse();
                Ok(Value::Null)
            }
        }
    }
}

pub type BuiltinImpl = fn(&mut Interpreter, &[Value]) -> Result<Value, ExceptionValue>;

// ---------------------------------------------------------------------------
// Core builtins (stdlib subset; M4 registers the full set)
// ---------------------------------------------------------------------------

fn builtin_print(interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let parts: Vec<String> = args.iter().map(|a| a.to_display(true)).collect();
    let result = parts.join(" ");
    interp.stdout.borrow_mut().push_str(&result);
    interp.stdout.borrow_mut().push('\n');
    Ok(Value::Str(Rc::from(result.as_str())))
}

fn builtin_len(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let v = args.first().cloned().unwrap_or(Value::Null);
    let n: i64 = match &v {
        Value::Str(s) => s.len() as i64, // byte length (D4 divergence)
        Value::List(l) => l.borrow().len() as i64,
        Value::Map(m) => m.borrow().len() as i64,
        other => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("object of type '{}' has no len()", other.type_name()),
                None,
            ))
        }
    };
    Ok(Value::Int(BigInt::from(n)))
}

fn builtin_str(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let v = args.first().cloned().unwrap_or(Value::Null);
    Ok(Value::Str(Rc::from(v.python_str().as_str())))
}

fn builtin_int(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let v = args.first().cloned().unwrap_or(Value::Null);
    match v {
        Value::Int(i) => Ok(Value::Int(i)),
        Value::Bool(b) => Ok(Value::Int(if b { BigInt::from(1) } else { BigInt::from(0) })),
        Value::Float(f) => {
            if f.is_nan() || f.is_infinite() {
                return Err(ExceptionValue::new(
                    "RuntimeError",
                    format!("Python ValueError: cannot convert float {f} to integer"),
                    None,
                ));
            }
            Ok(Value::Int(BigInt::from(f.trunc() as i64)))
        }
        Value::Str(s) => match s.trim().parse::<i128>() {
            Ok(n) => Ok(Value::Int(BigInt::from(n))),
            Err(_) => Err(ExceptionValue::new(
                "RuntimeError",
                format!("Python ValueError: invalid literal for int() with base 10: '{s}'"),
                None,
            )),
        },
        other => Err(ExceptionValue::new(
            "RuntimeError",
            format!(
                "Python TypeError: int() argument must be a string, a bytes-like object or a real number, not '{}'",
                other.type_name()
            ),
            None,
        )),
    }
}

fn builtin_float(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let v = args.first().cloned().unwrap_or(Value::Null);
    match v {
        Value::Float(f) => Ok(Value::Float(f)),
        Value::Int(i) => Ok(Value::Float(i.to_f64().unwrap_or(0.0))),
        Value::Bool(b) => Ok(Value::Float(if b { 1.0 } else { 0.0 })),
        Value::Str(s) => match s.trim().parse::<f64>() {
            Ok(f) => Ok(Value::Float(f)),
            Err(_) => Err(ExceptionValue::new(
                "RuntimeError",
                format!("Python ValueError: could not convert string to float: '{s}'"),
                None,
            )),
        },
        other => Err(ExceptionValue::new(
            "RuntimeError",
            format!(
                "Python TypeError: float() argument must be a string or a real number, not '{}'",
                other.type_name()
            ),
            None,
        )),
    }
}

fn builtin_bool(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let v = args.first().cloned().unwrap_or(Value::Null);
    Ok(Value::Bool(v.truthy()))
}

fn builtin_type(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let v = args.first().cloned().unwrap_or(Value::Null);
    Ok(Value::Str(Rc::from(v.type_name().as_str())))
}

fn builtin_isinstance(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let v = args.first().cloned().unwrap_or(Value::Null);
    let type_name = match args.get(1) {
        Some(Value::Str(s)) => s.to_string(),
        _ => return Ok(Value::Bool(false)),
    };
    let ok = match type_name.as_str() {
        "int" => matches!(v, Value::Int(_) | Value::Bool(_)),
        "float" => matches!(v, Value::Float(_) | Value::Int(_) | Value::Bool(_)),
        "str" => matches!(v, Value::Str(_)),
        "bool" => matches!(v, Value::Bool(_)),
        "list" => matches!(v, Value::List(_)),
        "dict" => matches!(v, Value::Map(_)),
        "NoneType" => matches!(v, Value::Null),
        _ => false,
    };
    Ok(Value::Bool(ok))
}

fn builtin_range(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let get = |i: usize| -> Option<BigInt> {
        args.get(i).and_then(|v| match v {
            Value::Int(n) => Some(n.clone()),
            _ => None,
        })
    };
    let (start, stop, step) = match args.len() {
        0 => return Ok(Value::List(Rc::new(RefCell::new(vec![])))),
        1 => (BigInt::from(0), get(0).unwrap_or_default(), BigInt::from(1)),
        2 => (
            get(0).unwrap_or_default(),
            get(1).unwrap_or_default(),
            BigInt::from(1),
        ),
        _ => (
            get(0).unwrap_or_default(),
            get(1).unwrap_or_default(),
            get(2).unwrap_or_default(),
        ),
    };
    if step.is_zero() {
        return Err(ExceptionValue::new(
            "RuntimeError",
            "Python ValueError: range() arg 3 must not be zero".into(),
            None,
        ));
    }
    let mut out = Vec::new();
    if step > 0u32.into() {
        let mut cur = start.clone();
        while cur < stop {
            out.push(Value::Int(cur.clone()));
            cur += &step;
        }
    } else {
        let mut cur = start.clone();
        while cur > stop {
            out.push(Value::Int(cur.clone()));
            cur += &step;
        }
    }
    Ok(Value::List(Rc::new(RefCell::new(out))))
}

fn builtin_abs(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let v = args.first().cloned().unwrap_or(Value::Null);
    match v {
        Value::Int(i) => Ok(Value::Int(i.abs())),
        Value::Float(f) => Ok(Value::Float(f.abs())),
        Value::Bool(b) => Ok(Value::Int(if b {
            BigInt::from(1)
        } else {
            BigInt::from(0)
        })),
        other => Err(ExceptionValue::new(
            "RuntimeError",
            format!("bad operand type for abs(): '{}'", other.type_name()),
            None,
        )),
    }
}

fn builtin_min(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let values: Vec<Value> = if args.len() == 1 {
        match &args[0] {
            Value::List(l) => l.borrow().clone(),
            _ => args.to_vec(),
        }
    } else {
        args.to_vec()
    };
    values
        .into_iter()
        .min_by(|a, b| cmp_values(a, b).unwrap_or(Ordering::Equal))
        .ok_or_else(|| {
            ExceptionValue::new(
                "RuntimeError",
                "min() arg is an empty sequence".into(),
                None,
            )
        })
}

fn builtin_max(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let values: Vec<Value> = if args.len() == 1 {
        match &args[0] {
            Value::List(l) => l.borrow().clone(),
            _ => args.to_vec(),
        }
    } else {
        args.to_vec()
    };
    values
        .into_iter()
        .max_by(|a, b| cmp_values(a, b).unwrap_or(Ordering::Equal))
        .ok_or_else(|| {
            ExceptionValue::new(
                "RuntimeError",
                "max() arg is an empty sequence".into(),
                None,
            )
        })
}

fn builtin_list(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    if args.is_empty() {
        return Ok(Value::List(Rc::new(RefCell::new(vec![]))));
    }
    let v = &args[0];
    match v {
        Value::List(l) => Ok(Value::List(Rc::new(RefCell::new(l.borrow().clone())))),
        Value::Str(s) => {
            // Python list("abc") -> ['a','b','c'] (codepoints; byte divergence for non-ASCII)
            let chars: Vec<Value> = s
                .chars()
                .map(|c| Value::Str(Rc::from(c.to_string().as_str())))
                .collect();
            Ok(Value::List(Rc::new(RefCell::new(chars))))
        }
        Value::Map(m) => {
            // list(dict) -> keys
            let keys: Vec<Value> = m.borrow().keys().cloned().collect();
            Ok(Value::List(Rc::new(RefCell::new(keys))))
        }
        other => Err(ExceptionValue::new(
            "RuntimeError",
            format!("'{}' object is not iterable", other.type_name()),
            None,
        )),
    }
}

fn builtin_dict(_interp: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    if args.is_empty() {
        return Ok(Value::Map(Rc::new(RefCell::new(indexmap::IndexMap::new()))));
    }
    let v = &args[0];
    match v {
        Value::Map(m) => Ok(Value::Map(Rc::new(RefCell::new(m.borrow().clone())))),
        Value::List(l) => {
            let mut map = indexmap::IndexMap::new();
            for item in l.borrow().iter() {
                if let Value::List(pair) = item {
                    let b = pair.borrow();
                    if b.len() == 2 {
                        map.insert(b[0].clone(), b[1].clone());
                    }
                }
            }
            Ok(Value::Map(Rc::new(RefCell::new(map))))
        }
        other => Err(ExceptionValue::new(
            "RuntimeError",
            format!("cannot convert '{}' to dict", other.type_name()),
            None,
        )),
    }
}

#[cfg(test)]
mod m3_tests {
    use super::*;
    use helen_core::lexer::Scanner;

    fn run_src(src: &str) -> (Result<Option<Value>, ExceptionValue>, String) {
        let mut scanner = Scanner::new(src, "t.helen");
        let tokens = scanner.scan_all();
        let mut parser = helen_parser::Parser::new(tokens);
        let program = parser.parse();
        assert!(
            parser.errors().is_empty(),
            "parse errs: {:?}",
            parser.errors()
        );
        let mut interp = Interpreter::new();
        let r = interp.interpret(&program);
        let out = interp.stdout.borrow().clone();
        (r, out)
    }

    #[test]
    fn prints_hello() {
        let (r, out) = run_src("import std.core.*\nmain {\n    print(\"hello\")\n}\n");
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "hello\n");
    }

    #[test]
    fn arithmetic_and_string_interp() {
        let src = "import std.core.*\nmain {\n    let x = 6 * 7\n    print(\"sum:\", x)\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "sum: 42\n");
    }

    #[test]
    fn float_str_matches_python() {
        let src =
            "import std.core.*\nmain {\n    print(3.0)\n    print(3.14)\n    print(2.0 + 1)\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "3.0\n3.14\n3.0\n");
    }

    #[test]
    fn function_call_and_return() {
        let src = "import std.core.*\nfn add(a: int, b: int): int {\n    return a + b\n}\nmain {\n    print(add(2, 3))\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "5\n");
    }

    #[test]
    fn closure_captures_env() {
        let src = "import std.core.*\nmain {\n    let make_counter = fn() {\n        let n = 0\n        return fn(): int {\n            n = n + 1\n            return n\n        }\n    }\n    let c = make_counter()\n    c()\n    c()\n    print(c())\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "3\n");
    }

    #[test]
    fn try_catch_swallows_exception() {
        // Python parity: the 11-entry runtime whitelist rejects ValueError;
        // RuntimeError is the generic catch-all the language exposes.
        let src = "import std.core.*\nmain {\n    try {\n        throw RuntimeError(\"boom\")\n    } catch RuntimeError e {\n        print(\"caught\")\n    }\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "caught\n");
    }

    #[test]
    fn division_by_zero_raises() {
        // Python parity: int/int division by zero raises a plain RuntimeError
        // ("RuntimeError: Division by zero"), NOT ZeroDivisionError.
        let src = "import std.core.*\nmain {\n    let x = 5 / 0\n    print(x)\n}\n";
        let (r, _out) = run_src(src);
        assert!(r.is_err(), "should raise");
        let e = r.unwrap_err();
        assert_eq!(e.class_name, "RuntimeError");
        assert_eq!(e.message, "Division by zero");
    }

    #[test]
    fn while_loop_sums() {
        let src = "import std.core.*\nmain {\n    let total = 0\n    let i = 0\n    while i < 5 {\n        total = total + i\n        i = i + 1\n    }\n    print(total)\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "10\n");
    }

    #[test]
    fn list_methods_and_index() {
        let src = "import std.core.*\nmain {\n    let xs = [1, 2, 3]\n    print(len(xs))\n    print(xs[0])\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "3\n1\n");
    }

    #[test]
    fn list_append_and_pop() {
        let src = "import std.core.*\nmain {\n    let xs = [1, 2, 3]\n    xs.append(4)\n    print(xs)\n    xs.pop()\n    print(xs)\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "[1, 2, 3, 4]\n[1, 2, 3]\n");
    }

    #[test]
    fn try_finally_runs() {
        // Python parity: finally always runs even when the body throws.
        let src = "import std.core.*\nmain {\n    try {\n        throw RuntimeError(\"x\")\n    } finally {\n        print(\"cleanup\")\n    }\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_err(), "should rethrow");
        assert_eq!(out, "cleanup\n");
    }

    #[test]
    fn template_interpolation() {
        let src = "import std.core.*\nmain {\n    let x = 42\n    print({{x}})\n    print({{x + 1}})\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "42\n43\n");
    }

    // ------------------------------------------------------------------
    // Task 3.7: stdlib module imports (three forms)
    // ------------------------------------------------------------------

    #[test]
    fn stdlib_wildcard_import() {
        // `import std.list.*` binds sort; `import std.math.*` binds round.
        let src = "import std.core.*\nimport std.list.*\nimport std.math.*\nmain {\n    print(sort([3, 1, 2]))\n    print(round(3.7))\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "[1, 2, 3]\n4.0\n");
    }

    #[test]
    fn stdlib_selective_import() {
        // `import std.str.{upper, lower}` binds only the named exports.
        let src = "import std.core.*\nimport std.str.{upper, lower}\nmain {\n    print(upper(\"hi\"))\n    print(lower(\"HI\"))\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "HI\nhi\n");
    }

    #[test]
    fn stdlib_namespace_import() {
        // `import std.dict as D` creates a module object (map of fns).
        let src = "import std.core.*\nimport std.dict as D\nmain {\n    let d = {\"a\": 1}\n    print(D.keys(d))\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "['a']\n");
    }

    #[test]
    fn stdlib_unknown_module_errors() {
        // Python parity: `_runtime_error` on unknown module.
        let src = "import std.core.*\nimport std.nope.*\nmain {\n    print(1)\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_err(), "should raise");
        let e = r.unwrap_err();
        assert!(
            e.message.contains("Unknown stdlib module 'std.nope'"),
            "msg: {}",
            e.message
        );
        assert_eq!(out, "");
    }

    #[test]
    fn stdlib_unknown_function_errors() {
        // Python parity: selective import of an unknown export errors.
        let src = "import std.core.*\nimport std.str.{nope}\nmain {\n    print(1)\n}\n";
        let (r, _out) = run_src(src);
        assert!(r.is_err(), "should raise");
        let e = r.unwrap_err();
        assert!(
            e.message
                .contains("Function 'nope' not found in module 'std.str'"),
            "msg: {}",
            e.message
        );
    }

    #[test]
    fn stdlib_higher_order_functions() {
        // map/filter/reduce receive closures (Python `_map`/`_filter`/`_reduce`).
        let src = "import std.core.*\nimport std.list.*\nmain {\n    let nums = [1, 2, 3, 4, 5]\n    let doubled = map(nums, fn(x) { return x * 2 })\n    let evens = filter(nums, fn(x) { return x % 2 == 0 })\n    let total = reduce(nums, fn(acc, x) { return acc + x }, 0)\n    print(doubled)\n    print(evens)\n    print(total)\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "[2, 4, 6, 8, 10]\n[2, 4]\n15\n");
    }

    #[test]
    fn stdlib_str_functions() {
        let src = "import std.core.*\nimport std.str.*\nmain {\n    print(upper(\"Hello\"))\n    print(substring(\"Hello\", 1, 3))\n    print(join(\"-\", [\"a\", \"b\"]))\n    print(contains(\"hello\", \"ell\"))\n}\n";
        let (r, out) = run_src(src);
        assert!(r.is_ok(), "{r:?}");
        assert_eq!(out, "HELLO\nel\na-b\ntrue\n");
    }
}
