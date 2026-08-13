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

    fn append_range(self, start: usize, end: usize, output: &mut String) -> Result<()> {
        let local_start = start
            .checked_sub(self.start())
            .ok_or_else(invalid_composed_text_plan)?;
        let local_end = end
            .checked_sub(self.start())
            .ok_or_else(invalid_composed_text_plan)?;
        match self {
            Self::Borrowed(fragment) => output.push_str(
                fragment
                    .value
                    .get(local_start..local_end)
                    .ok_or_else(invalid_composed_text_plan)?,
            ),
            Self::Scalar { value, .. } => {
                let mut buffer = [0u8; 4];
                output.push_str(
                    value
                        .encode_utf8(&mut buffer)
                        .get(local_start..local_end)
                        .ok_or_else(invalid_composed_text_plan)?,
                );
            }
        }
        Ok(())
    }
}

/// A borrowed plan for text assembled from family-owned fields and separators.
///
/// The fragment producer is replayed once to measure and once to retain borrowed slices. The
/// complete logical byte stream is then segmented with `GraphemeCursor`, so a combining mark or
/// joiner at a fragment boundary cannot bypass the final grapheme limit. No authored text is
/// copied until the caller admits `materialization_work_units` and calls
/// [`Self::materialize_after_admission`].
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
        let mut piece_count = 0usize;
        let mut display_width = 0usize;
        let mut source_start = 0usize;
        for &source_end in &self.grapheme_boundaries {
            let grapheme = collect_composed_range(&self.fragments, source_start, source_end)?;
            visit_normalized_output_graphemes(&grapheme.value, profile, |_, width| {
                piece_count = resources.checked_work_add(piece_count, 1)?;
                display_width = resources.checked_grid_add(display_width, width)?;
                Ok(())
            })?;
            source_start = source_end;
        }

        let replay_work_units = resources.checked_work_add(
            resources.checked_work_mul(piece_count.max(1), self.fragments.len().max(1))?,
            self.metrics.materialized_bytes.max(1),
        )?;
        let planning_work = resources.checked_work_mul(replay_work_units, 2)?;
        resources.check_usage(planning_work, 0)?;
        resources.charge_usage(planning_work, 0)?;

        let mut pieces = Vec::new();
        pieces
            .try_reserve_exact(piece_count)
            .map_err(|_| layout_allocation_failed())?;
        source_start = 0;
        for &source_end in &self.grapheme_boundaries {
            let grapheme = collect_composed_range(&self.fragments, source_start, source_end)?;
            let mut output_index = 0u32;
            visit_normalized_output_graphemes(&grapheme.value, profile, |value, width| {
                pieces.push(DeferredTextPiece {
                    source_start,
                    source_end,
                    output_index,
                    display_width: u8::try_from(width).map_err(|_| invalid_composed_text_plan())?,
                    plain_bytes: value.len(),
                    html_bytes: encoded_html_bytes(resources, value)?,
                    replay_work_units: resources.checked_work_add(
                        self.fragments.len().max(1),
                        source_end
                            .checked_sub(source_start)
                            .ok_or_else(invalid_composed_text_plan)?,
                    )?,
                });
                output_index = output_index
                    .checked_add(1)
                    .ok_or_else(invalid_composed_text_plan)?;
                Ok(())
            })?;
            source_start = source_end;
        }
        if pieces.len() != piece_count {
            return Err(invalid_composed_text_plan());
        }
        Ok((pieces, DeferredTextMetrics { display_width }))
    }

    pub(crate) fn try_visit_deferred_piece(
        &self,
        profile: TerminalWidthProfile,
        piece: DeferredTextPiece,
        mut visit: impl FnMut(&str) -> Result<()>,
    ) -> Result<()> {
        let grapheme =
            collect_composed_range(&self.fragments, piece.source_start, piece.source_end)?;
        let mut output_index = 0u32;
        let mut found = false;
        visit_normalized_output_graphemes(&grapheme.value, profile, |value, _| {
            if output_index == piece.output_index {
                visit(value)?;
                found = true;
            }
            output_index = output_index
                .checked_add(1)
                .ok_or_else(invalid_composed_text_plan)?;
            Ok(())
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
        let mut output = String::new();
        output
            .try_reserve_exact(self.metrics.materialized_bytes)
            .map_err(|_| layout_allocation_failed())?;
        for fragment in self.fragments {
            fragment.append_range(
                fragment.start(),
                fragment
                    .start()
                    .checked_add(fragment.len())
                    .ok_or_else(invalid_composed_text_plan)?,
                &mut output,
            )?;
        }
        if output.len() != self.metrics.materialized_bytes {
            return Err(invalid_composed_text_plan());
        }
        Ok(output)
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

    let mut boundaries = Vec::new();
    boundaries
        .try_reserve_exact(fragments.len().max(1))
        .map_err(|_| layout_allocation_failed())?;
    let mut cursor = GraphemeCursor::new(0, materialized_bytes, true);
    let mut fragment_index = 0usize;
    let mut grapheme_start = 0usize;
    loop {
        while fragment_index < fragments.len()
            && cursor.cur_cursor()
                >= fragments[fragment_index]
                    .start()
                    .checked_add(fragments[fragment_index].len())
                    .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxOutputBytes))?
        {
            fragment_index += 1;
        }
        if fragment_index == fragments.len() {
            if grapheme_start < materialized_bytes {
                resources.check_grapheme_bytes(materialized_bytes - grapheme_start)?;
                try_push_grapheme_boundary(&mut boundaries, materialized_bytes)?;
            }
            return Ok(boundaries);
        }

        let fragment = fragments[fragment_index];
        let mut scalar_buffer = [0u8; 4];
        let value = match fragment {
            ReplayFragment::Borrowed(fragment) => fragment.value,
            ReplayFragment::Scalar { value, .. } => value.encode_utf8(&mut scalar_buffer),
        };
        match cursor.next_boundary(value, fragment.start()) {
            Ok(Some(boundary)) => {
                resources.check_grapheme_bytes(boundary - grapheme_start)?;
                grapheme_start = boundary;
                try_push_grapheme_boundary(&mut boundaries, boundary)?;
            }
            Ok(None) => return Ok(boundaries),
            Err(GraphemeIncomplete::NextChunk) => {
                fragment_index += 1;
            }
            Err(GraphemeIncomplete::PreContext(offset)) => {
                let Some(context) = context_ending_at(fragments, offset) else {
                    return Err(invalid_composed_text_plan());
                };
                cursor.provide_context(context.value.as_str(), context.start);
            }
            Err(GraphemeIncomplete::PrevChunk | GraphemeIncomplete::InvalidOffset) => {
                return Err(invalid_composed_text_plan());
            }
        }
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

fn context_ending_at<'a>(
    fragments: &'a [ReplayFragment<'a>],
    offset: usize,
) -> Option<ComposedGrapheme> {
    fragments.iter().rev().find_map(|fragment| {
        let end = fragment.start().checked_add(fragment.len())?;
        (fragment.start() < offset && offset <= end)
            .then(|| collect_composed_range(fragments, fragment.start(), offset))?
            .ok()
    })
}

struct ComposedGrapheme {
    start: usize,
    value: String,
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

fn collect_composed_range(
    fragments: &[ReplayFragment<'_>],
    start: usize,
    end: usize,
) -> Result<ComposedGrapheme> {
    let mut value = String::new();
    value
        .try_reserve_exact(
            end.checked_sub(start)
                .ok_or_else(invalid_composed_text_plan)?,
        )
        .map_err(|_| layout_allocation_failed())?;
    for fragment in fragments {
        let fragment_start = fragment.start();
        let fragment_end = fragment_start
            .checked_add(fragment.len())
            .ok_or_else(invalid_composed_text_plan)?;
        let kept_start = start.max(fragment_start);
        let kept_end = end.min(fragment_end);
        if kept_start < kept_end {
            fragment.append_range(kept_start, kept_end, &mut value)?;
        }
    }
    Ok(ComposedGrapheme { start, value })
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
}
