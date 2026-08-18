use super::SequenceCheckpointCursor;
use super::boxes::render_sequence_boxes;
use super::chars::SequenceChars;
use super::layout::SequenceLayout;
use super::model::AsciiSequenceDiagram;
use super::text::{SequenceLine, blank_line_with_checkpoints, trim_right};
use crate::color::{AsciiColorMode, AsciiColorRole};
use crate::error::{AsciiError, Result};
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
use crate::resource::{AsciiResourceLimitPhase, CheckedOutput, ResourceContext};
use crate::safe_text::visit_safe_line_graphemes;
use crate::terminal::{SurfaceCellCheckpoints, TerminalCellText, primary_width_with_checkpoints};
use merman_core::OperationPhase;

#[derive(Debug)]
pub(super) struct SequenceRowDocument {
    lines: Vec<SequenceLine>,
}

impl SequenceRowDocument {
    pub(super) fn new(lines: Vec<SequenceLine>) -> Self {
        Self { lines }
    }

    pub(super) fn render(
        self,
        diagram: &AsciiSequenceDiagram,
        layout: &SequenceLayout,
        chars: &SequenceChars,
        options: &AsciiRenderOptions,
        resources: &mut ResourceContext,
        layout_checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<String> {
        let mut lines = self.lines;
        if !diagram.boxes.is_empty() {
            lines = render_sequence_boxes(
                lines,
                diagram,
                layout,
                chars,
                resources,
                layout_checkpoints,
            )?;
        }
        if let Some(title) = diagram.title.as_deref() {
            prepend_title_line(&mut lines, title, resources, layout_checkpoints)?;
        }
        // Box/title geometry, extent admission, and canvas construction are layout work. Only
        // after those complete do we bind the shared ledger to Emit for byte/document emission.
        let mut emit_resources = layout_checkpoints
            .execution()
            .resource_context(resources, OperationPhase::Emit);
        let mut emit_checkpoints = layout_checkpoints.next_phase(OperationPhase::Emit);
        finish_sequence_lines(lines, options, &mut emit_resources, &mut emit_checkpoints)
    }
}

fn finish_sequence_lines(
    lines: Vec<SequenceLine>,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<String> {
    if options.color_mode == AsciiColorMode::Plain {
        let document_resources = resources.scoped();
        let mut output = CheckedOutput::new(resources);
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
        surface_checkpoints.force()?;
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
    title: &str,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<()> {
    let mut width = 0usize;
    for line in lines.iter() {
        checkpoints.tick()?;
        width = width.max(line.len());
    }
    let width_profile = lines
        .first()
        .map(SequenceLine::width_profile)
        .unwrap_or(TerminalWidthProfile::Unicode);
    let title_width = measure_title_width(title, width_profile, resources, checkpoints)?;
    let height = resources.checked_grid_add(lines.len(), 1)?;
    resources.grid_extent(width.max(title_width), height)?;
    checkpoints.before_charge()?;
    resources.charge_layout_work(title_width.max(1))?;
    lines.try_reserve(1).map_err(|_| allocation_failed())?;
    lines.insert(
        0,
        render_title_line(
            title,
            title_width,
            width,
            width_profile,
            resources,
            checkpoints,
        )?,
    );
    Ok(())
}

fn measure_title_width(
    title: &str,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<usize> {
    let mut width = 0usize;
    let mut overflowed = false;
    visit_safe_line_graphemes(resources, title, width_profile, |_, grapheme_width| {
        checkpoints.tick()?;
        let Some(next_width) = width.checked_add(grapheme_width) else {
            overflowed = true;
            return Ok(false);
        };
        width = next_width;
        Ok(true)
    })?;
    if overflowed {
        return resources.checked_grid_add(usize::MAX, 1);
    }
    Ok(width)
}

fn render_title_line(
    title: &str,
    title_width: usize,
    width: usize,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceLine> {
    let left = width.saturating_sub(title_width) / 2;
    let mut line = blank_line_with_checkpoints(left, width_profile, resources, checkpoints)?;
    line.try_push_role_text_with_checkpoint(title, AsciiColorRole::Text, resources, || {
        checkpoints.tick()
    })?;
    trim_right(line)
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
        control.cancel_after_checkpoints(66);
        let execution = AsciiExecution::new(&control, &policy);
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
