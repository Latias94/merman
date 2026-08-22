use super::normalization::{NormalizedSegmentKind, visit_normalized_segments};
use super::width::grapheme_display_width;
use crate::Result;
use crate::error::AsciiError;
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use merman_core::entities::{DecodedHtmlFragment, visit_decoded_html_entity_fragments};
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ComposedTextMetrics {
    materialized_bytes: usize,
    materialization_work_units: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DeferredTextMetrics {
    display_width: usize,
}

impl DeferredTextMetrics {
    pub(crate) const fn display_width(self) -> usize {
        self.display_width
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DeferredTextPiece {
    source_start: usize,
    source_end: usize,
    fragment_start: usize,
    fragment_end: usize,
    output_index: u32,
    display_width: u8,
    plain_bytes: usize,
    html_bytes: usize,
    replay_work_units: usize,
}

impl DeferredTextPiece {
    pub(crate) const fn display_width(self) -> usize {
        self.display_width as usize
    }

    pub(crate) const fn replay_work_units(self) -> usize {
        self.replay_work_units
    }

    pub(crate) const fn encoded_bytes(self, html: bool) -> usize {
        if html {
            self.html_bytes
        } else {
            self.plain_bytes
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct TextFragment<'a> {
    start: usize,
    value: &'a str,
}

#[derive(Debug, Clone, Copy)]
enum ReplayFragment<'a> {
    Borrowed(TextFragment<'a>),
    Scalar { start: usize, value: char },
}

impl ReplayFragment<'_> {
    const fn start(self) -> usize {
        match self {
            Self::Borrowed(fragment) => fragment.start,
            Self::Scalar { start, .. } => start,
        }
    }

    fn len(self) -> usize {
        match self {
            Self::Borrowed(fragment) => fragment.value.len(),
            Self::Scalar { value, .. } => value.len_utf8(),
        }
    }

    fn try_visit_range<T>(
        self,
        start: usize,
        end: usize,
        visit: impl FnOnce(&str) -> Result<T>,
    ) -> Result<T> {
        let local_start = start
            .checked_sub(self.start())
            .ok_or_else(invalid_composed_text_plan)?;
        let local_end = end
            .checked_sub(self.start())
            .ok_or_else(invalid_composed_text_plan)?;
        match self {
            Self::Borrowed(fragment) => visit(
                fragment
                    .value
                    .get(local_start..local_end)
                    .ok_or_else(invalid_composed_text_plan)?,
            ),
            Self::Scalar { value, .. } => {
                let mut buffer = [0u8; 4];
                visit(
                    value
                        .encode_utf8(&mut buffer)
                        .get(local_start..local_end)
                        .ok_or_else(invalid_composed_text_plan)?,
                )
            }
        }
    }

    fn append_range(self, start: usize, end: usize, output: &mut String) -> Result<()> {
        self.try_visit_range(start, end, |value| {
            output.push_str(value);
            Ok(())
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FragmentSpan {
    start: usize,
    end: usize,
}

impl FragmentSpan {
    const fn len(self) -> usize {
        self.end - self.start
    }
}

/// Replays monotonically increasing logical ranges without restarting the fragment scan.
struct ComposedRangeCursor<'fragments, 'text> {
    fragments: &'fragments [ReplayFragment<'text>],
    next_fragment: usize,
    buffer: String,
}

impl<'fragments, 'text> ComposedRangeCursor<'fragments, 'text> {
    const fn new(fragments: &'fragments [ReplayFragment<'text>]) -> Self {
        Self {
            fragments,
            next_fragment: 0,
            buffer: String::new(),
        }
    }

    fn try_visit_next<T>(
        &mut self,
        start: usize,
        end: usize,
        visit: impl FnOnce(&str, FragmentSpan) -> Result<T>,
    ) -> Result<T> {
        let expected_bytes = end
            .checked_sub(start)
            .filter(|bytes| *bytes > 0)
            .ok_or_else(invalid_composed_text_plan)?;

        while self.next_fragment < self.fragments.len() {
            let fragment = self.fragments[self.next_fragment];
            let fragment_end = fragment
                .start()
                .checked_add(fragment.len())
                .ok_or_else(invalid_composed_text_plan)?;
            if fragment_end > start {
                break;
            }
            self.next_fragment += 1;
        }

        let first = self
            .fragments
            .get(self.next_fragment)
            .copied()
            .ok_or_else(invalid_composed_text_plan)?;
        let first_end = first
            .start()
            .checked_add(first.len())
            .ok_or_else(invalid_composed_text_plan)?;
        if first.start() > start {
            return Err(invalid_composed_text_plan());
        }

        if end <= first_end {
            let span_end = self
                .next_fragment
                .checked_add(1)
                .ok_or_else(invalid_composed_text_plan)?;
            let span = FragmentSpan {
                start: self.next_fragment,
                end: span_end,
            };
            if end == first_end {
                self.next_fragment = span.end;
            }
            return first.try_visit_range(start, end, |value| {
                if value.len() != expected_bytes {
                    return Err(invalid_composed_text_plan());
                }
                visit(value, span)
            });
        }

        self.buffer.clear();
        self.buffer
            .try_reserve_exact(expected_bytes)
            .map_err(|_| layout_allocation_failed())?;
        let span_start = self.next_fragment;
        let mut span_end = span_start;
        let mut last_end = first_end;
        while let Some(fragment) = self.fragments.get(span_end).copied() {
            let fragment_start = fragment.start();
            let fragment_end = fragment_start
                .checked_add(fragment.len())
                .ok_or_else(invalid_composed_text_plan)?;
            let kept_start = start.max(fragment_start);
            let kept_end = end.min(fragment_end);
            if kept_start < kept_end {
                fragment.append_range(kept_start, kept_end, &mut self.buffer)?;
            }
            span_end = span_end
                .checked_add(1)
                .ok_or_else(invalid_composed_text_plan)?;
            last_end = fragment_end;
            if fragment_end >= end {
                break;
            }
        }
        if last_end < end {
            return Err(invalid_composed_text_plan());
        }
        if self.buffer.len() != expected_bytes {
            return Err(invalid_composed_text_plan());
        }
        let span = FragmentSpan {
            start: span_start,
            end: span_end,
        };
        self.next_fragment = if last_end == end {
            span.end
        } else {
            span.end - 1
        };
        visit(self.buffer.as_str(), span)
    }
}

/// A borrowed plan for text assembled from family-owned fields and separators.
///
/// The fragment producer is replayed once to measure and once to retain borrowed slices. The
/// complete logical byte stream is segmented through a bounded streaming buffer that retains only
/// the unresolved grapheme and one lookahead scalar. This keeps planning linear while ensuring
/// that a combining mark or joiner at a fragment boundary cannot bypass the final grapheme limit.
/// The plan retains only borrowed fragments; final output is copied only after the caller admits
/// `materialization_work_units` and calls [`Self::materialize_after_admission`].
#[derive(Debug)]
pub(crate) struct ComposedTextPlan<'a> {
    fragments: Vec<ReplayFragment<'a>>,
    grapheme_boundaries: Vec<usize>,
    metrics: ComposedTextMetrics,
}

impl<'a> ComposedTextPlan<'a> {
    pub(crate) fn try_new(
        resources: &ResourceContext,
        producer_work_per_pass: usize,
        produce: impl Fn(&mut dyn FnMut(&'a str) -> Result<()>) -> Result<()>,
    ) -> Result<Self> {
        resources.transaction(|resources| {
            let mut fragment_count = 0usize;
            let mut materialized_bytes = 0usize;
            produce(&mut |fragment| {
                if fragment.is_empty() {
                    return Ok(());
                }
                fragment_count = checked_work_add(resources, fragment_count, 1)?;
                materialized_bytes =
                    checked_output_add(resources, materialized_bytes, fragment.len())?;
                Ok(())
            })?;

            resources.check(AsciiResourceLimitId::MaxOutputBytes, materialized_bytes)?;
            let producer_work = resources.checked_work_mul(producer_work_per_pass, 2)?;
            let fragment_work = resources.checked_work_mul(fragment_count, 3)?;
            let planning_work = resources.checked_work_add(
                resources.checked_work_add(producer_work, fragment_work)?,
                materialized_bytes.max(1),
            )?;
            let materialization_work_units =
                resources.checked_work_add(materialized_bytes.max(1), fragment_count.max(1))?;
            resources.check_usage(
                resources.checked_work_add(planning_work, materialization_work_units)?,
                0,
            )?;

            let mut fragments = Vec::new();
            fragments
                .try_reserve_exact(fragment_count)
                .map_err(|_| layout_allocation_failed())?;
            let mut collected_bytes = 0usize;
            produce(&mut |fragment| {
                if fragment.is_empty() {
                    return Ok(());
                }
                let start = collected_bytes;
                collected_bytes = checked_output_add(resources, collected_bytes, fragment.len())?;
                fragments.push(ReplayFragment::Borrowed(TextFragment {
                    start,
                    value: fragment,
                }));
                Ok(())
            })?;
            if fragments.len() != fragment_count || collected_bytes != materialized_bytes {
                return Err(invalid_composed_text_plan());
            }

            let grapheme_boundaries =
                plan_grapheme_boundaries(&fragments, materialized_bytes, resources)?;
            resources.charge_usage(planning_work, 0)?;
            Ok(Self {
                fragments,
                grapheme_boundaries,
                metrics: ComposedTextMetrics {
                    materialized_bytes,
                    materialization_work_units,
                },
            })
        })
    }

    pub(crate) fn try_new_html_decoded(
        input: &'a str,
        resources: &ResourceContext,
    ) -> Result<Self> {
        resources.transaction(|resources| {
            let scan_work = input.len().max(1);
            let mut fragment_count = 0usize;
            let mut materialized_bytes = 0usize;
            visit_decoded_html_entity_fragments(input, |fragment| {
                let bytes = decoded_html_fragment_len(fragment);
                if bytes == 0 {
                    return Ok::<(), AsciiError>(());
                }
                fragment_count = checked_work_add(resources, fragment_count, 1)?;
                materialized_bytes = checked_output_add(resources, materialized_bytes, bytes)?;
                Ok::<(), AsciiError>(())
            })?;
            resources.check(AsciiResourceLimitId::MaxOutputBytes, materialized_bytes)?;
            let producer_work = resources.checked_work_mul(scan_work, 2)?;
            let fragment_work = resources.checked_work_mul(fragment_count, 3)?;
            let planning_work = resources.checked_work_add(
                resources.checked_work_add(producer_work, fragment_work)?,
                materialized_bytes.max(1),
            )?;
            let materialization_work_units =
                resources.checked_work_add(materialized_bytes.max(1), fragment_count.max(1))?;
            resources.check_usage(
                resources.checked_work_add(planning_work, materialization_work_units)?,
                0,
            )?;

            let mut fragments = Vec::new();
            fragments
                .try_reserve_exact(fragment_count)
                .map_err(|_| layout_allocation_failed())?;
            let mut collected_bytes = 0usize;
            visit_decoded_html_entity_fragments(input, |fragment| {
                let bytes = decoded_html_fragment_len(fragment);
                if bytes == 0 {
                    return Ok::<(), AsciiError>(());
                }
                let start = collected_bytes;
                collected_bytes = checked_output_add(resources, collected_bytes, bytes)?;
                fragments.push(decoded_html_replay_fragment(fragment, start));
                Ok::<(), AsciiError>(())
            })?;
            if fragments.len() != fragment_count || collected_bytes != materialized_bytes {
                return Err(invalid_composed_text_plan());
            }

            let grapheme_boundaries =
                plan_grapheme_boundaries(&fragments, materialized_bytes, resources)?;
            resources.charge_usage(planning_work, 0)?;
            Ok(Self {
                fragments,
                grapheme_boundaries,
                metrics: ComposedTextMetrics {
                    materialized_bytes,
                    materialization_work_units,
                },
            })
        })
    }

    pub(crate) fn try_deferred_pieces(
        &self,
        profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<(Vec<DeferredTextPiece>, DeferredTextMetrics)> {
        resources
            .transaction(|resources| self.try_deferred_pieces_transactional(profile, resources))
    }

    /// Reports whether the logical source retained by this plan differs from one authored field.
    ///
    /// HTML-decoded plans use borrowed and scalar fragments rather than retaining a second owned
    /// string. Comparing that stream here lets a caller disclose the authored UTF-8 bytes without
    /// materializing the decoded value first.
    pub(crate) fn source_differs_from(
        &self,
        authored: &str,
        resources: &ResourceContext,
    ) -> Result<bool> {
        resources.transaction(|resources| {
            let scan_work = resources.checked_work_add(
                resources.checked_work_add(
                    self.fragments.len().max(1),
                    self.metrics.materialized_bytes.max(1),
                )?,
                authored.len().max(1),
            )?;
            resources.check_usage(scan_work, 0)?;

            let mut logical_offset = 0usize;
            let mut differs = self.metrics.materialized_bytes != authored.len();
            for (iteration, fragment) in self.fragments.iter().copied().enumerate() {
                if iteration.is_multiple_of(64) {
                    resources.checkpoint()?;
                }
                if fragment.start() != logical_offset {
                    return Err(invalid_composed_text_plan());
                }
                let fragment_end = fragment
                    .start()
                    .checked_add(fragment.len())
                    .ok_or_else(invalid_composed_text_plan)?;
                fragment.try_visit_range(fragment.start(), fragment_end, |value| {
                    if !differs && authored.get(logical_offset..fragment_end) != Some(value) {
                        differs = true;
                    }
                    Ok(())
                })?;
                logical_offset = fragment_end;
            }
            if logical_offset != self.metrics.materialized_bytes {
                return Err(invalid_composed_text_plan());
            }

            resources.charge_usage(scan_work, 0)?;
            Ok(differs)
        })
    }

    fn try_deferred_pieces_transactional(
        &self,
        profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<(Vec<DeferredTextPiece>, DeferredTextMetrics)> {
        let range_scan_work = self.deferred_range_scan_work_units(resources)?;
        // The first pass may retain one cross-fragment grapheme in `ranges.buffer`. Admit its
        // complete additive structural scan before the first collection or temporary allocation.
        resources.check_usage(range_scan_work, 0)?;

        let (piece_count, display_width) = self.measure_deferred_pieces(profile, resources)?;
        let planning_work = self.deferred_planning_work(resources, range_scan_work, piece_count)?;
        resources.check_usage(planning_work, 0)?;

        let pieces = self.collect_deferred_pieces(profile, resources, piece_count)?;
        resources.charge_usage(planning_work, 0)?;
        Ok((pieces, DeferredTextMetrics { display_width }))
    }

    fn measure_deferred_pieces(
        &self,
        profile: TerminalWidthProfile,
        resources: &ResourceContext,
    ) -> Result<(usize, usize)> {
        let mut piece_count = 0usize;
        let mut display_width = 0usize;
        let mut source_start = 0usize;
        let mut ranges = ComposedRangeCursor::new(&self.fragments);
        for &source_end in &self.grapheme_boundaries {
            ranges.try_visit_next(source_start, source_end, |grapheme, _| {
                visit_normalized_output_graphemes(grapheme, profile, |_, width| {
                    piece_count = resources.checked_work_add(piece_count, 1)?;
                    display_width = resources.checked_grid_add(display_width, width)?;
                    Ok(())
                })
            })?;
            source_start = source_end;
        }
        Ok((piece_count, display_width))
    }

    fn deferred_planning_work(
        &self,
        resources: &ResourceContext,
        range_scan_work: usize,
        piece_count: usize,
    ) -> Result<usize> {
        resources.checked_work_mul(
            resources.checked_work_add(range_scan_work, piece_count.max(1))?,
            2,
        )
    }

    fn collect_deferred_pieces(
        &self,
        profile: TerminalWidthProfile,
        resources: &ResourceContext,
        piece_count: usize,
    ) -> Result<Vec<DeferredTextPiece>> {
        let mut pieces = Vec::new();
        pieces
            .try_reserve_exact(piece_count)
            .map_err(|_| layout_allocation_failed())?;
        let mut ranges = ComposedRangeCursor::new(&self.fragments);
        let mut source_start = 0usize;
        for &source_end in &self.grapheme_boundaries {
            let mut output_index = 0u32;
            ranges.try_visit_next(source_start, source_end, |grapheme, span| {
                visit_normalized_output_graphemes(grapheme, profile, |value, width| {
                    pieces.push(DeferredTextPiece {
                        source_start,
                        source_end,
                        fragment_start: span.start,
                        fragment_end: span.end,
                        output_index,
                        display_width: u8::try_from(width)
                            .map_err(|_| invalid_composed_text_plan())?,
                        plain_bytes: value.len(),
                        html_bytes: encoded_html_bytes(resources, value)?,
                        replay_work_units: resources.checked_work_add(
                            span.len().max(1),
                            source_end
                                .checked_sub(source_start)
                                .ok_or_else(invalid_composed_text_plan)?,
                        )?,
                    });
                    output_index = output_index
                        .checked_add(1)
                        .ok_or_else(invalid_composed_text_plan)?;
                    Ok(())
                })
            })?;
            source_start = source_end;
        }
        if pieces.len() != piece_count {
            return Err(invalid_composed_text_plan());
        }
        Ok(pieces)
    }

    fn deferred_range_scan_work_units(&self, resources: &ResourceContext) -> Result<usize> {
        Ok(resources
            .checked_work_add(
                resources.checked_work_add(self.fragments.len(), self.grapheme_boundaries.len())?,
                self.metrics.materialized_bytes,
            )?
            .max(1))
    }

    #[cfg(test)]
    fn try_deferred_pieces_with_probe(
        &self,
        profile: TerminalWidthProfile,
        resources: &ResourceContext,
        phase_probe: &std::cell::Cell<usize>,
    ) -> Result<(Vec<DeferredTextPiece>, DeferredTextMetrics)> {
        resources.transaction(|resources| {
            let range_scan_work = self.deferred_range_scan_work_units(resources)?;
            resources.check_usage(range_scan_work, 0)?;

            phase_probe.set(phase_probe.get() + 1);
            let (piece_count, display_width) = self.measure_deferred_pieces(profile, resources)?;
            let planning_work =
                self.deferred_planning_work(resources, range_scan_work, piece_count)?;
            resources.check_usage(planning_work, 0)?;

            phase_probe.set(phase_probe.get() + 1);
            let pieces = self.collect_deferred_pieces(profile, resources, piece_count)?;
            resources.charge_usage(planning_work, 0)?;
            Ok((pieces, DeferredTextMetrics { display_width }))
        })
    }

    pub(crate) fn try_visit_deferred_piece(
        &self,
        profile: TerminalWidthProfile,
        piece: DeferredTextPiece,
        mut visit: impl FnMut(&str) -> Result<()>,
    ) -> Result<()> {
        let fragments = self
            .fragments
            .get(piece.fragment_start..piece.fragment_end)
            .ok_or_else(invalid_composed_text_plan)?;
        let mut ranges = ComposedRangeCursor::new(fragments);
        let mut output_index = 0u32;
        let mut found = false;
        ranges.try_visit_next(piece.source_start, piece.source_end, |grapheme, _| {
            visit_normalized_output_graphemes(grapheme, profile, |value, _| {
                if output_index == piece.output_index {
                    visit(value)?;
                    found = true;
                }
                output_index = output_index
                    .checked_add(1)
                    .ok_or_else(invalid_composed_text_plan)?;
                Ok(())
            })
        })?;
        if !found {
            return Err(invalid_composed_text_plan());
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) const fn materialized_bytes(&self) -> usize {
        self.metrics.materialized_bytes
    }

    #[cfg(test)]
    pub(crate) const fn materialization_work_units(&self) -> usize {
        self.metrics.materialization_work_units
    }

    #[cfg(test)]
    pub(crate) fn materialize(
        self,
        resources: &ResourceContext,
        before_materialize: impl FnOnce(),
    ) -> Result<String> {
        resources.transaction(|resources| {
            resources.check_usage(self.metrics.materialization_work_units, 0)?;
            resources.charge_usage(self.metrics.materialization_work_units, 0)?;
            self.materialize_after_admission(before_materialize)
        })
    }

    #[cfg(test)]
    pub(crate) fn materialize_after_admission(
        self,
        before_materialize: impl FnOnce(),
    ) -> Result<String> {
        before_materialize();
        collect_composed_text(&self.fragments, self.metrics.materialized_bytes)
    }
}

fn plan_grapheme_boundaries(
    fragments: &[ReplayFragment<'_>],
    materialized_bytes: usize,
    resources: &ResourceContext,
) -> Result<Vec<usize>> {
    if materialized_bytes == 0 {
        return Ok(Vec::new());
    }

    // Repeat the local byte/work checks so the bounded streaming scratch cannot be separated from
    // admission by a future refactor. The scratch holds one unresolved grapheme plus at most one
    // lookahead scalar; it never materializes the complete logical stream.
    resources.check(AsciiResourceLimitId::MaxOutputBytes, materialized_bytes)?;
    resources.check_usage(materialized_bytes.max(1), 0)?;

    let mut planner =
        GraphemeBoundaryPlanner::try_new(materialized_bytes, fragments.len().max(1), resources)?;
    let mut visited_bytes = 0usize;
    for fragment in fragments.iter().copied() {
        if fragment.start() != visited_bytes {
            return Err(invalid_composed_text_plan());
        }
        let fragment_end = fragment
            .start()
            .checked_add(fragment.len())
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
        fragment.try_visit_range(fragment.start(), fragment_end, |value| {
            planner.try_push_str(value)
        })?;
        visited_bytes = fragment_end;
    }
    if visited_bytes != materialized_bytes {
        return Err(invalid_composed_text_plan());
    }
    planner.finish()
}

struct GraphemeBoundaryPlanner<'resources> {
    total_bytes: usize,
    committed_bytes: usize,
    scratch: String,
    cursor: GraphemeCursor,
    boundaries: Vec<usize>,
    resources: &'resources ResourceContext,
}

impl<'resources> GraphemeBoundaryPlanner<'resources> {
    fn try_new(
        total_bytes: usize,
        boundary_capacity: usize,
        resources: &'resources ResourceContext,
    ) -> Result<Self> {
        let mut boundaries = Vec::new();
        boundaries
            .try_reserve_exact(boundary_capacity)
            .map_err(|_| layout_allocation_failed())?;
        Ok(Self {
            total_bytes,
            committed_bytes: 0,
            scratch: String::new(),
            cursor: GraphemeCursor::new(0, total_bytes, true),
            boundaries,
            resources,
        })
    }

    fn try_push_str(&mut self, value: &str) -> Result<()> {
        for scalar in value.chars() {
            self.try_push_scalar(scalar)?;
        }
        Ok(())
    }

    fn try_push_scalar(&mut self, scalar: char) -> Result<()> {
        // `GraphemeCursor` may need one scalar of lookahead before it can confirm the previous
        // boundary. Only the existing unresolved prefix is known to belong to one grapheme here;
        // counting `scalar` as part of that grapheme would reject adjacent exact-limit graphemes.
        // Checking before the append still bounds the scratch overrun to one UTF-8 scalar.
        self.resources.check_grapheme_bytes(self.scratch.len())?;
        let pending_unresolved_bytes = self
            .scratch
            .len()
            .checked_add(scalar.len_utf8())
            .ok_or_else(|| {
                self.resources
                    .overflow(AsciiResourceLimitId::MaxGraphemeBytes)
            })?;
        let pending_bytes = self
            .committed_bytes
            .checked_add(pending_unresolved_bytes)
            .ok_or_else(|| {
                self.resources
                    .overflow(AsciiResourceLimitId::MaxOutputBytes)
            })?;
        if pending_bytes > self.total_bytes {
            return Err(invalid_composed_text_plan());
        }
        self.scratch
            .try_reserve(scalar.len_utf8())
            .map_err(|_| layout_allocation_failed())?;
        self.scratch.push(scalar);
        self.try_advance()
    }

    fn try_advance(&mut self) -> Result<()> {
        loop {
            match self.cursor.next_boundary(self.scratch.as_str(), 0) {
                Ok(Some(boundary)) => {
                    if boundary == 0 || self.scratch.get(..boundary).is_none() {
                        return Err(invalid_composed_text_plan());
                    }
                    self.resources.check_grapheme_bytes(boundary)?;
                    let absolute_boundary =
                        self.committed_bytes.checked_add(boundary).ok_or_else(|| {
                            self.resources
                                .overflow(AsciiResourceLimitId::MaxOutputBytes)
                        })?;
                    try_push_grapheme_boundary(&mut self.boundaries, absolute_boundary)?;
                    self.committed_bytes = absolute_boundary;
                    drop(self.scratch.drain(..boundary));

                    if self.committed_bytes == self.total_bytes {
                        if !self.scratch.is_empty() {
                            return Err(invalid_composed_text_plan());
                        }
                        return Ok(());
                    }
                    let remaining_bytes = self
                        .total_bytes
                        .checked_sub(self.committed_bytes)
                        .ok_or_else(invalid_composed_text_plan)?;
                    self.cursor = GraphemeCursor::new(0, remaining_bytes, true);
                    if self.scratch.is_empty() {
                        return Ok(());
                    }
                }
                Err(GraphemeIncomplete::NextChunk) => return Ok(()),
                Ok(None)
                | Err(
                    GraphemeIncomplete::PreContext(_)
                    | GraphemeIncomplete::PrevChunk
                    | GraphemeIncomplete::InvalidOffset,
                ) => return Err(invalid_composed_text_plan()),
            }
        }
    }

    fn finish(self) -> Result<Vec<usize>> {
        if self.committed_bytes != self.total_bytes
            || !self.scratch.is_empty()
            || self.boundaries.last().copied() != Some(self.total_bytes)
        {
            return Err(invalid_composed_text_plan());
        }
        Ok(self.boundaries)
    }
}

fn decoded_html_fragment_len(fragment: DecodedHtmlFragment<'_>) -> usize {
    match fragment {
        DecodedHtmlFragment::Borrowed(value) => value.len(),
        DecodedHtmlFragment::Scalar(value) => value.len_utf8(),
    }
}

fn decoded_html_replay_fragment<'a>(
    fragment: DecodedHtmlFragment<'a>,
    start: usize,
) -> ReplayFragment<'a> {
    match fragment {
        DecodedHtmlFragment::Borrowed(value) => {
            ReplayFragment::Borrowed(TextFragment { start, value })
        }
        DecodedHtmlFragment::Scalar(value) => ReplayFragment::Scalar { start, value },
    }
}

fn try_push_grapheme_boundary(boundaries: &mut Vec<usize>, boundary: usize) -> Result<()> {
    if boundaries.len() == boundaries.capacity() {
        boundaries
            .try_reserve(1)
            .map_err(|_| layout_allocation_failed())?;
    }
    boundaries.push(boundary);
    Ok(())
}

fn visit_normalized_output_graphemes(
    value: &str,
    profile: TerminalWidthProfile,
    mut visit: impl FnMut(&str, usize) -> Result<()>,
) -> Result<()> {
    visit_normalized_segments(value, |segment| match segment.kind {
        NormalizedSegmentKind::Grapheme(value) => {
            visit(value, grapheme_display_width(value, profile))
        }
        NormalizedSegmentKind::VisibleEscape(_) => {
            let mut buffer = [0u8; 10];
            let value = segment.text(&mut buffer);
            for byte in value.as_bytes() {
                let scalar = std::str::from_utf8(std::slice::from_ref(byte))
                    .expect("visible escapes contain only ASCII");
                visit(scalar, 1)?;
            }
            Ok(())
        }
        NormalizedSegmentKind::LineBreak => {
            for byte in b"\\u{A}" {
                let scalar = std::str::from_utf8(std::slice::from_ref(byte))
                    .expect("visible escapes contain only ASCII");
                visit(scalar, 1)?;
            }
            Ok(())
        }
    })
}

#[cfg(test)]
fn collect_composed_text(
    fragments: &[ReplayFragment<'_>],
    expected_bytes: usize,
) -> Result<String> {
    let mut value = String::new();
    value
        .try_reserve_exact(expected_bytes)
        .map_err(|_| layout_allocation_failed())?;
    for fragment in fragments.iter().copied() {
        let fragment_end = fragment
            .start()
            .checked_add(fragment.len())
            .ok_or_else(invalid_composed_text_plan)?;
        fragment.append_range(fragment.start(), fragment_end, &mut value)?;
    }
    if value.len() != expected_bytes {
        return Err(invalid_composed_text_plan());
    }
    Ok(value)
}

fn checked_work_add(resources: &ResourceContext, left: usize, right: usize) -> Result<usize> {
    resources.checked_work_add(left, right)
}

fn checked_output_add(resources: &ResourceContext, left: usize, right: usize) -> Result<usize> {
    left.checked_add(right)
        .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxOutputBytes))
}

fn encoded_html_bytes(resources: &ResourceContext, value: &str) -> Result<usize> {
    let mut bytes = 0usize;
    super::encode::visit_html_escaped_text(value, |fragment| {
        bytes = checked_output_add(resources, bytes, fragment.len())?;
        Ok(())
    })?;
    Ok(bytes)
}

fn layout_allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

fn invalid_composed_text_plan() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "terminal_text",
        feature: "composed text replay",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::resources::ResourceProfile;
    use std::cell::Cell;

    fn policy_with_limit(id: AsciiResourceLimitId, max: usize) -> AsciiResourcePolicy {
        AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(id, max)
            .expect("test limit should be valid")
    }

    #[test]
    fn composed_text_checks_graphemes_across_fragment_boundaries_before_materializing() {
        let resources =
            ResourceContext::new(policy_with_limit(AsciiResourceLimitId::MaxGraphemeBytes, 2));
        resources
            .charge_usage(3, 5)
            .expect("test checkpoint should fit");
        let error = ComposedTextPlan::try_new(&resources, 1, |push| {
            push(" ")?;
            push("\u{301}")
        })
        .expect_err("space plus combining mark is one three-byte grapheme");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGraphemeBytes
                    && details.actual == 3
                    && details.max == 2
        ));
        assert_eq!(resources.layout_work_used(), 3);
        assert_eq!(resources.document_cells_used(), 5);
    }

    #[test]
    fn composed_text_preserves_zwj_and_regional_graphemes_split_across_fragments() {
        for (fragments, expected_bytes) in [
            (vec!["👩", "\u{200d}", "💻"], 11usize),
            (vec!["🇺", "🇸"], 8usize),
            (vec![" ", "\u{301}", "\u{301}"], 5usize),
        ] {
            let exact = ResourceContext::new(policy_with_limit(
                AsciiResourceLimitId::MaxGraphemeBytes,
                expected_bytes,
            ));
            let plan = ComposedTextPlan::try_new(&exact, 1, |push| {
                for fragment in &fragments {
                    push(fragment)?;
                }
                Ok(())
            })
            .expect("exact grapheme limit should admit fragmented text");
            let materialized = plan
                .materialize(&exact, || {})
                .expect("admitted fragmented text should materialize");
            assert_eq!(materialized, fragments.concat());

            let below = ResourceContext::new(policy_with_limit(
                AsciiResourceLimitId::MaxGraphemeBytes,
                expected_bytes - 1,
            ));
            let error = ComposedTextPlan::try_new(&below, 1, |push| {
                for fragment in &fragments {
                    push(fragment)?;
                }
                Ok(())
            })
            .expect_err("max-minus-one grapheme limit should reject fragmented text");
            assert!(matches!(
                error,
                AsciiError::ResourceLimitExceeded(details)
                    if details.limit == AsciiResourceLimitId::MaxGraphemeBytes
                        && details.actual == expected_bytes
                        && details.max == expected_bytes - 1
            ));
        }
    }

    #[test]
    fn composed_text_allows_one_scalar_of_lookahead_at_the_exact_grapheme_limit() {
        for (fragments, exact_bytes) in [(vec!["A", "B"], 1usize), (vec!["🍒", "🍋"], 4usize)] {
            let exact = ResourceContext::new(policy_with_limit(
                AsciiResourceLimitId::MaxGraphemeBytes,
                exact_bytes,
            ));
            let plan = ComposedTextPlan::try_new(&exact, 1, |push| {
                for fragment in &fragments {
                    push(fragment)?;
                }
                Ok(())
            })
            .expect("adjacent exact-limit graphemes should be admitted");
            assert_eq!(
                plan.materialize(&exact, || {})
                    .expect("the admitted text should materialize"),
                fragments.concat()
            );

            if exact_bytes > 1 {
                let below = ResourceContext::new(policy_with_limit(
                    AsciiResourceLimitId::MaxGraphemeBytes,
                    exact_bytes - 1,
                ));
                let error = ComposedTextPlan::try_new(&below, 1, |push| {
                    for fragment in &fragments {
                        push(fragment)?;
                    }
                    Ok(())
                })
                .expect_err("max-minus-one must reject an individual grapheme");
                assert!(matches!(
                    error,
                    AsciiError::ResourceLimitExceeded(details)
                        if details.limit == AsciiResourceLimitId::MaxGraphemeBytes
                            && details.actual == exact_bytes
                            && details.max == exact_bytes - 1
                ));
                assert_eq!(below.layout_work_used(), 0);
                assert_eq!(below.document_cells_used(), 0);
            }
        }
    }

    #[test]
    fn composed_text_planning_stays_additive_for_many_boundary_sensitive_fragments() {
        const FRAGMENT_COUNT: usize = 64;
        const PLANNING_WORK: usize = FRAGMENT_COUNT * 9;
        const MATERIALIZATION_WORK: usize = FRAGMENT_COUNT * 5;
        const COMPLETE_WORK: usize = PLANNING_WORK + MATERIALIZATION_WORK;

        let fragments = ["🍒"; FRAGMENT_COUNT];
        let exact = ResourceContext::new(policy_with_limit(
            AsciiResourceLimitId::MaxLayoutWorkUnits,
            COMPLETE_WORK,
        ));
        let plan = ComposedTextPlan::try_new(&exact, FRAGMENT_COUNT, |push| {
            for fragment in &fragments {
                push(fragment)?;
            }
            Ok(())
        })
        .expect("the exact additive budget should admit fragmented emoji");
        assert_eq!(exact.layout_work_used(), PLANNING_WORK);
        let materialized = plan
            .materialize(&exact, || {})
            .expect("the admitted fragmented emoji should materialize");
        assert_eq!(materialized, fragments.concat());
        assert_eq!(exact.layout_work_used(), COMPLETE_WORK);

        let below = ResourceContext::new(policy_with_limit(
            AsciiResourceLimitId::MaxLayoutWorkUnits,
            COMPLETE_WORK - 1,
        ));
        let error = ComposedTextPlan::try_new(&below, FRAGMENT_COUNT, |push| {
            for fragment in &fragments {
                push(fragment)?;
            }
            Ok(())
        })
        .expect_err("max-minus-one must reject before the segmentation buffer allocation");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == COMPLETE_WORK
                    && details.max == COMPLETE_WORK - 1
        ));
        assert_eq!(below.layout_work_used(), 0);
        assert_eq!(below.document_cells_used(), 0);
    }

    #[test]
    fn composed_text_checks_output_and_work_before_final_string_allocation() {
        let measured = ResourceContext::new(AsciiResourcePolicy::for_profile(
            ResourceProfile::UnboundedForTrustedInput,
        ));
        let plan = ComposedTextPlan::try_new(&measured, 2, |push| {
            push("alpha")?;
            push(": ")?;
            push("beta")
        })
        .expect("unbounded composed text should plan");
        let exact_work = measured
            .layout_work_used()
            .checked_add(plan.materialization_work_units())
            .expect("test work should fit usize");
        let output_bytes = plan.materialized_bytes();

        let below_output = ResourceContext::new(policy_with_limit(
            AsciiResourceLimitId::MaxOutputBytes,
            output_bytes - 1,
        ));
        let error = ComposedTextPlan::try_new(&below_output, 2, |push| {
            push("alpha")?;
            push(": ")?;
            push("beta")
        })
        .expect_err("max-minus-one output limit should reject before retaining fragments");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxOutputBytes
                    && details.actual == output_bytes
                    && details.max == output_bytes - 1
        ));
        assert_eq!(below_output.layout_work_used(), 0);

        let exact_work_resources = ResourceContext::new(policy_with_limit(
            AsciiResourceLimitId::MaxLayoutWorkUnits,
            exact_work,
        ));
        let plan = ComposedTextPlan::try_new(&exact_work_resources, 2, |push| {
            push("alpha")?;
            push(": ")?;
            push("beta")
        })
        .expect("exact work limit should admit the plan");
        let probe = Cell::new(false);
        plan.materialize(&exact_work_resources, || probe.set(true))
            .expect("exact work limit should admit materialization");
        assert!(probe.get());
        assert_eq!(exact_work_resources.layout_work_used(), exact_work);

        let below_work = ResourceContext::new(policy_with_limit(
            AsciiResourceLimitId::MaxLayoutWorkUnits,
            exact_work - 1,
        ));
        let error = ComposedTextPlan::try_new(&below_work, 2, |push| {
            push("alpha")?;
            push(": ")?;
            push("beta")
        })
        .expect_err("max-minus-one work should reject before the plan allocation");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == exact_work
                    && details.max == exact_work - 1
        ));
        assert_eq!(below_work.layout_work_used(), 0);
    }

    #[test]
    fn deferred_piece_planning_admits_before_collection_and_rolls_back_at_n_minus_one() {
        const CHECKPOINT_WORK: usize = 7;
        const CHECKPOINT_CELLS: usize = 5;
        const RANGE_SCAN_WORK: usize = 6;
        const PLANNING_WORK: usize = 24;

        let plan_resources = ResourceContext::new(AsciiResourcePolicy::for_profile(
            ResourceProfile::UnboundedForTrustedInput,
        ));
        let plan = ComposedTextPlan::try_new(&plan_resources, 2, |push| {
            push("a")?;
            push("\u{7}")
        })
        .expect("fixed composed text should plan");
        assert_eq!(
            plan.deferred_range_scan_work_units(&plan_resources)
                .expect("fixed scan work should fit"),
            RANGE_SCAN_WORK
        );

        let exact = ResourceContext::new(policy_with_limit(
            AsciiResourceLimitId::MaxLayoutWorkUnits,
            CHECKPOINT_WORK + PLANNING_WORK,
        ));
        exact
            .charge_usage(CHECKPOINT_WORK, CHECKPOINT_CELLS)
            .expect("exact checkpoint should fit");
        let exact_probe = Cell::new(0usize);
        let (pieces, metrics) = plan
            .try_deferred_pieces_with_probe(TerminalWidthProfile::Unicode, &exact, &exact_probe)
            .expect("the exact additive planning budget should fit");
        assert_eq!(pieces.len(), 6);
        assert_eq!(metrics.display_width(), 6);
        assert_eq!(exact_probe.get(), 2);
        assert_eq!(exact.layout_work_used(), CHECKPOINT_WORK + PLANNING_WORK);
        assert_eq!(exact.document_cells_used(), CHECKPOINT_CELLS);

        let below = ResourceContext::new(policy_with_limit(
            AsciiResourceLimitId::MaxLayoutWorkUnits,
            CHECKPOINT_WORK + PLANNING_WORK - 1,
        ));
        below
            .charge_usage(CHECKPOINT_WORK, CHECKPOINT_CELLS)
            .expect("below-exact checkpoint should fit");
        let below_probe = Cell::new(0usize);
        let error = plan
            .try_deferred_pieces_with_probe(TerminalWidthProfile::Unicode, &below, &below_probe)
            .expect_err("max-minus-one planning work should reject before the second pass");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == CHECKPOINT_WORK + PLANNING_WORK
                    && details.max == CHECKPOINT_WORK + PLANNING_WORK - 1
        ));
        assert_eq!(below_probe.get(), 1);
        assert_eq!(below.layout_work_used(), CHECKPOINT_WORK);
        assert_eq!(below.document_cells_used(), CHECKPOINT_CELLS);

        let below_scan = ResourceContext::new(policy_with_limit(
            AsciiResourceLimitId::MaxLayoutWorkUnits,
            CHECKPOINT_WORK + RANGE_SCAN_WORK - 1,
        ));
        below_scan
            .charge_usage(CHECKPOINT_WORK, CHECKPOINT_CELLS)
            .expect("below-scan checkpoint should fit");
        let scan_probe = Cell::new(0usize);
        let error = plan
            .try_deferred_pieces_with_probe(TerminalWidthProfile::Unicode, &below_scan, &scan_probe)
            .expect_err("insufficient structural scan work should reject before collection");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == CHECKPOINT_WORK + RANGE_SCAN_WORK
                    && details.max == CHECKPOINT_WORK + RANGE_SCAN_WORK - 1
        ));
        assert_eq!(scan_probe.get(), 0);
        assert_eq!(below_scan.layout_work_used(), CHECKPOINT_WORK);
        assert_eq!(below_scan.document_cells_used(), CHECKPOINT_CELLS);
    }

    #[test]
    fn deferred_piece_planning_charges_additive_work_for_many_fragments() {
        const FRAGMENT_COUNT: usize = 64;
        const EXPECTED_PLANNING_WORK: usize = FRAGMENT_COUNT * 8;

        let fragments = ["a"; FRAGMENT_COUNT];
        let plan_resources = ResourceContext::new(AsciiResourcePolicy::for_profile(
            ResourceProfile::UnboundedForTrustedInput,
        ));
        let plan = ComposedTextPlan::try_new(&plan_resources, FRAGMENT_COUNT, |push| {
            for fragment in &fragments {
                push(fragment)?;
            }
            Ok(())
        })
        .expect("fragmented composed text should plan");

        let resources = ResourceContext::new(policy_with_limit(
            AsciiResourceLimitId::MaxLayoutWorkUnits,
            EXPECTED_PLANNING_WORK,
        ));
        let phase_probe = Cell::new(0usize);
        let (pieces, metrics) = plan
            .try_deferred_pieces_with_probe(TerminalWidthProfile::Unicode, &resources, &phase_probe)
            .expect("the exact additive work budget should admit fragmented text");
        assert_eq!(pieces.len(), FRAGMENT_COUNT);
        assert_eq!(metrics.display_width(), FRAGMENT_COUNT);
        assert_eq!(phase_probe.get(), 2);
        assert_eq!(resources.layout_work_used(), EXPECTED_PLANNING_WORK);
        assert!(pieces.iter().all(|piece| piece.replay_work_units() == 2));
    }

    #[test]
    fn deferred_piece_replay_preserves_fragmented_entities_and_visible_escapes() {
        let plan_resources = ResourceContext::new(AsciiResourcePolicy::for_profile(
            ResourceProfile::UnboundedForTrustedInput,
        ));
        let plan = ComposedTextPlan::try_new_html_decoded(
            "e&#x301;👩&#x200D;💻\u{7}&amp;",
            &plan_resources,
        )
        .expect("fragmented HTML-decoded text should plan");
        let resources = ResourceContext::new(AsciiResourcePolicy::for_profile(
            ResourceProfile::UnboundedForTrustedInput,
        ));
        let (pieces, metrics) = plan
            .try_deferred_pieces(TerminalWidthProfile::Unicode, &resources)
            .expect("fragmented deferred text should plan");

        let mut replayed = String::new();
        for &piece in &pieces {
            plan.try_visit_deferred_piece(TerminalWidthProfile::Unicode, piece, |fragment| {
                replayed.push_str(fragment);
                Ok(())
            })
            .expect("each deferred piece should replay");
        }

        let expected = "e\u{301}👩\u{200d}💻\\u{7}&";
        let expected_html = "e\u{301}👩\u{200d}💻\\u{7}&amp;";
        assert_eq!(replayed, expected);
        assert_eq!(pieces.len(), 8);
        assert_eq!(metrics.display_width(), 9);
        assert_eq!(
            pieces
                .iter()
                .map(|piece| piece.encoded_bytes(false))
                .sum::<usize>(),
            expected.len()
        );
        assert_eq!(
            pieces
                .iter()
                .map(|piece| piece.encoded_bytes(true))
                .sum::<usize>(),
            expected_html.len()
        );
        assert!(
            pieces[2..]
                .iter()
                .all(|piece| piece.replay_work_units() == 2)
        );
    }
}
