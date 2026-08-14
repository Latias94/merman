mod boxes;
mod control;
mod events;
mod layout;
mod lifecycle;
mod model;
mod notes;
mod plan;
mod prepared_body;
mod render;
mod text;
mod tree;
mod validate;

use crate::error::Result;
use crate::operation::AsciiExecution;
use crate::options::TerminalWidthProfile;
use crate::resource::ResourceContext;
use crate::safe_text::{
    LabelBreakPolicy, NormalizedLabelPlan, charge_text_layout,
    try_plan_normalized_label_lines_with_policy,
};
use merman_core::OperationPhase;

pub(crate) use model::from_sequence_model;
pub(crate) use render::render_sequence_diagram_with_execution;

#[derive(Debug, Clone, Copy)]
struct SequenceCheckpointCursor<'a> {
    execution: AsciiExecution<'a>,
    phase: OperationPhase,
    iteration: usize,
}

#[derive(Debug, Clone, Copy)]
struct SequenceActorRenderState<'a> {
    active_counts: &'a [usize],
    visible_actors: &'a [bool],
}

impl<'a> SequenceActorRenderState<'a> {
    const fn new(active_counts: &'a [usize], visible_actors: &'a [bool]) -> Self {
        Self {
            active_counts,
            visible_actors,
        }
    }
}

impl<'a> SequenceCheckpointCursor<'a> {
    const fn new(execution: AsciiExecution<'a>, phase: OperationPhase) -> Self {
        Self {
            execution,
            phase,
            iteration: 0,
        }
    }

    fn tick(&mut self) -> Result<()> {
        let iteration = self.iteration;
        self.iteration = self.iteration.wrapping_add(1);
        self.execution.checkpoint_loop(self.phase, iteration)
    }

    fn before_charge(&self) -> Result<()> {
        self.checkpoint()
    }

    fn checkpoint(&self) -> Result<()> {
        self.execution.checkpoint(self.phase)
    }

    const fn execution(&self) -> AsciiExecution<'a> {
        self.execution
    }
}

fn charge_sequence_projection_text(
    resources: &ResourceContext,
    value: &str,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    execution.checkpoint(OperationPhase::Semantic)?;
    let scratch = ResourceContext::new(resources.policy());
    let charged = charge_text_layout(&scratch, value);
    execution.checkpoint(OperationPhase::Semantic)?;
    charged?;
    execution.checkpoint(OperationPhase::Semantic)?;
    resources.charge_layout_work(scratch.layout_work_used())
}

#[allow(clippy::too_many_arguments)]
fn try_plan_sequence_projection_label(
    raw: &str,
    width_profile: TerminalWidthProfile,
    trim: bool,
    wrap_width: Option<usize>,
    break_policy: LabelBreakPolicy,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Option<NormalizedLabelPlan>> {
    try_plan_sequence_label_impl(
        raw,
        width_profile,
        trim,
        wrap_width,
        break_policy,
        resources,
        || execution.checkpoint(OperationPhase::Semantic),
    )
}

#[allow(clippy::too_many_arguments)]
fn try_plan_sequence_label(
    raw: &str,
    width_profile: TerminalWidthProfile,
    trim: bool,
    wrap_width: Option<usize>,
    break_policy: LabelBreakPolicy,
    resources: &ResourceContext,
    checkpoints: &SequenceCheckpointCursor<'_>,
) -> Result<Option<NormalizedLabelPlan>> {
    try_plan_sequence_label_impl(
        raw,
        width_profile,
        trim,
        wrap_width,
        break_policy,
        resources,
        || checkpoints.before_charge(),
    )
}

#[allow(clippy::too_many_arguments)]
fn try_plan_sequence_label_impl(
    raw: &str,
    width_profile: TerminalWidthProfile,
    trim: bool,
    wrap_width: Option<usize>,
    break_policy: LabelBreakPolicy,
    resources: &ResourceContext,
    mut checkpoint: impl FnMut() -> Result<()>,
) -> Result<Option<NormalizedLabelPlan>> {
    checkpoint()?;
    let scratch = ResourceContext::new(resources.policy());
    let planned = try_plan_normalized_label_lines_with_policy(
        raw,
        width_profile,
        trim,
        wrap_width,
        break_policy,
        &scratch,
    );
    checkpoint()?;
    let plan = planned?;
    checkpoint()?;
    resources.charge_layout_work(scratch.layout_work_used())?;
    Ok(plan)
}

const BOX_PADDING_LEFT_RIGHT: usize = 2;
const MIN_BOX_WIDTH: usize = 3;
const BOX_BORDER_WIDTH: usize = 2;
const LABEL_LEFT_MARGIN: usize = 2;
const LABEL_BUFFER_SPACE: usize = 10;
const NOTE_SIDE_GAP: usize = 2;
const NOTE_WRAP_TEXT_WIDTH: usize = 24;
const SEQUENCE_ACTOR_WRAP_TEXT_WIDTH: usize = 12;
const SEQUENCE_BOX_WRAP_TEXT_WIDTH: usize = 12;
const SEQUENCE_BOX_CONTENT_OFFSET: usize = BOX_BORDER_WIDTH;
const SEQUENCE_BOX_LABEL_MARGIN: usize = 2;

fn projection_allocation_failed() -> crate::error::AsciiError {
    crate::error::AsciiError::AllocationFailed {
        phase: crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AsciiError;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::{CancelReason, OperationControl};

    #[test]
    fn projection_text_cancellation_precedes_an_inner_resource_ceiling() {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .expect("one work unit should be a valid limit");
        let resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        control.cancel_after_checkpoints(1);

        let error = charge_sequence_projection_text(
            &resources,
            "AB",
            AsciiExecution::new(&control, &policy),
        )
        .expect_err("the post-scan checkpoint should win over the scratch ceiling");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Semantic
                    && cancelled.reason == CancelReason::Requested
        ));
        assert_eq!(resources.layout_work_used(), 0);
    }

    #[test]
    fn label_planning_cancellation_precedes_an_inner_resource_ceiling() {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .expect("one work unit should be a valid limit");
        let resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        control.cancel_after_checkpoints(1);
        let checkpoints = SequenceCheckpointCursor::new(
            AsciiExecution::new(&control, &policy),
            OperationPhase::Layout,
        );

        let error = try_plan_sequence_label(
            "AB",
            TerminalWidthProfile::Unicode,
            false,
            None,
            LabelBreakPolicy::VisibleLine,
            &resources,
            &checkpoints,
        )
        .expect_err("the post-plan checkpoint should win over the scratch ceiling");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested
        ));
        assert_eq!(resources.layout_work_used(), 0);
    }

    #[test]
    fn cadence_never_suppresses_the_checkpoint_immediately_before_a_charge() {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .expect("one work unit should be a valid limit");
        let resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        control.cancel_after_checkpoints(1);
        let mut checkpoints = SequenceCheckpointCursor::new(
            AsciiExecution::new(&control, &policy),
            OperationPhase::Layout,
        );

        checkpoints
            .tick()
            .expect("the cadence checkpoint at iteration zero should succeed");
        for _ in 1..64 {
            checkpoints
                .tick()
                .expect("intermediate cadence ticks should not poll the control");
        }
        let error = (|| {
            checkpoints.before_charge()?;
            resources.charge_layout_work(2)
        })()
        .expect_err("the direct pre-charge checkpoint should observe cancellation");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested
        ));
        assert_eq!(resources.layout_work_used(), 0);
    }
}
