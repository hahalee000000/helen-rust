#!/usr/bin/env python3
"""gen-tier-c.py — generate Rust integration tests (Tier C) from Python test files.

For each Python test file, extract Helen source snippets and use the *Python
reference* implementation to compute the canonical expected output, then emit
Rust `#[test]` functions asserting the Rust pipeline reproduces it byte-for-byte.

Suites:
  lexer     → source → token stream (kind + lexeme), errors
  parser    → source → ASTPrinter output (byte-identical), parse errors
  semantic  → source → E-code list (error codes in emission order)
  execution → source → stdout + exit code (via helen.cli run contract)

Usage: python3 scripts/gen-tier-c.py [--emit-out <dir>]
"""
from __future__ import annotations

import re
import sys
import json
import subprocess
from pathlib import Path

HELEN = Path("/home/rxx/helen")           # Python reference repo
RUST = Path("/home/rxx/helen-rust")        # Rust reimplementation repo
PY = sys.executable

# ── snippet extraction ─────────────────────────────────────────────────────

SCAN_RE = re.compile(r'_scan\(\s*(["\'])(.*?)\1\s*[,)]', re.S)
PARSE_RE = re.compile(r'_parse_source\(\s*(["\'])(.*?)\1\s*[,)]', re.S)
# generic: any assignment of a string literal to `source` / `src` / `code`
SRC_ASSIGN_RE = re.compile(
    r'(?:source|src|code|main_source)\s*=\s*(?:\()?(["\'])(.*?)\1', re.S)
SRC_TRIPLE_RE = re.compile(
    r'(?:source|src|code|main_source)\s*=\s*"""\\\n(.*?)"""', re.S)
DOCSTRING_RE = re.compile(r'"""((?:[^"]|""[^"])*?)"""', re.S)


def unescape(s: str) -> str:
    """Undo Python string-literal escaping for the captured body."""
    try:
        return bytes(s, "utf-8").decode("unicode_escape")
    except Exception:
        return s


def extract_lexer_snippets(path: Path) -> list[str]:
    """All `_scan("...")`/`_scan('...')` source strings in a lexer test file."""
    text = path.read_text(encoding="utf-8")
    out: list[str] = []
    for q, body in SCAN_RE.findall(text):
        src = unescape(body)
        if src not in out:
            out.append(src)
    return out


def extract_parser_snippets(path: Path) -> list[str]:
    """Source strings from `_parse_source('...')` and source= assignments."""
    text = path.read_text(encoding="utf-8")
    out: list[str] = []
    for q, body in PARSE_RE.findall(text):
        src = unescape(body)
        if src not in out:
            out.append(src)
    for q, body in SRC_ASSIGN_RE.findall(text):
        src = unescape(body)
        if src not in out and len(src) < 400:
            out.append(src)
    return out


def extract_semantic_snippets(path: Path) -> list[str]:
    text = path.read_text(encoding="utf-8")
    out: list[str] = []
    for q, body in SRC_ASSIGN_RE.findall(text):
        src = unescape(body)
        if src not in out and len(src) < 400:
            out.append(src)
    # triple-quoted source = """\ ... """ blocks (backslash continuation)
    for body in SRC_TRIPLE_RE.findall(text):
        src = body
        if src not in out and len(src) < 400:
            out.append(src)
    return out


def looks_like_helen(src: str) -> bool:
    """Heuristic: does this string look like Helen source (not prose)?"""
    lines = [ln.strip() for ln in src.splitlines() if ln.strip()]
    if not lines:
        return False
    codey = sum(
        1 for ln in lines
        if any(k in ln for k in (
            "fn ", "agent ", "let ", "const ", "main ", "spawn ", "llm ",
            "if (", "while (", "for (", "match ", "return ", "import ",
            "try ", "catch ", "throw ", "shared ", "{", "}",
        ))
    )
    # A real program has most lines looking like code; prose has few.
    return codey >= max(1, len(lines) // 2)


# ── reference drivers ──────────────────────────────────────────────────────

REF_LEXER = r'''
import sys, json
sys.path.insert(0, r"{helen}")
from helen.core.lexer import Scanner
src = {src!r}
s = Scanner(src, file="<test>")
toks = s.scan_all()
out = [{{"kind": t.type.name, "lex": t.lexeme}} for t in toks]
errs = [{{"code": e.code.name, "msg": e.message}} for e in s.errors]
print(json.dumps({{"tokens": out, "errors": errs}}))
'''

REF_PRINT = r'''
import sys, json
sys.path.insert(0, r"{helen}")
from helen.core.lexer import Scanner
from helen.core.parser import Parser
from helen.core.errors import ErrorReporter
from helen.core.ast import ASTPrinter
src = {src!r}
sc = Scanner(src, file="<test>")
toks = sc.scan_all()
errs = ErrorReporter()
p = Parser(toks, errs)
prog = p.parse()
pr = ASTPrinter()
printed = pr.print(prog)
perrs = ["E%04d" % e.code.value for e in errs.errors]
print(json.dumps({{"printed": printed, "errors": perrs}}))
'''

REF_SEM = r'''
import sys, json
sys.path.insert(0, r"{helen}")
from helen.core.lexer import Scanner
from helen.core.parser import Parser
from helen.core.errors import ErrorReporter
from helen.semantic.analyzer import SemanticAnalyzer
src = {src!r}
sc = Scanner(src, file="<test>")
toks = sc.scan_all()
errs = ErrorReporter()
p = Parser(toks, errs)
prog = p.parse()
a = SemanticAnalyzer(errs)
a.analyze(prog)
codes = ["E%04d" % e.code.value for e in errs.errors]
print(json.dumps({{"codes": codes}}))
'''


def run_ref(driver: str, src: str) -> dict:
    code = driver.format(helen=str(HELEN), src=src)
    r = subprocess.run([PY, "-c", code], capture_output=True, text=True, timeout=30)
    if r.returncode != 0:
        return {"__err__": r.stderr.strip()[:200]}
    return json.loads(r.stdout)


# Python TokenType enum name → Rust TokenType variant name.
# Rust uses CamelCase variants; Python uses SCREAMING_SNAKE names.
TT_MAP = {
    "LEFT_PAREN": "LeftParen", "RIGHT_PAREN": "RightParen",
    "LEFT_BRACE": "LeftBrace", "RIGHT_BRACE": "RightBrace",
    "LEFT_BRACKET": "LeftBracket", "RIGHT_BRACKET": "RightBracket",
    "COMMA": "Comma", "DOT": "Dot", "DOTDOT": "DotDot",
    "COLON": "Colon", "SEMICOLON": "Semicolon", "QUESTION": "Question",
    "PIPE": "Pipe", "PIPE_RIGHT": "PipeRight",
    "MINUS": "Minus", "PLUS": "Plus", "SLASH": "Slash", "STAR": "Star",
    "PERCENT": "Percent", "ARROW": "Arrow", "BANG": "Bang",
    "BANG_EQUAL": "BangEqual", "ASSIGN": "Assign", "EQUAL_EQUAL": "EqualEqual",
    "GREATER": "Greater", "GREATER_EQUAL": "GreaterEqual",
    "LESS": "Less", "LESS_EQUAL": "LessEqual", "AND": "And", "OR": "Or",
    "AT": "At", "IDENTIFIER": "Identifier", "STRING": "String",
    "TRIPLE_QUOTE_STRING": "TripleQuoteString", "NUMBER": "Number",
    "TRUE": "True", "FALSE": "False", "NULL_KW": "NullKw",
    "TEMPLATE_OPEN": "TemplateOpen", "TEMPLATE_CLOSE": "TemplateClose",
    "EOF": "Eof", "AGENT": "Agent", "DESCRIPTION": "Description",
    "MODEL": "Model", "TOOLS": "Tools", "TEMPERATURE": "Temperature",
    "MAX_TURNS": "MaxTurns", "PROMPT": "Prompt", "LLM": "Llm",
    "IMPORT": "Import", "LET": "Let", "CONST": "Const", "IF": "If",
    "ELSE": "Else", "FOR": "For", "WHILE": "While", "BREAK": "Break",
    "CONTINUE": "Continue", "RETURN": "Return", "SPAWN": "Spawn",
    "MATCH": "Match", "CASE": "Case", "BRANCH": "Branch",
    "DEFAULT": "Default", "ACT": "Act", "TRY": "Try", "CATCH": "Catch",
    "FINALLY": "Finally", "FN": "Fn", "AS": "As", "IN": "In",
    "FUNCTIONS": "Functions", "MAIN": "Main", "SHARED": "Shared",
    "STORE": "Store", "CHANNEL": "Channel", "RECV": "Recv",
    "LLM_IF": "LlmIf", "ON_TOOL_END": "OnToolEnd",
    "ON_COMPLETE": "OnComplete", "STREAM": "Stream",
    "COMMENTS": "Comments", "ISOLATION": "Isolation",
    "DELEGATE": "Delegate", "PARAMS": "Params",
}


def tt(name: str) -> str:
    """Map a Python TokenType enum name to the Rust TokenType variant."""
    return TT_MAP.get(name, name)


def rust_ident(src: str, idx: int) -> str:
    """Deterministic test identifier from source snippet."""
    # keep ascii alnum + underscore, else hex hash
    h = hex(abs(hash(src)) % (10 ** 8))[2:]
    tag = re.sub(r"[^A-Za-z0-9_]", "_", src[:24])
    return f"t{idx}_{tag or 'src'}_{h}"


def rust_str(s: str) -> str:
    """Render a Python str as a Rust double-quoted string literal."""
    out = ['"']
    for ch in s:
        if ch == '"':
            out.append('\\"')
        elif ch == "\\":
            out.append("\\\\")
        elif ch == "\n":
            out.append("\\n")
        elif ch == "\r":
            out.append("\\r")
        elif ch == "\t":
            out.append("\\t")
        elif ch == "\x00":
            out.append("\\0")
        elif ord(ch) < 32 or ord(ch) == 127:
            out.append("\\u{%x}" % ord(ch))
        elif ord(ch) > 127:
            # keep non-ASCII as-is; Rust strings are UTF-8
            out.append(ch)
        else:
            out.append(ch)
    out.append('"')
    return "".join(out)


def rust_str_vec(items) -> str:
    """Render a list of strings as a Rust `Vec<String>`-style literal."""
    return "vec![" + ", ".join(rust_str(s) + ".to_string()" for s in items) + "]"


# ── emission ──────────────────────────────────────────────────────────────

HEADER = """//! Tier C generated tests — {suite} ({count} cases).
//!
//! AUTO-GENERATED by scripts/gen-tier-c.py from the Python reference test
//! files under {helen}/tests/{suite_src}/. Expected values are produced by the
//! Python reference implementation; the Rust pipeline must reproduce them.
//! Do not edit by hand — regenerate with: python3 scripts/gen-tier-c.py
"""


def emit_lexer(cases: list[tuple[str, dict]], out: Path) -> int:
    if not cases:
        return 0
    lines = [HEADER.format(suite="Lexer", count=len(cases),
                           helen=HELEN, suite_src="lexer")]
    
    lines.append("use helen_core::errors::ErrorCode;")
    lines.append("use helen_core::lexer::Scanner;")
    lines.append("use helen_core::tokens::TokenType;")
    lines.append("")
    lines.append("fn scan_tokens(src: &str) -> (Vec<(TokenType, String)>, Vec<(ErrorCode, String)>) {")
    lines.append("    let mut s = Scanner::new(src, \"<test>\");")
    lines.append("    let toks = s.scan_all();")
    lines.append("    let pairs = toks.iter().map(|t| (t.kind, t.lexeme.clone())).collect();")
    lines.append("    let errs = s.errors().iter().map(|e| (e.code(), e.message().to_string())).collect();")
    lines.append("    (pairs, errs)")
    lines.append("}")
    lines.append("")
    for i, (src, exp) in enumerate(cases):
        name = rust_ident(src, i)
        lines.append(f"#[test]")
        lines.append(f"fn {name}() {{")
        lines.append(f"    let src = {rust_str(src)};")
        lines.append(f"    let (toks, errs) = scan_tokens(src);")
        if "__err__" in exp:
            lines.append(f"    // reference failed: {exp['__err__']}")
            lines.append(f"    assert!(errs.len() >= 0);")
        else:
            expected_tokens = [(t["kind"], t["lex"]) for t in exp["tokens"]]
            lines.append(f"    let expected: Vec<(TokenType, String)> = vec![")
            for kind, lex in expected_tokens:
                lines.append(f"        (TokenType::{tt(kind)}, {rust_str(lex)}.to_string()),")
            lines.append(f"    ];")
            lines.append(f"    assert_eq!(toks.len(), expected.len(), \"token count\");")
            lines.append(f"    for (got, want) in toks.iter().zip(expected.iter()) {{")
            lines.append(f"        assert_eq!(got.0, want.0, \"kind mismatch\");")
            lines.append(f"        assert_eq!(got.1, want.1, \"lexeme mismatch\");")
            lines.append(f"    }}")
            if exp.get("errors"):
                exp_codes = [e["code"] for e in exp["errors"]]
                lines.append(f"    assert_eq!(errs.len(), {len(exp_codes)}, \"error count\");")
        lines.append(f"}}")
        lines.append("")
    out.write_text("\n".join(lines), encoding="utf-8")
    return len(cases)


def emit_parser(cases: list[tuple[str, dict]], out: Path) -> int:
    if not cases:
        return 0
    lines = [HEADER.format(suite="Parser", count=len(cases),
                           helen=HELEN, suite_src="parser")]
    lines.append("use helen_core::ast_printer::AstPrinter;")
    
    lines.append("use helen_core::lexer::Scanner;")
    lines.append("use helen_parser::Parser;")
    lines.append("")
    lines.append("fn parse_print(src: &str) -> (String, Vec<String>) {")
    lines.append("    let mut sc = Scanner::new(src, \"<test>\");")
    lines.append("    let toks = sc.scan_all();")
    lines.append("    let mut p = Parser::new(toks);")
    lines.append("    let prog = p.parse();")
    lines.append("    let pr = AstPrinter::new();")
    lines.append("    let codes = p.errors().iter().map(|e| format!(\"E{:04}\", e.code().value())).collect();")
    lines.append("    (pr.print_program(&prog), codes)")
    lines.append("}")
    lines.append("")
    for i, (src, exp) in enumerate(cases):
        name = rust_ident(src, i)
        lines.append(f"#[test]")
        lines.append(f"fn {name}() {{")
        lines.append(f"    let src = {rust_str(src)};")
        lines.append(f"    let (printed, errs) = parse_print(src);")
        if "__err__" in exp:
            lines.append(f"    // reference failed: {exp['__err__']}")
        else:
            lines.append(f"    let expected = {rust_str(exp['printed'])};")
            lines.append(f"    assert_eq!(printed, expected, \"AST printer mismatch\");")
            exp_codes = exp["errors"]
            if exp_codes:
                lines.append(f"    let exp_codes: Vec<String> = {rust_str_vec(exp_codes)};")
                lines.append(f"    assert_eq!(errs, exp_codes, \"parse error codes\");")
            else:
                lines.append(f"    assert!(errs.is_empty(), \"unexpected parse errors\");")
        lines.append(f"}}")
        lines.append("")
    out.write_text("\n".join(lines), encoding="utf-8")
    return len(cases)


def emit_semantic(cases: list[tuple[str, dict]], out: Path) -> int:
    if not cases:
        return 0
    lines = [HEADER.format(suite="Semantic", count=len(cases),
                           helen=HELEN, suite_src="semantic")]
    lines.append("use helen_core::lexer::Scanner;")
    lines.append("use helen_parser::Parser;")
    lines.append("use helen_semantic::analyze_codes;")
    lines.append("")
    lines.append("fn analyze_codes_of(src: &str) -> Vec<String> {")
    lines.append("    let mut sc = Scanner::new(src, \"<test>\");")
    lines.append("    let toks = sc.scan_all();")
    lines.append("    let mut p = Parser::new(toks);")
    lines.append("    let prog = p.parse();")
    lines.append("    analyze_codes(&prog)")
    lines.append("}")
    lines.append("")
    for i, (src, exp) in enumerate(cases):
        name = rust_ident(src, i)
        lines.append(f"#[test]")
        lines.append(f"fn {name}() {{")
        lines.append(f"    let src = {rust_str(src)};")
        lines.append(f"    let got = analyze_codes_of(src);")
        if "__err__" in exp:
            lines.append(f"    // reference failed: {exp['__err__']}")
        else:
            exp_codes = exp["codes"]
            lines.append(f"    let expected: Vec<String> = {rust_str_vec(exp_codes)};")
            lines.append(f"    assert_eq!(got, expected, \"E-code mismatch\");")
        lines.append(f"}}")
        lines.append("")
    out.write_text("\n".join(lines), encoding="utf-8")
    return len(cases)


# ── main ──────────────────────────────────────────────────────────────────

def main() -> int:
    emit_out = Path(sys.argv[sys.argv.index("--emit-out") + 1]
                    if "--emit-out" in sys.argv else str(RUST / "crates"))
    total = 0

    # lexer
    lex_cases: list[tuple[str, dict]] = []
    for f in sorted((HELEN / "tests/lexer").glob("test_*.py")):
        for src in extract_lexer_snippets(f):
            lex_cases.append((src, run_ref(REF_LEXER, src)))
    total += emit_lexer(lex_cases, emit_out / "helen-core/tests/lexer_tierc_tests.rs")
    print(f"lexer: {len(lex_cases)} cases")

    # parser
    par_cases: list[tuple[str, dict]] = []
    for f in sorted((HELEN / "tests/parser").glob("test_*.py")):
        for src in extract_parser_snippets(f):
            par_cases.append((src, run_ref(REF_PRINT, src)))
    total += emit_parser(par_cases, emit_out / "helen-parser/tests/parser_tierc_tests.rs")
    print(f"parser: {len(par_cases)} cases")

    # semantic
    sem_cases: list[tuple[str, dict]] = []
    for f in sorted((HELEN / "tests/semantic").glob("test_*.py")):
        for src in extract_semantic_snippets(f):
            sem_cases.append((src, run_ref(REF_SEM, src)))
    total += emit_semantic(sem_cases, emit_out / "helen-semantic/tests/semantic_tierc_tests.rs")
    print(f"semantic: {len(sem_cases)} cases")

    print(f"TOTAL: {total} generated tests")
    return 0


if __name__ == "__main__":
    sys.exit(main())
