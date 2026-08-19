//! Test coverage measurement (Task 8.6) — port of `helen/runtime/coverage.py`.
//!
//! Function/line/branch coverage by instrumenting the interpreter at key
//! execution points. Default off (zero overhead); resource-bounded.

use serde_json::{json, Map, Value};
use std::collections::HashMap;

/// `CoverageCount` — coverage counters for a single source file.
#[derive(Debug, Clone, Default)]
pub struct CoverageCount {
    /// line -> execution count.
    pub lines: HashMap<u32, u64>,
    /// (func_name, line) -> call count.
    pub functions: HashMap<(String, u32), u64>,
    /// (line, branch_id) -> execution count. branch_id=0 false, 1 true.
    pub branches: HashMap<(u32, i64), u64>,
}

/// `CoverageTracker` — measures test coverage of Helen programs.
#[derive(Debug)]
pub struct CoverageTracker {
    files: HashMap<String, CoverageCount>,
    enabled: bool,
    max_counters: usize,
    total_counters: usize,
    source_files: HashMap<String, Vec<String>>,
}

impl Default for CoverageTracker {
    fn default() -> Self {
        Self::new(1_000_000)
    }
}

impl CoverageTracker {
    pub fn new(max_counters: usize) -> Self {
        Self {
            files: HashMap::new(),
            enabled: false,
            max_counters,
            total_counters: 0,
            source_files: HashMap::new(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, value: bool) {
        self.enabled = value;
    }

    fn get_file(&mut self, file_path: &str) -> &mut CoverageCount {
        self.files.entry(file_path.to_string()).or_default()
    }

    fn check_limit(&self) -> bool {
        self.total_counters < self.max_counters
    }

    /// `_abs_path` — normalize a span's file path to absolute.
    fn abs_path(file: Option<&str>) -> Option<String> {
        let f = file?;
        if f.is_empty() {
            return None;
        }
        let p = std::path::Path::new(f);
        if p.is_absolute() {
            Some(f.to_string())
        } else {
            Some(
                p.canonicalize()
                    .unwrap_or_else(|_| p.to_path_buf())
                    .to_string_lossy()
                    .to_string(),
            )
        }
    }

    pub fn record_line(&mut self, file: Option<&str>, start_line: u32) {
        if !self.enabled {
            return;
        }
        let Some(file_path) = Self::abs_path(file) else {
            return;
        };
        if !self.check_limit() {
            return;
        }
        let is_new = !self
            .files
            .get(&file_path)
            .map(|fc| fc.lines.contains_key(&start_line))
            .unwrap_or(false);
        if is_new {
            self.total_counters += 1;
        }
        let fc = self.get_file(&file_path);
        *fc.lines.entry(start_line).or_insert(0) += 1;
    }

    pub fn record_function(&mut self, file: Option<&str>, start_line: u32, func_name: &str) {
        if !self.enabled {
            return;
        }
        let Some(file_path) = Self::abs_path(file) else {
            return;
        };
        if !self.check_limit() {
            return;
        }
        let key = (func_name.to_string(), start_line);
        let is_new = !self
            .files
            .get(&file_path)
            .map(|fc| fc.functions.contains_key(&key))
            .unwrap_or(false);
        if is_new {
            self.total_counters += 1;
        }
        let fc = self.get_file(&file_path);
        *fc.functions.entry(key).or_insert(0) += 1;
    }

    pub fn record_branch(&mut self, file: Option<&str>, start_line: u32, branch_id: i64) {
        if !self.enabled {
            return;
        }
        let Some(file_path) = Self::abs_path(file) else {
            return;
        };
        if !self.check_limit() {
            return;
        }
        let key = (start_line, branch_id);
        let is_new = !self
            .files
            .get(&file_path)
            .map(|fc| fc.branches.contains_key(&key))
            .unwrap_or(false);
        if is_new {
            self.total_counters += 1;
        }
        let fc = self.get_file(&file_path);
        *fc.branches.entry(key).or_insert(0) += 1;
    }

    pub fn register_source(&mut self, file_path: &str, source_lines: Vec<String>) {
        let p = std::path::Path::new(file_path);
        let abs_path = if p.is_absolute() {
            file_path.to_string()
        } else {
            p.canonicalize()
                .unwrap_or_else(|_| p.to_path_buf())
                .to_string_lossy()
                .to_string()
        };
        self.source_files.insert(abs_path, source_lines);
    }

    pub fn register_function(&mut self, file: Option<&str>, start_line: u32, func_name: &str) {
        let Some(file_path) = Self::abs_path(file) else {
            return;
        };
        let fc = self.get_file(&file_path);
        fc.functions
            .entry((func_name.to_string(), start_line))
            .or_insert(0);
    }

    pub fn register_branch(&mut self, file: Option<&str>, start_line: u32, branch_ids: &[i64]) {
        let Some(file_path) = Self::abs_path(file) else {
            return;
        };
        let fc = self.get_file(&file_path);
        for bid in branch_ids {
            fc.branches.entry((start_line, *bid)).or_insert(0);
        }
    }

    pub fn reset(&mut self) {
        self.files.clear();
        self.total_counters = 0;
        self.source_files.clear();
    }

    pub fn clear(&mut self) {
        self.reset();
    }

    /// `get_summary` — line/function/branch coverage percentages.
    pub fn get_summary(&self) -> Value {
        let mut total_lines = 0usize;
        let mut covered_lines = 0usize;
        let mut total_functions = 0usize;
        let mut covered_functions = 0usize;
        let mut total_branches = 0usize;
        let mut covered_branches = 0usize;

        for (file_path, fc) in &self.files {
            // Line coverage from registered source (skip blank/comment lines).
            if let Some(source_lines) = self.source_files.get(file_path) {
                for (i, line) in source_lines.iter().enumerate() {
                    let stripped = line.trim();
                    if stripped.is_empty()
                        || stripped.starts_with("//")
                        || stripped.starts_with('#')
                    {
                        continue;
                    }
                    total_lines += 1;
                    if fc.lines.get(&((i + 1) as u32)).copied().unwrap_or(0) > 0 {
                        covered_lines += 1;
                    }
                }
            } else {
                total_lines += fc.lines.len();
                covered_lines += fc.lines.values().filter(|c| **c > 0).count();
            }

            total_functions += fc.functions.len();
            covered_functions += fc.functions.values().filter(|c| **c > 0).count();

            // Branch coverage — unique (line, bid) pairs.
            let mut branch_locations: HashMap<u32, std::collections::HashSet<i64>> = HashMap::new();
            for (line, bid) in fc.branches.keys() {
                branch_locations.entry(*line).or_default().insert(*bid);
            }
            for (line, bids) in &branch_locations {
                total_branches += bids.len();
                covered_branches += bids
                    .iter()
                    .filter(|bid| fc.branches.get(&(*line, **bid)).copied().unwrap_or(0) > 0)
                    .count();
            }
        }

        let line_pct = if total_lines > 0 {
            (covered_lines as f64 / total_lines as f64 * 100.0 * 10.0).round() / 10.0
        } else {
            0.0
        };
        let func_pct = if total_functions > 0 {
            (covered_functions as f64 / total_functions as f64 * 100.0 * 10.0).round() / 10.0
        } else {
            0.0
        };
        let branch_pct = if total_branches > 0 {
            (covered_branches as f64 / total_branches as f64 * 100.0 * 10.0).round() / 10.0
        } else {
            0.0
        };

        json!({
            "lines": {"total": total_lines, "covered": covered_lines, "percent": line_pct},
            "functions": {"total": total_functions, "covered": covered_functions, "percent": func_pct},
            "branches": {"total": total_branches, "covered": covered_branches, "percent": branch_pct},
        })
    }

    /// `get_file_report` — per-file detailed report (or None if not found).
    pub fn get_file_report(&self, file_path: &str) -> Option<Value> {
        let p = std::path::Path::new(file_path);
        let abs_path = if p.is_absolute() {
            file_path.to_string()
        } else {
            p.canonicalize()
                .unwrap_or_else(|_| p.to_path_buf())
                .to_string_lossy()
                .to_string()
        };
        let fc = self.files.get(&abs_path)?;
        let source_lines = self.source_files.get(&abs_path);

        let mut lines_data: Vec<Value> = Vec::new();
        if let Some(src) = source_lines {
            for (i, line_text) in src.iter().enumerate() {
                lines_data.push(json!({
                    "line": i + 1,
                    "text": line_text,
                    "count": fc.lines.get(&((i + 1) as u32)).copied().unwrap_or(0),
                }));
            }
        } else {
            let mut observed: Vec<(&u32, &u64)> = fc.lines.iter().collect();
            observed.sort_by_key(|(l, _)| **l);
            for (line, count) in observed {
                lines_data.push(json!({"line": line, "text": "", "count": count}));
            }
        }

        let mut functions_data: Vec<Value> = Vec::new();
        let mut funcs: Vec<(&(String, u32), &u64)> = fc.functions.iter().collect();
        funcs.sort_by_key(|((name, line), _)| (name.clone(), *line));
        for ((name, line), count) in funcs {
            functions_data.push(json!({"name": name, "line": line, "count": count}));
        }

        let mut branches_data: Vec<Value> = Vec::new();
        let mut branch_locs: HashMap<u32, HashMap<i64, u64>> = HashMap::new();
        for ((line, bid), count) in &fc.branches {
            branch_locs.entry(*line).or_default().insert(*bid, *count);
        }
        let mut lines_sorted: Vec<(&u32, &HashMap<i64, u64>)> = branch_locs.iter().collect();
        lines_sorted.sort_by_key(|(l, _)| **l);
        for (line, bids) in lines_sorted {
            let mut b: Vec<(&i64, &u64)> = bids.iter().collect();
            b.sort_by_key(|(bid, _)| **bid);
            for (bid, count) in b {
                let label = if *bid == 1 {
                    "then"
                } else if *bid == 0 {
                    "else"
                } else {
                    "case"
                };
                branches_data.push(json!({
                    "line": line,
                    "branch_id": bid,
                    "label": label,
                    "count": count,
                }));
            }
        }

        Some(json!({
            "file": abs_path,
            "lines": lines_data,
            "functions": functions_data,
            "branches": branches_data,
        }))
    }

    /// `generate_report` — text/json/html report.
    pub fn generate_report(&self, format: &str) -> String {
        match format {
            "json" => self.generate_json_report(),
            "html" => self.generate_html_report(),
            _ => self.generate_text_report(),
        }
    }

    fn generate_text_report(&self) -> String {
        let summary = self.get_summary();
        let mut lines = vec![
            "=".repeat(60),
            "HELEN COVERAGE REPORT".into(),
            "=".repeat(60),
            String::new(),
            format!(
                "  Lines:     {}/{}  ({}%)",
                summary["lines"]["covered"], summary["lines"]["total"], summary["lines"]["percent"]
            ),
            format!(
                "  Functions: {}/{}  ({}%)",
                summary["functions"]["covered"],
                summary["functions"]["total"],
                summary["functions"]["percent"]
            ),
            format!(
                "  Branches:  {}/{}  ({}%)",
                summary["branches"]["covered"],
                summary["branches"]["total"],
                summary["branches"]["percent"]
            ),
            String::new(),
        ];

        if !self.files.is_empty() {
            lines.push("Files:".into());
            lines.push(format!("  {:<40} {:>10} {:>10}", "File", "Lines", "Funcs"));
            lines.push(format!(
                "  {} {} {}",
                "-".repeat(40),
                "-".repeat(10),
                "-".repeat(10)
            ));
            let mut files: Vec<(&String, &CoverageCount)> = self.files.iter().collect();
            files.sort_by_key(|(p, _)| p.to_string());
            for (file_path, fc) in files {
                let mut display_path = file_path.clone();
                if let Ok(rel) = std::path::Path::new(file_path)
                    .strip_prefix(std::env::current_dir().unwrap_or_default())
                {
                    display_path = rel.to_string_lossy().to_string();
                }
                if display_path.chars().count() > 40 {
                    display_path =
                        format!("...{}", &display_path[display_path.chars().count() - 37..]);
                }
                let line_total = fc.lines.len();
                let line_covered = fc.lines.values().filter(|c| **c > 0).count();
                let func_total = fc.functions.len();
                let func_covered = fc.functions.values().filter(|c| **c > 0).count();
                lines.push(format!(
                    "  {:<40} {:>4}/{:<5} {:>4}/{:<5}",
                    display_path, line_covered, line_total, func_covered, func_total
                ));
            }
        }
        lines.push(String::new());
        lines.push("=".repeat(60));
        lines.join("\n")
    }

    fn generate_json_report(&self) -> String {
        let summary = self.get_summary();
        let mut files_data = Map::new();
        for (file_path, fc) in &self.files {
            files_data.insert(
                file_path.clone(),
                json!({
                    "lines": fc.lines,
                    "functions": fc.functions.iter().map(|((n, l), c)| {
                        (format!("{n}@{l}"), c)
                    }).collect::<HashMap<_, _>>(),
                    "branches": fc.branches.iter().map(|((l, b), c)| {
                        (format!("{l}:{b}"), c)
                    }).collect::<HashMap<_, _>>(),
                }),
            );
        }
        let mut root = Map::new();
        root.insert("summary".into(), summary);
        root.insert("files".into(), Value::Object(files_data));
        serde_json::to_string_pretty(&Value::Object(root)).unwrap_or_default()
    }

    fn generate_html_report(&self) -> String {
        let summary = self.get_summary();
        let mut rows = String::new();
        let mut files: Vec<(&String, &CoverageCount)> = self.files.iter().collect();
        files.sort_by_key(|(p, _)| p.to_string());
        for (file_path, fc) in files {
            let line_total = fc.lines.len();
            let line_covered = fc.lines.values().filter(|c| **c > 0).count();
            rows.push_str(&format!(
                "<tr><td>{file_path}</td><td>{line_covered}/{line_total}</td><td>{}</td></tr>",
                fc.functions.values().filter(|c| **c > 0).count()
            ));
        }
        format!(
            "<html><head><title>Helen Coverage</title></head><body>\
             <h1>Helen Coverage Report</h1>\
             <p>Lines: {}/{} ({}%)</p>\
             <p>Functions: {}/{} ({}%)</p>\
             <p>Branches: {}/{} ({}%)</p>\
             <table border='1'><tr><th>File</th><th>Lines</th><th>Funcs</th></tr>{rows}</table>\
             </body></html>",
            summary["lines"]["covered"],
            summary["lines"]["total"],
            summary["lines"]["percent"],
            summary["functions"]["covered"],
            summary["functions"]["total"],
            summary["functions"]["percent"],
            summary["branches"]["covered"],
            summary["branches"]["total"],
            summary["branches"]["percent"],
        )
    }

    /// `save_to_file` — write report to a file, return the path.
    pub fn save_to_file(&self, output_path: &str, format: &str) -> String {
        let content = self.generate_report(format);
        std::fs::write(output_path, content).expect("write coverage report");
        output_path.to_string()
    }

    /// `merge` — merge another tracker's data into this one.
    pub fn merge(&mut self, other: &CoverageTracker) {
        for (file_path, oc) in &other.files {
            let fc = self.files.entry(file_path.clone()).or_default();
            for (line, c) in &oc.lines {
                *fc.lines.entry(*line).or_insert(0) += c;
            }
            for (k, c) in &oc.functions {
                *fc.functions.entry(k.clone()).or_insert(0) += c;
            }
            for (k, c) in &oc.branches {
                *fc.branches.entry(*k).or_insert(0) += c;
            }
        }
        for (p, src) in &other.source_files {
            self.source_files
                .entry(p.clone())
                .or_insert_with(|| src.clone());
        }
        self.total_counters = self
            .files
            .values()
            .map(|fc| fc.lines.len() + fc.functions.len() + fc.branches.len())
            .sum();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_noop() {
        let mut t = CoverageTracker::new(100);
        t.record_line(Some("/tmp/a.helen"), 3);
        assert_eq!(t.get_summary()["lines"]["total"], 0);
    }

    #[test]
    fn records_lines_functions_branches() {
        let mut t = CoverageTracker::new(100);
        t.set_enabled(true);
        t.record_line(Some("/tmp/a.helen"), 1);
        t.record_line(Some("/tmp/a.helen"), 1);
        t.record_line(Some("/tmp/a.helen"), 2);
        t.record_function(Some("/tmp/a.helen"), 1, "main");
        t.record_branch(Some("/tmp/a.helen"), 2, 0);
        t.record_branch(Some("/tmp/a.helen"), 2, 1);

        let s = t.get_summary();
        assert_eq!(s["lines"]["total"], 2);
        assert_eq!(s["lines"]["covered"], 2);
        assert_eq!(s["functions"]["total"], 1);
        assert_eq!(s["branches"]["total"], 2);
        assert_eq!(s["branches"]["covered"], 2);
    }

    #[test]
    fn source_registered_lines_skip_comments() {
        let mut t = CoverageTracker::new(100);
        t.set_enabled(true);
        t.register_source(
            "/tmp/b.helen",
            vec!["// comment".into(), String::new(), "let x = 1".into()],
        );
        t.record_line(Some("/tmp/b.helen"), 3);
        let s = t.get_summary();
        assert_eq!(s["lines"]["total"], 1);
        assert_eq!(s["lines"]["covered"], 1);
        assert_eq!(s["lines"]["percent"], 100.0);
    }

    #[test]
    fn counter_limit_stops_recording() {
        let mut t = CoverageTracker::new(3);
        t.set_enabled(true);
        for i in 0..10u32 {
            t.record_line(Some("/tmp/c.helen"), i);
        }
        assert_eq!(t.get_summary()["lines"]["total"], 3);
    }

    #[test]
    fn merge_combines() {
        let mut a = CoverageTracker::new(100);
        a.set_enabled(true);
        a.record_line(Some("/tmp/d.helen"), 1);
        let mut b = CoverageTracker::new(100);
        b.set_enabled(true);
        b.record_line(Some("/tmp/d.helen"), 2);
        a.merge(&b);
        assert_eq!(a.get_summary()["lines"]["total"], 2);
    }

    #[test]
    fn file_report_with_source() {
        let mut t = CoverageTracker::new(100);
        t.set_enabled(true);
        t.register_source("/tmp/e.helen", vec!["let x = 1".into()]);
        t.record_line(Some("/tmp/e.helen"), 1);
        let rep = t.get_file_report("/tmp/e.helen").expect("get report");
        assert_eq!(rep["lines"][0]["line"], 1);
        assert_eq!(rep["lines"][0]["count"], 1);
        assert_eq!(t.get_file_report("/nonexistent.helen"), None);
    }

    #[test]
    fn report_formats() {
        let mut t = CoverageTracker::new(100);
        t.set_enabled(true);
        t.record_line(Some("/tmp/f.helen"), 1);
        let text = t.generate_report("text");
        assert!(text.contains("HELEN COVERAGE REPORT"), "{text}");
        let json = t.generate_report("json");
        assert!(json.contains("\"summary\""), "{json}");
        let html = t.generate_report("html");
        assert!(html.contains("<html>"), "{html}");
    }
}
