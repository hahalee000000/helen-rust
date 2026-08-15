# M9 — MCP (Model Context Protocol) Client

**Objective:** Port `runtime/mcp/*` (client, config, registry, server_manager, exceptions). Exit criterion: MCP server discovery/invocation tests pass; MCP tools appear in the agent tool registry.

## Files

```
crates/helen-runtime/src/mcp/mod.rs        // registry + config
crates/helen-runtime/src/mcp/client.rs     // JSON-RPC stdio/SSE client
crates/helen-runtime/src/mcp/server.rs     // server manager (spawn/monitor external servers)
crates/helen-runtime/src/mcp/errors.rs
```

## Task 9.1: Protocol client

MCP uses JSON-RPC 2.0 over stdio (and optional SSE). Rust: `serde_json` + `std::process::Child` with stdin/stdout pipes (blocking reads in a dedicated thread, matching Python's `subprocess`-based client). Port: `initialize` handshake, `tools/list`, `tools/call`, capability negotiation, error mapping (`McpError` hierarchy — port `mcp/exceptions.py`).

## Task 9.2: Registry + server manager

- `registry.rs`: MCP server definitions from config (command/args/env), tool discovery, health.
- `server.rs`: lifecycle — start on demand, keep-alive, shutdown on interpreter drop; multi-server isolation.

## Task 9.3: Integration

- MCP tool names merged into the tool registry (M6.3) with `mcp://` prefix or Python's naming scheme — **match exactly**.
- `_ensure_mcp_initialized()` hook (port from `runtime/tools.py`): lazily start configured MCP servers before tool resolution.
- Port `tests/runtime/test_mcp_*.py` (fixtures use a mock MCP server — reuse `tests/runtime/fixtures`).

## Definition of Done — M9

- [ ] A fixture MCP server (JSON-RPC over stdio) is discovered and its tools callable from a Helen program.
- [ ] Server crash → clear `McpError`; shutdown clean.
- [ ] Naming/schema parity with Python MCP integration.
