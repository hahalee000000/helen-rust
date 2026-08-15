//! helen-rust CLI — M1: `--lex` token-stream dump for differential testing.
//!
//! Emits the same JSON schema as `tests/conformance/reference.py --lex`:
//! a JSON array of `{type, lexeme, line, col, end_line, end_col, literal}`.
//! Float literals are formatted with Rust's shortest-round-trip `Debug`
//! repr; the comparison script parses both sides numerically.

use helen_core::ast_printer::AstPrinter;
use helen_core::lexer::Scanner;
use helen_core::tokens::LiteralValue;
use helen_parser::Parser;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() >= 3 && args[1] == "--parse" {
        let path = &args[2];
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("helen: cannot read {path}: {e}");
                std::process::exit(2);
            }
        };

        let mut scanner = Scanner::new(&source, path);
        let tokens = scanner.scan_all();
        let mut parser = Parser::new(tokens);
        let program = parser.parse();
        let printer = AstPrinter::new();
        let out = printer.print_program(&program);
        println!("{}", serde_json::json!({ "ast": out }));
        return;
    }

    if args.len() >= 3 && args[1] == "--lex" {
        let path = &args[2];
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("helen: cannot read {path}: {e}");
                std::process::exit(2);
            }
        };

        let mut scanner = Scanner::new(&source, path);
        let tokens = scanner.scan_all();

        let out: Vec<serde_json::Value> = tokens
            .iter()
            .map(|t| {
                let literal = match &t.literal {
                    LiteralValue::Null => serde_json::json!({"kind": "null"}),
                    LiteralValue::Bool(b) => serde_json::json!({"kind": "bool", "value": b}),
                    LiteralValue::Str(s) => serde_json::json!({"kind": "str", "value": s}),
                    LiteralValue::Int(i) => {
                        serde_json::json!({"kind": "int", "value": i.to_string()})
                    }
                    LiteralValue::Float(f) => {
                        serde_json::json!({"kind": "float", "value": format!("{f:?}")})
                    }
                };
                serde_json::json!({
                    "type": t.kind.name(),
                    "lexeme": t.lexeme,
                    "line": t.line,
                    "col": t.col,
                    "end_line": t.end_line,
                    "end_col": t.end_col,
                    "literal": literal,
                })
            })
            .collect();

        println!("{}", serde_json::to_string(&out).unwrap());
        return;
    }

    eprintln!("helen: usage: helen --lex <file.helen>  (M1 lexer differential mode)");
    std::process::exit(2);
}
