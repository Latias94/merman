#[cfg(feature = "presentation")]
use crate::ParseMetadata;
use crate::diagrams::scan::{split_indent, starts_with_case_insensitive, strip_line_ending};
use crate::family::EditorFactsDemand;
use crate::sanitize::sanitize_text;
use crate::{
    EditorSemanticFacts, EditorSemanticKind, EditorSemanticSymbol, Error, MermaidConfig, Result,
    SourceSpan,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[cfg(feature = "compat-json")]
use serde_json::{Map, json};
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
    ty: i32,
    metadata: Option<crate::json_value::ManagedJson>,
    parent_id: Option<String>,

    ticket: Option<String>,
    priority: Option<String>,
    assigned: Option<String>,
    icon: Option<String>,
    css_classes: Option<String>,
    shape: Option<String>,
}

/// Authored Kanban sections, cards, and card metadata, without visual dimensions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KanbanDiagramModel {
    pub nodes: Vec<KanbanNodeModel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KanbanNodeModel {
    pub id: String,
    pub label: String,
    pub level: usize,
    pub node_type: i32,
    pub span: SourceSpan,
    pub is_group: bool,
    pub parent_id: Option<String>,
    pub ticket: Option<String>,
    pub priority: Option<String>,
    pub assigned: Option<String>,
    pub icon: Option<String>,
    pub css_classes: Option<String>,
    pub shape: Option<String>,
    /// Source-authored metadata, including properties unknown to the renderer.
    pub metadata: Option<crate::json_value::ManagedJson>,
}

#[cfg(feature = "presentation")]
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct KanbanDiagramRenderModel {
    #[serde(default)]
    pub nodes: Vec<KanbanRenderNode>,
}

#[cfg(feature = "presentation")]
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

#[cfg(feature = "presentation")]
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

#[cfg(feature = "presentation")]
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
    next_auto_id: i64,
}

#[cfg(feature = "presentation")]
struct KanbanPresentationPolicy {
    padding: i64,
}

#[cfg(feature = "presentation")]
impl KanbanPresentationPolicy {
    fn from_config(config: &MermaidConfig) -> Self {
        Self {
            padding: get_i64(config, "mindmap.padding").unwrap_or(10),
        }
    }

    fn validate_node(&self, ty: i32, span: SourceSpan) -> Result<()> {
        if matches!(
            ty,
            NODE_TYPE_ROUNDED_RECT | NODE_TYPE_RECT | NODE_TYPE_HEXAGON
        ) {
            self.padding.checked_mul(2).ok_or_else(|| {
                Error::diagram_parse_exact(
                    "kanban",
                    "node padding exceeds the supported integer range",
                    span,
                )
            })?;
        }
        Ok(())
    }
}

struct KanbanSemanticSource {
    db: KanbanDb,
    editor_facts: Option<EditorSemanticFacts>,
}

struct KanbanParseFailure {
    error: Box<Error>,
    editor_facts: Option<EditorSemanticFacts>,
    span: SourceSpan,
}

impl KanbanParseFailure {
    #[cfg(feature = "compat-json")]
    fn into_error_and_editor_facts(self) -> (Error, EditorSemanticFacts) {
        let mut facts = self.editor_facts.unwrap_or_default();
        facts.mark_recovered_from_parse_error(
            format!("kanban parser recovered after parse error: {}", self.error),
            Some(self.span),
        );
        (*self.error, facts)
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
struct ParsedKanbanNode {
    spec: KanbanNodeSpec,
    entity: Option<SpannedText>,
    label: Option<SpannedText>,
    span: SourceSpan,
}

#[derive(Debug, Clone)]
struct KanbanMetadataField {
    key: SpannedText,
    value: Option<SpannedText>,
    is_string_value: bool,
}

#[derive(Debug, Clone)]
struct KanbanShapeData {
    text: String,
    span: SourceSpan,
    fields: Vec<KanbanMetadataField>,
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
        shape_data: Option<(KanbanShapeData, crate::json_value::ManagedJson)>,
        config: &MermaidConfig,
    ) -> Result<()> {
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
            ty: spec.ty,
            metadata: None,
            parent_id: None,
            ticket: None,
            priority: None,
            assigned: None,
            icon: None,
            css_classes: None,
            shape: None,
        };

        if let Some((shape_data, document)) = shape_data {
            apply_shape_data(&mut node, document.as_value(), shape_data.span)?;
            node.metadata = Some(document);
        }

        if let Some(section_idx) = self.get_section_index(level)? {
            node.parent_id = Some(self.nodes[section_idx].id.clone());
        }
        self.nodes.push(node);
        Ok(())
    }

    fn into_semantic_model(self, config: &MermaidConfig) -> KanbanDiagramModel {
        KanbanDiagramModel {
            nodes: self
                .nodes
                .into_iter()
                .map(|node| KanbanNodeModel {
                    id: node.id,
                    label: sanitize_text(&node.label, config),
                    level: node.level,
                    node_type: node.ty,
                    span: node.span,
                    is_group: node.parent_id.is_none(),
                    parent_id: node.parent_id,
                    ticket: node.ticket,
                    priority: node.priority,
                    assigned: node.assigned,
                    icon: node.icon,
                    css_classes: node.css_classes,
                    shape: node.shape,
                    metadata: node.metadata,
                })
                .collect(),
        }
    }
}

fn apply_shape_data(node: &mut KanbanNode, document: &Value, span: SourceSpan) -> Result<()> {
    let Some(obj) = document.as_object() else {
        return Ok(());
    };

    if let Some(Value::String(shape)) = obj.get("shape") {
        if shape != &shape.to_lowercase() || shape.contains('_') {
            return Err(Error::diagram_parse_exact(
                "kanban",
                format!("No such shape: {shape}. Shape names should be lowercase."),
                span,
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

fn parse_node_spec_for_render(
    input: &str,
    input_start: usize,
    editor_demand: EditorFactsDemand,
) -> Result<ParsedKanbanNode> {
    let input = input.trim_end();
    if input.is_empty() {
        return Err(Error::diagram_parse_insertion_point(
            "kanban",
            "expected node",
            input_start,
        ));
    }

    if let Some((start, end)) = node_delimiter_pair_at_start(input) {
        let inner_start = input_start + start.len();
        let (inner, tail) = match extract_delimited(input, start, end) {
            Ok(parts) => parts,
            Err(message) => {
                return Err(Error::diagram_parse_insertion_point(
                    "kanban",
                    message,
                    input_start + input.len(),
                ));
            }
        };
        let label = parse_kanban_label(inner, inner_start);
        let closing_start = inner_start + inner.len();
        if !tail.trim().is_empty() {
            let tail_start = input_start + input.len() - tail.len();
            let leading = tail.len() - tail.trim_start().len();
            let trailing = tail.trim_end().len();
            let span = SourceSpan::new(tail_start + leading, tail_start + trailing);
            return Err(Error::diagram_parse_exact(
                "kanban",
                "unexpected trailing input",
                span,
            ));
        }
        let ty = node_type_for(start, end);
        return Ok(ParsedKanbanNode {
            spec: KanbanNodeSpec {
                id_raw: label.text.clone(),
                descr_raw: label.text.clone(),
                ty,
            },
            entity: matches!(editor_demand, EditorFactsDemand::Capture).then_some(label),
            label: None,
            span: SourceSpan::new(input_start, closing_start + end.len()),
        });
    }

    let (id_raw, rest) = split_node_id(input);
    let id = matches!(editor_demand, EditorFactsDemand::Capture).then(|| SpannedText {
        text: id_raw.to_string(),
        span: SourceSpan::new(input_start, input_start + id_raw.len()),
    });
    let rest = rest.trim_end();
    if rest.is_empty() {
        return Ok(ParsedKanbanNode {
            spec: KanbanNodeSpec {
                id_raw: id_raw.to_string(),
                descr_raw: id_raw.to_string(),
                ty: NODE_TYPE_DEFAULT,
            },
            entity: id,
            label: None,
            span: SourceSpan::new(input_start, input_start + input.len()),
        });
    }

    let Some((start, end)) = node_delimiter_pair_at_start(rest) else {
        return Err(Error::diagram_parse_insertion_point(
            "kanban",
            "expected node delimiter",
            input_start + id_raw.len(),
        ));
    };

    let rest_start = input_start + id_raw.len();
    let inner_start = rest_start + start.len();
    let (inner, tail) = match extract_delimited(rest, start, end) {
        Ok(parts) => parts,
        Err(message) => {
            return Err(Error::diagram_parse_insertion_point(
                "kanban",
                message,
                rest_start + rest.len(),
            ));
        }
    };
    let label = parse_kanban_label(inner, inner_start);
    let closing_start = inner_start + inner.len();
    if !tail.trim().is_empty() {
        let tail_start = rest_start + rest.len() - tail.len();
        let leading = tail.len() - tail.trim_start().len();
        let trailing = tail.trim_end().len();
        let span = SourceSpan::new(tail_start + leading, tail_start + trailing);
        return Err(Error::diagram_parse_exact(
            "kanban",
            "unexpected trailing input",
            span,
        ));
    }
    let ty = node_type_for(start, end);
    Ok(ParsedKanbanNode {
        spec: KanbanNodeSpec {
            id_raw: id_raw.to_string(),
            descr_raw: label.text.clone(),
            ty,
        },
        entity: id,
        label: matches!(editor_demand, EditorFactsDemand::Capture).then_some(label),
        span: SourceSpan::new(input_start, closing_start + end.len()),
    })
}

fn parse_kanban_label(raw: &str, raw_start: usize) -> SpannedText {
    let (text, span) = if let Some(raw) = raw.strip_prefix("\"`") {
        if let Some(raw) = raw.strip_suffix("`\"") {
            let closing_start = raw_start + 2 + raw.len();
            (
                raw.to_string(),
                SourceSpan::new(raw_start + 2, closing_start),
            )
        } else {
            (
                raw.to_string(),
                SourceSpan::new(raw_start + 2, raw_start + 2 + raw.len()),
            )
        }
    } else if let Some(raw) = raw.strip_prefix('"') {
        if let Some(raw) = raw.strip_suffix('"') {
            let closing_start = raw_start + 1 + raw.len();
            (
                raw.to_string(),
                SourceSpan::new(raw_start + 1, closing_start),
            )
        } else {
            (
                raw.to_string(),
                SourceSpan::new(raw_start + 1, raw_start + 1 + raw.len()),
            )
        }
    } else {
        (
            raw.to_string(),
            SourceSpan::new(raw_start, raw_start + raw.len()),
        )
    };
    SpannedText { text, span }
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

#[cfg(feature = "presentation")]
fn get_i64(cfg: &MermaidConfig, dotted_path: &str) -> Option<i64> {
    let mut cur = cfg.as_value();
    for segment in dotted_path.split('.') {
        cur = cur.as_object()?.get(segment)?;
    }
    cur.as_i64().or_else(|| cur.as_f64().map(|f| f as i64))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KanbanMetadataMode {
    Key,
    AfterKey,
    Value,
    AfterValue,
}

#[derive(Debug, Clone, Copy)]
struct KanbanMetadataQuote {
    delimiter: char,
    content_start: usize,
    is_key: bool,
}

struct KanbanMetadataCursor<'a> {
    source: &'a str,
    mode: KanbanMetadataMode,
    token_start: Option<usize>,
    current_key: Option<SpannedText>,
    quote: Option<KanbanMetadataQuote>,
    fields: Vec<KanbanMetadataField>,
}

impl<'a> KanbanMetadataCursor<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            mode: KanbanMetadataMode::Key,
            token_start: None,
            current_key: None,
            quote: None,
            fields: Vec::new(),
        }
    }

    fn consume(&mut self, offset: usize, ch: char) {
        if let Some(quote) = self.quote {
            if ch == quote.delimiter {
                let span = SourceSpan::new(quote.content_start, offset);
                if quote.is_key {
                    self.current_key = Some(SpannedText {
                        text: self.source[span.start..span.end].to_string(),
                        span,
                    });
                    self.mode = KanbanMetadataMode::AfterKey;
                } else {
                    self.finish_field(Some(span), true);
                    self.mode = KanbanMetadataMode::AfterValue;
                }
                self.quote = None;
            }
            return;
        }

        match self.mode {
            KanbanMetadataMode::Key => match ch {
                ':' => {
                    self.finish_key(offset);
                    self.mode = KanbanMetadataMode::Value;
                }
                ',' => self.finish_invalid_token(),
                '"' | '\'' if self.token_start.is_none() => {
                    self.quote = Some(KanbanMetadataQuote {
                        delimiter: ch,
                        content_start: offset + ch.len_utf8(),
                        is_key: true,
                    });
                }
                _ if ch.is_whitespace() && self.token_start.is_none() => {}
                _ => {
                    self.token_start.get_or_insert(offset);
                }
            },
            KanbanMetadataMode::AfterKey => match ch {
                ':' => {
                    self.mode = KanbanMetadataMode::Value;
                }
                _ if ch.is_whitespace() => {}
                _ => {
                    self.token_start = Some(offset);
                    self.mode = KanbanMetadataMode::Key;
                }
            },
            KanbanMetadataMode::Value => match ch {
                ',' => {
                    self.finish_bare_value(offset);
                    self.mode = KanbanMetadataMode::Key;
                }
                '"' | '\'' if self.token_start.is_none() => {
                    self.quote = Some(KanbanMetadataQuote {
                        delimiter: ch,
                        content_start: offset + ch.len_utf8(),
                        is_key: false,
                    });
                }
                _ if ch.is_whitespace() && self.token_start.is_none() => {}
                _ => {
                    self.token_start.get_or_insert(offset);
                }
            },
            KanbanMetadataMode::AfterValue => match ch {
                ',' => {
                    self.mode = KanbanMetadataMode::Key;
                }
                _ if ch.is_whitespace() => {}
                _ => {
                    self.token_start = Some(offset);
                    self.mode = KanbanMetadataMode::Key;
                }
            },
        }
    }

    fn line_break(&mut self, offset: usize) {
        if self.quote.is_some() {
            return;
        }
        match self.mode {
            KanbanMetadataMode::Value => self.finish_bare_value(offset),
            KanbanMetadataMode::Key => self.finish_invalid_token(),
            KanbanMetadataMode::AfterKey | KanbanMetadataMode::AfterValue => {}
        }
        if self.mode != KanbanMetadataMode::AfterKey {
            self.mode = KanbanMetadataMode::Key;
        }
    }

    fn close(&mut self, offset: usize) {
        if let Some(quote) = self.quote.take() {
            let span = SourceSpan::new(quote.content_start, offset);
            if !quote.is_key {
                self.finish_field(Some(span), true);
            }
        } else {
            match self.mode {
                KanbanMetadataMode::Value => self.finish_bare_value(offset),
                KanbanMetadataMode::Key => self.finish_invalid_token(),
                KanbanMetadataMode::AfterKey | KanbanMetadataMode::AfterValue => {}
            }
        }
    }

    fn finish_at_eof(&mut self, offset: usize) {
        if let Some(quote) = self.quote.take() {
            let span = SourceSpan::new(quote.content_start, offset);
            if !quote.is_key {
                self.finish_field(Some(span), true);
            }
            return;
        }
        match self.mode {
            KanbanMetadataMode::Value => self.finish_bare_value(offset),
            KanbanMetadataMode::Key => self.finish_invalid_token(),
            KanbanMetadataMode::AfterKey | KanbanMetadataMode::AfterValue => {}
        }
    }

    fn finish_key(&mut self, end: usize) {
        let Some(span) = self.trimmed_token_span(end) else {
            return;
        };
        self.current_key = Some(SpannedText {
            text: self.source[span.start..span.end].to_string(),
            span,
        });
    }

    fn finish_bare_value(&mut self, end: usize) {
        let span = self.trimmed_token_span(end);
        let is_string_value = span.is_some_and(|span| {
            let raw = &self.source[span.start..span.end];
            !matches!(raw, "true" | "false" | "null") && !is_kanban_inline_number(raw)
        });
        self.finish_field(span, is_string_value);
    }

    fn finish_invalid_token(&mut self) {
        self.token_start = None;
    }

    fn finish_field(&mut self, value_span: Option<SourceSpan>, is_string_value: bool) {
        let Some(key) = self.current_key.take() else {
            return;
        };
        let value = value_span.map(|span| SpannedText {
            text: self.source[span.start..span.end].to_string(),
            span,
        });
        self.fields.push(KanbanMetadataField {
            key,
            value,
            is_string_value,
        });
    }

    fn trimmed_token_span(&mut self, end: usize) -> Option<SourceSpan> {
        let start = self.token_start.take()?;
        let raw = &self.source[start..end];
        let leading = raw.len().saturating_sub(raw.trim_start().len());
        let trailing = raw.trim_end().len();
        (leading < trailing).then(|| SourceSpan::new(start + leading, start + trailing))
    }

    fn into_fields(self) -> Vec<KanbanMetadataField> {
        self.fields
    }
}

fn is_kanban_inline_number(raw: &str) -> bool {
    raw.as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'-')
        && matches!(
            serde_json::from_str::<Value>(raw),
            Ok(Value::Number(number)) if number.as_f64().is_some_and(f64::is_finite)
        )
}

fn consume_shape_data(
    lines: &mut KanbanLineCursor<'_>,
    first: &str,
    first_start: usize,
    editor_demand: EditorFactsDemand,
) -> Result<KanbanShapeData> {
    let Some(mut rest) = first.strip_prefix("@{") else {
        return Ok(KanbanShapeData {
            text: String::new(),
            span: SourceSpan::new(first_start, first_start),
            fields: Vec::new(),
        });
    };

    let mut out = String::new();
    let mut in_quote = false;
    let mut quoted = String::new();
    let block_start = first_start;
    let mut current_scan_start = first_start + "@{".len();
    let mut metadata = matches!(editor_demand, EditorFactsDemand::Capture)
        .then(|| KanbanMetadataCursor::new(lines.source));

    loop {
        let it = rest.char_indices().peekable();
        for (idx, ch) in it {
            let source_offset = current_scan_start + idx;
            if !in_quote && ch == '}' {
                let fields = if let Some(mut metadata) = metadata.take() {
                    metadata.close(source_offset);
                    metadata.into_fields()
                } else {
                    Vec::new()
                };
                let after_close = &rest[idx + ch.len_utf8()..];
                let visible_tail = strip_inline_comment(after_close);
                if !visible_tail.trim().is_empty() {
                    let leading = visible_tail.len() - visible_tail.trim_start().len();
                    let trailing = visible_tail.trim_end().len();
                    let tail_start = source_offset + ch.len_utf8();
                    let span = SourceSpan::new(tail_start + leading, tail_start + trailing);
                    return Err(Error::diagram_parse_exact(
                        "kanban",
                        "unexpected trailing input",
                        span,
                    ));
                }
                return Ok(KanbanShapeData {
                    text: out,
                    span: SourceSpan::new(block_start, source_offset + ch.len_utf8()),
                    fields,
                });
            }
            if let Some(metadata) = &mut metadata {
                metadata.consume(source_offset, ch);
            }
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

            out.push(ch);
        }

        if let Some(metadata) = &mut metadata {
            metadata.line_break(current_scan_start + rest.len());
        }

        let Some(next_line) = lines.next() else {
            if let Some(metadata) = &mut metadata {
                metadata.finish_at_eof(lines.offset());
            }
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
    editor_demand: EditorFactsDemand,
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
            let shape_data =
                consume_shape_data(lines, &rest[idx..], rest_start + idx, editor_demand)?;
            return Ok((node_part, Some(shape_data)));
        }
    }

    Ok((rest.trim_end().to_string(), None))
}

fn construct_kanban_semantic_source_controlled(
    code: &str,
    diagram_type: &str,
    config: &MermaidConfig,
    editor_demand: EditorFactsDemand,
    #[cfg(feature = "presentation")] presentation: Option<&KanbanPresentationPolicy>,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<std::result::Result<KanbanSemanticSource, KanbanParseFailure>> {
    control.checkpoint()?;
    #[cfg(test)]
    KANBAN_SYNTAX_CONSTRUCTION_COUNT.set(KANBAN_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

    let mut db = KanbanDb::default();
    let mut editor_facts = match editor_demand {
        EditorFactsDemand::Omit => None,
        EditorFactsDemand::Capture => {
            #[cfg(test)]
            crate::editor::test_work::record_collection();
            Some(EditorSemanticFacts::new())
        }
    };
    let mut first_failure = None;
    let mut lines = KanbanLineCursor::new(code);
    let header_tail = loop {
        control.checkpoint()?;
        let Some(line) = lines.next() else {
            let span = SourceSpan::new(lines.offset(), lines.offset());
            return Ok(Err(KanbanParseFailure {
                error: Box::new(Error::diagram_parse_insertion_point(
                    diagram_type,
                    "expected kanban header",
                    span.start,
                )),
                editor_facts,
                span,
            }));
        };
        let visible = strip_inline_comment(line.text);
        let visible_start = visible.len() - visible.trim_start().len();
        let visible_end = visible.trim_end().len();
        if visible_start >= visible_end {
            continue;
        }
        let trimmed = &visible[visible_start..visible_end];
        let trimmed_start = line.start + visible_start;
        let after_keyword = trimmed
            .get("kanban".len()..)
            .filter(|_| starts_with_case_insensitive(trimmed, "kanban"))
            .filter(|after| {
                after.is_empty() || after.chars().next().is_some_and(char::is_whitespace)
            });
        if let Some(after_keyword) = after_keyword {
            let keyword_end = trimmed_start + "kanban".len();
            if !after_keyword.trim().is_empty() {
                break Some((after_keyword.to_string(), keyword_end));
            }
            break None;
        }
        let span = SourceSpan::new(trimmed_start, trimmed_start + trimmed.len());
        return Ok(Err(KanbanParseFailure {
            error: Box::new(Error::diagram_parse_exact(
                diagram_type,
                "expected kanban header",
                span,
            )),
            editor_facts,
            span,
        }));
    };

    control.checkpoint()?;
    if let Some((tail, tail_start)) = &header_tail
        && let Err(error) = parse_kanban_statement(
            &mut lines,
            KanbanStatementContext {
                db: &mut db,
                facts: editor_facts.as_mut(),
                config,
                editor_demand,
                #[cfg(feature = "presentation")]
                presentation,
                control,
            },
            tail,
            *tail_start,
        )?
    {
        let fallback = SourceSpan::new(*tail_start, *tail_start + tail.len());
        record_kanban_failure(&mut first_failure, error, fallback);
    }
    control.checkpoint()?;
    while let Some(source_line) = lines.next() {
        control.checkpoint()?;
        if let Err(error) = parse_kanban_statement(
            &mut lines,
            KanbanStatementContext {
                db: &mut db,
                facts: editor_facts.as_mut(),
                config,
                editor_demand,
                #[cfg(feature = "presentation")]
                presentation,
                control,
            },
            source_line.text,
            source_line.start,
        )? {
            let fallback = SourceSpan::new(
                source_line.start,
                source_line.start + source_line.text.len(),
            );
            record_kanban_failure(&mut first_failure, error, fallback);
        }
    }
    if let Some((error, span)) = first_failure {
        Ok(Err(KanbanParseFailure {
            error: Box::new(error),
            editor_facts,
            span,
        }))
    } else {
        control.checkpoint()?;
        Ok(Ok(KanbanSemanticSource { db, editor_facts }))
    }
}

fn record_kanban_failure(
    first_failure: &mut Option<(Error, SourceSpan)>,
    error: Error,
    fallback: SourceSpan,
) {
    if first_failure.is_none() {
        let span = kanban_error_span(&error, fallback);
        *first_failure = Some((error, span));
    }
}

struct KanbanStatementContext<'a> {
    db: &'a mut KanbanDb,
    facts: Option<&'a mut EditorSemanticFacts>,
    config: &'a MermaidConfig,
    editor_demand: EditorFactsDemand,
    #[cfg(feature = "presentation")]
    presentation: Option<&'a KanbanPresentationPolicy>,
    control: &'a crate::OperationControl,
}

fn parse_kanban_statement(
    lines: &mut KanbanLineCursor<'_>,
    context: KanbanStatementContext<'_>,
    source: &str,
    source_start: usize,
) -> crate::OperationControlResult<Result<()>> {
    let KanbanStatementContext {
        db,
        mut facts,
        config,
        editor_demand,
        #[cfg(feature = "presentation")]
        presentation,
        control,
    } = context;
    control.checkpoint()?;
    let line = strip_inline_comment(source).trim_end();
    if line.trim().is_empty() {
        return Ok(Ok(()));
    }
    let (indent, rest) = split_indent(line);
    let rest = rest.trim_end();
    if rest.is_empty() {
        return Ok(Ok(()));
    }
    let rest_start = source_start + line.len() - rest.len();

    if starts_with_case_insensitive(rest, "::icon(") {
        let value_start = "::icon(".len();
        let suffix = &rest[value_start..];
        let close = suffix
            .char_indices()
            .find_map(|(offset, ch)| (ch == ')').then_some(offset));
        let raw_value = close.map_or(suffix, |end| &suffix[..end]);
        let leading = raw_value.len() - raw_value.trim_start().len();
        let trailing = raw_value.trim_end().len();
        let icon = SpannedText {
            text: raw_value[leading.min(trailing)..trailing].to_string(),
            span: SourceSpan::new(
                rest_start + value_start + leading.min(trailing),
                rest_start + value_start + trailing,
            ),
        };
        let Some(close) = close else {
            return Ok(Err(Error::diagram_parse_insertion_point(
                "kanban",
                "unterminated icon decoration",
                rest_start + rest.len(),
            )));
        };
        let close_start = rest_start + value_start + close;
        let after_close = &suffix[close + 1..];
        let visible_tail = strip_inline_comment(after_close);
        if !visible_tail.trim().is_empty() {
            let leading = visible_tail.len() - visible_tail.trim_start().len();
            let trailing = visible_tail.trim_end().len();
            let tail_start = close_start + 1;
            let span = SourceSpan::new(tail_start + leading, tail_start + trailing);
            return Ok(Err(Error::diagram_parse_exact(
                "kanban",
                "unexpected trailing input",
                span,
            )));
        }
        if let Some(facts) = facts.as_deref_mut() {
            facts.push_directive_prefix("icon");
        }
        if icon.text.is_empty() {
            return Ok(Ok(()));
        }
        db.decorate_last(None, Some(icon.text.clone()), config);
        if let Some(facts) = facts.as_deref_mut() {
            facts.push_symbol(EditorSemanticSymbol::payload(
                icon.text,
                Some("kanban icon".to_string()),
                EditorSemanticKind::String,
                icon.span,
                icon.span,
            ));
        }
        return Ok(Ok(()));
    }

    if let Some(after) = rest.strip_prefix(":::") {
        let class_raw = after.trim();
        let leading = after.len() - after.trim_start().len();
        let class_name = SpannedText {
            text: class_raw.to_string(),
            span: SourceSpan::new(
                rest_start + 3 + leading,
                rest_start + 3 + leading + class_raw.len(),
            ),
        };
        db.decorate_last(Some(class_name.text.clone()), None, config);
        if !class_name.text.is_empty() {
            if let Some(facts) = facts.as_deref_mut() {
                facts.push_directive_prefix(":::");
            }
            if let Some(facts) = facts.as_deref_mut() {
                facts.push_symbol(EditorSemanticSymbol::payload(
                    class_name.text,
                    Some("kanban class".to_string()),
                    EditorSemanticKind::Class,
                    class_name.span,
                    class_name.span,
                ));
            }
        }
        return Ok(Ok(()));
    }

    let (node_part, shape_data) =
        match split_node_and_shape_data(lines, rest, rest_start, editor_demand) {
            Ok(parsed) => parsed,
            Err(error) => return Ok(Err(error)),
        };
    if node_part.trim().is_empty() {
        return Ok(Ok(()));
    }
    let parsed = match parse_node_spec_for_render(&node_part, rest_start, editor_demand) {
        Ok(parsed) => parsed,
        Err(error) => return Ok(Err(error)),
    };
    let fact_kind = kanban_node_editor_kind(parsed.spec.ty);
    if let Some(facts) = facts.as_deref_mut()
        && let Some(shape_data) = &shape_data
    {
        push_kanban_metadata_facts(facts, &shape_data.fields);
    }
    let shape_data = match shape_data {
        Some(shape_data) => {
            let document = match crate::inline_config::parse_mermaid_inline_object_controlled(
                &shape_data.text,
                control,
            )? {
                Ok(document) => document,
                Err(error) => {
                    return Ok(Err(Error::diagram_parse_exact(
                        "kanban",
                        error,
                        shape_data.span,
                    )));
                }
            };
            Some((
                shape_data,
                crate::json_value::ManagedJson::from_value(document),
            ))
        }
        None => None,
    };
    #[cfg(feature = "presentation")]
    if let Some(presentation) = presentation
        && let Err(error) = presentation.validate_node(parsed.spec.ty, parsed.span)
    {
        return Ok(Err(error));
    }
    if let Err(error) = db.add_node(indent, parsed.spec, parsed.span, shape_data, config) {
        return Ok(Err(error));
    }
    if let Some(facts) = facts {
        let entity = parsed.entity.expect("captured node entity");
        let is_section = db.nodes.last().is_some_and(|node| node.parent_id.is_none());
        if is_section {
            facts.push_symbol(EditorSemanticSymbol::outline(
                entity.text,
                Some("kanban section".to_string()),
                EditorSemanticKind::Namespace,
                entity.span,
                entity.span,
            ));
        } else {
            facts.push_symbol(EditorSemanticSymbol::new(
                entity.text,
                Some("kanban item".to_string()),
                fact_kind,
                entity.span,
                entity.span,
            ));
        }
        if let Some(label) = parsed.label {
            facts.push_symbol(EditorSemanticSymbol::payload(
                label.text,
                Some("kanban label".to_string()),
                EditorSemanticKind::String,
                label.span,
                label.span,
            ));
        }
    }
    control.checkpoint()?;
    Ok(Ok(()))
}

fn push_kanban_metadata_facts(facts: &mut EditorSemanticFacts, fields: &[KanbanMetadataField]) {
    for field in fields {
        facts.push_symbol(EditorSemanticSymbol::payload(
            field.key.text.clone(),
            Some("kanban metadata key".to_string()),
            EditorSemanticKind::Property,
            field.key.span,
            field.key.span,
        ));
        if field.is_string_value
            && let Some(value) = &field.value
        {
            facts.push_symbol(EditorSemanticSymbol::payload(
                value.text.clone(),
                Some(format!("kanban metadata {}", field.key.text)),
                EditorSemanticKind::String,
                value.span,
                value.span,
            ));
        }
    }
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

pub(crate) fn parse_kanban_semantic_model_controlled(
    code: &str,
    metadata: &crate::SemanticMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<KanbanDiagramModel>> {
    Ok(construct_kanban_semantic_source_controlled(
        code,
        &metadata.diagram_type,
        &metadata.semantic_config,
        EditorFactsDemand::Omit,
        #[cfg(feature = "presentation")]
        None,
        control,
    )?
    .map(|source| source.db.into_semantic_model(&metadata.semantic_config))
    .map_err(|failure| *failure.error))
}

#[cfg(feature = "presentation")]
impl KanbanDiagramModel {
    fn into_render_model(self, config: &MermaidConfig) -> Result<KanbanDiagramRenderModel> {
        let width = get_i64(config, "mindmap.maxNodeWidth").unwrap_or(200);
        let base_padding = get_i64(config, "mindmap.padding").unwrap_or(10);
        let nodes = self
            .nodes
            .into_iter()
            .map(|node| {
                let padding = if matches!(
                    node.node_type,
                    NODE_TYPE_ROUNDED_RECT | NODE_TYPE_RECT | NODE_TYPE_HEXAGON
                ) {
                    base_padding.checked_mul(2).ok_or_else(|| {
                        Error::diagram_parse_exact(
                            "kanban",
                            "node padding exceeds the supported integer range",
                            node.span,
                        )
                    })?
                } else {
                    base_padding
                };
                Ok(KanbanRenderNode {
                    id: node.id,
                    label: node.label,
                    is_group: node.is_group,
                    parent_id: node.parent_id,
                    ticket: node.ticket.clone(),
                    priority: node.priority.clone(),
                    assigned: node.assigned.clone(),
                    icon: node.icon.clone(),
                    compatibility: KanbanRenderNodeCompatibility {
                        level: node.level,
                        width,
                        padding,
                        ticket: node.ticket,
                        priority: node.priority,
                        assigned: node.assigned,
                        icon: node.icon,
                        css_classes: node.css_classes,
                        shape: node.shape,
                    },
                })
            })
            .collect::<Result<Vec<_>>>()?;
        // Mermaid emits each section followed by every card with that section ID. Repeated
        // authored section IDs therefore intentionally repeat their matching cards.
        let (sections, cards): (Vec<_>, Vec<_>) = nodes.into_iter().partition(|node| node.is_group);
        let mut nodes = Vec::new();
        for section in sections {
            let section_id = section.id.clone();
            nodes.push(section);
            nodes.extend(
                cards
                    .iter()
                    .filter(|card| card.parent_id.as_deref() == Some(section_id.as_str()))
                    .cloned(),
            );
        }
        Ok(KanbanDiagramRenderModel { nodes })
    }
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_kanban(code: &str, meta: &ParseMetadata) -> Result<Value> {
    let model =
        parse_kanban_model_for_render_controlled(code, meta, &crate::OperationControl::new())
            .expect("a private parse control cannot be cancelled")?;
    render_model_to_compat_json(&model, meta)
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_kanban_json_and_editor_facts(
    code: &str,
    meta: &ParseMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::CombinedSemanticParse> {
    control.checkpoint()?;
    let parsed = crate::family::CombinedSemanticParse::from_construction(
        construct_kanban_semantic_source_controlled(
            code,
            &meta.diagram_type,
            meta.effective_config.as_config(),
            EditorFactsDemand::Capture,
            Some(&KanbanPresentationPolicy::from_config(
                meta.effective_config.as_config(),
            )),
            control,
        )?,
        |source| {
            let model = source
                .db
                .into_semantic_model(meta.effective_config.as_config())
                .into_render_model(meta.effective_config.as_config())
                .and_then(|model| render_model_to_compat_json(&model, meta));
            (model, source.editor_facts.unwrap_or_default())
        },
        KanbanParseFailure::into_error_and_editor_facts,
    );
    control.checkpoint()?;
    Ok(parsed)
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &KanbanDiagramRenderModel,
    meta: &ParseMetadata,
) -> Result<Value> {
    let mut out = Map::with_capacity(6);
    out.insert("type".to_string(), Value::String(meta.diagram_type.clone()));
    out.insert("sections".to_string(), kanban_sections_to_json(model));
    out.insert(
        "nodes".to_string(),
        Value::Array(kanban_nodes_to_json(
            model,
            meta.effective_config.as_config(),
        )),
    );
    out.insert("edges".to_string(), Value::Array(Vec::new()));
    out.insert("other".to_string(), Value::Object(Map::new()));
    out.insert(
        "config".to_string(),
        crate::json_value::clone_value_nonrecursive(meta.effective_config.as_value()),
    );
    Ok(Value::Object(out))
}

#[cfg(feature = "compat-json")]
fn kanban_sections_to_json(model: &KanbanDiagramRenderModel) -> Value {
    Value::Array(
        model
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
            .collect(),
    )
}

#[cfg(feature = "compat-json")]
fn kanban_nodes_to_json(model: &KanbanDiagramRenderModel, config: &MermaidConfig) -> Vec<Value> {
    let look = config.get_str("look").unwrap_or("classic");
    model.nodes.iter().map(|node| {
        if node.is_group {
            json!({"id": node.id, "label": node.label, "isGroup": true, "ticket": node.ticket,
                "shape": "kanbanSection", "level": node.compatibility.level, "look": look})
        } else {
            json!({"id": node.id, "parentId": node.parent_id, "label": node.label, "isGroup": false,
                "ticket": node.ticket, "priority": node.priority, "assigned": node.assigned, "icon": node.icon,
                "shape": "kanbanItem", "level": node.compatibility.level, "rx": 5, "ry": 5,
                "cssStyles": ["text-align: left"]})
        }
    }).collect()
}

#[cfg(all(test, feature = "presentation"))]
pub(crate) fn parse_kanban_model_for_render(
    code: &str,
    meta: &ParseMetadata,
) -> Result<KanbanDiagramRenderModel> {
    parse_kanban_model_for_render_controlled(code, meta, &crate::OperationControl::new())
        .expect("a private parse control cannot be cancelled")
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_kanban_model_for_render_controlled(
    code: &str,
    meta: &ParseMetadata,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<KanbanDiagramRenderModel>> {
    let construction = construct_kanban_semantic_source_controlled(
        code,
        &meta.diagram_type,
        meta.effective_config.as_config(),
        EditorFactsDemand::Omit,
        Some(&KanbanPresentationPolicy::from_config(
            meta.effective_config.as_config(),
        )),
        control,
    )?;
    control.checkpoint()?;
    Ok(construction
        .map_err(|failure| *failure.error)
        .and_then(|source| {
            source
                .db
                .into_semantic_model(meta.effective_config.as_config())
                .into_render_model(meta.effective_config.as_config())
        }))
}

#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::*;
    use crate::{
        EditorSemanticCompleteness, Engine, MermaidConfig, ParseDiagnosticSpanKind, ParseMetadata,
        ParseOptions,
    };
    use futures::executor::block_on;

    fn parse(text: &str) -> crate::ManagedSemanticJson {
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
            effective_config: crate::presentation::PresentationConfig::empty_object(),
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
        meta.effective_config =
            crate::presentation::PresentationConfig::from_value(serde_json::json!({
                "look": "handDrawn",
                "theme": "forest"
            }));
        let expected_json = parse_kanban(text, &meta).unwrap();
        let expected_model = parse_kanban_model_for_render(text, &meta).unwrap();

        reset_kanban_syntax_construction_count();
        let (json, facts) = crate::family::test_support::into_result(
            parse_kanban_json_and_editor_facts(text, &meta, &crate::OperationControl::new()),
        )
        .unwrap();

        assert_eq!(kanban_syntax_construction_count(), 1);
        assert_eq!(json, expected_json);
        assert!(!facts.symbols.is_empty());
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
        let facts = crate::family::test_support::editor_facts(
            parse_kanban_json_and_editor_facts,
            text,
            &meta(),
        );

        assert_eq!(kanban_syntax_construction_count(), 1);
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "backlog"));
        assert_eq!(facts.diagnostics.len(), 1);
    }

    #[test]
    fn kanban_recovery_keeps_first_error_prefix_and_later_safe_statements() {
        let text = concat!(
            "kanban\r\n",
            "  todo[\"之前\"]\r\n",
            "    broken[\"Open 🤓\r\n",
            "    invalid[Valid] trailing\r\n",
            "    later[\"后来\"]@{ ticket: 42, active: false }\r\n",
        );
        let strict_diagnostic = parse_err(text);
        let broken_end = text.find("\r\n    invalid").unwrap();
        assert_eq!(
            strict_diagnostic.span(),
            Some(SourceSpan::new(broken_end, broken_end))
        );

        reset_kanban_syntax_construction_count();
        let facts = crate::family::test_support::editor_facts(
            parse_kanban_json_and_editor_facts,
            text,
            &meta(),
        );
        assert_eq!(kanban_syntax_construction_count(), 1);
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert_eq!(facts.diagnostics.len(), 1);
        assert_eq!(facts.diagnostics[0].span, strict_diagnostic.span());
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "todo"));
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "later"));
        assert!(!facts.symbols.iter().any(|symbol| symbol.name == "invalid"));
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
            .parse_editor_semantic_facts_with_type_sync("kanban", text)
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
            .parse_editor_semantic_facts_with_type_sync("kanban", text)
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
    fn empty_icon_decoration_is_an_upstream_compatible_noop() {
        let text = "kanban\n    root[The root]\n    ::icon()\n";
        let model = parse(text);
        let sections = sections(&model);
        assert!(sections[0].get("icon").is_none());

        let (_, facts) = crate::family::test_support::into_result(
            parse_kanban_json_and_editor_facts(text, &meta(), &crate::OperationControl::new()),
        )
        .unwrap();
        assert!(facts.directive_prefixes.iter().any(|value| value == "icon"));
        assert!(facts.symbols.iter().all(|symbol| symbol.name != "icon"));
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
        assert_eq!(sections[0]["label"].as_str().unwrap(), "line1<br>line2");
    }
}

#[cfg(test)]
mod semantic_tests {
    use super::*;

    fn metadata() -> crate::SemanticMetadata {
        crate::SemanticMetadata {
            diagram_type: "kanban".to_string(),
            config: MermaidConfig::empty_object(),
            semantic_config: MermaidConfig::empty_object(),
            title: None,
        }
    }

    #[test]
    fn semantic_cards_preserve_authored_metadata_without_editor_work() {
        reset_kanban_syntax_construction_count();
        crate::editor::test_work::reset();
        let model = parse_kanban_semantic_model_controlled(
            "kanban\n  todo[Todo]\n    card[Caption]@{ ticket: MC-1, priority: high, custom: [1, 2] }\n    ::icon(star)\n    :::highlight\n",
            &metadata(), &crate::OperationControl::new(),
        ).unwrap().unwrap();
        assert_eq!(model.nodes.len(), 2);
        assert!(model.nodes[0].is_group);
        let card = &model.nodes[1];
        assert_eq!(
            (
                card.id.as_str(),
                card.label.as_str(),
                card.parent_id.as_deref()
            ),
            ("card", "Caption", Some("todo"))
        );
        assert_eq!(card.ticket.as_deref(), Some("MC-1"));
        assert_eq!(card.icon.as_deref(), Some("star"));
        assert_eq!(card.css_classes.as_deref(), Some("highlight"));
        assert_eq!(
            card.metadata.as_ref().unwrap()["custom"],
            serde_json::json!([1, 2])
        );
        assert_eq!(kanban_syntax_construction_count(), 1);
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }

    #[test]
    fn semantic_config_and_exact_errors_share_the_parser() {
        let mut metadata = metadata();
        metadata.semantic_config =
            MermaidConfig::from_value(serde_json::json!({"securityLevel": "strict"}));
        let model = parse_kanban_semantic_model_controlled(
            "kanban\n  a[<script>bad()</script>Safe]\n",
            &metadata,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(model.nodes[0].label, "Safe");
        let input = "kanban\n  broken[Open";
        let error = parse_kanban_semantic_model_controlled(
            input,
            &metadata,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = &error else {
            panic!("expected exact parser diagnostic")
        };
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(input.len(), input.len()))
        );
        #[cfg(feature = "presentation")]
        {
            let full = ParseMetadata {
                diagram_type: metadata.diagram_type,
                config: metadata.config,
                effective_config: crate::presentation::PresentationConfig::from_value(
                    crate::json_value::clone_value_nonrecursive(
                        metadata.semantic_config.as_value(),
                    ),
                ),
                title: None,
            };
            let full_error = parse_kanban_model_for_render_controlled(
                input,
                &full,
                &crate::OperationControl::new(),
            )
            .unwrap()
            .unwrap_err();
            assert_eq!(error.to_string(), full_error.to_string());
        }
    }

    #[test]
    fn semantic_cancellation_stops_before_construction() {
        reset_kanban_syntax_construction_count();
        let control = crate::OperationControl::new();
        control.cancel();
        assert!(
            parse_kanban_semantic_model_controlled("kanban\n  root", &metadata(), &control)
                .is_err()
        );
        assert_eq!(kanban_syntax_construction_count(), 0);
    }
    #[cfg(feature = "compat-json")]
    #[test]
    fn typed_and_json_paths_skip_editor_projection() {
        let full = ParseMetadata {
            diagram_type: "kanban".to_string(),
            config: crate::MermaidConfig::empty_object(),
            effective_config: crate::presentation::PresentationConfig::empty_object(),
            title: None,
        };
        crate::editor::test_work::reset();
        reset_kanban_syntax_construction_count();
        parse_kanban_model_for_render_controlled(
            "kanban\n  section\n    card",
            &full,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(kanban_syntax_construction_count(), 1);
        assert_eq!(crate::editor::test_work::read(), Default::default());
        reset_kanban_syntax_construction_count();
        parse_kanban("kanban\n  section\n    card", &full).unwrap();
        assert_eq!(kanban_syntax_construction_count(), 1);
        assert_eq!(crate::editor::test_work::read(), Default::default());
    }
    #[cfg(feature = "presentation")]
    #[test]
    fn canonical_cards_keep_source_identity_while_full_repeats_duplicate_section_groups() {
        let source = "kanban\n  same[First]\n    one[One]\n  same[Second]\n    two[Two]\n";
        let canonical = parse_kanban_semantic_model_controlled(
            source,
            &metadata(),
            &crate::OperationControl::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(canonical.nodes.len(), 4);
        let projected = canonical
            .into_render_model(&MermaidConfig::empty_object())
            .unwrap();
        let ids: Vec<_> = projected
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect();
        assert_eq!(ids, vec!["same", "one", "two", "same", "one", "two"]);
    }
    #[cfg(feature = "compat-json")]
    #[test]
    fn full_visual_validation_keeps_first_error_and_recovery_span() {
        let source = "kanban\n  section[Section]\n  broken[";
        let full = ParseMetadata {
            diagram_type: "kanban".to_string(),
            config: MermaidConfig::empty_object(),
            effective_config: crate::presentation::PresentationConfig::from_value(
                serde_json::json!({"mindmap": {"padding": i64::MAX}}),
            ),
            title: None,
        };
        let error = parse_kanban_model_for_render_controlled(
            source,
            &full,
            &crate::OperationControl::new(),
        )
        .unwrap()
        .unwrap_err();
        let Error::DiagramParse { diagnostic, .. } = &error else {
            panic!("expected parser diagnostic")
        };
        assert_eq!(
            diagnostic.message(),
            "node padding exceeds the supported integer range"
        );
        let start = source.find("section").unwrap();
        assert_eq!(
            diagnostic.span(),
            Some(SourceSpan::new(start, start + "section[Section]".len()))
        );
        let facts = crate::family::test_support::editor_facts(
            parse_kanban_json_and_editor_facts,
            source,
            &full,
        );
        assert_eq!(
            facts.completeness,
            crate::EditorSemanticCompleteness::Recovered
        );
        assert_eq!(facts.diagnostics[0].span, diagnostic.span());
    }
}
