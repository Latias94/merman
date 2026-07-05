use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

mod core_facts;
mod text_scan;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByteSpan {
    pub start: usize,
    pub end: usize,
}

impl ByteSpan {
    pub fn contains(self, offset: usize) -> bool {
        if self.start == self.end {
            offset == self.start
        } else {
            offset >= self.start && offset < self.end
        }
    }

    pub fn contains_inclusive_end(self, offset: usize) -> bool {
        offset >= self.start && offset <= self.end
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditorSymbolKind {
    Class,
    Event,
    Function,
    Module,
    Namespace,
    Object,
    Package,
    Property,
    String,
    Struct,
    Variable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FenceLineItem {
    pub name: String,
    pub detail: Option<String>,
    pub kind: EditorSymbolKind,
    pub span: ByteSpan,
    pub selection: ByteSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FenceSemanticRole {
    Entity,
    Outline,
    Payload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FenceSemanticItem {
    pub name: String,
    pub detail: Option<String>,
    pub kind: EditorSymbolKind,
    pub role: FenceSemanticRole,
    pub span: ByteSpan,
    pub selection: ByteSpan,
}

impl FenceSemanticItem {
    fn to_line_item(&self) -> FenceLineItem {
        FenceLineItem {
            name: self.name.clone(),
            detail: self.detail.clone(),
            kind: self.kind,
            span: self.span,
            selection: self.selection,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FenceReferenceGroup {
    pub name: String,
    pub kind: EditorSymbolKind,
}

impl FenceReferenceGroup {
    pub fn new(name: impl Into<String>, kind: EditorSymbolKind) -> Self {
        Self {
            name: name.into(),
            kind,
        }
    }

    pub fn from_semantic_item(item: &FenceSemanticItem) -> Self {
        Self::new(item.name.clone(), item.kind)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FenceTextIndexSource {
    /// Legacy text scan used only when parser facts are unavailable.
    #[default]
    TextScan,
    /// Parser-backed facts from a complete family parse.
    ParserComplete,
    /// Parser-backed complete facts whose spans remain in parser-input coordinates.
    ParserCompleteDegradedSpans,
    /// Parser-backed facts from a recoverable partial parse.
    ParserRecovered,
    /// Parser-backed recovered facts whose spans remain in parser-input coordinates.
    ParserRecoveredDegradedSpans,
}

impl FenceTextIndexSource {
    pub fn is_parser_backed(self) -> bool {
        matches!(
            self,
            Self::ParserComplete
                | Self::ParserCompleteDegradedSpans
                | Self::ParserRecovered
                | Self::ParserRecoveredDegradedSpans
        )
    }

    pub fn is_text_scan(self) -> bool {
        matches!(self, Self::TextScan)
    }

    pub fn is_recovered(self) -> bool {
        matches!(
            self,
            Self::ParserRecovered | Self::ParserRecoveredDegradedSpans
        )
    }

    pub fn has_source_mapped_spans(self) -> bool {
        !matches!(
            self,
            Self::ParserCompleteDegradedSpans | Self::ParserRecoveredDegradedSpans
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FenceCursorCompletionKind {
    DiagramHeader,
    Operator,
    Directive,
    Direction,
    Shape,
    NodeIdentifier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FenceExpectedSyntaxKind {
    IdList,
    NodeIdentifier,
    Shape,
    ShapeTrigger,
    Direction,
    Payload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FenceExpectedSyntax {
    pub kind: FenceExpectedSyntaxKind,
    pub span: ByteSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FenceCursorContext {
    prefix: String,
    prefix_start: usize,
    cursor: usize,
    source: FenceTextIndexSource,
    source_start: bool,
    directive_prefix: Option<&'static str>,
    comment_or_directive_line: bool,
    expected_syntax: Option<FenceExpectedSyntaxKind>,
    expected_syntax_span: Option<ByteSpan>,
    completion_kinds: Vec<FenceCursorCompletionKind>,
}

impl FenceCursorContext {
    pub fn prefix(&self) -> &str {
        &self.prefix
    }

    pub fn prefix_start(&self) -> usize {
        self.prefix_start
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn source(&self) -> FenceTextIndexSource {
        self.source
    }

    pub fn has_parser_backed_facts(&self) -> bool {
        self.source.is_parser_backed()
    }

    pub fn is_source_start(&self) -> bool {
        self.source_start
    }

    pub fn directive_prefix(&self) -> Option<&'static str> {
        self.directive_prefix
    }

    pub fn is_comment_or_directive_line(&self) -> bool {
        self.comment_or_directive_line
    }

    pub fn expected_syntax(&self) -> Option<FenceExpectedSyntaxKind> {
        self.expected_syntax
    }

    pub fn expected_syntax_span(&self) -> Option<ByteSpan> {
        self.expected_syntax_span
    }

    pub fn completion_kinds(&self) -> &[FenceCursorCompletionKind] {
        &self.completion_kinds
    }

    pub fn offers(&self, kind: FenceCursorCompletionKind) -> bool {
        self.completion_kinds.contains(&kind)
    }
}

#[derive(Debug, Clone, Default)]
pub struct FenceTextIndex {
    node_ids: BTreeSet<String>,
    class_names: BTreeSet<String>,
    directive_prefixes: BTreeSet<String>,
    references: BTreeMap<FenceReferenceGroup, Vec<ByteSpan>>,
    outline_items: Vec<FenceLineItem>,
    semantic_items: Vec<FenceSemanticItem>,
    expected_syntax: Vec<FenceExpectedSyntax>,
    source: FenceTextIndexSource,
}

impl FenceTextIndex {
    pub fn from_text(text: &str, diagram_type: Option<&str>) -> Self {
        let mut index = Self::default();
        let mut relative_start = 0usize;

        for line in text.split_inclusive('\n') {
            let line_end = relative_start + line.len();
            let line_no_newline = line.strip_suffix('\n').unwrap_or(line);
            let trimmed = line_no_newline.trim_start();
            let leading = line_no_newline.len().saturating_sub(trimmed.len());
            let abs_start = relative_start + leading;
            let abs_end = line_end;

            index.record_line(diagram_type, line_no_newline, trimmed, abs_start, abs_end);
            relative_start = line_end;
        }

        if !text.ends_with('\n') && relative_start < text.len() {
            let line_no_newline = &text[relative_start..];
            let trimmed = line_no_newline.trim_start();
            let leading = line_no_newline.len().saturating_sub(trimmed.len());
            index.record_line(
                diagram_type,
                line_no_newline,
                trimmed,
                relative_start + leading,
                text.len(),
            );
        }

        index.outline_items.sort_by(|left, right| {
            (
                left.span.start,
                left.span.end,
                left.name.as_str(),
                left.selection.start,
                left.selection.end,
            )
                .cmp(&(
                    right.span.start,
                    right.span.end,
                    right.name.as_str(),
                    right.selection.start,
                    right.selection.end,
                ))
        });
        index.outline_items.dedup_by(|left, right| {
            left.span.start == right.span.start
                && left.span.end == right.span.end
                && left.name == right.name
        });

        index
    }

    pub fn from_core_facts(facts: merman_core::EditorSemanticFacts) -> Self {
        core_facts::from_core_facts(facts)
    }

    pub fn merge_text_scan_node_ids(&mut self, text: &str, diagram_type: Option<&str>) {
        let text_index = Self::from_text(text, diagram_type);
        self.node_ids.extend(text_index.node_ids);
    }

    pub fn node_ids(&self) -> impl Iterator<Item = &String> {
        self.node_ids.iter()
    }

    pub fn class_names(&self) -> impl Iterator<Item = &String> {
        self.class_names.iter()
    }

    pub fn directive_prefixes(&self) -> impl Iterator<Item = &String> {
        self.directive_prefixes.iter()
    }

    pub fn has_directive_prefix(&self, prefix: &str) -> bool {
        self.directive_prefixes.contains(prefix)
    }

    pub fn first_reference_span(&self, name: &str) -> Option<ByteSpan> {
        self.references
            .iter()
            .find(|(group, _)| group.name == name)
            .map(|(_, spans)| spans)
            .and_then(|spans| spans.first().copied())
    }

    pub fn reference_spans(&self, name: &str) -> &[ByteSpan] {
        self.references
            .iter()
            .find(|(group, _)| group.name == name)
            .map(|(_, spans)| spans.as_slice())
            .unwrap_or(&[])
    }

    pub fn first_reference_span_for_item(&self, item: &FenceSemanticItem) -> Option<ByteSpan> {
        self.first_reference_span_in_group(&FenceReferenceGroup::from_semantic_item(item))
    }

    pub fn reference_spans_for_item(&self, item: &FenceSemanticItem) -> &[ByteSpan] {
        self.reference_spans_in_group(&FenceReferenceGroup::from_semantic_item(item))
    }

    pub fn first_reference_span_in_group(&self, group: &FenceReferenceGroup) -> Option<ByteSpan> {
        self.references
            .get(group)
            .and_then(|spans| spans.first().copied())
    }

    pub fn reference_spans_in_group(&self, group: &FenceReferenceGroup) -> &[ByteSpan] {
        self.references.get(group).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn references(&self) -> impl Iterator<Item = (&FenceReferenceGroup, &[ByteSpan])> {
        self.references
            .iter()
            .map(|(group, spans)| (group, spans.as_slice()))
    }

    pub fn symbol_at_offset(&self, offset: usize) -> Option<(String, ByteSpan)> {
        self.references.iter().find_map(|(group, spans)| {
            spans
                .iter()
                .copied()
                .find(|span| span.contains(offset))
                .map(|span| (group.name.clone(), span))
        })
    }

    pub fn semantic_item_at_offset(&self, offset: usize) -> Option<&FenceSemanticItem> {
        self.semantic_items
            .iter()
            .filter(|item| item.span.contains(offset))
            .min_by(|left, right| {
                let left_len = left.span.end.saturating_sub(left.span.start);
                let right_len = right.span.end.saturating_sub(right.span.start);
                (
                    left_len,
                    left.selection.start,
                    left.selection.end,
                    left.name.as_str(),
                )
                    .cmp(&(
                        right_len,
                        right.selection.start,
                        right.selection.end,
                        right.name.as_str(),
                    ))
            })
    }

    pub fn entity_item_at_offset(&self, offset: usize) -> Option<&FenceSemanticItem> {
        self.semantic_item_at_offset(offset)
            .filter(|item| item.role == FenceSemanticRole::Entity)
    }

    pub fn outline_items(&self) -> &[FenceLineItem] {
        &self.outline_items
    }

    pub fn semantic_items(&self) -> &[FenceSemanticItem] {
        &self.semantic_items
    }

    pub fn expected_syntax(&self) -> &[FenceExpectedSyntax] {
        &self.expected_syntax
    }

    pub fn source(&self) -> FenceTextIndexSource {
        self.source
    }

    pub fn cursor_context(&self, text: &str, cursor_offset: usize) -> FenceCursorContext {
        let cursor = clamp_to_char_boundary(text, cursor_offset);
        let (prefix_start, prefix) = current_line_prefix(text, cursor);
        let directive_prefix = directive_prefix(&prefix);
        let comment_or_directive_line =
            prefix.trim_start().starts_with("%%") || directive_prefix.is_some();
        let mut completion_kinds = Vec::new();
        let source_start = is_source_start_context(text, prefix_start);
        let expected_syntax = self.expected_syntax_at_offset(cursor).copied();
        let expected_syntax_kind = expected_syntax.map(|expected| expected.kind);
        let expected_syntax_span = expected_syntax.map(|expected| expected.span);

        if let Some(expected_syntax) = expected_syntax_kind {
            apply_expected_syntax_to_completion(expected_syntax, &mut completion_kinds);
        } else {
            if offer_diagram_headers(source_start, &prefix) {
                completion_kinds.push(FenceCursorCompletionKind::DiagramHeader);
            }

            if self.source.is_parser_backed() {
                if offer_operator_items(&prefix) {
                    completion_kinds.push(FenceCursorCompletionKind::Operator);
                }
                if offer_direction_items(&prefix) {
                    completion_kinds.push(FenceCursorCompletionKind::Direction);
                }
                if offer_directive_items(&prefix, directive_prefix) {
                    completion_kinds.push(FenceCursorCompletionKind::Directive);
                }
                if offer_shape_items(&prefix) {
                    completion_kinds.push(FenceCursorCompletionKind::Shape);
                }
            }
        }

        FenceCursorContext {
            prefix,
            prefix_start,
            cursor,
            source: self.source,
            source_start,
            directive_prefix,
            comment_or_directive_line,
            expected_syntax: expected_syntax_kind,
            expected_syntax_span,
            completion_kinds,
        }
    }

    fn expected_syntax_at_offset(&self, offset: usize) -> Option<&FenceExpectedSyntax> {
        self.expected_syntax
            .iter()
            .filter(|expected| expected.span.contains_inclusive_end(offset))
            .min_by(|left, right| {
                let left_len = left.span.end.saturating_sub(left.span.start);
                let right_len = right.span.end.saturating_sub(right.span.start);
                (left_len, left.span.start, left.span.end).cmp(&(
                    right_len,
                    right.span.start,
                    right.span.end,
                ))
            })
    }

    fn record_line(
        &mut self,
        diagram_type: Option<&str>,
        line_no_newline: &str,
        trimmed: &str,
        abs_start: usize,
        abs_end: usize,
    ) {
        let directive_prefix = directive_prefix(line_no_newline);
        if let Some(prefix) = directive_prefix {
            self.directive_prefixes.insert(prefix.to_string());
            if is_payload_only_text_scan_prefix(prefix) {
                return;
            }
        }

        if directive_prefix.is_none_or(|prefix| !is_classify_only_text_scan_prefix(prefix)) {
            text_scan::collect_node_ids(diagram_type, line_no_newline, &mut self.node_ids);
        }

        if let Some(item) = text_scan::classify_line_item(diagram_type, trimmed, abs_start, abs_end)
        {
            if is_class_definition_detail(item.detail.as_deref()) {
                self.class_names.insert(item.name.clone());
            }
            self.references
                .entry(FenceReferenceGroup::new(item.name.clone(), item.kind))
                .or_default()
                .push(item.selection);
            self.outline_items.push(item);
        }
    }
}

fn clamp_to_char_boundary(text: &str, offset: usize) -> usize {
    let mut cursor = offset.min(text.len());
    while cursor > 0 && !text.is_char_boundary(cursor) {
        cursor -= 1;
    }
    cursor
}

fn current_line_prefix(text: &str, cursor: usize) -> (usize, String) {
    let before = &text[..cursor];
    let line_start = before.rfind('\n').map(|index| index + 1).unwrap_or(0);
    let raw_prefix = &before[line_start..];
    let trimmed = raw_prefix.trim_start();
    let prefix_start = line_start + raw_prefix.len().saturating_sub(trimmed.len());

    (prefix_start, trimmed.to_string())
}

fn is_source_start_context(text: &str, prefix_start: usize) -> bool {
    text[..prefix_start].trim().is_empty()
}

const DIRECTIVE_PREFIXES: &[&str] = &[
    "classDef",
    "class",
    "style",
    "cssClass",
    "linkStyle",
    "click",
    "link",
    "callback",
    "links",
    "properties",
    "details",
    "dateFormat",
    "inclusiveEndDates",
    "topAxis",
    "axisFormat",
    "tickInterval",
    "includes",
    "excludes",
    "todayMarker",
    "weekday",
    "weekend",
    "section",
    "accTitle",
    "accDescr",
    "accDescription",
    "title",
];

const DIRECTIVE_HELPER_PREFIXES: &[&str] = &[
    "classDef",
    "class",
    "style",
    "cssClass",
    "linkStyle",
    "click",
    "link",
    "callback",
    ":::",
];

const DIRECTIVE_CLASSIFY_ONLY_PREFIXES: &[&str] = &[
    "classDef",
    "class",
    "style",
    "linkStyle",
    "click",
    "section",
];

const PAYLOAD_ONLY_TEXT_SCAN_PREFIXES: &[&str] = &[
    "init",
    "initialize",
    "wrap",
    "cssClass",
    "link",
    "callback",
    "links",
    "properties",
    "details",
    "dateFormat",
    "inclusiveEndDates",
    "topAxis",
    "axisFormat",
    "tickInterval",
    "includes",
    "excludes",
    "todayMarker",
    "weekday",
    "weekend",
    "accTitle",
    "accDescr",
    "accDescription",
    "title",
    ":::",
];

fn offer_diagram_headers(source_start: bool, prefix: &str) -> bool {
    if !source_start {
        return false;
    }
    let prefix = prefix.trim_end();

    prefix.is_empty() || diagram_header_prefix_matches(prefix)
}

fn offer_operator_items(prefix: &str) -> bool {
    let prefix = prefix.trim_end();

    prefix.ends_with("--") || prefix.ends_with("->")
}

fn offer_directive_items(prefix: &str, directive_prefix: Option<&str>) -> bool {
    let prefix = prefix.trim_end();

    prefix.trim_start().starts_with("%%")
        || directive_prefix.is_some_and(|prefix| DIRECTIVE_HELPER_PREFIXES.contains(&prefix))
}

fn offer_direction_items(prefix: &str) -> bool {
    prefix.trim_end() == "direction"
}

fn offer_shape_items(prefix: &str) -> bool {
    let prefix = prefix.trim_end();

    prefix.contains("@{ shape:")
        || prefix.ends_with("((")
        || prefix.ends_with("{{")
        || prefix.ends_with('[')
        || prefix.ends_with("[/")
        || prefix.ends_with("[\\")
        || prefix.ends_with('>')
}

fn diagram_header_prefix_matches(prefix: &str) -> bool {
    let prefix = prefix.trim_end();
    if prefix.is_empty() {
        return false;
    }

    text_scan::diagram_header_facts()
        .iter()
        .any(|fact| fact.label.starts_with(prefix))
}

fn is_payload_only_text_scan_prefix(prefix: &str) -> bool {
    PAYLOAD_ONLY_TEXT_SCAN_PREFIXES.contains(&prefix)
}

fn is_classify_only_text_scan_prefix(prefix: &str) -> bool {
    DIRECTIVE_CLASSIFY_ONLY_PREFIXES.contains(&prefix)
}

fn is_class_definition_detail(detail: Option<&str>) -> bool {
    detail.is_some_and(|detail| detail.ends_with("class definition"))
}

fn apply_expected_syntax_to_completion(
    expected: FenceExpectedSyntaxKind,
    completion_kinds: &mut Vec<FenceCursorCompletionKind>,
) {
    match expected {
        FenceExpectedSyntaxKind::IdList => {
            completion_kinds.clear();
            completion_kinds.push(FenceCursorCompletionKind::NodeIdentifier);
        }
        FenceExpectedSyntaxKind::NodeIdentifier => {
            completion_kinds.clear();
            completion_kinds.push(FenceCursorCompletionKind::NodeIdentifier);
        }
        FenceExpectedSyntaxKind::Shape => {
            completion_kinds.clear();
            completion_kinds.push(FenceCursorCompletionKind::Shape);
        }
        FenceExpectedSyntaxKind::ShapeTrigger => {
            completion_kinds.clear();
            completion_kinds.push(FenceCursorCompletionKind::Shape);
        }
        FenceExpectedSyntaxKind::Direction => {
            completion_kinds.clear();
            completion_kinds.push(FenceCursorCompletionKind::Direction);
        }
        FenceExpectedSyntaxKind::Payload => completion_kinds.clear(),
    }
}

fn directive_prefix(line: &str) -> Option<&'static str> {
    let trimmed = line.trim_start();

    if let Some(rest) = trimmed.strip_prefix("%%{") {
        let name = rest
            .split(|ch: char| ch.is_whitespace() || matches!(ch, ':' | '}'))
            .next()
            .filter(|name| !name.is_empty())?;

        return matches!(name, "init" | "initialize" | "wrap").then_some(match name {
            "init" => "init",
            "initialize" => "initialize",
            "wrap" => "wrap",
            _ => unreachable!(),
        });
    }

    if trimmed.starts_with(":::") {
        return Some(":::");
    }

    for &prefix in DIRECTIVE_PREFIXES {
        if has_word_boundary(trimmed, prefix) {
            return Some(prefix);
        }
    }

    None
}

fn has_word_boundary(text: &str, prefix: &str) -> bool {
    text.strip_prefix(prefix).is_some_and(|rest| {
        rest.is_empty()
            || rest
                .chars()
                .next()
                .is_some_and(|ch| ch.is_whitespace() || matches!(ch, ':' | '{'))
    })
}

#[cfg(test)]
mod tests;
