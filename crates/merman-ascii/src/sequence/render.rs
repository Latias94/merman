use super::SequenceCheckpointCursor;
use super::chars::SequenceChars;
use super::layout::calculate_layout_with_resources;
use super::model::AsciiSequenceDiagram;
use super::notes::apply_note_gutters;
use super::plan::build_sequence_row_document;
use crate::error::{AsciiError, Result};
use crate::operation::AsciiExecution;
use crate::options::AsciiRenderOptions;
#[cfg(test)]
use crate::resource::AsciiResourcePolicy;
use crate::resource::ResourceContext;
use merman_core::OperationPhase;

#[cfg(test)]
pub(crate) fn render_sequence_diagram(
    diagram: &AsciiSequenceDiagram,
    options: &AsciiRenderOptions,
    policy: &AsciiResourcePolicy,
) -> Result<String> {
    let mut resources = ResourceContext::new(*policy);
    render_sequence_diagram_inner(
        diagram,
        options,
        &mut resources,
        AsciiExecution::for_test(policy),
    )
}

pub(crate) fn render_sequence_diagram_with_execution(
    diagram: &AsciiSequenceDiagram,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    debug_assert_eq!(resources.policy(), *execution.resources());
    render_sequence_diagram_inner(diagram, options, resources, execution)
}

fn render_sequence_diagram_inner(
    diagram: &AsciiSequenceDiagram,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let transaction = resources.clone();
    transaction.transaction(|_| {
        render_sequence_diagram_transactional(diagram, options, resources, execution)
    })
}

fn render_sequence_diagram_transactional(
    diagram: &AsciiSequenceDiagram,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    options.validate()?;
    if diagram.participants.is_empty() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "no participants",
        });
    }

    execution.checkpoint(merman_core::OperationPhase::Layout)?;
    debug_assert_eq!(resources.policy(), *execution.resources());
    let mut layout_resources = execution.resource_context(resources, OperationPhase::Layout);
    let chars = SequenceChars::for_options(options);
    let mut layout_checkpoints = SequenceCheckpointCursor::new(execution, OperationPhase::Layout);
    let mut layout = calculate_layout_with_resources(
        diagram,
        options,
        &mut layout_resources,
        &mut layout_checkpoints,
    )?;
    apply_note_gutters(
        diagram,
        &mut layout,
        &mut layout_resources,
        &mut layout_checkpoints,
    )?;
    let row_document = build_sequence_row_document(
        diagram,
        &layout,
        &chars,
        options.sequence_mirror_actors,
        &mut layout_resources,
        &mut layout_checkpoints,
    )?;
    row_document.render(
        diagram,
        &layout,
        &chars,
        options,
        &mut layout_resources,
        &mut layout_checkpoints,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::TerminalWidthProfile;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use crate::sequence::model::{
        SequenceActorLifecycle, SequenceParticipant, SequenceParticipantLabel,
    };

    #[test]
    fn sequence_grid_extent_accepts_exact_limit_and_rejects_limit_minus_one() {
        let diagram = single_participant_diagram();
        let options = AsciiRenderOptions::ascii();
        let renders_with_limit = |limit| {
            let policy = AsciiResourcePolicy::default()
                .with_limit(AsciiResourceLimitId::MaxGridCells, limit)
                .expect("valid grid limit");
            render_sequence_diagram(&diagram, &options, &policy)
        };

        let mut upper = 1usize;
        while renders_with_limit(upper).is_err() {
            upper = upper.checked_mul(2).expect("grid boundary must fit usize");
        }
        let mut lower = 1usize;
        while lower < upper {
            let midpoint = lower + (upper - lower) / 2;
            if renders_with_limit(midpoint).is_ok() {
                upper = midpoint;
            } else {
                lower = midpoint + 1;
            }
        }
        let exact_cells = lower;

        let exact = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, exact_cells)
            .unwrap();
        assert!(render_sequence_diagram(&diagram, &options, &exact).is_ok());

        let below = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, exact_cells - 1)
            .unwrap();
        let error = render_sequence_diagram(&diagram, &options, &below).unwrap_err();
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGridCells
                    && details.actual == exact_cells
                    && details.max == exact_cells - 1
        ));
    }

    #[test]
    fn sequence_plain_output_enforces_custom_grapheme_limit() {
        let mut diagram = single_participant_diagram();
        diagram.participants[0].label =
            SequenceParticipantLabel::from_raw("👨‍👩‍👧‍👦", false, TerminalWidthProfile::Unicode);
        let options = AsciiRenderOptions::ascii();
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGraphemeBytes, 4)
            .unwrap();

        let error = render_sequence_diagram(&diagram, &options, &policy).unwrap_err();
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGraphemeBytes
                    && details.actual > details.max
        ));
    }

    #[test]
    fn final_output_failure_restores_layout_and_document_ledgers() {
        let mut diagram = single_participant_diagram();
        diagram.participants[0].label =
            SequenceParticipantLabel::from_raw("P", false, TerminalWidthProfile::Unicode);
        let options = AsciiRenderOptions::ascii();
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, 1)
            .expect("one output byte should be a valid limit");
        let mut resources = ResourceContext::new(policy);
        resources
            .charge_layout_work(5)
            .expect("the pre-existing work debit should fit");
        resources
            .charge_document_cells(3)
            .expect("the pre-existing document debit should fit");
        let control = merman_core::OperationControl::new();

        let error = render_sequence_diagram_with_execution(
            &diagram,
            &options,
            &mut resources,
            AsciiExecution::new(&control, &policy),
        )
        .expect_err("the complete rendered document should exceed one output byte");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxOutputBytes
                    && details.actual > details.max
        ));
        assert_eq!(resources.layout_work_used(), 5);
        assert_eq!(resources.document_cells_used(), 3);
    }

    fn single_participant_diagram() -> AsciiSequenceDiagram {
        AsciiSequenceDiagram {
            title: None,
            participants: vec![SequenceParticipant {
                id: "p0".to_string(),
                label: SequenceParticipantLabel::from_raw(
                    "P0",
                    false,
                    TerminalWidthProfile::Unicode,
                ),
            }],
            lifecycles: vec![SequenceActorLifecycle::default()],
            boxes: Vec::new(),
            body: crate::sequence::tree::SequenceBody::default(),
        }
    }
}
