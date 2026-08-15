//! Semantic analyzer for the Helen language.
//!
//! Byte-faithful port of `helen/semantic/analyzer.py` (v1.44.0): walks the
//! AST performing semantic checks — variable declaration/resolution, type
//! compatibility, control-flow validation, const protection, agent scope
//! isolation, catch-type validation, import verification, and more.
//!
//! Produces the same `ErrorCode` diagnostics (and messages) as the Python
//! reference, in the same order, so the E-code differential is byte-exact.

use crate::diagnostics::ErrorReporter;
use crate::stdlib;
use crate::symbols::{Symbol, SymbolTable};
use crate::type_utils::from_type_ref;
use crate::types::{type_compatible, type_of_literal, Type};
use helen_core::ast::*;
use helen_core::errors::ErrorCode;
use helen_core::source::SourceSpan;
use helen_core::tokens::TokenType;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Predefined exception type names (HLD 3.6.4).
///
/// **Verified against the reference** `helen/semantic/analyzer.py`:
/// the analyzer's own frozenset has **15 entries** — it accepts the six
/// Python-style names (`ValueError`, `TypeError`, `KeyError`, `IndexError`,
/// `FileNotFoundError`, `PermissionError`) and rejects
/// `PromptTooLongError`/`LLMOutputContractError`. The plan note ("exactly 11
/// native names", sourced from `interpreter/exceptions.py`) is outdated; the
/// differential must mirror this frozenset.
pub const PREDEFINED_EXCEPTIONS: &[&str] = &[
    "AnyError",
    "LLMError",
    "TimeoutError",
    "ModelError",
    "AgentError",
    "ToolError",
    "RuntimeError",
    "AggregateError",
    "AssertionError",
    "ValueError",
    "TypeError",
    "KeyError",
    "IndexError",
    "FileNotFoundError",
    "PermissionError",
];

/// Stdlib module order for the "Available: …" message (Python `module_map`
/// insertion order in `_analyze_stdlib_import`).
const STDLIB_MODULE_ORDER: &[&str] = &[
    "std.str",
    "std.list",
    "std.dict",
    "std.math",
    "std.time",
    "std.file",
    "std.system",
    "std.io",
    "std.core",
    "std.data",
    "std.network",
    "std.path",
    "std.tools",
    "std.debug",
    "std.context",
    "std.transcript",
    "std.media",
    "std.test",
    "std.quality",
    "std.llm",
    "std.crypto",
    "std.concurrency",
];

/// File names whose top-level statement check is skipped
/// (programmatic/test/REPL usage).
const SKIP_TOP_LEVEL_FILES: &[&str] = &["<test>", "<unknown>", "<repl>"];

/// Helen data file extensions (`_HELEN_DATA_EXTS`).
const HELEN_DATA_EXTS: &[&str] = &[".helen", ".json", ".md", ".txt", ".yaml", ".yml"];

/// The semantic analyzer (mirrors `SemanticAnalyzer`).
pub struct SemanticAnalyzer {
    pub errors: ErrorReporter,
    pub symbols: SymbolTable,
    pub base_dir: String,
    // --- transient state ---
    in_loop: usize,     // nesting depth of for/while loops
    in_function: usize, // nesting depth of fn blocks
    current_return_type: Option<Type>,
    agent_names: HashMap<String, AgentDecl>, // global agent registry
    imported_paths: HashSet<String>,         // validated import paths (abs)
    function_param_types: HashMap<String, Vec<Option<Box<TypeRef>>>>,
    in_agent: bool, // inside an agent main {} block
    agent_scope_boundary: usize,
    shared_var_names: HashSet<String>,
    in_closure: usize,
    agent_isolation_open: bool,
    in_main: bool,
    in_store_method: bool,
}

impl SemanticAnalyzer {
    pub fn new(errors: ErrorReporter, base_dir: &str) -> Self {
        let mut analyzer = SemanticAnalyzer {
            errors,
            symbols: SymbolTable::new(),
            base_dir: base_dir.to_string(),
            in_loop: 0,
            in_function: 0,
            current_return_type: None,
            agent_names: HashMap::new(),
            imported_paths: HashSet::new(),
            function_param_types: HashMap::new(),
            in_agent: false,
            agent_scope_boundary: 0,
            shared_var_names: HashSet::new(),
            in_closure: 0,
            agent_isolation_open: false,
            in_main: false,
            in_store_method: false,
        };
        analyzer.register_stdlib();
        analyzer
    }

    /// Register pre-defined const variables in the global scope
    /// (`_register_stdlib`; v1.39: stdlib functions are *not* auto-registered,
    /// users must `import std.xxx.*` explicitly).
    fn register_stdlib(&mut self) {
        let argv_sym = Symbol::new("argv", "const");
        let _ = self.symbols.define("argv", argv_sym);
    }

    /// Run semantic analysis on a full program (`analyze`). Resets transient
    /// state for REPL safety; errors are collected, not raised.
    pub fn analyze(&mut self, program: &Program) {
        self.in_loop = 0;
        self.in_function = 0;
        self.visit_program(program);
    }

    /// Reset all state for REPL `:reset` (`reset`).
    pub fn reset(&mut self) {
        self.symbols = SymbolTable::new();
        self.in_loop = 0;
        self.in_function = 0;
        self.current_return_type = None;
        self.agent_names.clear();
        self.imported_paths.clear();
        self.function_param_types.clear();
        self.register_stdlib();
    }

    /// Remove a symbol from the global scope; returns True if it existed
    /// (`undefine`).
    pub fn undefine(&mut self, name: &str) -> bool {
        let mut removed = self.symbols.global_undefine(name);
        if self.agent_names.remove(name).is_some() {
            removed = true;
        }
        if self.function_param_types.remove(name).is_some() {
            removed = true;
        }
        removed
    }

    // ------------------------------------------------------------------
    // Helpers
    // ------------------------------------------------------------------

    fn visit_stmts(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            self.visit_stmt(stmt);
        }
    }

    fn check_break_continue_position(&mut self, span: &SourceSpan, kind: &str) {
        if self.in_loop == 0 {
            let code = if kind == "break" {
                ErrorCode::BreakOutsideLoop
            } else {
                ErrorCode::ContinueOutsideLoop
            };
            self.errors.error(
                code,
                format!("{kind} can only be used inside a loop"),
                span.clone(),
            );
        }
    }

    fn check_const_assignment(&mut self, name: &str, span: &SourceSpan) {
        let is_const = self
            .symbols
            .resolve(name)
            .map(|s| s.is_const)
            .unwrap_or(false);
        if is_const {
            self.errors.error(
                ErrorCode::ConstAssignment,
                format!("cannot assign to const variable '{name}'"),
                span.clone(),
            );
        }
    }

    /// `_is_value_type` — value types are safe for `shared let`.
    fn is_value_type(&self, type_obj: &Type) -> bool {
        match type_obj {
            Type::Bool | Type::Number | Type::Int | Type::Float | Type::Str | Type::Null => true,
            Type::Optional(inner) => self.is_value_type(inner),
            Type::Any => true,
            Type::List(_) | Type::Map(_, _) | Type::Agent(_) => false,
            // UnionType and anything else: conservative reject.
            _ => false,
        }
    }

    fn check_branch_completeness(&mut self, has_default: bool, span: &SourceSpan, stmt_type: &str) {
        if !has_default {
            let code = if stmt_type == "llm_if" {
                ErrorCode::LlmIfNoDefault
            } else {
                ErrorCode::MatchNoDefault
            };
            self.errors.error(
                code,
                format!("{stmt_type} must have a default branch"),
                span.clone(),
            );
        }
    }

    /// `_check_match_completeness` — match statement must be exhaustive.
    fn check_match_completeness(&mut self, node: &MatchStmtNode) {
        let has_default_pattern = node.cases.iter().any(|case| {
            matches!(
                case.pattern,
                Expr::WildcardPattern(_) | Expr::VariablePattern(_)
            )
        });
        self.check_branch_completeness(
            !node.default.is_empty() || has_default_pattern,
            &node.span,
            "match",
        );
    }

    /// `_check_match_expr_completeness` — match expression must be exhaustive.
    fn check_match_expr_completeness(&mut self, node: &MatchExprNode) {
        let has_default_pattern = node.cases.iter().any(|case| {
            matches!(
                case.pattern,
                Expr::WildcardPattern(_) | Expr::VariablePattern(_)
            )
        });
        self.check_branch_completeness(
            node.default_body.is_some() || has_default_pattern,
            &node.span,
            "match",
        );
    }

    /// `_type_from_typenode` — delegate to the shared utility.
    fn type_from_typenode(&self, type_node: Option<&TypeRef>) -> Type {
        match type_node {
            Some(tr) => from_type_ref(tr),
            None => Type::Any,
        }
    }
    fn infer_type(&self, expr: &Expr) -> Type {
        match expr {
            Expr::Literal(Lit { value, .. }) => type_of_literal(value),
            Expr::List(ListLit { elements, .. }) => {
                if let Some(first) = elements.first() {
                    Type::List(Box::new(self.infer_type(first)))
                } else {
                    Type::Any
                }
            }
            Expr::Map(MapLit { entries, .. }) => {
                if let Some(entry) = entries.first() {
                    Type::Map(
                        Box::new(self.infer_type(&entry.key)),
                        Box::new(self.infer_type(&entry.value)),
                    )
                } else {
                    Type::Any
                }
            }
            _ => Type::Any,
        }
    }

    /// `_extract_assignment_target` — base variable name of an assignment
    /// target (`x`, `arr[i]` → `arr`, `obj.field` → `obj`).
    fn extract_assignment_target<'a>(&self, expr: &'a Expr) -> Option<&'a str> {
        match expr {
            Expr::Variable(Variable { name, .. }) => Some(name),
            Expr::Index(Index { target, .. }) => self.extract_assignment_target(target),
            Expr::Access(Access { target, .. }) => self.extract_assignment_target(target),
            _ => None,
        }
    }

    /// `_in_store_scope` — whether the *immediate* scope is a store scope.
    fn in_store_scope(&self) -> bool {
        self.symbols.depth() >= 1 && self.symbols.current_scope_type() == "store"
    }

    /// `_define_user_symbol` — define a user symbol, rejecting builtin
    /// shadowing. Returns the existing symbol (cloned) on conflict, else None.
    fn define_user_symbol(
        &mut self,
        name: &str,
        symbol: Symbol,
        span: Option<&SourceSpan>,
    ) -> Option<Symbol> {
        if !self.in_store_scope() {
            if let Some(existing) = self.symbols.resolve(name).cloned() {
                // Case 1: existing is builtin, new is not builtin → shadowing.
                if existing.kind == "builtin" && symbol.kind != "builtin" {
                    self.report_builtin_shadowed(name, span);
                    return Some(existing);
                }
                // Case 2: existing is not builtin, new is builtin → shadowing.
                if existing.kind != "builtin" && symbol.kind == "builtin" {
                    self.report_builtin_shadowed(name, span);
                    return Some(existing);
                }
                // Non-builtin duplicate: Python's `symbols.define` returns the
                // existing symbol so the caller emits DUPLICATE_SYMBOL etc.
                return self.symbols.define(name, symbol).cloned();
            }
        }
        let _ = self.symbols.define(name, symbol);
        None
    }

    fn report_builtin_shadowed(&mut self, name: &str, span: Option<&SourceSpan>) {
        let message = format!(
            "cannot shadow builtin '{name}'; choose a different name \
             (hint: try '{name}s', 'my_{name}', or another non-builtin name)"
        );
        match span {
            Some(sp) => self
                .errors
                .error(ErrorCode::BuiltinShadowed, message, sp.clone()),
            None => self
                .errors
                .error_no_span(ErrorCode::BuiltinShadowed, message),
        }
    }

    // ------------------------------------------------------------------
    // Statement / expression dispatch
    // ------------------------------------------------------------------

    fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::VarDecl(n) => self.visit_var_decl(n),
            Stmt::SharedStoreDecl(n) => self.visit_shared_store_decl(n),
            Stmt::If(n) => self.visit_if_stmt(n),
            Stmt::For(n) => self.visit_for_stmt(n),
            Stmt::While(n) => self.visit_while_stmt(n),
            Stmt::Break(n) => self.visit_break_stmt(n),
            Stmt::Continue(n) => self.visit_continue_stmt(n),
            Stmt::Return(n) => self.visit_return_stmt(n),
            Stmt::Expr(n) => self.visit_expr_stmt(n),
            Stmt::PromptDef(_) => {}
            Stmt::Declaration(_) => {}
            Stmt::AgentParam(n) => self.visit_agent_param(n),
            Stmt::ContextConfig(_) => {}
            Stmt::AgentDecl(n) => self.visit_agent_decl(n),
            Stmt::MainBlock(n) => self.visit_main_block(n),
            Stmt::FunctionDecl(n) => self.visit_function_decl(n),
            Stmt::FnBlock(n) => self.visit_fn_block(n),
            Stmt::ProtocolDecl(n) => self.visit_protocol_decl(n),
            Stmt::ImplDecl(n) => self.visit_impl_decl(n),
            Stmt::Import(n) => self.visit_import_stmt(n),
            Stmt::Alias(n) => self.visit_alias_stmt(n),
            Stmt::Case(n) => self.visit_case(n),
            Stmt::CatchClause(n) => self.visit_catch_clause(n),
            Stmt::CatchAll(n) => self.visit_catch_all(n),
            Stmt::FinallyBlock(n) => self.visit_finally_block(n),
            Stmt::Try(n) => self.visit_try_stmt(n),
            Stmt::Throw(n) => self.visit_throw_stmt(n),
            Stmt::Assert(n) => self.visit_assert_stmt(n),
            Stmt::LlmBranch(n) => self.visit_llm_branch(n),
            Stmt::LlmIf(n) => self.visit_llm_if_stmt(n),
            Stmt::Match(n) => self.visit_match_stmt(n),
        }
    }

    fn visit_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal(_) => {} // literals are always valid
            Expr::Variable(n) => self.visit_variable(n),
            Expr::Binary(n) => self.visit_binary_op(n),
            Expr::Pipe(n) => self.visit_pipe_expr(n),
            Expr::Unary(n) => self.visit_unary_op(n),
            Expr::Grouping(n) => self.visit_grouping(n),
            Expr::Call(n) => self.visit_call(n),
            Expr::Index(n) => self.visit_index(n),
            Expr::Access(n) => self.visit_access(n),
            Expr::List(n) => self.visit_list_literal(n),
            Expr::Map(n) => self.visit_map_literal(n),
            Expr::TemplateRef(n) => self.visit_template_ref(n),
            Expr::Type(_) => {} // type references are validated during usage
            Expr::OptionalType(n) => self.visit_type_ref(&n.inner),
            Expr::UnionType(n) => {
                for member in &n.members {
                    self.visit_type_ref(member);
                }
            }
            Expr::LiteralType(n) => {
                for value in &n.values {
                    self.visit_expr(value);
                }
            }
            Expr::Lambda(n) => self.visit_lambda(n),
            Expr::Spawn(n) => self.visit_spawn_expr(n),
            Expr::LlmAct(n) => self.visit_llm_act_expr(n),
            Expr::MatchExpr(n) => self.visit_match_expr(n),
            Expr::RangePattern(n) => self.visit_range_pattern(n),
            Expr::WildcardPattern(_) => {}
            Expr::VariablePattern(_) => {} // binding handled by the interpreter
            Expr::TypePattern(_) => {}     // runtime type check
        }
    }

    fn visit_type_ref(&mut self, tr: &TypeRef) {
        match &tr.kind {
            TypeRefKind::Simple => {}
            TypeRefKind::Optional(inner) => self.visit_type_ref(inner),
            TypeRefKind::Union(members) => {
                for member in members {
                    self.visit_type_ref(member);
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Program & blocks
    // ------------------------------------------------------------------

    fn visit_program(&mut self, program: &Program) {
        // Pass 1: collect function and agent declarations (names only).
        for stmt in &program.statements {
            match stmt {
                Stmt::FunctionDecl(f) => self.register_function_signature(f),
                Stmt::AgentDecl(a) => self.register_agent_signature(a),
                _ => {}
            }
        }
        // v1.30: top-level restrictions — only declarations allowed.
        self.check_top_level_statements(program);
        // Pass 2: full analysis.
        for stmt in &program.statements {
            self.visit_stmt(stmt);
        }
    }

    fn check_top_level_statements(&mut self, program: &Program) {
        if let Some(first) = program.statements.first() {
            if SKIP_TOP_LEVEL_FILES.contains(&stmt_span(first).file.as_str()) {
                return;
            }
        }

        let mut main_count = 0;
        for stmt in &program.statements {
            match stmt {
                // Allowed: pure declarations.
                Stmt::FunctionDecl(_)
                | Stmt::AgentDecl(_)
                | Stmt::Import(_)
                | Stmt::Alias(_)
                | Stmt::ProtocolDecl(_)
                | Stmt::ImplDecl(_)
                | Stmt::SharedStoreDecl(_) => {}
                // Allowed: const (mutable=false).
                Stmt::VarDecl(v) if !v.mutable => {}
                // Allowed: shared let / shared const.
                Stmt::VarDecl(v) if v.shared => {}
                // Forbidden: bare let (mutable and not shared).
                Stmt::VarDecl(v) => {
                    self.errors.error(
                        ErrorCode::TopLevelStatement,
                        "module-level 'let' is not allowed; \
                         use 'const' for constants or wrap in 'main { ... }'",
                        v.span.clone(),
                    );
                }
                // Allowed: main block (at most one).
                Stmt::MainBlock(m) => {
                    main_count += 1;
                    if main_count > 1 {
                        self.errors.error(
                            ErrorCode::DuplicateDeclaration,
                            "duplicate 'main' block; only one is allowed per file",
                            m.span.clone(),
                        );
                    }
                }
                // Forbidden: all other executable statements.
                _ => {
                    self.errors.error(
                        ErrorCode::TopLevelStatement,
                        "top-level executable statements are not allowed; \
                         wrap in 'main { ... }' or move into a function",
                        stmt_span(stmt),
                    );
                }
            }
        }
    }

    /// `_register_function_signature` — pass 1, names only.
    fn register_function_signature(&mut self, node: &FunctionDecl) {
        // Duplicate param names.
        let mut seen_params: HashSet<String> = HashSet::new();
        for param in &node.params {
            if seen_params.contains(&param.name) {
                self.errors.error(
                    ErrorCode::DuplicateParam,
                    format!(
                        "duplicate parameter '{}' in function '{}'",
                        param.name, node.name
                    ),
                    param.span.clone(),
                );
            }
            seen_params.insert(param.name.clone());
        }

        let sym = Symbol {
            name: node.name.clone(),
            kind: "function".into(),
            type_node: node.return_type.clone(),
            is_const: false,
        };
        let existing = self.define_user_symbol(&node.name, sym, Some(&node.span));
        if let Some(existing) = existing {
            if existing.kind != "builtin" {
                self.errors.error(
                    ErrorCode::DuplicateSymbol,
                    format!("duplicate declaration of '{}'", node.name),
                    node.span.clone(),
                );
            }
        }

        self.function_param_types.insert(
            node.name.clone(),
            node.params
                .iter()
                .map(|p| p.type_annotation.clone())
                .collect(),
        );
    }

    /// `_register_agent_signature` — pass 1, names only.
    fn register_agent_signature(&mut self, node: &AgentDecl) {
        let sym = Symbol::new(&node.name, "agent");
        let existing = self.define_user_symbol(&node.name, sym, Some(&node.span));
        if let Some(existing) = existing {
            if existing.kind != "builtin" {
                self.errors.error(
                    ErrorCode::DuplicateAgentName,
                    format!("duplicate agent name '{}'", node.name),
                    node.span.clone(),
                );
            }
        }
        self.agent_names.insert(node.name.clone(), node.clone());
    }

    fn visit_main_block(&mut self, node: &MainBlock) {
        let old_in_main = self.in_main;
        self.in_main = true;
        self.symbols.enter_scope("main", "block");
        self.visit_stmts(&node.body);
        self.symbols.exit_scope();
        self.in_main = old_in_main;
    }

    // ------------------------------------------------------------------
    // Variable declarations
    // ------------------------------------------------------------------

    fn visit_var_decl(&mut self, node: &VarDecl) {
        // v1.18: recursive closure support — pre-define the symbol when the
        // initializer is a lambda so the lambda can reference its own name.
        let is_lambda_init = matches!(node.initializer.as_deref(), Some(Expr::Lambda(_)));
        if is_lambda_init {
            let fwd_symbol = Symbol {
                name: node.name.clone(),
                kind: "variable".into(),
                type_node: node.type_annotation.clone(),
                is_const: !node.mutable,
            };
            let _ = self.symbols.define(&node.name, fwd_symbol);
        }

        // Evaluate initializer first.
        if let Some(init) = &node.initializer {
            self.visit_expr(init);
        }

        // Type check v1: annotation + initializer.
        if let (Some(ta), Some(init)) = (&node.type_annotation, &node.initializer) {
            let expected = self.type_from_typenode(Some(ta));
            let actual = self.infer_type(init);
            if !type_compatible(&actual, &expected) {
                self.errors.error(
                    ErrorCode::SemanticTypeError,
                    format!(
                        "cannot assign {} to '{}' of type {}",
                        actual.name(),
                        node.name,
                        expected.name()
                    ),
                    node.span.clone(),
                );
            }
        }

        // Define symbol (skip if pre-defined for recursive closure).
        if !is_lambda_init {
            let symbol = Symbol {
                name: node.name.clone(),
                kind: "variable".into(),
                type_node: node.type_annotation.clone(),
                is_const: !node.mutable,
            };
            let existing = self.define_user_symbol(&node.name, symbol, Some(&node.span));
            if let Some(existing) = existing {
                if existing.kind != "builtin" {
                    self.errors.error(
                        ErrorCode::DuplicateSymbol,
                        format!("duplicate declaration of '{}'", node.name),
                        node.span.clone(),
                    );
                }
            }
        }

        // v1.10: track shared let names; v1.12: value-type restriction.
        if node.shared {
            let declared_type = match &node.type_annotation {
                Some(ta) => Some(self.type_from_typenode(Some(ta))),
                None => node.initializer.as_ref().map(|init| self.infer_type(init)),
            };

            if let Some(dt) = declared_type {
                if !self.is_value_type(&dt) {
                    self.errors.error(
                        ErrorCode::SemanticTypeError,
                        format!(
                            "shared let '{}' must have a value type (int, float, str, bool). \
                             Reference types (list, map) are not allowed in shared let. \
                             Use 'shared store' for sharing mutable reference types across agents.",
                            node.name
                        ),
                        node.span.clone(),
                    );
                }
            }
            self.shared_var_names.insert(node.name.clone());
        }
    }

    /// `_visit_shared_container` — shared store/channel declaration analysis.
    fn visit_shared_container(&mut self, node: &SharedStoreDecl, kind: &str) {
        let symbol = Symbol {
            name: node.name.clone(),
            kind: kind.into(),
            type_node: None,
            is_const: true,
        };
        let existing = self.define_user_symbol(&node.name, symbol, Some(&node.span));
        if let Some(existing) = existing {
            if existing.kind != "builtin" {
                self.errors.error(
                    ErrorCode::DuplicateSymbol,
                    format!("duplicate declaration of '{}'", node.name),
                    node.span.clone(),
                );
            }
        }

        self.shared_var_names.insert(node.name.clone());
        self.symbols
            .enter_scope(&format!("{kind}:{}", node.name), "store");

        let mut field_names: HashSet<String> = HashSet::new();
        for field_node in &node.fields {
            if field_names.contains(&field_node.name) {
                self.errors.error(
                    ErrorCode::DuplicateSymbol,
                    format!(
                        "duplicate field '{}' in {} '{}'",
                        field_node.name, kind, node.name
                    ),
                    field_node.span.clone(),
                );
            }
            field_names.insert(field_node.name.clone());
            self.visit_var_decl(field_node);
        }

        let mut method_names: HashSet<String> = HashSet::new();
        for method_node in &node.methods {
            if method_names.contains(&method_node.name) {
                self.errors.error(
                    ErrorCode::DuplicateSymbol,
                    format!(
                        "duplicate method '{}' in {} '{}'",
                        method_node.name, kind, node.name
                    ),
                    method_node.span.clone(),
                );
            }
            if field_names.contains(&method_node.name) {
                self.errors.error(
                    ErrorCode::DuplicateSymbol,
                    format!(
                        "method '{}' clashes with field of same name in {} '{}'",
                        method_node.name, kind, node.name
                    ),
                    method_node.span.clone(),
                );
            }
            method_names.insert(method_node.name.clone());
            let method_sym = Symbol {
                name: method_node.name.clone(),
                kind: "function".into(),
                type_node: method_node.return_type.clone(),
                is_const: false,
            };
            let _ = self.symbols.define(&method_node.name, method_sym);

            let prev_return_type = self.current_return_type.clone();
            let old_in_store_method = self.in_store_method;
            self.in_store_method = true;
            self.current_return_type =
                Some(self.type_from_typenode(method_node.return_type.as_deref()));
            self.symbols
                .enter_scope(&format!("{kind}_method:{}", method_node.name), "function");
            for param in &method_node.params {
                let param_sym = Symbol {
                    name: param.name.clone(),
                    kind: "param".into(),
                    type_node: param.type_annotation.clone(),
                    is_const: false,
                };
                self.define_user_symbol(&param.name, param_sym, Some(&param.span));
            }
            method_node.body.accept_into(self);
            self.symbols.exit_scope();
            self.current_return_type = prev_return_type;
            self.in_store_method = old_in_store_method;
        }

        self.symbols.exit_scope();
    }

    fn visit_shared_store_decl(&mut self, node: &SharedStoreDecl) {
        self.visit_shared_container(node, "store");
    }

    // ------------------------------------------------------------------
    // Variable references
    // ------------------------------------------------------------------

    fn visit_variable(&mut self, node: &Variable) {
        let resolved = self.symbols.resolve(&node.name).cloned();
        match resolved {
            None => {
                self.errors.error(
                    ErrorCode::UndeclaredVariable,
                    format!("undeclared variable '{}'", node.name),
                    node.span.clone(),
                );
            }
            Some(sym) => {
                if self.in_agent && sym.kind == "variable" {
                    if self.agent_isolation_open {
                        return;
                    }
                    let local_sym = self
                        .symbols
                        .resolve_in_chain(&node.name)
                        .map(|s| s.name.clone());
                    if local_sym.is_none() {
                        if sym.is_const {
                            return; // const is read-only shared — always OK
                        }
                        if self.shared_var_names.contains(&node.name) {
                            return; // shared let — explicitly cross-agent
                        }
                        self.errors.error(
                            ErrorCode::ScopeViolation,
                            self.agent_isolation_message(&node.name),
                            node.span.clone(),
                        );
                    }
                }
            }
        }
    }

    /// The multi-line SCOPE_VIOLATION message for agent main {} access.
    fn agent_isolation_message(&self, name: &str) -> String {
        format!(
            "agent scope isolation: cannot access module-level variable \
             '{name}' from agent main {{}}.\n\n\
             💡 Helen agents are strictly isolated — agent main runs in a \
             fresh environment that does NOT inherit module-level `let` \
             variables. This is by design (see: 'Caller Decides Context' \
             principle in helen-agent-collaboration skill).\n\n\
             How to fix:\n\
             \x20 1. Pass as parameter (recommended):\n\
             \x20      agent Worker({name}: <type>) {{ ... }}\n\
             \x20      main {{ Worker({name}) }}\n\
             \x20 2. Make it a const (read-only, auto-visible):\n\
             \x20      const {} = {{...}}\n\
             \x20 3. Use shared let (mutable, cross-agent):\n\
             \x20      shared let {name} = {{...}}\n\
             \x20 4. Pass via Channel message (spawn scenario):\n\
             \x20      ch.send({name})  // in caller\n\
             \x20      let v = ch.receive()   // in agent",
            name.to_uppercase()
        )
    }

    // ------------------------------------------------------------------
    // Control flow
    // ------------------------------------------------------------------

    fn visit_if_stmt(&mut self, node: &IfStmt) {
        self.visit_expr(&node.condition);
        // then branch: MainBlockNode creates its own scope.
        if let Stmt::MainBlock(_) = node.then_branch.as_ref() {
            self.visit_stmt(&node.then_branch);
        } else {
            self.symbols.enter_scope("if-then", "block");
            self.visit_stmt(&node.then_branch);
            self.symbols.exit_scope();
        }
        if let Some(else_branch) = &node.else_branch {
            if let Stmt::MainBlock(_) = else_branch.as_ref() {
                self.visit_stmt(else_branch);
            } else {
                self.symbols.enter_scope("if-else", "block");
                self.visit_stmt(else_branch);
                self.symbols.exit_scope();
            }
        }
    }

    fn visit_for_stmt(&mut self, node: &ForStmt) {
        self.visit_expr(&node.iterable);
        self.in_loop += 1;
        self.symbols.enter_scope("for", "block");
        if let Some(iterator) = &node.iterator {
            let sym = Symbol::new(&iterator.name, "variable");
            self.define_user_symbol(&iterator.name, sym, Some(&iterator.span));
        }
        self.visit_stmt(&node.body);
        self.symbols.exit_scope();
        self.in_loop -= 1;
    }

    fn visit_while_stmt(&mut self, node: &WhileStmt) {
        self.visit_expr(&node.condition);
        self.in_loop += 1;
        self.symbols.enter_scope("while", "block");
        self.visit_stmt(&node.body);
        self.symbols.exit_scope();
        self.in_loop -= 1;
    }

    fn visit_break_stmt(&mut self, node: &BreakStmt) {
        self.check_break_continue_position(&node.span, "break");
    }

    fn visit_continue_stmt(&mut self, node: &ContinueStmt) {
        self.check_break_continue_position(&node.span, "continue");
    }

    fn visit_return_stmt(&mut self, node: &ReturnStmt) {
        // v1.12: allow return in top-level main (Issue #26).
        if self.in_function == 0 && !self.in_agent && !self.in_main && !self.in_store_method {
            self.errors.error(
                ErrorCode::ReturnOutsideFunction,
                "return can only be used inside a function, agent main block, \
                 top-level main block, or store method",
                node.span.clone(),
            );
        }
        if let Some(value) = &node.value {
            self.visit_expr(value);
            if let Some(expected) = &self.current_return_type {
                let actual_type = self.infer_type(value);
                // Only check if we can infer a concrete type (not AnyType).
                if !matches!(actual_type, Type::Any) && !type_compatible(&actual_type, expected) {
                    self.errors.error(
                        ErrorCode::TypeMismatch,
                        format!(
                            "return type '{}' is not compatible with declared return type '{}'",
                            actual_type.name(),
                            expected.name()
                        ),
                        node.span.clone(),
                    );
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Expressions
    // ------------------------------------------------------------------

    fn visit_binary_op(&mut self, node: &Binary) {
        self.visit_expr(&node.left);
        self.visit_expr(&node.right);

        // Const assignment protection: x = value where x is const.
        if node.operator.kind == TokenType::Assign {
            let target_var_name = self
                .extract_assignment_target(&node.left)
                .map(|s| s.to_string());
            if let Some(target) = target_var_name {
                self.check_const_assignment(&target, &node.span);

                // Agent scope isolation: writing to module-level variable
                // from agent main {} is not allowed at runtime.
                if self.in_agent && !self.agent_isolation_open {
                    let sym = self.symbols.resolve(&target).cloned();
                    if let Some(sym) = sym {
                        if sym.kind == "variable" {
                            let local_sym = self
                                .symbols
                                .resolve_in_chain(&target)
                                .map(|s| s.name.clone());
                            if local_sym.is_none() {
                                // Not local — check if shared or const.
                                if sym.is_const {
                                    self.errors.error(
                                        ErrorCode::ScopeViolation,
                                        format!(
                                            "agent scope isolation: cannot assign to const \
                                             '{target}' from agent main {{}}. \
                                             const is read-only shared across agents."
                                        ),
                                        node.span.clone(),
                                    );
                                } else if !self.shared_var_names.contains(&target) {
                                    self.errors.error(
                                        ErrorCode::ScopeViolation,
                                        format!(
                                            "agent scope isolation: cannot assign to module-level \
                                             variable '{target}' from agent main {{}}. \
                                             Agent main runs in an isolated environment. \
                                             Use 'shared let' or a setter function instead."
                                        ),
                                        node.span.clone(),
                                    );
                                }
                            }
                        }
                    }
                }

                // Type check on reassignment (simple variable targets only).
                if let Expr::Variable(Variable { name, .. }) = node.left.as_ref() {
                    let sym = self.symbols.resolve(name).cloned();
                    if let Some(sym) = sym {
                        if sym.type_node.is_some() {
                            let expected = self.type_from_typenode(sym.type_node.as_deref());
                            let actual = self.infer_type(&node.right);
                            if !type_compatible(&actual, &expected) {
                                self.errors.error(
                                    ErrorCode::SemanticTypeError,
                                    format!(
                                        "cannot assign {} to '{}' of type {}",
                                        actual.name(),
                                        name,
                                        expected.name()
                                    ),
                                    node.span.clone(),
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    fn visit_pipe_expr(&mut self, node: &Pipe) {
        self.visit_expr(&node.value);
        self.visit_expr(&node.function);
    }

    fn visit_unary_op(&mut self, node: &Unary) {
        self.visit_expr(&node.operand);
    }

    fn visit_grouping(&mut self, node: &Grouping) {
        self.visit_expr(&node.expression);
    }

    fn visit_call(&mut self, node: &Call) {
        // Agents are not in the symbol table — skip variable resolution.
        if let Expr::Variable(Variable { name, .. }) = node.callee.as_ref() {
            if !self.agent_names.contains_key(name) {
                self.visit_expr(&node.callee);
            }
        } else {
            self.visit_expr(&node.callee);
        }
        for arg in &node.arguments {
            self.visit_expr(&arg.value);
        }

        // Compile-time parameter type check for function calls (literals).
        if let Expr::Variable(Variable { name, .. }) = node.callee.as_ref() {
            if let Some(param_types) = self.function_param_types.get(name) {
                for (i, arg) in node.arguments.iter().enumerate() {
                    if let Some(pt) = param_types.get(i).and_then(|o| o.as_deref()) {
                        let expected_type = self.type_from_typenode(Some(pt));
                        if let Expr::Literal(Lit { value, span }) = &arg.value {
                            let actual_type = type_of_literal(value);
                            if !type_compatible(&actual_type, &expected_type) {
                                self.errors.error(
                                    ErrorCode::TypeMismatch,
                                    format!(
                                        "argument {} type '{}' is not compatible with parameter type '{}'",
                                        i + 1,
                                        actual_type.name(),
                                        expected_type.name()
                                    ),
                                    span.clone(),
                                );
                            }
                        }
                    }
                }
            }
        }

        // AGENT_PARAM_MISMATCH: known agent callee → validate args.
        if let Expr::Variable(Variable { name, .. }) = node.callee.as_ref() {
            if let Some(agent_node) = self.agent_names.get(name) {
                let param_names: HashSet<&String> =
                    agent_node.params.iter().map(|p| &p.name).collect();
                let mut call_arg_names: HashSet<String> = HashSet::new();
                for arg in &node.arguments {
                    if let Some(arg_name) = &arg.name {
                        if !param_names.contains(arg_name) {
                            let arg_span = expr_span(&arg.value);
                            self.errors.error(
                                ErrorCode::AgentParamMismatch,
                                format!("agent '{name}' has no parameter named '{arg_name}'"),
                                arg_span,
                            );
                        }
                        call_arg_names.insert(arg_name.clone());
                    }
                }
                // Soft check: required params without defaults — Helen allows
                // partial args.
            }
        }

        // v1.42: agent function static call `AgentName.function_name(args)`.
        if let Expr::Access(Access {
            target, property, ..
        }) = node.callee.as_ref()
        {
            if let Expr::Variable(Variable { name, .. }) = target.as_ref() {
                let agent_name = name.clone();
                let fn_name = property.clone();
                if let Some(agent_node) = self.agent_names.get(&agent_name) {
                    let fn_node = agent_node
                        .functions
                        .iter()
                        .find(|f| f.name == fn_name)
                        .cloned();
                    match fn_node {
                        None => {
                            self.errors.error(
                                ErrorCode::UndeclaredAgentFunction,
                                format!(
                                    "Agent '{agent_name}' has no function '{fn_name}' \
                                     in its functions{{}} block"
                                ),
                                node.span.clone(),
                            );
                        }
                        Some(fn_node) => {
                            // Argument count check.
                            let n_params = fn_node.params.len();
                            let n_args = node.arguments.len();
                            let n_required = fn_node
                                .params
                                .iter()
                                .filter(|p| p.default_value.is_none())
                                .count();
                            if n_args < n_required || n_args > n_params {
                                let expected = if n_required == n_params {
                                    format!("{n_params}")
                                } else {
                                    format!("{n_required}..{n_params}")
                                };
                                self.errors.error(
                                    ErrorCode::AgentFunctionArgMismatch,
                                    format!(
                                        "agent function '{agent_name}.{fn_name}' expects \
                                         {expected} argument(s), got {n_args}"
                                    ),
                                    node.span.clone(),
                                );
                            } else {
                                // Literal type check.
                                for (i, arg) in node.arguments.iter().enumerate() {
                                    if let Some(pt) = fn_node
                                        .params
                                        .get(i)
                                        .and_then(|p| p.type_annotation.as_deref())
                                    {
                                        let expected_type = self.type_from_typenode(Some(pt));
                                        if let Expr::Literal(Lit { value, span }) = &arg.value {
                                            let actual_type = type_of_literal(value);
                                            if !type_compatible(&actual_type, &expected_type) {
                                                self.errors.error(
                                                    ErrorCode::TypeMismatch,
                                                    format!(
                                                        "argument {} type '{}' is not compatible with parameter type '{}'",
                                                        i + 1,
                                                        actual_type.name(),
                                                        expected_type.name()
                                                    ),
                                                    span.clone(),
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn visit_index(&mut self, node: &Index) {
        self.visit_expr(&node.target);
        self.visit_expr(&node.index);
    }

    fn visit_access(&mut self, node: &Access) {
        self.visit_expr(&node.target);
    }

    fn visit_expr_stmt(&mut self, node: &ExprStmt) {
        self.visit_expr(&node.expression);
    }

    fn visit_list_literal(&mut self, node: &ListLit) {
        for element in &node.elements {
            self.visit_expr(element);
        }
    }

    fn visit_range_pattern(&mut self, node: &RangePattern) {
        self.visit_expr(&node.start);
        self.visit_expr(&node.end);
    }

    fn visit_map_literal(&mut self, node: &MapLit) {
        for entry in &node.entries {
            self.visit_expr(&entry.key);
            self.visit_expr(&entry.value);
        }
    }

    fn visit_template_ref(&mut self, node: &TemplateRef) {
        self.visit_expr(&node.expression);
    }

    // ------------------------------------------------------------------
    // Agent declaration & parameters
    // ------------------------------------------------------------------

    fn visit_agent_decl(&mut self, node: &AgentDecl) {
        // Already registered in pass 1 (forward reference support, v1.6)?
        let already_registered = self.agent_names.contains_key(&node.name);

        // ── Validate tools declarations (HLD 3.5) ──────────────────
        let mut tools_count = 0;
        for decl in &node.declarations {
            if let Some(tools) = &decl.tools {
                tools_count += 1;
                if tools_count > 1 {
                    self.errors.error(
                        ErrorCode::InvalidToolsDeclaration,
                        format!(
                            "duplicate 'tools' declaration in agent '{}'; \
                             use a const to combine tool lists",
                            node.name
                        ),
                        decl.span.clone(),
                    );
                }
                // If tools references an identifier, verify module-level const.
                if let Expr::Variable(Variable { name, span }) = tools {
                    let const_name = name.clone();
                    let sym = self.symbols.resolve(&const_name).cloned();
                    match sym {
                        None => {
                            self.errors.error(
                                ErrorCode::UndeclaredVariable,
                                format!("undefined const '{const_name}' in tools declaration"),
                                span.clone(),
                            );
                        }
                        Some(sym) => {
                            if !matches!(sym.kind.as_str(), "variable" | "const") {
                                self.errors.error(
                                    ErrorCode::InvalidToolsDeclaration,
                                    format!(
                                        "tools must reference a const list, \
                                         but '{const_name}' is a {}",
                                        sym.kind
                                    ),
                                    span.clone(),
                                );
                            } else if !sym.is_const {
                                self.errors.error(
                                    ErrorCode::InvalidToolsDeclaration,
                                    format!(
                                        "tools must reference a const, \
                                         but '{const_name}' is a mutable variable"
                                    ),
                                    span.clone(),
                                );
                            }
                        }
                    }
                }
            }
        }

        // Duplicate param names + visit params.
        let mut seen_params: HashSet<String> = HashSet::new();
        for param in &node.params {
            if seen_params.contains(&param.name) {
                self.errors.error(
                    ErrorCode::DuplicateParam,
                    format!(
                        "duplicate parameter '{}' in agent '{}'",
                        param.name, node.name
                    ),
                    param.span.clone(),
                );
            }
            seen_params.insert(param.name.clone());
            self.visit_agent_param(param);
        }

        // Agent name PascalCase check (warning).
        if let Some(first) = node.name.chars().next() {
            if first.is_lowercase() {
                self.errors.warning(
                    ErrorCode::InvalidAgentName,
                    format!("Agent name '{}' should be PascalCase", node.name),
                    node.span.clone(),
                );
            }
        }

        // Duplicate agent names (skip if registered in pass 1).
        if !already_registered {
            if self.agent_names.contains_key(&node.name) {
                self.errors.error(
                    ErrorCode::DuplicateAgentName,
                    format!("duplicate agent name '{}'", node.name),
                    node.span.clone(),
                );
            } else {
                self.agent_names.insert(node.name.clone(), node.clone());
            }
        }

        // Validate prompt (optional).
        if node.prompt.is_some() {
            // prompt content is a string, validated at runtime.
        }

        // Analyze agent main {} with scope isolation.
        if let Some(logic) = &node.logic {
            let old_in_agent = self.in_agent;
            let old_boundary = self.agent_scope_boundary;
            let old_isolation_open = self.agent_isolation_open;
            self.in_agent = true;
            self.agent_isolation_open = node.isolation_level == "open";

            self.symbols
                .enter_scope(&format!("agent:{}", node.name), "agent");
            self.agent_scope_boundary = self.symbols.depth();

            // Register agent parameters.
            let mut registered_params: Vec<String> = Vec::new();
            for param in &node.params {
                let sym = Symbol {
                    name: param.name.clone(),
                    kind: "variable".into(),
                    type_node: param.type_annotation.clone(),
                    is_const: false,
                };
                self.define_user_symbol(&param.name, sym, Some(&param.span));
                registered_params.push(param.name.clone());
            }

            // Register agent-scoped functions (forward refs within agent).
            let mut registered_funcs: Vec<String> = Vec::new();
            for func_node in &node.functions {
                let sym = Symbol {
                    name: func_node.name.clone(),
                    kind: "function".into(),
                    type_node: func_node.return_type.clone(),
                    is_const: false,
                };
                self.define_user_symbol(&func_node.name, sym, Some(&func_node.span));
                registered_funcs.push(func_node.name.clone());
            }

            // v1.12: function_vars — register + analyze + type check.
            let mut registered_fvars: Vec<String> = Vec::new();
            for var_node in &node.function_vars {
                let mut sym = Symbol {
                    name: var_node.name.clone(),
                    kind: "variable".into(),
                    type_node: var_node.type_annotation.clone(),
                    is_const: false,
                };
                if !var_node.mutable {
                    sym.is_const = true;
                }
                self.define_user_symbol(&var_node.name, sym, Some(&var_node.span));
                registered_fvars.push(var_node.name.clone());

                if let Some(init) = &var_node.initializer {
                    self.visit_expr(init);
                }
                if let (Some(ta), Some(init)) = (&var_node.type_annotation, &var_node.initializer) {
                    let expected = self.type_from_typenode(Some(ta));
                    let actual = self.infer_type(init);
                    if !type_compatible(&actual, &expected) {
                        self.errors.error(
                            ErrorCode::SemanticTypeError,
                            format!(
                                "cannot assign {} to '{}' of type {}",
                                actual.name(),
                                var_node.name,
                                expected.name()
                            ),
                            var_node.span.clone(),
                        );
                    }
                }
            }

            // Analyze agent functions {} bodies.
            for func_node in &node.functions {
                let prev_return_type = self.current_return_type.clone();
                self.current_return_type =
                    Some(self.type_from_typenode(func_node.return_type.as_deref()));

                self.in_function += 1;
                self.symbols
                    .enter_scope(&format!("agent_fn:{}", func_node.name), "function");
                for param in &func_node.params {
                    let sym = Symbol {
                        name: param.name.clone(),
                        kind: "param".into(),
                        type_node: param.type_annotation.clone(),
                        is_const: false,
                    };
                    self.define_user_symbol(&param.name, sym, Some(&param.span));
                }
                func_node.body.accept_into(self);
                self.symbols.exit_scope();
                self.in_function -= 1;
                self.current_return_type = prev_return_type;
            }

            self.visit_stmt(logic);

            // Clean up agent-scoped registrations.
            for fname in &registered_funcs {
                self.symbols.undefine(fname);
            }
            for vname in &registered_fvars {
                self.symbols.undefine(vname);
            }
            for pname in &registered_params {
                self.symbols.undefine(pname);
            }
            self.symbols.exit_scope();
            self.in_agent = old_in_agent;
            self.agent_scope_boundary = old_boundary;
            self.agent_isolation_open = old_isolation_open;
        }
    }

    fn visit_agent_param(&mut self, node: &AgentParam) {
        if let Some(default_value) = &node.default_value {
            self.visit_expr(default_value);
        }
        if let Some(ta) = &node.type_annotation {
            self.visit_type_ref(ta);
        }
    }

    // ------------------------------------------------------------------
    // Functions
    // ------------------------------------------------------------------

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        // Already registered in pass 1 (forward reference support, v1.6)?
        let already_registered = self.function_param_types.contains_key(&node.name);

        // Duplicate param names.
        let mut seen_params: HashSet<String> = HashSet::new();
        for param in &node.params {
            if seen_params.contains(&param.name) {
                self.errors.error(
                    ErrorCode::DuplicateParam,
                    format!(
                        "duplicate parameter '{}' in function '{}'",
                        param.name, node.name
                    ),
                    param.span.clone(),
                );
            }
            seen_params.insert(param.name.clone());
            self.visit_agent_param(param);
        }

        // Return type annotation.
        if let Some(rt) = &node.return_type {
            self.visit_type_ref(rt);
        }

        // Register function (skip if already registered in pass 1).
        if !already_registered {
            let sym = Symbol {
                name: node.name.clone(),
                kind: "function".into(),
                type_node: node.return_type.clone(),
                is_const: false,
            };
            let existing = self.define_user_symbol(&node.name, sym, Some(&node.span));
            if let Some(existing) = existing {
                if existing.kind != "builtin" {
                    self.errors.error(
                        ErrorCode::DuplicateSymbol,
                        format!("duplicate declaration of '{}'", node.name),
                        node.span.clone(),
                    );
                }
            }
            self.function_param_types.insert(
                node.name.clone(),
                node.params
                    .iter()
                    .map(|p| p.type_annotation.clone())
                    .collect(),
            );
        }

        // Record error count before body analysis.
        let errors_before_body = self.errors.error_count();

        // Save previous return type (nested functions) and set current.
        let prev_return_type = self.current_return_type.clone();
        self.current_return_type = Some(self.type_from_typenode(node.return_type.as_deref()));

        // Function body gets its own scope.
        self.in_function += 1;
        self.symbols
            .enter_scope(&format!("fn:{}", node.name), "function");
        for param in &node.params {
            let sym = Symbol {
                name: param.name.clone(),
                kind: "param".into(),
                type_node: param.type_annotation.clone(),
                is_const: false,
            };
            self.define_user_symbol(&param.name, sym, Some(&param.span));
        }
        node.body.accept_into(self);
        self.symbols.exit_scope();
        self.in_function -= 1;
        self.current_return_type = prev_return_type;

        // If the body produced new errors, remove the symbol so the function
        // can be redefined after fixing the error.
        if self.errors.error_count() > errors_before_body {
            self.symbols.undefine(&node.name);
        }
    }

    fn visit_fn_block(&mut self, node: &FnBlock) {
        self.visit_stmts(&node.body);
    }

    fn visit_lambda(&mut self, node: &Lambda) {
        self.in_function += 1;
        self.in_closure += 1; // v1.10: closure nesting for agent scope check
        self.symbols.enter_scope("lambda", "lambda");
        for param in &node.params {
            let sym = Symbol {
                name: param.name.clone(),
                kind: "param".into(),
                type_node: param.type_annotation.clone(),
                is_const: false,
            };
            self.define_user_symbol(&param.name, sym, Some(&param.span));
        }
        node.body.accept_into(self);
        self.symbols.exit_scope();
        self.in_function -= 1;
        self.in_closure -= 1;
    }

    // ------------------------------------------------------------------
    // Protocol & impl
    // ------------------------------------------------------------------

    fn visit_protocol_decl(&mut self, node: &ProtocolDecl) {
        let sym = Symbol::new(&node.name, "protocol");
        let existing = self.define_user_symbol(&node.name, sym, Some(&node.span));
        if let Some(existing) = existing {
            if existing.kind != "builtin" {
                self.errors.error(
                    ErrorCode::DuplicateSymbol,
                    format!("duplicate declaration of protocol '{}'", node.name),
                    node.span.clone(),
                );
            }
        }
        for method in &node.methods {
            self.visit_function_decl(method);
        }
    }

    fn visit_impl_decl(&mut self, node: &ImplDecl) {
        let protocol_sym = self.symbols.resolve(&node.protocol_name).cloned();
        if protocol_sym.is_none() || protocol_sym.as_ref().unwrap().kind != "protocol" {
            self.errors.error(
                ErrorCode::UndeclaredVariable,
                format!("undefined protocol '{}'", node.protocol_name),
                node.span.clone(),
            );
        }

        let struct_sym = self.symbols.resolve(&node.struct_name).cloned();
        if struct_sym.is_none() || struct_sym.as_ref().unwrap().kind != "struct" {
            self.errors.error(
                ErrorCode::UndeclaredVariable,
                format!("undefined struct '{}'", node.struct_name),
                node.span.clone(),
            );
        }

        for method in &node.methods {
            self.visit_function_decl(method);
        }
    }

    // ------------------------------------------------------------------
    // Import
    // ------------------------------------------------------------------

    fn visit_import_stmt(&mut self, node: &ImportStmt) {
        // v1.34: stdlib module imports.
        if node.is_stdlib_module {
            self.analyze_stdlib_import(node);
            return;
        }

        let path = node.module_path.clone();

        // Python module import (no extension / .py / dotted names).
        if !is_helen_data_file(&path) {
            let alias = node
                .alias
                .clone()
                .unwrap_or_else(|| path.rsplit('.').next().unwrap_or("").to_string());
            let sym = Symbol::new(&alias, "import");
            self.define_user_symbol(&alias, sym, Some(&node.span));
            return;
        }

        // P0 FIX: resolve relative paths from the importing file's directory.
        let target = self.resolve_import_target(&path, &node.span);

        if !target.exists() {
            self.errors.error(
                ErrorCode::ImportNotFound,
                format!("import file not found: '{path}'"),
                node.span.clone(),
            );
            return;
        }

        // Data files (.json/.md/.txt/.yaml/.yml) → register alias.
        if [".json", ".md", ".txt", ".yaml", ".yml"]
            .iter()
            .any(|ext| path.ends_with(ext))
        {
            let alias = node.alias.clone().unwrap_or_else(|| {
                Path::new(&path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default()
            });
            let sym = Symbol::new(&alias, "import");
            self.define_user_symbol(&alias, sym, Some(&node.span));
            return;
        }

        // .helen file import: parse and register functions/agents/consts.
        let abs_target = target
            .canonicalize()
            .unwrap_or_else(|_| target.clone())
            .to_string_lossy()
            .to_string();
        if self.imported_paths.contains(&abs_target) {
            if let Some(alias) = &node.alias {
                let sym = Symbol::new(alias, "module");
                self.define_user_symbol(alias, sym, Some(&node.span));
            }
            return;
        }
        self.imported_paths.insert(abs_target);

        match std::fs::read_to_string(&target) {
            Ok(source) => {
                let target_str = target.to_string_lossy().to_string();
                let mut scanner = helen_core::lexer::Scanner::new(&source, &target_str);
                let tokens = scanner.scan_all();
                let mut parser = helen_parser::Parser::new(tokens);
                let imported_program = parser.parse();

                for stmt in &imported_program.statements {
                    match stmt {
                        Stmt::FunctionDecl(f) => {
                            let sym = Symbol {
                                name: f.name.clone(),
                                kind: "function".into(),
                                type_node: None,
                                is_const: true,
                            };
                            self.define_user_symbol(&f.name, sym, Some(&f.span));
                            self.function_param_types.insert(
                                f.name.clone(),
                                f.params.iter().map(|p| p.type_annotation.clone()).collect(),
                            );
                        }
                        Stmt::AgentDecl(a) => {
                            let sym = Symbol {
                                name: a.name.clone(),
                                kind: "agent".into(),
                                type_node: None,
                                is_const: true,
                            };
                            self.define_user_symbol(&a.name, sym, Some(&a.span));
                            self.agent_names.insert(a.name.clone(), a.clone());
                        }
                        Stmt::VarDecl(v) if !v.mutable || v.shared => {
                            // Register const and shared let declarations.
                            let kind = if !v.mutable { "const" } else { "shared" };
                            let sym = Symbol {
                                name: v.name.clone(),
                                kind: kind.into(),
                                type_node: None,
                                is_const: !v.mutable,
                            };
                            self.define_user_symbol(&v.name, sym, Some(&v.span));
                        }
                        Stmt::SharedStoreDecl(s) => {
                            // v1.17: register imported shared stores/channels.
                            let sym = Symbol {
                                name: s.name.clone(),
                                kind: "shared".into(),
                                type_node: None,
                                is_const: true,
                            };
                            self.define_user_symbol(&s.name, sym, Some(&s.span));
                            self.shared_var_names.insert(s.name.clone());
                        }
                        Stmt::Import(i) => {
                            // Recursively process imports in the imported file.
                            self.visit_import_stmt(i);
                        }
                        _ => {}
                    }
                }

                // Register alias as a module reference.
                if let Some(alias) = &node.alias {
                    let sym = Symbol::new(alias, "module");
                    self.define_user_symbol(alias, sym, Some(&node.span));
                }
            }
            Err(e) => {
                self.errors.error(
                    ErrorCode::ImportError,
                    format!("failed to parse imported file '{path}': {e}"),
                    node.span.clone(),
                );
            }
        }
    }

    /// Resolve an import path against the importing file's directory
    /// (Python `os.path` logic in `visit_import_stmt`).
    fn resolve_import_target(&self, path: &str, span: &SourceSpan) -> PathBuf {
        let p = Path::new(path);
        if p.is_absolute() {
            return p.to_path_buf();
        }
        let importing_file = span.file.clone();
        if !importing_file.starts_with('<') {
            let abs = std::fs::canonicalize(&importing_file)
                .unwrap_or_else(|_| PathBuf::from(&importing_file));
            let importing_dir = abs
                .parent()
                .map(|d| d.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("."));
            importing_dir.join(path)
        } else {
            Path::new(&self.base_dir).join(path)
        }
    }

    /// `_analyze_stdlib_import` — validate stdlib module imports (v1.34).
    fn analyze_stdlib_import(&mut self, node: &ImportStmt) {
        let module_name = node.module_name.clone().unwrap_or_default();
        if !stdlib::module_exports(&module_name).is_some() {
            self.errors.error(
                ErrorCode::ImportNotFound,
                format!(
                    "Unknown stdlib module '{module_name}'. Available: {}",
                    STDLIB_MODULE_ORDER.join(", ")
                ),
                node.span.clone(),
            );
            return;
        }

        let exports = stdlib::module_exports(&module_name).unwrap().clone();

        if let Some(namespace) = &node.namespace {
            // Namespace import: `import std.str as S` — module exists.
            let sym = Symbol {
                name: namespace.clone(),
                kind: "module".into(),
                type_node: None,
                is_const: true,
            };
            self.define_user_symbol(namespace, sym, Some(&node.span));
        } else if let Some(names) = &node.imported_names {
            if names.iter().any(|n| n == "*") {
                // Wildcard import: register all exports (kind="builtin" for
                // v1.23 builtin shadowing protection).
                for name in &exports {
                    let sym = Symbol {
                        name: name.clone(),
                        kind: "builtin".into(),
                        type_node: None,
                        is_const: true,
                    };
                    self.define_user_symbol(name, sym, Some(&node.span));
                }
                // v1.39: also register aliases (e.g. Chinese 长度 for len).
                let aliases: Vec<(String, String)> = stdlib::all_aliases();
                for (alias, canonical) in aliases {
                    if exports.contains(&canonical) {
                        let sym = Symbol {
                            name: alias.clone(),
                            kind: "builtin".into(),
                            type_node: None,
                            is_const: true,
                        };
                        self.define_user_symbol(&alias, sym, Some(&node.span));
                    }
                }
            } else {
                // Selective import: validate and register names.
                for name in names {
                    if !exports.contains(name) {
                        self.errors.error(
                            ErrorCode::ImportNotFound,
                            format!(
                                "Function '{name}' not found in module '{module_name}'. \
                                 Available: {}",
                                exports.join(", ")
                            ),
                            node.span.clone(),
                        );
                        return;
                    }
                    let sym = Symbol {
                        name: name.clone(),
                        kind: "builtin".into(),
                        type_node: None,
                        is_const: true,
                    };
                    self.define_user_symbol(name, sym, Some(&node.span));
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Alias statement
    // ------------------------------------------------------------------

    fn visit_alias_stmt(&mut self, node: &AliasStmt) {
        let canonical = node.canonical.clone();
        let alias_name = node.alias_name.clone();

        let sym = self.symbols.resolve(&canonical).cloned();
        if sym.is_none() {
            // Also check stdlib for aliases of builtin functions.
            if !stdlib::is_canonical_builtin(&canonical) {
                self.errors.error(
                    ErrorCode::UndeclaredVariable,
                    format!("cannot alias '{canonical}': name not found"),
                    node.span.clone(),
                );
                return;
            }
        }

        // Check alias_name doesn't already exist in current scope
        // (allow shadowing from outer scopes, but not in same scope).
        let existing = self.symbols.resolve_local(&alias_name).cloned();
        if existing.is_some() {
            // Allow re-aliasing the same canonical (no-op).
            let existing_canonical = stdlib::canonical_name(&alias_name);
            if existing_canonical != Some(canonical.as_str()) && stdlib::is_alias(&alias_name) {
                self.errors.error(
                    ErrorCode::DuplicateSymbol,
                    format!("alias name '{alias_name}' already defined in current scope"),
                    node.span.clone(),
                );
                return;
            }
            if stdlib::is_alias(&alias_name) {
                return;
            }
        }

        let sym = sym.unwrap_or_else(|| Symbol {
            name: alias_name.clone(),
            kind: "function".into(),
            type_node: None,
            is_const: true,
        });
        let new_sym = Symbol {
            name: alias_name.clone(),
            kind: sym.kind.clone(),
            type_node: sym.type_node.clone(),
            is_const: sym.is_const,
        };
        let _ = self.define_user_symbol(&alias_name, new_sym, Some(&node.span));
    }

    // ------------------------------------------------------------------
    // Spawn
    // ------------------------------------------------------------------

    fn visit_spawn_expr(&mut self, node: &Spawn) {
        self.visit_call(&node.call);
        if let Some(resume_session) = &node.resume_session {
            self.visit_expr(resume_session);
        }
    }

    // ------------------------------------------------------------------
    // Try / catch / finally
    // ------------------------------------------------------------------

    fn visit_try_stmt(&mut self, node: &TryStmt) {
        self.symbols.enter_scope("try", "block");
        self.visit_stmts(&node.body);
        self.symbols.exit_scope();

        for clause in &node.catch_clauses {
            self.visit_catch_clause(clause);
        }

        // Catch-all must be after all typed catches (parser guarantees order).
        if node.catch_all.is_some() && !node.catch_clauses.is_empty() {
            // In v1 we trust the parser; future: detect catch before catch_all.
        }

        if let Some(catch_all) = &node.catch_all {
            self.visit_catch_all(catch_all);
        }

        if let Some(finally_block) = &node.finally_block {
            self.visit_finally_block(finally_block);
        }
    }

    fn visit_throw_stmt(&mut self, node: &ThrowStmt) {
        let type_name = node.exception_type.name.clone();
        let is_predefined = PREDEFINED_EXCEPTIONS.contains(&type_name.as_str());
        let matched = if is_predefined {
            true
        } else {
            let lower = type_name.to_lowercase();
            PREDEFINED_EXCEPTIONS
                .iter()
                .any(|t| t.to_lowercase() == lower)
        };
        if !matched {
            self.errors.error(
                ErrorCode::InvalidCatchType,
                format!("'{type_name}' is not a predefined exception type"),
                node.exception_type.span.clone(),
            );
        }

        if let Some(message) = &node.message {
            self.visit_expr(message);
        }
    }

    fn visit_assert_stmt(&mut self, node: &AssertStmt) {
        self.visit_expr(&node.condition);
        if let Some(message) = &node.message {
            self.visit_expr(message);
        }
    }

    fn visit_catch_clause(&mut self, node: &CatchClauseNode) {
        let error_type_name = node.error_type.name.to_lowercase();
        let type_name_pascal = node.error_type.name.clone();
        let is_predefined = PREDEFINED_EXCEPTIONS.contains(&type_name_pascal.as_str());
        let matched = if is_predefined {
            true
        } else {
            PREDEFINED_EXCEPTIONS
                .iter()
                .any(|t| t.to_lowercase() == error_type_name)
        };
        if !matched {
            self.errors.error(
                ErrorCode::InvalidCatchType,
                format!("'{type_name_pascal}' is not a predefined exception type"),
                node.error_type.span.clone(),
            );
        }

        // Enter scope for catch clause and bind error name.
        self.symbols.enter_scope("catch", "block");
        let sym = Symbol::new(&node.error_name, "variable");
        self.define_user_symbol(&node.error_name, sym, Some(&node.span));
        self.visit_stmts(&node.body);
        self.symbols.exit_scope();
    }

    fn visit_catch_all(&mut self, node: &CatchAllNode) {
        self.symbols.enter_scope("catch-all", "block");
        self.visit_stmts(&node.body);
        self.symbols.exit_scope();
    }

    fn visit_finally_block(&mut self, node: &FinallyBlockNode) {
        self.visit_stmts(&node.body);
    }

    // ------------------------------------------------------------------
    // LLM statements
    // ------------------------------------------------------------------

    fn visit_llm_if_stmt(&mut self, node: &LlmIfStmtNode) {
        // Analyze description expression (always an expression in the AST).
        self.visit_expr(&node.description);
        let mut has_default = false;
        for branch in &node.branches {
            self.visit_llm_branch(branch);
            if branch.condition.is_none() {
                has_default = true;
            }
        }
        self.check_branch_completeness(has_default, &node.span, "llm_if");
    }

    fn visit_llm_branch(&mut self, node: &LlmBranchNode) {
        if let Some(condition) = &node.condition {
            self.visit_expr(condition);
        }
        self.symbols.enter_scope("llm-branch", "block");
        self.visit_stmts(&node.body);
        self.symbols.exit_scope();
    }

    fn visit_llm_act_expr(&mut self, node: &LlmAct) {
        if let Some(prompt) = &node.prompt {
            self.visit_expr(prompt);
        }
        for media_expr in &node.media {
            self.visit_expr(media_expr);
        }
        if let Some(on_chunk) = &node.on_chunk {
            self.visit_expr(on_chunk);
        }
        if let Some(on_complete) = &node.on_complete {
            self.visit_expr(on_complete);
        }
        if let Some(on_tool_end) = &node.on_tool_end {
            self.visit_expr(on_tool_end);
        }
        if let Some(on_media) = &node.on_media {
            self.visit_expr(on_media);
        }
        for gen_expr in &node.on_generate {
            self.visit_expr(gen_expr);
        }
        if let Some(provider) = &node.provider {
            self.visit_expr(provider);
        }
    }

    // ------------------------------------------------------------------
    // Match
    // ------------------------------------------------------------------

    fn visit_match_stmt(&mut self, node: &MatchStmtNode) {
        self.visit_expr(&node.subject);
        for case in &node.cases {
            self.visit_case(case);
        }
        self.check_match_completeness(node);
    }

    fn visit_match_expr(&mut self, node: &MatchExprNode) {
        self.visit_expr(&node.subject);
        for case in &node.cases {
            self.visit_case(case);
        }
        if let Some(default_body) = &node.default_body {
            self.visit_expr(default_body);
        }
        self.check_match_expr_completeness(node);
    }

    fn visit_case(&mut self, node: &CaseNode) {
        self.visit_expr(&node.pattern);
        self.symbols.enter_scope("match-case", "block");
        // Define variables from variable binding patterns BEFORE guard eval.
        match &node.pattern {
            Expr::VariablePattern(VariablePattern { name, span }) => {
                let sym = Symbol::new(name, "variable");
                self.define_user_symbol(name, sym, Some(span));
            }
            Expr::TypePattern(TypePattern {
                binding_name: Some(binding_name),
                span,
                ..
            }) => {
                let sym = Symbol::new(binding_name, "variable");
                self.define_user_symbol(binding_name, sym, Some(span));
            }
            _ => {}
        }
        if let Some(guard) = &node.guard {
            self.visit_expr(guard);
        }
        self.visit_stmts(&node.body);
        self.symbols.exit_scope();
    }
}

// ---------------------------------------------------------------------------
// Free helpers
// ---------------------------------------------------------------------------

/// Span of any statement (for diagnostics).
pub fn stmt_span(stmt: &Stmt) -> SourceSpan {
    match stmt {
        Stmt::VarDecl(n) => n.span.clone(),
        Stmt::SharedStoreDecl(n) => n.span.clone(),
        Stmt::If(n) => n.span.clone(),
        Stmt::For(n) => n.span.clone(),
        Stmt::While(n) => n.span.clone(),
        Stmt::Break(n) => n.span.clone(),
        Stmt::Continue(n) => n.span.clone(),
        Stmt::Return(n) => n.span.clone(),
        Stmt::Expr(n) => n.span.clone(),
        Stmt::PromptDef(n) => n.span.clone(),
        Stmt::Declaration(n) => n.span.clone(),
        Stmt::AgentParam(n) => n.span.clone(),
        Stmt::ContextConfig(n) => n
            .span
            .clone()
            .unwrap_or_else(|| SourceSpan::new("<unknown>", 1, 1, 1, 1)),
        Stmt::AgentDecl(n) => n.span.clone(),
        Stmt::MainBlock(n) => n.span.clone(),
        Stmt::FunctionDecl(n) => n.span.clone(),
        Stmt::FnBlock(n) => n.span.clone(),
        Stmt::ProtocolDecl(n) => n.span.clone(),
        Stmt::ImplDecl(n) => n.span.clone(),
        Stmt::Import(n) => n.span.clone(),
        Stmt::Alias(n) => n.span.clone(),
        Stmt::Case(n) => n.span.clone(),
        Stmt::CatchClause(n) => n.span.clone(),
        Stmt::CatchAll(n) => n.span.clone(),
        Stmt::FinallyBlock(n) => n.span.clone(),
        Stmt::Try(n) => n.span.clone(),
        Stmt::Throw(n) => n.span.clone(),
        Stmt::Assert(n) => n.span.clone(),
        Stmt::LlmBranch(n) => n.span.clone(),
        Stmt::LlmIf(n) => n.span.clone(),
        Stmt::Match(n) => n.span.clone(),
    }
}

/// Span of any expression (for diagnostics).
pub fn expr_span(expr: &Expr) -> SourceSpan {
    match expr {
        Expr::Literal(Lit { span, .. }) => span.clone(),
        Expr::Variable(Variable { span, .. }) => span.clone(),
        Expr::Binary(Binary { span, .. }) => span.clone(),
        Expr::Pipe(Pipe { span, .. }) => span.clone(),
        Expr::Unary(Unary { span, .. }) => span.clone(),
        Expr::Grouping(Grouping { span, .. }) => span.clone(),
        Expr::Call(Call { span, .. }) => span.clone(),
        Expr::Index(Index { span, .. }) => span.clone(),
        Expr::Access(Access { span, .. }) => span.clone(),
        Expr::List(ListLit { span, .. }) => span.clone(),
        Expr::Map(MapLit { span, .. }) => span.clone(),
        Expr::TemplateRef(TemplateRef { span, .. }) => span.clone(),
        Expr::Type(TypeRef { span, .. }) => span.clone(),
        Expr::OptionalType(OptionalType { span, .. }) => span.clone(),
        Expr::UnionType(UnionType { span, .. }) => span.clone(),
        Expr::LiteralType(LiteralType { span, .. }) => span.clone(),
        Expr::Lambda(Lambda { span, .. }) => span.clone(),
        Expr::Spawn(Spawn { span, .. }) => span.clone(),
        Expr::LlmAct(LlmAct { span, .. }) => span.clone(),
        Expr::MatchExpr(MatchExprNode { span, .. }) => span.clone(),
        Expr::RangePattern(RangePattern { span, .. }) => span.clone(),
        Expr::WildcardPattern(WildcardPattern { span }) => span.clone(),
        Expr::VariablePattern(VariablePattern { span, .. }) => span.clone(),
        Expr::TypePattern(TypePattern { span, .. }) => span.clone(),
    }
}

/// Python `is_helen_data_file` — data file extensions.
fn is_helen_data_file(path: &str) -> bool {
    HELEN_DATA_EXTS.iter().any(|ext| path.ends_with(ext))
}

/// Adapter so `FnBlock` bodies can drive the analyzer without exposing the
/// visitor trait (mirrors Python `accept`).
pub trait AcceptIntoAnalyzer {
    fn accept_into(&self, analyzer: &mut SemanticAnalyzer);
}

impl AcceptIntoAnalyzer for FnBlock {
    fn accept_into(&self, analyzer: &mut SemanticAnalyzer) {
        analyzer.visit_stmts(&self.body);
    }
}

/// Convenience: analyze a program, returning the E-codes in emission order.
pub fn analyze_codes(program: &Program) -> Vec<String> {
    let reporter = ErrorReporter::new();
    let mut analyzer = SemanticAnalyzer::new(reporter, ".");
    analyzer.analyze(program);
    analyzer
        .errors
        .errors()
        .iter()
        .map(|d| format!("E{:04}", d.code.value()))
        .collect()
}

/// Full diagnostic strings in Python's `Diagnostic.__str__` format:
/// `E{code:04d} at {loc}: {message}` where `loc` is the span or `<unknown>`.
/// Used by `--run` mode (stderr is later span-normalized, mirroring the
/// reference CLI).
pub fn analyze_messages(program: &Program) -> Vec<String> {
    let reporter = ErrorReporter::new();
    let mut analyzer = SemanticAnalyzer::new(reporter, ".");
    analyzer.analyze(program);
    analyzer
        .errors
        .errors()
        .iter()
        .map(|d| match &d.span {
            Some(sp) => format!("E{:04} at {}: {}", d.code.value(), sp, d.message),
            None => format!("E{:04} at <unknown>: {}", d.code.value(), d.message),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use helen_core::lexer::Scanner;

    /// Parse + analyze source, returning E-codes in emission order.
    fn codes(src: &str) -> Vec<String> {
        let mut scanner = Scanner::new(src, "<test>");
        let tokens = scanner.scan_all();
        let mut parser = helen_parser::Parser::new(tokens);
        let program = parser.parse();
        analyze_codes(&program)
    }

    #[test]
    fn duplicate_function_emits_duplicate_symbol() {
        let c = codes(
            "fn foo() {\n    print(\"a\")\n}\n\
             fn foo() {\n    print(\"b\")\n}\n\
             main {\n    foo()\n}\n",
        );
        assert!(c.contains(&"E0333".to_string()), "codes: {c:?}");
    }

    #[test]
    fn duplicate_agent_emits_duplicate_agent_name() {
        let c = codes(
            "agent Worker(name: str) {\n    description \"w\"\n    prompt \"Hi {{name}}\"\n    main { print(name) }\n}\n\
             agent Worker(name: str) {\n    description \"w2\"\n    prompt \"Hi {{name}}\"\n    main { print(name) }\n}\n\
             main {\n    print(\"ok\")\n}\n",
        );
        assert!(c.contains(&"E0335".to_string()), "codes: {c:?}");
    }

    #[test]
    fn impl_method_return_is_inside_function() {
        // E0340 (RETURN_OUTSIDE_FUNCTION) must NOT fire for impl method bodies.
        let c = codes(
            "import std.core.*\n\
             protocol Shape {\n    fn area(): float\n}\n\
             impl Shape for Circle {\n    fn area(): float {\n        return 3.14\n    }\n}\n\
             main {\n    print(\"ok\")\n}\n",
        );
        assert!(!c.contains(&"E0340".to_string()), "codes: {c:?}");
    }

    #[test]
    fn catch_invalid_type_emits_invalid_catch_type() {
        let c = codes(
            "import std.core.*\n\
             main {\n    try {\n        throw RuntimeError(\"boom\")\n    } catch CustomError e {\n        print(e)\n    }\n}\n",
        );
        assert!(c.contains(&"E0342".to_string()), "codes: {c:?}");
    }

    #[test]
    fn catch_valid_native_emits_no_catch_error() {
        let c = codes(
            "import std.core.*\n\
             main {\n    try {\n        throw RuntimeError(\"boom\")\n    } catch RuntimeError e {\n        print(e)\n    }\n}\n",
        );
        assert!(!c.contains(&"E0342".to_string()), "codes: {c:?}");
    }

    #[test]
    fn break_outside_loop_emits_break_error() {
        let c = codes("import std.core.*\nmain {\n    break\n}\n");
        assert!(c.contains(&"E0338".to_string()), "codes: {c:?}");
    }

    #[test]
    fn return_outside_function_emits_return_error() {
        // v1.12 (Issue #26): `return` inside main {} is legal. A top-level
        // return is the only reachable trigger. NOTE: with the `<test>`
        // filename the analyzer skips TOP_LEVEL_STATEMENT (E0355) — the
        // same skip list as Python — so only E0340 fires here. The E0355
        // pairing is covered by the fixture (real path) differential.
        let c = codes("return 42\n");
        assert!(c.contains(&"E0340".to_string()), "codes: {c:?}");
        assert!(!c.contains(&"E0355".to_string()), "codes: {c:?}");
    }

    #[test]
    fn const_assignment_emits_const_error() {
        let c = codes("main {\n    const MAX = 100\n    MAX = 5\n    print(MAX)\n}\n");
        assert!(c.contains(&"E0346".to_string()), "codes: {c:?}");
    }

    #[test]
    fn match_without_default_emits_match_error() {
        let c = codes(
            "import std.core.*\n\
             main {\n    match 3 {\n        case 1 { print(\"one\") }\n        case 2 { print(\"two\") }\n    }\n}\n",
        );
        assert!(c.contains(&"E0345".to_string()), "codes: {c:?}");
    }
}
