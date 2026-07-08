use crate::diagrams::scan::{split_indent, starts_with_case_insensitive, strip_line_ending};
use crate::sanitize::sanitize_text;
use crate::{
    EditorSemanticFacts, EditorSemanticKind, EditorSemanticSymbol, Error, MermaidConfig,
    ParseMetadata, Result, SourceSpan,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

const NODE_TYPE_DEFAULT: i32 = 0;
const NODE_TYPE_ROUNDED_RECT: i32 = 1;
const NODE_TYPE_RECT: i32 = 2;
const NODE_TYPE_CIRCLE: i32 = 3;
const NODE_TYPE_CLOUD: i32 = 4;
const NODE_TYPE_BANG: i32 = 5;
const NODE_TYPE_HEXAGON: i32 = 6;

#[derive(Debug, Clone)]
struct KanbanNode {
    id: String,
    span: SourceSpan,
    level: usize,
    label: String,
    width: i64,
    padding: i64,
    parent_id: Option<String>,

    ticket: Option<String>,
    priority: Option<String>,
    assigned: Option<String>,
    icon: Option<String>,
    css_classes: Option<String>,
    shape: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KanbanDiagramRenderModel {
    #[serde(default)]
    pub nodes: Vec<KanbanRenderNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KanbanRenderNode {
    pub id: String,
    pub label: String,
    #[serde(default, rename = "isGroup")]
    pub is_group: bool,
    #[serde(default, rename = "parentId")]
    pub parent_id: Option<String>,
    #[serde(default)]
    pub ticket: Option<String>,
    #[serde(default)]
    pub priority: Option<String>,
    #[serde(default)]
    pub assigned: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Default)]
struct KanbanDb {
    nodes: Vec<KanbanNode>,
    section_indices: Vec<usize>,
    next_auto_id: i64,
}

#[derive(Debug, Clone)]
struct SpannedText {
    text: String,
    span: SourceSpan,
    kind: EditorSemanticKind,
}

#[derive(Debug, Clone)]
struct KanbanNodeSpec {
    id_raw: String,
    descr_raw: String,
    ty: i32,
}

#[derive(Debug, Clone)]
struct KanbanShapeData {
    text: String,
    span: SourceSpan,
}

#[derive(Debug, Clone, Copy)]
struct KanbanSourceLine<'a> {
    text: &'a str,
    start: usize,
}

#[derive(Debug)]
struct KanbanLineCursor<'a> {
    source: &'a str,
    offset: usize,
}

impl<'a> KanbanLineCursor<'a> {
    fn new(source: &'a str) -> Self {
        Self { source, offset: 0 }
    }

    fn next(&mut self) -> Option<KanbanSourceLine<'a>> {
        if self.offset >= self.source.len() {
            return None;
        }

        let start = self.offset;
        let rest = &self.source[start..];
        let end = if let Some(newline) = rest.find('\n') {
            start + newline + 1
        } else {
            self.source.len()
        };
        self.offset = end;

        Some(KanbanSourceLine {
            text: strip_line_ending(&self.source[start..end]),
            start,
        })
    }

    fn offset(&self) -> usize {
        self.offset
    }
}

impl KanbanDb {
    fn clear(&mut self) {
        *self = Self::default();
    }

    fn get_section_index(&self, level: usize) -> Result<Option<usize>> {
        if self.nodes.is_empty() {
            return Ok(None);
        }

        let section_level = self.nodes[0].level;
        let mut last_section_idx: Option<usize> = None;
        for (idx, node) in self.nodes.iter().enumerate().rev() {
            if node.level == section_level && last_section_idx.is_none() {
                last_section_idx = Some(idx);
            }
            if node.level < section_level {
                return Err(Error::diagram_parse_exact(
                    "kanban",
                    format!(
                        "Items without section detected, found section (\"{}\")",
                        node.label
                    ),
                    node.span,
                ));
            }
        }

        let Some(last_section_idx) = last_section_idx else {
            return Ok(None);
        };

        if level == self.nodes[last_section_idx].level {
            return Ok(None);
        }
        Ok(Some(last_section_idx))
    }

    fn decorate_last(
        &mut self,
        class: Option<String>,
        icon: Option<String>,
        config: &MermaidConfig,
    ) {
        let Some(last) = self.nodes.last_mut() else {
            return;
        };
        if let Some(icon) = icon {
            last.icon = Some(sanitize_text(&icon, config));
        }
        if let Some(class) = class {
            last.css_classes = Some(sanitize_text(&class, config));
        }
    }

    fn add_node(
        &mut self,
        level: usize,
        spec: KanbanNodeSpec,
        span: SourceSpan,
        shape_data: Option<KanbanShapeData>,
        config: &MermaidConfig,
    ) -> Result<()> {
        let mut padding = get_i64(config, "mindmap.padding").unwrap_or(10);
        let width = get_i64(config, "mindmap.maxNodeWidth").unwrap_or(200);
        match spec.ty {
            NODE_TYPE_ROUNDED_RECT | NODE_TYPE_RECT | NODE_TYPE_HEXAGON => {
                padding *= 2;
            }
            _ => {}
        }

        let mut id = sanitize_text(&spec.id_raw, config);
        if id.is_empty() {
            id = format!("kbn{}", self.next_auto_id);
            self.next_auto_id += 1;
        }

        let mut node = KanbanNode {
            id,
            span,
            level,
            label: sanitize_text(&spec.descr_raw, config),
            width,
            padding,
            parent_id: None,
            ticket: None,
            priority: None,
            assigned: None,
            icon: None,
            css_classes: None,
            shape: None,
        };

        if let Some(shape_data) = shape_data {
            apply_shape_data(&mut node, &shape_data)?;
        }

        if let Some(section_idx) = self.get_section_index(level)? {
            node.parent_id = Some(self.nodes[section_idx].id.clone());
        } else {
            self.section_indices.push(self.nodes.len());
        }
        self.nodes.push(node);
        Ok(())
    }

    fn sections_json(&self) -> Value {
        let mut out = Vec::new();
        for &idx in &self.section_indices {
            let Some(n) = self.nodes.get(idx) else {
                continue;
            };
            let mut obj = Map::new();
            obj.insert("id".to_string(), json!(n.id));
            obj.insert("label".to_string(), json!(n.label));
            obj.insert("level".to_string(), json!(n.level));
            obj.insert("width".to_string(), json!(n.width));
            obj.insert("padding".to_string(), json!(n.padding));
            obj.insert("isGroup".to_string(), json!(false));
            if let Some(v) = &n.ticket {
                obj.insert("ticket".to_string(), json!(v));
            }
            if let Some(v) = &n.priority {
                obj.insert("priority".to_string(), json!(v));
            }
            if let Some(v) = &n.assigned {
                obj.insert("assigned".to_string(), json!(v));
            }
            if let Some(v) = &n.icon {
                obj.insert("icon".to_string(), json!(v));
            }
            if let Some(v) = &n.css_classes {
                obj.insert("cssClasses".to_string(), json!(v));
            }
            if let Some(v) = &n.shape {
                obj.insert("shape".to_string(), json!(v));
            }
            out.push(Value::Object(obj));
        }
        Value::Array(out)
    }

    fn data_nodes_json(&self, config: &MermaidConfig) -> Vec<Value> {
        let look = config.get_str("look").unwrap_or("classic").to_string();

        let mut out = Vec::new();
        for &section_idx in &self.section_indices {
            let Some(section) = self.nodes.get(section_idx) else {
                continue;
            };
            out.push(json!({
                "id": section.id,
                "label": sanitize_text(&section.label, config),
                "isGroup": true,
                "ticket": section.ticket,
                "shape": "kanbanSection",
                "level": section.level,
                "look": look,
            }));

            for item in self
                .nodes
                .iter()
                .filter(|n| n.parent_id.as_deref() == Some(&section.id))
            {
                out.push(json!({
                    "id": item.id,
                    "parentId": section.id,
                    "label": sanitize_text(&item.label, config),
                    "isGroup": false,
                    "ticket": item.ticket,
                    "priority": item.priority,
                    "assigned": item.assigned,
                    "icon": item.icon,
                    "shape": "kanbanItem",
                    "level": item.level,
                    "rx": 5,
                    "ry": 5,
                    "cssStyles": ["text-align: left"],
                }));
            }
        }
        out
    }

    fn data_nodes_for_render(&self, config: &MermaidConfig) -> Vec<KanbanRenderNode> {
        let mut out = Vec::new();
        for &section_idx in &self.section_indices {
            let Some(section) = self.nodes.get(section_idx) else {
                continue;
            };
            out.push(KanbanRenderNode {
                id: section.id.clone(),
                label: sanitize_text(&section.label, config),
                is_group: true,
                parent_id: None,
                ticket: section.ticket.clone(),
                priority: None,
                assigned: None,
                icon: None,
            });

            for item in self
                .nodes
                .iter()
                .filter(|n| n.parent_id.as_deref() == Some(&section.id))
            {
                out.push(KanbanRenderNode {
                    id: item.id.clone(),
                    label: sanitize_text(&item.label, config),
                    is_group: false,
                    parent_id: Some(section.id.clone()),
                    ticket: item.ticket.clone(),
                    priority: item.priority.clone(),
                    assigned: item.assigned.clone(),
                    icon: item.icon.clone(),
                });
            }
        }
        out
    }
}

fn apply_shape_data(node: &mut KanbanNode, shape_data: &KanbanShapeData) -> Result<()> {
    let doc = crate::inline_config::parse_mermaid_inline_object(&shape_data.text)
        .map_err(|e| Error::diagram_parse_exact("kanban", e, shape_data.span))?;
    let Some(obj) = doc.as_object() else {
        return Ok(());
    };

    if let Some(Value::String(shape)) = obj.get("shape") {
        if shape != &shape.to_lowercase() || shape.contains('_') {
            return Err(Error::diagram_parse_exact(
                "kanban",
                format!("No such shape: {shape}. Shape names should be lowercase."),
                shape_data.span,
            ));
        }
        if shape == "kanbanItem" {
            node.shape = Some(shape.clone());
        }
    }

    if let Some(Value::String(label)) = obj.get("label") {
        node.label = label.clone();
    }
    if let Some(icon) = obj.get("icon") {
        node.icon = Some(value_to_string(icon));
    }
    if let Some(assigned) = obj.get("assigned") {
        node.assigned = Some(value_to_string(assigned));
    }
    if let Some(ticket) = obj.get("ticket") {
        node.ticket = Some(value_to_string(ticket));
    }
    if let Some(priority) = obj.get("priority") {
        node.priority = Some(value_to_string(priority));
    }

    Ok(())
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        other => other.to_string(),
    }
}

fn strip_inline_comment(line: &str) -> &str {
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

fn kanban_suffix_start(line: &str, line_start: usize, suffix: &str) -> usize {
    debug_assert!(line.len() >= suffix.len());
    line_start + line.len().saturating_sub(suffix.len())
}

fn kanban_insertion_at_suffix(
    message: impl Into<String>,
    line: &str,
    line_start: usize,
    suffix: &str,
) -> Error {
    let suffix = suffix.trim_start();
    Error::diagram_parse_insertion_point(
        "kanban",
        message,
        kanban_suffix_start(line, line_start, suffix),
    )
}

fn kanban_exact_suffix(
    message: impl Into<String>,
    line: &str,
    line_start: usize,
    suffix: &str,
) -> Error {
    let suffix = suffix.trim_start();
    let start = kanban_suffix_start(line, line_start, suffix);
    let end = start + suffix.trim_end().len();
    Error::diagram_parse_exact("kanban", message, SourceSpan::new(start, end))
}

fn kanban_delimited_suffix_error(
    message: String,
    line: &str,
    line_start: usize,
    suffix: &str,
) -> Error {
    let suffix = suffix.trim_start();
    let start = kanban_suffix_start(line, line_start, suffix);
    if message == "unterminated node delimiter" {
        return Error::diagram_parse_insertion_point("kanban", message, start + suffix.len());
    }

    Error::diagram_parse_insertion_point("kanban", message, start)
}

fn parse_node_spec_for_render(
    input: &str,
    line: &str,
    line_start: usize,
) -> Result<(KanbanNodeSpec, SourceSpan)> {
    let input = input.trim_end();
    if input.is_empty() {
        return Err(kanban_insertion_at_suffix(
            "expected node",
            line,
            line_start,
            input,
        ));
    }

    if let Some((start, end)) = node_delimiter_pair_at_start(input) {
        let (inner, tail) = extract_delimited(input, start, end)
            .map_err(|message| kanban_delimited_suffix_error(message, line, line_start, input))?;
        if !tail.trim().is_empty() {
            return Err(kanban_exact_suffix(
                "unexpected trailing input",
                line,
                line_start,
                tail,
            ));
        }
        let descr = unquote_node_descr(inner);
        let ty = node_type_for(start, end);
        let rel = kanban_suffix_start(line, line_start, input) + start.len();
        return Ok((
            KanbanNodeSpec {
                id_raw: descr.clone(),
                descr_raw: descr,
                ty,
            },
            SourceSpan::new(rel, rel + inner.len()),
        ));
    }

    let (id_raw, rest) = split_node_id(input);
    let id_start = kanban_suffix_start(line, line_start, input);
    let rest = rest.trim_end();
    if rest.is_empty() {
        return Ok((
            KanbanNodeSpec {
                id_raw: id_raw.to_string(),
                descr_raw: id_raw.to_string(),
                ty: NODE_TYPE_DEFAULT,
            },
            SourceSpan::new(id_start, id_start + id_raw.len()),
        ));
    }

    let Some((start, end)) = node_delimiter_pair_at_start(rest) else {
        return Err(kanban_insertion_at_suffix(
            "expected node delimiter",
            line,
            line_start,
            rest,
        ));
    };

    let (inner, tail) = extract_delimited(rest, start, end)
        .map_err(|message| kanban_delimited_suffix_error(message, line, line_start, rest))?;
    if !tail.trim().is_empty() {
        return Err(kanban_exact_suffix(
            "unexpected trailing input",
            line,
            line_start,
            tail,
        ));
    }

    let descr = unquote_node_descr(inner);
    let ty = node_type_for(start, end);
    let rest_start = kanban_suffix_start(line, line_start, rest);
    let inner_start = rest_start + start.len();
    Ok((
        KanbanNodeSpec {
            id_raw: id_raw.to_string(),
            descr_raw: descr,
            ty,
        },
        SourceSpan::new(inner_start, inner_start + inner.len()),
    ))
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

fn unquote_node_descr(raw: &str) -> String {
    if let Some(inner) = raw.strip_prefix("\"`").and_then(|s| s.strip_suffix("`\"")) {
        return inner.to_string();
    }
    if let Some(inner) = raw.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        return inner.to_string();
    }
    raw.to_string()
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

fn get_i64(cfg: &MermaidConfig, dotted_path: &str) -> Option<i64> {
    let mut cur = cfg.as_value();
    for segment in dotted_path.split('.') {
        cur = cur.as_object()?.get(segment)?;
    }
    cur.as_i64().or_else(|| cur.as_f64().map(|f| f as i64))
}

fn consume_shape_data(
    lines: &mut KanbanLineCursor<'_>,
    first: &str,
    first_start: usize,
) -> Result<KanbanShapeData> {
    let Some(mut rest) = first.strip_prefix("@{") else {
        return Ok(KanbanShapeData {
            text: String::new(),
            span: SourceSpan::new(first_start, first_start),
        });
    };

    let mut out = String::new();
    let mut in_quote = false;
    let mut quoted = String::new();
    let block_start = first_start;
    let mut current_scan_start = first_start + "@{".len();

    loop {
        let it = rest.char_indices().peekable();
        for (idx, ch) in it {
            if in_quote {
                if ch == '"' {
                    out.push_str(&replace_newline_whitespace_with_br(&quoted));
                    quoted.clear();
                    out.push('"');
                    in_quote = false;
                    continue;
                }
                quoted.push(ch);
                continue;
            }

            if ch == '"' {
                out.push('"');
                in_quote = true;
                continue;
            }

            if ch == '}' {
                return Ok(KanbanShapeData {
                    text: out,
                    span: SourceSpan::new(block_start, current_scan_start + idx + ch.len_utf8()),
                });
            }

            out.push(ch);
        }

        let Some(next_line) = lines.next() else {
            return Err(Error::diagram_parse_insertion_point(
                "kanban",
                "unterminated @{ ... } metadata block",
                lines.offset(),
            ));
        };
        if in_quote {
            quoted.push('\n');
        } else {
            out.push('\n');
        }
        rest = next_line.text;
        current_scan_start = next_line.start;
    }
}

fn replace_newline_whitespace_with_br(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars().peekable();
    while let Some(ch) = it.next() {
        if ch == '\n' {
            while it.peek().is_some_and(|c| c.is_whitespace()) {
                it.next();
            }
            out.push_str("<br/>");
            continue;
        }
        out.push(ch);
    }
    out
}

fn split_node_and_shape_data(
    lines: &mut KanbanLineCursor<'_>,
    rest: &str,
    rest_start: usize,
) -> Result<(String, Option<KanbanShapeData>)> {
    let mut in_quote = false;
    let mut in_backtick_quote = false;
    let mut it = rest.char_indices().peekable();
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

        if rest[idx..].starts_with("@{") {
            let node_part = rest[..idx].trim_end().to_string();
            let shape_data = consume_shape_data(lines, &rest[idx..], rest_start + idx)?;
            return Ok((node_part, Some(shape_data)));
        }
    }

    Ok((rest.trim_end().to_string(), None))
}

fn parse_kanban_db(code: &str, meta: &ParseMetadata) -> Result<KanbanDb> {
    let mut db = KanbanDb::default();
    db.clear();

    let mut lines = KanbanLineCursor::new(code);
    let mut found_header = false;
    let mut header_tail: Option<(String, usize)> = None;
    while let Some(line) = lines.next() {
        let t = strip_inline_comment(line.text);
        let trimmed = t.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.eq_ignore_ascii_case("kanban") {
            found_header = true;
            break;
        }
        if starts_with_case_insensitive(trimmed, "kanban")
            && trimmed.len() > "kanban".len()
            && trimmed["kanban".len()..]
                .chars()
                .next()
                .is_some_and(|c| c.is_whitespace())
        {
            found_header = true;
            let after_keyword = &trimmed["kanban".len()..];
            let rest = after_keyword.trim_start();
            if !rest.is_empty() {
                let after_keyword_start = line.start + t.find(after_keyword).unwrap_or(0);
                header_tail = Some((after_keyword.to_string(), after_keyword_start));
            }
            break;
        }
        break;
    }

    if !found_header {
        return Err(Error::diagram_parse_fallback(
            meta.diagram_type.clone(),
            "expected kanban header".to_string(),
        ));
    }

    if let Some((tail, tail_start)) = &header_tail {
        let tail = strip_inline_comment(tail);
        let tail = tail.trim_end();
        if !tail.trim().is_empty() {
            let (indent, rest) = split_indent(tail);
            let rest = rest.trim_end();
            let rest_start = tail_start + tail.find(rest).unwrap_or(0);
            if starts_with_case_insensitive(rest, "::icon(") {
                let after = &rest["::icon(".len()..];
                if let Some(end) = after.find(')') {
                    db.decorate_last(None, Some(after[..end].to_string()), &meta.effective_config);
                }
            } else if let Some(after) = rest.strip_prefix(":::") {
                db.decorate_last(Some(after.trim().to_string()), None, &meta.effective_config);
            } else {
                let (node_part, shape_data) =
                    split_node_and_shape_data(&mut lines, rest, rest_start)?;
                if !node_part.trim().is_empty() {
                    let (spec, span) = parse_node_spec_for_render(&node_part, tail, *tail_start)?;
                    db.add_node(indent, spec, span, shape_data, &meta.effective_config)?;
                }
            }
        }
    }

    while let Some(source_line) = lines.next() {
        let line = strip_inline_comment(source_line.text);
        let line = line.trim_end();
        if line.trim().is_empty() {
            continue;
        }

        let (indent, rest) = split_indent(line);
        let rest = rest.trim_end();
        let rest_start = source_line.start + line.find(rest).unwrap_or(0);
        if rest.is_empty() {
            continue;
        }

        if starts_with_case_insensitive(rest, "::icon(") {
            let after = &rest["::icon(".len()..];
            if let Some(end) = after.find(')') {
                db.decorate_last(None, Some(after[..end].to_string()), &meta.effective_config);
            }
            continue;
        }

        if let Some(after) = rest.strip_prefix(":::") {
            db.decorate_last(Some(after.trim().to_string()), None, &meta.effective_config);
            continue;
        }

        let (node_part, shape_data) = split_node_and_shape_data(&mut lines, rest, rest_start)?;
        if node_part.trim().is_empty() {
            continue;
        }

        let (spec, span) = parse_node_spec_for_render(&node_part, line, source_line.start)?;

        db.add_node(indent, spec, span, shape_data, &meta.effective_config)?;
    }

    Ok(db)
}

pub fn parse_kanban(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let db = parse_kanban_db(code, meta)?;
    let mut out = Map::with_capacity(6);
    out.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
    out.insert("sections".to_string(), db.sections_json());
    out.insert(
        "nodes".to_string(),
        Value::Array(db.data_nodes_json(&meta.effective_config)),
    );
    out.insert("edges".to_string(), Value::Array(Vec::new()));
    out.insert("other".to_string(), Value::Object(Map::new()));
    out.insert(
        "config".to_string(),
        crate::config::clone_value_nonrecursive(meta.effective_config.as_value()),
    );
    Ok(Value::Object(out))
}

pub fn parse_kanban_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<KanbanDiagramRenderModel> {
    let db = parse_kanban_db(code, meta)?;
    Ok(KanbanDiagramRenderModel {
        nodes: db.data_nodes_for_render(&meta.effective_config),
    })
}

fn parse_node_spec_spanned(
    trimmed: &str,
    line: &str,
    line_start: usize,
) -> std::result::Result<Option<SpannedText>, String> {
    if let Some((id, descr, ty, span)) = parse_node_spec_spanned_inner(trimmed, line, line_start)? {
        let text = if id.is_empty() { descr } else { id };
        return Ok(Some(SpannedText {
            text,
            span,
            kind: if ty == NODE_TYPE_CIRCLE
                || ty == NODE_TYPE_CLOUD
                || ty == NODE_TYPE_BANG
                || ty == NODE_TYPE_HEXAGON
            {
                EditorSemanticKind::Object
            } else {
                EditorSemanticKind::Variable
            },
        }));
    }
    Ok(None)
}

fn parse_node_spec_spanned_inner(
    input: &str,
    line: &str,
    line_start: usize,
) -> std::result::Result<Option<(String, String, i32, SourceSpan)>, String> {
    let input = input.trim_end();
    if input.is_empty() {
        return Ok(None);
    }

    if let Some((start, end)) = node_delimiter_pair_at_start(input) {
        let (inner, tail) = extract_delimited(input, start, end)?;
        if !tail.trim().is_empty() {
            return Err("unexpected trailing input".to_string());
        }
        let descr = unquote_node_descr(inner);
        let ty = node_type_for(start, end);
        let rel = line.find(inner).unwrap_or(0);
        return Ok(Some((
            descr.clone(),
            descr,
            ty,
            SourceSpan::new(line_start + rel, line_start + rel + inner.len()),
        )));
    }

    let (id_raw, rest) = split_node_id(input);
    let id_raw = id_raw.to_string();
    let rest = rest.trim_end();
    let id_span = line.find(id_raw.as_str()).unwrap_or(0);
    if rest.is_empty() {
        let span_end = line_start + id_span + id_raw.len();
        return Ok(Some((
            id_raw.clone(),
            id_raw,
            NODE_TYPE_DEFAULT,
            SourceSpan::new(line_start + id_span, span_end),
        )));
    }

    let Some((start, end)) = node_delimiter_pair_at_start(rest) else {
        return Err("expected node delimiter".to_string());
    };

    let (inner, tail) = extract_delimited(rest, start, end)?;
    if !tail.trim().is_empty() {
        return Err("unexpected trailing input".to_string());
    }

    let descr = unquote_node_descr(inner);
    let ty = node_type_for(start, end);
    let rel = line.find(inner).unwrap_or(id_span);
    Ok(Some((
        id_raw,
        descr,
        ty,
        SourceSpan::new(line_start + rel, line_start + rel + inner.len()),
    )))
}

fn parse_icon_spanned(line: &str, line_start: usize) -> Option<SpannedText> {
    let t = line.trim_start();
    let prefix = "::icon(";
    if !t.starts_with(prefix) {
        return None;
    }
    let rest = &t[prefix.len()..];
    let end = rest.find(')')?;
    let value = rest[..end].trim();
    let rel = line.find(value).unwrap_or(0);
    Some(SpannedText {
        text: value.to_string(),
        span: SourceSpan::new(line_start + rel, line_start + rel + value.len()),
        kind: EditorSemanticKind::String,
    })
}

fn parse_css_class_spanned(line: &str, line_start: usize) -> Option<SpannedText> {
    let t = line.trim_start();
    if !t.starts_with(":::") {
        return None;
    }
    let value = t.trim_start_matches(":::").trim();
    if value.is_empty() {
        return None;
    }
    let rel = line.find(value).unwrap_or(0);
    Some(SpannedText {
        text: value.to_string(),
        span: SourceSpan::new(line_start + rel, line_start + rel + value.len()),
        kind: EditorSemanticKind::String,
    })
}

pub fn parse_kanban_editor_facts(code: &str, _meta: &ParseMetadata) -> EditorSemanticFacts {
    let mut facts = EditorSemanticFacts::new();
    let mut offset = 0usize;
    let mut header_seen = false;
    let mut section_level: Option<usize> = None;

    for segment in code.split_inclusive('\n') {
        let line_start = offset;
        offset += segment.len();
        let line = strip_line_ending(segment);
        let stripped = strip_inline_comment(line);
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            continue;
        }

        if !header_seen {
            if !starts_with_case_insensitive(trimmed, "kanban") {
                return facts;
            }
            header_seen = true;
            if trimmed.len() > "kanban".len() {
                let after_keyword = &trimmed["kanban".len()..];
                let rest = after_keyword.trim_start();
                if !rest.is_empty() {
                    let indent = after_keyword
                        .chars()
                        .take_while(|c| c.is_whitespace())
                        .count();
                    section_level.get_or_insert(indent);
                    let rel = line.find(rest).unwrap_or(0);
                    if let Some(icon) = parse_icon_spanned(rest, line_start + rel) {
                        facts.push_directive_prefix("icon");
                        facts.push_symbol(EditorSemanticSymbol::payload(
                            icon.text,
                            Some("kanban icon".to_string()),
                            EditorSemanticKind::String,
                            icon.span,
                            icon.span,
                        ));
                    } else if let Some(class_name) = parse_css_class_spanned(rest, line_start + rel)
                    {
                        facts.push_directive_prefix(":::");
                        facts.push_symbol(EditorSemanticSymbol::payload(
                            class_name.text,
                            Some("kanban class".to_string()),
                            EditorSemanticKind::String,
                            class_name.span,
                            class_name.span,
                        ));
                    } else {
                        match parse_node_spec_spanned(rest, line, line_start)
                            .ok()
                            .flatten()
                        {
                            Some(value) => {
                                facts.push_symbol(EditorSemanticSymbol::outline(
                                    value.text,
                                    Some("kanban section".to_string()),
                                    EditorSemanticKind::Namespace,
                                    value.span,
                                    value.span,
                                ));
                            }
                            None => {
                                facts.mark_recovered_with_diagnostic(
                                    "Unable to recover kanban node semantics from the header line",
                                    Some(SourceSpan::new(
                                        line_start + rel,
                                        line_start + rel + rest.len(),
                                    )),
                                );
                            }
                        }
                    }
                }
            }
            continue;
        }

        let (indent, rest) = split_indent(stripped);
        let rest = rest.trim_end();
        let rest_offset = line_start + line.find(rest).unwrap_or(0);

        if let Some(icon) = parse_icon_spanned(rest, rest_offset) {
            facts.push_directive_prefix("icon");
            facts.push_symbol(EditorSemanticSymbol::payload(
                icon.text,
                Some("kanban icon".to_string()),
                EditorSemanticKind::String,
                icon.span,
                icon.span,
            ));
            continue;
        }

        if let Some(class_name) = parse_css_class_spanned(rest, rest_offset) {
            facts.push_directive_prefix(":::");
            facts.push_symbol(EditorSemanticSymbol::payload(
                class_name.text,
                Some("kanban class".to_string()),
                EditorSemanticKind::String,
                class_name.span,
                class_name.span,
            ));
            continue;
        }

        if let Some(value) = parse_node_spec_spanned(rest, line, line_start)
            .ok()
            .flatten()
        {
            let is_section = match section_level {
                Some(level) => indent == level,
                None => {
                    section_level = Some(indent);
                    true
                }
            };

            if is_section {
                facts.push_symbol(EditorSemanticSymbol::outline(
                    value.text,
                    Some("kanban section".to_string()),
                    EditorSemanticKind::Namespace,
                    value.span,
                    value.span,
                ));
            } else {
                facts.push_symbol(EditorSemanticSymbol::new(
                    value.text,
                    Some("kanban item".to_string()),
                    value.kind,
                    value.span,
                    value.span,
                ));
            }
            continue;
        } else if rest.contains('[')
            || rest.contains('(')
            || rest.contains('{')
            || rest.contains(')')
            || rest.contains(']')
        {
            facts.mark_recovered_with_diagnostic(
                "Unable to fully recover kanban node semantics",
                Some(SourceSpan::new(
                    line_start + line.find(rest).unwrap_or(0),
                    line_start + line.find(rest).unwrap_or(0) + rest.len(),
                )),
            );
        }
    }

    facts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Engine, ParseDiagnosticSpanKind, ParseOptions};
    use futures::executor::block_on;

    fn parse(text: &str) -> Value {
        let engine = Engine::new();
        block_on(engine.parse_diagram(text, ParseOptions::default()))
            .unwrap()
            .unwrap()
            .model
    }

    fn parse_err(text: &str) -> crate::ParseDiagnostic {
        let engine = Engine::new();
        match block_on(engine.parse_diagram(text, ParseOptions::default())).unwrap_err() {
            Error::DiagramParse { diagnostic, .. } => diagnostic,
            other => panic!("expected kanban parse error, got {other:?}"),
        }
    }

    fn sections(model: &Value) -> Vec<Value> {
        model["sections"].as_array().cloned().unwrap_or_default()
    }

    fn data_nodes(model: &Value) -> Vec<Value> {
        model["nodes"].as_array().cloned().unwrap_or_default()
    }

    #[test]
    fn parse_kanban_editor_facts_expose_parser_backed_spans() {
        let engine = Engine::new();
        let text = "kanban\n    root\n      child1\n    :::highlight\n";
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("kanban", text, ParseOptions::strict())
            .unwrap()
            .unwrap();

        assert!(facts.symbols.iter().any(|symbol| symbol.name == "root"));
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "child1"));
        assert!(
            facts
                .symbols
                .iter()
                .any(|symbol| symbol.name == "highlight")
        );
    }

    #[test]
    fn kanban_recovered_editor_fact_diagnostics_are_english() {
        let engine = Engine::new();
        let text = "kanban\n  broken[Open\n";
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("kanban", text, ParseOptions::strict())
            .unwrap()
            .unwrap();

        assert!(facts.diagnostics.iter().any(
            |diagnostic| diagnostic.message == "Unable to fully recover kanban node semantics"
        ));
        assert!(facts.diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message
                .chars()
                .any(|ch| ('\u{4E00}'..='\u{9FFF}').contains(&ch))
        }));
    }

    #[test]
    fn kanban_unterminated_node_delimiter_reports_insertion_point() {
        let text = "kanban\n  root[Open\n";
        let diagnostic = parse_err(text);
        let offset = text.trim_end().len();

        assert_eq!(diagnostic.message(), "unterminated node delimiter");
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(offset, offset)));
        assert_eq!(
            diagnostic.span_kind(),
            ParseDiagnosticSpanKind::InsertionPoint
        );
    }

    #[test]
    fn kanban_trailing_node_input_reports_exact_span() {
        let text = "kanban\n  root[Root] extra\n";
        let diagnostic = parse_err(text);
        let offset = text.find("extra").unwrap();

        assert_eq!(diagnostic.message(), "unexpected trailing input");
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(offset, offset + "extra".len()))
        );
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
    }

    #[test]
    fn kanban_unterminated_metadata_reports_eof_insertion_point() {
        let text = "kanban\n  root@{ icon: star\n";
        let diagnostic = parse_err(text);

        assert_eq!(diagnostic.message(), "unterminated @{ ... } metadata block");
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(text.len(), text.len()))
        );
        assert_eq!(
            diagnostic.span_kind(),
            ParseDiagnosticSpanKind::InsertionPoint
        );
    }

    #[test]
    fn kanban_invalid_shape_metadata_reports_exact_metadata_span() {
        let text = "kanban\n  root@{ shape: bad_shape }\n";
        let diagnostic = parse_err(text);
        let offset = text.find("@{").unwrap();

        assert!(diagnostic.message().contains("No such shape"));
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(
                offset,
                offset + "@{ shape: bad_shape }".len()
            ))
        );
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
    }

    #[test]
    fn knbn_1_simple_root() {
        let model = parse("kanban\n    root");
        let sections = sections(&model);
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0]["label"].as_str().unwrap(), "root");
    }

    #[test]
    fn knbn_2_hierarchy_two_children() {
        let model = parse("kanban\n    root\n      child1\n      child2\n");
        let sections = sections(&model);
        let nodes = data_nodes(&model);
        let section_id = sections[0]["id"].as_str().unwrap();
        let children: Vec<Value> = nodes
            .into_iter()
            .filter(|n| n["parentId"].as_str() == Some(section_id))
            .collect();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0]["label"].as_str().unwrap(), "root");
        assert_eq!(children.len(), 2);
        assert_eq!(children[0]["label"].as_str().unwrap(), "child1");
        assert_eq!(children[1]["label"].as_str().unwrap(), "child2");
    }

    #[test]
    fn knbn_3_shape_without_id() {
        let model = parse("kanban\n    (root)");
        let sections = sections(&model);
        assert_eq!(sections[0]["label"].as_str().unwrap(), "root");
    }

    #[test]
    fn knbn_4_does_not_distinguish_deeper_levels() {
        let model = parse("kanban\n    root\n      child1\n        leaf1\n      child2");
        let sections = sections(&model);
        let nodes = data_nodes(&model);
        let section_id = sections[0]["id"].as_str().unwrap();
        let children: Vec<Value> = nodes
            .into_iter()
            .filter(|n| n["parentId"].as_str() == Some(section_id))
            .collect();
        assert_eq!(sections.len(), 1);
        assert_eq!(children.len(), 3);
    }

    #[test]
    fn knbn_5_multiple_sections() {
        let model = parse("kanban\n    section1\n    section2");
        let sections = sections(&model);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0]["label"].as_str().unwrap(), "section1");
        assert_eq!(sections[1]["label"].as_str().unwrap(), "section2");
    }

    #[test]
    fn knbn_6_real_root_in_wrong_place_is_error() {
        let engine = Engine::new();
        let text = "kanban\n          root\n        fakeRoot\n    realRootWrongPlace";
        let err = block_on(engine.parse_diagram(text, ParseOptions::default())).unwrap_err();
        assert!(
            err.to_string()
                .contains("Items without section detected, found section (\"fakeRoot\")")
        );
    }

    #[test]
    fn knbn_7_id_and_label_rect() {
        let model = parse("kanban\n    root[The root]\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["id"].as_str().unwrap(), "root");
        assert_eq!(sections[0]["label"].as_str().unwrap(), "The root");
    }

    #[test]
    fn knbn_8_child_id_and_label() {
        let model = parse("kanban\n    root\n      theId(child1)");
        let sections = sections(&model);
        let nodes = data_nodes(&model);
        let section_id = sections[0]["id"].as_str().unwrap();
        let children: Vec<Value> = nodes
            .into_iter()
            .filter(|n| n["parentId"].as_str() == Some(section_id))
            .collect();
        assert_eq!(sections[0]["label"].as_str().unwrap(), "root");
        assert_eq!(children.len(), 1);
        assert_eq!(children[0]["label"].as_str().unwrap(), "child1");
        assert_eq!(children[0]["id"].as_str().unwrap(), "theId");
    }

    #[test]
    fn knbn_9_child_id_and_label_without_indent_on_root() {
        let model = parse("kanban\nroot\n      theId(child1)");
        let sections = sections(&model);
        let nodes = data_nodes(&model);
        let section_id = sections[0]["id"].as_str().unwrap();
        let children: Vec<Value> = nodes
            .into_iter()
            .filter(|n| n["parentId"].as_str() == Some(section_id))
            .collect();
        assert_eq!(sections[0]["label"].as_str().unwrap(), "root");
        assert_eq!(children.len(), 1);
        assert_eq!(children[0]["label"].as_str().unwrap(), "child1");
        assert_eq!(children[0]["id"].as_str().unwrap(), "theId");
    }

    #[test]
    fn knbn_13_set_icon_for_node() {
        let model = parse("kanban\n    root[The root]\n    ::icon(bomb)\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["id"].as_str().unwrap(), "root");
        assert_eq!(sections[0]["label"].as_str().unwrap(), "The root");
        assert_eq!(sections[0]["icon"].as_str().unwrap(), "bomb");
    }

    #[test]
    fn knbn_14_set_classes_for_node() {
        let model = parse("kanban\n    root[The root]\n    :::m-4 p-8\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["id"].as_str().unwrap(), "root");
        assert_eq!(sections[0]["label"].as_str().unwrap(), "The root");
        assert_eq!(sections[0]["cssClasses"].as_str().unwrap(), "m-4 p-8");
    }

    #[test]
    fn knbn_15_set_classes_and_icon_classes_first() {
        let model = parse("kanban\n    root[The root]\n    :::m-4 p-8\n    ::icon(bomb)\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["cssClasses"].as_str().unwrap(), "m-4 p-8");
        assert_eq!(sections[0]["icon"].as_str().unwrap(), "bomb");
    }

    #[test]
    fn knbn_16_set_classes_and_icon_icon_first() {
        let model = parse("kanban\n    root[The root]\n    ::icon(bomb)\n    :::m-4 p-8\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["cssClasses"].as_str().unwrap(), "m-4 p-8");
        assert_eq!(sections[0]["icon"].as_str().unwrap(), "bomb");
    }

    #[test]
    fn knbn_17_node_syntax_in_description() {
        let model = parse("kanban\n    root[\"String containing []\"]\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["id"].as_str().unwrap(), "root");
        assert_eq!(
            sections[0]["label"].as_str().unwrap(),
            "String containing []"
        );
    }

    #[test]
    fn knbn_18_node_syntax_in_child_description() {
        let model = parse(
            "kanban\n    root[\"String containing []\"]\n      child1[\"String containing ()\"]\n",
        );
        let sections = sections(&model);
        let nodes = data_nodes(&model);
        let section_id = sections[0]["id"].as_str().unwrap();
        let children: Vec<Value> = nodes
            .into_iter()
            .filter(|n| n["parentId"].as_str() == Some(section_id))
            .collect();
        assert_eq!(sections[0]["id"].as_str().unwrap(), "root");
        assert_eq!(
            sections[0]["label"].as_str().unwrap(),
            "String containing []"
        );
        assert_eq!(children.len(), 1);
        assert_eq!(
            children[0]["label"].as_str().unwrap(),
            "String containing ()"
        );
    }

    #[test]
    fn knbn_19_child_after_class_assignment() {
        let model = parse(
            "kanban\n  root(Root)\n    Child(Child)\n    :::hot\n      a(a)\n      b[New Stuff]",
        );
        let sections = sections(&model);
        let nodes = data_nodes(&model);
        let section_id = sections[0]["id"].as_str().unwrap();
        let children: Vec<Value> = nodes
            .into_iter()
            .filter(|n| n["parentId"].as_str() == Some(section_id))
            .collect();
        assert_eq!(sections[0]["id"].as_str().unwrap(), "root");
        assert_eq!(sections[0]["label"].as_str().unwrap(), "Root");
        assert_eq!(children.len(), 3);
        assert_eq!(children[0]["id"].as_str().unwrap(), "Child");
        assert_eq!(children[1]["id"].as_str().unwrap(), "a");
        assert_eq!(children[2]["id"].as_str().unwrap(), "b");
    }

    #[test]
    fn knbn_20_empty_rows() {
        let model =
            parse("kanban\n  root(Root)\n    Child(Child)\n      a(a)\n\n      b[New Stuff]");
        let sections = sections(&model);
        let nodes = data_nodes(&model);
        let section_id = sections[0]["id"].as_str().unwrap();
        let children: Vec<Value> = nodes
            .into_iter()
            .filter(|n| n["parentId"].as_str() == Some(section_id))
            .collect();
        assert_eq!(sections[0]["id"].as_str().unwrap(), "root");
        assert_eq!(sections[0]["label"].as_str().unwrap(), "Root");
        assert_eq!(children.len(), 3);
        assert_eq!(children[0]["id"].as_str().unwrap(), "Child");
        assert_eq!(children[1]["id"].as_str().unwrap(), "a");
        assert_eq!(children[2]["id"].as_str().unwrap(), "b");
    }

    #[test]
    fn knbn_22_inline_comment_at_end_of_line() {
        let model = parse(
            "kanban\n  root(Root)\n    Child(Child)\n      a(a) %% This is a comment\n      b[New Stuff]",
        );
        let sections = sections(&model);
        let nodes = data_nodes(&model);
        let section_id = sections[0]["id"].as_str().unwrap();
        let children: Vec<Value> = nodes
            .into_iter()
            .filter(|n| n["parentId"].as_str() == Some(section_id))
            .collect();
        assert_eq!(sections[0]["id"].as_str().unwrap(), "root");
        assert_eq!(children.len(), 3);
        assert_eq!(children[0]["id"].as_str().unwrap(), "Child");
        assert_eq!(children[1]["id"].as_str().unwrap(), "a");
        assert_eq!(children[2]["id"].as_str().unwrap(), "b");
    }

    #[test]
    fn knbn_23_rows_with_only_spaces_should_not_interfere() {
        let model = parse("kanban\nroot\n A\n \n\n B");
        let sections = sections(&model);
        let nodes = data_nodes(&model);
        let section_id = sections[0]["id"].as_str().unwrap();
        let children: Vec<Value> = nodes
            .into_iter()
            .filter(|n| n["parentId"].as_str() == Some(section_id))
            .collect();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0]["id"].as_str().unwrap(), "A");
        assert_eq!(children[1]["id"].as_str().unwrap(), "B");
    }

    #[test]
    fn knbn_24_rows_above_header() {
        let model = parse("\n \nkanban\nroot\n A\n \n\n B");
        let sections = sections(&model);
        let nodes = data_nodes(&model);
        let section_id = sections[0]["id"].as_str().unwrap();
        let children: Vec<Value> = nodes
            .into_iter()
            .filter(|n| n["parentId"].as_str() == Some(section_id))
            .collect();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0]["id"].as_str().unwrap(), "A");
        assert_eq!(children[1]["id"].as_str().unwrap(), "B");
    }

    #[test]
    fn knbn_30_priority_metadata() {
        let model = parse("kanban\n        root@{ priority: high }\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["id"].as_str().unwrap(), "root");
        assert_eq!(sections[0]["priority"].as_str().unwrap(), "high");
    }

    #[test]
    fn knbn_31_assigned_metadata() {
        let model = parse("kanban\n        root@{ assigned: knsv }\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["assigned"].as_str().unwrap(), "knsv");
    }

    #[test]
    fn knbn_32_icon_metadata() {
        let model = parse("kanban\n        root@{ icon: star }\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["icon"].as_str().unwrap(), "star");
    }

    #[test]
    fn knbn_34_multiline_metadata() {
        let model = parse(
            "kanban\n        root@{\n          icon: star\n          assigned: knsv\n        }\n",
        );
        let sections = sections(&model);
        assert_eq!(sections[0]["icon"].as_str().unwrap(), "star");
        assert_eq!(sections[0]["assigned"].as_str().unwrap(), "knsv");
    }

    #[test]
    fn knbn_35_inline_metadata_multiple_pairs() {
        let model = parse("kanban\n        root@{ icon: star, assigned: knsv }\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["icon"].as_str().unwrap(), "star");
        assert_eq!(sections[0]["assigned"].as_str().unwrap(), "knsv");
    }

    #[test]
    fn knbn_36_label_override_metadata() {
        let model = parse("kanban\n        root@{ icon: star, label: 'fix things' }\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["label"].as_str().unwrap(), "fix things");
    }

    #[test]
    fn knbn_37_ticket_metadata() {
        let model = parse("kanban\n        root@{ ticket: MC-1234 }\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["ticket"].as_str().unwrap(), "MC-1234");
    }

    #[test]
    fn kanban_get_data_sanitizes_labels_again() {
        let model = parse("kanban\n    root[<b>x</b>]");
        let nodes = data_nodes(&model);
        assert_eq!(nodes[0]["label"].as_str().unwrap(), "<b>x</b>");
    }

    #[test]
    fn kanban_shape_data_rewrites_newline_whitespace_in_double_quotes() {
        let model = parse("kanban\n  root@{ label: \"line1\n      line2\" }\n");
        let sections = sections(&model);
        assert_eq!(sections[0]["label"].as_str().unwrap(), "line1<br/>line2");
    }
}
