//! Fuzzy matching module (Task 6.3) — port of `helen/runtime/fuzzy_match.py`.
//!
//! Implements a multi-strategy matching chain to robustly find and replace
//! text, accommodating whitespace/indentation/escaping variations common in
//! LLM-generated code. The 9-strategy chain, tried in order:
//!   1. exact              6. trimmed_boundary
//!   2. line_trimmed       7. unicode_normalized
//!   3. whitespace_norm    8. block_anchor
//!   4. indentation_flex   9. context_aware
//!   5. escape_normalized

use std::collections::HashMap;

/// Smart-quote / dash Unicode normalization map (Python `UNICODE_MAP`).
fn unicode_map() -> HashMap<char, &'static str> {
    let mut m = HashMap::new();
    m.insert('\u{201c}', "\"");
    m.insert('\u{201d}', "\"");
    m.insert('\u{2018}', "'");
    m.insert('\u{2019}', "'");
    m.insert('\u{2014}', "--");
    m.insert('\u{2013}', "-");
    m.insert('\u{2026}', "...");
    m.insert('\u{a0}', " ");
    m
}

/// Python `_unicode_normalize` — map Unicode punctuation to ASCII.
fn unicode_normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match unicode_map().get(&ch) {
            Some(repl) => out.push_str(repl),
            None => out.push(ch),
        }
    }
    out
}

/// SequenceMatcher-style ratio (difflib port — Ratcliff/Obershelp).
fn similarity(a: &str, b: &str) -> f64 {
    let aa: Vec<char> = a.chars().collect();
    let bb: Vec<char> = b.chars().collect();
    if aa.is_empty() && bb.is_empty() {
        return 1.0;
    }
    if aa.is_empty() || bb.is_empty() {
        return 0.0;
    }
    let total = aa.len() + bb.len();
    let mut matches = 0usize;
    let mut stack = vec![(0usize, aa.len(), 0usize, bb.len())];
    while let Some((a_start, a_len, b_start, b_len)) = stack.pop() {
        if a_len == 0 || b_len == 0 {
            continue;
        }
        // Find longest contiguous matching block.
        let mut best_len = 0usize;
        let mut best_i = 0usize;
        let mut best_j = 0usize;
        for i in 0..a_len {
            for j in 0..b_len {
                let mut k = 0usize;
                while i + k < a_len && j + k < b_len && aa[a_start + i + k] == bb[b_start + j + k] {
                    k += 1;
                }
                if k > best_len {
                    best_len = k;
                    best_i = i;
                    best_j = j;
                }
            }
        }
        if best_len > 0 {
            matches += best_len;
            // Recurse on the non-matching prefix/suffix regions.
            stack.push((a_start, best_i, b_start, best_j));
            stack.push((
                a_start + best_i + best_len,
                a_len - best_i - best_len,
                b_start + best_j + best_len,
                b_len - best_j - best_len,
            ));
        }
    }
    2.0 * matches as f64 / total as f64
}

// ---------------------------------------------------------------------------
// Position helpers
// ---------------------------------------------------------------------------

type Match = (usize, usize);
/// Strategy signature: `fn(original, new_string) -> Vec<Match>`.
type Strategy = fn(&str, &str) -> Vec<Match>;

fn calculate_line_positions(
    content_lines: &[&str],
    start_line: usize,
    end_line: usize,
    content_length: usize,
) -> (usize, usize) {
    let start_pos: usize = content_lines[..start_line]
        .iter()
        .map(|l| l.chars().count() + 1)
        .sum();
    let end_pos = {
        let raw: usize = content_lines[..end_line]
            .iter()
            .map(|l| l.chars().count() + 1)
            .sum();
        if raw == 0 {
            0
        } else {
            raw - 1
        }
    };
    (start_pos, end_pos.min(content_length))
}

/// Split into lines keeping no trailing newline (Python `str.split('\n')`).
fn split_lines(text: &str) -> Vec<&str> {
    text.split('\n').collect()
}

/// `_strategy_exact` — direct substring match.
fn strategy_exact(content: &str, pattern: &str) -> Vec<Match> {
    let mut matches = Vec::new();
    let bytes = content.as_bytes();
    let pat = pattern.as_bytes();
    if pat.is_empty() {
        return matches;
    }
    let mut start = 0usize;
    while let Some(pos) = find_subslice(&bytes[start..], pat) {
        let abs = start + pos;
        matches.push((abs, abs + pat.len()));
        start = abs + 1;
    }
    matches
}

/// Byte-wise substring search (no regex, O(n·m) fine for patches).
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// `_find_normalized_matches` — line-block equality on normalized lines,
/// mapped back to original char positions.
fn find_normalized_matches(
    content: &str,
    content_lines: &[&str],
    content_norm_lines: &[&str],
    pattern_normalized: &str,
) -> Vec<Match> {
    let pat_lines = split_lines(pattern_normalized);
    let num = pat_lines.len();
    if num == 0 || content_norm_lines.len() < num {
        return Vec::new();
    }
    let mut matches = Vec::new();
    for i in 0..=(content_norm_lines.len() - num) {
        let block = content_norm_lines[i..i + num].join("\n");
        if block == pattern_normalized {
            let (s, e) =
                calculate_line_positions(content_lines, i, i + num, content.chars().count());
            matches.push((s, e));
        }
    }
    matches
}

/// `_map_normalized_positions` — map normalized-string positions back to
/// original (Python uses a char-walk; we approximate with an offset map).
fn map_normalized_positions(
    original: &str,
    normalized: &str,
    norm_matches: &[Match],
) -> Vec<Match> {
    if norm_matches.is_empty() {
        return Vec::new();
    }
    // Build per-char original->normalized offset table.
    let orig_chars: Vec<char> = original.chars().collect();
    let norm_chars: Vec<char> = normalized.chars().collect();
    let mut orig_to_norm: Vec<usize> = Vec::with_capacity(orig_chars.len() + 1);
    let mut oi = 0usize;
    let mut ni = 0usize;
    while oi < orig_chars.len() && ni < norm_chars.len() {
        if orig_chars[oi] == norm_chars[ni] {
            orig_to_norm.push(ni);
            oi += 1;
            ni += 1;
        } else if (orig_chars[oi] == ' ' || orig_chars[oi] == '\t') && norm_chars[ni] == ' ' {
            orig_to_norm.push(ni);
            oi += 1;
            if oi < orig_chars.len() && orig_chars[oi] != ' ' && orig_chars[oi] != '\t' {
                ni += 1;
            }
        } else {
            // Python has two identical branches here (`orig in ' \t'` and `else`);
            // both append the current normalized index and advance. Merged.
            orig_to_norm.push(ni);
            oi += 1;
        }
    }
    while oi < orig_chars.len() {
        orig_to_norm.push(norm_chars.len());
        oi += 1;
    }
    // norm_pos -> first/last orig index.
    let mut norm_to_orig_start: HashMap<usize, usize> = HashMap::new();
    let mut norm_to_orig_end: HashMap<usize, usize> = HashMap::new();
    for (op, np) in orig_to_norm.iter().enumerate() {
        norm_to_orig_start.entry(*np).or_insert(op);
        norm_to_orig_end.insert(*np, op);
    }
    let orig_len = orig_chars.len();
    let mut out = Vec::new();
    for (ns, ne) in norm_matches {
        let os = match norm_to_orig_start.get(ns) {
            Some(v) => *v,
            None => orig_to_norm
                .iter()
                .position(|n| *n >= *ns)
                .unwrap_or(orig_len),
        };
        let oe = if *ne == 0 {
            os
        } else {
            match norm_to_orig_end.get(&(ne - 1)) {
                Some(v) => v + 1,
                None => os + (ne - ns),
            }
        };
        let mut oe = oe.min(orig_len);
        while oe < orig_len && (orig_chars[oe] == ' ' || orig_chars[oe] == '\t') {
            oe += 1;
        }
        out.push((os, oe.min(orig_len)));
    }
    out
}

// ---------------------------------------------------------------------------
// Strategy 1..=9
// ---------------------------------------------------------------------------

fn strategy_line_trimmed(content: &str, pattern: &str) -> Vec<Match> {
    let pattern_normalized = pattern
        .split('\n')
        .map(|l| l.trim())
        .collect::<Vec<_>>()
        .join("\n");
    let content_lines = split_lines(content);
    let content_norm: Vec<&str> = content_lines.iter().map(|l| l.trim()).collect();
    find_normalized_matches(content, &content_lines, &content_norm, &pattern_normalized)
}

fn normalize_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for ch in s.chars() {
        if ch == ' ' || ch == '\t' {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out
}

fn strategy_whitespace_normalized(content: &str, pattern: &str) -> Vec<Match> {
    let pat_norm = normalize_ws(pattern);
    let content_norm = normalize_ws(content);
    let inner = strategy_exact(&content_norm, &pat_norm);
    map_normalized_positions(content, &content_norm, &inner)
}

fn strategy_indentation_flexible(content: &str, pattern: &str) -> Vec<Match> {
    let content_lines = split_lines(content);
    let content_stripped: Vec<&str> = content_lines.iter().map(|l| l.trim_start()).collect();
    let pattern_normalized = pattern
        .split('\n')
        .map(|l| l.trim_start())
        .collect::<Vec<_>>()
        .join("\n");
    find_normalized_matches(
        content,
        &content_lines,
        &content_stripped,
        &pattern_normalized,
    )
}

fn unescape(s: &str) -> String {
    s.replace("\\n", "\n")
        .replace("\\t", "\t")
        .replace("\\r", "\r")
}

fn strategy_escape_normalized(content: &str, pattern: &str) -> Vec<Match> {
    let pattern_unescaped = unescape(pattern);
    if pattern_unescaped == pattern {
        return Vec::new();
    }
    strategy_exact(content, &pattern_unescaped)
}

fn strategy_trimmed_boundary(content: &str, pattern: &str) -> Vec<Match> {
    let mut pat_lines: Vec<&str> = pattern.split('\n').collect();
    if pat_lines.is_empty() {
        return Vec::new();
    }
    pat_lines[0] = pat_lines[0].trim();
    if pat_lines.len() > 1 {
        let last = pat_lines.len() - 1;
        pat_lines[last] = pat_lines[last].trim();
    }
    let modified = pat_lines.join("\n");
    let content_lines = split_lines(content);
    let n = pat_lines.len();
    if content_lines.len() < n {
        return Vec::new();
    }
    let mut matches = Vec::new();
    for i in 0..=(content_lines.len() - n) {
        let mut check: Vec<&str> = content_lines[i..i + n].to_vec();
        check[0] = check[0].trim();
        if check.len() > 1 {
            let last = check.len() - 1;
            check[last] = check[last].trim();
        }
        if check.join("\n") == modified {
            matches.push(calculate_line_positions(
                &content_lines,
                i,
                i + n,
                content.chars().count(),
            ));
        }
    }
    matches
}

fn build_orig_to_norm_map(original: &str) -> Vec<usize> {
    let mut result = Vec::new();
    let mut norm_pos = 0usize;
    let map = unicode_map();
    for ch in original.chars() {
        result.push(norm_pos);
        match map.get(&ch) {
            Some(repl) => norm_pos += repl.chars().count(),
            None => norm_pos += 1,
        }
    }
    result.push(norm_pos);
    result
}

fn map_positions_norm_to_orig(orig_to_norm: &[usize], norm_matches: &[Match]) -> Vec<Match> {
    let mut norm_to_orig_start: HashMap<usize, usize> = HashMap::new();
    for (orig_pos, norm_pos) in orig_to_norm[..orig_to_norm.len() - 1].iter().enumerate() {
        norm_to_orig_start.entry(*norm_pos).or_insert(orig_pos);
    }
    let mut results = Vec::new();
    let orig_len = orig_to_norm.len() - 1;
    for (ns, ne) in norm_matches {
        let orig_start = match norm_to_orig_start.get(ns) {
            Some(v) => *v,
            None => continue,
        };
        let mut orig_end = orig_start;
        while orig_end < orig_len && orig_to_norm[orig_end] < *ne {
            orig_end += 1;
        }
        results.push((orig_start, orig_end));
    }
    results
}

fn strategy_unicode_normalized(content: &str, pattern: &str) -> Vec<Match> {
    let norm_pattern = unicode_normalize(pattern);
    let norm_content = unicode_normalize(content);
    if norm_content == content && norm_pattern == pattern {
        return Vec::new();
    }
    let mut norm_matches = strategy_exact(&norm_content, &norm_pattern);
    if norm_matches.is_empty() {
        norm_matches = strategy_line_trimmed(&norm_content, &norm_pattern);
    }
    if norm_matches.is_empty() {
        return Vec::new();
    }
    let orig_to_norm = build_orig_to_norm_map(content);
    map_positions_norm_to_orig(&orig_to_norm, &norm_matches)
}

fn strategy_block_anchor(content: &str, pattern: &str) -> Vec<Match> {
    let norm_pattern = unicode_normalize(pattern);
    let norm_content = unicode_normalize(content);
    let pattern_lines: Vec<&str> = norm_pattern.split('\n').collect();
    if pattern_lines.len() < 2 {
        return Vec::new();
    }
    let first_line = pattern_lines[0].trim();
    let last_line = pattern_lines[pattern_lines.len() - 1].trim();
    let norm_content_lines: Vec<&str> = norm_content.split('\n').collect();
    let orig_content_lines: Vec<&str> = content.split('\n').collect();
    let n = pattern_lines.len();
    if norm_content_lines.len() < n {
        return Vec::new();
    }
    let mut potential = Vec::new();
    for i in 0..=(norm_content_lines.len() - n) {
        if norm_content_lines[i].trim() == first_line
            && norm_content_lines[i + n - 1].trim() == last_line
        {
            potential.push(i);
        }
    }
    let threshold = if potential.len() == 1 { 0.50 } else { 0.70 };
    let mut matches = Vec::new();
    for i in potential {
        let similarity = if n <= 2 {
            1.0
        } else {
            let content_middle = norm_content_lines[i + 1..i + n - 1].join("\n");
            let pattern_middle = pattern_lines[1..n - 1].join("\n");
            similarity(&content_middle, &pattern_middle)
        };
        if similarity >= threshold {
            matches.push(calculate_line_positions(
                &orig_content_lines,
                i,
                i + n,
                content.chars().count(),
            ));
        }
    }
    matches
}

fn strategy_context_aware(content: &str, pattern: &str) -> Vec<Match> {
    let pattern_lines: Vec<&str> = pattern.split('\n').collect();
    let content_lines: Vec<&str> = content.split('\n').collect();
    if pattern_lines.is_empty() {
        return Vec::new();
    }
    let n = pattern_lines.len();
    if content_lines.len() < n {
        return Vec::new();
    }
    let mut matches = Vec::new();
    for i in 0..=(content_lines.len() - n) {
        let block = &content_lines[i..i + n];
        let mut high = 0usize;
        for (p_line, c_line) in pattern_lines.iter().zip(block.iter()) {
            if similarity(p_line.trim(), c_line.trim()) >= 0.80 {
                high += 1;
            }
        }
        if (high as f64) >= (pattern_lines.len() as f64) * 0.5 {
            matches.push(calculate_line_positions(
                &content_lines,
                i,
                i + n,
                content.chars().count(),
            ));
        }
    }
    matches
}

// ---------------------------------------------------------------------------
// Replacement helpers
// ---------------------------------------------------------------------------

fn leading_whitespace(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    &line[..i]
}

fn first_meaningful_line(text: &str) -> Option<&str> {
    text.split('\n').find(|l| !l.trim().is_empty())
}

/// Python `_reindent_replacement` — align new_string indentation to the file.
fn reindent_replacement(file_region: &str, old_string: &str, new_string: &str) -> String {
    if new_string.is_empty() {
        return new_string.to_string();
    }
    let old_first = first_meaningful_line(old_string);
    let file_first = first_meaningful_line(file_region);
    let (Some(old_first), Some(file_first)) = (old_first, file_first) else {
        return new_string.to_string();
    };
    let old_indent = leading_whitespace(old_first).to_string();
    let file_indent = leading_whitespace(file_first).to_string();
    if old_indent == file_indent {
        return new_string.to_string();
    }
    let mut out: Vec<String> = Vec::new();
    for line in new_string.split('\n') {
        if line.trim().is_empty() {
            out.push(line.to_string());
            continue;
        }
        let line_indent = leading_whitespace(line);
        if line_indent.starts_with(old_indent.as_str()) {
            let remainder = &line[old_indent.len()..];
            out.push(format!("{file_indent}{remainder}"));
        } else {
            out.push(format!(
                "{file_indent}{}",
                line.trim_start_matches([' ', '\t'])
            ));
        }
    }
    out.join("\n")
}

fn maybe_unescape_new_string(new_string: &str, content: &str, matches: &[Match]) -> String {
    if !new_string.contains("\\t") && !new_string.contains("\\r") {
        return new_string.to_string();
    }
    let matched_regions: String = matches
        .iter()
        .map(|(s, e)| content.chars().skip(*s).take(e - s).collect::<String>())
        .collect();
    let mut out = new_string.to_string();
    if out.contains("\\t") && matched_regions.contains('\t') {
        out = out.replace("\\t", "\t");
    }
    if out.contains("\\r") && matched_regions.contains('\r') {
        out = out.replace("\\r", "\r");
    }
    out
}

fn apply_replacements(
    content: &str,
    matches: &[Match],
    new_string: &str,
    old_string: Option<&str>,
) -> String {
    let mut sorted: Vec<&Match> = matches.iter().collect();
    sorted.sort_by_key(|m| std::cmp::Reverse(m.0));
    let mut result = content.to_string();
    for (start, end) in sorted {
        let adjusted = match old_string {
            Some(os) => {
                let region: String = content.chars().skip(*start).take(end - start).collect();
                reindent_replacement(&region, os, new_string)
            }
            None => new_string.to_string(),
        };
        // Replace by char index (Python slices are char-based).
        let mut chars: Vec<char> = result.chars().collect();
        if *start <= chars.len() && *end <= chars.len() {
            let tail: String = chars[*end..].iter().collect();
            chars.truncate(*start);
            let head: String = chars.iter().collect();
            result = format!("{head}{adjusted}{tail}");
        }
    }
    result
}

/// Escape-drift guard (Python `_detect_escape_drift`).
fn detect_escape_drift(
    content: &str,
    matches: &[Match],
    old_string: &str,
    new_string: &str,
) -> Option<String> {
    if !new_string.contains("\\'") && !new_string.contains("\\\"") {
        return None;
    }
    let matched_regions: String = matches
        .iter()
        .map(|(s, e)| content.chars().skip(*s).take(e - s).collect::<String>())
        .collect();
    for suspect in ["\\'", "\\\""] {
        if new_string.contains(suspect)
            && old_string.contains(suspect)
            && !matched_regions.contains(suspect)
        {
            let plain = suspect.chars().nth(1).unwrap_or('\'');
            return Some(format!(
                "Escape-drift detected: old_string and new_string contain \
                 the literal sequence {suspect:?} but the matched region of \
                 the file does not. This is almost always a tool-call \
                 serialization artifact where an apostrophe or quote got \
                 prefixed with a spurious backslash. Re-read the file with \
                 read_file and pass old_string/new_string without \
                 backslash-escaping {plain:?} characters."
            ));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Result of a fuzzy find-and-replace.
#[derive(Debug)]
pub struct FuzzyResult {
    pub new_content: String,
    pub match_count: usize,
    pub strategy: Option<String>,
    pub error: Option<String>,
}

/// Python `fuzzy_find_and_replace` — the 9-strategy chain.
pub fn fuzzy_find_and_replace(
    content: &str,
    old_string: &str,
    new_string: &str,
    replace_all: bool,
) -> FuzzyResult {
    if old_string.is_empty() {
        return FuzzyResult {
            new_content: content.to_string(),
            match_count: 0,
            strategy: None,
            error: Some("old_string cannot be empty".to_string()),
        };
    }
    if old_string == new_string {
        return FuzzyResult {
            new_content: content.to_string(),
            match_count: 0,
            strategy: None,
            error: Some("old_string and new_string are identical".to_string()),
        };
    }
    let strategies: Vec<(&str, Strategy)> = vec![
        ("exact", strategy_exact),
        ("line_trimmed", strategy_line_trimmed),
        ("whitespace_normalized", strategy_whitespace_normalized),
        ("indentation_flexible", strategy_indentation_flexible),
        ("escape_normalized", strategy_escape_normalized),
        ("trimmed_boundary", strategy_trimmed_boundary),
        ("unicode_normalized", strategy_unicode_normalized),
        ("block_anchor", strategy_block_anchor),
        ("context_aware", strategy_context_aware),
    ];
    for (name, strat) in strategies {
        let matches = strat(content, old_string);
        if matches.is_empty() {
            continue;
        }
        if matches.len() > 1 && !replace_all {
            return FuzzyResult {
                new_content: content.to_string(),
                match_count: 0,
                strategy: None,
                error: Some(format!(
                    "Found {} matches for old_string. Provide more context to make it unique, \
                     or use replace_all=True.",
                    matches.len()
                )),
            };
        }
        if name != "exact" {
            if let Some(drift) = detect_escape_drift(content, &matches, old_string, new_string) {
                return FuzzyResult {
                    new_content: content.to_string(),
                    match_count: 0,
                    strategy: None,
                    error: Some(drift),
                };
            }
        }
        let effective_new = maybe_unescape_new_string(new_string, content, &matches);
        let new_content = apply_replacements(
            content,
            &matches,
            &effective_new,
            if name == "exact" {
                None
            } else {
                Some(old_string)
            },
        );
        return FuzzyResult {
            new_content,
            match_count: matches.len(),
            strategy: Some(name.to_string()),
            error: None,
        };
    }
    FuzzyResult {
        new_content: content.to_string(),
        match_count: 0,
        strategy: None,
        error: Some("Could not find a match for old_string in the file".to_string()),
    }
}

/// `find_closest_lines` — 'did you mean?' feedback snippet.
pub fn find_closest_lines(
    old_string: &str,
    content: &str,
    context_lines: usize,
    max_results: usize,
) -> String {
    if old_string.is_empty() || content.is_empty() {
        return String::new();
    }
    let old_lines: Vec<&str> = old_string.split('\n').collect();
    let content_lines: Vec<&str> = content.split('\n').collect();
    if old_lines.is_empty() || content_lines.is_empty() {
        return String::new();
    }
    let anchor = match old_lines[0].trim() {
        "" => old_lines.iter().map(|l| l.trim()).find(|l| !l.is_empty()),
        a => Some(a),
    };
    let Some(anchor) = anchor else {
        return String::new();
    };
    let mut scored: Vec<(f64, usize)> = Vec::new();
    for (i, line) in content_lines.iter().enumerate() {
        let stripped = line.trim();
        if stripped.is_empty() {
            continue;
        }
        let ratio = similarity(anchor, stripped);
        if ratio > 0.3 {
            scored.push((ratio, i));
        }
    }
    if scored.is_empty() {
        return String::new();
    }
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(max_results);
    let mut parts: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
    for (_, line_idx) in scored {
        let start = line_idx.saturating_sub(context_lines);
        let end = (line_idx + old_lines.len() + context_lines).min(content_lines.len());
        let key = (start, end);
        if !seen.insert(key) {
            continue;
        }
        let mut snippet = String::new();
        for j in 0..(end - start) {
            if j > 0 {
                snippet.push('\n');
            }
            snippet.push_str(&format!(
                "{:>4}| {}",
                start + j + 1,
                content_lines[start + j]
            ));
        }
        parts.push(snippet);
    }
    if parts.is_empty() {
        return String::new();
    }
    parts.join("\n---\n")
}

/// Python `format_no_match_hint`.
pub fn format_no_match_hint(
    error: &str,
    match_count: usize,
    old_string: &str,
    content: &str,
) -> String {
    if match_count != 0 {
        return String::new();
    }
    if !error.starts_with("Could not find") {
        return String::new();
    }
    let hint = find_closest_lines(old_string, content, 2, 3);
    if hint.is_empty() {
        return String::new();
    }
    format!("\n\nDid you mean one of these sections?\n{hint}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fr(content: &str, old: &str, new: &str, replace_all: bool) -> FuzzyResult {
        fuzzy_find_and_replace(content, old, new, replace_all)
    }

    #[test]
    fn exact_match() {
        let r = fr("def foo():\n    pass", "def foo():", "def bar():", false);
        assert!(r.error.is_none());
        assert_eq!(r.match_count, 1);
        assert_eq!(r.strategy.as_deref(), Some("exact"));
        assert_eq!(r.new_content, "def bar():\n    pass");
    }

    #[test]
    fn exact_multiple_requires_unique() {
        let r = fr("a\nb\na\n", "a", "x", false);
        assert!(r.error.is_some());
        assert!(r.error.unwrap().contains("2 matches"));
    }

    #[test]
    fn replace_all_handles_multi() {
        let r = fr("a\nb\na\n", "a", "x", true);
        assert!(r.error.is_none());
        assert_eq!(r.match_count, 2);
        assert_eq!(r.new_content, "x\nb\nx\n");
    }

    #[test]
    fn line_trimmed_strategy() {
        // old_string has trailing spaces on a line that file doesn't.
        let r = fr(
            "def foo():\n    pass\n",
            "def foo():  \n    pass",
            "def bar():\n    pass",
            false,
        );
        assert!(r.error.is_none());
        assert_eq!(r.strategy.as_deref(), Some("line_trimmed"));
    }

    #[test]
    fn whitespace_normalized_strategy() {
        let r = fr("a   b", "a b", "x", false);
        assert!(r.error.is_none());
        assert_eq!(r.strategy.as_deref(), Some("whitespace_normalized"));
        assert_eq!(r.new_content, "x");
    }

    #[test]
    fn indentation_flexible_strategy() {
        // Python chain order: line_trimmed fires first for this input
        // (both trim to the same block). Verify the strategy fn directly.
        let m = strategy_indentation_flexible("    if x:\n        y()", "if x:\n    y()");
        assert_eq!(m.len(), 1);
        let r = fr(
            "    if x:\n        y()",
            "if x:\n    y()",
            "if z:\n    y()",
            false,
        );
        assert!(r.error.is_none());
        assert_eq!(r.strategy.as_deref(), Some("line_trimmed"));
    }

    #[test]
    fn escape_normalized_strategy() {
        let r = fr("line1\nline2", "line1\\nline2", "x", false);
        assert!(r.error.is_none());
        assert_eq!(r.strategy.as_deref(), Some("escape_normalized"));
        assert_eq!(r.new_content, "x");
    }

    #[test]
    fn trimmed_boundary_strategy() {
        // Python chain order: line_trimmed fires first. Verify the strategy fn.
        let m = strategy_trimmed_boundary("  hello  \nworld", "hello\nworld");
        assert_eq!(m.len(), 1);
        let r = fr("  hello  \nworld", "hello\nworld", "x\ny", false);
        assert!(r.error.is_none());
        assert_eq!(r.strategy.as_deref(), Some("line_trimmed"));
    }

    #[test]
    fn unicode_normalized_strategy() {
        let r = fr("hello \u{201c}world\u{201d}", "hello \"world\"", "x", false);
        assert!(r.error.is_none());
        assert_eq!(r.strategy.as_deref(), Some("unicode_normalized"));
    }

    #[test]
    fn block_anchor_strategy() {
        let content = "def foo():\n    return 1\n\ndef bar():\n    return 2";
        let old = "def foo():\n    return 99\n\ndef bar():";
        let r = fr(
            content,
            old,
            "def baz():\n    return 3\n\ndef bar():",
            false,
        );
        assert!(r.error.is_none());
        assert_eq!(r.strategy.as_deref(), Some("block_anchor"));
    }

    #[test]
    fn context_aware_strategy() {
        let content = "def foo():\n    x = 1\n    return x";
        let old = "def fooo():\n    x = 2\n    return x";
        let r = fr(content, old, "def bar():\n    x = 3\n    return x", false);
        assert!(r.error.is_none());
        assert_eq!(r.strategy.as_deref(), Some("context_aware"));
    }

    #[test]
    fn no_match_error() {
        let r = fr("abc", "zzz", "x", false);
        assert!(r.error.is_some());
        assert_eq!(r.match_count, 0);
    }

    #[test]
    fn empty_old_string_error() {
        let r = fr("abc", "", "x", false);
        assert!(r.error.as_deref() == Some("old_string cannot be empty"));
    }

    #[test]
    fn identical_error() {
        let r = fr("abc", "abc", "abc", false);
        assert!(r.error.is_some());
    }

    #[test]
    fn reindent_preserves_nested_indent() {
        // new_string has deeper indent than old; file region has even deeper.
        let content = "fn a() {\n        inner();\n}\n";
        let old = "fn a() {\n    inner();\n}";
        let r = fr(content, old, "fn b() {\n        inner();\n}", false);
        assert!(r.error.is_none());
        assert_eq!(r.new_content, "fn b() {\n        inner();\n}\n");
    }

    #[test]
    fn similarity_ratio_basics() {
        assert_eq!(similarity("", ""), 1.0);
        assert!(similarity("abc", "abc") > 0.99);
        assert!(similarity("abc", "xyz") < 0.2);
        assert!(similarity("hello world", "hello wrld") > 0.7);
    }

    #[test]
    fn find_closest_lines_basic() {
        let content = "line one\nline two\nline three";
        let hint = find_closest_lines("line twoo", content, 0, 1);
        assert!(hint.contains("line two"));
    }

    #[test]
    fn format_no_match_hint_gate() {
        // "abc" is similar to itself -> hint is non-empty (Python parity).
        assert!(!format_no_match_hint("Could not find a match", 0, "abc", "abc").is_empty());
        assert_eq!(format_no_match_hint("other error", 0, "abc", "abc"), "");
        assert_eq!(
            format_no_match_hint("Could not find a match", 1, "abc", "abc"),
            ""
        );
        assert_eq!(
            format_no_match_hint("Could not find a match", 0, "zzz", "abc"),
            ""
        );
    }
}
