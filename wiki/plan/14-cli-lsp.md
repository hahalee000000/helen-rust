# M12 — CLI, REPL, Formatter, Docgen, LSP

**Objective:** Port `cli/*` (2,000+ lines) and `lsp/server.py` (1,375 lines). Exit criterion: `tests/cli` golden tests + LSP integration tests pass.

## Files

```
crates/helen-rust/src/main.rs        // arg parsing + subcommand dispatch
crates/helen-rust/src/run.rs         // helen <file>
crates/helen-rust/src/check.rs       // helen check <file> (lint)
crates/helen-rust/src/test.rs        // helen test <file> (port stdlib test runner)
crates/helen-rust/src/repl.rs        // interactive REPL
crates/helen-rust/src/docgen.rs      // helen docgen (stdlib docs + wiki)
crates/helen-rust/src/formatter.rs   // code formatter
crates/helen-rust/src/ask.rs         // helen ask
crates/helen-rust/src/coverage.rs    // helen coverage
crates/helen-lsp/src/server.rs      // JSON-RPC over stdio
crates/helen-lsp/src/features.rs    // hover, completion, diagnostics, goto-def
```

## Task 12.1: `helen` CLI (port `cli/__main__.py`)

**Package:** crates.io package name `helen-rust` (`[package] name = "helen-rust"`, `[[bin]] name = "helen"`) so `cargo install helen-rust` works.

Subcommands and flags — **match exactly**: `helen <file>` (run, exit code = program exit/error code), `helen check <file>` (report semantic errors + line numbers; exit non-zero on errors), `helen test <file>`, `helen repl`, `helen docgen`, `helen ask`, `helen coverage`, `helen --version`, `helen --provider-detect`. Use `clap` (or hand-rolled parser if flag semantics are unusual). Error output format (class name + message + `at line X`) must match for golden tests. **Exit codes**: map `HelenRuntimeError` classes to the same process exit codes as Python's CLI (check `cli/__main__.py` `main()`).

## Task 12.2: REPL (port `cli/repl.py`)

Multiline input detection (unclosed braces/strings), continuation prompt, history file, `import` cache semantics (documented Python caveat — new code in REPL requires reload; match it), error printing, `:help`-style commands if present. Interactive vs piped behavior.

## Task 12.3: Formatter + docgen

- `formatter.rs`: port `cli/formatter.py` (token-based reflow). Golden tests on `examples/*.helen`.
- `docgen.rs`: render the stdlib catalog (M4 Task 4.1 `stdlib_catalog.json`) in Python's output format; `--zh` for Chinese aliases.

## Task 12.4: LSP (port `lsp/server.py`)

JSON-RPC 2.0 over stdio: `initialize`, `textDocument/didOpen|didChange|didSave`, `publishDiagnostics` (from parser + semantic analyzer — reuse M1/M2 crates), `hover` (function/type docs from stdlib catalog + source), `completion` (keywords, builtins, locals), `gotoDefinition` (imports, fn/agent decls). Port the exact JSON-RPC framing and notification names. Tests: port `tests/lsp/test_server.py` (raw JSON-RPC fixture-driven).

## Definition of Done — M12

- [ ] `tests/cli` golden tests pass (help text, version, run/check exit codes, error format).
- [ ] REPL interactive flow works; multiline + history.
- [ ] `docgen` output matches Python's for the same catalog.
- [ ] LSP: VSCode-style client gets correct diagnostics/hover/completion on the corpus; ported protocol tests pass.
