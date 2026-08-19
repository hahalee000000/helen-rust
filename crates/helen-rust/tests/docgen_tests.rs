//! Tests for docgen module — documentation generation.

use helen_rust::docgen::*;

// ── AgentDoc tests ──────────────────────────────────────────────────────

#[test]
fn agent_doc_to_json_minimal() {
    let doc = AgentDoc {
        name: "TestAgent".into(),
        description: None,
        model: None,
        temperature: None,
        max_turns: None,
        params: vec![],
        prompt: None,
        source_file: "".into(),
        line: 0,
    };
    let json = doc.to_json();
    assert_eq!(json["name"], "TestAgent");
}

#[test]
fn agent_doc_to_json_with_description() {
    let doc = AgentDoc {
        name: "Agent1".into(),
        description: Some("A test agent".into()),
        model: None,
        temperature: None,
        max_turns: None,
        params: vec![],
        prompt: None,
        source_file: "".into(),
        line: 0,
    };
    let json = doc.to_json();
    assert_eq!(json["description"], "A test agent");
}

#[test]
fn agent_doc_to_json_with_model() {
    let doc = AgentDoc {
        name: "Agent1".into(),
        description: None,
        model: Some("gpt-4".into()),
        temperature: None,
        max_turns: None,
        params: vec![],
        prompt: None,
        source_file: "".into(),
        line: 0,
    };
    let json = doc.to_json();
    assert_eq!(json["model"], "gpt-4");
}

#[test]
fn agent_doc_to_json_with_temperature() {
    let doc = AgentDoc {
        name: "Agent1".into(),
        description: None,
        model: None,
        temperature: Some(0.7),
        max_turns: None,
        params: vec![],
        prompt: None,
        source_file: "".into(),
        line: 0,
    };
    let json = doc.to_json();
    assert_eq!(json["temperature"], 0.7);
}

#[test]
fn agent_doc_to_json_with_max_turns() {
    let doc = AgentDoc {
        name: "Agent1".into(),
        description: None,
        model: None,
        temperature: None,
        max_turns: Some(10),
        params: vec![],
        prompt: None,
        source_file: "".into(),
        line: 0,
    };
    let json = doc.to_json();
    assert_eq!(json["max_turns"], 10);
}

#[test]
fn agent_doc_to_json_with_params() {
    let doc = AgentDoc {
        name: "Agent1".into(),
        description: None,
        model: None,
        temperature: None,
        max_turns: None,
        params: vec![
            ("input".into(), "str".into()),
            ("count".into(), "int".into()),
        ],
        prompt: None,
        source_file: "".into(),
        line: 0,
    };
    let json = doc.to_json();
    assert_eq!(json["params"].as_array().unwrap().len(), 2);
}

#[test]
fn agent_doc_to_json_with_prompt() {
    let doc = AgentDoc {
        name: "Agent1".into(),
        description: None,
        model: None,
        temperature: None,
        max_turns: None,
        params: vec![],
        prompt: Some("Do something".into()),
        source_file: "".into(),
        line: 0,
    };
    let json = doc.to_json();
    assert_eq!(json["prompt"], "Do something");
}

#[test]
fn agent_doc_to_json_with_source() {
    let doc = AgentDoc {
        name: "Agent1".into(),
        description: None,
        model: None,
        temperature: None,
        max_turns: None,
        params: vec![],
        prompt: None,
        source_file: "test.helen".into(),
        line: 42,
    };
    let json = doc.to_json();
    assert_eq!(json["source_file"], "test.helen");
    assert_eq!(json["line"], 42);
}

// ── FunctionDoc tests ───────────────────────────────────────────────────

#[test]
fn function_doc_to_json_minimal() {
    let doc = FunctionDoc {
        name: "my_func".into(),
        params: vec![],
        source_file: "".into(),
        line: 0,
    };
    let json = doc.to_json();
    assert_eq!(json["name"], "my_func");
    assert_eq!(json["params"].as_array().unwrap().len(), 0);
}

#[test]
fn function_doc_to_json_with_params() {
    let doc = FunctionDoc {
        name: "add".into(),
        params: vec!["a".into(), "b".into()],
        source_file: "math.helen".into(),
        line: 10,
    };
    let json = doc.to_json();
    assert_eq!(json["name"], "add");
    assert_eq!(json["params"].as_array().unwrap().len(), 2);
    assert_eq!(json["source_file"], "math.helen");
    assert_eq!(json["line"], 10);
}

// ── parse_source tests ──────────────────────────────────────────────────

#[test]
fn parse_source_empty() {
    let result = parse_source("", "test.helen");
    assert!(result.is_some());
}

#[test]
fn parse_source_simple_function() {
    let source = "fn add(a, b) { return a + b }";
    let result = parse_source(source, "test.helen");
    assert!(result.is_some());
}

#[test]
fn parse_source_simple_agent() {
    let source = r#"agent MyAgent(input: str) {
        prompt "Do something"
        main {
            let result = llm act
        }
    }"#;
    let result = parse_source(source, "test.helen");
    assert!(result.is_some());
}

// ── generate_docs tests ─────────────────────────────────────────────────

#[test]
fn generate_docs_empty() {
    let docs = generate_docs(&[], false);
    assert!(docs.is_object());
}

#[test]
fn generate_docs_with_source() {
    let sources = vec![
        "fn helper() { return 1 }".to_string(),
    ];
    let docs = generate_docs(&sources, false);
    assert!(docs.is_object());
}

#[test]
fn generate_docs_with_builtins() {
    let docs = generate_docs(&[], true);
    assert!(docs.is_object());
}

// ── format_markdown tests ───────────────────────────────────────────────

#[test]
fn format_markdown_empty() {
    let docs = serde_json::json!({
        "agents": [],
        "functions": [],
        "builtins": []
    });
    let md = format_markdown(&docs);
    assert!(!md.is_empty());
}

#[test]
fn format_markdown_with_agent() {
    let docs = serde_json::json!({
        "agents": [{
            "name": "TestAgent",
            "description": "A test agent",
            "params": []
        }],
        "functions": [],
        "builtins": []
    });
    let md = format_markdown(&docs);
    assert!(md.contains("TestAgent"));
}

#[test]
fn format_markdown_with_function() {
    let docs = serde_json::json!({
        "agents": [],
        "functions": [{
            "name": "helper",
            "params": ["x", "y"]
        }],
        "builtins": []
    });
    let md = format_markdown(&docs);
    assert!(md.contains("helper"));
}
