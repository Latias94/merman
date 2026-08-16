use super::normalization::visit_normalized_segments;
use crate::Result;
use crate::error::AsciiError;
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NormalizedTrimmedTextMetrics {
    pub(crate) materialized_bytes: usize,
    pub(crate) document_cells: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NormalizedTextPlan<'a> {
    source: &'a str,
    start: usize,
    end: usize,
    metrics: NormalizedTrimmedTextMetrics,
    materialization_work_units: usize,
}

pub(crate) type NormalizedTrimmedTextPlan<'a> = NormalizedTextPlan<'a>;

impl NormalizedTextPlan<'_> {
    pub(crate) const fn metrics(self) -> NormalizedTrimmedTextMetrics {
        self.metrics
    }

    pub(crate) fn materialization_work_units(self) -> usize {
        self.materialization_work_units
    }

    /// Materializes the exact normalized range after its caller has admitted the aggregate work.
    #[cfg(test)]
    pub(crate) fn materialize_after_admission(self) -> Result<String> {
        self.materialize_after_admission_with_checkpoint(|_| Ok(()))
    }

    /// Materializes the admitted range while allowing long replays to observe cancellation.
    pub(crate) fn materialize_after_admission_with_checkpoint(
        self,
        mut checkpoint: impl FnMut(usize) -> Result<()>,
    ) -> Result<String> {
        checkpoint(0)?;
        let mut output = String::new();
        output
            .try_reserve_exact(self.metrics.materialized_bytes)
            .map_err(|_| layout_allocation_failed())?;

        let mut offset = 0usize;
        let mut segment_index = 1usize;
        visit_normalized_segments(self.source, |segment| {
            checkpoint(segment_index)?;
            segment_index = segment_index
                .checked_add(1)
                .ok_or_else(layout_allocation_failed)?;
            let mut buffer = [0u8; 10];
            let text = segment.text(&mut buffer);
            let segment_end = offset
                .checked_add(text.len())
                .ok_or_else(layout_allocation_failed)?;
            let kept_start = self.start.max(offset);
            let kept_end = self.end.min(segment_end);
            if kept_start < kept_end {
                output.push_str(&text[kept_start - offset..kept_end - offset]);
            }
            offset = segment_end;
            Ok::<(), AsciiError>(())
        })?;

        debug_assert_eq!(output.len(), self.metrics.materialized_bytes);
        Ok(output)
    }
}

/// Plans terminal normalization for a complete borrowed string without retaining its expansion.
pub(crate) fn try_plan_normalized_text<'a>(
    value: &'a str,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<NormalizedTextPlan<'a>> {
    let mut materialized_bytes = 0usize;
    let mut document_cells = 0usize;

    resources.charge_layout_work(1)?;
    visit_normalized_segments(value, |segment| {
        segment.check_grapheme_budget(resources)?;
        resources.charge_layout_work(segment.layout_work())?;
        let mut buffer = [0u8; 10];
        let text = segment.text(&mut buffer);
        resources.charge_layout_work(text.len().max(1))?;
        materialized_bytes = materialized_bytes
            .checked_add(text.len())
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
        document_cells = checked_document_add(
            resources,
            document_cells,
            segment.display_width(width_profile),
        )?;
        Ok::<(), AsciiError>(())
    })?;

    let materialization_work_units =
        resources.checked_work_add(value.len().max(1), materialized_bytes)?;
    Ok(NormalizedTextPlan {
        source: value,
        start: 0,
        end: materialized_bytes,
        metrics: NormalizedTrimmedTextMetrics {
            materialized_bytes,
            document_cells,
        },
        materialization_work_units,
    })
}

/// Plans terminal normalization and `str::trim` semantics without retaining the normalized text.
pub(crate) fn try_plan_normalized_trimmed_text<'a>(
    value: &'a str,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<Option<NormalizedTrimmedTextPlan<'a>>> {
    let mut offset = 0usize;
    let mut start = None;
    let mut end = 0usize;

    resources.charge_layout_work(1)?;
    visit_normalized_segments(value, |segment| {
        segment.check_grapheme_budget(resources)?;
        resources.charge_layout_work(segment.layout_work())?;
        let mut buffer = [0u8; 10];
        let text = segment.text(&mut buffer);
        resources.charge_layout_work(text.len().max(1))?;
        let segment_end = resources.checked_work_add(offset, text.len())?;
        for (relative, ch) in text.char_indices() {
            if ch.is_whitespace() {
                continue;
            }
            let absolute = resources.checked_work_add(offset, relative)?;
            start.get_or_insert(absolute);
            end = resources.checked_work_add(absolute, ch.len_utf8())?;
        }
        offset = segment_end;
        Ok::<(), AsciiError>(())
    })?;

    let Some(start) = start else {
        return Ok(None);
    };
    let retained_bytes = end
        .checked_sub(start)
        .ok_or_else(layout_allocation_failed)?;
    let document_cells = measure_normalized_range(value, start, end, width_profile, resources)?;
    let materialization_work_units =
        resources.checked_work_add(value.len().max(1), retained_bytes)?;
    Ok(Some(NormalizedTextPlan {
        source: value,
        start,
        end,
        metrics: NormalizedTrimmedTextMetrics {
            materialized_bytes: retained_bytes,
            document_cells,
        },
        materialization_work_units,
    }))
}

fn measure_normalized_range(
    value: &str,
    start: usize,
    end: usize,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<usize> {
    let mut offset = 0usize;
    let mut document_cells = 0usize;

    resources.charge_layout_work(1)?;
    visit_normalized_segments(value, |segment| {
        segment.check_grapheme_budget(resources)?;
        resources.charge_layout_work(segment.layout_work())?;
        let mut buffer = [0u8; 10];
        let text = segment.text(&mut buffer);
        resources.charge_layout_work(text.len().max(1))?;
        let segment_end = resources.checked_work_add(offset, text.len())?;
        let kept_start = start.max(offset);
        let kept_end = end.min(segment_end);
        if kept_start < kept_end {
            let relative_start = kept_start - offset;
            let relative_end = kept_end - offset;
            let width = if relative_start == 0 && relative_end == text.len() {
                segment.display_width(width_profile)
            } else {
                measure_normalized_fragment(
                    &text[relative_start..relative_end],
                    width_profile,
                    resources,
                )?
            };
            document_cells = checked_document_add(resources, document_cells, width)?;
        }
        offset = segment_end;
        Ok::<(), AsciiError>(())
    })?;

    Ok(document_cells)
}

fn measure_normalized_fragment(
    value: &str,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<usize> {
    let mut document_cells = 0usize;
    resources.charge_layout_work(value.len().max(1))?;
    visit_normalized_segments(value, |segment| {
        segment.check_grapheme_budget(resources)?;
        resources.charge_layout_work(segment.layout_work())?;
        document_cells = checked_document_add(
            resources,
            document_cells,
            segment.display_width(width_profile),
        )?;
        Ok::<(), AsciiError>(())
    })?;
    Ok(document_cells)
}

fn checked_document_add(resources: &ResourceContext, left: usize, right: usize) -> Result<usize> {
    left.checked_add(right)
        .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxDocumentCells))
}

pub(crate) fn try_concat_layout_text(
    left: &str,
    right: &str,
    resources: &ResourceContext,
) -> Result<String> {
    try_concat_layout_text_impl(left, right, resources, || {})
}

pub(crate) fn try_clone_layout_text(value: &str, resources: &ResourceContext) -> Result<String> {
    try_concat_layout_text(value, "", resources)
}

pub(crate) fn try_repeat_layout_char(
    ch: char,
    count: usize,
    resources: &ResourceContext,
) -> Result<String> {
    let byte_count = resources.checked_work_mul(ch.len_utf8(), count)?;
    resources.charge_layout_work(byte_count)?;
    let mut output = String::new();
    output
        .try_reserve_exact(byte_count)
        .map_err(|_| layout_allocation_failed())?;
    for _ in 0..count {
        output.push(ch);
    }
    Ok(output)
}

fn try_concat_layout_text_impl(
    left: &str,
    right: &str,
    resources: &ResourceContext,
    before_materialize: impl FnOnce(),
) -> Result<String> {
    let byte_count = resources.checked_work_add(left.len(), right.len())?;
    resources.charge_layout_work(byte_count)?;
    before_materialize();
    let mut output = String::new();
    output
        .try_reserve_exact(byte_count)
        .map_err(|_| layout_allocation_failed())?;
    output.push_str(left);
    output.push_str(right);
    Ok(output)
}

fn layout_allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use crate::text::normalize_optional_text;
    use merman_core::resources::ResourceProfile;
    use merman_core::{OperationControl, OperationPhase};
    use std::cell::Cell;

    #[test]
    fn normalized_trimmed_plan_matches_existing_optional_text_semantics() {
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        for raw in [
            "",
            "   ",
            " alpha ",
            "\r\n alpha \r\n",
            "\talpha\t",
            " \u{301}word ",
            "  边\u{7}  ",
        ] {
            let resources = ResourceContext::new(policy);
            let plan =
                try_plan_normalized_trimmed_text(raw, TerminalWidthProfile::Unicode, &resources)
                    .expect("optional text normalization should be measurable");
            let actual = match plan {
                Some(plan) => {
                    resources
                        .charge_layout_work(plan.materialization_work_units())
                        .expect("unbounded policy should admit optional text materialization");
                    Some(
                        plan.materialize_after_admission()
                            .expect("admitted optional text should materialize"),
                    )
                }
                None => None,
            };
            assert_eq!(actual, normalize_optional_text(Some(raw)), "raw={raw:?}");
        }
    }

    #[test]
    fn normalized_trimmed_plan_reports_retained_output_metrics_without_materializing() {
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let resources = ResourceContext::new(policy);
        let plan = try_plan_normalized_trimmed_text(
            "  边\u{7}  ",
            TerminalWidthProfile::Unicode,
            &resources,
        )
        .expect("trimmed text metrics should be measurable")
        .expect("the normalized label should remain non-empty");

        assert_eq!(
            plan.metrics(),
            NormalizedTrimmedTextMetrics {
                materialized_bytes: "边\\u{7}".len(),
                document_cells: 7,
            }
        );
    }

    #[test]
    fn normalized_trimmed_materialization_observes_mid_replay_cancellation() {
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let resources = ResourceContext::new(policy);
        let raw = "a".repeat(128);
        let plan =
            try_plan_normalized_trimmed_text(&raw, TerminalWidthProfile::Unicode, &resources)
                .expect("long normalized text should be measurable")
                .expect("long normalized text should remain non-empty");
        let control = OperationControl::new();
        control.cancel_after_checkpoints(1);

        let error = plan
            .materialize_after_admission_with_checkpoint(|iteration| {
                if iteration.is_multiple_of(64) {
                    control
                        .checkpoint_at(OperationPhase::Semantic)
                        .map_err(AsciiError::Cancelled)?;
                }
                Ok(())
            })
            .expect_err("the second bounded checkpoint should cancel replay");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Semantic
        ));
    }

    #[test]
    fn layout_text_accepts_exact_work_and_rejects_n_minus_one_before_materializing() {
        const REQUIRED_WORK: usize = 6;
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, REQUIRED_WORK)
            .expect("exact layout-text work limit should be valid");
        let exact_resources = ResourceContext::new(exact_policy);
        let exact_materialized = Cell::new(false);

        let output = try_concat_layout_text_impl("abcd", "ef", &exact_resources, || {
            exact_materialized.set(true);
        })
        .expect("exact layout-text work should permit materialization");

        assert_eq!(output, "abcdef");
        assert!(exact_materialized.get());
        assert_eq!(exact_resources.layout_work_used(), REQUIRED_WORK);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, REQUIRED_WORK - 1)
            .expect("max-minus-one layout-text work limit should be valid");
        let below_resources = ResourceContext::new(below_policy);
        let below_materialized = Cell::new(false);
        let error = try_concat_layout_text_impl("abcd", "ef", &below_resources, || {
            below_materialized.set(true);
        })
        .expect_err("max-minus-one work should fail before materialization");

        assert!(!below_materialized.get());
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == REQUIRED_WORK
                    && details.max == REQUIRED_WORK - 1
        ));
        assert_eq!(below_resources.layout_work_used(), 0);
    }
}
