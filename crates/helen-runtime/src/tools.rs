//! Tool registry (Task 6.3) — port of `helen/runtime/tools.py`.
//!
//! Registers the 11 built-in tools with byte-identical OpenAI-format schemas
//! and Python-parity handler results:
//!   web_search, web_fetch, read_file, write_file, shell_exec, calculate,
//!   patch_file, find_files, search_files, load_skill, list_skill_references.

use crate::mcp::MCPToolRegistry;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

pub mod fuzzy {
    pub use crate::fuzzy_match::*;
}
pub mod skills {
    pub use crate::skills::*;
}

/// Global MCP tool registry (Python `_mcp_registry` in tools.py).
static MCP_REGISTRY: OnceLock<Mutex<Option<MCPToolRegistry>>> = OnceLock::new();

fn mcp_registry() -> &'static Mutex<Option<MCPToolRegistry>> {
    MCP_REGISTRY.get_or_init(|| Mutex::new(None))
}

/// Registered tool: name + OpenAI schema + handler.
pub struct HelenTool {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: Value,
    pub handler: fn(&Value) -> String,
}

/// Get the OpenAI-format schema for a tool (Python `get_tool_schemas([name])`).
pub fn tool_schema(tool: &HelenTool) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.parameters,
        }
    })
}

/// Look up a tool by name.
pub fn get_tool(name: &str) -> Option<HelenTool> {
    all_tools().into_iter().find(|t| t.name == name)
}

/// All 11 built-in tools (Python `_register_builtin_tools` order).
pub fn all_tools() -> Vec<HelenTool> {
    vec![
        web_search_def(),
        web_fetch_def(),
        read_file_def(),
        write_file_def(),
        shell_exec_def(),
        calculate_def(),
        patch_file_def(),
        find_files_def(),
        search_files_def(),
        load_skill_def(),
        list_skill_references_def(),
    ]
}

/// Python `get_tool_schemas(names)` — schemas for the given names.
///
/// Python parity: after built-in tools, MCP tool schemas are appended.
/// When `names` is provided, only MCP tools whose name is in `names`
/// are included (Python: `if schema["function"]["name"] in tool_names`).
pub fn get_tool_schemas(names: &[String]) -> Vec<Value> {
    let mut out = Vec::new();
    for name in names {
        if let Some(t) = get_tool(name) {
            out.push(tool_schema(&t));
        }
    }

    // Add MCP tools (Python `get_mcp_tool_schemas()` + name filter).
    let mcp_schemas = get_mcp_tool_schemas();
    for schema in mcp_schemas {
        let sname = schema["function"]["name"].as_str().unwrap_or("");
        if names.iter().any(|n| n == sname) {
            out.push(schema);
        }
    }
    out
}

/// Python `dispatch_tool(name, args)` — execute a tool by name.
///
/// Checks built-in tools first, then falls back to MCP tools
/// (Python: `dispatch_mcp_tool(name, args)`).
pub fn dispatch_tool(name: &str, args: &Value) -> String {
    match get_tool(name) {
        Some(tool) => {
            let handler = tool.handler;
            handler(args)
        }
        None => dispatch_mcp_tool(name, args),
    }
}

// ---------------------------------------------------------------------------
// MCP integration (Python tools.py `_mcp_registry` lifecycle)
// ---------------------------------------------------------------------------

/// Python `_ensure_mcp_initialized()` — lazily initialize MCP from
/// `Path.cwd() / ".mcp.json"` if it exists. Safe to call multiple times.
pub fn ensure_mcp_initialized() {
    let mut guard = mcp_registry().lock().expect("mutex poisoned");
    if guard.is_none() {
        let config_path = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".mcp.json");
        if config_path.exists() {
            let mut registry = MCPToolRegistry::new();
            registry.initialize(&config_path);
            *guard = Some(registry);
        }
    }
}

/// Python `initialize_mcp(config_path)` — initialize MCP servers.
/// Idempotent (no-op if already initialized).
pub fn initialize_mcp(config_path: &Path) {
    let mut guard = mcp_registry().lock().expect("mutex poisoned");
    if guard.is_some() {
        return;
    }
    let mut registry = MCPToolRegistry::new();
    registry.initialize(config_path);
    *guard = Some(registry);
}

/// Python `get_mcp_tool_schemas()` — MCP tool schemas (OpenAI format).
pub fn get_mcp_tool_schemas() -> Vec<Value> {
    ensure_mcp_initialized();
    let mut guard = mcp_registry().lock().expect("mutex poisoned");
    match guard.as_mut() {
        Some(registry) => registry.get_tool_schemas(),
        None => Vec::new(),
    }
}

/// Python `dispatch_mcp_tool(name, args)` — dispatch a tool call to MCP.
/// Returns an error JSON string if MCP is not available.
pub fn dispatch_mcp_tool(name: &str, args: &Value) -> String {
    ensure_mcp_initialized();
    let mut guard = mcp_registry().lock().expect("mutex poisoned");
    match guard.as_mut() {
        Some(registry) => registry.dispatch(name, args.clone()),
        None => json!({"error": format!("Unknown tool: {name}")}).to_string(),
    }
}

/// Python `shutdown_mcp()` — shut down all MCP servers and reset.
pub fn shutdown_mcp() {
    let mut guard = mcp_registry().lock().expect("mutex poisoned");
    if let Some(mut registry) = guard.take() {
        registry.shutdown();
    }
}

// ---------------------------------------------------------------------------
// Tool definitions
// ---------------------------------------------------------------------------

fn web_search_def() -> HelenTool {
    HelenTool {
    name: "web_search",
    description: "Search the web for information. Returns search results with titles, snippets, and links.",
    parameters: json!({
        "type": "object",
        "properties": {
            "query": {"type": "string", "description": "Search query"},
            "num_results": {"type": "integer", "description": "Number of results (default 3)", "default": 3},
        },
        "required": ["query"],
    }),
    handler: tool_web_search,
    }
}

fn web_fetch_def() -> HelenTool {
    HelenTool {
        name: "web_fetch",
        description: "Fetch the text content of a web page by URL.",
        parameters: json!({
            "type": "object",
            "properties": {
                "url": {"type": "string", "description": "URL to fetch"},
            },
            "required": ["url"],
        }),
        handler: tool_web_fetch,
    }
}

fn read_file_def() -> HelenTool {
    HelenTool {
        name: "read_file",
        description: "Read the content of a local file by path.",
        parameters: json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "File path to read"},
            },
            "required": ["path"],
        }),
        handler: tool_read_file,
    }
}

fn write_file_def() -> HelenTool {
    HelenTool {
        name: "write_file",
        description: "Write content to a local file. Creates parent directories if needed.",
        parameters: json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "File path to write"},
                "content": {"type": "string", "description": "Content to write"},
            },
            "required": ["path", "content"],
        }),
        handler: tool_write_file,
    }
}

fn shell_exec_def() -> HelenTool {
    HelenTool {
        name: "shell_exec",
        description: "Execute a shell command with full bash syntax support (&&, ||, |, {}, etc.).",
        parameters: json!({
            "type": "object",
            "properties": {
                "command": {"type": "string", "description": "Command to execute"},
                "timeout": {"type": "integer", "description": "Timeout in seconds (default 30)", "default": 30},
                "shell": {"type": "boolean", "description": "Use shell execution (default: true for full shell syntax)", "default": true},
            },
            "required": ["command"],
        }),
        handler: tool_shell_exec,
    }
}

fn calculate_def() -> HelenTool {
    HelenTool {
    name: "calculate",
    description: "Evaluate a mathematical expression. Supports basic arithmetic and math functions (sqrt, sin, cos, log, etc.).",
    parameters: json!({
        "type": "object",
        "properties": {
            "expression": {"type": "string", "description": "Math expression to evaluate, e.g. 'sqrt(16) + 2**3'"},
        },
        "required": ["expression"],
    }),
    handler: tool_calculate,
    }
}

fn patch_file_def() -> HelenTool {
    HelenTool {
    name: "patch_file",
    description: "Patch a file by replacing old_string with new_string. Uses fuzzy matching (9 strategies) to handle whitespace/indentation differences. More reliable than write_file for targeted edits.",
    parameters: json!({
        "type": "object",
        "properties": {
            "path": {"type": "string", "description": "File path to patch"},
            "old_string": {"type": "string", "description": "Text to find and replace (should be unique in file)"},
            "new_string": {"type": "string", "description": "Replacement text"},
            "replace_all": {"type": "boolean", "description": "Replace all occurrences (default: false, requires unique match)", "default": false},
        },
        "required": ["path", "old_string", "new_string"],
    }),
    handler: tool_patch_file,
    }
}

fn find_files_def() -> HelenTool {
    HelenTool {
    name: "find_files",
    description: "Find files matching a glob pattern. Use ** for recursive search. Returns structured list of matching file paths.",
    parameters: json!({
        "type": "object",
        "properties": {
            "path": {"type": "string", "description": "Root directory to search"},
            "pattern": {"type": "string", "description": "Glob pattern (e.g. '*.py', '**/*.txt', 'src/**/*.js'). Use ** for recursive. Default: '**/*'", "default": "**/*"},
            "max_results": {"type": "integer", "description": "Maximum results to return (default: 200)", "default": 200},
        },
        "required": ["path"],
    }),
    handler: tool_find_files,
    }
}

fn search_files_def() -> HelenTool {
    HelenTool {
    name: "search_files",
    description: "Search file contents for a text or regex pattern. Returns matches with file path, line number, and line content.",
    parameters: json!({
        "type": "object",
        "properties": {
            "path": {"type": "string", "description": "File or directory to search"},
            "pattern": {"type": "string", "description": "Text or regex pattern to search for"},
            "regex": {"type": "boolean", "description": "Treat pattern as regex (default: false, literal text)", "default": false},
            "case_sensitive": {"type": "boolean", "description": "Case-sensitive search (default: true)", "default": true},
            "max_results": {"type": "integer", "description": "Maximum matches to return (default: 100)", "default": 100},
        },
        "required": ["path", "pattern"],
    }),
    handler: tool_search_files,
    }
}

fn load_skill_def() -> HelenTool {
    HelenTool {
    name: "load_skill",
    description: "Load a skill's full SKILL.md content by name. Use this to get detailed instructions for a skill listed in <available_skills>. Set include_references=true to also see available reference documents.",
    parameters: json!({
        "type": "object",
        "properties": {
            "name": {"type": "string", "description": "Skill name to load (from <available_skills> list)"},
            "include_references": {"type": "boolean", "description": "If true, also list reference files in the skill's references/ directory", "default": false},
        },
        "required": ["name"],
    }),
    handler: tool_load_skill,
    }
}

fn list_skill_references_def() -> HelenTool {
    HelenTool {
    name: "list_skill_references",
    description: "List reference documents available for a skill. Returns file names, paths, sizes, and previews. Use read_file to load specific reference content.",
    parameters: json!({
        "type": "object",
        "properties": {
            "name": {"type": "string", "description": "Skill name (from <available_skills> list)"},
        },
        "required": ["name"],
    }),
    handler: tool_list_skill_references,
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

fn arg_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_str())
}

fn arg_str_or<'a>(args: &'a Value, key: &str, default: &'a str) -> &'a str {
    arg_str(args, key).unwrap_or(default)
}

fn arg_int(args: &Value, key: &str, default: i64) -> i64 {
    args.get(key).and_then(|v| v.as_i64()).unwrap_or(default)
}

fn arg_bool(args: &Value, key: &str, default: bool) -> bool {
    args.get(key).and_then(|v| v.as_bool()).unwrap_or(default)
}

fn tool_web_search(args: &Value) -> String {
    let query = arg_str(args, "query").unwrap_or_default();
    let num_results = arg_int(args, "num_results", 3) as usize;
    let encoded: String = query
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => b as char,
            b' ' => '+',
            _ => '%',
        })
        .collect();
    // URL-encode non-ASCII chars properly.
    let mut url_encoded = String::new();
    for byte in query.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                url_encoded.push(byte as char)
            }
            b' ' => url_encoded.push('+'),
            _ => url_encoded.push_str(&format!("%{:02X}", byte)),
        }
    }
    let _ = encoded;
    let search_url = format!(
        "https://www.bing.com/search?q={}&count={}",
        url_encoded, num_results
    );

    // Fetch the Bing results page.
    let html = match fetch_url(&search_url, 15) {
        Ok(h) => h,
        Err(e) => {
            return json!({"results": [], "message": format!("Search failed: {e}")}).to_string()
        }
    };

    // Extract <ol id="b_results">...</ol>
    let Some(results_match) = find_between(&html, r#"<ol id="b_results""#, "</ol>") else {
        return json!({"results": [], "message": format!("No results found for '{query}'.")})
            .to_string();
    };
    let results_html = results_match;

    // Find all <li class="b_algo"...>...</li>
    let mut results: Vec<String> = Vec::new();
    for item in find_all_between(results_html, r#"<li class="b_algo""#, "</li>") {
        let Some(title_match) = find_between(item, "<h2", "</a>") else {
            continue;
        };
        // Extract href="URL"
        let Some(href_start) = title_match.find("href=\"") else {
            continue;
        };
        let url_start = href_start + 6;
        let url_end = title_match[url_start..]
            .find('"')
            .map(|i| url_start + i)
            .unwrap_or(title_match.len());
        let url = &title_match[url_start..url_end];
        // Title text after the anchor tag.
        let title_html = &title_match[title_match.find('>').map(|i| i + 1).unwrap_or(0)..];
        let title = strip_html(title_html);

        // Description from <p class="b_lineclamp..."> or <div class="b_caption">
        let mut snippet = String::new();
        if let Some(dm) = find_between(item, r#"<p class="b_lineclamp"#, "</p>") {
            snippet = strip_html(dm);
        } else if let Some(dm) = find_between(item, r#"<div class="b_caption""#, "</div>") {
            snippet = strip_html(dm);
        }
        snippet = snippet.split_whitespace().collect::<Vec<_>>().join(" ");
        snippet = snippet.replace("&ensp;", " ").replace("&amp;", "&");

        results.push(format!("- {title}\n  {snippet}\n  {url}"));
        if results.len() >= num_results {
            break;
        }
    }

    if results.is_empty() {
        return json!({"results": [], "message": format!("No results found for '{query}'.")})
            .to_string();
    }
    json!({"results": results}).to_string()
}

/// Minimal HTML tag stripper (Python `re.sub(r'<[^>]+>', '', s)`).
fn strip_html(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut in_tag = false;
    for ch in html.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            out.push(ch);
        }
    }
    out
}

/// Find text between `open` marker and first `close` occurrence (from open end).
fn find_between<'a>(haystack: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let open_idx = haystack.find(open)?;
    let rest = &haystack[open_idx + open.len()..];
    let close_idx = rest.find(close)?;
    Some(&rest[..close_idx])
}

/// Find all text regions between `open` and `close`, starting after each open.
fn find_all_between<'a>(haystack: &'a str, open: &str, close: &str) -> Vec<&'a str> {
    let mut out = Vec::new();
    let mut rest = haystack;
    while let Some(open_idx) = rest.find(open) {
        let after_open = &rest[open_idx + open.len()..];
        match after_open.find(close) {
            Some(close_idx) => {
                out.push(&after_open[..close_idx]);
                rest = &after_open[close_idx + close.len()..];
            }
            None => break,
        }
    }
    out
}

/// Fetch a URL with a timeout, honoring gzip (via ureq's built-in support).
fn fetch_url(url: &str, timeout_secs: u64) -> Result<String, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build();
    let resp = agent
        .get(url)
        .set("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
        .set("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
        .call()
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    use std::io::Read;
    let mut reader = resp.into_reader();
    reader.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

fn tool_web_fetch(args: &Value) -> String {
    let url = arg_str(args, "url").unwrap_or_default();
    match fetch_url(url, 15) {
        Ok(content) => {
            // Strip script/style blocks then tags.
            let mut text = String::new();
            let mut rest = content.as_str();
            let in_script = false;
            while !rest.is_empty() {
                if let Some(idx) = rest.find("<script") {
                    text.push_str(&rest[..idx]);
                    rest = &rest[idx..];
                    if let Some(end) = rest.find("</script>") {
                        rest = &rest[end + 9..];
                    } else {
                        rest = "";
                    }
                } else if let Some(idx) = rest.find("<style") {
                    text.push_str(&rest[..idx]);
                    rest = &rest[idx..];
                    if let Some(end) = rest.find("</style>") {
                        rest = &rest[end + 8..];
                    } else {
                        rest = "";
                    }
                } else {
                    text.push_str(rest);
                    rest = "";
                }
            }
            let _ = in_script;
            text = strip_html(&text);
            text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if text.chars().count() > 8000 {
                let truncated: String = text.chars().take(8000).collect();
                text = format!("{truncated}... [truncated]");
            }
            json!({"url": url, "content": text}).to_string()
        }
        Err(e) => json!({"error": format!("Fetch failed: {e}")}).to_string(),
    }
}

fn tool_read_file(args: &Value) -> String {
    let path = arg_str(args, "path").unwrap_or_default();
    match std::fs::read_to_string(path) {
        Ok(mut content) => {
            if content.chars().count() > 16000 {
                let truncated: String = content.chars().take(16000).collect();
                content = format!("{truncated}\n... [truncated]");
            }
            json!({"path": path, "content": content}).to_string()
        }
        Err(e) => json!({"error": format!("Read failed: {e}")}).to_string(),
    }
}

fn tool_write_file(args: &Value) -> String {
    let path = arg_str(args, "path").unwrap_or_default();
    let content = arg_str(args, "content").unwrap_or_default();
    match (|| -> std::io::Result<()> {
        if let Some(parent) = Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, content)
    })() {
        Ok(()) => {
            let bytes = content.len();
            json!({"path": path, "bytes_written": bytes, "status": "ok"}).to_string()
        }
        Err(e) => json!({"error": format!("Write failed: {e}")}).to_string(),
    }
}

fn tool_shell_exec(args: &Value) -> String {
    let command = arg_str(args, "command").unwrap_or_default();
    let timeout = arg_int(args, "timeout", 120) as u64;
    let shell = arg_bool(args, "shell", true);

    let result = run_command(command, timeout, shell);
    match result {
        Ok(output) => {
            let mut output = output;
            if output.chars().count() > 8000 {
                let truncated: String = output.chars().take(8000).collect();
                output = format!("{truncated}\n... [truncated]");
            }
            output
        }
        Err(e) => e,
    }
}

/// Run a shell command capturing stdout. Returns stdout or an error string
/// with the same format as Python (raw stdout on success).
fn run_command(command: &str, timeout_secs: u64, shell: bool) -> Result<String, String> {
    let (program, args): (&str, Vec<String>) = if shell {
        ("/bin/bash", vec!["-c".to_string(), command.to_string()])
    } else {
        let parts: Vec<String> = command.split_whitespace().map(|s| s.to_string()).collect();
        if parts.is_empty() {
            return Ok(String::new());
        }
        let prog = parts[0].clone();
        (Box::leak(prog.into_boxed_str()), parts[1..].to_vec())
    };

    let child = std::process::Command::new(program)
        .args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("[error] Exec failed: {e}"))?;

    // Wait with timeout.
    let start = std::time::Instant::now();
    let mut child = child;
    let output = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|e| format!("[error] Exec failed: {e}"))?
        {
            // Collect output.
            let out = child
                .wait_with_output()
                .map_err(|e| format!("[error] Exec failed: {e}"))?;
            let _ = status;
            break String::from_utf8_lossy(&out.stdout).to_string();
        }
        if start.elapsed().as_secs() > timeout_secs {
            let _ = child.kill();
            return Err(format!("[error] Command timed out after {timeout_secs}s"));
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    Ok(output)
}

fn tool_calculate(args: &Value) -> String {
    let expression = arg_str(args, "expression").unwrap_or_default();
    match crate::calc::eval_simple(expression) {
        Ok(result) => {
            // Python returns the numeric result; format as number when int.
            let parsed: Result<i64, _> = result.parse();
            if let Ok(i) = parsed {
                json!({"expression": expression, "result": i}).to_string()
            } else {
                let parsed_f: Result<f64, _> = result.parse();
                match parsed_f {
                    Ok(f) => json!({"expression": expression, "result": f}).to_string(),
                    Err(_) => json!({"expression": expression, "result": result}).to_string(),
                }
            }
        }
        Err(e) => json!({"error": format!("Calculation failed: {e}")}).to_string(),
    }
}

fn tool_patch_file(args: &Value) -> String {
    let path = arg_str(args, "path").unwrap_or_default();
    let old_string = arg_str(args, "old_string").unwrap_or_default();
    let new_string = arg_str(args, "new_string").unwrap_or_default();
    let replace_all = arg_bool(args, "replace_all", false);

    let file_path = PathBuf::from(path);
    if !file_path.exists() {
        return json!({"error": format!("File not found: {path}")}).to_string();
    }
    let mut content = match std::fs::read_to_string(&file_path) {
        Ok(c) => c,
        Err(e) => return json!({"error": format!("Patch failed: {e}")}).to_string(),
    };
    // Strip UTF-8 BOM.
    if let Some(stripped) = content.strip_prefix('\u{feff}') {
        content = stripped.to_string();
    }

    let result =
        crate::fuzzy_match::fuzzy_find_and_replace(&content, old_string, new_string, replace_all);

    if let Some(error) = &result.error {
        let hint = crate::fuzzy_match::format_no_match_hint(
            error,
            result.match_count,
            old_string,
            &content,
        );
        return json!({"error": error, "hint": hint}).to_string();
    }
    if result.match_count == 0 {
        return json!({"error": "No matches found"}).to_string();
    }

    if let Err(e) = std::fs::write(&file_path, &result.new_content) {
        return json!({"error": format!("Patch failed: {e}")}).to_string();
    }

    // Unified diff for feedback.
    let diff = unified_diff(&content, &result.new_content, path, path);
    let mut diff = diff;
    if diff.chars().count() > 4000 {
        let truncated: String = diff.chars().take(4000).collect();
        diff = format!("{truncated}\n... [diff truncated]");
    }

    json!({
        "path": path,
        "status": "patched",
        "matches": result.match_count,
        "strategy": result.strategy,
        "diff": diff,
    })
    .to_string()
}

/// Minimal unified diff (Python `difflib.unified_diff` parity for the
/// common single-hunk case).
fn unified_diff(old: &str, new: &str, fromfile: &str, tofile: &str) -> String {
    let old_lines: Vec<&str> = old.split('\n').collect();
    let new_lines: Vec<&str> = new.split('\n').collect();
    let mut out = String::new();
    out.push_str(&format!("--- a/{fromfile}\n"));
    out.push_str(&format!("+++ b/{tofile}\n"));
    // Compute a simple LCS-based diff.
    let ops = diff_ops(&old_lines, &new_lines);
    let old_count = old_lines.len();
    let new_count = new_lines.len();
    out.push_str(&format!("@@ -1,{old_count} +1,{new_count} @@\n"));
    for op in ops {
        match op {
            Op::Equal(line) => out.push_str(&format!(" {line}\n")),
            Op::Delete(line) => out.push_str(&format!("-{line}\n")),
            Op::Insert(line) => out.push_str(&format!("+{line}\n")),
        }
    }
    out
}

enum Op<'a> {
    Equal(&'a str),
    Delete(&'a str),
    Insert(&'a str),
}

fn diff_ops<'a>(old: &[&'a str], new: &[&'a str]) -> Vec<Op<'a>> {
    // Simple LCS-based op list.
    let n = old.len();
    let m = new.len();
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if old[i] == new[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }
    let mut ops = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if old[i] == new[j] {
            ops.push(Op::Equal(old[i]));
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            ops.push(Op::Delete(old[i]));
            i += 1;
        } else {
            ops.push(Op::Insert(new[j]));
            j += 1;
        }
    }
    while i < n {
        ops.push(Op::Delete(old[i]));
        i += 1;
    }
    while j < m {
        ops.push(Op::Insert(new[j]));
        j += 1;
    }
    ops
}

fn tool_find_files(args: &Value) -> String {
    let path = arg_str(args, "path").unwrap_or_default();
    let pattern = arg_str_or(args, "pattern", "**/*").to_string();
    let max_results = arg_int(args, "max_results", 200) as usize;

    let root = PathBuf::from(path);
    if !root.exists() {
        return json!({"error": format!("Directory not found: {path}")}).to_string();
    }
    if !root.is_dir() {
        return json!({"error": format!("Not a directory: {path}")}).to_string();
    }

    let matches = glob_files(&root, &pattern);
    let truncated = matches.len() > max_results;
    let mut result = json!({
        "path": path,
        "pattern": pattern,
        "matches": matches.iter().take(max_results).collect::<Vec<_>>(),
        "count": matches.len(),
    });
    if truncated {
        result["truncated"] = json!(true);
        result["message"] = json!(format!(
            "Results truncated to {max_results} matches. Use a more specific pattern."
        ));
    }
    result.to_string()
}

/// Port of `_glob_files` — recursive glob with `**` support.
fn glob_files(root: &Path, pattern: &str) -> Vec<String> {
    // Python pathlib: `**` in pattern or no `/` → rglob (recursive);
    // otherwise glob (respect directory structure).
    let recursive = pattern.contains("**") || !pattern.contains('/');
    let search_pattern = pattern.replace("**/", "");

    let mut matches = Vec::new();
    collect_recursive(root, &mut matches);

    let mut out = Vec::new();
    for rel in &matches {
        if glob_match(&search_pattern, rel) {
            out.push(rel.clone());
        }
    }
    if !recursive {
        // For non-recursive glob patterns with '/', Python glob() matches at
        // any depth but relative path structure; our collect is recursive, so
        // filter to files matching the pattern at the right depth.
        out.retain(|rel| glob_match(&search_pattern, rel));
    }
    out
}

fn collect_recursive(dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let rel = path
            .strip_prefix(dir)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        if path.is_dir() {
            collect_recursive(&path, out);
        } else {
            out.push(rel);
        }
    }
}

/// Convert a glob pattern to a regex (supports `*`, `?`, `**`, char classes).
fn glob_to_regex(pattern: &str) -> regex::Regex {
    let mut re = String::from("^");
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '*' => {
                if i + 1 < chars.len() && chars[i + 1] == '*' {
                    re.push_str(".*");
                    i += 2;
                    continue;
                }
                re.push_str("[^/]*");
            }
            '?' => re.push_str("[^/]"),
            '[' => {
                // char class
                let mut j = i + 1;
                let mut cls = String::from("[");
                let mut closed = false;
                while j < chars.len() {
                    if chars[j] == ']' {
                        closed = true;
                        break;
                    }
                    cls.push(chars[j]);
                    j += 1;
                }
                if closed {
                    re.push_str(&cls);
                    re.push(']');
                    i = j;
                } else {
                    re.push_str("\\[");
                }
            }
            c if "\\^$.|+(){}".contains(c) => {
                re.push('\\');
                re.push(c);
            }
            c => re.push(c),
        }
        i += 1;
    }
    re.push('$');
    regex::Regex::new(&re).unwrap_or_else(|_| regex::Regex::new("$^").expect("fallback regex"))
}

fn glob_match(pattern: &str, rel_path: &str) -> bool {
    let re = glob_to_regex(pattern);
    re.is_match(rel_path)
}

fn tool_search_files(args: &Value) -> String {
    let path = arg_str(args, "path").unwrap_or_default();
    let pattern = arg_str(args, "pattern").unwrap_or_default();
    let regex_mode = arg_bool(args, "regex", false);
    let case_sensitive = arg_bool(args, "case_sensitive", true);
    let max_results = arg_int(args, "max_results", 100) as usize;

    let search_path = PathBuf::from(path);
    if !search_path.exists() {
        return json!({"error": format!("Path not found: {path}")}).to_string();
    }

    // Build the regex.
    let flags = if case_sensitive { "" } else { "(?i)" };
    let pattern_src = if regex_mode {
        pattern.to_string()
    } else {
        regex::escape(pattern)
    };
    let compiled = match regex::Regex::new(&format!("{flags}{pattern_src}")) {
        Ok(r) => r,
        Err(e) => {
            return json!({"error": format!("Invalid regex pattern: {e}")}).to_string();
        }
    };

    // Collect files to search.
    let mut files: Vec<PathBuf> = Vec::new();
    if search_path.is_file() {
        files.push(search_path.clone());
    } else {
        collect_file_paths(&search_path, &mut files);
    }

    let mut results: Vec<Value> = Vec::new();
    'outer: for file_path in &files {
        if results.len() >= max_results {
            break;
        }
        // Skip files > 1MB.
        if let Ok(meta) = file_path.metadata() {
            if meta.len() > 1_000_000 {
                continue;
            }
        }
        let Ok(content) = std::fs::read_to_string(file_path) else {
            continue;
        };
        for (line_num, line) in content.lines().enumerate() {
            if compiled.is_match(line) {
                results.push(json!({
                    "file": file_path.to_string_lossy().to_string(),
                    "line": line_num + 1,
                    "text": line,
                }));
                if results.len() >= max_results {
                    break 'outer;
                }
            }
        }
    }

    let mut result = json!({
        "path": path,
        "pattern": pattern,
        "regex": regex_mode,
        "matches": results,
        "count": results.len(),
    });
    if results.len() >= max_results {
        result["truncated"] = json!(true);
        result["message"] = json!(format!("Results truncated to {max_results} matches."));
    }
    result.to_string()
}

fn collect_file_paths(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_file_paths(&path, out);
        } else if path.is_file() {
            out.push(path);
        }
    }
}

fn tool_load_skill(args: &Value) -> String {
    let name = arg_str(args, "name").unwrap_or_default();
    let include_references = arg_bool(args, "include_references", false);
    crate::skills::load_skill(name, include_references).to_string()
}

fn tool_list_skill_references(args: &Value) -> String {
    let name = arg_str(args, "name").unwrap_or_default();
    crate::skills::list_skill_references(name).to_string()
}

/// Backward-compatible dispatch used by http_llm (M5 wiring).
pub fn tools_dispatch(name: &str, args: &Value) -> Result<String, String> {
    Ok(dispatch_tool(name, args))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schemas_match_python_byte_identical() {
        // Spot-check the schemas against the Python source (tools.py).
        let ws = get_tool("web_search").unwrap();
        assert_eq!(ws.description, "Search the web for information. Returns search results with titles, snippets, and links.");
        assert_eq!(
            ws.parameters,
            json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Search query"},
                    "num_results": {"type": "integer", "description": "Number of results (default 3)", "default": 3},
                },
                "required": ["query"],
            })
        );
        let rf = get_tool("read_file").unwrap();
        assert_eq!(
            rf.parameters,
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "File path to read"},
                },
                "required": ["path"],
            })
        );
        let pf = get_tool("patch_file").unwrap();
        assert!(pf.parameters["required"] == json!(["path", "old_string", "new_string"]));
    }

    #[test]
    fn all_11_tools_registered() {
        let names: Vec<&str> = all_tools().iter().map(|t| t.name).collect();
        assert_eq!(
            names,
            vec![
                "web_search",
                "web_fetch",
                "read_file",
                "write_file",
                "shell_exec",
                "calculate",
                "patch_file",
                "find_files",
                "search_files",
                "load_skill",
                "list_skill_references",
            ]
        );
    }

    #[test]
    fn unknown_tool_error() {
        let v: Value = serde_json::from_str(&dispatch_tool("nope", &json!({}))).unwrap();
        assert!(v["error"]
            .as_str()
            .expect("string value")
            .contains("Unknown tool"));
    }

    #[test]
    fn calculate_tool_returns_json() {
        let r = dispatch_tool("calculate", &json!({"expression": "2 + 3 * 4"}));
        let v: Value = serde_json::from_str(&r).expect("from_str");
        assert_eq!(v["result"], json!(14));
    }

    #[test]
    fn read_file_missing_error() {
        let r = dispatch_tool("read_file", &json!({"path": "/no/such/file/xyz"}));
        let v: Value = serde_json::from_str(&r).expect("from_str");
        assert!(v["error"]
            .as_str()
            .expect("string value")
            .starts_with("Read failed"));
    }

    #[test]
    fn write_then_read_roundtrip() {
        let dir = std::env::temp_dir().join(format!("helen_tool_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create dir");
        let p = dir.join("sub").join("test.txt");
        let ps = p.to_string_lossy().to_string();
        let r = dispatch_tool("write_file", &json!({"path": ps, "content": "hello"}));
        let v: Value = serde_json::from_str(&r).expect("from_str");
        assert_eq!(v["status"], json!("ok"));
        assert_eq!(v["bytes_written"], json!(5));
        let r2 = dispatch_tool("read_file", &json!({"path": ps}));
        let v2: Value = serde_json::from_str(&r2).expect("from_str");
        assert_eq!(v2["content"], json!("hello"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn patch_file_exact_roundtrip() {
        let dir = std::env::temp_dir().join(format!("helen_patch_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create dir");
        let p = dir.join("a.txt");
        std::fs::write(&p, "def foo():\n    pass\n").unwrap();
        let ps = p.to_string_lossy().to_string();
        let r = dispatch_tool(
            "patch_file",
            &json!({"path": ps, "old_string": "def foo():", "new_string": "def bar():"}),
        );
        let v: Value = serde_json::from_str(&r).expect("from_str");
        assert_eq!(v["status"], json!("patched"));
        assert_eq!(v["strategy"], json!("exact"));
        let content = std::fs::read_to_string(&p).unwrap();
        assert_eq!(content, "def bar():\n    pass\n");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn patch_file_fuzzy_whitespace() {
        let dir = std::env::temp_dir().join(format!("helen_patch_fz_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create dir");
        let p = dir.join("b.txt");
        std::fs::write(&p, "    if x:\n        y()\n").unwrap();
        let ps = p.to_string_lossy().to_string();
        // old_string has different indentation (no leading spaces).
        let r = dispatch_tool(
            "patch_file",
            &json!({"path": ps, "old_string": "if x:\n    y()", "new_string": "if z:\n    y()"}),
        );
        let v: Value = serde_json::from_str(&r).expect("from_str");
        assert_eq!(v["status"], json!("patched"));
        assert!(v["strategy"].as_str().is_some());
        let content = std::fs::read_to_string(&p).unwrap();
        assert!(content.contains("if z:"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn patch_file_no_match_hint() {
        let dir = std::env::temp_dir().join(format!("helen_patch_nm_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create dir");
        let p = dir.join("c.txt");
        std::fs::write(&p, "alpha\nbeta\ngamma\n").expect("write file");
        let ps = p.to_string_lossy().to_string();
        let r = dispatch_tool(
            "patch_file",
            &json!({"path": ps, "old_string": "zzz not here", "new_string": "x"}),
        );
        let v: Value = serde_json::from_str(&r).expect("from_str");
        assert!(v["error"]
            .as_str()
            .expect("string value")
            .contains("Could not find"));
        assert!(v["hint"].as_str().is_some());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn find_files_glob() {
        let dir = std::env::temp_dir().join(format!("helen_find_{}", std::process::id()));
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("src").join("a.rs"), "").unwrap();
        std::fs::write(dir.join("src").join("b.txt"), "").unwrap();
        std::fs::write(dir.join("top.txt"), "").unwrap();
        let ds = dir.to_string_lossy().to_string();
        let r = dispatch_tool("find_files", &json!({"path": ds, "pattern": "*.txt"}));
        let v: Value = serde_json::from_str(&r).expect("from_str");
        assert_eq!(v["count"], json!(2));
        let r2 = dispatch_tool("find_files", &json!({"path": ds, "pattern": "**/*.rs"}));
        let v2: Value = serde_json::from_str(&r2).expect("from_str");
        assert_eq!(v2["count"], json!(1));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn search_files_literal_and_regex() {
        let dir = std::env::temp_dir().join(format!("helen_grep_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create dir");
        std::fs::write(dir.join("f.txt"), "hello world\nTODO fix\n").unwrap();
        let ds = dir.to_string_lossy().to_string();
        let r = dispatch_tool("search_files", &json!({"path": ds, "pattern": "TODO"}));
        let v: Value = serde_json::from_str(&r).expect("from_str");
        assert_eq!(v["count"], json!(1));
        assert_eq!(v["matches"][0]["line"], json!(2));
        let r2 = dispatch_tool(
            "search_files",
            &json!({"path": ds, "pattern": "h.llo", "regex": true}),
        );
        let v2: Value = serde_json::from_str(&r2).expect("from_str");
        assert_eq!(v2["count"], json!(1));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn shell_exec_basic() {
        let r = dispatch_tool("shell_exec", &json!({"command": "echo hello"}));
        assert_eq!(r.trim(), "hello");
    }

    #[test]
    fn shell_exec_timeout() {
        let r = dispatch_tool("shell_exec", &json!({"command": "sleep 5", "timeout": 1}));
        assert!(r.contains("[error] Command timed out"));
    }

    #[test]
    fn glob_regex_basics() {
        assert!(glob_match("*.txt", "a.txt"));
        assert!(!glob_match("*.txt", "a.rs"));
        assert!(glob_match("**/*.rs", "src/a.rs"));
        assert!(glob_match("src/**/*.js", "src/deep/x.js"));
        assert!(!glob_match("src/**/*.js", "other/x.js"));
    }
}
