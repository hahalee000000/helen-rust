//! AST serializer producing **byte-identical** S-expressions to Python's
//! `helen.core.ast.ASTPrinter` (v1.44.0).
//!
//! Replicates every quirk of the Python implementation:
//! - `_parenthesize` flattening semantics (None → `<none>`, lists flattened)
//! - `visit_map_literal` prints **dataclass reprs** of `MapEntryNode`
//!   (they are not `ASTNode`s, so `str()` is used — including spans)
//! - `visit_call` joins argument *values* with spaces (dropping names)
//! - `visit_access` formats `(target . property)`
//! - `visit_llm_act_expr` omits `on_tool_end` (Python printer quirk)
//! - `visit_try_stmt` prints only the body (not catches/finally)
//! - `visit_import_stmt` prints `module_path` even when empty (`(import )`)
//! - Python `str()` for floats (shortest round-trip + Py3 formatting rules)
//! - Python `repr()` for strings (`'...'` with escapes) and dataclass reprs

use crate::ast::*;
use crate::source::SourceSpan;
use crate::tokens::{LiteralValue, Token};

// ---------------------------------------------------------------------------
// AstPrinter
// ---------------------------------------------------------------------------

/// Serialize an AST to S-expressions, matching Python's `ASTPrinter` output.
pub struct AstPrinter;

impl AstPrinter {
    pub fn new() -> Self {
        AstPrinter
    }

    /// `print(ProgramNode)` → `(program stmt*)`.
    pub fn print_program(&self, node: &Program) -> String {
        let items: Vec<Part> = node.statements.iter().map(|s| self.text_stmt(s)).collect();
        self.parenthesize("program", &items)
    }

    /// `print(StatementNode)`.
    pub fn print_stmt(&self, s: &Stmt) -> String {
        self.print_node(NodeRef::Stmt(s))
    }

    /// `print(ExpressionNode)`.
    pub fn print_expr(&self, e: &Expr) -> String {
        self.print_expr_only(e)
    }

    /// Universal dispatch: statement or expression.
    fn print_node(&self, node: NodeRef) -> String {
        match node {
            NodeRef::Stmt(s) => match s {
                Stmt::VarDecl(n) => self.visit_var_decl(n),
                Stmt::SharedStoreDecl(n) => self.visit_shared_store_decl(n),
                Stmt::If(n) => self.visit_if_stmt(n),
                Stmt::For(n) => self.visit_for_stmt(n),
                Stmt::While(n) => self.visit_while_stmt(n),
                Stmt::Break(_) => "(break)".to_string(),
                Stmt::Continue(_) => "(continue)".to_string(),
                Stmt::Return(n) => self.visit_return_stmt(n),
                Stmt::Expr(n) => self.visit_expr_stmt(n),
                Stmt::PromptDef(n) => self.visit_prompt_def(n),
                Stmt::Declaration(_) => "(declaration)".to_string(),
                Stmt::AgentParam(n) => self.visit_agent_param(n),
                Stmt::ContextConfig(n) => self.visit_context_config(n),
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
                Stmt::CatchAll(_) => "(catch-all)".to_string(),
                Stmt::FinallyBlock(n) => self.visit_finally_block(n),
                Stmt::Try(n) => self.visit_try_stmt(n),
                Stmt::Throw(n) => self.visit_throw_stmt(n),
                Stmt::Assert(n) => self.visit_assert_stmt(n),
                Stmt::LlmBranch(n) => self.visit_llm_branch(n),
                Stmt::LlmIf(n) => self.visit_llm_if_stmt(n),
                Stmt::Match(n) => self.visit_match_stmt(n),
            },
            NodeRef::Expr(e) => self.print_expr_only(e),
        }
    }

    /// Print an expression node.
    fn print_expr_only(&self, e: &Expr) -> String {
        match e {
            Expr::Literal(n) => self.visit_literal(n),
            Expr::Variable(n) => n.name.clone(),
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
            Expr::Type(n) => n.name.clone(),
            Expr::OptionalType(n) => self.visit_optional_type(n),
            Expr::UnionType(n) => self.visit_union_type(n),
            Expr::LiteralType(n) => self.visit_literal_type(n),
            Expr::Lambda(n) => self.visit_lambda(n),
            Expr::Spawn(n) => self.visit_spawn_expr(n),
            Expr::LlmAct(n) => self.visit_llm_act_expr(n),
            Expr::MatchExpr(n) => self.visit_match_expr(n),
            Expr::RangePattern(n) => self.visit_range_pattern(n),
            Expr::WildcardPattern(_) => "_".to_string(),
            Expr::VariablePattern(n) => self.visit_variable_pattern(n),
            Expr::TypePattern(n) => self.visit_type_pattern(n),
        }
    }

    // -- _parenthesize ------------------------------------------------------

    /// Render a node to a `Part::Text` (Python `part.accept(self)`).
    fn text_stmt(&self, s: &Stmt) -> Part {
        Part::Text(self.print_node(NodeRef::Stmt(s)))
    }

    /// Render an expression to a `Part::Text`.
    fn text_expr(&self, e: &Expr) -> Part {
        Part::Text(self.print_expr_only(e))
    }

    /// Python `_parenthesize(name, *parts)` — parts are pre-rendered strings.
    fn parenthesize(&self, name: &str, parts: &[Part]) -> String {
        let mut result: Vec<String> = vec![name.to_string()];
        for p in parts {
            match p {
                Part::None => result.push("<none>".to_string()),
                Part::Text(t) => result.push(t.clone()),
            }
        }
        format!("({})", result.join(" "))
    }

    // -- visit_* methods ----------------------------------------------------

    fn visit_literal(&self, node: &Lit) -> String {
        match &node.value {
            LiteralValue::Null => "null".to_string(),
            LiteralValue::Bool(b) => {
                if *b {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            }
            LiteralValue::Str(s) => format!("\"{}\"", s),
            LiteralValue::Int(i) => i.to_string(),
            LiteralValue::Float(f) => py_str_float(*f),
        }
    }

    fn visit_binary_op(&self, node: &Binary) -> String {
        self.parenthesize(
            &node.operator.lexeme,
            &[self.text_expr(&node.left), self.text_expr(&node.right)],
        )
    }

    fn visit_pipe_expr(&self, node: &Pipe) -> String {
        self.parenthesize(
            "|>",
            &[self.text_expr(&node.value), self.text_expr(&node.function)],
        )
    }

    fn visit_unary_op(&self, node: &Unary) -> String {
        self.parenthesize(&node.operator.lexeme, &[self.text_expr(&node.operand)])
    }

    fn visit_grouping(&self, node: &Grouping) -> String {
        self.parenthesize("group", &[self.text_expr(&node.expression)])
    }

    fn visit_call(&self, node: &Call) -> String {
        let args: Vec<String> = node
            .arguments
            .iter()
            .map(|a| self.print_expr_only(&a.value))
            .collect();
        let args_str = args.join(" ");
        self.parenthesize(
            "call",
            &[
                Part::Text(self.print_expr_only(&node.callee)),
                Part::Text(args_str),
            ],
        )
    }

    fn visit_index(&self, node: &Index) -> String {
        self.parenthesize(
            "index",
            &[self.text_expr(&node.target), self.text_expr(&node.index)],
        )
    }

    fn visit_access(&self, node: &Access) -> String {
        format!(
            "({} . {})",
            self.print_expr_only(&node.target),
            node.property
        )
    }

    fn visit_var_decl(&self, node: &VarDecl) -> String {
        let kw = if node.mutable { "let" } else { "const" };
        let mut parts: Vec<Part> = vec![Part::Text(node.name.clone())];
        if let Some(init) = &node.initializer {
            parts.push(Part::Text("=".to_string()));
            parts.push(self.text_expr(init));
        }
        self.parenthesize(kw, &parts)
    }

    fn visit_shared_store_decl(&self, node: &SharedStoreDecl) -> String {
        let mut parts: Vec<Part> = vec![Part::Text(node.name.clone())];
        for f in &node.fields {
            parts.push(self.text_stmt(&Stmt::VarDecl(f.clone())));
        }
        for m in &node.methods {
            parts.push(self.text_stmt(&Stmt::FunctionDecl(m.clone())));
        }
        self.parenthesize("shared store", &parts)
    }

    fn visit_if_stmt(&self, node: &IfStmt) -> String {
        let mut parts: Vec<Part> = vec![
            self.text_expr(&node.condition),
            self.text_stmt(&node.then_branch),
        ];
        if let Some(els) = &node.else_branch {
            parts.push(self.text_stmt(els));
        }
        self.parenthesize("if", &parts)
    }

    fn visit_for_stmt(&self, node: &ForStmt) -> String {
        let iter_part = match &node.iterator {
            Some(v) => Part::Text(v.name.clone()),
            None => Part::None,
        };
        self.parenthesize(
            "for",
            &[
                iter_part,
                self.text_expr(&node.iterable),
                self.text_stmt(&node.body),
            ],
        )
    }

    fn visit_while_stmt(&self, node: &WhileStmt) -> String {
        self.parenthesize(
            "while",
            &[self.text_expr(&node.condition), self.text_stmt(&node.body)],
        )
    }

    fn visit_return_stmt(&self, node: &ReturnStmt) -> String {
        match &node.value {
            Some(v) => self.parenthesize("return", &[self.text_expr(v)]),
            None => "(return)".to_string(),
        }
    }

    fn visit_expr_stmt(&self, node: &ExprStmt) -> String {
        self.print_expr_only(&node.expression)
    }

    fn visit_agent_decl(&self, node: &AgentDecl) -> String {
        let mut parts: Vec<Part> = vec![Part::Text(node.name.clone())];
        if !node.params.is_empty() {
            let items: Vec<Part> = node
                .params
                .iter()
                .map(|p| self.text_stmt(&Stmt::AgentParam(p.clone())))
                .collect();
            parts.extend(items);
        }
        if let Some(p) = &node.prompt {
            parts.push(self.text_stmt(&Stmt::PromptDef(p.clone())));
        }
        if let Some(cc) = &node.context_config {
            parts.push(self.text_stmt(&Stmt::ContextConfig(cc.clone())));
        }
        self.parenthesize("agent", &parts)
    }

    fn visit_context_config(&self, node: &ContextConfig) -> String {
        self.parenthesize(
            "context-config",
            &[
                Part::Text(format!("compression={}", node.compression)),
                Part::Text(format!("cache_aware={}", node.cache_aware)),
                Part::Text(format!("working_memory={}", node.working_memory)),
                Part::Text(format!("tokens={}", node.working_memory_tokens)),
            ],
        )
    }

    fn visit_prompt_def(&self, node: &PromptDef) -> String {
        format!("\"{}\"", node.content)
    }

    fn visit_main_block(&self, node: &MainBlock) -> String {
        let items: Vec<Part> = node.body.iter().map(|s| self.text_stmt(s)).collect();
        self.parenthesize("main-block", &items)
    }

    fn visit_function_decl(&self, node: &FunctionDecl) -> String {
        self.parenthesize("fn", &[Part::Text(node.name.clone())])
    }

    fn visit_lambda(&self, _node: &Lambda) -> String {
        "(lambda)".to_string()
    }

    fn visit_protocol_decl(&self, node: &ProtocolDecl) -> String {
        self.parenthesize("protocol", &[Part::Text(node.name.clone())])
    }

    fn visit_impl_decl(&self, node: &ImplDecl) -> String {
        self.parenthesize(
            "impl",
            &[
                Part::Text(node.protocol_name.clone()),
                Part::Text(node.struct_name.clone()),
            ],
        )
    }

    fn visit_import_stmt(&self, node: &ImportStmt) -> String {
        self.parenthesize("import", &[Part::Text(node.module_path.clone())])
    }

    fn visit_alias_stmt(&self, node: &AliasStmt) -> String {
        self.parenthesize(
            "alias",
            &[
                Part::Text(node.canonical.clone()),
                Part::Text(node.alias_name.clone()),
            ],
        )
    }

    fn visit_agent_param(&self, node: &AgentParam) -> String {
        self.parenthesize("param", &[Part::Text(node.name.clone())])
    }

    fn visit_spawn_expr(&self, node: &Spawn) -> String {
        let call_expr = Expr::Call(*node.call.clone());
        let s = self.parenthesize("spawn", &[self.text_expr(&call_expr)]);
        if let Some(rs) = &node.resume_session {
            format!(
                "{} {}",
                s,
                self.parenthesize("resume", &[self.text_expr(rs)])
            )
        } else {
            s
        }
    }

    fn visit_case(&self, node: &CaseNode) -> String {
        self.parenthesize("case", &[self.text_expr(&node.pattern)])
    }

    fn visit_range_pattern(&self, node: &RangePattern) -> String {
        self.parenthesize(
            "range",
            &[self.text_expr(&node.start), self.text_expr(&node.end)],
        )
    }

    fn visit_variable_pattern(&self, node: &VariablePattern) -> String {
        self.parenthesize("var", &[Part::Text(node.name.clone())])
    }

    fn visit_type_pattern(&self, node: &TypePattern) -> String {
        match &node.binding_name {
            Some(b) => self.parenthesize(
                "is",
                &[Part::Text(node.type_name.clone()), Part::Text(b.clone())],
            ),
            None => self.parenthesize("is", &[Part::Text(node.type_name.clone())]),
        }
    }

    fn visit_catch_clause(&self, node: &CatchClauseNode) -> String {
        let type_expr = Expr::Type(node.error_type.clone());
        self.parenthesize("catch", &[self.text_expr(&type_expr)])
    }

    fn visit_finally_block(&self, node: &FinallyBlockNode) -> String {
        let items: Vec<Part> = node.body.iter().map(|s| self.text_stmt(s)).collect();
        self.parenthesize("finally", &items)
    }

    fn visit_try_stmt(&self, node: &TryStmt) -> String {
        let items: Vec<Part> = node.body.iter().map(|s| self.text_stmt(s)).collect();
        self.parenthesize("try", &items)
    }

    fn visit_throw_stmt(&self, node: &ThrowStmt) -> String {
        let mut parts: Vec<Part> = vec![self.text_expr(&Expr::Type(node.exception_type.clone()))];
        if let Some(m) = &node.message {
            parts.push(self.text_expr(m));
        }
        self.parenthesize("throw", &parts)
    }

    fn visit_assert_stmt(&self, node: &AssertStmt) -> String {
        let mut parts: Vec<Part> = vec![self.text_expr(&node.condition)];
        if let Some(m) = &node.message {
            parts.push(self.text_expr(m));
        }
        self.parenthesize("assert", &parts)
    }

    fn visit_llm_branch(&self, node: &LlmBranchNode) -> String {
        let items: Vec<Part> = node.body.iter().map(|s| self.text_stmt(s)).collect();
        self.parenthesize("branch", &items)
    }

    fn visit_llm_if_stmt(&self, node: &LlmIfStmtNode) -> String {
        let mut parts: Vec<Part> = vec![self.text_expr(&node.description)];
        for b in &node.branches {
            parts.push(self.text_stmt(&Stmt::LlmBranch(b.clone())));
        }
        self.parenthesize("llm-if", &parts)
    }

    fn visit_llm_act_expr(&self, node: &LlmAct) -> String {
        let mut parts: Vec<Part> = Vec::new();
        match &node.prompt {
            Some(p) => parts.push(self.text_expr(p)),
            None => parts.push(Part::None),
        }
        for m in &node.media {
            parts.push(self.text_expr(m));
        }
        if let Some(c) = &node.on_chunk {
            parts.push(self.text_expr(c));
        }
        if let Some(c) = &node.on_complete {
            parts.push(self.text_expr(c));
        }
        if let Some(c) = &node.on_media {
            parts.push(self.text_expr(c));
        }
        for g in &node.on_generate {
            parts.push(self.text_expr(g));
        }
        if let Some(p) = &node.provider {
            parts.push(self.text_expr(p));
        }
        self.parenthesize("llm-act", &parts)
    }

    fn visit_match_stmt(&self, node: &MatchStmtNode) -> String {
        self.parenthesize("match", &[self.text_expr(&node.subject)])
    }

    fn visit_match_expr(&self, node: &MatchExprNode) -> String {
        self.parenthesize("match-expr", &[self.text_expr(&node.subject)])
    }

    fn visit_optional_type(&self, node: &OptionalType) -> String {
        let inner = Expr::Type(*node.inner.clone());
        self.parenthesize("optional", &[self.text_expr(&inner)])
    }

    fn visit_union_type(&self, node: &UnionType) -> String {
        let items: Vec<Part> = node
            .members
            .iter()
            .map(|m| self.text_expr(&Expr::Type(m.clone())))
            .collect();
        self.parenthesize("union", &items)
    }

    fn visit_literal_type(&self, node: &LiteralType) -> String {
        let items: Vec<Part> = node.values.iter().map(|v| self.text_expr(v)).collect();
        self.parenthesize("literal-type", &items)
    }

    fn visit_list_literal(&self, node: &ListLit) -> String {
        let items: Vec<Part> = node.elements.iter().map(|e| self.text_expr(e)).collect();
        self.parenthesize("list", &items)
    }

    fn visit_map_literal(&self, node: &MapLit) -> String {
        // Python quirk: MapEntryNode is NOT an ASTNode, so `str(item)` is used
        // — the dataclass repr (including spans). Replicate byte-for-byte.
        let items: Vec<Part> = node
            .entries
            .iter()
            .map(|e| Part::Text(py_repr_map_entry(e)))
            .collect();
        self.parenthesize("map", &items)
    }

    fn visit_template_ref(&self, node: &TemplateRef) -> String {
        self.parenthesize("template-ref", &[self.text_expr(&node.expression)])
    }

    fn visit_fn_block(&self, node: &FnBlock) -> String {
        let items: Vec<Part> = node.body.iter().map(|s| self.text_stmt(s)).collect();
        self.parenthesize("fn-block", &items)
    }
}

impl Default for AstPrinter {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Part — a `_parenthesize` argument (pre-rendered)
// ---------------------------------------------------------------------------

enum Part {
    None,
    Text(String),
}

enum NodeRef<'a> {
    Stmt(&'a Stmt),
    #[allow(dead_code)]
    Expr(&'a Expr),
}

// ---------------------------------------------------------------------------
// Python value reprs (used inside dataclass reprs)
// ---------------------------------------------------------------------------

/// Python `str(float)` — shortest round-trip with Py3 formatting.
///
/// Uses ryu for the shortest digits, then reformats to Python's rules:
/// scientific when the decimal exponent is outside `[-4, 16)`, fixed
/// otherwise, exponent with sign and ≥2 digits (`1e+20`, `1.5e-05`).
pub fn py_str_float(v: f64) -> String {
    if v == 0.0 {
        return if v.is_sign_negative() {
            "-0.0".into()
        } else {
            "0.0".into()
        };
    }
    if v.is_nan() {
        return "nan".into();
    }
    if v.is_infinite() {
        return if v < 0.0 { "-inf".into() } else { "inf".into() };
    }
    let neg = v < 0.0;
    let av = v.abs();
    // ryu gives the shortest round-trip representation (Rust-style).
    let mut buf = ryu::Buffer::new();
    let s = buf.format(av);
    // Parse ryu output into (digits, kk) where value = 0.digits × 10^kk.
    let (digits, kk) = parse_shortest(s);
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    // Python scientific when kk <= -4 or kk >= 17.
    if kk <= -4 || kk >= 17 {
        out.push(digits.as_bytes()[0] as char);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        let exp = kk - 1; // exponent of d.ddd form
        out.push('e');
        if exp < 0 {
            out.push('-');
            out.push_str(&format!("{:02}", -exp));
        } else {
            out.push('+');
            out.push_str(&format!("{:02}", exp));
        }
    } else if kk <= 0 {
        out.push_str("0.");
        for _ in 0..(-kk) {
            out.push('0');
        }
        out.push_str(&digits);
    } else if (kk as usize) >= digits.len() {
        out.push_str(&digits);
        for _ in 0..(kk as usize - digits.len()) {
            out.push('0');
        }
        out.push_str(".0");
    } else {
        let kku = kk as usize;
        out.push_str(&digits[..kku]);
        out.push('.');
        out.push_str(&digits[kku..]);
    }
    out
}

/// Parse ryu's shortest output `s` into `(digits, kk)` where
/// `value = 0.digits × 10^kk` (Python dtoa convention: `kk` is the
/// position of the decimal point relative to the first digit, i.e.
/// `10^(kk-1) <= value < 10^kk`).
fn parse_shortest(s: &str) -> (String, i32) {
    // Scientific form: "1e20", "1.5e-5".
    if let Some(e_pos) = s.find('e') {
        let mant = &s[..e_pos];
        let exp: i32 = s[e_pos + 1..].parse().unwrap_or(0);
        let (digits, _kk) = mantissa_to_digits(mant);
        // value = 0.digits × 10^kk; scientific shows d.ddd × 10^(kk-1),
        // so the printed exponent is kk - 1. Thus kk = exp + 1.
        return (digits, exp + 1);
    }
    // Fixed form: "3.14", "0.001234", "100.0", "12340000000.0".
    let (int_part, frac_part) = match s.split_once('.') {
        Some((i, f)) => (i, f),
        None => (s, ""),
    };
    if int_part.chars().any(|c| c != '0') {
        let first = int_part.find(|c| c != '0').unwrap();
        let digits: String = int_part[first..].to_string() + frac_part;
        let kk = (int_part.len() - first) as i32;
        (digits, kk)
    } else {
        let first = frac_part.find(|c| c != '0').unwrap();
        let digits = frac_part[first..].to_string();
        // value = digits × 10^-(first+n) → 0.digits × 10^kk with kk = -first.
        let kk = -(first as i32);
        (digits, kk)
    }
}

/// Convert a mantissa ("1", "1.5") into (digits, kk) with kk ignored for
/// the scientific path (the caller recomputes it from the exponent).
fn mantissa_to_digits(mant: &str) -> (String, i32) {
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    (digits, 0)
}

/// Python `repr(str)` — single quotes with escape sequences.
pub fn py_str_repr(s: &str) -> String {
    let mut out = String::from("'");
    for c in s.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\x00'..='\x1f' => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            _ => out.push(c),
        }
    }
    out.push('\'');
    out
}

/// Python `repr(value)` for a literal value.
pub fn py_repr_value(v: &LiteralValue) -> String {
    match v {
        LiteralValue::Null => "None".to_string(),
        LiteralValue::Bool(b) => {
            if *b {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        LiteralValue::Str(s) => py_str_repr(s),
        LiteralValue::Int(i) => i.to_string(),
        LiteralValue::Float(f) => py_str_float(*f),
    }
}

/// Python `repr(SourceSpan)` — dataclass repr.
pub fn py_repr_span(sp: &SourceSpan) -> String {
    format!(
        "SourceSpan(file={}, start_line={}, start_col={}, end_line={}, end_col={})",
        py_str_repr(&sp.file),
        sp.start_line,
        sp.start_col,
        sp.end_line,
        sp.end_col
    )
}

/// Python `Token.__repr__` — `Token(type=PLUS, lexeme='+', literal=None, ...)`.
pub fn py_repr_token(t: &Token) -> String {
    format!(
        "Token(type={}, lexeme={}, literal={}, line={}, col={}, end_line={}, end_col={}, file={})",
        t.kind.name(),
        py_str_repr(&t.lexeme),
        py_repr_value(&t.literal),
        t.line,
        t.col,
        t.end_line,
        t.end_col,
        py_str_repr(&t.file)
    )
}

/// Python `repr(MapEntryNode)` — dataclass repr with full field reprs.
pub fn py_repr_map_entry(e: &MapEntry) -> String {
    format!(
        "MapEntryNode(key={}, value={}, span={})",
        py_repr_expr(&e.key),
        py_repr_expr(&e.value),
        py_repr_span(&e.span)
    )
}

/// Python dataclass `repr(expr_node)`.
pub fn py_repr_expr(e: &Expr) -> String {
    match e {
        Expr::Literal(n) => format!(
            "LiteralNode(value={}, span={})",
            py_repr_value(&n.value),
            py_repr_span(&n.span)
        ),
        Expr::Variable(n) => format!(
            "VariableNode(name={}, span={})",
            py_str_repr(&n.name),
            py_repr_span(&n.span)
        ),
        Expr::Binary(n) => format!(
            "BinaryOpNode(left={}, operator={}, right={}, span={})",
            py_repr_expr(&n.left),
            py_repr_token(&n.operator),
            py_repr_expr(&n.right),
            py_repr_span(&n.span)
        ),
        Expr::Pipe(n) => format!(
            "PipeExprNode(value={}, function={}, span={})",
            py_repr_expr(&n.value),
            py_repr_expr(&n.function),
            py_repr_span(&n.span)
        ),
        Expr::Unary(n) => format!(
            "UnaryOpNode(operator={}, operand={}, span={})",
            py_repr_token(&n.operator),
            py_repr_expr(&n.operand),
            py_repr_span(&n.span)
        ),
        Expr::Grouping(n) => format!(
            "GroupingNode(expression={}, span={})",
            py_repr_expr(&n.expression),
            py_repr_span(&n.span)
        ),
        Expr::Call(n) => {
            let args: Vec<String> = n.arguments.iter().map(py_repr_call_arg).collect();
            format!(
                "CallNode(callee={}, arguments=[{}], span={})",
                py_repr_expr(&n.callee),
                args.join(", "),
                py_repr_span(&n.span)
            )
        }
        Expr::Index(n) => format!(
            "IndexNode(target={}, index={}, span={})",
            py_repr_expr(&n.target),
            py_repr_expr(&n.index),
            py_repr_span(&n.span)
        ),
        Expr::Access(n) => format!(
            "AccessNode(target={}, property={}, span={})",
            py_repr_expr(&n.target),
            py_str_repr(&n.property),
            py_repr_span(&n.span)
        ),
        Expr::List(n) => {
            let elems: Vec<String> = n.elements.iter().map(py_repr_expr).collect();
            format!(
                "ListLiteralNode(elements=[{}], span={})",
                elems.join(", "),
                py_repr_span(&n.span)
            )
        }
        Expr::Map(n) => {
            let entries: Vec<String> = n.entries.iter().map(py_repr_map_entry).collect();
            format!(
                "MapLiteralNode(entries=[{}], span={})",
                entries.join(", "),
                py_repr_span(&n.span)
            )
        }
        Expr::TemplateRef(n) => format!(
            "TemplateRefNode(expression={}, span={})",
            py_repr_expr(&n.expression),
            py_repr_span(&n.span)
        ),
        Expr::Type(n) => format!(
            "TypeNode(name={}, span={})",
            py_str_repr(&n.name),
            py_repr_span(&n.span)
        ),
        Expr::OptionalType(n) => format!(
            "OptionalTypeNode(inner={}, span={})",
            py_repr_expr(&Expr::Type(*n.inner.clone())),
            py_repr_span(&n.span)
        ),
        Expr::UnionType(n) => {
            let members: Vec<String> = n
                .members
                .iter()
                .map(|m| py_repr_expr(&Expr::Type(m.clone())))
                .collect();
            format!(
                "UnionTypeNode(members=[{}], span={})",
                members.join(", "),
                py_repr_span(&n.span)
            )
        }
        Expr::LiteralType(n) => {
            let vals: Vec<String> = n.values.iter().map(py_repr_expr).collect();
            format!(
                "LiteralTypeNode(values=[{}], span={})",
                vals.join(", "),
                py_repr_span(&n.span)
            )
        }
        Expr::Lambda(n) => format!("LambdaNode(span={})", py_repr_span(&n.span)),
        Expr::Spawn(n) => format!(
            "SpawnExprNode(call={}, span={}, resume_session={})",
            py_repr_expr(&Expr::Call(*n.call.clone())),
            py_repr_span(&n.span),
            match &n.resume_session {
                Some(r) => py_repr_expr(r),
                None => "None".to_string(),
            }
        ),
        Expr::LlmAct(n) => format!("LlmActExprNode(span={})", py_repr_span(&n.span)),
        Expr::MatchExpr(n) => format!(
            "MatchExprNode(subject={}, span={})",
            py_repr_expr(&n.subject),
            py_repr_span(&n.span)
        ),
        Expr::RangePattern(n) => format!(
            "RangePatternNode(start={}, end={}, span={})",
            py_repr_expr(&n.start),
            py_repr_expr(&n.end),
            py_repr_span(&n.span)
        ),
        Expr::WildcardPattern(n) => format!("WildcardPatternNode(span={})", py_repr_span(&n.span)),
        Expr::VariablePattern(n) => format!(
            "VariablePatternNode(name={}, span={})",
            py_str_repr(&n.name),
            py_repr_span(&n.span)
        ),
        Expr::TypePattern(n) => format!(
            "TypePatternNode(type_name={}, span={}, binding_name={})",
            py_str_repr(&n.type_name),
            py_repr_span(&n.span),
            match &n.binding_name {
                Some(b) => py_str_repr(b),
                None => "None".to_string(),
            }
        ),
    }
}

/// Python `repr(CallArgNode)`.
pub fn py_repr_call_arg(a: &CallArg) -> String {
    format!(
        "CallArgNode(name={}, value={})",
        match &a.name {
            Some(n) => py_str_repr(n),
            None => "None".to_string(),
        },
        py_repr_expr(&a.value)
    )
}
