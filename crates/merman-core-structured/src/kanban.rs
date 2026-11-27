//! Kanban syntax, hierarchy, and family model construction.

use merman_core_preprocess::scan::{starts_with_case_insensitive, strip_line_ending};
use merman_markup::sanitize::sanitize_text;
use merman_model::families::kanban::{
    KanbanDiagramModel, KanbanNodeModel, NODE_TYPE_BANG, NODE_TYPE_CIRCLE, NODE_TYPE_CLOUD,
    NODE_TYPE_DEFAULT, NODE_TYPE_HEXAGON, NODE_TYPE_RECT, NODE_TYPE_ROUNDED_RECT,
};
#[cfg(feature = "presentation")]
use merman_model::families::kanban::{
    KanbanDiagramRenderModel, KanbanRenderNode, KanbanRenderNodeCompatibility,
};
use merman_model::{
    ManagedJson, MermaidConfig, OperationControl, OperationControlResult, SourceSpan,
};
use serde_json::Value;
#[cfg(any(test, feature = "test-support"))]
use std::cell::Cell;
use std::collections::BTreeMap;

#[cfg(any(test, feature = "test-support"))]
thread_local! {
    static KANBAN_SYNTAX_CONSTRUCTION_COUNT: Cell<usize> = const { Cell::new(0) };
}
#[cfg(test)]
thread_local! {
    static KANBAN_NODE_SCAN_STEPS: Cell<usize> = const { Cell::new(0) };
}

#[cfg(any(test, feature = "test-support"))]
pub fn reset_kanban_syntax_construction_count() {
    KANBAN_SYNTAX_CONSTRUCTION_COUNT.set(0);
}

#[cfg(any(test, feature = "test-support"))]
pub fn kanban_syntax_construction_count() -> usize {
    KANBAN_SYNTAX_CONSTRUCTION_COUNT.get()
}

#[derive(Debug)]
pub enum KanbanSourceIssue {
    HeaderExact { span: SourceSpan },
    HeaderInsertionPoint { offset: usize },
    Exact { message: String, span: SourceSpan },
    InsertionPoint { message: String, offset: usize },
}

impl KanbanSourceIssue {
    fn exact(message: impl Into<String>, span: SourceSpan) -> Self {
        Self::Exact {
            message: message.into(),
            span,
        }
    }

    fn insertion_point(message: impl Into<String>, offset: usize) -> Self {
        Self::InsertionPoint {
            message: message.into(),
            offset,
        }
    }

    #[cfg(feature = "editor-facts")]
    pub fn span(&self) -> SourceSpan {
        match *self {
            Self::HeaderExact { span } | Self::Exact { span, .. } => span,
            Self::HeaderInsertionPoint { offset } | Self::InsertionPoint { offset, .. } => {
                SourceSpan::new(offset, offset)
            }
        }
    }
}

type SyntaxResult<T> = Result<T, KanbanSourceIssue>;
type CollectionText = BTreeMap<&'static str, String>;

#[cfg(feature = "editor-facts")]
pub enum KanbanObservation<'source> {
    CollectionStarted,
    Directive(&'static str),
    Field {
        kind: KanbanFieldKind<'source>,
        text: &'source str,
        span: SourceSpan,
    },
}

#[cfg(feature = "editor-facts")]
pub enum KanbanFieldKind<'source> {
    Section,
    Item { node_type: i32 },
    Label,
    Icon,
    Class,
    MetadataKey,
    MetadataValue { key: &'source str },
}

#[cfg(feature = "editor-facts")]
impl SpannedText {
    fn observation<'source>(
        &'source self,
        kind: KanbanFieldKind<'source>,
    ) -> KanbanObservation<'source> {
        KanbanObservation::Field {
            kind,
            text: &self.text,
            span: self.span,
        }
    }
}

/// The existing controlled decoder for a Kanban metadata block.
pub type KanbanInlineMetadataParser = fn(
    input: &str,
    collection_text_keys: &[&'static str],
    control: &OperationControl,
) -> OperationControlResult<
    Result<(ManagedJson, BTreeMap<&'static str, String>), String>,
>;

/// Configuration shared by source and presentation construction.
pub struct KanbanSourceOptions<'config> {
    config: &'config MermaidConfig,
    parse_inline_metadata: KanbanInlineMetadataParser,
    #[cfg(feature = "presentation")]
    presentation: Option<KanbanPresentationPolicy>,
}

/// A parsed board with its source order and authored metadata intact.
pub struct ParsedKanban<'config> {
    db: KanbanDb,
    config: &'config MermaidConfig,
}

impl ParsedKanban<'_> {
    pub fn into_semantic_model(self) -> KanbanDiagramModel {
        self.db.into_semantic_model(self.config)
    }
}

impl<'config> KanbanSourceOptions<'config> {
    pub fn new(
        config: &'config MermaidConfig,
        parse_inline_metadata: KanbanInlineMetadataParser,
    ) -> Self {
        Self {
            config,
            parse_inline_metadata,
            #[cfg(feature = "presentation")]
            presentation: None,
        }
    }

    #[cfg(feature = "presentation")]
    pub fn with_presentation(mut self) -> Self {
        self.presentation = Some(KanbanPresentationPolicy::from_config(self.config));
        self
    }

    pub fn parse_controlled(
        &self,
        code: &str,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ParsedKanban<'config>, KanbanSourceIssue>> {
        self.parse_impl(
            code,
            #[cfg(feature = "editor-facts")]
            None,
            control,
        )
    }

    #[cfg(feature = "editor-facts")]
    pub fn parse_with_observer_controlled(
        &self,
        code: &str,
        observe: &mut impl FnMut(KanbanObservation<'_>),
        control: &OperationControl,
    ) -> OperationControlResult<Result<ParsedKanban<'config>, KanbanSourceIssue>> {
        self.parse_impl(code, Some(observe), control)
    }

    fn parse_impl(
        &self,
        code: &str,
        #[cfg(feature = "editor-facts")] mut observe: Option<&mut dyn FnMut(KanbanObservation<'_>)>,
        control: &OperationControl,
    ) -> OperationControlResult<SyntaxResult<ParsedKanban<'config>>> {
        control.checkpoint()?;
        #[cfg(any(test, feature = "test-support"))]
        KANBAN_SYNTAX_CONSTRUCTION_COUNT.set(KANBAN_SYNTAX_CONSTRUCTION_COUNT.get() + 1);

        let mut db = KanbanDb::default();
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            observe(KanbanObservation::CollectionStarted);
        }
        let mut first_failure = None;
        let mut lines = KanbanLineCursor::new(code);
        let header_tail = loop {
            control.checkpoint()?;
            let Some(line) = lines.next() else {
                return Ok(Err(KanbanSourceIssue::HeaderInsertionPoint {
                    offset: lines.offset(),
                }));
            };
            let visible = line.text;
            let visible_start = visible.len() - visible.trim_start().len();
            let visible_end = visible.trim_end().len();
            if visible_start >= visible_end {
                continue;
            }
            let trimmed = &visible[visible_start..visible_end];
            if trimmed.starts_with("%%") {
                continue;
            }
            let trimmed_start = line.start + visible_start;
            let after_keyword = strip_kanban_keyword(trimmed);
            if let Some(after_keyword) = after_keyword {
                let keyword_end = trimmed_start + "kanban".len();
                if !after_keyword.trim().is_empty() {
                    break Some((
                        line.text[keyword_end - line.start..].to_string(),
                        keyword_end,
                    ));
                }
                break None;
            }
            let span = SourceSpan::new(trimmed_start, trimmed_start + trimmed.len());
            return Ok(Err(KanbanSourceIssue::HeaderExact { span }));
        };

        control.checkpoint()?;
        if let Some((tail, tail_start)) = &header_tail
            && let Err(issue) = parse_kanban_statement(
                &mut lines,
                KanbanStatementContext {
                    db: &mut db,
                    options: self,
                    control,
                },
                #[cfg(feature = "editor-facts")]
                &mut observe,
                tail,
                *tail_start,
            )?
        {
            first_failure.get_or_insert(issue);
        }
        control.checkpoint()?;
        while let Some(source_line) = lines.next() {
            control.checkpoint()?;
            if let Err(issue) = parse_kanban_statement(
                &mut lines,
                KanbanStatementContext {
                    db: &mut db,
                    options: self,
                    control,
                },
                #[cfg(feature = "editor-facts")]
                &mut observe,
                source_line.text,
                source_line.start,
            )? {
                first_failure.get_or_insert(issue);
            }
        }
        if let Some(issue) = first_failure {
            Ok(Err(issue))
        } else {
            control.checkpoint()?;
            Ok(Ok(ParsedKanban {
                db,
                config: self.config,
            }))
        }
    }
}

#[derive(Debug, Clone)]
struct KanbanNode {
    id: String,
    span: SourceSpan,
    level: usize,
    label: String,
    ty: i32,
    metadata: Option<ManagedJson>,
    parent_id: Option<String>,

    ticket: Option<String>,
    priority: Option<String>,
    assigned: Option<String>,
    icon: Option<String>,
    css_classes: Option<String>,
    shape: Option<String>,
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

    fn validate_node(&self, ty: i32, span: SourceSpan) -> SyntaxResult<()> {
        if matches!(
            ty,
            NODE_TYPE_ROUNDED_RECT | NODE_TYPE_RECT | NODE_TYPE_HEXAGON
        ) {
            self.padding.checked_mul(2).ok_or_else(|| {
                KanbanSourceIssue::exact("node padding exceeds the supported integer range", span)
            })?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct SpannedText {
    text: String,
    #[cfg(feature = "editor-facts")]
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
    #[cfg(feature = "editor-facts")]
    entity: Option<SpannedText>,
    #[cfg(feature = "editor-facts")]
    label: Option<SpannedText>,
    span: SourceSpan,
}

#[cfg(feature = "editor-facts")]
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
    #[cfg(feature = "editor-facts")]
    fields: Vec<KanbanMetadataField>,
}

struct ParsedKanbanShapeData {
    span: SourceSpan,
    document: ManagedJson,
    collection_text: CollectionText,
}

#[derive(Debug, Clone, Copy)]
struct KanbanSourceLine<'a> {
    text: &'a str,
    start: usize,
}

#[derive(Debug, Clone)]
struct KanbanLineCursor<'a> {
    source: &'a str,
    offset: usize,
    missing_markdown_terminator_from: Option<usize>,
}

impl<'a> KanbanLineCursor<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            offset: 0,
            missing_markdown_terminator_from: None,
        }
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
    fn get_section_index(&self, level: usize) -> SyntaxResult<Option<usize>> {
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
                return Err(KanbanSourceIssue::exact(
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
        shape_data: Option<ParsedKanbanShapeData>,
        config: &MermaidConfig,
    ) -> SyntaxResult<()> {
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

        if let Some(ParsedKanbanShapeData {
            span,
            document,
            collection_text,
        }) = shape_data
        {
            apply_shape_data(&mut node, document.as_value(), collection_text, span)?;
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

fn apply_shape_data(
    node: &mut KanbanNode,
    document: &Value,
    mut collection_text: CollectionText,
    span: SourceSpan,
) -> SyntaxResult<()> {
    let Some(obj) = document.as_object() else {
        return Ok(());
    };

    if let Some(Value::String(shape)) = obj.get("shape") {
        if shape != &shape.to_lowercase() || shape.contains('_') {
            return Err(KanbanSourceIssue::exact(
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
        node.icon = value_to_string(icon, collection_text.remove("icon"));
    }
    if let Some(assigned) = obj.get("assigned") {
        node.assigned = value_to_string(assigned, collection_text.remove("assigned"));
    }
    // Mermaid gates the YAML value before converting it to a string (kanbanDb.ts).
    if let Some(ticket) = obj.get("ticket").filter(|value| match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }) {
        node.ticket = value_to_string(ticket, collection_text.remove("ticket"));
    }
    if let Some(priority) = obj.get("priority") {
        node.priority = value_to_string(priority, collection_text.remove("priority"));
    }

    Ok(())
}

fn value_to_string(v: &Value, collection: Option<String>) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.as_f64().map_or_else(
            || n.to_string(),
            |value| ryu_js::Buffer::new().format(value).to_owned(),
        )),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => Some("null".to_string()),
        Value::Array(_) | Value::Object(_) => collection,
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

fn strip_kanban_keyword(input: &str) -> Option<&str> {
    input
        .get("kanban".len()..)
        .filter(|_| starts_with_case_insensitive(input, "kanban"))
        .filter(|after| {
            after
                .as_bytes()
                .first()
                .is_none_or(|byte| !byte.is_ascii_alphanumeric() && *byte != b'_')
        })
}

fn parse_node_spec_for_render(
    input: &str,
    input_start: usize,
    #[cfg(feature = "editor-facts")] capture: bool,
) -> SyntaxResult<ParsedKanbanNode> {
    let input = input.trim_end();
    if input.is_empty() {
        return Err(KanbanSourceIssue::insertion_point(
            "expected node",
            input_start,
        ));
    }

    if let Some((start, end)) = node_delimiter_pair_at_start(input) {
        let inner_start = input_start + start.len();
        let (inner, tail) = match extract_delimited(input, start, end) {
            Ok(parts) => parts,
            Err(message) => {
                return Err(KanbanSourceIssue::insertion_point(
                    message,
                    input_start + input.len(),
                ));
            }
        };
        let label = parse_kanban_label(
            inner,
            #[cfg(feature = "editor-facts")]
            inner_start,
        );
        let closing_start = inner_start + inner.len();
        if !tail.trim().is_empty() {
            let tail_start = input_start + input.len() - tail.len();
            let leading = tail.len() - tail.trim_start().len();
            let trailing = tail.trim_end().len();
            let span = SourceSpan::new(tail_start + leading, tail_start + trailing);
            return Err(KanbanSourceIssue::exact("unexpected trailing input", span));
        }
        let ty = node_type_for(start, end);
        return Ok(ParsedKanbanNode {
            spec: KanbanNodeSpec {
                id_raw: label.text.clone(),
                descr_raw: label.text.clone(),
                ty,
            },
            #[cfg(feature = "editor-facts")]
            entity: capture.then_some(label),
            #[cfg(feature = "editor-facts")]
            label: None,
            span: SourceSpan::new(input_start, closing_start + end.len()),
        });
    }

    let (id_raw, rest) = split_node_id(input);
    #[cfg(feature = "editor-facts")]
    let id = capture.then(|| SpannedText {
        text: id_raw.to_string(),
        #[cfg(feature = "editor-facts")]
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
            #[cfg(feature = "editor-facts")]
            entity: id,
            #[cfg(feature = "editor-facts")]
            label: None,
            span: SourceSpan::new(input_start, input_start + input.len()),
        });
    }

    let Some((start, end)) = node_delimiter_pair_at_start(rest) else {
        return Err(KanbanSourceIssue::insertion_point(
            "expected node delimiter",
            input_start + id_raw.len(),
        ));
    };

    let rest_start = input_start + id_raw.len();
    let inner_start = rest_start + start.len();
    let (inner, tail) = match extract_delimited(rest, start, end) {
        Ok(parts) => parts,
        Err(message) => {
            return Err(KanbanSourceIssue::insertion_point(
                message,
                rest_start + rest.len(),
            ));
        }
    };
    let label = parse_kanban_label(
        inner,
        #[cfg(feature = "editor-facts")]
        inner_start,
    );
    let closing_start = inner_start + inner.len();
    if !tail.trim().is_empty() {
        let tail_start = rest_start + rest.len() - tail.len();
        let leading = tail.len() - tail.trim_start().len();
        let trailing = tail.trim_end().len();
        let span = SourceSpan::new(tail_start + leading, tail_start + trailing);
        return Err(KanbanSourceIssue::exact("unexpected trailing input", span));
    }
    let ty = node_type_for(start, end);
    Ok(ParsedKanbanNode {
        spec: KanbanNodeSpec {
            id_raw: id_raw.to_string(),
            descr_raw: label.text.clone(),
            ty,
        },
        #[cfg(feature = "editor-facts")]
        entity: id,
        #[cfg(feature = "editor-facts")]
        label: capture.then_some(label),
        span: SourceSpan::new(input_start, closing_start + end.len()),
    })
}

fn parse_kanban_label(raw: &str, #[cfg(feature = "editor-facts")] raw_start: usize) -> SpannedText {
    if let Some(raw) = raw.strip_prefix("\"`") {
        let raw = raw.strip_suffix("`\"").unwrap_or(raw);
        return SpannedText {
            text: raw.to_string(),
            #[cfg(feature = "editor-facts")]
            span: SourceSpan::new(raw_start + 2, raw_start + 2 + raw.len()),
        };
    }
    if let Some(raw) = raw.strip_prefix('"') {
        let raw = raw.strip_suffix('"').unwrap_or(raw);
        return SpannedText {
            text: raw.to_string(),
            #[cfg(feature = "editor-facts")]
            span: SourceSpan::new(raw_start + 1, raw_start + 1 + raw.len()),
        };
    }
    SpannedText {
        text: raw.to_string(),
        #[cfg(feature = "editor-facts")]
        span: SourceSpan::new(raw_start, raw_start + raw.len()),
    }
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

#[cfg(feature = "editor-facts")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KanbanMetadataMode {
    Key,
    AfterKey,
    Value,
    AfterValue,
}

#[cfg(feature = "editor-facts")]
#[derive(Debug, Clone, Copy)]
struct KanbanMetadataQuote {
    delimiter: char,
    content_start: usize,
    is_key: bool,
}

#[cfg(feature = "editor-facts")]
struct KanbanMetadataCursor<'a> {
    source: &'a str,
    mode: KanbanMetadataMode,
    token_start: Option<usize>,
    current_key: Option<SpannedText>,
    quote: Option<KanbanMetadataQuote>,
    #[cfg(feature = "editor-facts")]
    fields: Vec<KanbanMetadataField>,
}

#[cfg(feature = "editor-facts")]
impl<'a> KanbanMetadataCursor<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            mode: KanbanMetadataMode::Key,
            token_start: None,
            current_key: None,
            quote: None,
            #[cfg(feature = "editor-facts")]
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
                        #[cfg(feature = "editor-facts")]
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
            #[cfg(feature = "editor-facts")]
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
            #[cfg(feature = "editor-facts")]
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

#[cfg(feature = "editor-facts")]
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
    #[cfg(feature = "editor-facts")] capture: bool,
    control: &OperationControl,
) -> OperationControlResult<SyntaxResult<KanbanShapeData>> {
    let Some(mut rest) = first.strip_prefix("@{") else {
        return Ok(Ok(KanbanShapeData {
            text: String::new(),
            span: SourceSpan::new(first_start, first_start),
            #[cfg(feature = "editor-facts")]
            fields: Vec::new(),
        }));
    };

    let mut out = String::new();
    let mut in_quote = false;
    let mut quoted = String::new();
    let block_start = first_start;
    let mut current_scan_start = first_start + "@{".len();
    #[cfg(feature = "editor-facts")]
    let mut metadata = capture.then(|| KanbanMetadataCursor::new(lines.source));

    loop {
        control.checkpoint()?;
        let it = rest.char_indices().peekable();
        for (step, (idx, ch)) in it.enumerate() {
            if step.is_multiple_of(4096) {
                control.checkpoint()?;
            }
            let source_offset = current_scan_start + idx;
            if !in_quote && ch == '}' {
                #[cfg(feature = "editor-facts")]
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
                    return Ok(Err(KanbanSourceIssue::exact(
                        "unexpected trailing input",
                        span,
                    )));
                }
                return Ok(Ok(KanbanShapeData {
                    text: out,
                    span: SourceSpan::new(block_start, source_offset + ch.len_utf8()),
                    #[cfg(feature = "editor-facts")]
                    fields,
                }));
            }
            #[cfg(feature = "editor-facts")]
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

        #[cfg(feature = "editor-facts")]
        if let Some(metadata) = &mut metadata {
            metadata.line_break(current_scan_start + rest.len());
        }

        let Some(next_line) = lines.next() else {
            #[cfg(feature = "editor-facts")]
            if let Some(metadata) = &mut metadata {
                metadata.finish_at_eof(lines.offset());
            }
            return Ok(Err(KanbanSourceIssue::insertion_point(
                "unterminated @{ ... } metadata block",
                lines.offset(),
            )));
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

#[derive(Clone, Copy)]
enum KanbanNodeScanMode {
    Id,
    Label,
    QuotedLabel,
    MarkdownLabel,
    Tail,
}

fn split_node_and_shape_data(
    lines: &mut KanbanLineCursor<'_>,
    rest: &str,
    rest_start: usize,
    #[cfg(feature = "editor-facts")] capture: bool,
    control: &OperationControl,
) -> OperationControlResult<SyntaxResult<(String, Option<KanbanShapeData>)>> {
    // Speculate across quoted physical lines. An unfinished label must leave later
    // statements available to recovery, just as an unfinished single-line label does.
    let mut continued_lines = lines.clone();
    let mut mode = KanbanNodeScanMode::Id;
    let mut after_quote = KanbanNodeScanMode::Id;
    let mut markdown_start = rest_start;
    let mut close = "";
    let mut current = KanbanSourceLine {
        text: rest,
        start: rest_start,
    };
    'scan: loop {
        control.checkpoint()?;
        let mut it = current.text.char_indices().peekable();
        let mut steps = 0usize;
        while let Some((idx, ch)) = it.next() {
            #[cfg(test)]
            KANBAN_NODE_SCAN_STEPS.set(KANBAN_NODE_SCAN_STEPS.get() + 1);
            if steps.is_multiple_of(4096) {
                control.checkpoint()?;
            }
            steps += 1;
            let suffix = &current.text[idx..];
            match mode {
                KanbanNodeScanMode::QuotedLabel => {
                    if ch == '"' {
                        mode = after_quote;
                    }
                }
                KanbanNodeScanMode::MarkdownLabel => {
                    if suffix.starts_with("`\"") {
                        it.next();
                        mode = after_quote;
                    }
                }
                _ if ch == '"' => {
                    after_quote = mode;
                    mode = if it.peek().is_some_and(|(_, next)| *next == '`') {
                        it.next();
                        markdown_start = current.start + idx + 2;
                        if lines
                            .missing_markdown_terminator_from
                            .is_some_and(|start| markdown_start >= start)
                        {
                            break 'scan;
                        }
                        KanbanNodeScanMode::MarkdownLabel
                    } else {
                        KanbanNodeScanMode::QuotedLabel
                    };
                }
                KanbanNodeScanMode::Label => {
                    if suffix.starts_with(close) {
                        for _ in 1..close.len() {
                            it.next();
                        }
                        mode = KanbanNodeScanMode::Tail;
                    }
                }
                KanbanNodeScanMode::Id | KanbanNodeScanMode::Tail => {
                    let offset = current.start + idx;
                    if suffix.starts_with("%%") {
                        *lines = continued_lines;
                        return Ok(Ok((
                            lines.source[rest_start..offset].trim_end().to_string(),
                            None,
                        )));
                    }
                    if suffix.starts_with("@{") {
                        *lines = continued_lines;
                        let node_part = lines.source[rest_start..offset].trim_end().to_string();
                        return Ok(consume_shape_data(
                            lines,
                            suffix,
                            offset,
                            #[cfg(feature = "editor-facts")]
                            capture,
                            control,
                        )?
                        .map(|shape_data| (node_part, Some(shape_data))));
                    }
                    if matches!(mode, KanbanNodeScanMode::Id)
                        && let Some((start, end)) = node_delimiter_pair_at_start(suffix)
                    {
                        for _ in 1..start.len() {
                            it.next();
                        }
                        close = end;
                        mode = KanbanNodeScanMode::Label;
                    }
                }
            }
        }
        match mode {
            KanbanNodeScanMode::QuotedLabel | KanbanNodeScanMode::MarkdownLabel
                if matches!(after_quote, KanbanNodeScanMode::Label) =>
            {
                let Some(next_line) = continued_lines.next() else {
                    if matches!(mode, KanbanNodeScanMode::MarkdownLabel) {
                        // Recovery keeps the real cursor in place. Reuse the failed
                        // terminator lookup for later quotes in this immutable suffix.
                        lines.missing_markdown_terminator_from = Some(markdown_start);
                    }
                    break;
                };
                current = next_line;
            }
            KanbanNodeScanMode::Id | KanbanNodeScanMode::Tail => {
                let end = current.start + current.text.len();
                *lines = continued_lines;
                return Ok(Ok((
                    lines.source[rest_start..end].trim_end().to_string(),
                    None,
                )));
            }
            KanbanNodeScanMode::Label
            | KanbanNodeScanMode::QuotedLabel
            | KanbanNodeScanMode::MarkdownLabel => break,
        }
    }

    Ok(Ok((rest.trim_end().to_string(), None)))
}

fn split_kanban_indent(line: &str) -> (usize, &str) {
    let mut total = 0;
    for (indent, (offset, ch)) in line.char_indices().enumerate() {
        if !ch.is_whitespace() {
            return (indent, &line[offset..]);
        }
        total = indent + 1;
    }
    (total, &line[line.len()..])
}

struct KanbanStatementContext<'a, 'config> {
    db: &'a mut KanbanDb,
    options: &'a KanbanSourceOptions<'config>,
    control: &'a OperationControl,
}

fn parse_kanban_statement(
    lines: &mut KanbanLineCursor<'_>,
    context: KanbanStatementContext<'_, '_>,
    #[cfg(feature = "editor-facts")] observe: &mut Option<&mut dyn FnMut(KanbanObservation<'_>)>,
    source: &str,
    source_start: usize,
) -> OperationControlResult<SyntaxResult<()>> {
    let KanbanStatementContext {
        db,
        options,
        control,
    } = context;
    let config = options.config;
    #[cfg(feature = "editor-facts")]
    let capture = observe.is_some();
    control.checkpoint()?;
    let line = source;
    if line.trim().is_empty() {
        return Ok(Ok(()));
    }
    let (indent, rest) = split_kanban_indent(line);
    if rest.trim().is_empty() || rest.starts_with("%%") {
        return Ok(Ok(()));
    }
    let rest_start = source_start + line.len() - rest.len();

    if strip_kanban_keyword(rest).is_some() {
        return Ok(Err(KanbanSourceIssue::exact(
            "unexpected kanban header in document",
            SourceSpan::new(rest_start, rest_start + "kanban".len()),
        )));
    }

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
            #[cfg(feature = "editor-facts")]
            span: SourceSpan::new(
                rest_start + value_start + leading.min(trailing),
                rest_start + value_start + trailing,
            ),
        };
        let Some(close) = close else {
            return Ok(Err(KanbanSourceIssue::insertion_point(
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
            return Ok(Err(KanbanSourceIssue::exact(
                "unexpected trailing input",
                span,
            )));
        }
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe.as_deref_mut() {
            observe(KanbanObservation::Directive("icon"));
        }
        if icon.text.is_empty() {
            return Ok(Ok(()));
        }
        db.decorate_last(None, Some(icon.text.clone()), config);
        #[cfg(feature = "editor-facts")]
        if let Some(observe) = observe {
            observe(icon.observation(KanbanFieldKind::Icon));
        }
        return Ok(Ok(()));
    }

    if let Some(after) = rest.strip_prefix(":::") {
        let after = strip_inline_comment(after);
        let class_raw = after.trim();
        #[cfg(feature = "editor-facts")]
        let leading = after.len() - after.trim_start().len();
        let class_name = SpannedText {
            text: class_raw.to_string(),
            #[cfg(feature = "editor-facts")]
            span: SourceSpan::new(
                rest_start + 3 + leading,
                rest_start + 3 + leading + class_raw.len(),
            ),
        };
        db.decorate_last(Some(class_name.text.clone()), None, config);
        #[cfg(feature = "editor-facts")]
        if !class_name.text.is_empty()
            && let Some(observe) = observe
        {
            observe(KanbanObservation::Directive(":::"));
            observe(class_name.observation(KanbanFieldKind::Class));
        }
        return Ok(Ok(()));
    }

    let (node_part, shape_data) = match split_node_and_shape_data(
        lines,
        rest,
        rest_start,
        #[cfg(feature = "editor-facts")]
        capture,
        control,
    )? {
        Ok(parsed) => parsed,
        Err(error) => return Ok(Err(error)),
    };
    if node_part.trim().is_empty() {
        return Ok(Ok(()));
    }
    let parsed = match parse_node_spec_for_render(
        &node_part,
        rest_start,
        #[cfg(feature = "editor-facts")]
        capture,
    ) {
        Ok(parsed) => parsed,
        Err(error) => return Ok(Err(error)),
    };
    #[cfg(feature = "editor-facts")]
    let node_type = parsed.spec.ty;
    #[cfg(feature = "editor-facts")]
    if let Some(observe) = observe.as_deref_mut()
        && let Some(shape_data) = &shape_data
    {
        for field in &shape_data.fields {
            observe(field.key.observation(KanbanFieldKind::MetadataKey));
            if field.is_string_value
                && let Some(value) = &field.value
            {
                observe(value.observation(KanbanFieldKind::MetadataValue {
                    key: &field.key.text,
                }));
            }
        }
    }
    let shape_data = match shape_data {
        Some(shape_data) => {
            let parsed_metadata = (options.parse_inline_metadata)(
                &shape_data.text,
                &["icon", "assigned", "ticket", "priority"],
                control,
            )?;
            let (document, collection_text) = match parsed_metadata {
                Ok(data) => data,
                Err(error) => {
                    return Ok(Err(KanbanSourceIssue::exact(error, shape_data.span)));
                }
            };
            Some(ParsedKanbanShapeData {
                span: shape_data.span,
                document,
                collection_text,
            })
        }
        None => None,
    };
    #[cfg(feature = "presentation")]
    if let Some(presentation) = &options.presentation
        && let Err(error) = presentation.validate_node(parsed.spec.ty, parsed.span)
    {
        return Ok(Err(error));
    }
    if let Err(error) = db.add_node(indent, parsed.spec, parsed.span, shape_data, config) {
        return Ok(Err(error));
    }
    #[cfg(feature = "editor-facts")]
    if let Some(observe) = observe {
        let entity = parsed.entity.expect("captured node entity");
        let kind = if db.nodes.last().is_some_and(|node| node.parent_id.is_none()) {
            KanbanFieldKind::Section
        } else {
            KanbanFieldKind::Item { node_type }
        };
        observe(entity.observation(kind));
        if let Some(label) = parsed.label {
            observe(label.observation(KanbanFieldKind::Label));
        }
    }
    control.checkpoint()?;
    Ok(Ok(()))
}

#[cfg(feature = "presentation")]
pub fn project_kanban_render_model(
    model: KanbanDiagramModel,
    config: &MermaidConfig,
) -> Result<KanbanDiagramRenderModel, KanbanSourceIssue> {
    let width = get_i64(config, "mindmap.maxNodeWidth").unwrap_or(200);
    let base_padding = get_i64(config, "mindmap.padding").unwrap_or(10);
    let nodes = model
        .nodes
        .into_iter()
        .map(|node| {
            let padding = if matches!(
                node.node_type,
                NODE_TYPE_ROUNDED_RECT | NODE_TYPE_RECT | NODE_TYPE_HEXAGON
            ) {
                base_padding.checked_mul(2).ok_or_else(|| {
                    KanbanSourceIssue::exact(
                        "node padding exceeds the supported integer range",
                        node.span,
                    )
                })?
            } else {
                base_padding
            };
            let mut rendered = KanbanRenderNode::new(node.id, node.label);
            rendered.is_group = node.is_group;
            rendered.parent_id = node.parent_id;
            rendered.ticket = node.ticket.clone();
            rendered.priority = node.priority.clone();
            rendered.assigned = node.assigned.clone();
            rendered.icon = node.icon.clone();
            Ok(rendered.with_compatibility(KanbanRenderNodeCompatibility {
                level: node.level,
                width,
                padding,
                ticket: node.ticket,
                priority: node.priority,
                assigned: node.assigned,
                icon: node.icon,
                css_classes: node.css_classes,
                shape: node.shape,
            }))
        })
        .collect::<SyntaxResult<Vec<_>>>()?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn no_inline_metadata(
        _: &str,
        _: &[&'static str],
        _: &OperationControl,
    ) -> OperationControlResult<Result<(ManagedJson, CollectionText), String>> {
        panic!("fixture does not contain metadata")
    }

    #[test]
    fn invalid_node_syntax_does_not_call_the_inline_decoder() {
        let config = MermaidConfig::empty_object();
        let source = "kanban\n  section\n    invalid[Open @{ticket: 1}\n";
        let result = KanbanSourceOptions::new(&config, no_inline_metadata)
            .parse_controlled(source, &OperationControl::new())
            .unwrap();
        assert!(result.is_err());
    }

    #[cfg(feature = "editor-facts")]
    #[test]
    fn requested_metadata_facts_precede_decoder_failure_and_later_recovery() {
        fn reject(
            text: &str,
            keys: &[&'static str],
            control: &OperationControl,
        ) -> OperationControlResult<Result<(ManagedJson, CollectionText), String>> {
            control.checkpoint()?;
            assert_eq!(text.trim(), "ticket: \"X\"");
            assert_eq!(keys, ["icon", "assigned", "ticket", "priority"]);
            Ok(Err("decode rejected".into()))
        }

        let config = MermaidConfig::empty_object();
        let source = "kanban\n  section\n    card@{ticket: \"X\"}\n    later\n";
        let mut observed = Vec::new();
        let result = KanbanSourceOptions::new(&config, reject)
            .parse_with_observer_controlled(
                source,
                &mut |observation| {
                    if let KanbanObservation::Field { text, span, .. } = observation {
                        observed.push((text.to_owned(), span));
                    }
                },
                &OperationControl::new(),
            )
            .unwrap();
        let start = source.find("@{").unwrap();
        let end = source.find('}').unwrap() + 1;
        assert!(
            matches!(result, Err(KanbanSourceIssue::Exact { message, span })
            if message == "decode rejected" && span == SourceSpan::new(start, end))
        );
        assert_eq!(
            observed
                .iter()
                .map(|(text, _)| text.as_str())
                .collect::<Vec<_>>(),
            ["section", "ticket", "X", "later"]
        );
        for (text, span) in observed {
            assert_eq!(text, source[span.start..span.end]);
        }
    }

    #[test]
    fn semantic_unclosed_markdown_recovery_scans_only_linear_work() {
        for count in [8, 64, 512] {
            let source = format!(
                "kanban\n  todo\n{}    safe[Recovered]\n",
                "    broken[\"`unfinished\n".repeat(count),
            );
            KANBAN_NODE_SCAN_STEPS.set(0);
            let config = MermaidConfig::empty_object();
            let parsed = KanbanSourceOptions::new(&config, no_inline_metadata)
                .parse_controlled(&source, &OperationControl::new())
                .unwrap();
            assert!(parsed.is_err());
            let steps = KANBAN_NODE_SCAN_STEPS.get();
            assert!(steps > count, "the regression must exercise node scanning");
            assert!(
                steps <= 2 * source.len(),
                "{count} unclosed labels used {steps} scan steps for {} bytes",
                source.len(),
            );
        }
    }

    #[test]
    fn semantic_label_and_metadata_continuations_remain_cancellable() {
        for (source, checkpoints) in [
            ("card[\"First\nSecond\"]\n", 2),
            ("card@{ ticket: \"First\n  Second\" }\n", 4),
        ] {
            let mut lines = KanbanLineCursor::new(source);
            let first = lines.next().unwrap();
            let control = OperationControl::new().for_phase(merman_model::OperationPhase::Parse);
            control.cancel_after_checkpoints(checkpoints);
            let error = split_node_and_shape_data(
                &mut lines,
                first.text,
                first.start,
                #[cfg(feature = "editor-facts")]
                false,
                &control,
            )
            .unwrap_err();
            assert_eq!(error.phase, merman_model::OperationPhase::Parse);
            assert_eq!(control.checkpoint().unwrap_err(), error);
        }
        let source = format!("card[\"{}\"]\n", "x".repeat(16 * 1024));
        let mut lines = KanbanLineCursor::new(&source);
        let first = lines.next().unwrap();
        let control = OperationControl::new();
        control.cancel_after_checkpoints(2);
        assert!(
            split_node_and_shape_data(
                &mut lines,
                first.text,
                first.start,
                #[cfg(feature = "editor-facts")]
                false,
                &control,
            )
            .is_err()
        );
    }
}
