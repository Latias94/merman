use std::collections::{BTreeMap, BTreeSet};
use std::mem::size_of;
use std::sync::Arc;

use crate::retained_weight::{
    ARC_ALLOCATION_OVERHEAD, RetainedWeight, conservative_btree_entry_bytes,
};
use merman_core::{
    EditorExpectedSyntax, EditorSemanticKind, EditorSemanticRole, EditorSemanticSymbol, SourceSpan,
};
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct FenceReferenceGroup {
    pub(crate) name: String,
    pub(crate) kind: EditorSemanticKind,
}

impl FenceReferenceGroup {
    pub(crate) fn from_semantic_item(item: &EditorSemanticSymbol) -> Self {
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
    pub(super) directive_prefixes: BTreeSet<String>,
    pub(super) references: BTreeMap<FenceReferenceGroup, Vec<ByteSpan>>,
    pub(super) semantic_items: Vec<EditorSemanticSymbol>,
    pub(super) lexemes: Vec<merman_core::EditorLexeme>,
    pub(super) lexeme_failure: Option<merman_core::EditorLexemeFailure>,
    pub(super) expected_syntax: Vec<EditorExpectedSyntax>,
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
            semantic_intervals.push((byte_span_from_source(item.span), semantic_item_id));
            if item.role == EditorSemanticRole::Entity {
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

    fn estimated_owned_heap_bytes(&self) -> usize {
        let mut weight = RetainedWeight::new(
            ARC_ALLOCATION_OVERHEAD.saturating_add(size_of::<FenceTextIndexData>()),
        );
        weight.add(
            self.directive_prefixes
                .len()
                .saturating_mul(conservative_btree_entry_bytes::<String, ()>()),
        );
        for value in &self.directive_prefixes {
            weight.add_string(value);
        }
        weight.add(
            self.references
                .len()
                .saturating_mul(conservative_btree_entry_bytes::<
                    FenceReferenceGroup,
                    Vec<ByteSpan>,
                >()),
        );
        for (group, spans) in &self.references {
            weight.add_string(&group.name);
            weight.add_array::<ByteSpan>(spans.capacity());
        }
        weight.add_array::<EditorSemanticSymbol>(self.semantic_items.capacity());
        for item in &self.semantic_items {
            weight.add_string(&item.name);
            weight.add_optional_string(&item.detail);
        }
        weight.add_array::<merman_core::EditorLexeme>(self.lexemes.capacity());
        weight.add_array::<EditorExpectedSyntax>(self.expected_syntax.capacity());
        weight.add_array::<PointIntervalEntry<usize>>(self.semantic_point_index.entries.capacity());
        weight.add_array::<PointIntervalEntry<ReferenceIntervalId>>(
            self.reference_point_index.entries.capacity(),
        );
        weight.finish()
    }
}

impl FenceTextIndex {
    pub(super) fn from_data(data: FenceTextIndexData) -> Self {
        Self {
            data: Arc::new(data),
        }
    }

    pub(crate) fn estimated_owned_heap_bytes(&self) -> usize {
        self.data.estimated_owned_heap_bytes()
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
        sorted_unique_semantic_names(&self.data.semantic_items, |role| {
            role == EditorSemanticRole::Entity
        })
    }

    pub fn class_names(&self) -> impl Iterator<Item = &String> {
        sorted_unique_semantic_names(&self.data.semantic_items, |role| {
            role == EditorSemanticRole::ClassDefinition
        })
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

    pub fn first_reference_span_for_item(&self, item: &EditorSemanticSymbol) -> Option<ByteSpan> {
        self.first_reference_span_in_group(&FenceReferenceGroup::from_semantic_item(item))
    }

    pub fn reference_spans_for_item(&self, item: &EditorSemanticSymbol) -> &[ByteSpan] {
        self.reference_spans_in_group(&FenceReferenceGroup::from_semantic_item(item))
    }

    pub(crate) fn first_reference_span_in_group(
        &self,
        group: &FenceReferenceGroup,
    ) -> Option<ByteSpan> {
        self.data
            .references
            .get(group)
            .and_then(|spans| spans.first().copied())
    }

    pub(crate) fn reference_spans_in_group(&self, group: &FenceReferenceGroup) -> &[ByteSpan] {
        self.data
            .references
            .get(group)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub(crate) fn references(&self) -> impl Iterator<Item = (&FenceReferenceGroup, &[ByteSpan])> {
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

    pub fn semantic_item_at_offset(&self, offset: usize) -> Option<&EditorSemanticSymbol> {
        let (item_id, _) = self.semantic_item_id_at_offset_indexed(offset);
        self.data.semantic_items.get(item_id?)
    }

    pub fn entity_item_at_offset(&self, offset: usize) -> Option<&EditorSemanticSymbol> {
        self.semantic_item_at_offset(offset)
            .filter(|item| item.role == EditorSemanticRole::Entity)
    }

    pub fn semantic_items(&self) -> &[EditorSemanticSymbol] {
        &self.data.semantic_items
    }

    pub fn lexemes(&self) -> &[merman_core::EditorLexeme] {
        &self.data.lexemes
    }

    pub fn lexeme_failure(&self) -> Option<merman_core::EditorLexemeFailure> {
        self.data.lexeme_failure
    }

    pub fn expected_syntax(&self) -> &[EditorExpectedSyntax] {
        &self.data.expected_syntax
    }

    pub fn completion_vocabulary(&self) -> merman_core::EditorCompletionVocabulary {
        self.data.completion_vocabulary
    }

    pub fn source(&self) -> FenceTextIndexSource {
        self.data.source
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
            .filter(|item_id| {
                byte_span_from_source(self.data.semantic_items[*item_id].span).contains(offset)
            })
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
                    item.role == EditorSemanticRole::Entity
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

fn byte_span_from_source(span: SourceSpan) -> ByteSpan {
    ByteSpan {
        start: span.start,
        end: span.end,
    }
}

fn sorted_unique_semantic_names(
    items: &[EditorSemanticSymbol],
    predicate: impl Fn(EditorSemanticRole) -> bool,
) -> std::vec::IntoIter<&String> {
    let mut names = items
        .iter()
        .filter(|item| predicate(item.role))
        .map(|item| &item.name)
        .collect::<Vec<_>>();
    names.sort_unstable();
    names.dedup();
    names.into_iter()
}

#[cfg(test)]
mod tests;
