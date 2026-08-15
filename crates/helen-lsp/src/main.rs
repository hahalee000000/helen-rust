//! `helen-lsp` binary — run the Helen Language Server (JSON-RPC over stdio).
//!
//! Usage: `helen lsp` (or run the crate binary directly).

fn main() {
    let mut server = helen_lsp::HelenLanguageServer::new();
    server.run();
}
