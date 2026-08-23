//! Helen code executor

use helen_core::lexer::Scanner;
use helen_interpreter::interpreter::Interpreter;
use helen_parser::Parser;

/// Execute Helen code and return the captured stdout output
pub async fn execute_helen(code: &str) -> Result<String, String> {
    // Lex
    let mut scanner = Scanner::new(code, "<web>");
    let tokens = scanner.scan_all();

    // Parse
    let mut parser = Parser::new(tokens);
    let program = parser.parse();
    let errors = parser.errors();
    if !errors.is_empty() {
        let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        return Err(format!("Parse errors: {}", msgs.join("; ")));
    }

    // Execute
    let mut interp = Interpreter::new();
    interp
        .interpret(&program)
        .map_err(|e| format!("Execution error: {:?}", e))?;

    // Return captured stdout
    let output = interp.stdout.lock().unwrap().clone();
    Ok(output)
}
