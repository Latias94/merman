mod line_index;

use self::line_index::LineIndex;
use crate::payload::{DiagnosticSpan, LspRange, SourcePosition, Utf16Position};
use crate::retained_weight::{
    ARC_ALLOCATION_OVERHEAD, RetainedWeight, conservative_btree_entry_bytes,
};
use std::collections::BTreeMap;
use std::mem::size_of;
use std::ops::Range;
use std::sync::{Arc, Mutex};

pub type LineCol = SourcePosition;

/// An immutable UTF-8 slice that retains one shared source allocation.
///
/// Cloning and subslicing do not copy source bytes. Use [`SharedTextSlice::to_owned_text`] only
/// when the caller explicitly needs an independent `String`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedTextSlice {
    source: Arc<str>,
    start: usize,
    end: usize,
}

impl SharedTextSlice {
    /// Covers the complete shared source allocation.
    pub fn whole(source: Arc<str>) -> Self {
        let end = source.len();
        Self {
            source,
            start: 0,
            end,
        }
    }

    /// Creates a shared slice when both bounds are ordered, in range, and UTF-8 boundaries.
    pub fn from_range(source: Arc<str>, start: usize, end: usize) -> Option<Self> {
        if start > end
            || end > source.len()
            || !source.is_char_boundary(start)
            || !source.is_char_boundary(end)
        {
            return None;
        }
        Some(Self { source, start, end })
    }

    pub(crate) fn new(source: Arc<str>, start: usize, end: usize) -> Self {
        Self::from_range(source, start, end)
            .expect("document extraction should produce valid UTF-8 slice bounds")
    }

    /// Borrows the selected UTF-8 text.
    pub fn as_str(&self) -> &str {
        &self.source[self.start..self.end]
    }

    /// Clones the owning `Arc` without copying source bytes.
    pub fn source_arc(&self) -> Arc<str> {
        Arc::clone(&self.source)
    }

    /// Copies the selected text into a new owned `String`.
    pub fn to_owned_text(&self) -> String {
        self.as_str().to_owned()
    }

    pub const fn start(&self) -> usize {
        self.start
    }

    pub const fn end(&self) -> usize {
        self.end
    }

    #[cfg(test)]
    pub(crate) fn shares_source_allocation_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.source, &other.source)
    }
}

impl AsRef<str> for SharedTextSlice {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::ops::Deref for SharedTextSlice {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SourceMapError {
    #[error("byte offset {offset} is outside source length {source_len}")]
    OffsetOutOfBounds { offset: usize, source_len: usize },
    #[error("byte offset {offset} is not a UTF-8 character boundary")]
    OffsetNotCharBoundary { offset: usize },
    #[error("range start {start} is after range end {end}")]
    ReversedRange { start: usize, end: usize },
}

/// Source-position mapping backed by a private adaptive line index.
///
/// Callers query line and position behavior; the compact retained representation is deliberately
/// not exposed.
#[derive(Debug, Clone)]
pub struct SourceMap {
    source: SharedTextSlice,
    line_index: Arc<LineIndex>,
    line_metrics: Arc<Mutex<LineMetricCache>>,
}

/// Per-source allowance charged for lazily materialized UTF-16 line metrics.
pub(crate) const SOURCE_MAP_LINE_METRIC_CACHE_BUDGET_BYTES: usize = 256 * 1024;

impl SourceMap {
    /// Builds a complete source map synchronously.
    ///
    /// This convenience constructor scans the full source before returning. Cooperative
    /// cancellation is reserved for the crate's analysis pipelines.
    pub fn new(source: impl Into<Arc<str>>) -> Self {
        Self::from_shared_text(SharedTextSlice::whole(source.into()))
    }

    pub(crate) fn from_shared_text(source: SharedTextSlice) -> Self {
        let line_index = LineIndex::build(source.as_str());
        Self::from_source_and_line_index(source, line_index)
    }

    pub(crate) fn new_cancellable(
        source: impl Into<Arc<str>>,
        cancellation: &crate::AnalysisCancellationToken,
    ) -> Result<Self, crate::AnalysisCancelled> {
        Self::from_shared_text_cancellable(SharedTextSlice::whole(source.into()), cancellation)
    }

    pub(crate) fn from_shared_text_cancellable(
        source: SharedTextSlice,
        cancellation: &crate::AnalysisCancellationToken,
    ) -> Result<Self, crate::AnalysisCancelled> {
        let line_index = LineIndex::build_cancellable(source.as_str(), cancellation)?;
        cancellation.checkpoint()?;
        Ok(Self::from_source_and_line_index(source, line_index))
    }

    fn from_source_and_line_index(source: SharedTextSlice, line_index: LineIndex) -> Self {
        Self {
            source,
            line_index: Arc::new(line_index),
            line_metrics: Arc::new(Mutex::new(LineMetricCache::new(
                SOURCE_MAP_LINE_METRIC_CACHE_BUDGET_BYTES,
            ))),
        }
    }

    pub fn source(&self) -> &str {
        self.source.as_str()
    }

    /// Returns the shared source view retained by this map.
    pub fn shared_source(&self) -> &SharedTextSlice {
        &self.source
    }

    pub fn source_len(&self) -> usize {
        self.source.as_str().len()
    }

    /// Returns the number of logical source lines, including a trailing empty line.
    pub fn line_count(&self) -> usize {
        self.line_index.line_count()
    }

    /// Returns the byte offset at which the requested logical line starts.
    pub fn line_start(&self, line_index: usize) -> Option<usize> {
        self.line_index.line_start(line_index)
    }

    pub(crate) fn estimated_owned_heap_bytes_excluding_source(&self) -> usize {
        let mut weight = RetainedWeight::default();
        weight.add(ARC_ALLOCATION_OVERHEAD);
        weight.add(self.line_index.estimated_owned_heap_bytes());
        weight.add(ARC_ALLOCATION_OVERHEAD);
        weight.add(size_of::<Mutex<LineMetricCache>>());
        weight.add(SOURCE_MAP_LINE_METRIC_CACHE_BUDGET_BYTES);
        weight.finish()
    }

    pub fn line_col(&self, offset: usize) -> Result<LineCol, SourceMapError> {
        let metrics = self.offset_metrics(offset)?;
        Ok(LineCol::new(
            metrics.line_index + 1,
            metrics.char_column + 1,
        ))
    }

    pub fn utf16_position(&self, offset: usize) -> Result<Utf16Position, SourceMapError> {
        let metrics = self.offset_metrics(offset)?;
        Ok(Utf16Position {
            line: metrics.line_index,
            character: metrics.utf16_column,
        })
    }

    pub fn span(&self, start: usize, end: usize) -> Result<DiagnosticSpan, SourceMapError> {
        if start > end {
            return Err(SourceMapError::ReversedRange { start, end });
        }

        let (start_metrics, end_metrics) = self.offset_metrics_pair(start, end)?;
        Ok(span_from_offset_metrics(
            start,
            end,
            start_metrics,
            end_metrics,
        ))
    }

    pub(crate) fn span_cancellable(
        &self,
        start: usize,
        end: usize,
        cancellation: &crate::AnalysisCancellationToken,
    ) -> Result<Result<DiagnosticSpan, SourceMapError>, crate::AnalysisCancelled> {
        if start > end {
            return Ok(Err(SourceMapError::ReversedRange { start, end }));
        }

        let (start_metrics, end_metrics) =
            match self.offset_metrics_pair_cancellable(start, end, cancellation)? {
                Ok(metrics) => metrics,
                Err(error) => return Ok(Err(error)),
            };
        Ok(Ok(span_from_offset_metrics(
            start,
            end,
            start_metrics,
            end_metrics,
        )))
    }

    pub fn whole_source_span(&self) -> Result<DiagnosticSpan, SourceMapError> {
        self.span(0, self.source_len())
    }

    pub(crate) fn whole_source_span_cancellable(
        &self,
        cancellation: &crate::AnalysisCancellationToken,
    ) -> Result<Result<DiagnosticSpan, SourceMapError>, crate::AnalysisCancelled> {
        self.span_cancellable(0, self.source_len(), cancellation)
    }

    /// Returns the content byte bounds for a logical line, excluding its line terminator.
    pub fn line_bounds(&self, line_index: usize) -> Option<(usize, usize)> {
        let start = self.line_start(line_index)?;
        let next_start = self.line_start(line_index + 1).unwrap_or(self.source_len());
        Some((
            start,
            line_content_end(self.source().as_bytes(), start, next_start),
        ))
    }

    pub fn byte_offset_for_utf16_position(&self, position: Utf16Position) -> Option<usize> {
        let line = self.line_metric(position.line)?;
        line.byte_offset_for_utf16_column(self.source(), position.character)
    }

    fn validate_offset(&self, offset: usize) -> Result<(), SourceMapError> {
        if offset > self.source_len() {
            return Err(SourceMapError::OffsetOutOfBounds {
                offset,
                source_len: self.source_len(),
            });
        }
        if !self.source().is_char_boundary(offset) {
            return Err(SourceMapError::OffsetNotCharBoundary { offset });
        }
        Ok(())
    }

    fn line_index_for_offset(&self, offset: usize) -> usize {
        self.line_index.line_index_for_offset(offset)
    }

    fn offset_metrics(&self, offset: usize) -> Result<OffsetMetrics, SourceMapError> {
        self.validate_offset(offset)?;
        let line_index = self.line_index_for_offset(offset);
        let line = self
            .line_metric(line_index)
            .expect("validated source offset should map to a cached line");
        Ok(self.offset_metrics_for_line(offset, line_index, &line))
    }

    fn offset_metrics_pair(
        &self,
        start: usize,
        end: usize,
    ) -> Result<(OffsetMetrics, OffsetMetrics), SourceMapError> {
        self.validate_offset(start)?;
        self.validate_offset(end)?;
        let start_line_index = self.line_index_for_offset(start);
        let end_line_index = self.line_index_for_offset(end);
        let start_line = self
            .line_metric(start_line_index)
            .expect("validated source offset should map to a cached line");
        let end_line = if start_line_index == end_line_index {
            Arc::clone(&start_line)
        } else {
            self.line_metric(end_line_index)
                .expect("validated source offset should map to a cached line")
        };
        Ok((
            self.offset_metrics_for_line(start, start_line_index, &start_line),
            self.offset_metrics_for_line(end, end_line_index, &end_line),
        ))
    }

    fn offset_metrics_for_line(
        &self,
        offset: usize,
        line_index: usize,
        line: &LineMetric,
    ) -> OffsetMetrics {
        let clamped = offset.clamp(line.start, line.content_end);
        let relative = clamped - line.start;
        let (char_column, utf16_column) = line
            .columns_for_relative_offset(self.source(), relative)
            .expect("validated source offset should map to a cached line boundary");
        OffsetMetrics {
            line_index,
            char_column,
            utf16_column,
        }
    }

    fn offset_metrics_pair_cancellable(
        &self,
        start: usize,
        end: usize,
        cancellation: &crate::AnalysisCancellationToken,
    ) -> Result<Result<(OffsetMetrics, OffsetMetrics), SourceMapError>, crate::AnalysisCancelled>
    {
        if let Err(error) = self.validate_offset(start) {
            return Ok(Err(error));
        }
        if let Err(error) = self.validate_offset(end) {
            return Ok(Err(error));
        }
        let start_line_index = self.line_index_for_offset(start);
        let end_line_index = self.line_index_for_offset(end);
        let start_line = self
            .line_metric_cancellable(start_line_index, cancellation)?
            .expect("validated source offset should map to a cached line");
        let end_line = if start_line_index == end_line_index {
            Arc::clone(&start_line)
        } else {
            self.line_metric_cancellable(end_line_index, cancellation)?
                .expect("validated source offset should map to a cached line")
        };
        Ok(Ok((
            self.offset_metrics_for_line(start, start_line_index, &start_line),
            self.offset_metrics_for_line(end, end_line_index, &end_line),
        )))
    }

    fn line_metric(&self, line_index: usize) -> Option<Arc<LineMetric>> {
        self.line_metric_with_checkpoint(line_index, || Ok::<_, std::convert::Infallible>(()))
            .expect("infallible line-metric scan")
    }

    fn line_metric_cancellable(
        &self,
        line_index: usize,
        cancellation: &crate::AnalysisCancellationToken,
    ) -> Result<Option<Arc<LineMetric>>, crate::AnalysisCancelled> {
        self.line_metric_with_checkpoint(line_index, || cancellation.checkpoint())
    }

    fn line_metric_with_checkpoint<E>(
        &self,
        line_index: usize,
        mut checkpoint: impl FnMut() -> Result<(), E>,
    ) -> Result<Option<Arc<LineMetric>>, E> {
        checkpoint()?;
        let Some(start) = self.line_start(line_index) else {
            return Ok(None);
        };
        if let Some(metric) = self.line_metric_cache().get(line_index) {
            return Ok(Some(metric));
        }

        let next_start = self.line_start(line_index + 1).unwrap_or(self.source_len());
        let computed = Arc::new(line_metric_with_checkpoint(
            self.source(),
            start,
            next_start,
            &mut checkpoint,
        )?);
        let mut cache = self.line_metric_cache();
        checkpoint()?;
        Ok(Some(cache.insert(line_index, computed)))
    }

    fn line_metric_cache(&self) -> std::sync::MutexGuard<'_, LineMetricCache> {
        self.line_metrics
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[cfg(test)]
    fn cached_line_metric_count(&self) -> usize {
        self.line_metric_cache().entries.len()
    }

    #[cfg(test)]
    fn cached_line_checkpoint_count(&self, line_index: usize) -> Option<usize> {
        self.line_metric_cache()
            .entries
            .get(&line_index)
            .map(|entry| entry.metric.stored_checkpoint_count())
    }

    #[cfg(test)]
    fn cached_line_metric_bytes(&self) -> usize {
        self.line_metric_cache().retained_bytes
    }

    #[cfg(test)]
    fn estimated_line_metric_cache_allocation_bytes(&self) -> usize {
        self.line_metric_cache().estimated_allocation_bytes()
    }

    #[cfg(test)]
    pub(crate) fn shares_source_allocation_with(&self, source: &SharedTextSlice) -> bool {
        self.source.shares_source_allocation_with(source)
    }
}

#[derive(Debug)]
struct LineMetricCache {
    budget_bytes: usize,
    retained_bytes: usize,
    oldest: Option<usize>,
    newest: Option<usize>,
    entries: BTreeMap<usize, LineMetricCacheEntry>,
    #[cfg(test)]
    statistics: LineMetricCacheStatistics,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct LineMetricCacheStatistics {
    hits: usize,
    misses: usize,
    evictions: usize,
    oversized_entries: usize,
    current_weight: usize,
    high_water_weight: usize,
}

#[derive(Debug)]
struct LineMetricCacheEntry {
    metric: Arc<LineMetric>,
    weight: usize,
    previous: Option<usize>,
    next: Option<usize>,
}

impl LineMetricCache {
    fn new(budget_bytes: usize) -> Self {
        Self {
            budget_bytes,
            retained_bytes: 0,
            oldest: None,
            newest: None,
            entries: BTreeMap::new(),
            #[cfg(test)]
            statistics: LineMetricCacheStatistics::default(),
        }
    }

    fn get(&mut self, line_index: usize) -> Option<Arc<LineMetric>> {
        let Some(entry) = self.entries.get(&line_index) else {
            self.record_miss();
            return None;
        };
        let metric = Arc::clone(&entry.metric);
        self.record_hit();
        self.touch(line_index);
        Some(metric)
    }

    fn insert(&mut self, line_index: usize, computed: Arc<LineMetric>) -> Arc<LineMetric> {
        if let Some(existing) = self
            .entries
            .get(&line_index)
            .map(|entry| Arc::clone(&entry.metric))
        {
            self.touch(line_index);
            return existing;
        }

        let weight = line_metric_cache_entry_weight(&computed);
        if weight > self.budget_bytes {
            self.record_oversized_entry();
            return computed;
        }

        while self.retained_bytes > self.budget_bytes - weight {
            self.remove_oldest();
        }

        let previous = self.newest;
        self.entries.insert(
            line_index,
            LineMetricCacheEntry {
                metric: Arc::clone(&computed),
                weight,
                previous,
                next: None,
            },
        );
        if let Some(previous) = previous {
            self.entries
                .get_mut(&previous)
                .expect("newest line metric must still be cached")
                .next = Some(line_index);
        } else {
            self.oldest = Some(line_index);
        }
        self.newest = Some(line_index);
        self.retained_bytes = self
            .retained_bytes
            .checked_add(weight)
            .expect("line metric cache weight must not overflow");
        self.record_retained_weight();
        debug_assert!(self.retained_bytes <= self.budget_bytes);
        computed
    }

    fn touch(&mut self, line_index: usize) {
        if self.newest == Some(line_index) {
            return;
        }

        let (previous, next) = {
            let entry = self
                .entries
                .get(&line_index)
                .expect("touched line metric must be cached");
            (entry.previous, entry.next)
        };
        if let Some(previous) = previous {
            self.entries
                .get_mut(&previous)
                .expect("previous line metric must still be cached")
                .next = next;
        } else {
            self.oldest = next;
        }
        if let Some(next) = next {
            self.entries
                .get_mut(&next)
                .expect("next line metric must still be cached")
                .previous = previous;
        }

        let newest = self
            .newest
            .expect("a non-newest cached line metric must have a newest peer");
        self.entries
            .get_mut(&newest)
            .expect("newest line metric must still be cached")
            .next = Some(line_index);
        let entry = self
            .entries
            .get_mut(&line_index)
            .expect("touched line metric must still be cached");
        entry.previous = Some(newest);
        entry.next = None;
        self.newest = Some(line_index);
    }

    fn remove_oldest(&mut self) {
        let oldest = self
            .oldest
            .expect("an over-budget line metric cache must have a victim");
        let removed = self
            .entries
            .remove(&oldest)
            .expect("oldest line metric must still be cached");
        self.oldest = removed.next;
        if let Some(next) = removed.next {
            self.entries
                .get_mut(&next)
                .expect("next-oldest line metric must still be cached")
                .previous = None;
        } else {
            self.newest = None;
        }
        self.retained_bytes = self
            .retained_bytes
            .checked_sub(removed.weight)
            .expect("line metric cache weight must match its entries");
        self.record_eviction();
        self.record_retained_weight();
    }

    #[cfg(test)]
    fn statistics(&self) -> LineMetricCacheStatistics {
        self.statistics
    }

    #[cfg(test)]
    fn estimated_allocation_bytes(&self) -> usize {
        self.entries.values().fold(0usize, |bytes, entry| {
            let checkpoints = match &entry.metric.columns {
                LineColumns::Ascii => 0,
                LineColumns::Unicode { checkpoints } => checkpoints
                    .len()
                    .saturating_mul(size_of::<ColumnCheckpoint>()),
            };
            bytes
                .saturating_add(conservative_btree_entry_bytes::<usize, LineMetricCacheEntry>())
                .saturating_add(size_of::<LineMetric>())
                .saturating_add(ARC_ALLOCATION_OVERHEAD)
                .saturating_add(checkpoints)
        })
    }

    #[cfg(test)]
    fn record_hit(&mut self) {
        self.statistics.hits = self.statistics.hits.saturating_add(1);
    }

    #[cfg(not(test))]
    fn record_hit(&mut self) {}

    #[cfg(test)]
    fn record_miss(&mut self) {
        self.statistics.misses = self.statistics.misses.saturating_add(1);
    }

    #[cfg(not(test))]
    fn record_miss(&mut self) {}

    #[cfg(test)]
    fn record_eviction(&mut self) {
        self.statistics.evictions = self.statistics.evictions.saturating_add(1);
    }

    #[cfg(not(test))]
    fn record_eviction(&mut self) {}

    #[cfg(test)]
    fn record_oversized_entry(&mut self) {
        self.statistics.oversized_entries = self.statistics.oversized_entries.saturating_add(1);
    }

    #[cfg(not(test))]
    fn record_oversized_entry(&mut self) {}

    #[cfg(test)]
    fn record_retained_weight(&mut self) {
        self.statistics.current_weight = self.retained_bytes;
        self.statistics.high_water_weight =
            self.statistics.high_water_weight.max(self.retained_bytes);
    }

    #[cfg(not(test))]
    fn record_retained_weight(&mut self) {}
}

fn line_metric_cache_entry_weight(metric: &LineMetric) -> usize {
    let checkpoints = match &metric.columns {
        LineColumns::Ascii => 0,
        LineColumns::Unicode { checkpoints } => checkpoints
            .len()
            .saturating_mul(size_of::<ColumnCheckpoint>()),
    };
    conservative_btree_entry_bytes::<usize, LineMetricCacheEntry>()
        .saturating_add(size_of::<LineMetric>())
        .saturating_add(ARC_ALLOCATION_OVERHEAD)
        .saturating_add(checkpoints)
}

const LINE_COLUMN_CHECKPOINT_BYTES: usize = 1024;

#[derive(Debug, Clone)]
struct LineMetric {
    start: usize,
    content_end: usize,
    columns: LineColumns,
}

#[derive(Debug, Clone)]
enum LineColumns {
    Ascii,
    Unicode {
        checkpoints: Box<[ColumnCheckpoint]>,
    },
}

#[derive(Debug, Clone, Copy)]
struct ColumnCheckpoint {
    byte_offset: usize,
    char_column: usize,
    utf16_column: usize,
}

impl LineMetric {
    fn byte_offset_for_utf16_column(&self, source: &str, column: usize) -> Option<usize> {
        match &self.columns {
            LineColumns::Ascii => Some(self.start.saturating_add(column).min(self.content_end)),
            LineColumns::Unicode { checkpoints } => {
                let line = source.get(self.start..self.content_end)?;
                let checkpoint_index = checkpoints
                    .partition_point(|checkpoint| checkpoint.utf16_column <= column)
                    .saturating_sub(1);
                let checkpoint = checkpoints[checkpoint_index];
                let mut byte_offset = checkpoint.byte_offset;
                let mut utf16_column = checkpoint.utf16_column;

                if utf16_column == column {
                    return Some(self.start + byte_offset);
                }

                for ch in line.get(byte_offset..)?.chars() {
                    let next_utf16_column = utf16_column + ch.len_utf16();
                    if column < next_utf16_column {
                        return None;
                    }
                    byte_offset += ch.len_utf8();
                    utf16_column = next_utf16_column;
                    if utf16_column == column {
                        return Some(self.start + byte_offset);
                    }
                }

                Some(self.content_end)
            }
        }
    }

    fn columns_for_relative_offset(&self, source: &str, relative: usize) -> Option<(usize, usize)> {
        match &self.columns {
            LineColumns::Ascii => Some((relative, relative)),
            LineColumns::Unicode { checkpoints } => {
                let line = source.get(self.start..self.content_end)?;
                let checkpoint_index = checkpoints
                    .partition_point(|checkpoint| checkpoint.byte_offset <= relative)
                    .saturating_sub(1);
                let checkpoint = checkpoints[checkpoint_index];
                let suffix = line.get(checkpoint.byte_offset..relative)?;
                let (char_delta, utf16_delta) =
                    suffix.chars().fold((0usize, 0usize), |(chars, utf16), ch| {
                        (chars + 1, utf16 + ch.len_utf16())
                    });
                Some((
                    checkpoint.char_column + char_delta,
                    checkpoint.utf16_column + utf16_delta,
                ))
            }
        }
    }

    #[cfg(test)]
    fn stored_checkpoint_count(&self) -> usize {
        match &self.columns {
            LineColumns::Ascii => 0,
            LineColumns::Unicode { checkpoints } => checkpoints.len(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OffsetMetrics {
    line_index: usize,
    char_column: usize,
    utf16_column: usize,
}

fn span_from_offset_metrics(
    start: usize,
    end: usize,
    start_metrics: OffsetMetrics,
    end_metrics: OffsetMetrics,
) -> DiagnosticSpan {
    DiagnosticSpan::new(
        start..end,
        LineCol::new(start_metrics.line_index + 1, start_metrics.char_column + 1),
        LineCol::new(end_metrics.line_index + 1, end_metrics.char_column + 1),
        LspRange::new(
            Utf16Position {
                line: start_metrics.line_index,
                character: start_metrics.utf16_column,
            },
            Utf16Position {
                line: end_metrics.line_index,
                character: end_metrics.utf16_column,
            },
        ),
    )
}

pub(crate) fn whole_text_span_without_source_copy(text: &str) -> DiagnosticSpan {
    byte_range_span_without_source_copy_with_checkpoint(text, 0..text.len(), || {
        Ok::<_, std::convert::Infallible>(())
    })
    .expect("infallible whole-source span scan")
}

pub(crate) fn whole_text_span_without_source_copy_cancellable(
    text: &str,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<DiagnosticSpan, crate::AnalysisCancelled> {
    byte_range_span_without_source_copy_cancellable(text, 0..text.len(), cancellation)
}

pub(crate) fn byte_range_span_without_source_copy_cancellable(
    text: &str,
    range: Range<usize>,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<DiagnosticSpan, crate::AnalysisCancelled> {
    byte_range_span_without_source_copy_with_checkpoint(text, range, || cancellation.checkpoint())
}

fn byte_range_span_without_source_copy_with_checkpoint<E>(
    text: &str,
    range: Range<usize>,
    mut checkpoint: impl FnMut() -> Result<(), E>,
) -> Result<DiagnosticSpan, E> {
    assert!(range.start <= range.end, "diagnostic range must be ordered");
    assert!(
        text.get(range.clone()).is_some(),
        "diagnostic range must be in bounds and on UTF-8 boundaries"
    );

    let mut metrics = OffsetMetrics {
        line_index: 0,
        char_column: 0,
        utf16_column: 0,
    };
    let mut start_metrics = None;
    let mut end_metrics = None;
    let mut cursor = 0usize;
    let mut bytes_since_checkpoint = 0usize;

    checkpoint()?;
    while cursor <= range.end {
        if cursor == range.start {
            start_metrics = Some(metrics);
        }
        if cursor == range.end {
            end_metrics = Some(metrics);
            break;
        }

        let ch = text[cursor..]
            .chars()
            .next()
            .expect("validated range end must be reachable from the source start");
        bytes_since_checkpoint += ch.len_utf8();
        match ch {
            '\r' => {
                let after_carriage_return = cursor + 1;
                if text.as_bytes().get(after_carriage_return) == Some(&b'\n') {
                    if after_carriage_return == range.start {
                        start_metrics = Some(metrics);
                    }
                    if after_carriage_return == range.end {
                        end_metrics = Some(metrics);
                        break;
                    }
                    cursor += 2;
                    bytes_since_checkpoint += 1;
                } else {
                    cursor += 1;
                }
                metrics.line_index += 1;
                metrics.char_column = 0;
                metrics.utf16_column = 0;
            }
            '\n' => {
                cursor += 1;
                metrics.line_index += 1;
                metrics.char_column = 0;
                metrics.utf16_column = 0;
            }
            _ => {
                cursor += ch.len_utf8();
                metrics.char_column += 1;
                metrics.utf16_column += ch.len_utf16();
            }
        }
        if bytes_since_checkpoint >= 4096 {
            checkpoint()?;
            bytes_since_checkpoint = 0;
        }
    }
    checkpoint()?;

    Ok(span_from_offset_metrics(
        range.start,
        range.end,
        start_metrics.expect("validated range start must be reached"),
        end_metrics.expect("validated range end must be reached"),
    ))
}

fn line_metric_with_checkpoint<E>(
    source: &str,
    start: usize,
    next_start: usize,
    mut checkpoint: impl FnMut() -> Result<(), E>,
) -> Result<LineMetric, E> {
    let content_end = line_content_end(source.as_bytes(), start, next_start);
    let line = &source[start..content_end];
    let mut ascii = true;
    for chunk in line.as_bytes().chunks(4096) {
        checkpoint()?;
        if !chunk.is_ascii() {
            ascii = false;
            break;
        }
    }
    checkpoint()?;
    if ascii {
        return Ok(LineMetric {
            start,
            content_end,
            columns: LineColumns::Ascii,
        });
    }

    let mut checkpoints = vec![ColumnCheckpoint {
        byte_offset: 0,
        char_column: 0,
        utf16_column: 0,
    }];
    let mut utf16_column = 0usize;
    let mut next_column_checkpoint = LINE_COLUMN_CHECKPOINT_BYTES;
    let mut next_cancellation_checkpoint = 0usize;

    for (char_column, (relative, ch)) in line.char_indices().enumerate() {
        if relative >= next_cancellation_checkpoint {
            checkpoint()?;
            next_cancellation_checkpoint = relative.saturating_add(4096);
        }
        if relative >= next_column_checkpoint {
            checkpoints.push(ColumnCheckpoint {
                byte_offset: relative,
                char_column,
                utf16_column,
            });
            next_column_checkpoint = relative.saturating_add(LINE_COLUMN_CHECKPOINT_BYTES);
        }
        utf16_column += ch.len_utf16();
    }
    checkpoint()?;

    Ok(LineMetric {
        start,
        content_end,
        columns: LineColumns::Unicode {
            checkpoints: checkpoints.into_boxed_slice(),
        },
    })
}

fn line_content_end(bytes: &[u8], start: usize, next_start: usize) -> usize {
    let mut end = next_start;
    if end > start && bytes.get(end - 1) == Some(&b'\n') {
        end -= 1;
    }
    if end > start && bytes.get(end - 1) == Some(&b'\r') {
        end -= 1;
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_text_slice_preserves_range_and_source_identity_without_copying() {
        let source = Arc::<str>::from("prefix 🤓 suffix");
        let start = source.find('🤓').unwrap();
        let end = start + '🤓'.len_utf8();
        let slice = SharedTextSlice::from_range(Arc::clone(&source), start, end).unwrap();
        let whole = SharedTextSlice::whole(Arc::clone(&source));

        assert_eq!(slice.as_str(), "🤓");
        assert_eq!(slice.start(), start);
        assert_eq!(slice.end(), end);
        assert!(slice.shares_source_allocation_with(&whole));
        assert!(Arc::ptr_eq(&slice.source_arc(), &source));
    }

    #[test]
    fn shared_text_slice_rejects_invalid_or_non_boundary_ranges() {
        let source = Arc::<str>::from("a🤓b");
        let emoji = source.find('🤓').unwrap();

        assert!(SharedTextSlice::from_range(Arc::clone(&source), 3, 2).is_none());
        assert!(SharedTextSlice::from_range(Arc::clone(&source), 0, source.len() + 1).is_none());
        assert!(
            SharedTextSlice::from_range(Arc::clone(&source), emoji + 1, source.len()).is_none()
        );
        assert!(SharedTextSlice::from_range(source, 0, emoji + 1).is_none());
    }

    #[test]
    fn public_line_queries_cover_empty_terminal_and_mixed_newline_lines() {
        let empty = SourceMap::new("");
        assert_eq!(empty.line_count(), 1);
        assert_eq!(empty.line_start(0), Some(0));
        assert_eq!(empty.line_start(1), None);

        let map = SourceMap::new("a\r\nb\rc\n");
        assert_eq!(map.line_count(), 4);
        assert_eq!(
            (0..map.line_count())
                .map(|line| map.line_start(line).unwrap())
                .collect::<Vec<_>>(),
            vec![0, 3, 5, 7]
        );
    }

    #[test]
    fn maps_ascii_offsets_to_one_based_cli_positions() {
        let map = SourceMap::new("flowchart TD\nA-->B\n");

        assert_eq!(map.line_col(0).unwrap(), LineCol::new(1, 1));
        assert_eq!(map.line_col(13).unwrap(), LineCol::new(2, 1));
        assert_eq!(map.line_col(map.source_len()).unwrap(), LineCol::new(3, 1));
    }

    #[test]
    fn maps_utf8_offsets_to_lsp_utf16_positions() {
        let map = SourceMap::new("flowchart TD\nA[🤓]-->B\n");
        let emoji_start = map.source().find('🤓').unwrap();
        let emoji_end = emoji_start + "🤓".len();
        let after_bracket = emoji_end + 1;

        assert_eq!(
            map.utf16_position(emoji_start).unwrap(),
            Utf16Position {
                line: 1,
                character: 2
            }
        );
        assert_eq!(
            map.utf16_position(after_bracket).unwrap(),
            Utf16Position {
                line: 1,
                character: 5
            }
        );
    }

    #[test]
    fn sparse_unicode_checkpoints_preserve_every_character_boundary() {
        let source = format!(
            "{}é{}🤓{}",
            "a".repeat(LINE_COLUMN_CHECKPOINT_BYTES - 1),
            "β".repeat(LINE_COLUMN_CHECKPOINT_BYTES),
            "z".repeat(LINE_COLUMN_CHECKPOINT_BYTES + 1),
        );
        let map = SourceMap::new(source.clone());
        let mut char_column = 0usize;
        let mut utf16_column = 0usize;

        for (byte_offset, ch) in source.char_indices() {
            assert_eq!(
                map.line_col(byte_offset).unwrap(),
                LineCol::new(1, char_column + 1)
            );
            assert_eq!(
                map.utf16_position(byte_offset).unwrap(),
                Utf16Position {
                    line: 0,
                    character: utf16_column,
                }
            );
            assert_eq!(
                map.byte_offset_for_utf16_position(Utf16Position {
                    line: 0,
                    character: utf16_column,
                }),
                Some(byte_offset)
            );
            for interior_column in 1..ch.len_utf16() {
                assert_eq!(
                    map.byte_offset_for_utf16_position(Utf16Position {
                        line: 0,
                        character: utf16_column + interior_column,
                    }),
                    None
                );
            }
            char_column += 1;
            utf16_column += ch.len_utf16();
        }

        assert_eq!(
            map.line_col(source.len()).unwrap(),
            LineCol::new(1, char_column + 1)
        );
        assert_eq!(
            map.utf16_position(source.len()).unwrap(),
            Utf16Position {
                line: 0,
                character: utf16_column,
            }
        );
        assert_eq!(
            map.byte_offset_for_utf16_position(Utf16Position {
                line: 0,
                character: utf16_column,
            }),
            Some(source.len())
        );
    }

    #[test]
    fn crlf_line_bounds_and_positions_ignore_carriage_return() {
        let source = "flowchart TD\r\nA[🤓]-->B\r\n";
        let map = SourceMap::new(source);
        let first_cr = source.find('\r').unwrap();
        let first_lf = source.find('\n').unwrap();

        assert_eq!(map.line_bounds(0), Some((0, first_cr)));
        assert_eq!(
            map.utf16_position(first_cr).unwrap(),
            Utf16Position {
                line: 0,
                character: "flowchart TD".len(),
            }
        );
        assert_eq!(
            map.utf16_position(first_lf).unwrap(),
            Utf16Position {
                line: 0,
                character: "flowchart TD".len(),
            }
        );
        assert_eq!(
            map.byte_offset_for_utf16_position(Utf16Position {
                line: 0,
                character: "flowchart TD".len(),
            }),
            Some(first_cr)
        );
    }

    #[test]
    fn bare_cr_line_bounds_and_positions_treat_carriage_return_as_line_ending() {
        let source = "flowchart TD\rA-->B\rC-->D";
        let map = SourceMap::new(source);
        let first_cr = source.find('\r').unwrap();
        let second_line_start = first_cr + 1;
        let second_cr = source[second_line_start..].find('\r').unwrap() + second_line_start;

        assert_eq!(map.line_bounds(0), Some((0, first_cr)));
        assert_eq!(map.line_bounds(1), Some((second_line_start, second_cr)));
        assert_eq!(
            map.utf16_position(second_line_start).unwrap(),
            Utf16Position {
                line: 1,
                character: 0,
            }
        );
        assert_eq!(
            map.byte_offset_for_utf16_position(Utf16Position {
                line: 2,
                character: 0,
            }),
            Some(second_cr + 1)
        );
    }

    #[test]
    fn utf16_position_past_line_end_clamps_to_content_end() {
        let source = "flowchart TD\nA[🤓]-->B\n";
        let map = SourceMap::new(source);
        let second_line_start = source.find("A[").unwrap();
        let second_line_end = source[second_line_start..].find('\n').unwrap() + second_line_start;

        assert_eq!(
            map.byte_offset_for_utf16_position(Utf16Position {
                line: 1,
                character: 10_000,
            }),
            Some(second_line_end)
        );
    }

    #[test]
    fn dense_span_conversion_uses_cached_line_metrics() {
        let nodes = (0..512)
            .map(|index| format!("N{index}[🤓]"))
            .collect::<Vec<_>>()
            .join(" ");
        let source = format!("flowchart TD {nodes}");
        let map = SourceMap::new(source.clone());

        assert_eq!(map.cached_line_metric_count(), 0);
        assert_eq!(map.cached_line_checkpoint_count(0), None);

        for offset in source.match_indices('N').map(|(offset, _)| offset) {
            let end = source[offset..].find('[').map(|len| offset + len).unwrap();
            let span = map.span(offset, end).unwrap();
            assert_eq!(span.lsp_range.start.line, 0);
            assert!(span.lsp_range.end.character > span.lsp_range.start.character);
        }
        assert_eq!(map.cached_line_metric_count(), 1);
        let checkpoint_count = map.cached_line_checkpoint_count(0).unwrap();
        assert!(checkpoint_count > 0);
        assert!(checkpoint_count <= source.len() / LINE_COLUMN_CHECKPOINT_BYTES + 2);
    }

    #[test]
    fn dense_line_sources_cache_only_queried_line_metrics() {
        let source = "\n".repeat(100_000);
        let map = SourceMap::new(source);

        assert_eq!(map.line_count(), 100_001);
        assert_eq!(map.cached_line_metric_count(), 0);
        assert_eq!(map.line_bounds(99_999), Some((99_999, 99_999)));
        assert_eq!(map.cached_line_metric_count(), 0);
        assert_eq!(
            map.utf16_position(99_999).unwrap(),
            Utf16Position {
                line: 99_999,
                character: 0,
            }
        );
        assert_eq!(map.cached_line_metric_count(), 1);
    }

    #[test]
    fn line_metric_cache_bounds_dense_ascii_lines() {
        let line_count = 10_000;
        let source = "a\n".repeat(line_count);
        let map = SourceMap::new(source);

        for line_index in 0..line_count {
            let line_end = line_index * 2 + 1;
            assert_eq!(
                map.line_bounds(line_index),
                Some((line_index * 2, line_end))
            );
            assert_eq!(
                map.utf16_position(line_end).unwrap(),
                Utf16Position {
                    line: line_index,
                    character: 1,
                }
            );
        }

        assert!(map.cached_line_metric_bytes() <= SOURCE_MAP_LINE_METRIC_CACHE_BUDGET_BYTES);
        assert!(
            map.estimated_line_metric_cache_allocation_bytes()
                <= SOURCE_MAP_LINE_METRIC_CACHE_BUDGET_BYTES
        );
        assert!(map.cached_line_metric_count() < line_count);
    }

    #[test]
    fn line_metric_cache_bounds_many_short_unicode_lines() {
        let line_count = 10_000;
        let source = "é\n".repeat(line_count);
        let map = SourceMap::new(source);

        for line_index in 0..line_count {
            let line_end = line_index * 3 + 2;
            assert_eq!(
                map.line_bounds(line_index),
                Some((line_index * 3, line_end))
            );
            assert_eq!(
                map.utf16_position(line_end).unwrap(),
                Utf16Position {
                    line: line_index,
                    character: 1,
                }
            );
        }

        assert!(map.cached_line_metric_bytes() <= SOURCE_MAP_LINE_METRIC_CACHE_BUDGET_BYTES);
        assert!(
            map.estimated_line_metric_cache_allocation_bytes()
                <= SOURCE_MAP_LINE_METRIC_CACHE_BUDGET_BYTES
        );
        assert!(map.cached_line_metric_count() < line_count);
    }

    #[test]
    fn oversized_unicode_line_metric_is_correct_but_not_cached() {
        let character_count = 8 * 1024 * 1024;
        let source = "é".repeat(character_count);
        let map = SourceMap::new(source.clone());

        let span = map.span(0, source.len()).unwrap();
        assert_eq!(span.end_line, 1);
        assert_eq!(span.end_column, character_count + 1);
        assert_eq!(span.lsp_range.end.character, character_count);
        assert_eq!(map.cached_line_metric_count(), 0);
        assert_eq!(map.cached_line_metric_bytes(), 0);
    }

    #[test]
    fn line_metric_cache_touch_changes_the_deterministic_victim() {
        let metric = |start| {
            Arc::new(LineMetric {
                start,
                content_end: start,
                columns: LineColumns::Ascii,
            })
        };
        let entry_weight = line_metric_cache_entry_weight(&metric(0));
        let mut cache = LineMetricCache::new(entry_weight * 2);

        cache.insert(0, metric(0));
        cache.insert(1, metric(1));
        assert!(cache.get(0).is_some());
        cache.insert(2, metric(2));

        assert!(cache.entries.contains_key(&0));
        assert!(!cache.entries.contains_key(&1));
        assert!(cache.entries.contains_key(&2));
        assert!(cache.retained_bytes <= cache.budget_bytes);
    }

    #[test]
    fn individually_oversized_line_metric_bypasses_the_cache() {
        let metric = Arc::new(LineMetric {
            start: 0,
            content_end: 0,
            columns: LineColumns::Ascii,
        });
        let mut cache = LineMetricCache::new(line_metric_cache_entry_weight(&metric) - 1);
        let returned = cache.insert(0, Arc::clone(&metric));

        assert!(Arc::ptr_eq(&returned, &metric));
        assert!(cache.entries.is_empty());
        assert_eq!(cache.retained_bytes, 0);
        assert_eq!(cache.statistics().oversized_entries, 1);
    }

    #[test]
    fn line_metric_cache_statistics_cover_lookups_admission_and_high_water() {
        let metric = |start| {
            Arc::new(LineMetric {
                start,
                content_end: start,
                columns: LineColumns::Ascii,
            })
        };
        let entry_weight = line_metric_cache_entry_weight(&metric(0));
        let mut cache = LineMetricCache::new(entry_weight * 2);

        assert!(cache.get(0).is_none());
        cache.insert(0, metric(0));
        assert!(cache.get(0).is_some());
        cache.insert(1, metric(1));
        cache.insert(2, metric(2));
        let oversized = Arc::new(LineMetric {
            start: 3,
            content_end: 3,
            columns: LineColumns::Unicode {
                checkpoints: vec![
                    ColumnCheckpoint {
                        byte_offset: 0,
                        char_column: 0,
                        utf16_column: 0,
                    };
                    entry_weight
                ]
                .into_boxed_slice(),
            },
        });
        cache.insert(3, oversized);

        assert_eq!(
            cache.statistics(),
            LineMetricCacheStatistics {
                hits: 1,
                misses: 1,
                evictions: 1,
                oversized_entries: 1,
                current_weight: entry_weight * 2,
                high_water_weight: entry_weight * 2,
            }
        );
    }

    #[test]
    fn mixed_metric_sizes_cannot_retain_historical_container_capacity_past_the_budget() {
        let ascii_metric = |start| {
            Arc::new(LineMetric {
                start,
                content_end: start,
                columns: LineColumns::Ascii,
            })
        };
        let mut cache = LineMetricCache::new(SOURCE_MAP_LINE_METRIC_CACHE_BUDGET_BYTES);
        for line_index in 0..4096 {
            cache.insert(line_index, ascii_metric(line_index));
        }

        let checkpoint_count =
            SOURCE_MAP_LINE_METRIC_CACHE_BUDGET_BYTES / 2 / size_of::<ColumnCheckpoint>();
        let large = Arc::new(LineMetric {
            start: 4096,
            content_end: 4096,
            columns: LineColumns::Unicode {
                checkpoints: vec![
                    ColumnCheckpoint {
                        byte_offset: 0,
                        char_column: 0,
                        utf16_column: 0,
                    };
                    checkpoint_count
                ]
                .into_boxed_slice(),
            },
        });
        cache.insert(4096, large);

        assert!(cache.retained_bytes <= cache.budget_bytes);
        assert!(cache.estimated_allocation_bytes() <= cache.budget_bytes);
    }

    #[test]
    fn cancelled_line_metric_commit_preserves_residents_and_recency() {
        let map = SourceMap::new("a\nb");
        assert_eq!(
            map.utf16_position(1).unwrap(),
            Utf16Position {
                line: 0,
                character: 1,
            }
        );
        let before = {
            let cache = map.line_metric_cache();
            (
                cache.retained_bytes,
                cache.oldest,
                cache.newest,
                cache.entries.len(),
            )
        };
        let mut checkpoints = 0;

        let result = map.line_metric_with_checkpoint(1, || {
            checkpoints += 1;
            if checkpoints == 4 { Err(()) } else { Ok(()) }
        });

        assert!(matches!(result, Err(())));
        let cache = map.line_metric_cache();
        assert_eq!(
            (
                cache.retained_bytes,
                cache.oldest,
                cache.newest,
                cache.entries.len(),
            ),
            before
        );
        assert!(cache.entries.contains_key(&0));
        assert!(!cache.entries.contains_key(&1));
    }

    #[test]
    fn evicted_metric_lives_only_while_a_request_holds_it() {
        let metric = |start| {
            Arc::new(LineMetric {
                start,
                content_end: start,
                columns: LineColumns::Ascii,
            })
        };
        let entry_weight = line_metric_cache_entry_weight(&metric(0));
        let mut cache = LineMetricCache::new(entry_weight);
        let request_local = cache.insert(0, metric(0));
        let evicted = Arc::downgrade(&request_local);

        cache.insert(1, metric(1));

        assert!(evicted.upgrade().is_some());
        assert!(!cache.entries.contains_key(&0));
        drop(request_local);
        assert!(evicted.upgrade().is_none());
    }

    #[test]
    fn source_map_clones_share_one_line_metric_budget() {
        let map = SourceMap::new("a\né");
        let clone = map.clone();

        assert!(Arc::ptr_eq(&map.line_index, &clone.line_index));

        assert_eq!(
            clone.utf16_position(4).unwrap(),
            Utf16Position {
                line: 1,
                character: 1,
            }
        );
        assert_eq!(map.cached_line_metric_count(), 1);
        assert_eq!(
            map.cached_line_metric_bytes(),
            clone.cached_line_metric_bytes()
        );
    }

    #[test]
    fn retained_weight_excludes_source_and_reserves_the_full_metric_budget() {
        let short = SourceMap::new("a");
        let long = SourceMap::new("a".repeat(1024 * 1024));
        assert_eq!(
            short.estimated_owned_heap_bytes_excluding_source(),
            long.estimated_owned_heap_bytes_excluding_source()
        );

        let before = long.estimated_owned_heap_bytes_excluding_source();
        assert_eq!(
            long.utf16_position(long.source_len()).unwrap(),
            Utf16Position {
                line: 0,
                character: long.source_len(),
            }
        );
        assert_eq!(long.estimated_owned_heap_bytes_excluding_source(), before);

        let many_lines = SourceMap::new("\n".repeat(1024));
        assert!(many_lines.estimated_owned_heap_bytes_excluding_source() > before);
    }

    #[test]
    fn concurrent_same_line_misses_are_admitted_once() {
        let source = format!("{}é", "a".repeat(32 * 1024));
        let end = source.len();
        let map = SourceMap::new(source);
        let barrier = Arc::new(std::sync::Barrier::new(8));
        let handles = (0..8)
            .map(|_| {
                let map = map.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    map.utf16_position(end).unwrap()
                })
            })
            .collect::<Vec<_>>();

        for handle in handles {
            assert_eq!(handle.join().unwrap().line, 0);
        }
        assert_eq!(map.cached_line_metric_count(), 1);
        assert!(map.cached_line_metric_bytes() <= SOURCE_MAP_LINE_METRIC_CACHE_BUDGET_BYTES);
    }

    #[test]
    fn long_ascii_lines_use_constant_size_column_metrics() {
        let source = "a".repeat(64 * 1024);
        let map = SourceMap::new(source.clone());

        assert_eq!(
            map.utf16_position(source.len()).unwrap(),
            Utf16Position {
                line: 0,
                character: source.len(),
            }
        );
        assert_eq!(map.cached_line_checkpoint_count(0), Some(0));
        assert_eq!(
            map.byte_offset_for_utf16_position(Utf16Position {
                line: 0,
                character: source.len() + 1,
            }),
            Some(source.len())
        );

        let non_initial_line = SourceMap::new("x\nabc");
        assert_eq!(
            non_initial_line.byte_offset_for_utf16_position(Utf16Position {
                line: 1,
                character: usize::MAX,
            }),
            Some(5)
        );
    }

    #[test]
    fn long_mostly_ascii_unicode_lines_use_sparse_column_checkpoints() {
        let prefix_len = 64 * 1024 - 1;
        let suffix_len = 64 * 1024;
        let source = format!("{}🤓{}", "a".repeat(prefix_len), "b".repeat(suffix_len));
        let emoji_start = prefix_len;
        let emoji_end = emoji_start + '🤓'.len_utf8();
        let map = SourceMap::new(source.clone());

        assert_eq!(
            map.line_col(emoji_start).unwrap(),
            LineCol::new(1, prefix_len + 1)
        );
        assert_eq!(
            map.line_col(emoji_end).unwrap(),
            LineCol::new(1, prefix_len + 2)
        );
        assert_eq!(
            map.utf16_position(emoji_end).unwrap(),
            Utf16Position {
                line: 0,
                character: prefix_len + 2,
            }
        );
        assert_eq!(
            map.byte_offset_for_utf16_position(Utf16Position {
                line: 0,
                character: prefix_len + 1,
            }),
            None
        );
        assert_eq!(
            map.byte_offset_for_utf16_position(Utf16Position {
                line: 0,
                character: prefix_len + 2,
            }),
            Some(emoji_end)
        );
        assert_eq!(
            map.utf16_position(source.len()).unwrap(),
            Utf16Position {
                line: 0,
                character: prefix_len + 2 + suffix_len,
            }
        );

        let checkpoint_count = map.cached_line_checkpoint_count(0).unwrap();
        assert!(checkpoint_count <= source.len() / LINE_COLUMN_CHECKPOINT_BYTES + 2);
        assert!(checkpoint_count * 100 < source.chars().count());
    }

    #[test]
    fn dense_line_scan_observes_scheduled_cancellation() {
        let source = "\n".repeat(32 * 1024);
        let cancellation = crate::AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(2);

        assert!(matches!(
            SourceMap::new_cancellable(source, &cancellation),
            Err(crate::AnalysisCancelled)
        ));
        assert!(cancellation.is_cancelled());
    }

    #[test]
    fn long_single_line_metric_observes_scheduled_cancellation() {
        let source = "a".repeat(32 * 1024);
        let map = SourceMap::new(source);
        let cancellation = crate::AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(2);

        assert!(matches!(
            map.whole_source_span_cancellable(&cancellation),
            Err(crate::AnalysisCancelled)
        ));
        assert!(cancellation.is_cancelled());
        assert_eq!(map.cached_line_metric_count(), 0);
    }

    #[test]
    fn rejects_non_char_boundary_offsets() {
        let map = SourceMap::new("flowchart TD\nA[🤓]\n");
        let inside_emoji = map.source().find('🤓').unwrap() + 1;

        assert_eq!(
            map.line_col(inside_emoji).unwrap_err(),
            SourceMapError::OffsetNotCharBoundary {
                offset: inside_emoji
            }
        );
    }

    #[test]
    fn builds_diagnostic_span_with_cli_and_lsp_positions() {
        let map = SourceMap::new("flowchart TD\nA[🤓]-->B\n");
        let start = map.source().find('A').unwrap();
        let end = map.source().find("-->").unwrap();
        let span = map.span(start, end).unwrap();

        assert_eq!(span.byte_start, start);
        assert_eq!(span.byte_end, end);
        assert_eq!(span.line, 2);
        assert_eq!(span.column, 1);
        assert_eq!(span.end_line, 2);
        assert_eq!(span.end_column, 5);
        assert_eq!(span.lsp_range.start.line, 1);
        assert_eq!(span.lsp_range.start.character, 0);
        assert_eq!(span.lsp_range.end.character, 5);
    }

    #[test]
    fn whole_text_span_without_source_copy_matches_source_map_span() {
        for source in [
            "flowchart TD\nA[🤓]-->B\n",
            "flowchart TD\r\nA[🤓]-->B",
            "flowchart TD\r\nA[🤓]-->B\r",
            "flowchart TD\r\r\nA[🤓]-->B",
        ] {
            assert_eq!(
                whole_text_span_without_source_copy(source),
                SourceMap::new(source).whole_source_span().unwrap()
            );
        }
    }

    #[test]
    fn whole_text_span_without_source_copy_observes_scheduled_cancellation() {
        let source = "a".repeat(32 * 1024);
        let cancellation = crate::AnalysisCancellationToken::new();
        cancellation.cancel_after_checkpoints(2);

        assert!(matches!(
            whole_text_span_without_source_copy_cancellable(&source, &cancellation),
            Err(crate::AnalysisCancelled)
        ));
        assert!(cancellation.is_cancelled());
    }

    #[test]
    fn byte_range_span_without_source_copy_matches_source_map_span() {
        let source = "intro 🤓\r\n  ```mermaid\nflowchart TD\nA-->B\n```\n";
        let start = source.find("```").unwrap();
        let end = start + 3;
        let cancellation = crate::AnalysisCancellationToken::new();

        assert_eq!(
            byte_range_span_without_source_copy_cancellable(source, start..end, &cancellation,)
                .unwrap(),
            SourceMap::new(source).span(start, end).unwrap()
        );
    }

    #[test]
    fn generated_mixed_newline_sources_match_the_one_pass_mapping_oracle() {
        fn line_starts(source: &str) -> Vec<usize> {
            let mut starts = vec![0];
            let bytes = source.as_bytes();
            let mut offset = 0usize;
            while offset < bytes.len() {
                match bytes[offset] {
                    b'\r' => {
                        offset += 1;
                        if bytes.get(offset) == Some(&b'\n') {
                            offset += 1;
                        }
                        starts.push(offset);
                    }
                    b'\n' => {
                        offset += 1;
                        starts.push(offset);
                    }
                    _ => offset += 1,
                }
            }
            starts
        }

        let atoms = ["a", "\r", "\n", "é", "🤓"];
        let mut sources = vec![String::new()];
        let mut frontier = vec![String::new()];
        for _ in 0..5 {
            let mut next = Vec::with_capacity(frontier.len() * atoms.len());
            for prefix in &frontier {
                for atom in atoms {
                    let mut source = prefix.clone();
                    source.push_str(atom);
                    next.push(source);
                }
            }
            sources.extend(next.iter().cloned());
            frontier = next;
        }

        for source in sources {
            let map = SourceMap::new(source.as_str());
            let starts = line_starts(&source);
            assert_eq!(map.line_count(), starts.len(), "source {source:?}");
            for (line, &start) in starts.iter().enumerate() {
                assert_eq!(map.line_start(line), Some(start), "source {source:?}");
                let next_start = starts.get(line + 1).copied().unwrap_or(source.len());
                assert_eq!(
                    map.line_bounds(line),
                    Some((
                        start,
                        line_content_end(source.as_bytes(), start, next_start),
                    )),
                    "source {source:?}"
                );
            }
            assert_eq!(map.line_start(starts.len()), None, "source {source:?}");
            assert_eq!(map.line_bounds(starts.len()), None, "source {source:?}");

            let mut boundaries = source
                .char_indices()
                .map(|(offset, _)| offset)
                .collect::<Vec<_>>();
            boundaries.push(source.len());
            boundaries.dedup();
            for (start_index, &start) in boundaries.iter().enumerate() {
                let point = byte_range_span_without_source_copy_with_checkpoint(
                    &source,
                    start..start,
                    || Ok::<_, std::convert::Infallible>(()),
                )
                .expect("oracle scan is infallible");
                assert_eq!(
                    map.line_col(start).unwrap(),
                    LineCol::new(point.line, point.column),
                    "source {source:?}, offset {start}"
                );
                assert_eq!(
                    map.utf16_position(start).unwrap(),
                    point.lsp_range.start,
                    "source {source:?}, offset {start}"
                );
                let content_end = map.line_bounds(point.lsp_range.start.line).unwrap().1;
                assert_eq!(
                    map.byte_offset_for_utf16_position(point.lsp_range.start),
                    Some(start.min(content_end)),
                    "source {source:?}, offset {start}"
                );

                for &end in &boundaries[start_index..] {
                    let expected = byte_range_span_without_source_copy_with_checkpoint(
                        &source,
                        start..end,
                        || Ok::<_, std::convert::Infallible>(()),
                    )
                    .expect("oracle scan is infallible");
                    assert_eq!(
                        map.span(start, end).unwrap(),
                        expected,
                        "source {source:?}, range {start}..{end}"
                    );
                }
            }

            let expected_whole = byte_range_span_without_source_copy_with_checkpoint(
                &source,
                0..source.len(),
                || Ok::<_, std::convert::Infallible>(()),
            )
            .expect("oracle scan is infallible");
            assert_eq!(map.whole_source_span().unwrap(), expected_whole);
        }
    }
}
