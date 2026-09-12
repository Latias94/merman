//! Bounded family-layout selection before applying the final viewport overflow policy.

use crate::operation::AsciiExecution;
use crate::output::MeasuredOutput;
use crate::{
    AsciiError, AsciiExtent, AsciiLayoutProfile, AsciiRenderOptions, AsciiViewportPolicy,
    OverflowPolicy, Result,
};

pub(crate) enum LayoutCandidate {
    Measured(MeasuredOutput),
    Overflow(AsciiExtent),
}

impl LayoutCandidate {
    fn extent(&self) -> AsciiExtent {
        match self {
            Self::Measured(output) => output.metrics().extent,
            Self::Overflow(extent) => *extent,
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
    mut render: impl FnMut(AsciiRenderOptions, AsciiExecution<'_>) -> Result<String>,
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
        || viewport
            .max_width
            .is_none_or(|limit| selected.candidate.extent().width <= limit)
    {
        return Ok(selected);
    }
    selected.compact_attempted = true;
    // Under Allow, keep the canonical text but skip painting Compact when it cannot improve
    // width. Both candidates still use one operation and its cumulative work budget.
    let compact_viewport = if viewport.overflow == OverflowPolicy::Allow {
        AsciiViewportPolicy::with_max_width(selected.candidate.extent().width - 1)
            .overflow(OverflowPolicy::Fallback)
    } else {
        candidate_viewport
    };
    let (retained_bytes, retained_cells) = match &selected.candidate {
        LayoutCandidate::Measured(candidate) => (
            candidate.metrics().encoded_bytes,
            candidate.metrics().document_cells,
        ),
        LayoutCandidate::Overflow(_) => (0, 0),
    };
    let compact_ledger = execution
        .new_resource_context(merman_core::OperationPhase::Layout)
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
        Err(AsciiError::UnsupportedFeature { .. }) => return Ok(selected),
        Err(error) => return Err(error),
    };
    if compact.extent().width < selected.candidate.extent().width {
        selected.candidate = compact;
        selected.profile = AsciiLayoutProfile::Compact;
    }
    Ok(selected)
}

fn render_candidate(
    options: AsciiRenderOptions,
    execution: AsciiExecution<'_>,
    render: &mut impl FnMut(AsciiRenderOptions, AsciiExecution<'_>) -> Result<String>,
) -> Result<LayoutCandidate> {
    match render(options, execution) {
        Ok(text) => MeasuredOutput::measure(
            text,
            options.color_mode,
            options.terminal_width_profile,
            execution,
        )
        .map(LayoutCandidate::Measured),
        Err(AsciiError::PrimaryViewportOverflow {
            actual_width,
            height,
            ..
        }) => Ok(LayoutCandidate::Overflow(AsciiExtent::new(
            actual_width,
            height,
        ))),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::ResourceContext;
    use crate::{AsciiResourceLimitId, AsciiResourcePolicy, TerminalWidthProfile};
    use merman_core::{OperationControl, OperationPhase};

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
                        Ok("0123456789".to_string())
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
                assert_eq!(selected.candidate.extent().width, 10);
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
                    Ok(if options.layout_profile == AsciiLayoutProfile::Canonical {
                        "0123456789".to_string()
                    } else {
                        "0123".to_string()
                    })
                },
            );
            if limit == 13 {
                assert!(matches!(result, Err(AsciiError::ResourceLimitExceeded(_))));
            } else {
                assert_eq!(result.unwrap().profile, AsciiLayoutProfile::Compact);
            }
        }
    }
}
