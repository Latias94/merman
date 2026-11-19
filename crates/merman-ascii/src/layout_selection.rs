//! Bounded family-layout selection before applying the final viewport overflow policy.

use crate::operation::AsciiExecution;
use crate::output::{MeasuredOutput, PrimaryOutputFacts, RenderedPrimary};
use crate::{
    AsciiError, AsciiExtent, AsciiGraphPresentation, AsciiLayoutProfile, AsciiRenderOptions,
    AsciiViewportPolicy, OverflowPolicy, Result,
};

pub(crate) enum LayoutCandidate {
    Measured {
        output: MeasuredOutput,
        facts: PrimaryOutputFacts,
    },
    Overflow(AsciiExtent),
    /// Only an admitted routing-search or port-capacity error, never measured geometry.
    PlanningMiss(AsciiError),
}

impl LayoutCandidate {
    fn extent(&self) -> Option<AsciiExtent> {
        match self {
            Self::Measured { output, .. } => Some(output.metrics().extent),
            Self::Overflow(extent) => Some(*extent),
            Self::PlanningMiss(_) => None,
        }
    }
}

pub(crate) struct SelectedLayout {
    pub candidate: LayoutCandidate,
    pub profile: AsciiLayoutProfile,
    pub compact_attempted: bool,
}

pub(crate) fn validate_viewport(
    profile: AsciiLayoutProfile,
    viewport: AsciiViewportPolicy,
) -> Result<()> {
    if profile == AsciiLayoutProfile::Auto && viewport.max_width.is_none() {
        return Err(AsciiError::InvalidOption {
            field: "ascii_viewport.max_width",
            message: "auto layout requires an explicit maximum width",
        });
    }
    Ok(())
}

pub(crate) fn select(
    options: AsciiRenderOptions,
    viewport: AsciiViewportPolicy,
    execution: AsciiExecution<'_>,
    mut render: impl FnMut(AsciiRenderOptions, AsciiExecution<'_>) -> Result<RenderedPrimary>,
) -> Result<SelectedLayout> {
    let automatic = options.layout_profile == AsciiLayoutProfile::Auto;
    let first_profile = if automatic {
        AsciiLayoutProfile::Canonical
    } else {
        options.layout_profile
    };
    // Auto needs both dimensions of an oversized plan. The existing internal overflow signal
    // also avoids painting an oversized candidate for Error, without invoking a fallback yet.
    let candidate_viewport = if automatic && viewport.overflow == OverflowPolicy::Error {
        viewport.overflow(OverflowPolicy::Fallback)
    } else {
        viewport
    };
    let first = render_candidate(
        options.with_layout_profile(first_profile),
        execution.with_viewport(candidate_viewport),
        &mut render,
    )?;
    let mut selected = SelectedLayout {
        candidate: first,
        profile: first_profile,
        compact_attempted: false,
    };
    if !automatic
        || selected
            .candidate
            .extent()
            .is_some_and(|extent| viewport.max_width.is_none_or(|limit| extent.width <= limit))
    {
        return Ok(selected);
    }
    selected.compact_attempted = true;
    // Under Allow, keep the canonical text but skip painting Compact when it cannot improve
    // width. Both candidates still use one operation and its cumulative work budget.
    let compact_viewport = if let Some(extent) = selected
        .candidate
        .extent()
        .filter(|_| viewport.overflow == OverflowPolicy::Allow)
    {
        AsciiViewportPolicy::with_max_width(extent.width - 1).overflow(OverflowPolicy::Fallback)
    } else {
        candidate_viewport
    };
    let (retained_bytes, retained_cells) = match &selected.candidate {
        LayoutCandidate::Measured {
            output: candidate, ..
        } => (
            candidate.metrics().encoded_bytes,
            candidate.metrics().document_cells,
        ),
        LayoutCandidate::Overflow(_) | LayoutCandidate::PlanningMiss(_) => (0, 0),
    };
    let compact_ledger = execution
        .new_resource_context(merman_model::OperationPhase::Layout)
        .scoped()
        .with_retained_output(retained_bytes, retained_cells);
    let compact_execution = execution
        .with_viewport(compact_viewport)
        .with_render_ledger(&compact_ledger)
        .with_optional_layout_candidate();
    let compact = match render_candidate(
        options.with_layout_profile(AsciiLayoutProfile::Compact),
        compact_execution,
        &mut render,
    ) {
        Ok(candidate) => candidate,
        Err(
            AsciiError::RoutingSearchExhausted { .. } | AsciiError::PortCapacityExceeded { .. },
        ) => {
            return Ok(selected);
        }
        Err(AsciiError::UnsupportedFeature { .. })
            if options.graph_presentation == AsciiGraphPresentation::Strict =>
        {
            return Ok(selected);
        }
        Err(error) => return Err(error),
    };
    if compact.extent().is_some_and(|compact| {
        selected
            .candidate
            .extent()
            .is_none_or(|primary| compact.width < primary.width)
    }) {
        selected.candidate = compact;
        selected.profile = AsciiLayoutProfile::Compact;
    }
    Ok(selected)
}

fn render_candidate(
    options: AsciiRenderOptions,
    execution: AsciiExecution<'_>,
    render: &mut impl FnMut(AsciiRenderOptions, AsciiExecution<'_>) -> Result<RenderedPrimary>,
) -> Result<LayoutCandidate> {
    match render(options, execution) {
        Ok(RenderedPrimary { text, facts }) => MeasuredOutput::measure(
            text,
            options.color_mode,
            options.terminal_width_profile,
            execution,
        )
        .map(|output| LayoutCandidate::Measured { output, facts }),
        Err(AsciiError::PrimaryViewportOverflow {
            actual_width,
            height,
            ..
        }) => Ok(LayoutCandidate::Overflow(AsciiExtent::new(
            actual_width,
            height,
        ))),
        Err(error)
            if options.graph_presentation == AsciiGraphPresentation::Adaptive
                && error.graph_planning_fallback_reason().is_some() =>
        {
            Ok(LayoutCandidate::PlanningMiss(error))
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::ResourceContext;
    use crate::{AsciiResourceLimitId, AsciiResourcePolicy, TerminalWidthProfile};
    use merman_model::{OperationControl, OperationPhase};

    fn rendered(text: impl Into<String>) -> RenderedPrimary {
        RenderedPrimary::new(text.into(), crate::AsciiProjection::Diagrammatic)
    }

    #[test]
    fn selection_keeps_the_winning_text_and_its_output_facts_together() {
        use crate::AsciiProjection;
        use crate::output::PrimaryDocument;

        let resources = AsciiResourcePolicy::unbounded();
        let control = OperationControl::new();
        for compact_wins in [false, true] {
            let selected = select(
                AsciiRenderOptions::ascii().with_layout_profile(AsciiLayoutProfile::Auto),
                AsciiViewportPolicy::with_max_width(6),
                AsciiExecution::new(&control, &resources),
                |options, _| {
                    if options.layout_profile == AsciiLayoutProfile::Canonical {
                        Ok(RenderedPrimary::records(
                            "0123456789".into(),
                            AsciiProjection::StructuredText,
                        ))
                    } else if compact_wins {
                        Ok(rendered("fits"))
                    } else {
                        Err(planning_miss())
                    }
                },
            )
            .unwrap();
            let LayoutCandidate::Measured { output, facts } = selected.candidate else {
                panic!("expected a measured incumbent")
            };
            if compact_wins {
                assert_eq!(output.text(), "fits");
                assert_eq!(facts.projection, AsciiProjection::Diagrammatic);
                assert_eq!(facts.document, PrimaryDocument::Admitted);
            } else {
                assert_eq!(output.text(), "0123456789");
                assert_eq!(facts.projection, AsciiProjection::StructuredText);
                assert_eq!(facts.document, PrimaryDocument::CompleteRecords);
            }
        }
    }

    fn over_width() -> AsciiError {
        AsciiError::PrimaryViewportOverflow {
            max_width: 6,
            actual_width: 10,
            height: 2,
            profile: TerminalWidthProfile::Unicode,
        }
    }

    #[test]
    fn rejected_first_layout_keeps_work_charged_to_the_second_attempt() {
        for limit in [11, 12] {
            let resources = AsciiResourcePolicy::default()
                .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, limit)
                .unwrap();
            let ledger = ResourceContext::new(resources);
            let control = OperationControl::new();
            let execution = AsciiExecution::new(&control, &resources).with_render_ledger(&ledger);
            let mut calls = 0;
            let result = select(
                AsciiRenderOptions::ascii().with_layout_profile(AsciiLayoutProfile::Auto),
                AsciiViewportPolicy::with_max_width(6).overflow(OverflowPolicy::Error),
                execution,
                |_, attempt| {
                    calls += 1;
                    attempt
                        .new_resource_context(OperationPhase::Layout)
                        .charge_layout_work(6)?;
                    Err(over_width())
                },
            );
            assert_eq!(calls, 2);
            if limit == 11 {
                assert!(matches!(result, Err(AsciiError::ResourceLimitExceeded(_))));
            } else {
                let selected = result.unwrap();
                assert_eq!(selected.profile, AsciiLayoutProfile::Canonical);
                assert!(selected.compact_attempted);
            }
        }
    }

    #[test]
    fn cancellation_during_compact_is_terminal() {
        let resources = AsciiResourcePolicy::default();
        let ledger = ResourceContext::new(resources);
        let control = OperationControl::new();
        let execution = AsciiExecution::new(&control, &resources).with_render_ledger(&ledger);
        let result = select(
            AsciiRenderOptions::ascii().with_layout_profile(AsciiLayoutProfile::Auto),
            AsciiViewportPolicy::with_max_width(6).overflow(OverflowPolicy::Error),
            execution,
            |options, attempt| {
                if options.layout_profile == AsciiLayoutProfile::Compact {
                    control.cancel();
                }
                attempt.checkpoint(OperationPhase::Layout)?;
                Err(over_width())
            },
        );
        assert!(matches!(result, Err(AsciiError::Cancelled(_))));
    }

    #[test]
    fn optional_route_failure_retains_the_valid_primary_but_initial_failure_propagates() {
        let resources = AsciiResourcePolicy::default();
        let control = OperationControl::new();
        for fail_first in [false, true] {
            let mut calls = 0;
            let result = select(
                AsciiRenderOptions::ascii().with_layout_profile(AsciiLayoutProfile::Auto),
                AsciiViewportPolicy::with_max_width(6),
                AsciiExecution::new(&control, &resources),
                |options, _| {
                    calls += 1;
                    if fail_first || options.layout_profile == AsciiLayoutProfile::Compact {
                        Err(AsciiError::UnsupportedFeature {
                            diagram_type: "flowchart",
                            feature: "test route conflict",
                        })
                    } else {
                        Ok(rendered("0123456789"))
                    }
                },
            );
            if fail_first {
                assert!(matches!(result, Err(AsciiError::UnsupportedFeature { .. })));
                assert_eq!(calls, 1);
            } else {
                let selected = result.unwrap();
                assert_eq!(calls, 2);
                assert_eq!(selected.profile, AsciiLayoutProfile::Canonical);
                assert_eq!(selected.candidate.extent(), Some(AsciiExtent::new(10, 1)));
            }
        }
    }

    #[test]
    fn retained_primary_bytes_are_reserved_during_compact_encoding() {
        for limit in [13, 14] {
            let resources = AsciiResourcePolicy::default()
                .with_limit(AsciiResourceLimitId::MaxOutputBytes, limit)
                .unwrap();
            let control = OperationControl::new();
            let result = select(
                AsciiRenderOptions::ascii().with_layout_profile(AsciiLayoutProfile::Auto),
                AsciiViewportPolicy::with_max_width(6),
                AsciiExecution::new(&control, &resources),
                |options, _| {
                    Ok(rendered(
                        if options.layout_profile == AsciiLayoutProfile::Canonical {
                            "0123456789".to_string()
                        } else {
                            "0123".to_string()
                        },
                    ))
                },
            );
            if limit == 13 {
                assert!(matches!(result, Err(AsciiError::ResourceLimitExceeded(_))));
            } else {
                assert_eq!(result.unwrap().profile, AsciiLayoutProfile::Compact);
            }
        }
    }

    fn planning_miss() -> AsciiError {
        AsciiError::RoutingSearchExhausted {
            diagram_type: "flowchart",
            edge_index: 7,
        }
    }

    #[test]
    fn adaptive_auto_may_recover_with_compact_but_explicit_profiles_remain_fixed() {
        let resources = AsciiResourcePolicy::default();
        let control = OperationControl::new();
        for profile in [
            AsciiLayoutProfile::Auto,
            AsciiLayoutProfile::Canonical,
            AsciiLayoutProfile::Compact,
        ] {
            let mut calls = 0;
            let selected = select(
                AsciiRenderOptions::ascii()
                    .with_graph_presentation(AsciiGraphPresentation::Adaptive)
                    .with_layout_profile(profile),
                AsciiViewportPolicy::with_max_width(6),
                AsciiExecution::new(&control, &resources),
                |_, _| {
                    calls += 1;
                    if calls == 1 {
                        Err(planning_miss())
                    } else {
                        Ok(rendered("fits"))
                    }
                },
            )
            .unwrap();
            assert_eq!(
                selected.compact_attempted,
                profile == AsciiLayoutProfile::Auto
            );
            if profile == AsciiLayoutProfile::Auto {
                assert_eq!(calls, 2);
                assert_eq!(selected.profile, AsciiLayoutProfile::Compact);
                assert_eq!(selected.candidate.extent(), Some(AsciiExtent::new(4, 1)));
            } else {
                assert_eq!(calls, 1);
                assert_eq!(selected.profile, profile);
                assert!(matches!(
                    selected.candidate,
                    LayoutCandidate::PlanningMiss(error) if error == planning_miss()
                ));
            }
        }
    }

    #[test]
    fn adaptive_optional_planning_miss_preserves_valid_canonical() {
        let resources = AsciiResourcePolicy::default();
        let control = OperationControl::new();
        for miss in [
            planning_miss(),
            AsciiError::PortCapacityExceeded {
                diagram_type: "flowchart",
                endpoint: "A".into(),
                required: 97,
                available: 96,
            },
        ] {
            let selected = select(
                AsciiRenderOptions::ascii()
                    .with_layout_profile(AsciiLayoutProfile::Auto)
                    .with_graph_presentation(AsciiGraphPresentation::Adaptive),
                AsciiViewportPolicy::with_max_width(6),
                AsciiExecution::new(&control, &resources),
                |options, _| {
                    if options.layout_profile == AsciiLayoutProfile::Canonical {
                        Ok(rendered("0123456789"))
                    } else {
                        Err(miss.clone())
                    }
                },
            )
            .unwrap();
            assert!(selected.compact_attempted);
            assert_eq!(selected.profile, AsciiLayoutProfile::Canonical);
            assert!(matches!(
                selected.candidate,
                LayoutCandidate::Measured { .. }
            ));
        }
    }

    #[test]
    fn adaptive_preserves_fatal_and_unsupported_errors_even_with_an_incumbent() {
        let resources = AsciiResourcePolicy::default();
        let control = OperationControl::new();
        for error in [
            AsciiError::allocation_failed("test_candidate"),
            AsciiError::PlanningWorkAllowanceExhausted,
            AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature: "unsupported test semantics",
            },
        ] {
            for fail_first in [false, true] {
                let result = select(
                    AsciiRenderOptions::ascii()
                        .with_layout_profile(AsciiLayoutProfile::Auto)
                        .with_graph_presentation(AsciiGraphPresentation::Adaptive),
                    AsciiViewportPolicy::with_max_width(6),
                    AsciiExecution::new(&control, &resources),
                    |options, _| {
                        if fail_first || options.layout_profile == AsciiLayoutProfile::Compact {
                            Err(error.clone())
                        } else {
                            Ok(rendered("0123456789"))
                        }
                    },
                );
                assert!(matches!(result, Err(actual) if actual == error));
            }
        }
    }

    #[test]
    fn adaptive_misses_share_the_work_ledger_and_operation_planning_allowances() {
        for limit in [11, 12] {
            let resources = AsciiResourcePolicy::default()
                .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, limit)
                .unwrap();
            let ledger = ResourceContext::new(resources);
            let planning = crate::operation::GraphPlanningBudget::default();
            let initial_expansions = planning.feasibility.remaining().0;
            let control = OperationControl::new();
            let execution = AsciiExecution::new(&control, &resources)
                .with_render_ledger(&ledger)
                .with_graph_planning(&planning);
            let mut calls = 0;
            let result = select(
                AsciiRenderOptions::ascii()
                    .with_layout_profile(AsciiLayoutProfile::Auto)
                    .with_graph_presentation(AsciiGraphPresentation::Adaptive),
                AsciiViewportPolicy::with_max_width(6),
                execution,
                |_, attempt| {
                    calls += 1;
                    attempt.graph_planning().unwrap().feasibility.debit(1, 0);
                    attempt
                        .new_resource_context(OperationPhase::Layout)
                        .charge_layout_work(6)?;
                    Err(planning_miss())
                },
            );
            assert_eq!(calls, 2);
            assert_eq!(planning.feasibility.remaining().0, initial_expansions - 2);
            if limit == 11 {
                assert!(matches!(result, Err(AsciiError::ResourceLimitExceeded(_))));
            } else {
                let selected = result.unwrap();
                assert!(selected.compact_attempted);
                assert!(matches!(
                    selected.candidate,
                    LayoutCandidate::PlanningMiss(_)
                ));
                assert_eq!(ledger.layout_work_used(), 12);
            }
        }
    }
}
