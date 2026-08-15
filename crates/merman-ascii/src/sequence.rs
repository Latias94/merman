mod boxes;
mod chars;
mod control;
mod event_paint;
mod event_plan;
mod layout;
mod lifecycle;
mod lifeline;
mod model;
mod notes;
mod plan;
mod prepared_body;
mod render;
mod row_document;
mod text;
mod tree;
mod validate;

use crate::error::Result;
use crate::operation::AsciiExecution;
use crate::options::TerminalWidthProfile;
use crate::resource::ResourceContext;
use crate::safe_text::{
    LabelBreakPolicy, NormalizedLabelPlan, charge_text_layout,
    try_plan_normalized_label_lines_with_policy_and_checkpoint,
};
use merman_core::OperationPhase;

pub(crate) use model::from_sequence_model;
pub(crate) use render::render_sequence_diagram_with_execution;

#[derive(Debug, Clone, Copy)]
struct SequenceCheckpointCursor<'a> {
    execution: AsciiExecution<'a>,
    phase: OperationPhase,
    /// The counter is carried into the next phase so a pass cannot reset its cooperative cadence
    /// when it moves from layout planning to output emission.
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

    fn next_phase(&self, phase: OperationPhase) -> Self {
        Self {
            execution: self.execution,
            phase,
            iteration: self.iteration,
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
    resources.transaction(|resources| {
        execution.checkpoint(OperationPhase::Semantic)?;
        charge_text_layout(resources, value)?;
        execution.checkpoint(OperationPhase::Semantic)
    })
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
    resources.transaction(|resources| {
        checkpoint()?;
        let plan = try_plan_normalized_label_lines_with_policy_and_checkpoint(
            raw,
            width_profile,
            trim,
            wrap_width,
            break_policy,
            resources,
            &mut checkpoint,
        )?;
        checkpoint()?;
        Ok(plan)
    })
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
        let base_resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        control.cancel_after_checkpoints(3);
        let execution = AsciiExecution::new(&control, &policy);
        let resources = execution.resource_context(&base_resources, OperationPhase::Semantic);

        let error = charge_sequence_projection_text(&resources, "AB", execution)
            .expect_err("inner semantic cancellation should win over the shared ceiling");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Semantic
                    && cancelled.reason == CancelReason::Requested
        ));
        assert_eq!(base_resources.layout_work_used(), 0);
    }

    #[test]
    fn projection_text_uses_the_shared_ledger_before_scanning_the_next_grapheme() {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 10)
            .expect("ten work units should be a valid limit")
            .with_limit(AsciiResourceLimitId::MaxGraphemeBytes, 1)
            .expect("one grapheme byte should be a valid limit");
        let base_resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        let execution = AsciiExecution::new(&control, &policy);
        let resources = execution.resource_context(&base_resources, OperationPhase::Semantic);
        resources
            .charge_layout_work(9)
            .expect("the shared ledger should start one unit below its ceiling");

        let error = charge_sequence_projection_text(&resources, "Aé", execution)
            .expect_err("the first grapheme should exhaust work before the second is inspected");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == 11
                    && details.max == 10
        ));
        assert_eq!(base_resources.layout_work_used(), 9);
    }

    #[test]
    fn label_planning_cancellation_precedes_an_inner_resource_ceiling() {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .expect("one work unit should be a valid limit");
        let base_resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        control.cancel_after_checkpoints(1);
        let execution = AsciiExecution::new(&control, &policy);
        let resources = execution.resource_context(&base_resources, OperationPhase::Layout);
        let checkpoints = SequenceCheckpointCursor::new(execution, OperationPhase::Layout);

        let error = try_plan_sequence_label(
            "AB",
            TerminalWidthProfile::Unicode,
            false,
            None,
            LabelBreakPolicy::VisibleLine,
            &resources,
            &checkpoints,
        )
        .expect_err("layout cancellation should win over the shared ceiling");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested
        ));
        assert_eq!(base_resources.layout_work_used(), 0);
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

    #[test]
    fn phase_transition_preserves_the_monotonic_checkpoint_cadence() {
        let policy = AsciiResourcePolicy::default();
        let control = OperationControl::new();
        control.cancel_after_checkpoints(1);
        let execution = AsciiExecution::new(&control, &policy);
        let mut layout = SequenceCheckpointCursor::new(execution, OperationPhase::Layout);

        layout
            .tick()
            .expect("the first layout cadence checkpoint should succeed");
        for _ in 1..64 {
            layout
                .tick()
                .expect("intermediate layout ticks should not poll the control");
        }

        let mut emit = layout.next_phase(OperationPhase::Emit);
        let error = emit
            .tick()
            .expect_err("emit must continue at iteration 64 instead of resetting to zero");
        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Emit
                    && cancelled.reason == CancelReason::Requested
        ));
    }

    #[test]
    fn mermaid_label_scanner_cancels_without_an_authored_break() {
        let raw = "A".repeat(256);
        let policy = AsciiResourcePolicy::default();
        let base_resources = ResourceContext::new(policy);
        base_resources
            .charge_layout_work(7)
            .expect("the pre-existing work debit should fit");
        let control = OperationControl::new();
        control.cancel_after_checkpoints(4);
        let execution = AsciiExecution::new(&control, &policy);
        let resources = execution.resource_context(&base_resources, OperationPhase::Layout);
        let checkpoints = SequenceCheckpointCursor::new(execution, OperationPhase::Layout);

        let error = try_plan_sequence_label(
            &raw,
            TerminalWidthProfile::Unicode,
            false,
            None,
            LabelBreakPolicy::MermaidLabelBreaks,
            &resources,
            &checkpoints,
        )
        .expect_err("the scanner should stop inside a label without any authored break token");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested
        ));
        assert_eq!(base_resources.layout_work_used(), 7);
    }
}
