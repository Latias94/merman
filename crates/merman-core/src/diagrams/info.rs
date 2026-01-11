use crate::{ParseMetadata, Result};
use serde_json::{Value, json};

pub fn parse_info(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let mut lines = code.lines();
    let mut first = None;
    for line in &mut lines {
        let t = strip_inline_comment(line).trim();
        if !t.is_empty() {
            first = Some(t.to_string());
            break;
        }
    }

    let Some(first) = first else {
        return Ok(json!({}));
    };
    if !first.starts_with("info") {
        return Ok(json!({
            "error": "expected info"
        }));
    }

    let mut show_info = first.split_whitespace().any(|w| w == "showInfo");
    let mut acc_title = None;
    let mut acc_descr = None;
    let mut title = None;

    for line in lines {
        let t = strip_inline_comment(line).trim();
        if t.is_empty() {
            continue;
        }
        if t == "showInfo" {
            show_info = true;
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
    }

    Ok(json!({
        "type": meta.diagram_type,
        "showInfo": show_info,
        "title": title,
        "accTitle": acc_title,
        "accDescr": acc_descr,
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
