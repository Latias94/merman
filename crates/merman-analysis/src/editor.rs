use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

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
    Operator,
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
    data: Arc<FenceTextIndexData>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct ReferenceIntervalId {
    group_ordinal: usize,
    span_ordinal: usize,
    semantic_item_id: usize,
}

#[derive(Debug)]
struct PointIntervalEntry<T> {
    span: ByteSpan,
    value: T,
    subtree_max_end: usize,
}

#[derive(Debug)]
struct PointIntervalIndex<T> {
    entries: Vec<PointIntervalEntry<T>>,
}

impl<T> Default for PointIntervalIndex<T> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
}

impl<T: Copy> PointIntervalIndex<T> {
    fn from_start_ordered(intervals: Vec<(ByteSpan, T)>) -> Self {
        debug_assert!(
            intervals
                .windows(2)
                .all(|pair| pair[0].0.start <= pair[1].0.start)
        );
        let mut entries = intervals
            .into_iter()
            .map(|(span, value)| PointIntervalEntry {
                span,
                value,
                subtree_max_end: span.end,
            })
            .collect::<Vec<_>>();
        populate_subtree_max_ends(&mut entries);
        Self { entries }
    }

    fn for_each_at(&self, offset: usize, mut visit: impl FnMut(T, ByteSpan)) -> usize {
        fn visit_subtree<T: Copy>(
            entries: &[PointIntervalEntry<T>],
            offset: usize,
            visited: &mut usize,
            visit: &mut impl FnMut(T, ByteSpan),
        ) {
            if entries.is_empty() {
                return;
            }

            let middle = entries.len() / 2;
            let (left, rest) = entries.split_at(middle);
            let (node, right) = rest
                .split_first()
                .expect("a non-empty interval subtree has a root");
            *visited += 1;
            if node.subtree_max_end < offset {
                return;
            }

            visit_subtree(left, offset, visited, visit);
            if node.span.start > offset {
                return;
            }
            if node.span.contains(offset) {
                visit(node.value, node.span);
            }
            visit_subtree(right, offset, visited, visit);
        }

        let mut visited = 0;
        visit_subtree(&self.entries, offset, &mut visited, &mut visit);
        visited
    }
}

fn populate_subtree_max_ends<T>(entries: &mut [PointIntervalEntry<T>]) -> Option<usize> {
    if entries.is_empty() {
        return None;
    }

    let middle = entries.len() / 2;
    let (left, rest) = entries.split_at_mut(middle);
    let (node, right) = rest
        .split_first_mut()
        .expect("a non-empty interval subtree has a root");
    let left_max = populate_subtree_max_ends(left);
    let right_max = populate_subtree_max_ends(right);
    node.subtree_max_end = left_max
        .into_iter()
        .chain(right_max)
        .fold(node.span.end, usize::max);
    Some(node.subtree_max_end)
}

#[derive(Debug, Default)]
pub(super) struct FenceTextIndexData {
    pub(super) node_ids: BTreeSet<String>,
    pub(super) class_names: BTreeSet<String>,
    pub(super) directive_prefixes: BTreeSet<String>,
    pub(super) references: BTreeMap<FenceReferenceGroup, Vec<ByteSpan>>,
    pub(super) outline_items: Vec<FenceLineItem>,
    pub(super) semantic_items: Vec<FenceSemanticItem>,
    pub(super) lexemes: Vec<FenceLexeme>,
    pub(super) lexeme_failure: Option<FenceLexemeFailure>,
    pub(super) expected_syntax: Vec<FenceExpectedSyntax>,
    pub(super) completion_vocabulary: merman_core::EditorCompletionVocabulary,
    pub(super) source: FenceTextIndexSource,
    semantic_point_index: PointIntervalIndex<usize>,
    reference_point_index: PointIntervalIndex<ReferenceIntervalId>,
}

impl FenceTextIndexData {
    pub(super) fn build_point_indexes(
        &mut self,
        cancellation: &crate::AnalysisCancellationToken,
    ) -> Result<(), crate::AnalysisCancelled> {
        let mut semantic_intervals = Vec::with_capacity(self.semantic_items.len());
        let mut reference_item_ids = BTreeMap::new();
        for (semantic_item_id, item) in self.semantic_items.iter().enumerate() {
            if semantic_item_id.is_multiple_of(128) {
                cancellation.checkpoint()?;
            }
            semantic_intervals.push((item.span, semantic_item_id));
            if item.role == FenceSemanticRole::Entity {
                let group = FenceReferenceGroup::from_semantic_item(item);
                if self.references.contains_key(&group) {
                    reference_item_ids.entry(group).or_insert(semantic_item_id);
                }
            }
        }

        let reference_count = self.references.values().map(Vec::len).sum();
        let mut reference_intervals = Vec::with_capacity(reference_count);
        let mut reference_index = 0usize;
        for (group_ordinal, (group, spans)) in self.references.iter().enumerate() {
            let semantic_item_id = *reference_item_ids
                .get(group)
                .expect("reference groups are derived from canonical semantic items");
            for (span_ordinal, span) in spans.iter().copied().enumerate() {
                if reference_index.is_multiple_of(128) {
                    cancellation.checkpoint()?;
                }
                reference_intervals.push((
                    span,
                    ReferenceIntervalId {
                        group_ordinal,
                        span_ordinal,
                        semantic_item_id,
                    },
                ));
                reference_index += 1;
            }
        }
        reference_intervals.sort_by(|(left_span, left_id), (right_span, right_id)| {
            (left_span.start, left_span.end, left_id).cmp(&(
                right_span.start,
                right_span.end,
                right_id,
            ))
        });
        cancellation.checkpoint()?;

        self.semantic_point_index = PointIntervalIndex::from_start_ordered(semantic_intervals);
        self.reference_point_index = PointIntervalIndex::from_start_ordered(reference_intervals);
        Ok(())
    }
}

impl FenceTextIndex {
    pub(super) fn from_data(data: FenceTextIndexData) -> Self {
        Self {
            data: Arc::new(data),
        }
    }

    #[cfg(test)]
    pub(crate) fn shares_storage_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.data, &other.data)
    }

    #[cfg(test)]
    pub(crate) fn from_core_facts(facts: merman_core::EditorSemanticFacts) -> Self {
        core_facts::from_core_facts(facts)
    }

    pub(crate) fn from_core_facts_cancellable(
        facts: &merman_core::EditorSemanticFacts,
        cancellation: &crate::AnalysisCancellationToken,
    ) -> Result<Self, crate::AnalysisCancelled> {
        core_facts::from_core_facts_cancellable(facts, cancellation)
    }

    pub fn node_ids(&self) -> impl Iterator<Item = &String> {
        self.data.node_ids.iter()
    }

    pub fn class_names(&self) -> impl Iterator<Item = &String> {
        self.data.class_names.iter()
    }

    pub fn directive_prefixes(&self) -> impl Iterator<Item = &String> {
        self.data.directive_prefixes.iter()
    }

    pub fn has_directive_prefix(&self, prefix: &str) -> bool {
        self.data.directive_prefixes.contains(prefix)
    }

    pub fn first_reference_span(&self, name: &str) -> Option<ByteSpan> {
        self.data
            .references
            .iter()
            .find(|(group, _)| group.name == name)
            .map(|(_, spans)| spans)
            .and_then(|spans| spans.first().copied())
    }

    pub fn reference_spans(&self, name: &str) -> &[ByteSpan] {
        self.data
            .references
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
        self.data
            .references
            .get(group)
            .and_then(|spans| spans.first().copied())
    }

    pub fn reference_spans_in_group(&self, group: &FenceReferenceGroup) -> &[ByteSpan] {
        self.data
            .references
            .get(group)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn references(&self) -> impl Iterator<Item = (&FenceReferenceGroup, &[ByteSpan])> {
        self.data
            .references
            .iter()
            .map(|(group, spans)| (group, spans.as_slice()))
    }

    pub fn symbol_at_offset(&self, offset: usize) -> Option<(String, ByteSpan)> {
        let (reference, _) = self.reference_at_offset_indexed(offset);
        let (reference, span) = reference?;
        let item = self.data.semantic_items.get(reference.semantic_item_id)?;
        Some((item.name.clone(), span))
    }

    pub fn semantic_item_at_offset(&self, offset: usize) -> Option<&FenceSemanticItem> {
        let (item_id, _) = self.semantic_item_id_at_offset_indexed(offset);
        self.data.semantic_items.get(item_id?)
    }

    pub fn entity_item_at_offset(&self, offset: usize) -> Option<&FenceSemanticItem> {
        self.semantic_item_at_offset(offset)
            .filter(|item| item.role == FenceSemanticRole::Entity)
    }

    pub fn outline_items(&self) -> &[FenceLineItem] {
        &self.data.outline_items
    }

    pub fn semantic_items(&self) -> &[FenceSemanticItem] {
        &self.data.semantic_items
    }

    pub fn lexemes(&self) -> &[FenceLexeme] {
        &self.data.lexemes
    }

    pub fn lexeme_failure(&self) -> Option<FenceLexemeFailure> {
        self.data.lexeme_failure
    }

    pub fn expected_syntax(&self) -> &[FenceExpectedSyntax] {
        &self.data.expected_syntax
    }

    pub fn completion_vocabulary(&self) -> merman_core::EditorCompletionVocabulary {
        self.data.completion_vocabulary
    }

    pub fn source(&self) -> FenceTextIndexSource {
        self.data.source
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

            if self.data.source.is_parser_backed()
                && offer_directive_items(&prefix, directive_prefix)
            {
                completion_kinds.push(FenceCursorCompletionKind::Directive);
            }
        }

        FenceCursorContext {
            prefix,
            prefix_start,
            cursor,
            source: self.data.source,
            source_start,
            directive_prefix,
            comment_or_directive_line,
            expected_syntax: expected_syntax_kind,
            expected_syntax_span,
            completion_kinds,
        }
    }

    fn expected_syntax_at_offset(&self, offset: usize) -> Option<&FenceExpectedSyntax> {
        self.data
            .expected_syntax
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

    fn semantic_item_id_at_offset_indexed(&self, offset: usize) -> (Option<usize>, usize) {
        let mut best = None;
        let visited = self
            .data
            .semantic_point_index
            .for_each_at(offset, |item_id, _| {
                if best
                    .is_none_or(|current| self.compare_semantic_item_ids(item_id, current).is_lt())
                {
                    best = Some(item_id);
                }
            });
        (best, visited)
    }

    fn compare_semantic_item_ids(&self, left_id: usize, right_id: usize) -> std::cmp::Ordering {
        let left = &self.data.semantic_items[left_id];
        let right = &self.data.semantic_items[right_id];
        let left_len = left.span.end.saturating_sub(left.span.start);
        let right_len = right.span.end.saturating_sub(right.span.start);
        (
            left_len,
            left.selection.start,
            left.selection.end,
            left.name.as_str(),
            left_id,
        )
            .cmp(&(
                right_len,
                right.selection.start,
                right.selection.end,
                right.name.as_str(),
                right_id,
            ))
    }

    fn reference_at_offset_indexed(
        &self,
        offset: usize,
    ) -> (Option<(ReferenceIntervalId, ByteSpan)>, usize) {
        let mut best = None;
        let visited = self
            .data
            .reference_point_index
            .for_each_at(offset, |reference, span| {
                if best
                    .as_ref()
                    .is_none_or(|(current, _)| reference < *current)
                {
                    best = Some((reference, span));
                }
            });
        (best, visited)
    }

    #[cfg(test)]
    fn semantic_item_id_at_offset_linear(&self, offset: usize) -> Option<usize> {
        (0..self.data.semantic_items.len())
            .filter(|item_id| self.data.semantic_items[*item_id].span.contains(offset))
            .min_by(|left_id, right_id| {
                let left = &self.data.semantic_items[*left_id];
                let right = &self.data.semantic_items[*right_id];
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

    #[cfg(test)]
    fn reference_at_offset_linear(&self, offset: usize) -> Option<(ReferenceIntervalId, ByteSpan)> {
        self.data
            .references
            .iter()
            .enumerate()
            .find_map(|(group_ordinal, (group, spans))| {
                let semantic_item_id = self.data.semantic_items.iter().position(|item| {
                    item.role == FenceSemanticRole::Entity
                        && item.name == group.name
                        && item.kind == group.kind
                })?;
                spans
                    .iter()
                    .copied()
                    .enumerate()
                    .find(|(_, span)| span.contains(offset))
                    .map(|(span_ordinal, span)| {
                        (
                            ReferenceIntervalId {
                                group_ordinal,
                                span_ordinal,
                                semantic_item_id,
                            },
                            span,
                        )
                    })
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
    let line_start = before
        .as_bytes()
        .iter()
        .rposition(|byte| matches!(byte, b'\n' | b'\r'))
        .map(|index| index + 1)
        .unwrap_or(0);
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

fn offer_directive_items(prefix: &str, directive_prefix: Option<&str>) -> bool {
    let prefix = prefix.trim_end();

    prefix.trim_start().starts_with("%%")
        || directive_prefix.is_some_and(|prefix| DIRECTIVE_HELPER_PREFIXES.contains(&prefix))
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
        FenceExpectedSyntaxKind::Operator => {
            completion_kinds.clear();
            completion_kinds.push(FenceCursorCompletionKind::Operator);
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
