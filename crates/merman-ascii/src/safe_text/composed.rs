use crate::Result;
use crate::error::AsciiError;
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ComposedTextMetrics {
    materialized_bytes: usize,
    materialization_work_units: usize,
}

#[derive(Debug, Clone, Copy)]
struct TextFragment<'a> {
    start: usize,
    value: &'a str,
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
    fragments: Vec<TextFragment<'a>>,
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
                fragments.push(TextFragment {
                    start,
                    value: fragment,
                });
                Ok(())
            })?;
            if fragments.len() != fragment_count || collected_bytes != materialized_bytes {
                return Err(invalid_composed_text_plan());
            }

            validate_grapheme_budget(&fragments, materialized_bytes, resources)?;
            resources.charge_usage(planning_work, 0)?;
            Ok(Self {
                fragments,
                metrics: ComposedTextMetrics {
                    materialized_bytes,
                    materialization_work_units,
                },
            })
        })
    }

    #[cfg(test)]
    pub(crate) const fn materialized_bytes(&self) -> usize {
        self.metrics.materialized_bytes
    }

    #[cfg(test)]
    pub(crate) const fn materialization_work_units(&self) -> usize {
        self.metrics.materialization_work_units
    }

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
            output.push_str(fragment.value);
        }
        if output.len() != self.metrics.materialized_bytes {
            return Err(invalid_composed_text_plan());
        }
        Ok(output)
    }
}

fn validate_grapheme_budget(
    fragments: &[TextFragment<'_>],
    materialized_bytes: usize,
    resources: &ResourceContext,
) -> Result<()> {
    if materialized_bytes == 0 {
        return Ok(());
    }

    let mut cursor = GraphemeCursor::new(0, materialized_bytes, true);
    let mut fragment_index = 0usize;
    let mut grapheme_start = 0usize;
    loop {
        while fragment_index < fragments.len()
            && cursor.cur_cursor()
                >= fragments[fragment_index]
                    .start
                    .checked_add(fragments[fragment_index].value.len())
                    .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxOutputBytes))?
        {
            fragment_index += 1;
        }
        if fragment_index == fragments.len() {
            if grapheme_start < materialized_bytes {
                resources.check_grapheme_bytes(materialized_bytes - grapheme_start)?;
            }
            return Ok(());
        }

        let fragment = fragments[fragment_index];
        match cursor.next_boundary(fragment.value, fragment.start) {
            Ok(Some(boundary)) => {
                resources.check_grapheme_bytes(boundary - grapheme_start)?;
                grapheme_start = boundary;
            }
            Ok(None) => return Ok(()),
            Err(GraphemeIncomplete::NextChunk) => {
                fragment_index += 1;
            }
            Err(GraphemeIncomplete::PreContext(offset)) => {
                let Some(context) = context_ending_at(fragments, offset) else {
                    return Err(invalid_composed_text_plan());
                };
                cursor.provide_context(context.value, context.start);
            }
            Err(GraphemeIncomplete::PrevChunk | GraphemeIncomplete::InvalidOffset) => {
                return Err(invalid_composed_text_plan());
            }
        }
    }
}

fn context_ending_at<'a>(
    fragments: &'a [TextFragment<'a>],
    offset: usize,
) -> Option<TextFragment<'a>> {
    fragments.iter().rev().find_map(|fragment| {
        let end = fragment.start.checked_add(fragment.value.len())?;
        if fragment.start < offset && offset <= end {
            Some(TextFragment {
                start: fragment.start,
                value: &fragment.value[..offset - fragment.start],
            })
        } else {
            None
        }
    })
}

fn checked_work_add(resources: &ResourceContext, left: usize, right: usize) -> Result<usize> {
    resources.checked_work_add(left, right)
}

fn checked_output_add(resources: &ResourceContext, left: usize, right: usize) -> Result<usize> {
    left.checked_add(right)
        .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxOutputBytes))
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
