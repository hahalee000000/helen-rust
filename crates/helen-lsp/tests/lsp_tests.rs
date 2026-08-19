//! LSP server tests — port of `helen/tests/lsp/test_server.py` (M12).

use helen_lsp::constants::*;
use helen_lsp::server::*;
use serde_json::json;
use serde_json::Value as Json;

fn doc(server: &mut HelenLanguageServer, content: &str) -> String {
    let uri = "file:///test.helen";
    server
        .documents
        .insert(uri.to_string(), DocumentState::new(uri, content, 1));
    uri.to_string()
}

// ── Data structures ────────────────────────────────────────────────

#[test]
fn position_to_dict() {
    let pos = Position::new(5, 10);
    assert_eq!(pos.to_dict(), json!({"line": 5, "character": 10}));
}

#[test]
fn range_to_dict() {
    let r = Range {
        start: Position::new(0, 0),
        end: Position::new(0, 5),
    };
    assert_eq!(
        r.to_dict(),
        json!({"start": {"line": 0, "character": 0}, "end": {"line": 0, "character": 5}})
    );
}

#[test]
fn diagnostic_to_dict() {
    let d = Diagnostic {
        range: Range {
            start: Position::new(1, 0),
            end: Position::new(1, 5),
        },
        severity: 1,
        message: "test error".to_string(),
        source: "helen".to_string(),
        code: Some("E0301".to_string()),
    };
    let r = d.to_dict();
    assert_eq!(r["severity"], 1);
    assert_eq!(r["message"], "test error");
    assert_eq!(r["code"], "E0301");
    assert_eq!(r["source"], "helen");
}

#[test]
fn diagnostic_without_code_omits_it() {
    let d = Diagnostic {
        range: Range {
            start: Position::new(0, 0),
            end: Position::new(0, 1),
        },
        severity: 1,
        message: "error".to_string(),
        source: "helen".to_string(),
        code: None,
    };
    assert!(d.to_dict().get("code").is_none());
}

#[test]
fn completion_item_to_dict() {
    let item = CompletionItem::keyword("agent");
    let r = item.to_dict();
    assert_eq!(r["label"], "agent");
    assert_eq!(r["kind"], 14);
    assert_eq!(r["detail"], "Helen keyword");
}

#[test]
fn location_to_dict() {
    let loc = Location {
        uri: "file:///test.helen".to_string(),
        range: Range {
            start: Position::new(0, 0),
            end: Position::new(0, 5),
        },
    };
    let r = loc.to_dict();
    assert_eq!(r["uri"], "file:///test.helen");
    assert!(r.get("range").is_some());
}

// ── Initialize ─────────────────────────────────────────────────────

#[test]
fn initialize_returns_capabilities() {
    let mut server = HelenLanguageServer::new();
    let result = server.initialize(&Json::Null);
    assert!(result.get("capabilities").is_some());
    assert!(result.get("serverInfo").is_some());
    assert_eq!(result["serverInfo"]["name"], "helen-lsp");
}

#[test]
fn capabilities_include_sync() {
    let server = HelenLanguageServer::new();
    let caps = &server.capabilities;
    assert_eq!(caps["textDocumentSync"], 1);
    assert_eq!(caps["hoverProvider"], true);
    assert_eq!(caps["documentSymbolProvider"], true);
    assert!(caps.get("completionProvider").is_some());
    assert_eq!(caps["definitionProvider"], true);
    assert_eq!(caps["referencesProvider"], true);
}

// ── Document lifecycle ─────────────────────────────────────────────

#[test]
fn did_open_registers_document() {
    let mut server = HelenLanguageServer::new();
    server.did_open(&json!({
        "textDocument": {"uri": "file:///test.helen", "text": "const X = 1", "version": 1}
    }));
    let d = server.documents.get("file:///test.helen").unwrap();
    assert_eq!(d.content, "const X = 1");
    assert_eq!(d.version, 1);
}

#[test]
fn did_change_updates_content() {
    let mut server = HelenLanguageServer::new();
    server.did_open(&json!({
        "textDocument": {"uri": "file:///test.helen", "text": "const X = 1", "version": 1}
    }));
    server.did_change(&json!({
        "textDocument": {"uri": "file:///test.helen", "version": 2},
        "contentChanges": [{"text": "let x = 2"}]
    }));
    let d = server.documents.get("file:///test.helen").unwrap();
    assert_eq!(d.content, "let x = 2");
    assert_eq!(d.version, 2);
}

#[test]
fn did_close_removes_document() {
    let mut server = HelenLanguageServer::new();
    server.did_open(&json!({
        "textDocument": {"uri": "file:///test.helen", "text": "const X = 1", "version": 1}
    }));
    server.did_close(&json!({"textDocument": {"uri": "file:///test.helen"}}));
    assert!(!server.documents.contains_key("file:///test.helen"));
}

// ── Completion ─────────────────────────────────────────────────────

#[test]
fn completion_includes_keywords() {
    let mut server = HelenLanguageServer::new();
    let uri = doc(&mut server, "");
    let result = server.completion(&json!({
        "textDocument": {"uri": uri},
        "position": {"line": 0, "character": 0}
    }));
    let items = result["items"].as_array().unwrap();
    let labels: std::collections::HashSet<&str> =
        items.iter().filter_map(|i| i["label"].as_str()).collect();
    for kw in HELLEN_KEYWORDS {
        assert!(labels.contains(kw), "missing keyword: {kw}");
    }
}

#[test]
fn completion_includes_types() {
    let mut server = HelenLanguageServer::new();
    let uri = doc(&mut server, "");
    let result = server.completion(&json!({
        "textDocument": {"uri": uri},
        "position": {"line": 0, "character": 0}
    }));
    let items = result["items"].as_array().unwrap();
    let labels: std::collections::HashSet<&str> =
        items.iter().filter_map(|i| i["label"].as_str()).collect();
    for t in HELLEN_TYPES {
        assert!(labels.contains(t), "missing type: {t}");
    }
}

#[test]
fn completion_includes_builtins() {
    let mut server = HelenLanguageServer::new();
    let uri = doc(&mut server, "");
    let result = server.completion(&json!({
        "textDocument": {"uri": uri},
        "position": {"line": 0, "character": 0}
    }));
    let items = result["items"].as_array().unwrap();
    let labels: std::collections::HashSet<&str> =
        items.iter().filter_map(|i| i["label"].as_str()).collect();
    assert!(labels.contains("print"), "print missing from completion");
    assert!(labels.contains("len"), "len missing from completion");
}

#[test]
fn completion_for_unknown_doc_returns_empty() {
    let mut server = HelenLanguageServer::new();
    let result = server.completion(&json!({
        "textDocument": {"uri": "file:///unknown.helen"},
        "position": {"line": 0, "character": 0}
    }));
    assert_eq!(result["items"].as_array().unwrap().len(), 0);
}

// ── Definition ─────────────────────────────────────────────────────

#[test]
fn definition_finds_agent() {
    let content = "agent Greeter {\n    main { let x = 1 }\n}";
    // Click on "Greeter" on line 0, col 7 (1-based: line=1, col=7)
    let result = find_definition_at(content, "file:///test.helen", 1, 7);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["uri"], "file:///test.helen");
}

#[test]
fn definition_finds_function() {
    let content = "fn greet(name) {\n    let msg = name\n}";
    let result = find_definition_at(content, "file:///test.helen", 2, 12);
    assert_eq!(result.len(), 1);
}

#[test]
fn definition_finds_variable() {
    let content = "let x = 1\nlet y = x + 1";
    let result = find_definition_at(content, "file:///test.helen", 2, 5);
    assert_eq!(result.len(), 1);
}

#[test]
fn definition_not_found() {
    let content = "const X = 1";
    let result = find_definition_at(content, "file:///test.helen", 1, 1);
    assert_eq!(result.len(), 0);
}

#[test]
fn definition_empty_document() {
    let mut server = HelenLanguageServer::new();
    let result = server.definition(&json!({
        "textDocument": {"uri": "file:///unknown.helen"},
        "position": {"line": 0, "character": 0}
    }));
    assert_eq!(result.as_array().unwrap().len(), 0);
}

// ── Diagnostics ────────────────────────────────────────────────────

#[test]
fn analyze_valid_code_no_errors() {
    let diagnostics = analyze("const X = 1", "");
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn analyze_invalid_code_has_errors() {
    let diagnostics = analyze("agent {", "");
    assert!(!diagnostics.is_empty());
    assert!(diagnostics.iter().all(|d| d.severity == 1));
}

#[test]
fn analyze_empty_code_no_errors() {
    let diagnostics = analyze("", "");
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn diagnostic_has_error_code() {
    let diagnostics = analyze("agent {", "");
    if !diagnostics.is_empty() {
        assert!(diagnostics[0].code.is_some());
    }
}

// ── Message handling ───────────────────────────────────────────────

#[test]
fn handle_initialize_request() {
    let mut server = HelenLanguageServer::new();
    let response = server.handle_message(&json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}
    }));
    assert!(response.is_some());
    let r = response.unwrap();
    assert_eq!(r["id"], 1);
    assert!(r.get("result").unwrap().get("capabilities").is_some());
}

#[test]
fn handle_shutdown_request() {
    let mut server = HelenLanguageServer::new();
    let response = server.handle_message(&json!({
        "jsonrpc": "2.0", "id": 2, "method": "shutdown", "params": {}
    }));
    assert!(response.is_some());
    let r = response.unwrap();
    assert!(r["result"].is_null());
}

#[test]
fn handle_unknown_method() {
    let mut server = HelenLanguageServer::new();
    let response = server.handle_message(&json!({
        "jsonrpc": "2.0", "id": 3, "method": "unknown/method", "params": {}
    }));
    assert!(response.is_some());
    assert!(response.unwrap()["result"].is_null());
}

#[test]
fn handle_notification_no_response() {
    let mut server = HelenLanguageServer::new();
    let response = server.handle_message(&json!({
        "jsonrpc": "2.0", "method": "initialized", "params": {}
    }));
    assert!(response.is_none());
}

// ── URI → path ─────────────────────────────────────────────────────

#[test]
fn file_uri_to_path() {
    assert_eq!(
        uri_to_path("file:///tmp/helen_test/main.helen"),
        "/tmp/helen_test/main.helen"
    );
}

#[test]
fn file_uri_with_encoded_chars() {
    assert_eq!(
        uri_to_path("file:///home/user/my%20project/test.helen"),
        "/home/user/my project/test.helen"
    );
}

#[test]
fn plain_path_passthrough() {
    assert_eq!(
        uri_to_path("/tmp/plain/path.helen"),
        "/tmp/plain/path.helen"
    );
}

// ── Keywords coverage ──────────────────────────────────────────────

#[test]
fn context_keywords_present() {
    let expected = [
        "Channel",
        "send",
        "receive",
        "try_receive",
        "cancel",
        "close",
        "mailbox_select",
        "on_chunk",
        "on_complete",
        "on_tool_end",
        "on_media",
        "on_generate",
        "media",
        "provider",
        "context",
        "memory",
        "resume",
        "expect",
    ];
    for kw in expected {
        assert!(
            HELLEN_CONTEXT_KEYWORDS.contains(&kw),
            "context keyword missing: {kw}"
        );
    }
}

#[test]
fn chinese_formal_keywords_present() {
    let expected_cn = [
        "智能体",
        "大模型",
        "执行",
        "分生",
        "设",
        "定义",
        "常量",
        "函数",
        "返回",
        "如果",
        "否则",
        "对于",
        "属于",
        "当",
        "中断",
        "继续",
        "匹配",
        "情况",
        "默认",
        "分支",
        "尝试",
        "捕获",
        "最终",
        "抛出",
        "断言",
        "真",
        "假",
        "空",
        "是",
        "提示词",
        "描述",
        "模型",
        "工具",
        "流式输出",
        "温度",
        "最大轮次",
        "函数区",
        "主函",
        "导入",
        "作为",
        "协议",
        "实现",
        "共享",
        "别名",
        "仓库",
        "记录",
    ];
    for kw in expected_cn {
        assert!(
            HELLEN_KEYWORDS.contains(&kw),
            "Chinese keyword missing: {kw}"
        );
    }
}

#[test]
fn keyword_descriptions_cover_keywords() {
    let all: Vec<&str> = HELLEN_KEYWORDS
        .iter()
        .chain(HELLEN_CONTEXT_KEYWORDS.iter())
        .copied()
        .collect();
    for kw in all {
        assert!(
            keyword_description(kw).is_some(),
            "keyword without hover description: {kw}"
        );
    }
}

// ── Snippets ───────────────────────────────────────────────────────

#[test]
fn key_snippets_exist() {
    let labels: std::collections::HashSet<&str> = HELLEN_SNIPPETS.iter().map(|s| s.label).collect();
    let expected = [
        "agent",
        "fn",
        "llm act",
        "llm if",
        "shared store",
        "spawn",
        "match",
        "try",
        "for",
        "while",
        "import",
        "protocol",
        "@sandbox",
        "@open",
        "@strict",
    ];
    for label in expected {
        assert!(labels.contains(label), "snippet missing: {label}");
    }
}

// ── Hover ──────────────────────────────────────────────────────────

#[test]
fn hover_on_keyword() {
    let mut server = HelenLanguageServer::new();
    let uri = doc(&mut server, "agent Test {\n  main { }\n}\n");
    let result = server.hover(&json!({
        "textDocument": {"uri": uri},
        "position": {"line": 0, "character": 2}
    }));
    assert!(result.is_some());
    let v = result.unwrap()["contents"]["value"]
        .as_str()
        .unwrap()
        .to_lowercase();
    assert!(v.contains("agent"));
}

#[test]
fn hover_on_user_function() {
    let mut server = HelenLanguageServer::new();
    let uri = doc(
        &mut server,
        "fn greet(name: str): str {\n  return name\n}\n",
    );
    let result = server.hover(&json!({
        "textDocument": {"uri": uri},
        "position": {"line": 0, "character": 4}
    }));
    assert!(result.is_some());
    let v = result.unwrap()["contents"]["value"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(v.contains("greet"));
}

#[test]
fn hover_on_agent_declaration() {
    let mut server = HelenLanguageServer::new();
    let uri = doc(
        &mut server,
        "agent MyBot {\n  description \"test\"\n  main { }\n}\n",
    );
    let result = server.hover(&json!({
        "textDocument": {"uri": uri},
        "position": {"line": 0, "character": 8}
    }));
    assert!(result.is_some());
    let v = result.unwrap()["contents"]["value"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(v.contains("MyBot"));
    assert!(v.contains("Agent"));
}

#[test]
fn hover_unknown_symbol_returns_none() {
    let mut server = HelenLanguageServer::new();
    let uri = doc(&mut server, "let x = 42\n");
    let result = server.hover(&json!({
        "textDocument": {"uri": uri},
        "position": {"line": 0, "character": 20}
    }));
    assert!(result.is_none());
}

// ── Document symbols ───────────────────────────────────────────────

#[test]
fn symbols_includes_agent() {
    let mut server = HelenLanguageServer::new();
    let uri = doc(
        &mut server,
        "agent MyAgent {\n  description \"test\"\n  main { }\n}\n",
    );
    let result = server.document_symbol(&json!({"textDocument": {"uri": uri}}));
    let names: Vec<String> = result
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["name"].as_str().map(|n| n.to_string()))
        .collect();
    assert!(names.iter().any(|n| n.contains("MyAgent")));
}

#[test]
fn symbols_includes_function() {
    let mut server = HelenLanguageServer::new();
    let uri = doc(&mut server, "fn helper(): void {\n  return\n}\n");
    let result = server.document_symbol(&json!({"textDocument": {"uri": uri}}));
    let names: Vec<String> = result
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["name"].as_str().map(|n| n.to_string()))
        .collect();
    assert!(names.iter().any(|n| n.contains("helper")));
}

#[test]
fn symbols_includes_variable() {
    let mut server = HelenLanguageServer::new();
    let uri = doc(&mut server, "let counter = 0\n");
    let result = server.document_symbol(&json!({"textDocument": {"uri": uri}}));
    let names: Vec<String> = result
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["name"].as_str().map(|n| n.to_string()))
        .collect();
    assert!(names.iter().any(|n| n == "counter"));
}

#[test]
fn symbols_nested_in_agent() {
    let content = "agent Bot {\n  description \"bot\"\n  functions {\n    fn tool_call(): void { return }\n  }\n  main { }\n}\n";
    let mut server = HelenLanguageServer::new();
    let uri = doc(&mut server, content);
    let result = server.document_symbol(&json!({"textDocument": {"uri": uri}}));
    let arr = result.as_array().unwrap();
    let agent_syms: Vec<&Json> = arr
        .iter()
        .filter(|s| {
            s["name"]
                .as_str()
                .map(|n| n.contains("Bot"))
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(agent_syms.len(), 1);
    let children = agent_syms[0]["children"].as_array().unwrap();
    let child_names: Vec<String> = children
        .iter()
        .filter_map(|c| c["name"].as_str().map(|n| n.to_string()))
        .collect();
    assert!(child_names.iter().any(|n| n.contains("tool_call")));
}

#[test]
fn symbols_empty_document() {
    let mut server = HelenLanguageServer::new();
    let uri = doc(&mut server, "");
    let result = server.document_symbol(&json!({"textDocument": {"uri": uri}}));
    assert_eq!(result.as_array().unwrap().len(), 0);
}

#[test]
fn symbols_decorated_agent() {
    let mut server = HelenLanguageServer::new();
    let uri = doc(&mut server, "@sandbox agent SafeBot {\n  main { }\n}\n");
    let result = server.document_symbol(&json!({"textDocument": {"uri": uri}}));
    let names: Vec<String> = result
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["name"].as_str().map(|n| n.to_string()))
        .collect();
    assert!(names
        .iter()
        .any(|n| n.contains("@sandbox") && n.contains("SafeBot")));
}

// ── Additional LSP tests for coverage ───────────────────────────────

#[test]
fn position_new() {
    let pos = Position::new(10, 20);
    assert_eq!(pos.line, 10);
    assert_eq!(pos.character, 20);
}

#[test]
fn range_start_end() {
    let r = Range {
        start: Position::new(0, 0),
        end: Position::new(5, 10),
    };
    assert_eq!(r.start.line, 0);
    assert_eq!(r.start.character, 0);
    assert_eq!(r.end.line, 5);
    assert_eq!(r.end.character, 10);
}

#[test]
fn document_state_new() {
    let state = DocumentState::new("file:///test.helen", "let x = 1", 1);
    assert_eq!(state.uri, "file:///test.helen");
    assert_eq!(state.content, "let x = 1");
    assert_eq!(state.version, 1);
}

#[test]
fn server_new_creates_empty_state() {
    let server = HelenLanguageServer::new();
    assert!(server.documents.is_empty());
    assert!(server.capabilities.is_object());
}

#[test]
fn did_open_multiple_documents() {
    let mut server = HelenLanguageServer::new();
    server.did_open(&json!({
        "textDocument": {"uri": "file:///a.helen", "text": "let a = 1", "version": 1}
    }));
    server.did_open(&json!({
        "textDocument": {"uri": "file:///b.helen", "text": "let b = 2", "version": 1}
    }));
    assert_eq!(server.documents.len(), 2);
    assert!(server.documents.contains_key("file:///a.helen"));
    assert!(server.documents.contains_key("file:///b.helen"));
}

#[test]
fn did_change_incremental() {
    let mut server = HelenLanguageServer::new();
    server.did_open(&json!({
        "textDocument": {"uri": "file:///test.helen", "text": "let x = 1", "version": 1}
    }));
    server.did_change(&json!({
        "textDocument": {"uri": "file:///test.helen", "version": 2},
        "contentChanges": [{"text": "let x = 2"}]
    }));
    server.did_change(&json!({
        "textDocument": {"uri": "file:///test.helen", "version": 3},
        "contentChanges": [{"text": "let x = 3"}]
    }));
    let d = server.documents.get("file:///test.helen").unwrap();
    assert_eq!(d.content, "let x = 3");
    assert_eq!(d.version, 3);
}

#[test]
fn completion_empty_document() {
    let mut server = HelenLanguageServer::new();
    let uri = doc(&mut server, "");
    let result = server.completion(&json!({
        "textDocument": {"uri": uri},
        "position": {"line": 0, "character": 0}
    }));
    let items = result["items"].as_array().unwrap();
    assert!(!items.is_empty());
}

#[test]
fn definition_multiple_variables() {
    let content = "let x = 1\nlet y = x\nlet z = x + y";
    let result = find_definition_at(content, "file:///test.helen", 3, 9);
    assert!(!result.is_empty());
}

#[test]
fn definition_nested_function() {
    let content = "fn outer() {\n    fn inner() { return 1 }\n    return inner()\n}";
    let result = find_definition_at(content, "file:///test.helen", 3, 12);
    assert!(!result.is_empty());
}

#[test]
fn hover_on_constant() {
    let mut server = HelenLanguageServer::new();
    let uri = doc(&mut server, "const PI = 3.14159\nlet x = PI");
    let result = server.hover(&json!({
        "textDocument": {"uri": uri},
        "position": {"line": 1, "character": 9}
    }));
    assert!(result.is_some());
}

#[test]
fn symbols_multiple_functions() {
    let mut server = HelenLanguageServer::new();
    let uri = doc(
        &mut server,
        "fn foo() { return 1 }\nfn bar() { return 2 }\nfn baz() { return 3 }",
    );
    let result = server.document_symbol(&json!({"textDocument": {"uri": uri}}));
    let names: Vec<String> = result
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["name"].as_str().map(|n| n.to_string()))
        .collect();
    assert!(names.iter().any(|n| n.contains("foo")));
    assert!(names.iter().any(|n| n.contains("bar")));
    assert!(names.iter().any(|n| n.contains("baz")));
}

#[test]
fn symbols_nested_agent_functions() {
    let mut server = HelenLanguageServer::new();
    let uri = doc(
        &mut server,
        "agent MyBot {\n  fn helper() { return 1 }\n  main { return helper() }\n}",
    );
    let result = server.document_symbol(&json!({"textDocument": {"uri": uri}}));
    let names: Vec<String> = result
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|s| s["name"].as_str().map(|n| n.to_string()))
        .collect();
    assert!(names.iter().any(|n| n.contains("MyBot")));
    // Note: nested fn inside agent may or may not appear as separate symbol
    // depending on LSP implementation — just verify the agent is found
}

#[test]
fn references_finds_all_uses() {
    let content = "let x = 1\nlet y = x + 1\nlet z = x * 2";
    let result = find_references_in(content, "file:///test.helen", "x", true);
    assert!(result.len() >= 2);
}

#[test]
fn handle_initialize_with_params() {
    let mut server = HelenLanguageServer::new();
    let response = server.handle_message(&json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": {"rootUri": "file:///project", "capabilities": {}}
    }));
    assert!(response.is_some());
    let r = response.unwrap();
    assert!(r["result"]["capabilities"].is_object());
}

#[test]
fn handle_text_document_did_open() {
    let mut server = HelenLanguageServer::new();
    let response = server.handle_message(&json!({
        "jsonrpc": "2.0", "method": "textDocument/didOpen",
        "params": {"textDocument": {"uri": "file:///test.helen", "text": "let x = 1", "version": 1}}
    }));
    assert!(response.is_none());
    assert!(server.documents.contains_key("file:///test.helen"));
}

#[test]
fn handle_completion_request() {
    let mut server = HelenLanguageServer::new();
    server.did_open(&json!({
        "textDocument": {"uri": "file:///test.helen", "text": "", "version": 1}
    }));
    let response = server.handle_message(&json!({
        "jsonrpc": "2.0", "id": 2, "method": "textDocument/completion",
        "params": {"textDocument": {"uri": "file:///test.helen"}, "position": {"line": 0, "character": 0}}
    }));
    assert!(response.is_some());
    let r = response.unwrap();
    assert!(r["result"]["items"].is_array());
}

#[test]
fn handle_hover_request() {
    let mut server = HelenLanguageServer::new();
    server.did_open(&json!({
        "textDocument": {"uri": "file:///test.helen", "text": "agent Test { main {} }", "version": 1}
    }));
    let response = server.handle_message(&json!({
        "jsonrpc": "2.0", "id": 3, "method": "textDocument/hover",
        "params": {"textDocument": {"uri": "file:///test.helen"}, "position": {"line": 0, "character": 0}}
    }));
    assert!(response.is_some());
}

#[test]
fn handle_definition_request() {
    let mut server = HelenLanguageServer::new();
    server.did_open(&json!({
        "textDocument": {"uri": "file:///test.helen", "text": "let x = 1\nlet y = x", "version": 1}
    }));
    let response = server.handle_message(&json!({
        "jsonrpc": "2.0", "id": 4, "method": "textDocument/definition",
        "params": {"textDocument": {"uri": "file:///test.helen"}, "position": {"line": 1, "character": 8}}
    }));
    assert!(response.is_some());
}

#[test]
fn handle_document_symbol_request() {
    let mut server = HelenLanguageServer::new();
    server.did_open(&json!({
        "textDocument": {"uri": "file:///test.helen", "text": "fn test() { return 1 }", "version": 1}
    }));
    let response = server.handle_message(&json!({
        "jsonrpc": "2.0", "id": 5, "method": "textDocument/documentSymbol",
        "params": {"textDocument": {"uri": "file:///test.helen"}}
    }));
    assert!(response.is_some());
    let r = response.unwrap();
    assert!(r["result"].is_array());
}

#[test]
fn handle_references_request() {
    let mut server = HelenLanguageServer::new();
    server.did_open(&json!({
        "textDocument": {"uri": "file:///test.helen", "text": "let x = 1\nlet y = x", "version": 1}
    }));
    let response = server.handle_message(&json!({
        "jsonrpc": "2.0", "id": 6, "method": "textDocument/references",
        "params": {
            "textDocument": {"uri": "file:///test.helen"},
            "position": {"line": 0, "character": 4},
            "context": {"includeDeclaration": true}
        }
    }));
    assert!(response.is_some());
}

#[test]
fn keyword_description_returns_some_for_known() {
    assert!(keyword_description("agent").is_some());
    assert!(keyword_description("fn").is_some());
    assert!(keyword_description("let").is_some());
    assert!(keyword_description("const").is_some());
}

#[test]
fn keyword_description_returns_none_for_unknown() {
    assert!(keyword_description("unknown_keyword").is_none());
    assert!(keyword_description("xyz").is_none());
}

#[test]
fn snippet_has_insert_text() {
    let snippet = CompletionItem::snippet("agent", "Define an agent", "agent ${1:Name} {\n  $0\n}");
    let dict = snippet.to_dict();
    assert!(dict.get("insertText").is_some());
    assert!(dict.get("insertTextFormat").is_some());
}

#[test]
fn completion_item_with_detail() {
    let mut item = CompletionItem::keyword("agent");
    item.detail = Some("Define an agent".to_string());
    let dict = item.to_dict();
    assert_eq!(dict["detail"], "Define an agent");
}

#[test]
fn location_range_properties() {
    let loc = Location {
        uri: "file:///test.helen".to_string(),
        range: Range {
            start: Position::new(1, 0),
            end: Position::new(5, 10),
        },
    };
    let dict = loc.to_dict();
    assert_eq!(dict["uri"], "file:///test.helen");
    assert_eq!(dict["range"]["start"]["line"], 1);
    assert_eq!(dict["range"]["end"]["character"], 10);
}
