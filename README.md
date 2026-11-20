# helen-rust

A complete reimplementation of the [Helen programming language](https://github.com/hahalee000000/helen) in Rust.

**Goal:** Feature-parity with Helen v1.45.0 — including the Python FFI (Helen → Python) and the Python Bridge (Python → Helen) — verified by differential conformance testing against the reference implementation and its 3,860 pytest tests.

## Project Conventions

- **All code and documentation in this repository are written in English.**
- The only non-English text allowed is Helen source-level identifiers that are part of the language itself (e.g. Chinese keyword aliases like `智能体`/`agent`, `分生`/`spawn`), and stdlib locale data (`stdlib/locales/zh.py` equivalents).
- String semantics: **native UTF-8 byte-based strings** (`len()` = bytes; byte-offset index/slice). String **iteration is intentionally unsupported** (Helen raises at runtime; only lists are iterable). Deliberate, documented deviation from Python's code-point `str` for non-ASCII — see design decisions D4/D11 in `wiki/plan/README.md`.
- Integers are arbitrary-precision (`num-bigint`, D3) — matching Python's big-int behavior from day one.
- Code style follows `rustfmt` + `cargo clippy -- -D warnings`.

## Repository Layout

```
crates/
  helen-core/           # tokens, lexer, AST
  helen-parser/         # parser, syntax analysis
  helen-semantic/       # type system, symbol tables, analyzer
  helen-interpreter/    # value model, environment, tree-walk execution
  helen-stdlib/         # 378 builtin functions
  helen-runtime/        # LLM runtime, agents, tools, skills, MCP, context
  helen-rust/           # crates.io package "helen-rust", binary "helen" (CLI, REPL, docgen)
  helen-lsp/            # Language Server Protocol implementation
  helen-ffi/            # Python FFI (Helen → Python), PyO3, feature-gated
  helen-python-bridge/  # Python Bridge (Python → Helen), PyO3 cdylib
  helen-agent/          # WebUI server (Axum + React frontend)
scripts/
  install.sh            # Standalone installer (downloads or builds from source)
  dev.sh                # Development loop (fmt, clippy, test)
  run-all-tests.sh      # Run all test suites
wiki/plan/              # Master implementation plan (17 docs, phases M0–M14)
```

## Status

- [x] Master implementation plan (M0–M14) — `wiki/plan/`
- [x] Plan review: C1–C18 source-verified corrections + resolved decisions — 2026-08-15
- [x] M0–M14 Implementation phases — complete
- [x] Published to **crates.io** (11 crates, v0.1.6) — `cargo install helen-rust`
- [x] Published to **PyPI** (`helen-rust` v0.1.0) — `pip install helen-rust`
- [x] Helen Agent WebUI — pure Rust server with embedded React frontend

## Installation

### Option 1: Install Script (Recommended)

```bash
# Download and install latest release (includes pre-built frontend)
curl -fsSL https://raw.githubusercontent.com/hahalee000000/helen-rust/main/scripts/install.sh | bash

# Or clone and install from source (builds frontend automatically)
git clone https://github.com/hahalee000000/helen-rust.git
cd helen-rust
./scripts/install.sh --from-build
```

**Install script options:**
```bash
./scripts/install.sh                 # install binary to ~/.cargo/bin
./scripts/install.sh --prefix DIR    # install binary to DIR
./scripts/install.sh --with-bridge   # also pip-install the Python bridge
./scripts/install.sh --from-build    # build from source (includes frontend)
./scripts/install.sh --skip-frontend # skip frontend build (agent UI won't work)
```

**Requirements for `--from-build`:**
- Rust toolchain (rustc, cargo)
- Node.js + npm (for frontend build; use `--skip-frontend` if not needed)

### Option 2: From crates.io

```bash
cargo install helen-rust
```

**Note:** The crates.io release includes pre-built frontend assets. No Node.js required.

### Option 3: From PyPI (Python Bridge only)

```bash
pip install helen-rust
python -c "import helen_rust; print(helen_rust.__version__)"
```

### Option 4: Build from Source

```bash
git clone https://github.com/hahalee000000/helen-rust.git
cd helen-rust

# Build frontend (required for agent WebUI)
cd crates/helen-agent/frontend
npm install
npx vite build
cd ../../..

# Build binary
cargo build --release

# Run
./target/release/helen --version
```

## Usage

### CLI

```bash
# Interactive REPL
helen

# Run a Helen program
helen program.helen

# Check syntax without executing
helen check program.helen

# Run tests
helen test test_file.helen

# Run tests with coverage
helen coverage test_file.helen

# Generate documentation
helen doc --with-builtins -o docs.md

# Manage LLM providers
helen provider list
```

### Helen Agent WebUI

Launch the Helen programming assistant with a modern web interface:

```bash
# Start on default port 8001
helen agent

# Custom port
helen agent --port 8080

# With authentication
helen agent --auth my-secret-token
```

Open `http://127.0.0.1:8001` in your browser.

**Features:**
- Interactive chat with Helen agents
- Real-time WebSocket communication
- Session management (persistent conversations)
- File upload/download
- Syntax highlighting and code execution
- Token-based authentication (optional)

### Example Helen Program

```helen
import std.core.*

main {
    print("Hello from Helen-Rust!")
}
```

Save as `hello.helen` and run:

```bash
helen hello.helen
```

## Development

```bash
# Run development checks (fmt, clippy, test)
./scripts/dev.sh

# Run all tests
./scripts/run-all-tests.sh

# Check parity with Python Helen
./scripts/check-parity.sh
```

## Documentation

- [Release Notes v0.1.4](wiki/release-notes-v0.1.4.md) — WebUI features and installation
- [Implementation Plan](wiki/plan/README.md) — Master roadmap (M0–M14)
- [API Alignment](wiki/rust/hp-hr-api-alignment.md) — Python Helen vs Rust implementation comparison
- [Frontend Comparison](wiki/rust/hp-hr-frontend-comparison.md) — WebUI architecture details
- [Agent Isolation Features](wiki/runtime/agent-isolation-features.md) — Agent scope isolation implementation
- [Agent Latency Analysis](wiki/runtime/agent-latency-analysis.md) — Performance analysis and optimization

## License

MIT — see [LICENSE](LICENSE) for details.

## Links

- **Python Helen:** https://github.com/hahalee000000/helen
- **Issues:** https://github.com/hahalee000000/helen-rust/issues
- **crates.io:** https://crates.io/crates/helen-rust
- **PyPI:** https://pypi.org/project/helen-rust/
