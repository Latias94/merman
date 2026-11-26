//! Controlled UTF-8 text edits and source-offset mapping for Mermaid preprocessing.

use merman_model::{OperationControl, OperationControlResult, SourceSpan};
use std::ops::Range;
use std::sync::OnceLock;

#[doc(hidden)]
pub mod directives;
#[doc(hidden)]
pub mod frontmatter;
#[doc(hidden)]
pub mod scan;
#[doc(hidden)]
pub mod text;

const CONTROLLED_EDIT_REBUILD_CHECKPOINT_BYTES: usize = 4 * 1024;
const CONTROLLED_EDIT_REBUILD_CHECKPOINT_ITEMS: usize = 128;

/// How a replacement's offsets map to its original range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplacementMapping {
    ExactBytes,
    Boundaries,
}

/// One edit in the current text's byte coordinates.
#[derive(Debug, Clone)]
pub struct SourceEdit {
    range: Range<usize>,
    replacement: &'static str,
    mapping: ReplacementMapping,
}

impl SourceEdit {
    #[inline]
    pub fn delete(range: Range<usize>) -> Self {
        Self {
            range,
            replacement: "",
            mapping: ReplacementMapping::Boundaries,
        }
    }

    #[inline]
    pub fn replace(
        range: Range<usize>,
        replacement: &'static str,
        mapping: ReplacementMapping,
    ) -> Self {
        Self {
            range,
            replacement,
            mapping,
        }
    }
}

/// Rewritten text with a composable map back to the original byte offsets.
#[derive(Debug, Clone)]
pub struct MappedSource {
    text: String,
    mapping: SourceMapping,
}

impl MappedSource {
    pub fn new_controlled(
        source: &str,
        capture_spans: bool,
        control: &OperationControl,
    ) -> OperationControlResult<Self> {
        control.checkpoint()?;
        let mut text = String::with_capacity(source.len());
        let mut chunk_start = 0usize;
        while source.len().saturating_sub(chunk_start) >= CONTROLLED_EDIT_REBUILD_CHECKPOINT_BYTES {
            let mut chunk_end = chunk_start + CONTROLLED_EDIT_REBUILD_CHECKPOINT_BYTES;
            while !source.is_char_boundary(chunk_end) {
                chunk_end += 1;
            }
            text.push_str(&source[chunk_start..chunk_end]);
            control.checkpoint()?;
            chunk_start = chunk_end;
        }
        text.push_str(&source[chunk_start..]);
        control.checkpoint()?;
        Ok(Self {
            text,
            mapping: if capture_spans {
                SourceMapping::Eager(None)
            } else {
                SourceMapping::Deferred(DeferredSourceMap::new(source.len()))
            },
        })
    }

    #[inline]
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn try_map_span(&self, span: SourceSpan) -> Option<SourceSpan> {
        if !self.is_text_span(span) {
            return None;
        }
        self.resolved_edit_map()
            .map_or(Some(span), |map| map.try_map_span(span))
    }

    fn is_text_span(&self, span: SourceSpan) -> bool {
        span.start <= span.end
            && self.text.is_char_boundary(span.start)
            && self.text.is_char_boundary(span.end)
    }

    fn resolved_edit_map(&self) -> Option<&SourceEditMap> {
        match &self.mapping {
            SourceMapping::Eager(map) => map.as_ref(),
            SourceMapping::Deferred(map) => map.resolve(),
        }
    }

    pub fn prepare_span_mapping_controlled(
        &self,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        if let SourceMapping::Deferred(map) = &self.mapping {
            map.resolve_controlled(control)?;
        }
        Ok(())
    }

    /// Checks whether edits can prevent exact mapping of a span in the current text.
    pub fn may_drop_spans_controlled(
        &self,
        control: &OperationControl,
    ) -> OperationControlResult<bool> {
        // Rich facts report a diagnostic when an exact span crosses an edit. Such inputs must
        // observe those spans during the same family construction even for a recovery capture.
        // Prefix/suffix deletions and one-byte boundary replacements (including CRLF) cannot
        // contain an interior UTF-8 boundary and do not need the richer observer.
        self.prepare_span_mapping_controlled(control)?;
        let Some(map) = self.resolved_edit_map() else {
            control.checkpoint()?;
            return Ok(false);
        };
        for (index, gap) in map.gaps.iter().enumerate() {
            if index.is_multiple_of(128) {
                control.checkpoint()?;
            }
            if gap.output_offset > 0 && gap.output_offset < map.output_len {
                return Ok(true);
            }
        }
        for (index, segment) in map.segments.iter().enumerate() {
            if index.is_multiple_of(128) {
                control.checkpoint()?;
            }
            if segment.mapping == SegmentMapping::Unmapped
                || (segment.mapping == SegmentMapping::Boundaries && segment.output.len() > 1)
            {
                return Ok(true);
            }
        }
        control.checkpoint()?;
        Ok(false)
    }

    /// Maps span endpoints and allows deleted source bytes between them.
    /// Use the result as provenance rather than as a rewrite target.
    pub fn try_map_enclosing_span(&self, span: SourceSpan) -> Option<SourceSpan> {
        if !self.is_text_span(span) {
            return None;
        }
        self.resolved_edit_map()
            .map_or(Some(span), |map| map.try_map_enclosing_span(span))
    }

    pub fn into_text(self) -> String {
        self.text
    }

    /// Applies edits atomically with respect to cancellation.
    ///
    /// # Panics
    ///
    /// Panics if edits are unsorted, overlap, or use offsets outside UTF-8 boundaries.
    pub fn apply_edits(
        &mut self,
        edits: Vec<SourceEdit>,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        let mut checkpoints = ControlledEditRebuildCheckpoints::new(control)?;
        if edits.is_empty() {
            return Ok(());
        }
        // Preprocessors emit edits in source order. Keeping that as an invariant avoids an
        // uninterruptible sort inside the controlled reconstruction path.
        assert!(
            edits_are_sorted_controlled(&edits, &mut checkpoints)?,
            "source edits must be sorted by range"
        );
        let (replacement_bytes, removed_bytes) =
            assert_valid_edits_controlled(&self.text, &edits, &mut checkpoints)?;

        // Borrow the published state until every checkpoint and invariant check succeeds. A
        // cancellation drops only these local builders and cannot expose a partial rewrite.
        let old_text = &self.text;
        let mut text = String::with_capacity(
            old_text
                .len()
                .saturating_sub(removed_bytes)
                .saturating_add(replacement_bytes),
        );
        let mut cursor = 0usize;

        for edit in &edits {
            checkpoints.processed_item()?;
            checkpoints.push_str(&mut text, &old_text[cursor..edit.range.start])?;
            checkpoints.push_str(&mut text, edit.replacement)?;
            cursor = edit.range.end;
        }

        checkpoints.push_str(&mut text, &old_text[cursor..])?;
        match &mut self.mapping {
            SourceMapping::Eager(map) => {
                let identity;
                let old_map = match map.as_ref() {
                    Some(map) => map,
                    None => {
                        identity = SourceEditMap::identity(old_text.len());
                        &identity
                    }
                };
                let composed = old_map.compose_batch(&edits, text.len(), &mut checkpoints)?;
                checkpoints.finish()?;
                *map = Some(composed);
            }
            SourceMapping::Deferred(map) => {
                checkpoints.finish()?;
                map.composed.take();
                map.batches.push(EditBatch {
                    edits,
                    output_len: text.len(),
                });
            }
        }

        self.text = text;
        Ok(())
    }

    #[cfg(test)]
    fn apply_edits_uncontrolled(&mut self, edits: Vec<SourceEdit>) {
        self.apply_edits(edits, &OperationControl::new())
            .expect("a private parse control cannot be cancelled");
    }
}

#[derive(Debug, Clone)]
enum SourceMapping {
    Eager(Option<SourceEditMap>),
    Deferred(DeferredSourceMap),
}

#[derive(Debug, Clone)]
struct EditBatch {
    edits: Vec<SourceEdit>,
    output_len: usize,
}

/// Source edits are already validated in their pre-edit coordinate spaces. Save the batches
/// for ordinary parsing and compose them only if a diagnostic or warning needs original spans.
#[derive(Debug, Clone)]
struct DeferredSourceMap {
    original_len: usize,
    batches: Vec<EditBatch>,
    composed: OnceLock<Box<SourceEditMap>>,
}

impl DeferredSourceMap {
    fn new(original_len: usize) -> Self {
        Self {
            original_len,
            batches: Vec::new(),
            composed: OnceLock::new(),
        }
    }

    fn resolve(&self) -> Option<&SourceEditMap> {
        if let Some(map) = self.composed.get() {
            return Some(map);
        }
        if self.batches.is_empty() {
            return None;
        }
        self.resolve_controlled(&OperationControl::new())
            .expect("a private source mapping control cannot be cancelled");
        self.composed.get().map(Box::as_ref)
    }

    fn resolve_controlled(&self, control: &OperationControl) -> OperationControlResult<()> {
        if self.batches.is_empty() || self.composed.get().is_some() {
            return Ok(());
        }
        let mut checkpoints = ControlledEditRebuildCheckpoints::new(control)?;
        let mut map = SourceEditMap::identity(self.original_len);
        for batch in &self.batches {
            map = map.compose_batch(&batch.edits, batch.output_len, &mut checkpoints)?;
        }
        checkpoints.finish()?;
        // Another observer of the same source can finish the same deterministic map first.
        let _ = self.composed.set(Box::new(map));
        Ok(())
    }
}

struct ControlledEditRebuildCheckpoints<'a> {
    control: &'a OperationControl,
    bytes_since_checkpoint: usize,
    items_since_checkpoint: usize,
}

impl<'a> ControlledEditRebuildCheckpoints<'a> {
    fn new(control: &'a OperationControl) -> OperationControlResult<Self> {
        control.checkpoint()?;
        Ok(Self {
            control,
            bytes_since_checkpoint: 0,
            items_since_checkpoint: 0,
        })
    }

    fn push_str(&mut self, output: &mut String, value: &str) -> OperationControlResult<()> {
        let mut start = 0usize;
        while start < value.len() {
            let remaining_budget = CONTROLLED_EDIT_REBUILD_CHECKPOINT_BYTES
                .saturating_sub(self.bytes_since_checkpoint);
            if remaining_budget == 0 {
                self.checkpoint()?;
                continue;
            }

            let mut end = start.saturating_add(remaining_budget).min(value.len());
            while end > start && !value.is_char_boundary(end) {
                end -= 1;
            }
            if end == start {
                self.checkpoint()?;
                continue;
            }

            output.push_str(&value[start..end]);
            self.bytes_since_checkpoint += end - start;
            start = end;
            if self.bytes_since_checkpoint == CONTROLLED_EDIT_REBUILD_CHECKPOINT_BYTES {
                self.checkpoint()?;
            }
        }
        Ok(())
    }

    fn processed_item(&mut self) -> OperationControlResult<()> {
        self.items_since_checkpoint += 1;
        if self.items_since_checkpoint == CONTROLLED_EDIT_REBUILD_CHECKPOINT_ITEMS {
            self.checkpoint()?;
        }
        Ok(())
    }

    fn checkpoint(&mut self) -> OperationControlResult<()> {
        self.control.checkpoint()?;
        self.bytes_since_checkpoint = 0;
        self.items_since_checkpoint = 0;
        Ok(())
    }

    fn finish(&self) -> OperationControlResult<()> {
        self.control.checkpoint()
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
struct SourceEditMap {
    original_len: usize,
    output_len: usize,
    segments: Vec<EditMapSegment>,
    gaps: Vec<EditMapGap>,
    // Boundary mappings are frequent (notably CRLF normalization) but do not make a span
    // unmappable. Keep only the relevant regions in a searchable index for fact lookups.
    unmapped_output_ranges: Vec<Range<usize>>,
    #[cfg(test)]
    scan_stats: EditMapScanStats,
}

impl SourceEditMap {
    fn identity(source_len: usize) -> Self {
        let segments = (source_len > 0)
            .then_some(EditMapSegment {
                output: 0..source_len,
                original: 0..source_len,
                mapping: SegmentMapping::ExactBytes,
            })
            .into_iter()
            .collect();
        Self {
            original_len: source_len,
            output_len: source_len,
            segments,
            gaps: Vec::new(),
            unmapped_output_ranges: Vec::new(),
            #[cfg(test)]
            scan_stats: EditMapScanStats::default(),
        }
    }

    fn compose_batch(
        &self,
        edits: &[SourceEdit],
        output_len: usize,
        checkpoints: &mut ControlledEditRebuildCheckpoints<'_>,
    ) -> OperationControlResult<Self> {
        let mut builder = EditMapBuilder::new(self.original_len);
        let mut cursor = 0usize;
        for edit in edits {
            checkpoints.processed_item()?;
            builder.copy_from(self, cursor..edit.range.start, checkpoints)?;
            if edit.replacement.is_empty() {
                builder.delete_from(self, edit.range.clone(), checkpoints)?;
            } else {
                builder.replace_from(
                    self,
                    edit.range.clone(),
                    edit.replacement.len(),
                    edit.mapping,
                    checkpoints,
                )?;
            }
            cursor = edit.range.end;
        }
        builder.copy_from(self, cursor..self.output_len, checkpoints)?;
        let map = builder.finish(output_len, checkpoints)?;
        #[cfg(debug_assertions)]
        {
            let well_formed = map.is_well_formed_controlled(checkpoints)?;
            debug_assert!(well_formed);
        }
        Ok(map)
    }

    /// Maps a parser-input span to the smallest exact span in the original source.
    ///
    /// A span that crosses deleted text or ends inside a length-changing replacement is not exact
    /// and returns `None`. Adjacent facts remain independently mappable.
    fn try_map_span(&self, span: SourceSpan) -> Option<SourceSpan> {
        if span.start > span.end || span.end > self.output_len {
            return None;
        }
        if self.has_unmapped_overlap(span.start, span.end)
            || (span.start < span.end && self.has_gap_inside(span.start, span.end))
        {
            return None;
        }

        let start = self.original_at_start(span.start)?;
        let end = if span.start == span.end {
            start
        } else {
            self.original_at_end(span.end)?
        };
        (start <= end).then(|| SourceSpan::new(start, end))
    }

    /// Maps span boundaries while allowing deleted source bytes inside the enclosing range.
    ///
    /// This is provenance-only: callers must not treat the returned range as an exact rewrite
    /// target. It is used for parser facts such as multiline keys that cross dedented indentation
    /// or normalized line endings.
    fn try_map_enclosing_span(&self, span: SourceSpan) -> Option<SourceSpan> {
        if span.start > span.end
            || span.end > self.output_len
            || self.has_unmapped_overlap(span.start, span.end)
        {
            return None;
        }

        let start = self.original_at_start(span.start)?;
        let end = if span.start == span.end {
            start
        } else {
            self.original_at_end(span.end)?
        };
        (start <= end).then(|| SourceSpan::new(start, end))
    }

    fn original_at_start(&self, offset: usize) -> Option<usize> {
        if offset > self.output_len {
            return None;
        }
        if let Some(gap) = self.gap_at(offset) {
            return Some(gap.original_right);
        }
        if offset == self.output_len {
            return match self.segments.last() {
                Some(segment) => segment.original_at_end(offset),
                None => Some(self.original_len),
            };
        }
        self.segment_to_right(offset)?.original_at_start(offset)
    }

    fn original_at_end(&self, offset: usize) -> Option<usize> {
        if offset > self.output_len {
            return None;
        }
        if let Some(gap) = self.gap_at(offset) {
            return Some(gap.original_left);
        }
        if offset == 0 {
            return match self.segments.first() {
                Some(segment) => segment.original_at_start(offset),
                None => Some(0),
            };
        }
        self.segment_to_left(offset)?.original_at_end(offset)
    }

    fn segment_to_right(&self, offset: usize) -> Option<&EditMapSegment> {
        let index = self
            .segments
            .partition_point(|segment| segment.output.end <= offset);
        self.segments
            .get(index)
            .filter(|segment| segment.output.start <= offset && offset < segment.output.end)
    }

    fn segment_to_left(&self, offset: usize) -> Option<&EditMapSegment> {
        let index = self
            .segments
            .partition_point(|segment| segment.output.start < offset)
            .checked_sub(1)?;
        self.segments
            .get(index)
            .filter(|segment| segment.output.start < offset && offset <= segment.output.end)
    }

    fn gap_at(&self, offset: usize) -> Option<&EditMapGap> {
        self.gaps
            .binary_search_by_key(&offset, |gap| gap.output_offset)
            .ok()
            .and_then(|index| self.gaps.get(index))
    }

    fn has_gap_inside(&self, start: usize, end: usize) -> bool {
        let first = self.gaps.partition_point(|gap| gap.output_offset <= start);
        self.gaps
            .get(first)
            .is_some_and(|gap| gap.output_offset < end)
    }

    fn has_unmapped_overlap(&self, start: usize, end: usize) -> bool {
        let first = self
            .unmapped_output_ranges
            .partition_point(|range| range.end <= start);
        self.unmapped_output_ranges.get(first).is_some_and(|range| {
            if start == end {
                range.start < start && start < range.end
            } else {
                range.start < end && start < range.end
            }
        })
    }

    #[cfg(test)]
    fn unmapped_ranges_match_segments(&self) -> bool {
        self.unmapped_output_ranges.iter().eq(self
            .segments
            .iter()
            .filter(|segment| segment.mapping == SegmentMapping::Unmapped)
            .map(|segment| &segment.output))
    }

    #[cfg(test)]
    fn is_well_formed(&self) -> bool {
        let control = OperationControl::new();
        let mut checkpoints = ControlledEditRebuildCheckpoints::new(&control)
            .expect("a private parse control cannot be cancelled");
        self.is_well_formed_controlled(&mut checkpoints)
            .expect("a private parse control cannot be cancelled")
    }

    #[cfg(any(test, debug_assertions))]
    fn is_well_formed_controlled(
        &self,
        checkpoints: &mut ControlledEditRebuildCheckpoints<'_>,
    ) -> OperationControlResult<bool> {
        let mut previous_segment: Option<&EditMapSegment> = None;
        let mut unmapped_index = 0usize;
        for segment in &self.segments {
            checkpoints.processed_item()?;
            if segment.output.start >= segment.output.end {
                return Ok(false);
            }
            if let Some(previous) = previous_segment
                && (previous.output.end > segment.output.start
                    || previous.original.end > segment.original.start)
            {
                return Ok(false);
            }
            if segment.mapping == SegmentMapping::Unmapped {
                if self.unmapped_output_ranges.get(unmapped_index) != Some(&segment.output) {
                    return Ok(false);
                }
                unmapped_index += 1;
            }
            previous_segment = Some(segment);
        }
        if unmapped_index != self.unmapped_output_ranges.len()
            || self
                .segments
                .last()
                .is_some_and(|segment| segment.output.end > self.output_len)
        {
            return Ok(false);
        }

        let mut previous_gap_offset: Option<usize> = None;
        for gap in &self.gaps {
            checkpoints.processed_item()?;
            if previous_gap_offset.is_some_and(|offset| offset >= gap.output_offset) {
                return Ok(false);
            }
            previous_gap_offset = Some(gap.output_offset);
        }

        let mut previous_unmapped_end: Option<usize> = None;
        for range in &self.unmapped_output_ranges {
            checkpoints.processed_item()?;
            if previous_unmapped_end.is_some_and(|end| end > range.start) {
                return Ok(false);
            }
            previous_unmapped_end = Some(range.end);
        }
        Ok(true)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EditMapSegment {
    output: Range<usize>,
    original: Range<usize>,
    mapping: SegmentMapping,
}

impl EditMapSegment {
    fn original_at_start(&self, offset: usize) -> Option<usize> {
        match self.mapping {
            SegmentMapping::ExactBytes => self
                .original
                .start
                .checked_add(offset.checked_sub(self.output.start)?),
            SegmentMapping::Boundaries if offset == self.output.start => Some(self.original.start),
            SegmentMapping::Boundaries if offset == self.output.end => Some(self.original.end),
            SegmentMapping::Boundaries => None,
            SegmentMapping::Unmapped => None,
        }
    }

    fn original_at_end(&self, offset: usize) -> Option<usize> {
        self.original_at_start(offset)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SegmentMapping {
    ExactBytes,
    Boundaries,
    Unmapped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EditMapGap {
    output_offset: usize,
    original_left: usize,
    original_right: usize,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct EditMapScanStats {
    copy_ranges: usize,
    copy_segment_steps: usize,
    copy_gap_steps: usize,
    lookup_segment_advances: usize,
    lookup_gap_advances: usize,
    exact_segment_steps: usize,
    exact_gap_steps: usize,
}

/// Monotonic read positions into the map consumed by one sorted edit batch.
///
/// Copying, endpoint lookup, and exactness checks advance independently because replacements read
/// the same range for all three purposes. Every index only increases; copying can revisit at most
/// the one segment or gap that straddles the next range boundary.
#[derive(Default)]
struct EditMapCursor {
    copy_segment_index: usize,
    copy_gap_index: usize,
    lookup_segment_index: usize,
    lookup_gap_index: usize,
    exact_segment_index: usize,
    exact_gap_index: usize,
    #[cfg(debug_assertions)]
    last_copy_end: usize,
    #[cfg(debug_assertions)]
    last_lookup_offset: usize,
    #[cfg(debug_assertions)]
    last_exact_end: usize,
    #[cfg(test)]
    scan_stats: EditMapScanStats,
}

impl EditMapCursor {
    fn advance_lookup_to(
        &mut self,
        old: &SourceEditMap,
        offset: usize,
        checkpoints: &mut ControlledEditRebuildCheckpoints<'_>,
    ) -> OperationControlResult<()> {
        #[cfg(debug_assertions)]
        {
            debug_assert!(self.last_lookup_offset <= offset);
            self.last_lookup_offset = offset;
        }

        while old
            .segments
            .get(self.lookup_segment_index)
            .is_some_and(|segment| segment.output.end <= offset)
        {
            checkpoints.processed_item()?;
            self.lookup_segment_index += 1;
            #[cfg(test)]
            {
                self.scan_stats.lookup_segment_advances += 1;
            }
        }
        while old
            .gaps
            .get(self.lookup_gap_index)
            .is_some_and(|gap| gap.output_offset < offset)
        {
            checkpoints.processed_item()?;
            self.lookup_gap_index += 1;
            #[cfg(test)]
            {
                self.scan_stats.lookup_gap_advances += 1;
            }
        }
        Ok(())
    }

    fn gap_at_lookup_offset<'a>(
        &self,
        old: &'a SourceEditMap,
        offset: usize,
    ) -> Option<&'a EditMapGap> {
        old.gaps
            .get(self.lookup_gap_index)
            .filter(|gap| gap.output_offset == offset)
    }

    fn original_at_start(
        &mut self,
        old: &SourceEditMap,
        offset: usize,
        checkpoints: &mut ControlledEditRebuildCheckpoints<'_>,
    ) -> OperationControlResult<Option<usize>> {
        if offset > old.output_len {
            return Ok(None);
        }
        self.advance_lookup_to(old, offset, checkpoints)?;
        if let Some(gap) = self.gap_at_lookup_offset(old, offset) {
            return Ok(Some(gap.original_right));
        }
        if offset == old.output_len {
            return Ok(match old.segments.last() {
                Some(segment) => segment.original_at_end(offset),
                None => Some(old.original_len),
            });
        }
        Ok(old
            .segments
            .get(self.lookup_segment_index)
            .filter(|segment| segment.output.start <= offset && offset < segment.output.end)
            .and_then(|segment| segment.original_at_start(offset)))
    }

    fn original_at_end(
        &mut self,
        old: &SourceEditMap,
        offset: usize,
        checkpoints: &mut ControlledEditRebuildCheckpoints<'_>,
    ) -> OperationControlResult<Option<usize>> {
        if offset > old.output_len {
            return Ok(None);
        }
        self.advance_lookup_to(old, offset, checkpoints)?;
        if let Some(gap) = self.gap_at_lookup_offset(old, offset) {
            return Ok(Some(gap.original_left));
        }
        if offset == 0 {
            return Ok(match old.segments.first() {
                Some(segment) => segment.original_at_start(offset),
                None => Some(0),
            });
        }
        if let Some(segment) = old.segments.get(self.lookup_segment_index)
            && segment.output.start < offset
            && offset < segment.output.end
        {
            return Ok(segment.original_at_end(offset));
        }
        Ok(self
            .lookup_segment_index
            .checked_sub(1)
            .and_then(|index| old.segments.get(index))
            .filter(|segment| segment.output.start < offset && offset <= segment.output.end)
            .and_then(|segment| segment.original_at_end(offset)))
    }

    fn original_anchor(
        &mut self,
        old: &SourceEditMap,
        offset: usize,
        checkpoints: &mut ControlledEditRebuildCheckpoints<'_>,
    ) -> OperationControlResult<usize> {
        self.advance_lookup_to(old, offset, checkpoints)?;
        if let Some(gap) = self.gap_at_lookup_offset(old, offset) {
            return Ok(gap.original_right);
        }
        if offset == old.output_len {
            return Ok(old.original_len);
        }
        Ok(old
            .segments
            .get(self.lookup_segment_index)
            .filter(|segment| segment.output.start <= offset && offset < segment.output.end)
            .map_or(old.original_len, |segment| segment.original.start))
    }

    fn range_is_exact_bytes(
        &mut self,
        old: &SourceEditMap,
        range: &Range<usize>,
        checkpoints: &mut ControlledEditRebuildCheckpoints<'_>,
    ) -> OperationControlResult<bool> {
        #[cfg(debug_assertions)]
        {
            debug_assert!(self.last_exact_end <= range.start);
            self.last_exact_end = range.end;
        }
        if range.start > range.end || range.end > old.output_len {
            return Ok(false);
        }
        if range.start == range.end {
            return Ok(true);
        }

        while old
            .segments
            .get(self.exact_segment_index)
            .is_some_and(|segment| segment.output.end <= range.start)
        {
            checkpoints.processed_item()?;
            self.exact_segment_index += 1;
            #[cfg(test)]
            {
                self.scan_stats.exact_segment_steps += 1;
            }
        }
        while old
            .gaps
            .get(self.exact_gap_index)
            .is_some_and(|gap| gap.output_offset <= range.start)
        {
            checkpoints.processed_item()?;
            self.exact_gap_index += 1;
            #[cfg(test)]
            {
                self.scan_stats.exact_gap_steps += 1;
            }
        }
        if old
            .gaps
            .get(self.exact_gap_index)
            .is_some_and(|gap| gap.output_offset < range.end)
        {
            checkpoints.processed_item()?;
            #[cfg(test)]
            {
                self.scan_stats.exact_gap_steps += 1;
            }
            return Ok(false);
        }

        let mut output_offset = range.start;
        while output_offset < range.end {
            let Some(segment) = old.segments.get(self.exact_segment_index) else {
                return Ok(false);
            };
            checkpoints.processed_item()?;
            #[cfg(test)]
            {
                self.scan_stats.exact_segment_steps += 1;
            }
            if segment.output.start > output_offset || segment.mapping != SegmentMapping::ExactBytes
            {
                return Ok(false);
            }
            output_offset = segment.output.end.min(range.end);
            if segment.output.end <= range.end {
                self.exact_segment_index += 1;
            }
        }
        Ok(true)
    }
}

struct EditMapBuilder {
    original_len: usize,
    output_len: usize,
    segments: Vec<EditMapSegment>,
    gaps: Vec<EditMapGap>,
    cursor: EditMapCursor,
}

impl EditMapBuilder {
    fn new(original_len: usize) -> Self {
        Self {
            original_len,
            output_len: 0,
            segments: Vec::new(),
            gaps: Vec::new(),
            cursor: EditMapCursor::default(),
        }
    }

    fn copy_from(
        &mut self,
        old: &SourceEditMap,
        range: Range<usize>,
        checkpoints: &mut ControlledEditRebuildCheckpoints<'_>,
    ) -> OperationControlResult<()> {
        if range.start >= range.end {
            return Ok(());
        }
        #[cfg(debug_assertions)]
        {
            debug_assert!(self.cursor.last_copy_end <= range.start);
            self.cursor.last_copy_end = range.end;
        }
        #[cfg(test)]
        {
            self.cursor.scan_stats.copy_ranges += 1;
        }
        let output_base = self.output_len;

        while old
            .segments
            .get(self.cursor.copy_segment_index)
            .is_some_and(|segment| segment.output.end <= range.start)
        {
            checkpoints.processed_item()?;
            #[cfg(test)]
            {
                self.cursor.scan_stats.copy_segment_steps += 1;
            }
            self.cursor.copy_segment_index += 1;
        }
        while let Some(segment) = old.segments.get(self.cursor.copy_segment_index) {
            if segment.output.start >= range.end {
                break;
            }
            checkpoints.processed_item()?;
            #[cfg(test)]
            {
                self.cursor.scan_stats.copy_segment_steps += 1;
            }
            let start = segment.output.start.max(range.start);
            let end = segment.output.end.min(range.end);
            debug_assert!(start < end);

            let (original, mapping) = match segment.mapping {
                SegmentMapping::ExactBytes => {
                    let original_start = segment.original.start + (start - segment.output.start);
                    (
                        original_start..original_start + (end - start),
                        SegmentMapping::ExactBytes,
                    )
                }
                SegmentMapping::Boundaries
                    if start == segment.output.start && end == segment.output.end =>
                {
                    (segment.original.clone(), SegmentMapping::Boundaries)
                }
                SegmentMapping::Boundaries | SegmentMapping::Unmapped => {
                    let anchor = segment.original.start;
                    (anchor..anchor, SegmentMapping::Unmapped)
                }
            };
            self.push_segment(EditMapSegment {
                output: output_base + (start - range.start)..output_base + (end - range.start),
                original,
                mapping,
            });
            if segment.output.end <= range.end {
                self.cursor.copy_segment_index += 1;
            } else {
                break;
            }
        }

        while old
            .gaps
            .get(self.cursor.copy_gap_index)
            .is_some_and(|gap| gap.output_offset < range.start)
        {
            checkpoints.processed_item()?;
            #[cfg(test)]
            {
                self.cursor.scan_stats.copy_gap_steps += 1;
            }
            self.cursor.copy_gap_index += 1;
        }
        while let Some(gap) = old.gaps.get(self.cursor.copy_gap_index) {
            if gap.output_offset > range.end {
                break;
            }
            checkpoints.processed_item()?;
            #[cfg(test)]
            {
                self.cursor.scan_stats.copy_gap_steps += 1;
            }
            self.push_gap(EditMapGap {
                output_offset: output_base + (gap.output_offset - range.start),
                original_left: gap.original_left,
                original_right: gap.original_right,
            });
            if gap.output_offset == range.end {
                break;
            }
            self.cursor.copy_gap_index += 1;
        }
        self.output_len += range.end - range.start;
        Ok(())
    }

    fn delete_from(
        &mut self,
        old: &SourceEditMap,
        range: Range<usize>,
        checkpoints: &mut ControlledEditRebuildCheckpoints<'_>,
    ) -> OperationControlResult<()> {
        let left = match self.cursor.original_at_end(old, range.start, checkpoints)? {
            Some(offset) => offset,
            None => self.cursor.original_anchor(old, range.start, checkpoints)?,
        };
        let right = match self.cursor.original_at_start(old, range.end, checkpoints)? {
            Some(offset) => offset,
            None => self.cursor.original_anchor(old, range.end, checkpoints)?,
        };
        self.push_gap(EditMapGap {
            output_offset: self.output_len,
            original_left: left,
            original_right: right,
        });
        Ok(())
    }

    fn replace_from(
        &mut self,
        old: &SourceEditMap,
        range: Range<usize>,
        replacement_len: usize,
        mapping: ReplacementMapping,
        checkpoints: &mut ControlledEditRebuildCheckpoints<'_>,
    ) -> OperationControlResult<()> {
        let original_start = self
            .cursor
            .original_at_start(old, range.start, checkpoints)?;
        let start_is_mappable = original_start.is_some();
        let original_start = match original_start {
            Some(offset) => offset,
            None => self.cursor.original_anchor(old, range.start, checkpoints)?,
        };
        let original_end = self.cursor.original_at_end(old, range.end, checkpoints)?;
        let endpoints_are_mappable = start_is_mappable && original_end.is_some();
        let original_end = original_end.unwrap_or(original_start);
        let exact = mapping == ReplacementMapping::ExactBytes
            && endpoints_are_mappable
            && replacement_len == range.end - range.start
            && self.cursor.range_is_exact_bytes(old, &range, checkpoints)?
            && original_end.checked_sub(original_start) == Some(replacement_len);
        self.push_segment(EditMapSegment {
            output: self.output_len..self.output_len + replacement_len,
            original: original_start..original_end,
            mapping: if !endpoints_are_mappable {
                SegmentMapping::Unmapped
            } else if exact {
                SegmentMapping::ExactBytes
            } else {
                SegmentMapping::Boundaries
            },
        });
        self.output_len += replacement_len;
        Ok(())
    }

    fn push_segment(&mut self, segment: EditMapSegment) {
        if let Some(last) = self.segments.last_mut()
            && last.mapping == SegmentMapping::ExactBytes
            && segment.mapping == SegmentMapping::ExactBytes
            && last.output.end == segment.output.start
            && last.original.end == segment.original.start
        {
            last.output.end = segment.output.end;
            last.original.end = segment.original.end;
            return;
        }
        self.segments.push(segment);
    }

    fn push_gap(&mut self, gap: EditMapGap) {
        if let Some(last) = self.gaps.last_mut() {
            assert!(
                last.output_offset <= gap.output_offset,
                "source edit map gaps must be emitted in output order"
            );
            if last.output_offset == gap.output_offset {
                last.original_left = last.original_left.min(gap.original_left);
                last.original_right = last.original_right.max(gap.original_right);
                return;
            }
        }
        self.gaps.push(gap);
    }

    fn finish(
        self,
        output_len: usize,
        checkpoints: &mut ControlledEditRebuildCheckpoints<'_>,
    ) -> OperationControlResult<SourceEditMap> {
        debug_assert_eq!(self.output_len, output_len);
        let segments = self.segments;
        let mut unmapped_output_ranges = Vec::new();
        for segment in &segments {
            checkpoints.processed_item()?;
            if segment.mapping == SegmentMapping::Unmapped {
                unmapped_output_ranges.push(segment.output.clone());
            }
        }
        Ok(SourceEditMap {
            original_len: self.original_len,
            output_len,
            segments,
            gaps: self.gaps,
            unmapped_output_ranges,
            #[cfg(test)]
            scan_stats: self.cursor.scan_stats,
        })
    }
}

fn edits_are_sorted_controlled(
    edits: &[SourceEdit],
    checkpoints: &mut ControlledEditRebuildCheckpoints<'_>,
) -> OperationControlResult<bool> {
    for pair in edits.windows(2) {
        checkpoints.processed_item()?;
        if (pair[0].range.start, pair[0].range.end) > (pair[1].range.start, pair[1].range.end) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn assert_valid_edits_controlled(
    source: &str,
    edits: &[SourceEdit],
    checkpoints: &mut ControlledEditRebuildCheckpoints<'_>,
) -> OperationControlResult<(usize, usize)> {
    let mut cursor = 0usize;
    let mut replacement_bytes = 0usize;
    let mut removed_bytes = 0usize;
    for edit in edits {
        checkpoints.processed_item()?;
        assert!(edit.range.start <= edit.range.end, "reversed source edit");
        assert!(edit.range.start >= cursor, "overlapping source edits");
        assert!(edit.range.end <= source.len(), "source edit out of bounds");
        assert!(source.is_char_boundary(edit.range.start));
        assert!(source.is_char_boundary(edit.range.end));
        replacement_bytes = replacement_bytes.saturating_add(edit.replacement.len());
        removed_bytes = removed_bytes.saturating_add(edit.range.end - edit.range.start);
        cursor = edit.range.end;
    }
    Ok((replacement_bytes, removed_bytes))
}

#[cfg(test)]
mod tests;
