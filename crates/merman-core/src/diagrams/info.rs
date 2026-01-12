use crate::{Error, ParseMetadata, Result};
use serde_json::{Value, json};

pub fn parse_info(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let mut header: Option<String> = None;
    let mut rest_lines = Vec::new();

    for line in code.lines() {
        let t = strip_inline_comment(line).trim();
        if t.is_empty() {
            continue;
        }
        if header.is_none() {
            header = Some(t.to_string());
        } else {
            rest_lines.push(t.to_string());
        }
    }

    let Some(header) = header else {
        return Ok(json!({}));
    };

    let mut tokens = header.split_whitespace();
    let Some(first) = tokens.next() else {
        return Ok(json!({}));
    };

    if first != "info" {
        return Ok(json!({ "error": "expected info" }));
    }

    let mut show_info = false;
    let mut unsupported: Option<&str> = None;
    for tok in tokens {
        if tok == "showInfo" {
            show_info = true;
            continue;
        }
        unsupported = Some(tok);
        break;
    }

    if unsupported.is_none() && rest_lines.is_empty() {
        return Ok(json!({
            "type": meta.diagram_type,
            "showInfo": show_info,
        }));
    }

    let bad = unsupported
        .or_else(|| rest_lines.first().map(|s| s.as_str()))
        .unwrap();
    let ch = bad.chars().next().unwrap_or('?');
    let skipped = bad.chars().count();
    let offset = code.find(bad).unwrap_or(5);

    Err(Error::DiagramParse {
        diagram_type: meta.diagram_type.clone(),
        message: format!(
            "Parsing failed: unexpected character: ->{ch}<- at offset: {offset}, skipped {skipped} characters."
        ),
    })
}

fn strip_inline_comment(line: &str) -> &str {
    match line.find("%%") {
        Some(idx) => &line[..idx],
        None => line,
    }
}
