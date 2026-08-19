//! std.data format operations — HTML / Markdown / TOML / XML / YAML.
//!
//! Port of Python `helen/stdlib/data.py` (`_html_*`, `_markdown_*`) and
//! `helen/stdlib/data_formats.py` (`_toml_*`, `_xml_*`, `_yaml_*`).
//!
//! HTML and Markdown are regex/line-based (Python uses no external deps).
//! TOML/XML/YAML use the `toml`, `quick-xml` and `serde_yaml` crates.

use std::cell::RefCell;
use std::rc::Rc;

use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::exceptions::ExceptionValue;
use crate::interpreter::Interpreter;
use crate::stdlib::{json_to_value, value_to_json};
use crate::value::Value;

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn arg_str(args: &[Value], i: usize) -> Result<&str, ExceptionValue> {
    match args.get(i) {
        Some(Value::Str(s)) => Ok(s),
        Some(_) => Err(ExceptionValue::new(
            "RuntimeError",
            format!("expected string at argument {}", i + 1),
            None,
        )),
        None => Err(ExceptionValue::new(
            "RuntimeError",
            format!("missing argument {}", i + 1),
            None,
        )),
    }
}

fn value_to_map(v: &Value) -> Result<indexmap::IndexMap<Value, Value>, ExceptionValue> {
    match v {
        Value::Map(m) => Ok(m.borrow().clone()),
        other => Err(ExceptionValue::new(
            "TypeError",
            format!("expected dict, got {}", other.type_name()),
            None,
        )),
    }
}

fn map_get_str(m: &indexmap::IndexMap<Value, Value>, key: &str) -> Option<Value> {
    m.get(&Value::Str(Rc::from(key))).cloned()
}

// ---------------------------------------------------------------------------
// HTML
// ---------------------------------------------------------------------------

/// Python `_html_parse` — regex-based: `<tag attrs>content</tag>` or plain text.
pub fn data_html_parse(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let text = arg_str(args, 0)?;
    let mut tag = "text".to_string();
    let mut attrs: indexmap::IndexMap<Value, Value> = indexmap::IndexMap::new();
    let children: Vec<Value> = Vec::new();
    let mut content = text.to_string();

    if let Some(open) = text.find('<') {
        if let Some(gt) = text[open..].find('>') {
            let tag_part = &text[open + 1..open + gt];
            let tag_name: String = tag_part
                .chars()
                .take_while(|c| c.is_alphanumeric())
                .collect();
            if !tag_name.is_empty() {
                let close_tag = format!("</{}>", tag_name);
                if let Some(close) = text[open + gt + 1..].find(&close_tag) {
                    let attrs_str = &tag_part[tag_name.len()..];
                    // Parse attributes: name="value"
                    let mut pos = 0;
                    while pos < attrs_str.len() {
                        let rest = &attrs_str[pos..];
                        let name_start = rest.find(|c: char| c.is_alphanumeric() || c == '-');
                        let Some(ns) = name_start else { break };
                        let name_end = rest[ns..]
                            .find(|c: char| !c.is_alphanumeric() && c != '-')
                            .map(|e| ns + e)
                            .unwrap_or(rest.len());
                        let name = &rest[ns..name_end];
                        let after = &rest[name_end..];
                        if let Some(eq) = after.find('=') {
                            let val_rest = after[eq + 1..].trim_start();
                            if let Some(q) = val_rest.chars().next() {
                                if q == '"' || q == '\'' {
                                    if let Some(qe) = val_rest[1..].find(q) {
                                        let val = &val_rest[1..qe + 1];
                                        attrs.insert(
                                            Value::Str(Rc::from(name)),
                                            Value::Str(Rc::from(val)),
                                        );
                                        pos += ns + name_end + eq + 1 + 1 + qe + 1;
                                        continue;
                                    }
                                }
                            }
                        }
                        pos += name_end.max(1);
                    }
                    content = text[open + gt + 1..open + gt + 1 + close].to_string();
                    tag = tag_name;
                }
            }
        }
    }

    let mut result = indexmap::IndexMap::new();
    result.insert(Value::Str(Rc::from("tag")), Value::Str(Rc::from(tag.as_str())));
    result.insert(Value::Str(Rc::from("attrs")), Value::Map(Rc::new(RefCell::new(attrs))));
    result.insert(Value::Str(Rc::from("children")), Value::List(Rc::new(RefCell::new(children))));
    result.insert(Value::Str(Rc::from("text")), Value::Str(Rc::from(content.as_str())));
    Ok(Value::Map(Rc::new(RefCell::new(result))))
}

/// Python `_html_text` — strip tags, decode common entities.
pub fn data_html_text(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let text = arg_str(args, 0)?;
    // Remove all HTML tags `<...>`.
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(lt) = rest.find('<') {
        out.push_str(&rest[..lt]);
        let after = &rest[lt..];
        let Some(gt) = after.find('>') else {
            out.push_str(after);
            rest = "";
            break;
        };
        rest = &after[gt + 1..];
    }
    out.push_str(rest);
    // Decode common HTML entities.
    out = out.replace("&lt;", "<");
    out = out.replace("&gt;", ">");
    out = out.replace("&amp;", "&");
    out = out.replace("&quot;", "\"");
    out = out.replace("&#39;", "'");
    Ok(Value::Str(Rc::from(out.trim().to_string().as_str())))
}

/// Python `_html_links` — extract `href="..."` values.
pub fn data_html_links(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let text = arg_str(args, 0)?;
    let mut links = Vec::new();
    let mut rest = text;
    while let Some(pos) = rest.find("href=") {
        let after = &rest[pos + 5..];
        let after = after.trim_start();
        if let Some(q) = after.chars().next() {
            if q == '"' || q == '\'' {
                if let Some(end) = after[1..].find(q) {
                    links.push(Value::Str(Rc::from(&after[1..end + 1])));
                    rest = &after[end + 2..];
                    continue;
                }
            }
        }
        rest = &after[1..];
    }
    Ok(Value::List(Rc::new(RefCell::new(links))))
}

/// Python `_html_select` — basic CSS selector support.
/// Selector components: tag name, `.class`, `#id`, `[attr]`, `[attr=value]`.
pub fn data_html_select(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let text = arg_str(args, 0)?;
    let selector = arg_str(args, 1)?.trim().to_string();
    if selector.is_empty() {
        return Err(ExceptionValue::new(
            "ValueError",
            "Selector cannot be empty".to_string(),
            None,
        ));
    }

    let mut sel = selector.clone();
    // [attr] or [attr=value]
    let mut attr_name: Option<String> = None;
    let mut attr_value: Option<String> = None;
    if let Some(ab) = sel.find('[') {
        if let Some(ae_rel) = sel[ab..].find(']') {
            let ae = ab + ae_rel;
            let inner = &sel[ab + 1..ae];
            if let Some(eq) = inner.find('=') {
                attr_name = Some(inner[..eq].trim().to_string());
                attr_value = Some(
                    inner[eq + 1..]
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string(),
                );
            } else {
                attr_name = Some(inner.trim().to_string());
            }
            sel = format!("{}{}", &sel[..ab], &sel[ae + 1..]);
        }
    }
    // #id
    let mut id_value: Option<String> = None;
    if let Some(ib) = sel.find('#') {
        let rest = &sel[ib + 1..];
        let end = rest
            .find(|c: char| !c.is_alphanumeric() && c != '-')
            .unwrap_or(rest.len());
        if end > 0 {
            id_value = Some(rest[..end].to_string());
            sel = format!("{}{}", &sel[..ib], &rest[end..]);
        }
    }
    // .class
    let mut class_value: Option<String> = None;
    if let Some(cb) = sel.find('.') {
        let rest = &sel[cb + 1..];
        let end = rest
            .find(|c: char| !c.is_alphanumeric() && c != '-')
            .unwrap_or(rest.len());
        if end > 0 {
            class_value = Some(rest[..end].to_string());
            sel = format!("{}{}", &sel[..cb], &rest[end..]);
        }
    }
    let tag_pattern = sel.trim().to_string();
    if tag_pattern.is_empty() && id_value.is_none() && class_value.is_none() && attr_name.is_none() {
        return Err(ExceptionValue::new(
            "ValueError",
            format!("Invalid selector: '{selector}'"),
            None,
        ));
    }

    // Find all tags `<tag attrs>content</tag>` or `<tag attrs/>`.
    let mut results = Vec::new();
    let mut rest = text;
    while let Some(lt) = rest.find('<') {
        let after = &rest[lt + 1..];
        // Skip comments and closing tags.
        if after.starts_with('!') || after.starts_with('?') || after.starts_with('/') {
            let end = after.find('>').map(|e| e + 1).unwrap_or(after.len());
            rest = &after[end..];
            continue;
        }
        let tag_name: String = after
            .chars()
            .take_while(|c| c.is_alphanumeric())
            .collect();
        if tag_name.is_empty() {
            let end = after.find('>').map(|e| e + 1).unwrap_or(after.len());
            rest = &after[end..];
            continue;
        }
        // Find end of open tag.
        let Some(gt_rel) = after.find('>') else {
            break;
        };
        let gt = gt_rel;
        let attrs_str = &after[tag_name.len()..gt];
        // Self-closing?
        let self_closing = attrs_str.trim_end().ends_with('/');
        // Parse attrs.
        let mut attrs: indexmap::IndexMap<Value, Value> = indexmap::IndexMap::new();
        let mut pos = 0;
        let a_str = attrs_str.trim_end_matches('/');
        while pos < a_str.len() {
            let r = &a_str[pos..];
            let name_start = r.find(|c: char| c.is_alphanumeric() || c == '-');
            let Some(ns) = name_start else { break };
            let name_end = r[ns..]
                .find(|c: char| !c.is_alphanumeric() && c != '-')
                .map(|e| ns + e)
                .unwrap_or(r.len());
            let name = &r[ns..name_end];
            let after_name = &r[name_end..];
            if let Some(eq) = after_name.find('=') {
                let val_rest = after_name[eq + 1..].trim_start();
                if let Some(q) = val_rest.chars().next() {
                    if q == '"' || q == '\'' {
                        if let Some(qe) = val_rest[1..].find(q) {
                            let val = &val_rest[1..qe + 1];
                            attrs.insert(Value::Str(Rc::from(name)), Value::Str(Rc::from(val)));
                            pos += name_end + eq + 1 + (val_rest.len() - val_rest[qe + 2..].len());
                            continue;
                        }
                    }
                }
            }
            // Boolean attribute.
            attrs.insert(Value::Str(Rc::from(name)), Value::Str(Rc::from("")));
            pos += name_end.max(1);
        }
        // Content.
        let mut content = String::new();
        if !self_closing {
            let close_tag = format!("</{}>", tag_name);
            let after_gt = &after[gt + 1..];
            if let Some(close) = after_gt.find(&close_tag) {
                content = after_gt[..close].to_string();
            }
        }
        // Match checks.
        if !tag_pattern.is_empty() && tag_name != tag_pattern {
            rest = &after[gt + 1..];
            continue;
        }
        if let Some(idv) = &id_value {
            if map_get_str(&attrs, "id").map(|v| v.python_str()) != Some(idv.clone()) {
                rest = &after[gt + 1..];
                continue;
            }
        }
        if let Some(cv) = &class_value {
            let classes: Vec<String> = map_get_str(&attrs, "class")
                .map(|v| v.python_str().split_whitespace().map(String::from).collect())
                .unwrap_or_default();
            if !classes.iter().any(|c| c == cv) {
                rest = &after[gt + 1..];
                continue;
            }
        }
        if let Some(an) = &attr_name {
            let has = attrs.contains_key(&Value::Str(Rc::from(an.as_str())));
            if !has {
                rest = &after[gt + 1..];
                continue;
            }
            if let Some(av) = &attr_value {
                if map_get_str(&attrs, an).map(|v| v.python_str()) != Some(av.clone()) {
                    rest = &after[gt + 1..];
                    continue;
                }
            }
        }
        let mut el = indexmap::IndexMap::new();
        el.insert(Value::Str(Rc::from("tag")), Value::Str(Rc::from(tag_name.as_str())));
        el.insert(Value::Str(Rc::from("attrs")), Value::Map(Rc::new(RefCell::new(attrs))));
        el.insert(Value::Str(Rc::from("text")), Value::Str(Rc::from(content.as_str())));
        results.push(Value::Map(Rc::new(RefCell::new(el))));
        rest = &after[gt + 1..];
    }
    Ok(Value::List(Rc::new(RefCell::new(results))))
}

// ---------------------------------------------------------------------------
// Markdown
// ---------------------------------------------------------------------------

/// Python `_markdown_to_html` — headings, paragraphs, bold/italic/code.
pub fn data_markdown_to_html(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let text = arg_str(args, 0)?;
    let mut html: Vec<String> = Vec::new();
    let mut in_paragraph = false;
    for raw in text.split('\n') {
        let line = raw.trim_end();
        if let Some(h) = line.strip_prefix("# ") {
            if in_paragraph {
                html.push("</p>".to_string());
                in_paragraph = false;
            }
            html.push(format!("<h1>{h}</h1>"));
        } else if let Some(h) = line.strip_prefix("## ") {
            if in_paragraph {
                html.push("</p>".to_string());
                in_paragraph = false;
            }
            html.push(format!("<h2>{h}</h2>"));
        } else if let Some(h) = line.strip_prefix("### ") {
            if in_paragraph {
                html.push("</p>".to_string());
                in_paragraph = false;
            }
            html.push(format!("<h3>{h}</h3>"));
        } else if line.is_empty() {
            if in_paragraph {
                html.push("</p>".to_string());
                in_paragraph = false;
            }
        } else {
            if !in_paragraph {
                html.push("<p>".to_string());
                in_paragraph = true;
            }
            let mut l = line.to_string();
            // Bold **x**
            l = replace_regex(&l, r"\*\*(.+?)\*\*", "<strong>$1</strong>");
            // Italic *x*
            l = replace_regex(&l, r"\*(.+?)\*", "<em>$1</em>");
            // Code `x`
            l = replace_regex(&l, r"`(.+?)`", "<code>$1</code>");
            html.push(l);
        }
    }
    if in_paragraph {
        html.push("</p>".to_string());
    }
    Ok(Value::Str(Rc::from(html.join("\n").as_str())))
}

/// Regex replacement helper (`re.sub` with a `\1` capture template).
fn replace_regex(input: &str, pattern: &str, replacement: &str) -> String {
    let re = match regex::Regex::new(pattern) {
        Ok(r) => r,
        Err(_) => return input.to_string(),
    };
    let mut out = String::with_capacity(input.len());
    let mut last = 0;
    for caps in re.captures_iter(input) {
        let m = caps.get(0).unwrap();
        out.push_str(&input[last..m.start()]);
        // Expand $1 / $2 / $0.
        let mut rep = replacement.to_string();
        for (idx, c) in caps.iter().enumerate() {
            if idx == 0 {
                continue;
            }
            if let Some(cm) = c {
                rep = rep.replace(&format!("${idx}"), &cm.as_str().to_string());
            }
        }
        out.push_str(&rep);
        last = m.end();
    }
    out.push_str(&input[last..]);
    out
}

/// Python `_markdown_extract_headings` — `{level, text, id}`.
pub fn data_markdown_extract_headings(
    _i: &mut Interpreter,
    args: &[Value],
) -> Result<Value, ExceptionValue> {
    let text = arg_str(args, 0)?;
    let mut headings = Vec::new();
    for line in text.split('\n') {
        let line = line.trim_start();
        let hashes: usize = line.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&hashes) {
            let after = &line[hashes..];
            if let Some(content) = after.strip_prefix(' ') {
                let text_content = content.trim().to_string();
                // Generate ID: lowercase, keep [a-z0-9 _-], collapse separators.
                let mut id = String::new();
                for c in text_content.chars() {
                    let lc = c.to_ascii_lowercase();
                    if lc.is_alphanumeric() || lc == ' ' || lc == '-' {
                        id.push(lc);
                    }
                }
                // Collapse runs of spaces/hyphens to a single hyphen.
                let mut collapsed = String::new();
                let mut prev_sep = false;
                for c in id.chars() {
                    if c == ' ' || c == '-' {
                        if !prev_sep {
                            collapsed.push('-');
                        }
                        prev_sep = true;
                    } else {
                        collapsed.push(c);
                        prev_sep = false;
                    }
                }
                let mut h = indexmap::IndexMap::new();
                h.insert(Value::Str(Rc::from("level")), Value::Int(BigInt::from(hashes)));
                h.insert(Value::Str(Rc::from("text")), Value::Str(Rc::from(text_content.as_str())));
                h.insert(Value::Str(Rc::from("id")), Value::Str(Rc::from(collapsed.as_str())));
                headings.push(Value::Map(Rc::new(RefCell::new(h))));
            }
        }
    }
    Ok(Value::List(Rc::new(RefCell::new(headings))))
}

/// Python `_markdown_parse` — structured blocks.
pub fn data_markdown_parse(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let text = arg_str(args, 0)?;
    let lines: Vec<&str> = text.split('\n').collect();
    let n = lines.len();
    let mut blocks: Vec<Value> = Vec::new();
    let fence_re = regex::Regex::new(r"^(`{3,}|~{3,})(\w*)").unwrap();
    let heading_re = regex::Regex::new(r"^(#{1,6})\s+").unwrap();
    let ul_re = regex::Regex::new(r"^[-*+]\s+").unwrap();
    let ol_re = regex::Regex::new(r"^\d+\.\s+").unwrap();
    let mut i = 0;
    while i < n {
        let line = lines[i];
        let stripped = line.trim();
        if stripped.is_empty() {
            i += 1;
            continue;
        }
        // Fenced code block.
        let fence_match = fence_re.captures(stripped);
        if let Some(fm) = fence_match {
            let fence_char = fm.get(1).unwrap().as_str().chars().next().unwrap();
            let fence_len = fm.get(1).unwrap().as_str().len();
            let language = fm.get(2).map(|m| m.as_str().to_string()).unwrap_or_default();
            let mut code_lines = Vec::new();
            i += 1;
            let close_re = regex::Regex::new(&format!(
                r"^{}{{{},}}\s*$",
                regex::escape(&fence_char.to_string()),
                fence_len
            ))
            .unwrap();
            while i < n {
                if close_re.is_match(lines[i].trim()) {
                    i += 1;
                    break;
                }
                code_lines.push(lines[i].to_string());
                i += 1;
            }
            let mut b = indexmap::IndexMap::new();
            b.insert(Value::Str(Rc::from("type")), Value::Str(Rc::from("code_block")));
            b.insert(Value::Str(Rc::from("language")), Value::Str(Rc::from(language.as_str())));
            b.insert(Value::Str(Rc::from("text")), Value::Str(Rc::from(code_lines.join("\n").as_str())));
            blocks.push(Value::Map(Rc::new(RefCell::new(b))));
            continue;
        }
        // Horizontal rule: ---, ***, ___ (all same char).
        if stripped.len() >= 3
            && stripped.chars().all(|c| c == '-' || c == '*' || c == '_')
            && stripped.chars().all(|c| c == stripped.chars().next().unwrap())
        {
            let mut b = indexmap::IndexMap::new();
            b.insert(Value::Str(Rc::from("type")), Value::Str(Rc::from("hr")));
            blocks.push(Value::Map(Rc::new(RefCell::new(b))));
            i += 1;
            continue;
        }
        // Heading.
        let hashes: usize = stripped.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&hashes) {
            if let Some(content) = stripped[hashes..].strip_prefix(' ') {
                let mut b = indexmap::IndexMap::new();
                b.insert(Value::Str(Rc::from("type")), Value::Str(Rc::from("heading")));
                b.insert(Value::Str(Rc::from("level")), Value::Int(BigInt::from(hashes)));
                b.insert(Value::Str(Rc::from("text")), Value::Str(Rc::from(content.trim().to_string().as_str())));
                blocks.push(Value::Map(Rc::new(RefCell::new(b))));
                i += 1;
                continue;
            }
        }
        // Blockquote.
        if stripped.starts_with('>') {
            let mut quote_lines = Vec::new();
            while i < n && lines[i].trim().starts_with('>') {
                let content = lines[i].trim().trim_start_matches('>').trim_start().to_string();
                quote_lines.push(content);
                i += 1;
            }
            let mut b = indexmap::IndexMap::new();
            b.insert(Value::Str(Rc::from("type")), Value::Str(Rc::from("blockquote")));
            b.insert(Value::Str(Rc::from("text")), Value::Str(Rc::from(quote_lines.join("\n").as_str())));
            blocks.push(Value::Map(Rc::new(RefCell::new(b))));
            continue;
        }
        // Unordered list.
        if ul_re.is_match(stripped) {
            let mut items = Vec::new();
            let item_ul_re = regex::Regex::new(r"^\s*[-*+]\s+").unwrap();
            while i < n && item_ul_re.is_match(lines[i]) {
                let item_text = item_ul_re.replace(lines[i], "").to_string();
                items.push(Value::Str(Rc::from(item_text.as_str())));
                i += 1;
            }
            let mut b = indexmap::IndexMap::new();
            b.insert(Value::Str(Rc::from("type")), Value::Str(Rc::from("list")));
            b.insert(Value::Str(Rc::from("ordered")), Value::Bool(false));
            b.insert(Value::Str(Rc::from("items")), Value::List(Rc::new(RefCell::new(items))));
            blocks.push(Value::Map(Rc::new(RefCell::new(b))));
            continue;
        }
        // Ordered list.
        if ol_re.is_match(stripped) {
            let mut items = Vec::new();
            let item_ol_re = regex::Regex::new(r"^\s*\d+\.\s+").unwrap();
            while i < n && item_ol_re.is_match(lines[i]) {
                let item_text = item_ol_re.replace(lines[i], "").to_string();
                items.push(Value::Str(Rc::from(item_text.as_str())));
                i += 1;
            }
            let mut b = indexmap::IndexMap::new();
            b.insert(Value::Str(Rc::from("type")), Value::Str(Rc::from("list")));
            b.insert(Value::Str(Rc::from("ordered")), Value::Bool(true));
            b.insert(Value::Str(Rc::from("items")), Value::List(Rc::new(RefCell::new(items))));
            blocks.push(Value::Map(Rc::new(RefCell::new(b))));
            continue;
        }
        // Paragraph — collect consecutive non-special lines.
        let mut para_lines = Vec::new();
        while i < n {
            let l = lines[i];
            let ls = l.trim();
            if ls.is_empty() {
                break;
            }
            if heading_re.is_match(ls) {
                break;
            }
            if fence_re.is_match(ls) {
                break;
            }
            if ls.len() >= 3
                && ls.chars().all(|c| c == '-' || c == '*' || c == '_')
                && ls.chars().all(|c| c == ls.chars().next().unwrap())
            {
                break;
            }
            if ls.starts_with('>') {
                break;
            }
            if ul_re.is_match(ls) {
                break;
            }
            if ol_re.is_match(ls) {
                break;
            }
            para_lines.push(ls.to_string());
            i += 1;
        }
        if !para_lines.is_empty() {
            let mut b = indexmap::IndexMap::new();
            b.insert(Value::Str(Rc::from("type")), Value::Str(Rc::from("paragraph")));
            b.insert(Value::Str(Rc::from("text")), Value::Str(Rc::from(para_lines.join(" ").as_str())));
            blocks.push(Value::Map(Rc::new(RefCell::new(b))));
        }
    }
    Ok(Value::List(Rc::new(RefCell::new(blocks))))
}

// ---------------------------------------------------------------------------
// TOML
// ---------------------------------------------------------------------------

/// Python `_toml_parse` — parse TOML string.
pub fn data_toml_parse(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let text = arg_str(args, 0)?;
    match text.parse::<toml::Value>() {
        Ok(v) => Ok(toml_to_value(&v)),
        Err(e) => Err(ExceptionValue::new(
            "RuntimeError",
            format!("Python ValueError: Invalid TOML: {e}"),
            None,
        )),
    }
}

fn toml_to_value(v: &toml::Value) -> Value {
    match v {
        toml::Value::String(s) => Value::Str(Rc::from(s.as_str())),
        toml::Value::Integer(i) => Value::Int(BigInt::from(*i)),
        toml::Value::Float(f) => Value::Float(*f),
        toml::Value::Boolean(b) => Value::Bool(*b),
        toml::Value::Datetime(d) => Value::Str(Rc::from(d.to_string().as_str())),
        toml::Value::Array(a) => {
            let items: Vec<Value> = a.iter().map(toml_to_value).collect();
            Value::List(Rc::new(RefCell::new(items)))
        }
        toml::Value::Table(t) => {
            let mut m = indexmap::IndexMap::new();
            for (k, val) in t {
                m.insert(Value::Str(Rc::from(k.as_str())), toml_to_value(val));
            }
            Value::Map(Rc::new(RefCell::new(m)))
        }
    }
}

/// Python `_toml_stringify` — dict to TOML string.
pub fn data_toml_stringify(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let value = args
        .first()
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing argument".to_string(), None))?;
    let table = value_to_toml_table(&value)?;
    match toml::to_string(&table) {
        Ok(s) => Ok(Value::Str(Rc::from(s.as_str()))),
        Err(e) => Err(ExceptionValue::new(
            "RuntimeError",
            format!("Python ValueError: {e}"),
            None,
        )),
    }
}

/// Convert a Helen Value (Map of scalars) into a toml::Value table.
fn value_to_toml_table(v: &Value) -> Result<toml::Value, ExceptionValue> {
    let m = value_to_map(v)?;
    let mut table = toml::map::Map::new();
    for (k, val) in &m {
        let key = match k {
            Value::Str(s) => s.to_string(),
            other => other.python_str(),
        };
        table.insert(key, value_to_toml(val)?);
    }
    Ok(toml::Value::Table(table))
}

fn value_to_toml(v: &Value) -> Result<toml::Value, ExceptionValue> {
    match v {
        Value::Null => Ok(toml::Value::String(String::new())),
        Value::Bool(b) => Ok(toml::Value::Boolean(*b)),
        Value::Int(n) => n
            .to_i64()
            .map(toml::Value::Integer)
            .ok_or_else(|| ExceptionValue::new("TypeError", "integer too large for TOML".to_string(), None)),
        Value::Float(f) => Ok(toml::Value::Float(*f)),
        Value::Str(s) => Ok(toml::Value::String(s.to_string())),
        Value::List(l) => {
            let mut arr = Vec::new();
            for item in l.borrow().iter() {
                arr.push(value_to_toml(item)?);
            }
            Ok(toml::Value::Array(arr))
        }
        Value::Map(_) => value_to_toml_table(v),
        other => Err(ExceptionValue::new(
            "TypeError",
            format!("Object of type {} is not TOML serializable", other.type_name()),
            None,
        )),
    }
}

/// Python `_toml_load`.
pub fn data_toml_load(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    match std::fs::read_to_string(path) {
        Ok(content) => match content.parse::<toml::Value>() {
            Ok(v) => Ok(toml_to_value(&v)),
            Err(e) => Err(ExceptionValue::new(
                "RuntimeError",
                format!("Python ValueError: Invalid TOML in file: {e}"),
                None,
            )),
        },
        Err(e) => Err(ExceptionValue::new(
            "RuntimeError",
            format!("Python FileNotFoundError: File not found: {path} ({e})"),
            None,
        )),
    }
}

/// Python `_toml_save`.
pub fn data_toml_save(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?.to_string();
    let value = args
        .get(1)
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing argument 2".to_string(), None))?;
    let table = value_to_toml_table(&value)?;
    let s = toml::to_string(&table)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python ValueError: {e}"), None))?;
    if let Some(parent) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&path, s)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("IOError: {e}"), None))?;
    Ok(Value::Str(Rc::from(format!("Saved TOML to {path}").as_str())))
}

// ---------------------------------------------------------------------------
// XML
// ---------------------------------------------------------------------------

/// Python `_xml_parse` — parse XML to dict (ElementTree semantics).
pub fn data_xml_parse(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let text = arg_str(args, 0)?;
    match quick_xml::Reader::from_str(text).read_event() {
        Ok(_) => (),
        Err(e) => {
            return Err(ExceptionValue::new(
                "RuntimeError",
                format!("Python ValueError: Invalid XML: {e}"),
                None,
            ))
        }
    }
    // Re-parse into a DOM.
    match parse_xml_dom(text) {
        Ok(root) => Ok(xml_element_to_value(&root)),
        Err(e) => Err(ExceptionValue::new(
            "RuntimeError",
            format!("Python ValueError: Invalid XML: {e}"),
            None,
        )),
    }
}

/// Minimal XML DOM node.
#[derive(Debug, Clone)]
struct XmlNode {
    tag: String,
    attrs: Vec<(String, String)>,
    children: Vec<XmlNode>,
    text: String,
}

/// Parse an XML string into a DOM tree.
fn parse_xml_dom(text: &str) -> Result<XmlNode, String> {
    let mut reader = quick_xml::Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut stack: Vec<XmlNode> = Vec::new();
    let mut root: Option<XmlNode> = None;
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(quick_xml::events::Event::Start(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut attrs = Vec::new();
                for a in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(a.key.as_ref()).to_string();
                    let value = String::from_utf8_lossy(&a.value).to_string();
                    attrs.push((key, value));
                }
                stack.push(XmlNode {
                    tag,
                    attrs,
                    children: Vec::new(),
                    text: String::new(),
                });
            }
            Ok(quick_xml::events::Event::Empty(e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let mut attrs = Vec::new();
                for a in e.attributes().flatten() {
                    let key = String::from_utf8_lossy(a.key.as_ref()).to_string();
                    let value = String::from_utf8_lossy(&a.value).to_string();
                    attrs.push((key, value));
                }
                let node = XmlNode {
                    tag,
                    attrs,
                    children: Vec::new(),
                    text: String::new(),
                };
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(node);
                } else {
                    root = Some(node);
                }
            }
            Ok(quick_xml::events::Event::Text(t)) => {
                let txt = t
                    .xml_content(quick_xml::XmlVersion::Implicit1_0)
                    .map(|c| c.to_string())
                    .unwrap_or_default();
                if let Some(node) = stack.last_mut() {
                    node.text.push_str(&txt);
                }
            }
            Ok(quick_xml::events::Event::End(_)) => {
                let node = stack.pop();
                if let Some(node) = node {
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(node);
                    } else {
                        root = Some(node);
                    }
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(e) => return Err(format!("{e}")),
            _ => {}
        }
        buf.clear();
    }
    root.ok_or_else(|| "no root element".to_string())
}

/// Python `_xml_to_dict` — `{tag: {attrs (@key), children, #text}}`.
fn xml_element_to_value(node: &XmlNode) -> Value {
    let mut result: indexmap::IndexMap<Value, Value> = indexmap::IndexMap::new();
    for (k, v) in &node.attrs {
        result.insert(
            Value::Str(Rc::from(format!("@{k}").as_str())),
            Value::Str(Rc::from(v.as_str())),
        );
    }
    if !node.children.is_empty() {
        // Group children by tag, preserving order; repeated tags -> list.
        let mut child_map: indexmap::IndexMap<String, Vec<Value>> = indexmap::IndexMap::new();
        for child in &node.children {
            let cv = xml_element_to_value(child);
            let key = child.tag.clone();
            // Extract the inner value: `{tag: inner}`.
            let inner = match &cv {
                Value::Map(m) => m
                    .borrow()
                    .get(&Value::Str(Rc::from(child.tag.as_str())))
                    .cloned()
                    .unwrap_or(cv.clone()),
                _ => cv.clone(),
            };
            child_map.entry(key).or_default().push(inner);
        }
        for (tag, vals) in child_map {
            if vals.len() == 1 {
                result.insert(Value::Str(Rc::from(tag.as_str())), vals.into_iter().next().unwrap());
            } else {
                result.insert(
                    Value::Str(Rc::from(tag.as_str())),
                    Value::List(Rc::new(RefCell::new(vals))),
                );
            }
        }
    } else if !node.text.trim().is_empty() {
        let text = node.text.trim().to_string();
        if result.is_empty() {
            return Value::Str(Rc::from(text.as_str()));
        }
        result.insert(Value::Str(Rc::from("#text")), Value::Str(Rc::from(text.as_str())));
    }
    let mut outer = indexmap::IndexMap::new();
    outer.insert(
        Value::Str(Rc::from(node.tag.as_str())),
        Value::Map(Rc::new(RefCell::new(result))),
    );
    Value::Map(Rc::new(RefCell::new(outer)))
}

/// Python `_xml_stringify` — dict to XML string.
pub fn data_xml_stringify(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let value = args
        .first()
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing argument".to_string(), None))?;
    let root = match args.get(1) {
        Some(Value::Str(s)) => s.to_string(),
        _ => "root".to_string(),
    };
    let mut out = String::new();
    dict_to_xml(&value, &mut out, &root, true);
    Ok(Value::Str(Rc::from(out.as_str())))
}

/// Python `_dict_to_xml` — recursive dict to XML.
fn dict_to_xml(data: &Value, out: &mut String, tag: &str, is_root: bool) {
    if is_root {
        out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    }
    match data {
        Value::Map(m) => {
            let m = m.borrow();
            let mut attrs: Vec<(String, String)> = Vec::new();
            let mut children: Vec<(String, Value)> = Vec::new();
            let mut text: Option<String> = None;
            for (k, v) in m.iter() {
                let key = match k {
                    Value::Str(s) => s.to_string(),
                    other => other.python_str(),
                };
                if key.starts_with('@') {
                    attrs.push((key[1..].to_string(), v.python_str()));
                } else if key == "#text" {
                    text = Some(v.python_str());
                } else if let Value::List(l) = v {
                    for item in l.borrow().iter() {
                        children.push((key.clone(), item.clone()));
                    }
                } else {
                    children.push((key, v.clone()));
                }
            }
            let mut open = format!("<{tag}");
            for (k, v) in &attrs {
                open.push_str(&format!(" {k}=\"{v}\""));
            }
            if children.is_empty() && text.is_none() {
                out.push_str(&format!("{open}/>"));
                return;
            }
            out.push_str(&format!("{open}>"));
            if let Some(t) = text {
                out.push_str(&t);
            }
            for (ctag, cval) in &children {
                dict_to_xml(cval, out, ctag, false);
            }
            out.push_str(&format!("</{tag}>"));
        }
        other => {
            out.push_str(&format!("<{tag}>{}</{tag}>", other.python_str()));
        }
    }
}

/// Python `_xml_load`.
pub fn data_xml_load(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    match std::fs::read_to_string(path) {
        Ok(content) => match parse_xml_dom(&content) {
            Ok(root) => Ok(xml_element_to_value(&root)),
            Err(e) => Err(ExceptionValue::new(
                "RuntimeError",
                format!("Python ValueError: Invalid XML in file: {e}"),
                None,
            )),
        },
        Err(e) => Err(ExceptionValue::new(
            "RuntimeError",
            format!("Python FileNotFoundError: File not found: {path} ({e})"),
            None,
        )),
    }
}

/// Python `_xml_save`.
pub fn data_xml_save(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?.to_string();
    let value = args
        .get(1)
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing argument 2".to_string(), None))?;
    let root = match args.get(2) {
        Some(Value::Str(s)) => s.to_string(),
        _ => "root".to_string(),
    };
    let mut out = String::new();
    dict_to_xml(&value, &mut out, &root, true);
    if let Some(parent) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&path, out)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("IOError: {e}"), None))?;
    Ok(Value::Str(Rc::from(format!("Saved XML to {path}").as_str())))
}

// ---------------------------------------------------------------------------
// YAML
// ---------------------------------------------------------------------------

/// Python `_yaml_parse` — parse YAML string.
pub fn data_yaml_parse(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let text = arg_str(args, 0)?;
    match serde_yaml::from_str::<serde_json::Value>(text) {
        Ok(j) => Ok(json_to_value(&j)),
        Err(e) => Err(ExceptionValue::new(
            "RuntimeError",
            format!("Python ValueError: Invalid YAML: {e}"),
            None,
        )),
    }
}

/// Python `_yaml_stringify` — object to YAML string.
pub fn data_yaml_stringify(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let value = args
        .first()
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing argument".to_string(), None))?;
    let j = value_to_json(&value)
        .map_err(|m| ExceptionValue::new("RuntimeError", format!("Python TypeError: {m}"), None))?;
    match serde_yaml::to_string(&j) {
        Ok(s) => Ok(Value::Str(Rc::from(s.as_str()))),
        Err(e) => Err(ExceptionValue::new(
            "RuntimeError",
            format!("Python ValueError: {e}"),
            None,
        )),
    }
}

/// Python `_yaml_load`.
pub fn data_yaml_load(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?;
    match std::fs::read_to_string(path) {
        Ok(content) => match serde_yaml::from_str::<serde_json::Value>(&content) {
            Ok(j) => Ok(json_to_value(&j)),
            Err(e) => Err(ExceptionValue::new(
                "RuntimeError",
                format!("Python ValueError: Invalid YAML in file: {e}"),
                None,
            )),
        },
        Err(e) => Err(ExceptionValue::new(
            "RuntimeError",
            format!("Python FileNotFoundError: File not found: {path} ({e})"),
            None,
        )),
    }
}

/// Python `_yaml_save`.
pub fn data_yaml_save(_i: &mut Interpreter, args: &[Value]) -> Result<Value, ExceptionValue> {
    let path = arg_str(args, 0)?.to_string();
    let value = args
        .get(1)
        .cloned()
        .ok_or_else(|| ExceptionValue::new("RuntimeError", "missing argument 2".to_string(), None))?;
    let j = value_to_json(&value)
        .map_err(|m| ExceptionValue::new("RuntimeError", format!("Python TypeError: {m}"), None))?;
    let s = serde_yaml::to_string(&j)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("Python ValueError: {e}"), None))?;
    if let Some(parent) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&path, s)
        .map_err(|e| ExceptionValue::new("RuntimeError", format!("IOError: {e}"), None))?;
    Ok(Value::Str(Rc::from(format!("Saved YAML to {path}").as_str())))
}
