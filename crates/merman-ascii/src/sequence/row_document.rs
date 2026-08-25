use super::SequenceCheckpointCursor;
use super::boxes::{PreparedSequenceBoxes, prepare_sequence_boxes};
use super::chars::SequenceChars;
use super::layout::SequenceLayout;
use super::model::AsciiSequenceDiagram;
use super::text::{SequenceDocumentExtent, SequenceDocumentPlan};
use super::text::{SequenceLine, blank_line_with_checkpoints};
use crate::color::{AsciiColorMode, AsciiColorRole};
use crate::error::{AsciiError, Result};
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
use crate::resource::{
    AsciiResourceLimitId, AsciiResourceLimitPhase, CheckedOutput, ResourceContext,
};
use crate::safe_text::visit_safe_line_graphemes;
use crate::terminal::{SurfaceCellCheckpoints, TerminalCellText, primary_width_with_checkpoints};
use merman_core::OperationPhase;

#[derive(Debug)]
pub(super) struct SequenceRowDocument {
    lines: Vec<SequenceLine>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PreparedSequenceTitle<'a> {
    text: &'a str,
    alignment_width: usize,
    retained_width: usize,
    width_profile: TerminalWidthProfile,
}

#[derive(Debug, Clone, Copy)]
struct SequenceTitleWidths {
    alignment: usize,
    retained: usize,
}

#[derive(Debug)]
pub(super) struct PreparedSequenceDocument<'a> {
    title: Option<PreparedSequenceTitle<'a>>,
    boxes: Option<PreparedSequenceBoxes<'a>>,
    content_extent: SequenceDocumentExtent,
    output_extent: SequenceDocumentExtent,
}

impl PreparedSequenceDocument<'_> {
    #[cfg(test)]
    pub(super) const fn content_extent(&self) -> SequenceDocumentExtent {
        self.content_extent
    }

    #[cfg(test)]
    pub(super) const fn output_extent(&self) -> SequenceDocumentExtent {
        self.output_extent
    }
}

impl SequenceRowDocument {
    pub(super) fn new(lines: Vec<SequenceLine>) -> Self {
        Self { lines }
    }

    pub(super) fn render(
        self,
        document: PreparedSequenceDocument<'_>,
        layout: &SequenceLayout,
        chars: &SequenceChars,
        options: &AsciiRenderOptions,
        resources: &mut ResourceContext,
        layout_checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<String> {
        let mut lines = self.lines;
        if let Some(boxes) = document.boxes {
            let materialized_cells = boxes.materialized_cells(resources)?;
            let mut materialization_resources =
                resources.scoped_after_document_admission(materialized_cells)?;
            lines = boxes.materialize(
                lines,
                layout,
                chars,
                &mut materialization_resources,
                layout_checkpoints,
            )?;
        }
        validate_lines_extent(
            &lines,
            document.content_extent,
            resources,
            layout_checkpoints,
        )?;
        if let Some(title) = document.title {
            prepend_title_line(
                &mut lines,
                title,
                document.content_extent.width(),
                resources,
                layout_checkpoints,
            )?;
        }
        validate_lines_extent(
            &lines,
            document.output_extent,
            resources,
            layout_checkpoints,
        )?;
        let output_extent = document.output_extent;
        layout_checkpoints.execution().admit_primary_extent(
            output_extent.width(),
            output_extent.height(),
            options.terminal_width_profile,
        )?;
        // Box/title geometry, extent admission, and canvas construction are layout work. Only
        // after those complete do we bind the shared ledger to Emit for byte/document emission.
        let mut emit_resources = resources.with_operation_phase(OperationPhase::Emit);
        let mut emit_checkpoints = layout_checkpoints.next_phase(OperationPhase::Emit);
        finish_sequence_lines(lines, options, &mut emit_resources, &mut emit_checkpoints)
    }
}

pub(super) fn prepare_sequence_document<'a>(
    diagram: &'a AsciiSequenceDiagram,
    title: Option<PreparedSequenceTitle<'a>>,
    body_plan: SequenceDocumentPlan<'_>,
    layout: &SequenceLayout,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<PreparedSequenceDocument<'a>> {
    let transaction = resources.clone();
    transaction.transaction(|_| {
        prepare_sequence_document_transactional(
            diagram,
            title,
            body_plan,
            layout,
            resources,
            checkpoints,
        )
    })
}

fn prepare_sequence_document_transactional<'a>(
    diagram: &'a AsciiSequenceDiagram,
    title: Option<PreparedSequenceTitle<'a>>,
    body_plan: SequenceDocumentPlan<'_>,
    layout: &SequenceLayout,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<PreparedSequenceDocument<'a>> {
    let body_extent = body_plan.extent();
    let boxes = prepare_sequence_boxes(diagram, layout, body_plan, resources, checkpoints)?;
    let content_extent = boxes
        .as_ref()
        .map_or(body_extent, PreparedSequenceBoxes::output_extent);
    let output_extent = match title {
        Some(title) => {
            let title_cells =
                planned_title_retained_width(title, content_extent.width(), resources)?;
            let document_cells = content_extent
                .document_cells()
                .checked_add(title_cells)
                .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
            SequenceDocumentExtent::new(
                content_extent.width().max(title_cells),
                resources.checked_grid_add(content_extent.height(), 1)?,
                document_cells,
            )
        }
        None => content_extent,
    };
    resources.grid_extent(output_extent.width(), output_extent.height())?;
    resources.check(
        AsciiResourceLimitId::MaxDocumentCells,
        output_extent.document_cells(),
    )?;
    Ok(PreparedSequenceDocument {
        title,
        boxes,
        content_extent,
        output_extent,
    })
}

pub(super) fn prepare_sequence_title<'a>(
    title: Option<&'a str>,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<Option<PreparedSequenceTitle<'a>>> {
    let Some(title) = title.filter(|title| !title.is_empty()) else {
        return Ok(None);
    };
    let widths = measure_title_widths(title, width_profile, resources, checkpoints)?;
    Ok(Some(PreparedSequenceTitle {
        text: title,
        alignment_width: widths.alignment,
        retained_width: widths.retained,
        width_profile,
    }))
}

fn finish_sequence_lines(
    lines: Vec<SequenceLine>,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<String> {
    if options.color_mode == AsciiColorMode::Plain {
        let document_resources = resources.scoped();
        let mut output = if resources
            .policy()
            .value(AsciiResourceLimitId::MaxOutputBytes)
            .is_none()
        {
            CheckedOutput::new_unbounded(resources)
        } else {
            CheckedOutput::new(resources)
        };
        if lines.is_empty() {
            checkpoints.before_charge()?;
            document_resources.charge_layout_work(1)?;
            checkpoints.before_charge()?;
            output.push_char('\n')?;
            return Ok(output.finish());
        }
        for line in lines {
            checkpoints.before_charge()?;
            document_resources.charge_document_cells(line.len())?;
            checkpoints.before_charge()?;
            document_resources.charge_layout_work(line.len().max(1))?;
            write_plain_sequence_line(&line, &mut output, checkpoints)?;
            checkpoints.before_charge()?;
            output.push_char('\n')?;
        }
        return Ok(output.finish());
    }

    if lines.is_empty() {
        return Ok(String::new());
    }

    let deferred = crate::safe_text::DeferredTextRegistry::new();
    crate::canvas::finish_styled_line_iter_with_deferred_resources_with_execution(
        lines.iter(),
        options,
        true,
        resources,
        &deferred,
        checkpoints.execution(),
    )
}

fn write_plain_sequence_line(
    line: &SequenceLine,
    output: &mut CheckedOutput,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<()> {
    let mut offset = 0usize;
    let mut surface_checkpoints = SurfaceCellCheckpoints::cadenced(|| checkpoints.before_charge());
    while let Some(cell) = line.surface_cells().get(offset).copied() {
        surface_checkpoints.checkpoint_primary_cell()?;
        if let Some(text) = cell.try_output_text(line.surface_arena())? {
            match text {
                TerminalCellText::Scalar(ch) => output.push_char(ch)?,
                TerminalCellText::Grapheme(grapheme) => output.push_str(grapheme)?,
            }
        }
        let width =
            primary_width_with_checkpoints(line.surface_cells(), offset, &mut surface_checkpoints)?
                .max(1);
        offset = offset.checked_add(width).ok_or_else(allocation_failed)?;
    }
    Ok(())
}

fn prepend_title_line(
    lines: &mut Vec<SequenceLine>,
    title: PreparedSequenceTitle<'_>,
    content_width: usize,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<()> {
    checkpoints.before_charge()?;
    resources.charge_layout_work(title.alignment_width.max(1))?;
    lines.try_reserve(1).map_err(|_| allocation_failed())?;
    lines.insert(
        0,
        render_title_line(
            title.text,
            title.alignment_width,
            title.retained_width,
            content_width,
            title.width_profile,
            resources,
            checkpoints,
        )?,
    );
    Ok(())
}

fn planned_title_retained_width(
    title: PreparedSequenceTitle<'_>,
    content_width: usize,
    resources: &ResourceContext,
) -> Result<usize> {
    if title.retained_width == 0 {
        return Ok(0);
    }
    resources.checked_grid_add(
        content_width.saturating_sub(title.alignment_width) / 2,
        title.retained_width,
    )
}

fn validate_lines_extent(
    lines: &[SequenceLine],
    expected: SequenceDocumentExtent,
    resources: &ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<()> {
    let mut width = 0usize;
    let mut document_cells = 0usize;
    for line in lines {
        checkpoints.tick()?;
        width = width.max(line.len());
        document_cells = document_cells
            .checked_add(line.len())
            .ok_or_else(|| resources.overflow(AsciiResourceLimitId::MaxDocumentCells))?;
    }
    if width != expected.width()
        || lines.len() != expected.height()
        || document_cells != expected.document_cells()
    {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "document extent planning",
        });
    }
    resources.grid_extent(width, lines.len())?;
    Ok(())
}

fn measure_title_widths(
    title: &str,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceTitleWidths> {
    let mut widths = SequenceTitleWidths {
        alignment: 0,
        retained: 0,
    };
    let mut overflowed = false;
    visit_safe_line_graphemes(
        resources,
        title,
        width_profile,
        |grapheme, grapheme_width| {
            checkpoints.tick()?;
            let Some(next_width) = widths.alignment.checked_add(grapheme_width) else {
                overflowed = true;
                return Ok(false);
            };
            widths.alignment = next_width;
            if grapheme != " " {
                widths.retained = next_width;
            }
            Ok(true)
        },
    )?;
    if overflowed {
        return resources.checked_grid_add(usize::MAX, 1).map(|_| widths);
    }
    Ok(widths)
}

fn render_title_line(
    title: &str,
    title_width: usize,
    retained_width: usize,
    width: usize,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceLine> {
    if retained_width == 0 {
        return blank_line_with_checkpoints(0, width_profile, resources, checkpoints);
    }
    let left = width.saturating_sub(title_width) / 2;
    let mut line = blank_line_with_checkpoints(left, width_profile, resources, checkpoints)?;
    let mut written = 0usize;
    visit_safe_line_graphemes(
        &mut resources.clone(),
        title,
        width_profile,
        |grapheme, grapheme_width| {
            checkpoints.tick()?;
            if written >= retained_width {
                return Ok(false);
            }
            line.try_push_role_text_with_checkpoint(
                grapheme,
                AsciiColorRole::Text,
                resources,
                || checkpoints.tick(),
            )?;
            written = resources.checked_grid_add(written, grapheme_width)?;
            Ok(true)
        },
    )?;
    if written != retained_width {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "title extent planning",
        });
    }
    Ok(line)
}

fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::AsciiExecution;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::{OperationControl, OperationPhase};

    #[test]
    fn styled_sequence_rows_stream_through_one_output_budget_in_every_mode() {
        for mode in [
            AsciiColorMode::Plain,
            AsciiColorMode::Ansi16,
            AsciiColorMode::Ansi256,
            AsciiColorMode::TrueColor,
            AsciiColorMode::Html,
        ] {
            let base = AsciiRenderOptions::unicode().with_color_mode(mode);
            let base_policy = AsciiResourcePolicy::default();
            let expected = finish_styled_test_lines(&base, &base_policy)
                .expect("unmodified profile should encode styled sequence rows");

            let exact = AsciiResourcePolicy::default()
                .with_limit(AsciiResourceLimitId::MaxOutputBytes, expected.len())
                .expect("exact output limit should be valid");
            assert_eq!(
                finish_styled_test_lines(&base, &exact).expect("exact output budget should encode"),
                expected
            );

            let below = AsciiResourcePolicy::default()
                .with_limit(
                    AsciiResourceLimitId::MaxOutputBytes,
                    expected.len().saturating_sub(1),
                )
                .expect("limit below encoded output should be valid");
            let error = finish_styled_test_lines(&base, &below)
                .expect_err("aggregate output budget must reject before partial output escapes");
            assert!(matches!(
                error,
                AsciiError::ResourceLimitExceeded(details)
                    if details.limit == AsciiResourceLimitId::MaxOutputBytes
                        && details.actual > details.max
                        && details.max == expected.len() - 1
            ));
        }
    }

    #[test]
    fn plain_emit_cancellation_precedes_the_limit_minus_one_output_write() {
        let options = AsciiRenderOptions::ascii();
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, 64)
            .expect("the output limit should be valid");
        let mut resources = ResourceContext::new(policy);
        let line =
            SequenceLine::plain_text_with_profile(&"x".repeat(128), options.terminal_width_profile);
        let control = OperationControl::new();
        let execution = AsciiExecution::new(&control, &policy);
        resources = execution.resource_context(&resources, OperationPhase::Emit);
        control.cancel_after_checkpoints(70);
        let mut checkpoints = SequenceCheckpointCursor::new(execution, OperationPhase::Emit);

        let error = finish_sequence_lines(vec![line], &options, &mut resources, &mut checkpoints)
            .expect_err("the 65th byte should observe cancellation before exceeding 64 bytes");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Emit
                    && cancelled.reason == merman_core::CancelReason::Requested
        ));
    }

    #[test]
    fn plain_output_limit_replays_before_later_cancellation() {
        let options = AsciiRenderOptions::ascii();
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, 1)
            .expect("the output limit should be valid");
        let line = SequenceLine::plain_text_with_profile("AB", options.terminal_width_profile);
        let control = OperationControl::new();
        let execution = AsciiExecution::new(&control, &policy);
        let base_resources = ResourceContext::new(policy);
        let mut resources = execution.resource_context(&base_resources, OperationPhase::Emit);
        let mut checkpoints = SequenceCheckpointCursor::new(execution, OperationPhase::Emit);

        let first = finish_sequence_lines(vec![line], &options, &mut resources, &mut checkpoints)
            .expect_err("the second byte must exceed the output budget");
        assert!(matches!(
            &first,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxOutputBytes
                    && details.actual == 2
                    && details.max == 1
        ));

        control.cancel();
        assert_eq!(
            execution
                .checkpoint(OperationPhase::Layout)
                .expect_err("the first output terminal must remain sticky"),
            first
        );
    }

    fn finish_styled_test_lines(
        options: &AsciiRenderOptions,
        policy: &AsciiResourcePolicy,
    ) -> Result<String> {
        let mut resources = ResourceContext::new(*policy);
        let execution = AsciiExecution::for_test(policy);
        let mut checkpoints = SequenceCheckpointCursor::new(execution, OperationPhase::Emit);
        finish_sequence_lines(
            styled_test_lines(options, *policy),
            options,
            &mut resources,
            &mut checkpoints,
        )
    }

    fn styled_test_lines(
        options: &AsciiRenderOptions,
        policy: AsciiResourcePolicy,
    ) -> Vec<SequenceLine> {
        let mut first = SequenceLine::with_resource_policy(options.terminal_width_profile, policy);
        first
            .try_push_role_text("A<&", AsciiColorRole::Text)
            .expect("styled line should fit");
        let mut second = SequenceLine::with_resource_policy(options.terminal_width_profile, policy);
        second
            .try_push_role_text("B👩🏽‍💻", AsciiColorRole::EdgeArrow)
            .expect("styled line should fit");
        vec![first, second]
    }
}
