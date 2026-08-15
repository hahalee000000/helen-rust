//! Helen Language Server Protocol implementation — port of `helen/lsp/server.py`.
//!
//! Provides IDE support via LSP: diagnostics, completion (keywords + stdlib +
//! snippets), go-to-definition, find references, hover, document symbols.
//! JSON-RPC 2.0 over stdio (LSP standard transport).

use std::collections::HashMap;
use std::io::{BufRead, Read, Write};

use serde_json::{Map, Value as Json};

use crate::constants::*;

/// Log to stderr — visible in VS Code's 'Helen Language Server' output panel.
pub fn log(msg: &str) {
    eprintln!("[helen-lsp] {msg}");
}

// ── LSP data structures ────────────────────────────────────────────

/// LSP Position (0-based line and character).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: i64,
    pub character: i64,
}

impl Position {
    pub fn new(line: i64, character: i64) -> Self {
        Position { line, character }
    }

    pub fn to_dict(&self) -> Json {
        let mut m = Map::new();
        m.insert("line".into(), Json::from(self.line));
        m.insert("character".into(), Json::from(self.character));
        Json::Object(m)
    }
}

/// LSP Range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

impl Range {
    pub fn to_dict(&self) -> Json {
        let mut m = Map::new();
        m.insert("start".into(), self.start.to_dict());
        m.insert("end".into(), self.end.to_dict());
        Json::Object(m)
    }
}

/// LSP Diagnostic.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub range: Range,
    pub severity: i64, // 1=Error, 2=Warning, 3=Info, 4=Hint
    pub message: String,
    pub source: String,
    pub code: Option<String>,
}

impl Diagnostic {
    pub fn new(range: Range, severity: i64, message: impl Into<String>) -> Self {
        Diagnostic {
            range,
            severity,
            message: message.into(),
            source: "helen".to_string(),
            code: None,
        }
    }

    pub fn to_dict(&self) -> Json {
        let mut m = Map::new();
        m.insert("range".into(), self.range.to_dict());
        m.insert("severity".into(), Json::from(self.severity));
        m.insert("message".into(), Json::from(self.message.clone()));
        m.insert("source".into(), Json::from(self.source.clone()));
        if let Some(code) = &self.code {
            m.insert("code".into(), Json::from(code.clone()));
        }
        Json::Object(m)
    }
}

/// LSP CompletionItem.
#[derive(Debug, Clone)]
pub struct CompletionItem {
    pub label: String,
    pub kind: i64, // 1=Text, 2=Method, 3=Function, ... 14=Keyword, 15=Snippet
    pub detail: Option<String>,
    pub insert_text: Option<String>,
    pub insert_text_format: Option<i64>, // 1=PlainText, 2=Snippet
    pub documentation: Option<String>,
}

impl CompletionItem {
    pub fn keyword(label: &str) -> Self {
        CompletionItem {
            label: label.to_string(),
            kind: 14,
            detail: Some("Helen keyword".to_string()),
            insert_text: None,
            insert_text_format: None,
            documentation: None,
        }
    }

    pub fn context_keyword(label: &str) -> Self {
        CompletionItem {
            label: label.to_string(),
            kind: 14,
            detail: Some("context keyword".to_string()),
            insert_text: None,
            insert_text_format: None,
            documentation: None,
        }
    }

    pub fn type_item(label: &str) -> Self {
        CompletionItem {
            label: label.to_string(),
            kind: 8, // Interface (type)
            detail: Some("Helen type".to_string()),
            insert_text: None,
            insert_text_format: None,
            documentation: None,
        }
    }

    pub fn snippet(label: &str, detail: &str, insert_text: &str) -> Self {
        CompletionItem {
            label: label.to_string(),
            kind: 15, // Snippet
            detail: Some(detail.to_string()),
            insert_text: Some(insert_text.to_string()),
            insert_text_format: Some(2),
            documentation: None,
        }
    }

    pub fn function(label: &str, detail: &str, documentation: &str) -> Self {
        CompletionItem {
            label: label.to_string(),
            kind: 3, // Function
            detail: Some(detail.to_string()),
            insert_text: Some(format!("{label}(")),
            insert_text_format: None,
            documentation: Some(documentation.to_string()),
        }
    }

    pub fn to_dict(&self) -> Json {
        let mut m = Map::new();
        m.insert("label".into(), Json::from(self.label.clone()));
        m.insert("kind".into(), Json::from(self.kind));
        if let Some(detail) = &self.detail {
            m.insert("detail".into(), Json::from(detail.clone()));
        }
        if let Some(it) = &self.insert_text {
            m.insert("insertText".into(), Json::from(it.clone()));
        }
        if let Some(itf) = self.insert_text_format {
            m.insert("insertTextFormat".into(), Json::from(itf));
        }
        if let Some(doc) = &self.documentation {
            let mut d = Map::new();
            d.insert("kind".into(), Json::from("markdown"));
            d.insert("value".into(), Json::from(doc.clone()));
            m.insert("documentation".into(), Json::Object(d));
        }
        Json::Object(m)
    }
}

/// LSP Location.
#[derive(Debug, Clone)]
pub struct Location {
    pub uri: String,
    pub range: Range,
}

impl Location {
    pub fn to_dict(&self) -> Json {
        let mut m = Map::new();
        m.insert("uri".into(), Json::from(self.uri.clone()));
        m.insert("range".into(), self.range.to_dict());
        Json::Object(m)
    }
}

/// State for an open document.
#[derive(Debug, Clone)]
pub struct DocumentState {
    pub uri: String,
    pub content: String,
    pub version: i64,
    pub diagnostics: Vec<Diagnostic>,
}

impl DocumentState {
    pub fn new(uri: &str, content: &str, version: i64) -> Self {
        DocumentState {
            uri: uri.to_string(),
            content: content.to_string(),
            version,
            diagnostics: Vec::new(),
        }
    }
}

// ── LSP Server ─────────────────────────────────────────────────────

pub struct HelenLanguageServer {
    pub documents: HashMap<String, DocumentState>,
    pub capabilities: Json,
}

impl Default for HelenLanguageServer {
    fn default() -> Self {
        Self::new()
    }
}

impl HelenLanguageServer {
    pub fn new() -> Self {
        let mut completion_provider = Map::new();
        completion_provider.insert(
            "triggerCharacters".into(),
            Json::Array(vec![
                Json::from("."),
                Json::from("\""),
                Json::from("'"),
                Json::from(" "),
                Json::from("@"),
            ]),
        );
        completion_provider.insert("resolveProvider".into(), Json::Bool(false));

        let mut diagnostic_provider = Map::new();
        diagnostic_provider.insert("interFileDependencies".into(), Json::Bool(false));
        diagnostic_provider.insert("workspaceDiagnostics".into(), Json::Bool(false));

        let mut caps = Map::new();
        caps.insert("textDocumentSync".into(), Json::from(1)); // Full sync
        caps.insert(
            "completionProvider".into(),
            Json::Object(completion_provider),
        );
        caps.insert("definitionProvider".into(), Json::Bool(true));
        caps.insert("referencesProvider".into(), Json::Bool(true));
        caps.insert("hoverProvider".into(), Json::Bool(true));
        caps.insert("documentSymbolProvider".into(), Json::Bool(true));
        caps.insert(
            "diagnosticProvider".into(),
            Json::Object(diagnostic_provider),
        );

        HelenLanguageServer {
            documents: HashMap::new(),
            capabilities: Json::Object(caps),
        }
    }

    // ── Message handling ─────────────────────────────────

    /// Handle a JSON-RPC message and return the response (None for notifications).
    pub fn handle_message(&mut self, message: &Json) -> Option<Json> {
        let method = message
            .get("method")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();
        let msg_id = message.get("id").cloned();
        let params = message.get("params").cloned().unwrap_or(Json::Null);

        // Request (has id)
        if let Some(id) = msg_id {
            let result = self.handle_request(&method, &params);
            let mut m = Map::new();
            m.insert("jsonrpc".into(), Json::from("2.0"));
            m.insert("id".into(), id);
            m.insert("result".into(), result);
            return Some(Json::Object(m));
        }

        // Notification (no id)
        self.handle_notification(&method, &params);
        None
    }

    fn handle_request(&mut self, method: &str, params: &Json) -> Json {
        match method {
            "initialize" => self.initialize(params),
            "shutdown" => Json::Null,
            "textDocument/completion" => self.completion(params),
            "textDocument/definition" => self.definition(params),
            "textDocument/references" => self.references(params),
            "textDocument/diagnostic" => self.diagnostic(params),
            "textDocument/hover" => self.hover(params).unwrap_or(Json::Null),
            "textDocument/documentSymbol" => self.document_symbol(params),
            _ => Json::Null,
        }
    }

    fn handle_notification(&mut self, method: &str, params: &Json) {
        match method {
            "initialized" => {} // Server is ready
            "exit" => std::process::exit(0),
            "textDocument/didOpen" => self.did_open(params),
            "textDocument/didChange" => self.did_change(params),
            "textDocument/didClose" => self.did_close(params),
            _ => {}
        }
    }

    // ── LSP Methods ───────────────────────────────────────

    pub fn initialize(&mut self, _params: &Json) -> Json {
        log(&format!(
            "initialize — helen-lsp {HELEN_LSP_VERSION}, pid={}",
            std::process::id()
        ));
        let mut info = Map::new();
        info.insert("name".into(), Json::from("helen-lsp"));
        info.insert("version".into(), Json::from(HELEN_LSP_VERSION));
        let mut m = Map::new();
        m.insert("capabilities".into(), self.capabilities.clone());
        m.insert("serverInfo".into(), Json::Object(info));
        Json::Object(m)
    }

    pub fn did_open(&mut self, params: &Json) {
        let doc = params.get("textDocument").cloned().unwrap_or(Json::Null);
        let uri = doc
            .get("uri")
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .to_string();
        let content = doc
            .get("text")
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();
        let version = doc.get("version").and_then(|v| v.as_i64()).unwrap_or(0);

        log(&format!(
            "didOpen: {uri} ({} chars, version={version})",
            content.len()
        ));
        self.documents
            .insert(uri.clone(), DocumentState::new(&uri, &content, version));
        self.publish_diagnostics(&uri);
    }

    pub fn did_change(&mut self, params: &Json) {
        let uri = params
            .get("textDocument")
            .and_then(|d| d.get("uri"))
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .to_string();
        let version = params
            .get("textDocument")
            .and_then(|d| d.get("version"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let changes = params.get("contentChanges").cloned().unwrap_or(Json::Null);

        let Some(doc) = self.documents.get_mut(&uri) else {
            return;
        };
        doc.version = version;

        // Apply changes (Full sync — matches textDocumentSync: 1)
        if let Json::Array(changes) = changes {
            for change in changes {
                if let Some(text) = change.get("text").and_then(|t| t.as_str()) {
                    doc.content = text.to_string();
                }
            }
        }

        self.publish_diagnostics(&uri);
    }

    pub fn did_close(&mut self, params: &Json) {
        let uri = params
            .get("textDocument")
            .and_then(|d| d.get("uri"))
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .to_string();
        self.documents.remove(&uri);
    }

    pub fn completion(&mut self, params: &Json) -> Json {
        let uri = params
            .get("textDocument")
            .and_then(|d| d.get("uri"))
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .to_string();
        let position = params.get("position").cloned().unwrap_or(Json::Null);

        let Some(_doc) = self.documents.get(&uri) else {
            return Json::Object({
                let mut m = Map::new();
                m.insert("isIncomplete".into(), Json::Bool(false));
                m.insert("items".into(), Json::Array(Vec::new()));
                m
            });
        };

        let mut items: Vec<Json> = Vec::new();

        // Formal keywords
        for kw in HELLEN_KEYWORDS {
            items.push(CompletionItem::keyword(kw).to_dict());
        }
        // Context keywords
        for kw in HELLEN_CONTEXT_KEYWORDS {
            items.push(CompletionItem::context_keyword(kw).to_dict());
        }
        // Types
        for t in HELLEN_TYPES {
            items.push(CompletionItem::type_item(t).to_dict());
        }
        // Snippets
        for s in HELLEN_SNIPPETS {
            items.push(CompletionItem::snippet(s.label, s.detail, s.insert_text).to_dict());
        }
        // Built-in functions from the stdlib catalog (canonical + aliases)
        let (builtins, aliases) = stdlib_catalog();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (name, description) in &builtins {
            items.push(CompletionItem::function(name, description, description).to_dict());
            seen.insert(name.clone());
        }
        for (alias, canonical) in &aliases {
            if !seen.contains(alias) {
                items.push(
                    CompletionItem::function(
                        alias,
                        &format!("alias of {canonical}"),
                        &format!("Alias of `{canonical}`"),
                    )
                    .to_dict(),
                );
                seen.insert(alias.clone());
            }
        }
        let _ = position;

        let mut m = Map::new();
        m.insert("isIncomplete".into(), Json::Bool(false));
        m.insert("items".into(), Json::Array(items));
        Json::Object(m)
    }

    pub fn definition(&mut self, params: &Json) -> Json {
        let uri = params
            .get("textDocument")
            .and_then(|d| d.get("uri"))
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .to_string();
        let position = params.get("position").cloned().unwrap_or(Json::Null);

        let Some(doc) = self.documents.get(&uri) else {
            log(&format!("definition: doc not found for {uri}"));
            return Json::Array(Vec::new());
        };

        let line_num = position.get("line").and_then(|l| l.as_i64()).unwrap_or(0) + 1; // 1-based
        let char_num = position
            .get("character")
            .and_then(|c| c.as_i64())
            .unwrap_or(0)
            + 1;

        let result = find_definition_at(&doc.content, &uri, line_num, char_num);
        log(&format!(
            "definition: line={line_num} col={char_num} → {}",
            result.len()
        ));
        Json::Array(result)
    }

    pub fn references(&mut self, params: &Json) -> Json {
        let uri = params
            .get("textDocument")
            .and_then(|d| d.get("uri"))
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .to_string();
        let position = params.get("position").cloned().unwrap_or(Json::Null);
        let context = params.get("context").cloned().unwrap_or(Json::Null);
        let include_declaration = context
            .get("includeDeclaration")
            .and_then(|i| i.as_bool())
            .unwrap_or(true);

        let Some(doc) = self.documents.get(&uri) else {
            log(&format!("references: doc not found for {uri}"));
            return Json::Array(Vec::new());
        };

        let line_num = position.get("line").and_then(|l| l.as_i64()).unwrap_or(0) + 1;
        let char_num = position
            .get("character")
            .and_then(|c| c.as_i64())
            .unwrap_or(0)
            + 1;

        let target = get_symbol_at(&doc.content, line_num, char_num);
        let Some(target) = target else {
            log(&format!(
                "references: no symbol at line={line_num} col={char_num}"
            ));
            return Json::Array(Vec::new());
        };

        log(&format!(
            "references: searching for '{target}' across all documents"
        ));

        let mut results: Vec<Json> = Vec::new();
        // Collect URIs first to avoid borrow conflicts.
        let uris: Vec<String> = self.documents.keys().cloned().collect();
        for doc_uri in uris {
            let document = self.documents.get(&doc_uri);
            let Some(document) = document else { continue };
            let refs =
                find_references_in(&document.content, &doc_uri, &target, include_declaration);
            results.extend(refs);
        }

        log(&format!("references: found {} references", results.len()));
        Json::Array(results)
    }

    pub fn hover(&mut self, params: &Json) -> Option<Json> {
        let uri = params
            .get("textDocument")
            .and_then(|d| d.get("uri"))
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .to_string();
        let position = params.get("position").cloned().unwrap_or(Json::Null);

        let doc = self.documents.get(&uri)?;

        let line_num = position.get("line").and_then(|l| l.as_i64()).unwrap_or(0) + 1;
        let char_num = position
            .get("character")
            .and_then(|c| c.as_i64())
            .unwrap_or(0)
            + 1;

        let target = get_symbol_at(&doc.content, line_num, char_num)?;

        // Stdlib functions first
        let (builtins, aliases) = stdlib_catalog();
        for (name, description) in &builtins {
            if name == &target {
                let value = format!("`{name}()` — {description}");
                return Some(hover_markdown(&value));
            }
        }
        if let Some(canonical) = aliases.get(&target) {
            let value = format!("`{target}` — alias of `{canonical}`");
            return Some(hover_markdown(&value));
        }

        // Keywords
        if HELLEN_KEYWORDS.contains(&target.as_str())
            || HELLEN_CONTEXT_KEYWORDS.contains(&target.as_str())
        {
            let desc = keyword_description(&target).map(|d| d.to_string());
            let value = match desc {
                Some(d) => format!("**{target}** — {d}"),
                None => format!("**{target}** — Helen keyword: `{target}`"),
            };
            return Some(hover_markdown(&value));
        }

        // Types
        if HELLEN_TYPES.contains(&target.as_str()) {
            let value = format!("**{target}** — Helen type");
            return Some(hover_markdown(&value));
        }

        // Decorators
        if ["open", "strict", "sandbox", "开放", "严格", "沙箱"].contains(&target.as_str()) {
            let value = format!("**@{target}** — Agent isolation decorator");
            return Some(hover_markdown(&value));
        }

        // User-defined symbols (scan document for declarations)
        if let Some(value) = hover_user_symbol(&doc.content, &target) {
            return Some(hover_markdown(&value));
        }

        None
    }

    pub fn document_symbol(&mut self, params: &Json) -> Json {
        let uri = params
            .get("textDocument")
            .and_then(|d| d.get("uri"))
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .to_string();
        let Some(doc) = self.documents.get(&uri) else {
            return Json::Array(Vec::new());
        };
        Json::Array(document_symbols(&doc.content))
    }

    pub fn diagnostic(&mut self, params: &Json) -> Json {
        let uri = params
            .get("textDocument")
            .and_then(|d| d.get("uri"))
            .and_then(|u| u.as_str())
            .unwrap_or("")
            .to_string();
        let Some(doc) = self.documents.get(&uri) else {
            let mut m = Map::new();
            m.insert("kind".into(), Json::from("full"));
            m.insert("items".into(), Json::Array(Vec::new()));
            return Json::Object(m);
        };
        let items: Vec<Json> = doc.diagnostics.iter().map(|d| d.to_dict()).collect();
        let mut m = Map::new();
        m.insert("kind".into(), Json::from("full"));
        m.insert("items".into(), Json::Array(items));
        Json::Object(m)
    }

    // ── Analysis ─────────────────────────────────────────

    pub fn publish_diagnostics(&mut self, uri: &str) {
        let Some(doc) = self.documents.get_mut(uri) else {
            return;
        };
        let diagnostics = analyze(&doc.content, uri);
        doc.diagnostics = diagnostics.clone();

        let items: Vec<Json> = diagnostics.iter().map(|d| d.to_dict()).collect();
        let mut params = Map::new();
        params.insert("uri".into(), Json::from(uri.to_string()));
        params.insert("diagnostics".into(), Json::Array(items));
        let mut notification = Map::new();
        notification.insert("jsonrpc".into(), Json::from("2.0"));
        notification.insert(
            "method".into(),
            Json::from("textDocument/publishDiagnostics"),
        );
        notification.insert("params".into(), Json::Object(params));
        self.send(&Json::Object(notification));
    }

    // ── I/O ──────────────────────────────────────────────

    pub fn send(&mut self, message: &Json) {
        let body = serde_json::to_string(message).unwrap_or_else(|_| "{}".to_string());
        let body_bytes = body.as_bytes();
        let header = format!(
            "Content-Length: {}\r\nContent-Type: application/vscode-jsonrpc; charset=utf-8\r\n\r\n",
            body_bytes.len()
        );
        let stdout = std::io::stdout();
        let mut out = stdout.lock();
        let _ = out.write_all(header.as_bytes());
        let _ = out.write_all(body_bytes);
        let _ = out.flush();
    }

    /// Run the LSP server, reading JSON-RPC from stdin.
    pub fn run(&mut self) {
        let stdin = std::io::stdin();
        let mut reader = stdin.lock();
        let mut buf = Vec::new();

        loop {
            // Read headers
            let mut content_length: Option<usize> = None;
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => return, // EOF
                    Ok(_) => {}
                    Err(_) => return,
                }
                if line == "\r\n" {
                    break; // End of headers
                }
                if let Some(rest) = line.strip_prefix("Content-Length: ") {
                    content_length = rest.trim().parse::<usize>().ok();
                }
            }

            let Some(content_length) = content_length else {
                continue;
            };

            // Read body
            buf.clear();
            let mut chunk = vec![0u8; content_length];
            let mut read = 0;
            while read < content_length {
                match reader.read(&mut chunk[read..]) {
                    Ok(0) => return, // EOF
                    Ok(n) => read += n,
                    Err(_) => return,
                }
            }
            buf.extend_from_slice(&chunk);

            // Parse and handle
            let Ok(message) = serde_json::from_slice::<Json>(&buf) else {
                continue;
            };
            if let Some(response) = self.handle_message(&message) {
                self.send(&response);
            }
        }
    }
}

// ── Symbol helpers (free functions; Python regex ports) ────────────

/// Convert a `file://` URI to a filesystem path (Python `_uri_to_path`).
pub fn uri_to_path(uri: &str) -> String {
    if let Some(rest) = uri.strip_prefix("file://") {
        // Unquote percent-encoding
        percent_decode(rest)
    } else {
        uri.to_string()
    }
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = &s[i + 1..i + 3];
            if let Ok(v) = u8::from_str_radix(hex, 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Extract the symbol name at the given position (1-based line/col).
/// Python: `re.finditer(r'[\w一-鿿]+', current_line)`.
pub fn get_symbol_at(content: &str, line: i64, col: i64) -> Option<String> {
    let lines: Vec<&str> = content.split('\n').collect();
    if !(0 < line && line <= lines.len() as i64) {
        return None;
    }
    let current_line = lines[(line - 1) as usize];
    let re = regex::Regex::new(r"[\w\u4e00-\u9fff]+").unwrap();
    for m in re.find_iter(current_line) {
        let start = m.start() as i64;
        let end = m.end() as i64;
        if start < col && col <= end {
            return Some(m.as_str().to_string());
        }
    }
    None
}

/// Find all references to `target` in the given content.
/// Python: `_find_references_in`.
pub fn find_references_in(
    content: &str,
    uri: &str,
    target: &str,
    include_declaration: bool,
) -> Vec<Json> {
    let mut results: Vec<Json> = Vec::new();
    let escaped = regex::escape(target);
    let decl_patterns: Vec<regex::Regex> = vec![
        regex::Regex::new(&format!(r"^\s*(?:agent|fn|函数)\s+{escaped}\s*[\({{]")).unwrap(),
        regex::Regex::new(&format!(
            r"^\s*(?:shared\s+)?(?:let|const|定义|常量)\s+{escaped}\s*="
        ))
        .unwrap(),
    ];
    let ref_re = regex::Regex::new(&format!(r"\b{escaped}\b")).unwrap();

    for (i, line) in content.split('\n').enumerate() {
        // Skip comments
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') || trimmed.starts_with("//") || trimmed.starts_with('＃') {
            continue;
        }
        // Skip string literals (simple heuristic)
        if line.contains(&format!("\"{target}\"")) || line.contains(&format!("'{target}'")) {
            continue;
        }

        for m in ref_re.find_iter(line) {
            let is_declaration = decl_patterns.iter().any(|p| p.is_match(line));
            if is_declaration && !include_declaration {
                continue;
            }
            let loc = Location {
                uri: uri.to_string(),
                range: Range {
                    start: Position::new(i as i64, m.start() as i64),
                    end: Position::new(i as i64, m.end() as i64),
                },
            };
            results.push(loc.to_dict());
        }
    }
    results
}

/// Find the definition at a given position (1-based line/col).
/// Python: `_find_definition_at`.
pub fn find_definition_at(content: &str, uri: &str, line: i64, col: i64) -> Vec<Json> {
    let lines: Vec<&str> = content.split('\n').collect();
    if !(0 < line && line <= lines.len() as i64) {
        return Vec::new();
    }
    let current_line = lines[(line - 1) as usize];
    let word_re = regex::Regex::new(r"\b\w+\b").unwrap();
    let words: Vec<String> = word_re
        .find_iter(current_line)
        .map(|m| m.as_str().to_string())
        .collect();
    if words.is_empty() {
        return Vec::new();
    }

    // Find the word under cursor (Python uses `find` — first occurrence)
    let mut target: Option<String> = None;
    for word in &words {
        if let Some(idx) = current_line.find(word) {
            let idx = idx as i64;
            if idx < col && col - 1 <= idx + word.len() as i64 {
                target = Some(word.clone());
                break;
            }
        }
    }
    let Some(target) = target else {
        return Vec::new();
    };

    let escaped = regex::escape(&target);
    let patterns: Vec<regex::Regex> = vec![
        regex::Regex::new(&format!(r"agent\s+({escaped})\s*[\({{]")).unwrap(),
        regex::Regex::new(&format!(r"fn\s+({escaped})\s*\(")).unwrap(),
        regex::Regex::new(&format!(
            r"(?:shared\s+)?(?:let|const|定义|常量)\s+({escaped})\s*="
        ))
        .unwrap(),
    ];

    for (i, file_line) in lines.iter().enumerate() {
        for pattern in &patterns {
            if let Some(cap) = pattern.captures(file_line) {
                if let Some(m) = cap.get(1) {
                    let loc = Location {
                        uri: uri.to_string(),
                        range: Range {
                            start: Position::new(i as i64, m.start() as i64),
                            end: Position::new(i as i64, m.end() as i64),
                        },
                    };
                    return vec![loc.to_dict()];
                }
            }
        }
    }

    Vec::new()
}

/// Hover markdown response builder.
fn hover_markdown(value: &str) -> Json {
    let mut contents = Map::new();
    contents.insert("kind".into(), Json::from("markdown"));
    contents.insert("value".into(), Json::from(value.to_string()));
    let mut m = Map::new();
    m.insert("contents".into(), Json::Object(contents));
    Json::Object(m)
}

/// Hover on user-defined symbols — scan the document for declarations.
/// Python: the `_hover` user-symbol scan.
fn hover_user_symbol(content: &str, target: &str) -> Option<String> {
    let escaped = regex::escape(target);
    let agent_re = regex::Regex::new(&format!(r"(@\w+\s+)?agent\s+{escaped}\s*[\({{]")).unwrap();
    let fn_re = regex::Regex::new(&format!(r"fn\s+{escaped}\s*\(([^)]*)\)")).unwrap();
    let store_re = regex::Regex::new(&format!(r"shared\s+store\s+{escaped}\s*[\({{]")).unwrap();
    let var_re = regex::Regex::new(&format!(
        r"(?:shared\s+)?(?:let|const|设|定义|常量)\s+{escaped}\s*(?::\s*(\S+))?\s*="
    ))
    .unwrap();
    let proto_re = regex::Regex::new(&format!(r"protocol\s+{escaped}\s*[\({{]")).unwrap();

    for (i, line) in content.split('\n').enumerate() {
        let stripped = line.trim_start();
        if let Some(m) = agent_re.captures(stripped) {
            let _ = m;
            let value = format!(
                "```helen\n{stripped}\n```\nAgent declaration (line {})",
                i + 1
            );
            return Some(value);
        }
        if let Some(m) = fn_re.captures(stripped) {
            let _ = m;
            let value = format!(
                "```helen\n{stripped}\n```\nFunction declaration (line {})",
                i + 1
            );
            return Some(value);
        }
        if let Some(m) = store_re.captures(stripped) {
            let _ = m;
            let value = format!(
                "```helen\n{stripped}\n```\nShared store declaration (line {})",
                i + 1
            );
            return Some(value);
        }
        if let Some(m) = var_re.captures(stripped) {
            let type_info = m
                .get(1)
                .map(|g| g.as_str().to_string())
                .unwrap_or_else(|| "inferred".to_string());
            let value = format!(
                "```helen\n{stripped}\n```\nVariable (type: `{type_info}`, line {})",
                i + 1
            );
            return Some(value);
        }
        if let Some(m) = proto_re.captures(stripped) {
            let _ = m;
            let value = format!(
                "```helen\n{stripped}\n```\nProtocol declaration (line {})",
                i + 1
            );
            return Some(value);
        }
    }
    None
}

/// Document symbols (outline view). Python: `_document_symbol`.
pub fn document_symbols(content: &str) -> Vec<Json> {
    let agent_re = regex::Regex::new(r"(@\w+\s+)?agent\s+(\w+)\s*[\({]").unwrap();
    let store_re = regex::Regex::new(r"shared\s+store\s+(\w+)\s*[\({]").unwrap();
    let proto_re = regex::Regex::new(r"protocol\s+(\w+)\s*[\({]").unwrap();
    let fn_re = regex::Regex::new(r"fn\s+(\w+)\s*\(([^)]*)\)\s*(?::\s*(\S+))?\s*\{").unwrap();
    let var_re = regex::Regex::new(r"(?:shared\s+)?(?:let|const|设|定义|常量)\s+(\w+)").unwrap();

    let mut symbols: Vec<Json> = Vec::new();
    // Stack of (indent, index into `symbols`) for nesting (Python agent_stack
    // holds *references* to the same dicts, so mutations must be shared).
    let mut agent_stack: Vec<(usize, usize)> = Vec::new();

    for (i, line) in content.split('\n').enumerate() {
        let stripped = line.trim_start();
        let indent = line.len() - stripped.len();

        // Pop agents exited by indent
        while let Some((top_indent, _)) = agent_stack.last() {
            if *top_indent >= indent && indent > 0 {
                agent_stack.pop();
            } else {
                break;
            }
        }

        // @decorator + agent declaration
        if let Some(caps) = agent_re.captures(stripped) {
            let decorator = caps.get(1).map(|g| g.as_str()).unwrap_or("");
            let name = caps.get(2).map(|g| g.as_str()).unwrap_or("");
            let display = format!("{}agent {name}", decorator.trim())
                .trim()
                .to_string();
            let sel_start = indent + decorator.len() + "agent ".len();
            let sym = symbol_json(
                &display,
                2, // Struct
                i,
                line,
                indent,
                sel_start,
                sel_start + name.len(),
                true,
            );
            symbols.push(sym);
            agent_stack.push((indent, symbols.len() - 1));
            continue;
        }

        // shared store
        if let Some(caps) = store_re.captures(stripped) {
            let name = caps.get(1).map(|g| g.as_str()).unwrap_or("");
            let display = format!("shared store {name}");
            let sel_start = indent + "shared store ".len();
            let sym = symbol_json(
                &display,
                2, // Struct
                i,
                line,
                indent,
                sel_start,
                sel_start + name.len(),
                true,
            );
            symbols.push(sym);
            agent_stack.push((indent, symbols.len() - 1));
            continue;
        }

        // protocol
        if let Some(caps) = proto_re.captures(stripped) {
            let name = caps.get(1).map(|g| g.as_str()).unwrap_or("");
            let display = format!("protocol {name}");
            let sel_start = indent + "protocol ".len();
            let sym = symbol_json(
                &display,
                11, // Interface
                i,
                line,
                indent,
                sel_start,
                sel_start + name.len(),
                false,
            );
            symbols.push(sym);
            continue;
        }

        // fn declaration (top-level or inside agent)
        if let Some(caps) = fn_re.captures(stripped) {
            let name = caps.get(1).map(|g| g.as_str()).unwrap_or("");
            let params = caps.get(2).map(|g| g.as_str()).unwrap_or("");
            let ret_type = caps.get(3).map(|g| g.as_str()).unwrap_or("");
            let mut display = format!("fn {name}({params})");
            if !ret_type.is_empty() {
                display.push_str(&format!(": {ret_type}"));
            }
            let kind = if agent_stack.is_empty() { 12 } else { 6 };
            let sel_start = indent + "fn ".len();
            let sym = symbol_json(
                &display,
                kind,
                i,
                line,
                indent,
                sel_start,
                sel_start + name.len(),
                false,
            );
            if let Some((_, idx)) = agent_stack.last() {
                if let Some(parent) = symbols.get_mut(*idx) {
                    if let Some(children) =
                        parent.get_mut("children").and_then(|c| c.as_array_mut())
                    {
                        children.push(sym);
                    }
                }
            } else {
                symbols.push(sym);
            }
            continue;
        }

        // Variable declarations
        if let Some(caps) = var_re.captures(stripped) {
            let full = caps.get(0).map(|g| g.as_str()).unwrap_or("");
            let name = caps.get(1).map(|g| g.as_str()).unwrap_or("");
            let sel_start = indent + full.len() - name.len();
            let sym = symbol_json(
                name,
                13, // Variable
                i,
                line,
                indent,
                sel_start,
                indent + full.len(),
                false,
            );
            if let Some((_, idx)) = agent_stack.last() {
                if let Some(parent) = symbols.get_mut(*idx) {
                    if let Some(children) =
                        parent.get_mut("children").and_then(|c| c.as_array_mut())
                    {
                        children.push(sym);
                    }
                }
            } else {
                symbols.push(sym);
            }
            continue;
        }
    }

    // Clean internal _indent keys
    fn clean(sym: &mut Json) {
        if let Some(obj) = sym.as_object_mut() {
            obj.remove("_indent");
            if let Some(children) = obj.get_mut("children").and_then(|c| c.as_array_mut()) {
                for child in children {
                    clean(child);
                }
            }
        }
    }
    let mut result = symbols;
    for s in &mut result {
        clean(s);
    }
    result
}

/// Build a symbol dict (range + selectionRange + children + optional _indent).
#[allow(clippy::too_many_arguments)]
fn symbol_json(
    name: &str,
    kind: i64,
    line_idx: usize,
    line: &str,
    indent: usize,
    sel_start: usize,
    sel_end: usize,
    with_indent: bool,
) -> Json {
    let mut m = Map::new();
    m.insert("name".into(), Json::from(name.to_string()));
    m.insert("kind".into(), Json::from(kind));
    let mut range = Map::new();
    range.insert(
        "start".into(),
        Json::Object({
            let mut p = Map::new();
            p.insert("line".into(), Json::from(line_idx as i64));
            p.insert("character".into(), Json::from(0i64));
            p
        }),
    );
    range.insert(
        "end".into(),
        Json::Object({
            let mut p = Map::new();
            p.insert("line".into(), Json::from(line_idx as i64));
            p.insert("character".into(), Json::from(line.len() as i64));
            p
        }),
    );
    m.insert("range".into(), Json::Object(range));
    let mut sel = Map::new();
    sel.insert(
        "start".into(),
        Json::Object({
            let mut p = Map::new();
            p.insert("line".into(), Json::from(line_idx as i64));
            p.insert("character".into(), Json::from(sel_start as i64));
            p
        }),
    );
    sel.insert(
        "end".into(),
        Json::Object({
            let mut p = Map::new();
            p.insert("line".into(), Json::from(line_idx as i64));
            p.insert("character".into(), Json::from(sel_end as i64));
            p
        }),
    );
    m.insert("selectionRange".into(), Json::Object(sel));
    m.insert("children".into(), Json::Array(Vec::new()));
    if with_indent {
        m.insert("_indent".into(), Json::from(indent as i64));
    }
    Json::Object(m)
}

// ── Analysis pipeline ──────────────────────────────────────────────

/// Analyze source code and return diagnostics (lex → parse → analyze).
/// Python: `_analyze`.
pub fn analyze(content: &str, uri: &str) -> Vec<Diagnostic> {
    let file_path = if uri.is_empty() {
        "<lsp>".to_string()
    } else {
        uri_to_path(uri)
    };

    let mut diagnostics: Vec<Diagnostic> = Vec::new();

    // Lex
    let mut scanner = helen_core::lexer::Scanner::new(content, &file_path);
    let tokens = scanner.scan_all();

    // Parse
    let mut parser = helen_parser::pratt::Parser::new(tokens);
    let program = parser.parse();

    // Convert parser errors
    for err in parser.errors() {
        let span = err.span();
        let (start, end) = if span.start_line > 0 {
            (
                Position::new(span.start_line as i64 - 1, span.start_col as i64 - 1),
                Position::new(span.end_line as i64 - 1, span.end_col as i64 - 1),
            )
        } else {
            (Position::new(0, 0), Position::new(0, 1))
        };
        diagnostics.push(Diagnostic {
            range: Range { start, end },
            severity: 1, // Error
            message: err.message().to_string(),
            source: "helen".to_string(),
            code: Some(format!("E{:04}", err.code().value())),
        });
    }

    if parser.errors().is_empty() {
        // Analyze
        let base_dir = if file_path != "<lsp>" {
            std::path::Path::new(&file_path)
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| ".".to_string())
        } else {
            ".".to_string()
        };
        let mut analyzer =
            helen_semantic::SemanticAnalyzer::new(helen_semantic::ErrorReporter::new(), &base_dir);
        analyzer.analyze(&program);

        for d in analyzer.errors.errors() {
            let (start, end) = match &d.span {
                Some(sp) if sp.start_line > 0 => (
                    Position::new(sp.start_line as i64 - 1, sp.start_col as i64 - 1),
                    Position::new(sp.end_line as i64 - 1, sp.end_col as i64 - 1),
                ),
                _ => (Position::new(0, 0), Position::new(0, 1)),
            };
            diagnostics.push(Diagnostic {
                range: Range { start, end },
                severity: 1, // Error
                message: d.message.clone(),
                source: "helen".to_string(),
                code: Some(format!("E{:04}", d.code.value())),
            });
        }
    }

    diagnostics
}

/// Load the embedded stdlib catalog: (builtins[(name, description)], aliases{alias→canonical}).
pub fn stdlib_catalog() -> (
    Vec<(String, String)>,
    std::collections::HashMap<String, String>,
) {
    let raw = helen_interpreter::stdlib_catalog_json();
    let Ok(cat) = serde_json::from_str::<Json>(raw) else {
        return (Vec::new(), std::collections::HashMap::new());
    };
    let mut builtins = Vec::new();
    if let Some(arr) = cat.get("builtins").and_then(|b| b.as_array()) {
        for b in arr {
            let name = b
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let desc = b
                .get("description")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            builtins.push((name, desc));
        }
    }
    let mut aliases = std::collections::HashMap::new();
    if let Some(aliases_obj) = cat.get("aliases").and_then(|a| a.as_object()) {
        for (alias, canonical) in aliases_obj {
            if let Some(c) = canonical.as_str() {
                aliases.insert(alias.clone(), c.to_string());
            }
        }
    }
    (builtins, aliases)
}
