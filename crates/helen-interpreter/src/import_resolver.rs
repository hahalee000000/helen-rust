//! import_resolver.rs — `.helen` / data-file import resolution (Task 3.7b).
//!
//! Port of `helen/runtime/import_resolver.py`: path resolution with
//! traversal safety (HLD 3.9.2), circular-import detection, and per-file
//! symbol registration (v1.39.2). The interpreter consumes the per-file
//! registries to bind functions/agents/consts into its namespaces.
//!
//! Scope notes (vs Python):
//! - Python-module imports (`import "math"`, `import "x.py"`) go through the
//!   Python FFI and are **not supported** by the Rust runtime — `resolve`
//!   reports them via [`ResolvedImport::Python`] and the interpreter raises.
//! - YAML data files fall back to raw text (mirrors Python's no-pyyaml path).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use helen_core::ast::{AgentDecl, FunctionDecl, ImportStmt, SharedStoreDecl, Stmt, VarDecl};
use helen_core::lexer::Scanner;
use helen_parser::Parser;

use crate::value::Value;

/// A single loaded `.helen` file's registrable symbols (v1.39.2 per-file
/// registries). Functions/agents/consts are cloned into the interpreter on
/// import; `imports` are resolved recursively against this file's directory.
#[derive(Default, Clone)]
pub struct FileRegistry {
    pub functions: Vec<FunctionDecl>,
    pub agents: Vec<AgentDecl>,
    /// const / shared let declarations (`!mutable || shared`).
    pub data: Vec<VarDecl>,
    /// shared store declarations (v1.17).
    pub shared_stores: Vec<SharedStoreDecl>,
    /// nested imports declared in the file.
    pub imports: Vec<ImportStmt>,
}

/// Outcome of a successful `resolve` for the interpreter to act on.
pub enum ResolvedImport {
    /// A `.helen` file — symbols are in the per-file registries.
    Helen { path: PathBuf },
    /// A data file (text/json/yaml-as-text) — define `alias` in the env.
    Data { alias: String, value: Value },
    /// A Python module import — unsupported by the Rust runtime.
    Python,
}

/// `.helen`-file import resolver (fresh per `Interpreter` instance).
pub struct ImportResolver {
    base_dir: PathBuf,
    /// Absolute paths currently/ever loaded — circular import detection.
    loaded: HashSet<PathBuf>,
    /// Per-file registries keyed by absolute path.
    files: HashMap<PathBuf, FileRegistry>,
    /// Absolute paths of loaded `.helen` files, in load order.
    load_order: Vec<PathBuf>,
}

/// Detect the import format from a path's extension (HLD 3.9.1).
fn detect_format(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("helen") => "helen",
        Some("json") => "json",
        Some("yaml") | Some("yml") => "yaml",
        Some("py") => "python",
        _ => "text",
    }
}

/// Helen data files: .helen/.json/.md/.txt/.yaml/.yml (others -> Python).
fn is_helen_data_file(path: &str) -> bool {
    ["helen", "json", "md", "txt", "yaml", "yml"]
        .iter()
        .any(|ext| path.to_ascii_lowercase().ends_with(&format!(".{ext}")))
}

fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in p.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

impl ImportResolver {
    pub fn new(base_dir: PathBuf) -> Self {
        ImportResolver {
            base_dir,
            loaded: HashSet::new(),
            files: HashMap::new(),
            load_order: Vec::new(),
        }
    }

    /// Absolute paths of loaded `.helen` files, in load order (direct +
    /// transitively loaded).
    pub fn load_order(&self) -> &[PathBuf] {
        &self.load_order
    }

    /// Per-file registry for an absolute path.
    pub fn file(&self, abs: &Path) -> Option<&FileRegistry> {
        self.files.get(abs)
    }

    /// Resolve and load `import_path` (HLD 3.9.1/3.9.2).
    ///
    /// `from_file` is the absolute path of the importing file (relative
    /// imports resolve against its directory, falling back to `base_dir`).
    pub fn resolve(
        &mut self,
        import_path: &str,
        from_file: Option<&Path>,
    ) -> Result<ResolvedImport, String> {
        // Python-module imports never touch the filesystem resolver.
        if !is_helen_data_file(import_path) {
            return Ok(ResolvedImport::Python);
        }

        let resolved = self.resolve_path(import_path, from_file)?;
        if !self.is_safe_path(&resolved) {
            return Err(format!("Import path escapes base directory: {import_path}"));
        }
        let abs = normalize(&resolved);
        let abs = if abs.is_absolute() {
            abs
        } else {
            std::env::current_dir().map(|c| c.join(&abs)).unwrap_or(abs)
        };

        // Circular / duplicate import: already registered (or in progress).
        if self.loaded.contains(&abs) {
            return Ok(ResolvedImport::Helen { path: abs });
        }

        self.loaded.insert(abs.clone());
        let outcome = match detect_format(&abs) {
            "helen" => {
                self.load_helen(&abs)?;
                Ok(ResolvedImport::Helen { path: abs.clone() })
            }
            "json" => {
                let raw = std::fs::read_to_string(&abs)
                    .map_err(|e| format!("Failed to import '{import_path}': {e}"))?;
                let value = parse_json(&raw)
                    .map_err(|e| format!("Failed to import '{import_path}': {e}"))?;
                Ok(ResolvedImport::Data {
                    alias: stem(&abs),
                    value,
                })
            }
            _ => {
                // text / yaml-as-text
                let raw = std::fs::read_to_string(&abs)
                    .map_err(|e| format!("Failed to import '{import_path}': {e}"))?;
                Ok(ResolvedImport::Data {
                    alias: stem(&abs),
                    value: Value::Str(std::rc::Rc::from(raw.as_str())),
                })
            }
        };
        if outcome.is_err() {
            self.loaded.remove(&abs);
        }
        outcome
    }

    /// Resolve `import_path` to an absolute path (relative to `from_file`'s
    /// directory, then `base_dir`; absolute paths pass through).
    fn resolve_path(&self, import_path: &str, from_file: Option<&Path>) -> Result<PathBuf, String> {
        let p = Path::new(import_path);
        if p.is_absolute() {
            return Ok(p.to_path_buf());
        }
        if let Some(from) = from_file {
            let from_dir = from.parent().unwrap_or(Path::new("."));
            let cand = from_dir.join(import_path);
            if cand.exists() {
                return Ok(normalize(&cand));
            }
        }
        let cand = self.base_dir.join(import_path);
        if cand.exists() {
            return Ok(normalize(&cand));
        }
        Err(format!("Import file not found: {import_path}"))
    }

    /// HLD 3.9.2 path safety: absolute paths allowed (REPL/explicit);
    /// relative paths must stay within `base_dir`.
    fn is_safe_path(&self, resolved: &Path) -> bool {
        if resolved.is_absolute() {
            return true;
        }
        let base = normalize(&self.base_dir);
        let base = if base.is_absolute() {
            base
        } else {
            let joined = std::env::current_dir().map(|c| c.join(base.clone()));
            joined.unwrap_or(base)
        };
        let cand = normalize(&base.join(resolved));
        cand.starts_with(&base)
    }

    /// Parse a `.helen` file and register its symbols + nested imports.
    fn load_helen(&mut self, abs: &Path) -> Result<(), String> {
        let source = std::fs::read_to_string(abs)
            .map_err(|e| format!("Failed to import '{}': {e}", abs.display()))?;
        let name = abs.to_str().unwrap_or("import.helen");
        let mut scanner = Scanner::new(&source, name);
        let tokens = scanner.scan_all();
        let mut parser = Parser::new(tokens);
        let program = parser.parse();
        if !parser.errors().is_empty() {
            return Err(format!(
                "Failed to import '{}': parse errors",
                abs.display()
            ));
        }

        let mut reg = FileRegistry::default();
        for stmt in &program.statements {
            match stmt {
                Stmt::FunctionDecl(f) => reg.functions.push(f.clone()),
                Stmt::AgentDecl(a) => reg.agents.push(a.clone()),
                Stmt::VarDecl(v) if !v.mutable || v.shared => reg.data.push(v.clone()),
                Stmt::SharedStoreDecl(ss) => reg.shared_stores.push(ss.clone()),
                Stmt::Import(i) => reg.imports.push(i.clone()),
                _ => {}
            }
        }
        self.files.insert(abs.to_path_buf(), reg.clone());
        self.load_order.push(abs.to_path_buf());

        // Recursively resolve nested imports against this file's directory.
        for imp in &reg.imports {
            if imp.is_stdlib_module {
                continue; // stdlib modules bind at the interpreter level
            }
            if !is_helen_data_file(&imp.module_path) {
                continue; // Python FFI imports are unsupported
            }
            self.resolve(&imp.module_path, Some(abs))?;
        }
        Ok(())
    }
}

fn stem(p: &Path) -> String {
    p.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("data")
        .to_string()
}

/// Parse a JSON data file into a Helen value (objects -> maps, arrays ->
/// lists, primitives mapped directly).
fn parse_json(raw: &str) -> Result<Value, String> {
    let v: serde_json::Value =
        serde_json::from_str(raw).map_err(|e| format!("JSON parse error: {e}"))?;
    Ok(json_to_value(&v))
}

fn json_to_value(v: &serde_json::Value) -> Value {
    use indexmap::IndexMap;
    use std::rc::Rc;
    match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(num_bigint::BigInt::from(i))
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Null
            }
        }
        serde_json::Value::String(s) => Value::Str(Rc::from(s.as_str())),
        serde_json::Value::Array(items) => {
            let list: Vec<Value> = items.iter().map(json_to_value).collect();
            Value::List(Rc::new(std::cell::RefCell::new(list)))
        }
        serde_json::Value::Object(map) => {
            let mut out: IndexMap<Value, Value> = IndexMap::new();
            for (k, val) in map {
                out.insert(Value::Str(Rc::from(k.as_str())), json_to_value(val));
            }
            Value::Map(Rc::new(std::cell::RefCell::new(out)))
        }
    }
}
