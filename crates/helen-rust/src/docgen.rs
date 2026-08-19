//! Documentation generator — port of `cli/docgen.py`.
//!
//! Generates API docs from agent/function declarations (via the Rust AST)
//! and the embedded stdlib catalog (M4 Task 4.1), matching the Python
//! `generate_docs` / `format_markdown` output formats.

use helen_core::ast::{AgentDecl, Expr, FunctionDecl, Program, Stmt, TypeRef};
use helen_core::lexer::Scanner;
use helen_core::tokens::LiteralValue;
use helen_parser::Parser;
use serde_json::Value as Json;

/// `AgentDoc` (mirrors `docgen.AgentDoc.to_dict()`).
pub struct AgentDoc {
    pub name: String,
    pub description: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f64>,
    pub max_turns: Option<i64>,
    pub params: Vec<(String, String)>, // (name, type)
    pub prompt: Option<String>,
    pub source_file: String,
    pub line: u32,
}

impl AgentDoc {
    pub fn to_json(&self) -> Json {
        let mut m = serde_json::Map::new();
        m.insert("name".into(), Json::String(self.name.clone()));
        if let Some(d) = &self.description {
            m.insert("description".into(), Json::String(d.clone()));
        }
        if let Some(md) = &self.model {
            m.insert("model".into(), Json::String(md.clone()));
        }
        if let Some(t) = self.temperature {
            m.insert("temperature".into(), Json::from(t));
        }
        if let Some(mt) = self.max_turns {
            m.insert("max_turns".into(), Json::from(mt));
        }
        if !self.params.is_empty() {
            let params: Vec<Json> = self
                .params
                .iter()
                .map(|(n, t)| {
                    let mut pm = serde_json::Map::new();
                    pm.insert("name".into(), Json::String(n.clone()));
                    pm.insert("type".into(), Json::String(t.clone()));
                    Json::Object(pm)
                })
                .collect();
            m.insert("params".into(), Json::Array(params));
        }
        if let Some(p) = &self.prompt {
            m.insert("prompt".into(), Json::String(p.clone()));
        }
        if !self.source_file.is_empty() {
            m.insert("source_file".into(), Json::String(self.source_file.clone()));
            m.insert("line".into(), Json::from(self.line));
        }
        Json::Object(m)
    }
}

/// `FunctionDoc` (mirrors `docgen.FunctionDoc.to_dict()`).
pub struct FunctionDoc {
    pub name: String,
    pub params: Vec<String>,
    pub source_file: String,
    pub line: u32,
}

impl FunctionDoc {
    pub fn to_json(&self) -> Json {
        let mut m = serde_json::Map::new();
        m.insert("name".into(), Json::String(self.name.clone()));
        m.insert(
            "params".into(),
            Json::Array(self.params.iter().cloned().map(Json::String).collect()),
        );
        m.insert("source_file".into(), Json::String(self.source_file.clone()));
        m.insert("line".into(), Json::from(self.line));
        Json::Object(m)
    }
}

/// Extract the type-name string from a `TypeRef` (mirrors `_type_visitor`).
fn type_ref_name(t: &TypeRef) -> String {
    t.name.clone()
}

/// Extract a literal string value from an `Expr` (description/model/prompt).
fn expr_str(e: &Expr) -> Option<String> {
    match e {
        Expr::Literal(lit) => match &lit.value {
            LiteralValue::Str(s) => Some(s.clone()),
            LiteralValue::Int(i) => Some(i.to_string()),
            LiteralValue::Float(f) => Some(format!("{f}")),
            LiteralValue::Bool(b) => Some(b.to_string()),
            LiteralValue::Null => Some("null".to_string()),
        },
        _ => None,
    }
}

/// Extract a float from an `Expr` literal.
fn expr_float(e: &Expr) -> Option<f64> {
    match e {
        Expr::Literal(lit) => match &lit.value {
            LiteralValue::Float(f) => Some(*f),
            LiteralValue::Int(i) => i.to_string().parse::<f64>().ok(),
            _ => None,
        },
        _ => None,
    }
}

/// Extract an int from an `Expr` literal.
fn expr_int(e: &Expr) -> Option<i64> {
    match e {
        Expr::Literal(lit) => match &lit.value {
            LiteralValue::Int(i) => i.to_string().parse::<i64>().ok(),
            _ => None,
        },
        _ => None,
    }
}

/// `extract_agent_doc(node, source_file)`.
pub fn extract_agent_doc(node: &AgentDecl, source_file: &str) -> AgentDoc {
    let mut doc = AgentDoc {
        name: node.name.clone(),
        description: None,
        model: None,
        temperature: None,
        max_turns: None,
        params: Vec::new(),
        prompt: None,
        source_file: source_file.to_string(),
        line: node.span.start_line,
    };

    for p in &node.params {
        let ty = match &p.type_annotation {
            Some(t) => type_ref_name(t),
            None => "any".to_string(),
        };
        doc.params.push((p.name.clone(), ty));
    }

    for d in &node.declarations {
        if let Some(desc) = &d.description {
            doc.description = expr_str(desc);
        }
        if let Some(m) = &d.model {
            doc.model = expr_str(m);
        }
        if let Some(t) = &d.temperature {
            doc.temperature = expr_float(t);
        }
        if let Some(mt) = &d.max_turns {
            doc.max_turns = expr_int(mt);
        }
    }

    if let Some(p) = &node.prompt {
        doc.prompt = Some(p.content.clone());
    }

    doc
}

/// `extract_function_doc(node, source_file)`.
pub fn extract_function_doc(node: &FunctionDecl, source_file: &str) -> FunctionDoc {
    let params = node.params.iter().map(|p| p.name.clone()).collect();
    FunctionDoc {
        name: node.name.clone(),
        params,
        source_file: source_file.to_string(),
        line: node.span.start_line,
    }
}

/// `parse_source(source, source_file)` → Some(program) on success.
pub fn parse_source(source: &str, source_file: &str) -> Option<Program> {
    let mut scanner = Scanner::new(source, source_file);
    let tokens = scanner.scan_all();
    let mut parser = Parser::new(tokens);
    let program = parser.parse();
    if parser.errors().is_empty() {
        Some(program)
    } else {
        None
    }
}

/// `generate_docs(source_files, include_builtins)` → JSON doc object.
pub fn generate_docs(source_files: &[String], include_builtins: bool) -> Json {
    let mut agents: Vec<Json> = Vec::new();
    let mut functions: Vec<Json> = Vec::new();

    for path in source_files {
        let file_path = std::path::Path::new(path);
        if !file_path.exists() {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(file_path) else {
            continue;
        };
        let Some(program) = parse_source(&source, path) else {
            continue;
        };

        for stmt in &program.statements {
            match stmt {
                Stmt::AgentDecl(a) => {
                    agents.push(extract_agent_doc(a, path).to_json());
                }
                Stmt::FunctionDecl(f) => {
                    functions.push(extract_function_doc(f, path).to_json());
                }
                _ => {}
            }
        }
    }

    let mut result = serde_json::Map::new();
    result.insert("agents".into(), Json::Array(agents));
    result.insert("functions".into(), Json::Array(functions));

    if include_builtins {
        result.insert("builtins".into(), builtins_json());
    }

    Json::Object(result)
}

/// Build the `builtins` array from the embedded stdlib catalog + alias map.
fn builtins_json() -> Json {
    let raw = helen_interpreter::stdlib_catalog_json();
    let Ok(cat) = serde_json::from_str::<Json>(raw) else {
        return Json::Array(Vec::new());
    };

    // Build reverse alias map: canonical → [aliases].
    let mut reverse: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    if let Some(aliases) = cat.get("aliases").and_then(|a| a.as_object()) {
        for (alias, canonical) in aliases {
            if let Some(c) = canonical.as_str() {
                reverse
                    .entry(c.to_string())
                    .or_default()
                    .push(alias.clone());
            }
        }
    }

    let mut out: Vec<Json> = Vec::new();
    if let Some(builtins) = cat.get("builtins").and_then(|b| b.as_array()) {
        for b in builtins {
            let name = b
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let description = b
                .get("description")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let signature = b
                .get("signature")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let category = b
                .get("category")
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            let mut m = serde_json::Map::new();
            m.insert("name".into(), Json::String(name.clone()));
            m.insert("description".into(), Json::String(description));
            m.insert("signature".into(), Json::String(signature));
            m.insert("category".into(), Json::String(category));
            let aliases = reverse.get(&name).cloned().unwrap_or_default();
            if !aliases.is_empty() {
                m.insert(
                    "aliases".into(),
                    Json::Array(aliases.into_iter().map(Json::String).collect()),
                );
            }
            out.push(Json::Object(m));
        }
    }
    Json::Array(out)
}

/// `format_markdown(docs)` — port of the Python markdown renderer.
pub fn format_markdown(docs: &Json) -> String {
    let mut lines: Vec<String> = vec![
        "# Helen API Documentation".to_string(),
        String::new(),
        "Auto-generated from source code analysis.".to_string(),
        String::new(),
    ];

    // Agents
    if let Some(agents) = docs.get("agents").and_then(|a| a.as_array()) {
        if !agents.is_empty() {
            lines.push("## Agents".to_string());
            lines.push(String::new());
            for agent in agents {
                let name = agent.get("name").and_then(|n| n.as_str()).unwrap_or("");
                lines.push(format!("### `{name}`"));
                lines.push(String::new());
                if let Some(d) = agent.get("description").and_then(|d| d.as_str()) {
                    lines.push(format!("> {d}"));
                    lines.push(String::new());
                }
                if let Some(params) = agent.get("params").and_then(|p| p.as_array()) {
                    lines.push("**Parameters:**".to_string());
                    lines.push(String::new());
                    for p in params {
                        let pn = p.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        let pt = p.get("type").and_then(|n| n.as_str()).unwrap_or("any");
                        lines.push(format!("- `{pn}`: {pt}"));
                    }
                    lines.push(String::new());
                }
                if let Some(m) = agent.get("model").and_then(|m| m.as_str()) {
                    lines.push(format!("**Model:** {m}"));
                }
                if let Some(sf) = agent.get("source_file").and_then(|s| s.as_str()) {
                    let line = agent.get("line").and_then(|l| l.as_u64()).unwrap_or(0);
                    lines.push(format!("**Source:** `{sf}`:{line}"));
                }
                lines.push(String::new());
            }
        }
    }

    // Functions
    if let Some(functions) = docs.get("functions").and_then(|f| f.as_array()) {
        if !functions.is_empty() {
            lines.push("## Functions".to_string());
            lines.push(String::new());
            for func in functions {
                let name = func.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let params: Vec<String> = func
                    .get("params")
                    .and_then(|p| p.as_array())
                    .map(|arr| {
                        arr.iter()
                            .map(|x| x.as_str().unwrap_or("").to_string())
                            .collect()
                    })
                    .unwrap_or_default();
                lines.push(format!("### `{name}({})`", params.join(", ")));
                lines.push(String::new());
                if let Some(sf) = func.get("source_file").and_then(|s| s.as_str()) {
                    let line = func.get("line").and_then(|l| l.as_u64()).unwrap_or(0);
                    lines.push(format!("**Source:** `{sf}`:{line}"));
                }
                lines.push(String::new());
            }
        }
    }

    // Builtins — grouped by category, sorted by name.
    if let Some(builtins) = docs.get("builtins").and_then(|b| b.as_array()) {
        if !builtins.is_empty() {
            let mut categories: std::collections::BTreeMap<String, Vec<&Json>> =
                std::collections::BTreeMap::new();
            for b in builtins {
                let cat = b
                    .get("category")
                    .and_then(|c| c.as_str())
                    .unwrap_or("core")
                    .to_string();
                categories.entry(cat).or_default().push(b);
            }

            for (cat_name, items) in &categories {
                lines.push(format!("## Built-in Functions ({cat_name})"));
                lines.push(String::new());
                lines.push("| Function | Signature | Description |".to_string());
                lines.push("|----------|-----------|-------------|".to_string());
                let mut sorted = items.clone();
                sorted.sort_by_key(|b| {
                    b.get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_string()
                });
                for b in sorted {
                    let name = b.get("name").and_then(|n| n.as_str()).unwrap_or("");
                    let sig = b.get("signature").and_then(|n| n.as_str()).unwrap_or("");
                    let desc = b.get("description").and_then(|n| n.as_str()).unwrap_or("");
                    let mut name_cell = format!("`{name}`");
                    let aliases: Vec<&str> = b
                        .get("aliases")
                        .and_then(|a| a.as_array())
                        .map(|arr| arr.iter().filter_map(|x| x.as_str()).collect())
                        .unwrap_or_default();
                    if !aliases.is_empty() {
                        let shown = &aliases[..aliases.len().min(3)];
                        let alias_str = shown
                            .iter()
                            .map(|a| format!("`{a}`"))
                            .collect::<Vec<_>>()
                            .join(", ");
                        let more = if aliases.len() > 3 {
                            format!(" +{} more", aliases.len() - 3)
                        } else {
                            String::new()
                        };
                        name_cell += &format!(" <br><sup>aka {alias_str}{more}</sup>");
                    }
                    lines.push(format!("| {name_cell} | `{sig}` | {desc} |"));
                }
                lines.push(String::new());
            }
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_docs_empty() {
        let docs = generate_docs(&[], true);
        assert!(docs.get("agents").is_some());
        assert!(docs.get("functions").is_some());
        assert!(docs.get("builtins").is_some());
    }

    #[test]
    fn test_generate_docs_nonexistent_file() {
        let docs = generate_docs(&["/nonexistent/file.helen".to_string()], false);
        assert_eq!(docs["agents"].as_array().expect("array exists").len(), 0);
        assert_eq!(docs["functions"].as_array().expect("array exists").len(), 0);
    }

    #[test]
    fn test_generate_docs_with_agent() {
        let code = r#"
agent Greeter {
    description "A friendly greeter"
    model "gpt-4"

    prompt "You greet people"

    main {
        let msg = "Hello"
    }
}
"#;
        let dir = std::env::temp_dir();
        let path = dir.join("m12_docgen_agent.helen");
        std::fs::write(&path, code).expect("write file");
        let docs = generate_docs(&[path.to_str().expect("to_str").to_string()], false);
        std::fs::remove_file(&path).ok();
        let agents = docs["agents"].as_array().expect("array exists");
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0]["name"], "Greeter");
        assert_eq!(agents[0]["description"], "A friendly greeter");
        assert_eq!(agents[0]["model"], "gpt-4");
    }

    #[test]
    fn test_generate_docs_with_function() {
        let code = r#"
fn greet(name: string) {
    let msg = "Hello, " + name
}
"#;
        let dir = std::env::temp_dir();
        let path = dir.join("m12_docgen_fn.helen");
        std::fs::write(&path, code).expect("write file");
        let docs = generate_docs(&[path.to_str().expect("to_str").to_string()], false);
        std::fs::remove_file(&path).ok();
        let functions = docs["functions"].as_array().expect("array exists");
        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0]["name"], "greet");
        assert!(!functions[0]["params"].as_array().expect("array exists").is_empty());
    }

    #[test]
    fn test_generate_docs_no_builtins() {
        let docs = generate_docs(&[], false);
        assert!(docs.get("builtins").is_none());
    }

    #[test]
    fn test_generate_docs_with_builtins() {
        let docs = generate_docs(&[], true);
        let builtins = docs["builtins"].as_array().expect("array exists");
        assert!(!builtins.is_empty());
        let names: Vec<&str> = builtins
            .iter()
            .filter_map(|b| b.get("name").and_then(|n| n.as_str()))
            .collect();
        assert!(names.contains(&"print"), "{names:?}");
        assert!(names.contains(&"len"));
        assert!(names.contains(&"upper"));
        assert!(names.contains(&"sqrt"));
    }

    #[test]
    fn test_markdown_header() {
        let docs = generate_docs(&[], false);
        let md = format_markdown(&docs);
        assert!(md.contains("# Helen API Documentation"));
    }

    #[test]
    fn test_markdown_agents_section() {
        let code = r#"
agent TestAgent {
    description "Test agent"

    main {
        let x = 1
    }
}
"#;
        let dir = std::env::temp_dir();
        let path = dir.join("m12_docgen_md.helen");
        std::fs::write(&path, code).expect("write file");
        let docs = generate_docs(&[path.to_str().expect("to_str").to_string()], false);
        std::fs::remove_file(&path).ok();
        let md = format_markdown(&docs);
        assert!(md.contains("## Agents"));
        assert!(md.contains("### `TestAgent`"));
        assert!(md.contains("Test agent"));
    }

    #[test]
    fn test_markdown_builtins_section() {
        let docs = generate_docs(&[], true);
        let md = format_markdown(&docs);
        assert!(md.contains("Built-in Functions"));
        assert!(md.contains("| Function |"));
    }
}
