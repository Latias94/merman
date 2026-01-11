use crate::{ParseMetadata, Result};
use serde_json::{Value, json};

pub fn parse_pie(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let mut lines = code.lines();
    let mut header = None;
    for line in &mut lines {
        let t = strip_inline_comment(line).trim();
        if !t.is_empty() {
            header = Some(t.to_string());
            break;
        }
    }

    let Some(header) = header else {
        return Ok(json!({}));
    };
    let mut it = header.split_whitespace();
    let Some(first) = it.next() else {
        return Ok(json!({}));
    };
    if first != "pie" {
        return Ok(json!({ "error": "expected pie" }));
    }
    let show_data = it.any(|w| w == "showData");

    let mut acc_title = None;
    let mut acc_descr = None;
    let mut title = None;
    let mut sections: Vec<Value> = Vec::new();

    for line in lines {
        let t = strip_inline_comment(line).trim();
        if t.is_empty() {
            continue;
        }
        if let Some(v) = parse_key_value(t, "accTitle") {
            acc_title = Some(v);
            continue;
        }
        if let Some(v) = parse_acc_descr(t) {
            acc_descr = Some(v);
            continue;
        }
        if let Some(v) = parse_title(t) {
            title = Some(v);
            continue;
        }
        if let Some((label, value)) = parse_section(t) {
            sections.push(json!({ "label": label, "value": value }));
            continue;
        }
    }

    Ok(json!({
        "type": meta.diagram_type,
        "showData": show_data,
        "title": title,
        "accTitle": acc_title,
        "accDescr": acc_descr,
        "sections": sections,
    }))
}

fn strip_inline_comment(line: &str) -> &str {
    match line.find("%%") {
        Some(idx) => &line[..idx],
        None => line,
    }
}

fn parse_title(line: &str) -> Option<String> {
    let t = line.trim_start();
    if !t.starts_with("title") {
        return None;
    }
    let rest = t.strip_prefix("title")?.trim_start();
    Some(rest.to_string())
}

fn parse_key_value(line: &str, key: &str) -> Option<String> {
    let t = line.trim_start();
    if !t.starts_with(key) {
        return None;
    }
    let rest = t.strip_prefix(key)?.trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    Some(rest.to_string())
}

fn parse_acc_descr(line: &str) -> Option<String> {
    let t = line.trim_start();
    if !t.starts_with("accDescr") {
        return None;
    }
    let rest = t.strip_prefix("accDescr")?.trim_start();
    if let Some(rest) = rest.strip_prefix(':') {
        return Some(rest.trim_start().to_string());
    }
    if let Some(rest) = rest.strip_prefix('{') {
        let end = rest.find('}')?;
        return Some(rest[..end].to_string());
    }
    None
}

fn parse_section(line: &str) -> Option<(String, f64)> {
    let t = line.trim_start();
    let (label, rest) = parse_quoted_string(t)?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();

    let mut num = String::new();
    for c in rest.chars() {
        if c.is_ascii_digit() || c == '-' || c == '.' {
            num.push(c);
        } else {
            break;
        }
    }
    if num.is_empty() {
        return None;
    }
    let value: f64 = num.parse().ok()?;
    Some((label, value))
}

fn parse_quoted_string(input: &str) -> Option<(String, &str)> {
    let mut chars = input.chars();
    let quote = chars.next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let mut out = String::new();
    let mut escaped = false;
    let mut idx = 1;
    for c in chars {
        idx += c.len_utf8();
        if escaped {
            out.push(c);
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
            continue;
        }
        if c == quote {
            return Some((out, &input[idx..]));
        }
        out.push(c);
    }
    None
}
