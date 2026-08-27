use super::SequenceCheckpointCursor;
use super::chars::SequenceChars;
use super::layout::calculate_layout_with_policy;
#[cfg(test)]
use super::layout::calculate_layout_with_resources;
use super::model::AsciiSequenceDiagram;
use super::notes::apply_note_gutters;
use super::plan::prepare_sequence_row_document;
use super::row_document::{prepare_sequence_document, prepare_sequence_title};
use crate::error::{AsciiError, Result};
use crate::operation::AsciiExecution;
use crate::options::{AsciiRenderOptions, SequenceLayoutPolicy};
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
        None,
        options,
        options.sequence_layout(),
        &mut resources,
        AsciiExecution::for_test(policy),
    )
}

#[cfg(test)]
pub(crate) fn render_sequence_diagram_with_execution(
    diagram: &AsciiSequenceDiagram,
    title: Option<&str>,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    render_sequence_diagram_with_resolved_policy(
        diagram,
        title,
        options,
        options.sequence_layout(),
        resources,
        execution,
    )
}

pub(crate) fn render_sequence_diagram_with_resolved_policy(
    diagram: &AsciiSequenceDiagram,
    title: Option<&str>,
    options: &AsciiRenderOptions,
    layout_policy: SequenceLayoutPolicy,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    debug_assert_eq!(resources.policy(), *execution.resources());
    render_sequence_diagram_inner(diagram, title, options, layout_policy, resources, execution)
}

fn render_sequence_diagram_inner(
    diagram: &AsciiSequenceDiagram,
    title: Option<&str>,
    options: &AsciiRenderOptions,
    layout_policy: SequenceLayoutPolicy,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let transaction = resources.clone();
    transaction.transaction(|_| {
        let result = transaction.transaction_preserving_layout_work(|_| {
            render_sequence_diagram_transactional(
                diagram,
                title,
                options,
                layout_policy,
                resources,
                execution,
            )
        });
        match result {
            // A complete semantic fallback reuses the same render-wide ledger. Preserve the work
            // spent proving that the primary exceeds the viewport, but discard its speculative
            // document cells before the fallback candidate starts.
            Err(error @ AsciiError::PrimaryViewportOverflow { .. }) => Ok(Err(error)),
            Ok(rendered) => Ok(Ok(rendered)),
            Err(error) => Err(error),
        }
    })?
}

fn render_sequence_diagram_transactional(
    diagram: &AsciiSequenceDiagram,
    title: Option<&str>,
    options: &AsciiRenderOptions,
    layout_policy: SequenceLayoutPolicy,
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
    let chars = SequenceChars::for_charset(layout_policy.structural_charset);
    let mut layout_checkpoints = SequenceCheckpointCursor::new(execution, OperationPhase::Layout);
    let title = prepare_sequence_title(
        title,
        layout_policy.terminal_width_profile,
        &mut layout_resources,
        &mut layout_checkpoints,
    )?;
    let mut layout = calculate_layout_with_policy(
        diagram,
        layout_policy,
        &mut layout_resources,
        &mut layout_checkpoints,
    )?;
    apply_note_gutters(
        diagram,
        &mut layout,
        &mut layout_resources,
        &mut layout_checkpoints,
    )?;
    let row_plan = prepare_sequence_row_document(
        diagram,
        &layout,
        &chars,
        &mut layout_resources,
        &mut layout_checkpoints,
    )?;
    let row_output_plan = row_plan.output_plan();
    let row_materialized_cells = row_output_plan
        .extent()
        .materialized_cells(&layout_resources)?;
    let document = prepare_sequence_document(
        diagram,
        title,
        row_output_plan,
        &layout,
        &mut layout_resources,
        &mut layout_checkpoints,
    )?;
    let mut row_materialization_resources =
        layout_resources.scoped_after_document_admission(row_materialized_cells)?;
    let row_document = row_plan.materialize(
        diagram,
        &layout,
        &chars,
        &mut row_materialization_resources,
        &mut layout_checkpoints,
    )?;
    row_document.render(
        document,
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
        SequenceActorLifecycle, SequenceGroupBox, SequenceParticipant, SequenceParticipantLabel,
    };
    use crate::sequence::text::SequenceDocumentExtent;

    #[test]
    fn sequence_grid_extent_accepts_exact_limit_and_rejects_limit_minus_one() {
        let mut diagram = single_participant_diagram();
        diagram.participants[0].label = SequenceParticipantLabel::from_raw(
            "first<br>second",
            false,
            TerminalWidthProfile::Unicode,
        );
        let options = AsciiRenderOptions::ascii();
        let renders_with_limit = |limit| {
            let policy = AsciiResourcePolicy::default()
                .with_limit(AsciiResourceLimitId::MaxGridCells, limit)
                .expect("valid grid limit");
            let mut resources = ResourceContext::new(policy);
            let control = merman_core::OperationControl::new();
            render_sequence_diagram_with_execution(
                &diagram,
                None,
                &options,
                &mut resources,
                AsciiExecution::new(&control, &policy),
            )
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

        let rendered = renders_with_limit(exact_cells)
            .expect("the exact aggregate grid should materialize the multi-line participant");
        assert!(rendered.lines().any(|line| line.contains("first")));
        assert!(rendered.lines().any(|line| line.contains("second")));

        let below = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, exact_cells - 1)
            .unwrap();
        let mut resources = ResourceContext::new(below);
        resources
            .charge_layout_work(5)
            .expect("the pre-existing work debit should fit");
        resources
            .charge_document_cells(3)
            .expect("the pre-existing document debit should fit");
        let control = merman_core::OperationControl::new();
        let error = render_sequence_diagram_with_execution(
            &diagram,
            None,
            &options,
            &mut resources,
            AsciiExecution::new(&control, &below),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGridCells
                    && details.actual == exact_cells
                    && details.max == exact_cells - 1
        ));
        assert_eq!(resources.layout_work_used(), 5);
        assert_eq!(resources.document_cells_used(), 3);
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
    fn blank_title_uses_the_final_retained_grid_extent() {
        let diagram = single_participant_diagram();
        let options = AsciiRenderOptions::ascii();
        let title = " ".repeat(31);
        let measured = prepare_document_without_materializing(
            &diagram,
            &title,
            &options,
            AsciiResourcePolicy::default(),
        )
        .expect("the blank title should have a measurable final extent");
        let exact_cells = measured
            .width()
            .checked_mul(measured.height())
            .expect("the final sequence grid should fit usize");
        assert_eq!(exact_cells, 30);

        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, exact_cells)
            .expect("the exact final grid limit should be valid");
        let mut resources = ResourceContext::new(policy);
        let control = merman_core::OperationControl::new();

        let rendered = render_sequence_diagram_with_execution(
            &diagram,
            Some(&title),
            &options,
            &mut resources,
            AsciiExecution::new(&control, &policy),
        )
        .expect("the blank title must not be rejected by its discarded alignment width");
        assert_eq!(rendered.lines().next(), Some(""));
    }

    #[test]
    fn combined_title_and_box_grid_is_admitted_before_row_materialization() {
        let mut diagram = single_participant_diagram();
        diagram.boxes.push(SequenceGroupBox {
            actor_indices: vec![0],
            label: Some("group".to_string()),
            background: None,
            wrap: false,
        });
        let options = AsciiRenderOptions::ascii();
        let title = "T".repeat(40);
        let measured = prepare_document_without_materializing(
            &diagram,
            &title,
            &options,
            AsciiResourcePolicy::default(),
        )
        .expect("the default policy should admit the planned sequence document");
        let exact_cells = measured
            .width()
            .checked_mul(measured.height())
            .expect("the planned sequence grid should fit usize");

        let exact = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, exact_cells)
            .expect("the exact sequence grid limit should be valid");
        let admitted = prepare_document_without_materializing(&diagram, &title, &options, exact)
            .expect("the exact title, box, and body extent should be admitted");
        assert_eq!(admitted, measured);

        let below = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, exact_cells - 1)
            .expect("the limit below the sequence grid should be valid");
        let error = prepare_document_without_materializing(&diagram, &title, &options, below)
            .expect_err("the combined grid must reject before row materialization is invoked");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGridCells
                    && details.actual == exact_cells
                    && details.max == exact_cells - 1
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
            None,
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

    fn prepare_document_without_materializing(
        diagram: &AsciiSequenceDiagram,
        title: &str,
        options: &AsciiRenderOptions,
        policy: AsciiResourcePolicy,
    ) -> Result<SequenceDocumentExtent> {
        let resources = ResourceContext::new(policy);
        let execution = AsciiExecution::for_test(&policy);
        let mut layout_resources = execution.resource_context(&resources, OperationPhase::Layout);
        let mut checkpoints = SequenceCheckpointCursor::new(execution, OperationPhase::Layout);
        let title = prepare_sequence_title(
            Some(title),
            options.sequence_layout().terminal_width_profile,
            &mut layout_resources,
            &mut checkpoints,
        )?;
        let mut layout = calculate_layout_with_resources(
            diagram,
            options,
            &mut layout_resources,
            &mut checkpoints,
        )?;
        apply_note_gutters(
            diagram,
            &mut layout,
            &mut layout_resources,
            &mut checkpoints,
        )?;
        let chars = SequenceChars::for_options(options);
        let rows = prepare_sequence_row_document(
            diagram,
            &layout,
            &chars,
            &mut layout_resources,
            &mut checkpoints,
        )?;
        let document = prepare_sequence_document(
            diagram,
            title,
            rows.output_plan(),
            &layout,
            &mut layout_resources,
            &mut checkpoints,
        )?;
        Ok(document.output_extent())
    }
}
