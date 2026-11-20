# Helen Rust v0.1.4 Release Notes

## 🎉 Major Release: Helen Agent WebUI

This release introduces the **Helen Agent WebUI** — a pure Rust web server for running Helen agents with a modern React frontend.

### ✨ New Features

#### Phase 1-5: Core WebUI (M17)
- **Pure Rust Web Server** — Axum-based HTTP/WebSocket server
- **Embedded Frontend** — React UI compiled into binary (3.8MB)
- **REST API** — Health check, agent listing, chat status
- **WebSocket Support** — Real-time chat with Helen agents
- **Helen Execution** — Direct interpreter integration
- **CLI Integration** — `helen agent --port 8000`

#### Phase 6: Python Bridge Validation
- **Bridge Validation API** — `POST /api/bridge/validate`
- **Helen Code Validation** — Execute and validate Helen programs
- **Error Reporting** — Detailed success/output/error responses

#### Phase 7: Session Management
- **Persistent Sessions** — Conversations saved to disk
- **Session API** — `POST/GET/DELETE /api/sessions`
- **JSON Storage** — `~/.local/share/helen-agent/sessions/`

#### Phase 8: File Upload/Download
- **File Storage** — Upload/download/delete files
- **Multipart Support** — Standard file upload API
- **File API** — `POST/GET/DELETE /api/files`
- **Storage Location** — `~/.local/share/helen-agent/files/`

#### Phase 9: Authentication
- **Token-Based Auth** — Single-user authentication
- **Auth Middleware** — Protect API endpoints
- **CLI Flag** — `helen agent --auth <TOKEN>`

### 📦 Installation

#### From crates.io (Rust)
```bash
cargo install helen-rust
helen agent --port 8000
```

#### From PyPI (Python)
```bash
pip install helen-rust
python -c "import helen_rust; print(helen_rust.__version__)"
```

### 🚀 Usage

```bash
# Start web server
helen agent --port 8000

# With authentication
helen agent --port 8000 --auth my-secret-token

# Open browser
open http://localhost:8000
```

### 📊 Test Coverage
- **38 new tests** for helen-agent crate
- **1762 total workspace tests** passing
- **Full integration tests** for all phases

### 🔗 Links
- **crates.io**: https://crates.io/crates/helen-rust/0.1.4
- **PyPI**: https://pypi.org/project/helen-rust/0.1.4/
- **GitHub**: https://github.com/hahalee000000/helen-rust

### 📝 Commits
```
350c8eb M17: bump version to 0.1.4 for crates.io release with helen-agent
a7e372d M17: add version requirements for crates.io publishing
2d7416b M17: add Python bridge validation mode
36518bf M17: add single-user authentication support
a206725 M17: add file upload/download support
56f7b9d M17: add session management with persistence
4c89f78 M17: add full integration test for agent workflow
db9c64f M17: replace Python-based agent launcher with pure Rust implementation
17c2c1a M17: execute Helen programs and capture output
540d9ba M17: embed agent .helen files in binary
70fe2a0 M17: add WebSocket endpoint for real-time chat
0b19282 M17: add REST API endpoints for chat and agents
```

### 🎯 What's Next
- Multi-platform wheel builds (macOS, Windows)
- Docker container for easy deployment
- Enhanced agent collaboration features
- Performance optimizations

---

**Full Changelog**: https://github.com/hahalee000000/helen-rust/compare/v0.1.3...v0.1.4
