use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

mod core_facts;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FenceLexemeKind {
    Keyword,
    Comment,
    Operator,
    Delimiter,
    Identifier,
    Number,
    Date,
    Duration,
    Boolean,
    String,
    Style,
    Color,
    Literal,
    Frontmatter,
    Directive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FenceLexemeModifier {
    Declaration,
    Definition,
    Reference,
    Readonly,
    Documentation,
    DefaultLibrary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FenceLexeme {
    pub kind: FenceLexemeKind,
    pub modifiers: Vec<FenceLexemeModifier>,
    pub span: ByteSpan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FenceLexemeFailure {
    InvalidSpan { span: ByteSpan },
    Overlap { left: ByteSpan, right: ByteSpan },
    InvalidProvenance,
    UnknownModifierBits { bits: u8 },
    DuplicateModifiers { bits: u8 },
}

pub use merman_core::EditorRenamePolicy as FenceRenamePolicy;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct FenceSemanticItem {
    pub name: String,
    pub detail: Option<String>,
    pub kind: EditorSymbolKind,
    pub role: FenceSemanticRole,
    pub rename_policy: FenceRenamePolicy,
    pub span: ByteSpan,
    pub selection: ByteSpan,
}

impl FenceSemanticItem {
    pub fn new(
        name: impl Into<String>,
        detail: Option<String>,
        kind: EditorSymbolKind,
        role: FenceSemanticRole,
        span: ByteSpan,
        selection: ByteSpan,
    ) -> Self {
        Self {
            name: name.into(),
            detail,
            kind,
            role,
            rename_policy: if role == FenceSemanticRole::Entity {
                FenceRenamePolicy::Identifier
            } else {
                FenceRenamePolicy::None
            },
            span,
            selection,
        }
    }

    pub fn with_rename_policy(mut self, rename_policy: FenceRenamePolicy) -> Self {
        self.rename_policy = rename_policy;
        self
    }

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
#[non_exhaustive]
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
        Self {
            name: item.name.clone(),
            kind: item.kind,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FenceTextIndexSource {
    /// No parser-backed body facts are available.
    #[default]
    Unavailable,
    /// Parser-backed facts from a complete family parse.
    ParserComplete,
    /// Parser-backed facts from a recoverable partial parse.
    ParserRecovered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShapeObjectValuePrefix {
    pub value_start: usize,
    pub has_separator_space: bool,
}

impl FenceTextIndexSource {
    pub fn is_parser_backed(self) -> bool {
        matches!(self, Self::ParserComplete | Self::ParserRecovered)
    }

    pub fn is_unavailable(self) -> bool {
        matches!(self, Self::Unavailable)
    }

    pub fn is_recovered(self) -> bool {
        matches!(self, Self::ParserRecovered)
    }

    pub fn has_source_mapped_spans(self) -> bool {
        self.is_parser_backed()
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
    lexemes: Vec<FenceLexeme>,
    lexeme_failure: Option<FenceLexemeFailure>,
    expected_syntax: Vec<FenceExpectedSyntax>,
    completion_dialect: merman_core::EditorCompletionDialect,
    source: FenceTextIndexSource,
}

impl FenceTextIndex {
    pub(crate) fn from_core_facts(facts: merman_core::EditorSemanticFacts) -> Self {
        core_facts::from_core_facts(facts)
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

    pub fn lexemes(&self) -> &[FenceLexeme] {
        &self.lexemes
    }

    pub fn lexeme_failure(&self) -> Option<FenceLexemeFailure> {
        self.lexeme_failure
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

            if self.source.is_parser_backed() && offer_directive_items(&prefix, directive_prefix) {
                completion_kinds.push(FenceCursorCompletionKind::Directive);
            }

            if self.completion_dialect == merman_core::EditorCompletionDialect::Flowchart {
                if offer_operator_items(&prefix) {
                    completion_kinds.push(FenceCursorCompletionKind::Operator);
                }
                if offer_direction_items(&prefix) {
                    completion_kinds.push(FenceCursorCompletionKind::Direction);
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

    shape_object_value_prefix(prefix).is_some()
        || prefix.ends_with("((")
        || prefix.ends_with("{{")
        || prefix.ends_with('[')
        || prefix.ends_with("[/")
        || prefix.ends_with("[\\")
        || prefix.ends_with('>')
}

pub fn shape_object_value_prefix(prefix: &str) -> Option<ShapeObjectValuePrefix> {
    let mut search_end = prefix.len();
    while let Some(marker) = prefix[..search_end].rfind("@{") {
        let next_search_end = marker;
        let mut offset = marker + "@{".len();
        offset += leading_whitespace_len(&prefix[offset..]);

        let tail = &prefix[offset..];
        if !tail.starts_with("shape") {
            search_end = next_search_end;
            continue;
        }
        let after_shape = offset + "shape".len();
        if prefix[after_shape..]
            .chars()
            .next()
            .is_some_and(is_shape_key_continue)
        {
            search_end = next_search_end;
            continue;
        }

        offset = after_shape;
        offset += leading_whitespace_len(&prefix[offset..]);
        if !prefix[offset..].starts_with(':') {
            search_end = next_search_end;
            continue;
        }

        offset += ':'.len_utf8();
        let whitespace = leading_whitespace_len(&prefix[offset..]);
        let value_start = offset + whitespace;
        if !shape_object_prefix_is_inside_shape_value(prefix, value_start) {
            search_end = next_search_end;
            continue;
        }
        return Some(ShapeObjectValuePrefix {
            value_start,
            has_separator_space: whitespace > 0,
        });
    }

    None
}

fn shape_object_prefix_is_inside_shape_value(prefix: &str, value_start: usize) -> bool {
    prefix[value_start..]
        .chars()
        .all(|ch| !matches!(ch, ',' | '}' | '\n' | '\r'))
}

fn leading_whitespace_len(input: &str) -> usize {
    input
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .map(char::len_utf8)
        .sum()
}

fn is_shape_key_continue(ch: char) -> bool {
    ch == '_' || ch == '-' || ch.is_ascii_alphanumeric()
}

fn diagram_header_prefix_matches(prefix: &str) -> bool {
    let prefix = prefix.trim_end();
    if prefix.is_empty() {
        return false;
    }

    merman_core::diagram_header_facts()
        .iter()
        .any(|fact| fact.label.starts_with(prefix))
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

    DIRECTIVE_PREFIXES
        .iter()
        .find(|&&prefix| has_word_boundary(trimmed, prefix))
        .copied()
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
