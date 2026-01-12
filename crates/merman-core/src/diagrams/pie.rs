use crate::{Error, ParseMetadata, Result};
use serde_json::{Value, json};
use std::collections::HashSet;

pub fn parse_pie(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let mut raw_lines = code.lines();

    let mut header: Option<String> = None;
    for line in &mut raw_lines {
        let t = strip_inline_comment(line).trim();
        if !t.is_empty() {
            header = Some(t.to_string());
            break;
        }
    }

    let Some(header) = header else {
        return Ok(json!({}));
    };

    let mut it0 = header.split_whitespace();
    let Some(first) = it0.next() else {
        return Ok(json!({}));
    };
    if first != "pie" {
        return Ok(json!({ "error": "expected pie" }));
    }

    let mut show_data = false;
    let mut title: Option<String> = None;
    let mut unsupported: Option<String> = None;

    fn token_boundary_ok(s: &str, token_len: usize) -> bool {
        let Some(rest) = s.get(token_len..) else {
            return true;
        };
        match rest.chars().next() {
            None => true,
            Some(c) => c.is_whitespace(),
        }
    }

    let header_after = header
        .trim_start_matches(|c: char| c.is_whitespace())
        .strip_prefix("pie")
        .unwrap_or("");
    let mut rest = header_after.trim_start();
    while !rest.is_empty() {
        if rest.starts_with("showData") && token_boundary_ok(rest, "showData".len()) {
            show_data = true;
            rest = rest["showData".len()..].trim_start();
            continue;
        }
        if rest.starts_with("title") && token_boundary_ok(rest, "title".len()) {
            let after = rest["title".len()..].trim_start();
            title = Some(after.to_string());
            rest = "";
            continue;
        }
        unsupported = Some(rest.split_whitespace().next().unwrap_or(rest).to_string());
        break;
    }

    if let Some(tok) = unsupported {
        return Err(Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: format!("unexpected pie header token: {tok}"),
        });
    }

    let mut acc_title = None;
    let mut acc_descr = None;
    let mut sections: Vec<Value> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    let mut lines = raw_lines.peekable();
    while let Some(line) = lines.next() {
        let t = strip_inline_comment(line).trim();
        if t.is_empty() {
            continue;
        }

        if let Some(v) = parse_key_value(t, "accTitle") {
            acc_title = Some(v);
            continue;
        }

        if let Some(v) = parse_acc_descr_inline(t) {
            acc_descr = Some(v);
            continue;
        }

        if starts_acc_descr_block(t) {
            let mut parts: Vec<String> = Vec::new();
            while let Some(next_line) = lines.next() {
                let s = strip_inline_comment(next_line);
                if s.contains('}') {
                    let before = s.split('}').next().unwrap_or("").trim();
                    if !before.is_empty() {
                        parts.push(before.to_string());
                    }
                    break;
                }
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    continue;
                }
                parts.push(trimmed.to_string());
            }
            acc_descr = Some(parts.join("\n"));
            continue;
        }

        if let Some((label, value)) = parse_section(t) {
            if value < 0.0 {
                return Err(Error::DiagramParse {
                    diagram_type: meta.diagram_type.clone(),
                    message: format!(
                        "\"{label}\" has invalid value: {value}. Negative values are not allowed in pie charts. All slice values must be >= 0."
                    ),
                });
            }
            if seen.insert(label.clone()) {
                sections.push(json!({ "label": label, "value": value }));
            }
            continue;
        }

        return Err(Error::DiagramParse {
            diagram_type: meta.diagram_type.clone(),
            message: format!("unexpected pie statement: {t}"),
        });
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

fn parse_key_value(line: &str, key: &str) -> Option<String> {
    let t = line.trim_start();
    if !t.starts_with(key) {
        return None;
    }
    let rest = t.strip_prefix(key)?.trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    Some(rest.to_string())
}

fn parse_acc_descr_inline(line: &str) -> Option<String> {
    let t = line.trim_start();
    if !t.starts_with("accDescr") {
        return None;
    }
    let rest = t.strip_prefix("accDescr")?.trim_start();
    if let Some(rest) = rest.strip_prefix(':') {
        return Some(rest.trim_start().to_string());
    }
    None
}

fn starts_acc_descr_block(line: &str) -> bool {
    let t = line.trim_start();
    if !t.starts_with("accDescr") {
        return false;
    }
    let rest = t.trim_start_matches("accDescr").trim_start();
    rest.starts_with('{')
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
