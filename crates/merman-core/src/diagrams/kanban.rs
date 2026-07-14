use crate::diagrams::scan::{split_indent, starts_with_case_insensitive, strip_line_ending};
use crate::sanitize::sanitize_text;
use crate::{
    EditorSemanticFacts, EditorSemanticKind, EditorSemanticSymbol, Error, MermaidConfig,
    ParseMetadata, Result, SourceSpan,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static KANBAN_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn reset_kanban_syntax_construction_count() {
    KANBAN_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(test)]
pub(crate) fn kanban_syntax_construction_count() -> usize {
    KANBAN_SYNTAX_CONSTRUCTION_COUNT.get()
}

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    #[serde(skip)]
    compatibility: KanbanRenderNodeCompatibility,
}

#[derive(Debug, Clone, Default)]
struct KanbanRenderNodeCompatibility {
    level: usize,
    width: i64,
    padding: i64,
    ticket: Option<String>,
    priority: Option<String>,
    assigned: Option<String>,
    icon: Option<String>,
    css_classes: Option<String>,
    shape: Option<String>,
}

impl KanbanRenderNode {
    /// Creates a render node with no parent or optional card metadata.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            ..Self::default()
        }
    }
}

#[derive(Debug, Default)]
struct KanbanDb {
    nodes: Vec<KanbanNode>,
    section_indices: Vec<usize>,
    next_auto_id: i64,
}

struct KanbanSemanticSource {
    db: KanbanDb,
    editor_facts: EditorSemanticFacts,
}

struct KanbanParseFailure {
    error: Box<Error>,
    editor_facts: Box<EditorSemanticFacts>,
    span: SourceSpan,
}

impl KanbanParseFailure {
    fn into_editor_facts(self) -> EditorSemanticFacts {
        let mut facts = *self.editor_facts;
        facts.mark_recovered_from_parse_error(
            format!("kanban parser recovered after parse error: {}", self.error),
            Some(self.span),
        );
        facts
    }
}

#[derive(Debug, Clone)]
struct SpannedText {
    text: String,
    span: SourceSpan,
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
                compatibility: KanbanRenderNodeCompatibility {
                    level: section.level,
                    width: section.width,
                    padding: section.padding,
                    ticket: section.ticket.clone(),
                    priority: section.priority.clone(),
                    assigned: section.assigned.clone(),
                    icon: section.icon.clone(),
                    css_classes: section.css_classes.clone(),
                    shape: section.shape.clone(),
                },
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
                    compatibility: KanbanRenderNodeCompatibility {
                        level: item.level,
                        width: item.width,
                        padding: item.padding,
                        ticket: item.ticket.clone(),
                        priority: item.priority.clone(),
                        assigned: item.assigned.clone(),
                        icon: item.icon.clone(),
                        css_classes: item.css_classes.clone(),
                        shape: item.shape.clone(),
                    },
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

fn construct_kanban_semantic_source(
    code: &str,
    meta: &ParseMetadata,
) -> std::result::Result<KanbanSemanticSource, KanbanParseFailure> {
    #[cfg(test)]
    KANBAN_SYNTAX_CONSTRUCTION_COUNT.set(KANBAN_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

    let mut db = KanbanDb::default();
    db.clear();
    let mut editor_facts = EditorSemanticFacts::new();

    let mut lines = KanbanLineCursor::new(code);
    let header_tail = loop {
        let Some(line) = lines.next() else {
            let span = SourceSpan::new(lines.offset(), lines.offset());
            return Err(KanbanParseFailure {
                error: Box::new(Error::diagram_parse_insertion_point(
                    meta.diagram_type.clone(),
                    "expected kanban header",
                    span.start,
                )),
                editor_facts: Box::new(editor_facts),
                span,
            });
        };
        let t = strip_inline_comment(line.text);
        let trimmed = t.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.eq_ignore_ascii_case("kanban") {
            break None;
        }
        if starts_with_case_insensitive(trimmed, "kanban")
            && trimmed.len() > "kanban".len()
            && trimmed["kanban".len()..]
                .chars()
                .next()
                .is_some_and(|c| c.is_whitespace())
        {
            let after_keyword = &trimmed["kanban".len()..];
            let rest = after_keyword.trim_start();
            if !rest.is_empty() {
                let after_keyword_start = line.start + t.find(after_keyword).unwrap_or(0);
                break Some((after_keyword.to_string(), after_keyword_start));
            }
            break None;
        }
        let rel = line.text.find(trimmed).unwrap_or_default();
        let span = SourceSpan::new(line.start + rel, line.start + rel + trimmed.len());
        return Err(KanbanParseFailure {
            error: Box::new(Error::diagram_parse_exact(
                meta.diagram_type.clone(),
                "expected kanban header",
                span,
            )),
            editor_facts: Box::new(editor_facts),
            span,
        });
    };

    if let Some((tail, tail_start)) = &header_tail
        && let Err(error) = parse_kanban_statement(
            &mut lines,
            &mut db,
            &mut editor_facts,
            tail,
            *tail_start,
            meta,
        )
    {
        let fallback = SourceSpan::new(*tail_start, *tail_start + tail.len());
        let span = kanban_error_span(&error, fallback);
        return Err(KanbanParseFailure {
            error: Box::new(error),
            editor_facts: Box::new(editor_facts),
            span,
        });
    }

    while let Some(source_line) = lines.next() {
        if let Err(error) = parse_kanban_statement(
            &mut lines,
            &mut db,
            &mut editor_facts,
            source_line.text,
            source_line.start,
            meta,
        ) {
            let fallback = SourceSpan::new(
                source_line.start,
                source_line.start + source_line.text.len(),
            );
            let span = kanban_error_span(&error, fallback);
            return Err(KanbanParseFailure {
                error: Box::new(error),
                editor_facts: Box::new(editor_facts),
                span,
            });
        }
    }

    Ok(KanbanSemanticSource { db, editor_facts })
}

fn parse_kanban_statement(
    lines: &mut KanbanLineCursor<'_>,
    db: &mut KanbanDb,
    facts: &mut EditorSemanticFacts,
    source: &str,
    source_start: usize,
    meta: &ParseMetadata,
) -> Result<()> {
    let line = strip_inline_comment(source).trim_end();
    if line.trim().is_empty() {
        return Ok(());
    }
    let (indent, rest) = split_indent(line);
    let rest = rest.trim_end();
    if rest.is_empty() {
        return Ok(());
    }
    let rest_start = source_start + line.find(rest).unwrap_or_default();

    if starts_with_case_insensitive(rest, "::icon(") {
        if let Some(icon) = parse_icon_spanned(rest, rest_start) {
            db.decorate_last(None, Some(icon.text.clone()), &meta.effective_config);
            facts.push_directive_prefix("icon");
            facts.push_symbol(EditorSemanticSymbol::payload(
                icon.text,
                Some("kanban icon".to_string()),
                EditorSemanticKind::String,
                icon.span,
                icon.span,
            ));
        }
        return Ok(());
    }

    if let Some(after) = rest.strip_prefix(":::") {
        db.decorate_last(Some(after.trim().to_string()), None, &meta.effective_config);
        if let Some(class_name) = parse_css_class_spanned(rest, rest_start) {
            facts.push_directive_prefix(":::");
            facts.push_symbol(EditorSemanticSymbol::payload(
                class_name.text,
                Some("kanban class".to_string()),
                EditorSemanticKind::String,
                class_name.span,
                class_name.span,
            ));
        }
        return Ok(());
    }

    let (node_part, shape_data) = split_node_and_shape_data(lines, rest, rest_start)?;
    if node_part.trim().is_empty() {
        return Ok(());
    }
    let (spec, span) = parse_node_spec_for_render(&node_part, &node_part, rest_start)?;
    let fact_name = if spec.id_raw.is_empty() {
        spec.descr_raw.clone()
    } else {
        spec.id_raw.clone()
    };
    let fact_kind = kanban_node_editor_kind(spec.ty);
    db.add_node(indent, spec, span, shape_data, &meta.effective_config)?;
    let is_section = db.nodes.last().is_some_and(|node| node.parent_id.is_none());
    if is_section {
        facts.push_symbol(EditorSemanticSymbol::outline(
            fact_name,
            Some("kanban section".to_string()),
            EditorSemanticKind::Namespace,
            span,
            span,
        ));
    } else {
        facts.push_symbol(EditorSemanticSymbol::new(
            fact_name,
            Some("kanban item".to_string()),
            fact_kind,
            span,
            span,
        ));
    }
    Ok(())
}

fn kanban_node_editor_kind(ty: i32) -> EditorSemanticKind {
    if matches!(
        ty,
        NODE_TYPE_CIRCLE | NODE_TYPE_CLOUD | NODE_TYPE_BANG | NODE_TYPE_HEXAGON
    ) {
        EditorSemanticKind::Object
    } else {
        EditorSemanticKind::Variable
    }
}

fn kanban_error_span(error: &Error, fallback: SourceSpan) -> SourceSpan {
    match error {
        Error::DiagramParse { diagnostic, .. } => diagnostic.span().unwrap_or(fallback),
        _ => fallback,
    }
}

pub fn parse_kanban(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let source = construct_kanban_semantic_source(code, meta).map_err(|failure| *failure.error)?;
    let model = kanban_db_into_render_model(&source.db, meta);
    render_model_to_compat_json(&model, meta)
}

pub(crate) fn parse_kanban_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
) -> Result<(Value, EditorSemanticFacts)> {
    let source = construct_kanban_semantic_source(code, meta).map_err(|failure| *failure.error)?;
    let model = kanban_db_into_render_model(&source.db, meta);
    Ok((
        render_model_to_compat_json(&model, meta)?,
        source.editor_facts,
    ))
}

fn kanban_db_into_render_model(db: &KanbanDb, meta: &ParseMetadata) -> KanbanDiagramRenderModel {
    KanbanDiagramRenderModel {
        nodes: db.data_nodes_for_render(&meta.effective_config),
    }
}

pub(crate) fn render_model_to_compat_json(
    model: &KanbanDiagramRenderModel,
    meta: &ParseMetadata,
) -> Result<Value> {
    let mut out = Map::with_capacity(6);
    out.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
    out.insert("sections".to_string(), kanban_sections_to_json(model));
    out.insert(
        "nodes".to_string(),
        Value::Array(kanban_nodes_to_json(model, &meta.effective_config)),
    );
    out.insert("edges".to_string(), Value::Array(Vec::new()));
    out.insert("other".to_string(), Value::Object(Map::new()));
    out.insert(
        "config".to_string(),
        crate::config::clone_value_nonrecursive(meta.effective_config.as_value()),
    );
    Ok(Value::Object(out))
}

fn kanban_sections_to_json(model: &KanbanDiagramRenderModel) -> Value {
    let sections = model
        .nodes
        .iter()
        .filter(|node| node.is_group)
        .map(|node| {
            let compat = &node.compatibility;
            let mut out = Map::new();
            out.insert("id".to_string(), json!(&node.id));
            out.insert("label".to_string(), json!(&node.label));
            out.insert("level".to_string(), json!(compat.level));
            out.insert("width".to_string(), json!(compat.width));
            out.insert("padding".to_string(), json!(compat.padding));
            out.insert("isGroup".to_string(), json!(false));
            for (key, value) in [
                ("ticket", &compat.ticket),
                ("priority", &compat.priority),
                ("assigned", &compat.assigned),
                ("icon", &compat.icon),
                ("cssClasses", &compat.css_classes),
                ("shape", &compat.shape),
            ] {
                if let Some(value) = value {
                    out.insert(key.to_string(), json!(value));
                }
            }
            Value::Object(out)
        })
        .collect();
    Value::Array(sections)
}

fn kanban_nodes_to_json(model: &KanbanDiagramRenderModel, config: &MermaidConfig) -> Vec<Value> {
    let look = config.get_str("look").unwrap_or("classic");
    model
        .nodes
        .iter()
        .map(|node| {
            if node.is_group {
                json!({
                    "id": node.id,
                    "label": node.label,
                    "isGroup": true,
                    "ticket": node.ticket,
                    "shape": "kanbanSection",
                    "level": node.compatibility.level,
                    "look": look,
                })
            } else {
                json!({
                    "id": node.id,
                    "parentId": node.parent_id,
                    "label": node.label,
                    "isGroup": false,
                    "ticket": node.ticket,
                    "priority": node.priority,
                    "assigned": node.assigned,
                    "icon": node.icon,
                    "shape": "kanbanItem",
                    "level": node.compatibility.level,
                    "rx": 5,
                    "ry": 5,
                    "cssStyles": ["text-align: left"],
                })
            }
        })
        .collect()
}

pub fn parse_kanban_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<KanbanDiagramRenderModel> {
    let source = construct_kanban_semantic_source(code, meta).map_err(|failure| *failure.error)?;
    Ok(kanban_db_into_render_model(&source.db, meta))
}

fn parse_icon_spanned(line: &str, line_start: usize) -> Option<SpannedText> {
    let t = line.trim_start();
    let prefix = "::icon(";
    if !starts_with_case_insensitive(t, prefix) {
        return None;
    }
    let rest = &t[prefix.len()..];
    let end = rest.find(')')?;
    let value = rest[..end].trim();
    let rel = line.find(value).unwrap_or(0);
    Some(SpannedText {
        text: value.to_string(),
        span: SourceSpan::new(line_start + rel, line_start + rel + value.len()),
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
    })
}

pub fn parse_kanban_editor_facts(code: &str, meta: &ParseMetadata) -> EditorSemanticFacts {
    match construct_kanban_semantic_source(code, meta) {
        Ok(source) => source.editor_facts,
        Err(failure) => failure.into_editor_facts(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EditorSemanticCompleteness, Engine, MermaidConfig, ParseDiagnosticSpanKind, ParseMetadata,
        ParseOptions,
    };
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

    fn meta() -> ParseMetadata {
        ParseMetadata {
            diagram_type: "kanban".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: MermaidConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    fn combined_parse_constructs_once_and_preserves_all_projections() {
        let text = concat!(
            "kanban\r\n",
            "  backlog\r\n",
            "    task1@{ ticket: MC-1 }\r\n",
            "    ::icon(star)\r\n",
            "    :::highlight\r\n",
        );
        let mut meta = meta();
        meta.effective_config = MermaidConfig::from_value(serde_json::json!({
            "look": "handDrawn",
            "theme": "forest"
        }));
        let expected_json = parse_kanban(text, &meta).unwrap();
        let expected_facts = parse_kanban_editor_facts(text, &meta);
        let expected_model = parse_kanban_model_for_render(text, &meta).unwrap();

        reset_kanban_syntax_construction_count();
        let (json, facts) = parse_kanban_json_and_editor_facts(text, &meta).unwrap();

        assert_eq!(kanban_syntax_construction_count(), 1);
        assert_eq!(json, expected_json);
        assert_eq!(facts, expected_facts);
        assert_eq!(
            render_model_to_compat_json(&expected_model, &meta).unwrap(),
            json
        );
        assert_eq!(json["type"], "kanban");
        assert_eq!(json["config"], meta.effective_config.as_value().clone());
        assert_eq!(json["nodes"][0]["look"], "handDrawn");
        assert!(json["nodes"][0]["ticket"].is_null());

        for name in ["backlog", "task1", "star", "highlight"] {
            let start = text.find(name).unwrap();
            assert!(
                facts.symbols.iter().any(|symbol| {
                    symbol.name == name
                        && symbol.selection == SourceSpan::new(start, start + name.len())
                }),
                "missing exact Kanban fact for {name:?}"
            );
        }
    }

    #[test]
    fn malformed_editor_input_recovers_from_one_construction() {
        let text = "kanban\n  backlog[Backlog]\n    broken[Open\n";
        reset_kanban_syntax_construction_count();
        let facts = parse_kanban_editor_facts(text, &meta());

        assert_eq!(kanban_syntax_construction_count(), 1);
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "backlog"));
        assert_eq!(facts.diagnostics.len(), 1);
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

        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .starts_with("kanban parser recovered after parse error:")
        }));
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
