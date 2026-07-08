use crate::{MermaidConfig, SourceSpan};

use super::{
    NODE_TYPE_BANG, NODE_TYPE_CIRCLE, NODE_TYPE_CLOUD, NODE_TYPE_DEFAULT, NODE_TYPE_HEXAGON,
    NODE_TYPE_RECT, NODE_TYPE_ROUNDED_RECT,
};

pub(super) fn strip_inline_comment(line: &str) -> &str {
    let mut in_quote = false;
    let mut in_backtick_quote = false;

    let mut it = line.char_indices().peekable();
    while let Some((idx, ch)) = it.next() {
        if in_backtick_quote {
            if ch == '`' && it.peek().is_some_and(|(_, next)| *next == '"') {
                in_backtick_quote = false;
                it.next();
            }
            continue;
        }

        if in_quote {
            if ch == '"' {
                in_quote = false;
            }
            continue;
        }

        if ch == '"' {
            if it.peek().is_some_and(|(_, next)| *next == '`') {
                in_backtick_quote = true;
                it.next();
                continue;
            }
            in_quote = true;
            continue;
        }

        if ch == '%' && it.peek().is_some_and(|(_, next)| *next == '%') {
            return &line[..idx];
        }
    }

    line
}

pub(super) struct NodeSpec {
    pub id_raw: String,
    pub descr_raw: String,
    pub ty: i32,
    pub descr_is_markdown: bool,
    pub id_span: SourceSpan,
    pub payload_span: Option<SourceSpan>,
}

pub(super) fn parse_node_spec(input: &str) -> std::result::Result<NodeSpec, String> {
    let input = input.trim_end();
    if input.is_empty() {
        return Err("expected node".to_string());
    }

    if let Some((start, end)) = node_delimiter_pair_at_start(input) {
        let (inner, tail) = extract_delimited(input, start, end)?;
        if !tail.trim().is_empty() {
            return Err("unexpected trailing input".to_string());
        }
        let (descr, descr_is_markdown) = unquote_node_descr(inner);
        let ty = node_type_for(start, end);
        return Ok(NodeSpec {
            id_raw: descr.clone(),
            descr_raw: descr,
            ty,
            descr_is_markdown,
            id_span: SourceSpan::new(start.len(), input.len().saturating_sub(end.len())),
            payload_span: None,
        });
    }

    let (id_raw, rest) = split_node_id(input);
    let id_raw = id_raw.to_string();
    let rest = rest.trim_end();
    if rest.is_empty() {
        return Ok(NodeSpec {
            id_raw: id_raw.clone(),
            descr_raw: id_raw,
            ty: NODE_TYPE_DEFAULT,
            descr_is_markdown: false,
            id_span: SourceSpan::new(0, input.len()),
            payload_span: None,
        });
    }

    let Some((start, end)) = node_delimiter_pair_at_start(rest) else {
        return Err("expected node delimiter".to_string());
    };

    let (inner, tail) = extract_delimited(rest, start, end)?;
    if !tail.trim().is_empty() {
        return Err("unexpected trailing input".to_string());
    }

    let (descr, descr_is_markdown) = unquote_node_descr(inner);
    let ty = node_type_for(start, end);
    let id_end = id_raw.len();
    let payload_start = id_end + start.len();
    let payload_end = input.len().saturating_sub(end.len());
    Ok(NodeSpec {
        id_raw,
        descr_raw: descr,
        ty,
        descr_is_markdown,
        id_span: SourceSpan::new(0, id_end),
        payload_span: Some(SourceSpan::new(payload_start, payload_end)),
    })
}

fn split_node_id(input: &str) -> (&str, &str) {
    let bytes = input.as_bytes();
    for (idx, b) in bytes.iter().enumerate() {
        match b {
            b'(' | b')' | b'[' | b'{' | b'}' => return (&input[..idx], &input[idx..]),
            _ => {}
        }
    }
    (input, "")
}

fn node_delimiter_pair_at_start(input: &str) -> Option<(&'static str, &'static str)> {
    let pairs: &[(&str, &str)] = &[
        ("(-", "-)"),
        ("-)", "(-"),
        ("((", "))"),
        ("))", "(("),
        ("{{", "}}"),
        ("[", "]"),
        (")", "("),
        ("(", ")"),
    ];

    for (start, end) in pairs {
        if input.starts_with(start) {
            return Some((*start, *end));
        }
    }
    None
}

fn extract_delimited<'a>(
    input: &'a str,
    start: &str,
    end: &str,
) -> std::result::Result<(&'a str, &'a str), String> {
    if !input.starts_with(start) {
        return Err("expected delimiter start".to_string());
    }
    let mut in_quote = false;
    let mut in_backtick_quote = false;

    let start_len = start.len();
    let mut it = input[start_len..].char_indices().peekable();
    while let Some((off, ch)) = it.next() {
        let idx = start_len + off;

        if in_backtick_quote {
            if ch == '`' && it.peek().is_some_and(|(_, next)| *next == '"') {
                in_backtick_quote = false;
                it.next();
            }
            continue;
        }

        if in_quote {
            if ch == '"' {
                in_quote = false;
            }
            continue;
        }

        if ch == '"' {
            if it.peek().is_some_and(|(_, next)| *next == '`') {
                in_backtick_quote = true;
                it.next();
                continue;
            }
            in_quote = true;
            continue;
        }

        if input[idx..].starts_with(end) {
            let inner = &input[start_len..idx];
            let tail = &input[idx + end.len()..];
            return Ok((inner, tail));
        }
    }

    Err("unterminated node delimiter".to_string())
}

fn unquote_node_descr(raw: &str) -> (String, bool) {
    // Mermaid mindmap uses a special `"` + backtick quote form for Markdown strings, e.g.:
    //   id1["`**Root** with\nsecond line`"]
    if let Some(inner) = raw.strip_prefix("\"`").and_then(|s| s.strip_suffix("`\"")) {
        return (inner.to_string(), true);
    }
    if let Some(inner) = raw.strip_prefix('\"').and_then(|s| s.strip_suffix('\"')) {
        return (inner.to_string(), false);
    }
    (raw.to_string(), false)
}

fn node_type_for(start: &str, end: &str) -> i32 {
    match start {
        "[" => NODE_TYPE_RECT,
        "(" => {
            if end == ")" {
                NODE_TYPE_ROUNDED_RECT
            } else {
                NODE_TYPE_CLOUD
            }
        }
        "((" => NODE_TYPE_CIRCLE,
        ")" => NODE_TYPE_CLOUD,
        "))" => NODE_TYPE_BANG,
        "{{" => NODE_TYPE_HEXAGON,
        _ => NODE_TYPE_DEFAULT,
    }
}

pub(super) fn get_i64(cfg: &MermaidConfig, dotted_path: &str) -> Option<i64> {
    let mut cur = cfg.as_value();
    for segment in dotted_path.split('.') {
        cur = cur.as_object()?.get(segment)?;
    }
    cur.as_i64().or_else(|| cur.as_f64().map(|f| f as i64))
}
