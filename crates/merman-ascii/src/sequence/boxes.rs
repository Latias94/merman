use super::{
    BOX_BORDER_WIDTH, SEQUENCE_BOX_CONTENT_OFFSET, SEQUENCE_BOX_LABEL_MARGIN,
    SEQUENCE_BOX_WRAP_TEXT_WIDTH, SequenceCheckpointCursor, try_plan_sequence_label,
};
use crate::color::{AsciiColorRole, AsciiRgb};
use crate::error::{AsciiError, Result};
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};
use crate::safe_text::{LabelBreakPolicy, NormalizedLabelPlan};
use crate::text::{display_width_with_profile, truncate_display_width_with_profile};

use super::layout::SequenceLayout;
use super::model::{AsciiSequenceDiagram, SequenceGroupBox};
use super::render::SequenceChars;
use super::text::{
    SequenceBatchExtent, SequenceExtentLedger, SequenceLine, blank_line, trim_right,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SequenceGroupBoxBounds {
    left: usize,
    right: usize,
}

#[derive(Debug)]
struct PreparedSequenceGroupBox<'a> {
    bounds: SequenceGroupBoxBounds,
    label: Option<&'a str>,
    label_plan: Option<NormalizedLabelPlan>,
    background: Option<AsciiRgb>,
}

impl PreparedSequenceGroupBox<'_> {
    fn label_line_count(&self) -> usize {
        self.label_plan
            .map(NormalizedLabelPlan::metrics)
            .map_or(0, |metrics| metrics.line_count)
    }

    #[cfg(test)]
    fn materialize_label_with_probe(
        &self,
        resources: &ResourceContext,
        materialized: &std::cell::Cell<bool>,
    ) -> Result<()> {
        match (self.label, self.label_plan) {
            (Some(label), Some(plan)) => {
                plan.materialize_with_probe(label, resources, materialized)?;
                Ok(())
            }
            (None, None) => Ok(()),
            _ => Err(invalid_box_geometry()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SequenceBoxCanvasPlan {
    label_extra_rows: usize,
    width: usize,
    height: usize,
    output_batch: SequenceBatchExtent,
}

pub(super) fn render_sequence_boxes(
    lines: Vec<SequenceLine>,
    diagram: &AsciiSequenceDiagram,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<Vec<SequenceLine>> {
    let horizontal_padding = resources.checked_grid_mul(2, SEQUENCE_BOX_CONTENT_OFFSET)?;
    let mut content_width = 0;
    for line in &lines {
        checkpoints.tick()?;
        content_width =
            content_width.max(resources.checked_grid_add(line.len(), horizontal_padding)?);
    }

    checkpoints.before_charge()?;
    resources.charge_layout_work(diagram.boxes.len())?;
    resources.grid_extent(diagram.boxes.len(), 1)?;
    let mut boxes = Vec::new();
    boxes
        .try_reserve_exact(diagram.boxes.len())
        .map_err(|_| allocation_failed())?;
    for sequence_box in &diagram.boxes {
        checkpoints.tick()?;
        boxes.push(prepare_sequence_box(
            sequence_box,
            layout,
            content_width,
            resources,
            checkpoints,
        )?);
    }
    let canvas_plan =
        plan_sequence_box_canvas(&lines, &boxes, content_width, resources, checkpoints)?;
    let mut output_extent = SequenceExtentLedger::default();
    let output_reservation =
        output_extent.reserve(canvas_plan.output_batch, resources, checkpoints)?;

    let mut canvas = Vec::new();
    canvas
        .try_reserve_exact(canvas_plan.height)
        .map_err(|_| allocation_failed())?;
    canvas.push(blank_line(
        canvas_plan.width,
        layout.width_profile,
        resources,
    )?);
    for _ in 0..canvas_plan.label_extra_rows {
        checkpoints.tick()?;
        canvas.push(blank_line(
            canvas_plan.width,
            layout.width_profile,
            resources,
        )?);
    }
    for line in lines {
        checkpoints.tick()?;
        let mut row = blank_line(0, layout.width_profile, resources)?;
        row.try_push_spaces(SEQUENCE_BOX_CONTENT_OFFSET)?;
        row.try_push_line(&line)?;
        row.try_push_spaces(SEQUENCE_BOX_CONTENT_OFFSET)?;
        row.try_pad_to(canvas_plan.width)?;
        canvas.push(row);
    }
    canvas.push(blank_line(
        canvas_plan.width,
        layout.width_profile,
        resources,
    )?);

    for sequence_box in boxes {
        checkpoints.tick()?;
        draw_sequence_box(&mut canvas, sequence_box, chars, resources, checkpoints)?;
    }

    let mut rendered = Vec::new();
    rendered
        .try_reserve_exact(canvas.len())
        .map_err(|_| allocation_failed())?;
    for row in canvas {
        checkpoints.tick()?;
        rendered.push(trim_right(row)?);
    }
    output_reservation.commit(&mut output_extent, &rendered, resources)?;
    Ok(rendered)
}

fn plan_sequence_box_canvas(
    lines: &[SequenceLine],
    boxes: &[PreparedSequenceGroupBox<'_>],
    content_width: usize,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceBoxCanvasPlan> {
    let mut label_extra_rows = 0usize;
    let mut box_width = 0;
    for sequence_box in boxes {
        checkpoints.tick()?;
        label_extra_rows = label_extra_rows.max(sequence_box.label_line_count().saturating_sub(1));
        box_width = box_width.max(resources.checked_grid_add(sequence_box.bounds.right, 1)?);
    }
    let width = content_width.max(box_width);
    let height = resources.checked_grid_add(
        resources.checked_grid_add(lines.len(), label_extra_rows)?,
        2,
    )?;
    resources.grid_extent(width, height)?;
    checkpoints.before_charge()?;
    charge_work_product(resources, width, height)?;
    let output_batch =
        planned_box_output_extent(lines, label_extra_rows, box_width, width, resources)?;
    Ok(SequenceBoxCanvasPlan {
        label_extra_rows,
        width,
        height,
        output_batch,
    })
}

fn planned_box_output_extent(
    lines: &[SequenceLine],
    label_extra_rows: usize,
    box_width: usize,
    materialized_width: usize,
    resources: &ResourceContext,
) -> Result<SequenceBatchExtent> {
    let top_row_count = resources.checked_grid_add(label_extra_rows, 1)?;
    let top_and_label_rows = std::iter::repeat_n(box_width, top_row_count);
    let content_rows = lines.iter().map(|line| {
        resources
            .checked_grid_add(SEQUENCE_BOX_CONTENT_OFFSET, line.trimmed_len(false))
            .map(|content_width| content_width.max(box_width))
    });
    SequenceBatchExtent::try_from_line_lengths(
        materialized_width,
        top_and_label_rows
            .map(Ok)
            .chain(content_rows)
            .chain(std::iter::once(Ok(box_width))),
        resources,
    )
}

fn prepare_sequence_box<'a>(
    sequence_box: &'a SequenceGroupBox,
    layout: &SequenceLayout,
    content_width: usize,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<PreparedSequenceGroupBox<'a>> {
    let full_width_label_width = if sequence_box.actor_indices.is_empty() {
        match sequence_box.label.as_deref() {
            Some(label) => {
                checkpoints.before_charge()?;
                resources.charge_layout_work(label.len().max(1))?;
                display_width_with_profile(label, layout.width_profile)
            }
            None => 0,
        }
    } else {
        0
    };
    let mut bounds = sequence_box_bounds(
        sequence_box,
        layout,
        content_width,
        full_width_label_width,
        resources,
        checkpoints,
    )?;
    let label_margin = resources.checked_grid_mul(2, SEQUENCE_BOX_LABEL_MARGIN)?;
    let label_left = resources.checked_grid_add(bounds.left, label_margin)?;
    let label_width = bounds.right.saturating_sub(label_left).max(1);
    let wrap_width = sequence_box
        .wrap
        .then_some(label_width.max(SEQUENCE_BOX_WRAP_TEXT_WIDTH));
    let label_plan =
        sequence_box_label_plan(sequence_box, wrap_width, layout, resources, checkpoints)?;
    if let Some(plan) = label_plan {
        checkpoints.before_charge()?;
        plan.check_materialization_limits(resources)?;
    }

    if let Some(max_label_width) = label_plan
        .map(NormalizedLabelPlan::metrics)
        .map(|m| m.max_width)
    {
        let label_right = resources.checked_grid_add(
            resources.checked_grid_add(bounds.left, max_label_width)?,
            label_margin,
        )?;
        bounds.right = bounds.right.max(label_right);
    }

    Ok(PreparedSequenceGroupBox {
        bounds,
        label: sequence_box.label.as_deref(),
        label_plan,
        background: sequence_box.background,
    })
}

fn sequence_box_bounds(
    sequence_box: &SequenceGroupBox,
    layout: &SequenceLayout,
    content_width: usize,
    label_width: usize,
    resources: &ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceGroupBoxBounds> {
    if sequence_box.actor_indices.is_empty() {
        return sequence_box_full_width_bounds(content_width, label_width, resources);
    }

    sequence_box_actor_bounds(sequence_box, layout, content_width, resources, checkpoints)
}

fn sequence_box_full_width_bounds(
    content_width: usize,
    label_width: usize,
    resources: &ResourceContext,
) -> Result<SequenceGroupBoxBounds> {
    let label_width = resources.checked_grid_add(
        label_width,
        resources.checked_grid_mul(2, SEQUENCE_BOX_LABEL_MARGIN)?,
    )?;
    let minimum_width =
        resources.checked_grid_add(resources.checked_grid_mul(SEQUENCE_BOX_LABEL_MARGIN, 2)?, 1)?;
    let right = content_width.max(label_width).max(minimum_width);

    Ok(SequenceGroupBoxBounds { left: 0, right })
}

fn sequence_box_actor_bounds(
    sequence_box: &SequenceGroupBox,
    layout: &SequenceLayout,
    content_width: usize,
    resources: &ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceGroupBoxBounds> {
    let mut left = usize::MAX;
    let mut right = 0;

    for actor_index in &sequence_box.actor_indices {
        checkpoints.tick()?;
        let participant_width = layout
            .participant_widths
            .get(*actor_index)
            .copied()
            .ok_or_else(invalid_box_actor)?;
        let participant_center = layout
            .participant_centers
            .get(*actor_index)
            .copied()
            .ok_or_else(invalid_box_actor)?;
        let box_width = resources.checked_grid_add(participant_width, BOX_BORDER_WIDTH)?;
        let participant_left = participant_center
            .checked_sub(box_width / 2)
            .ok_or_else(invalid_box_actor)?;
        let participant_right = resources
            .checked_grid_add(participant_left, box_width)?
            .checked_sub(1)
            .ok_or_else(invalid_box_actor)?;
        left = left.min(participant_left);
        right = right.max(resources.checked_grid_add(
            resources.checked_grid_add(participant_right, SEQUENCE_BOX_CONTENT_OFFSET)?,
            1,
        )?);
    }

    if sequence_box.actor_indices.len() == layout.participant_widths.len() {
        right = right.max(content_width);
    }

    Ok(SequenceGroupBoxBounds { left, right })
}

fn sequence_box_label_plan(
    sequence_box: &SequenceGroupBox,
    wrap_width: Option<usize>,
    layout: &SequenceLayout,
    resources: &ResourceContext,
    checkpoints: &SequenceCheckpointCursor<'_>,
) -> Result<Option<NormalizedLabelPlan>> {
    let Some(label) = &sequence_box.label else {
        return Ok(None);
    };
    try_plan_sequence_label(
        label,
        layout.width_profile,
        false,
        wrap_width,
        LabelBreakPolicy::MermaidLabelBreaks,
        resources,
        checkpoints,
    )
}

fn draw_sequence_box(
    canvas: &mut [SequenceLine],
    sequence_box: PreparedSequenceGroupBox<'_>,
    chars: &SequenceChars,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<()> {
    let bounds = sequence_box.bounds;
    if canvas.is_empty() || bounds.left >= bounds.right {
        return Ok(());
    }

    let top = 0;
    let bottom = canvas
        .len()
        .checked_sub(1)
        .ok_or_else(invalid_box_geometry)?;
    let span = bounds
        .right
        .checked_sub(bounds.left)
        .ok_or_else(invalid_box_geometry)?;
    let span_width = resources.checked_grid_add(span, 1)?;
    checkpoints.before_charge()?;
    charge_work_product(resources, span_width, canvas.len())?;

    paint_sequence_box_background(canvas, bounds, sequence_box.background, checkpoints)?;

    for x in bounds.left..=bounds.right {
        checkpoints.tick()?;
        canvas[top].try_set_role(x, chars.horizontal, AsciiColorRole::SequenceFrame)?;
        canvas[bottom].try_set_role(x, chars.horizontal, AsciiColorRole::SequenceFrame)?;
    }
    canvas[top].try_set_role(bounds.left, chars.top_left, AsciiColorRole::SequenceFrame)?;
    canvas[top].try_set_role(bounds.right, chars.top_right, AsciiColorRole::SequenceFrame)?;
    canvas[bottom].try_set_role(
        bounds.left,
        chars.bottom_left,
        AsciiColorRole::SequenceFrame,
    )?;
    canvas[bottom].try_set_role(
        bounds.right,
        chars.bottom_right,
        AsciiColorRole::SequenceFrame,
    )?;

    for row in canvas.iter_mut().take(bottom).skip(1) {
        checkpoints.tick()?;
        draw_background_vertical(row, bounds.left, chars.vertical)?;
        draw_background_vertical(row, bounds.right, chars.vertical)?;
    }

    checkpoints.before_charge()?;
    let materialized = match (sequence_box.label, sequence_box.label_plan) {
        (Some(label), Some(plan)) => plan.materialize(label, resources).map(Some),
        (None, None) => Ok(None),
        _ => return Err(invalid_box_geometry()),
    };
    checkpoints.checkpoint()?;
    let label_lines = match materialized? {
        Some(lines) => lines.into_parts().0,
        None => Vec::new(),
    };
    for (line_index, line) in label_lines.iter().enumerate() {
        checkpoints.tick()?;
        let Some(row) = canvas.get_mut(line_index) else {
            break;
        };
        draw_sequence_box_label(row, line, bounds, resources)?;
    }
    Ok(())
}

fn paint_sequence_box_background(
    canvas: &mut [SequenceLine],
    bounds: SequenceGroupBoxBounds,
    background: Option<AsciiRgb>,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<()> {
    let Some(background) = background else {
        return Ok(());
    };

    for row in canvas {
        for x in bounds.left..=bounds.right {
            checkpoints.tick()?;
            row.set_background_color_if_unset(x, background);
        }
    }
    Ok(())
}

fn draw_sequence_box_label(
    row: &mut SequenceLine,
    label: &str,
    bounds: SequenceGroupBoxBounds,
    resources: &ResourceContext,
) -> Result<()> {
    let label = padded_box_label(label, resources)?;
    let index = resources.checked_grid_add(bounds.left, SEQUENCE_BOX_LABEL_MARGIN)?;
    let available = bounds.right.saturating_sub(index);
    let label = truncate_display_width_with_profile(&label, available, row.width_profile());
    row.try_write_text_role(index, &label, AsciiColorRole::Text)
}

fn padded_box_label(label: &str, resources: &ResourceContext) -> Result<String> {
    let capacity = label
        .len()
        .checked_add(2)
        .ok_or_else(|| work_overflow(resources))?;
    let mut padded = String::new();
    padded
        .try_reserve_exact(capacity)
        .map_err(|_| allocation_failed())?;
    padded.push(' ');
    padded.push_str(label);
    padded.push(' ');
    Ok(padded)
}

fn draw_background_vertical(row: &mut SequenceLine, index: usize, vertical: char) -> Result<()> {
    // Mermaid boxes are background regions; do not corrupt foreground labels or frames.
    if row.get(index) == Some(' ') {
        row.try_set_role(index, vertical, AsciiColorRole::SequenceFrame)?;
    }
    Ok(())
}

fn charge_work_product(resources: &mut ResourceContext, left: usize, right: usize) -> Result<()> {
    resources.charge_layout_work_product(left, right)
}

fn work_overflow(resources: &ResourceContext) -> AsciiError {
    resources.work_overflow()
}

fn invalid_box_actor() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "sequence",
        feature: "boxes with unknown actors",
    }
}

fn invalid_box_geometry() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "sequence",
        feature: "box geometry",
    }
}

fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::operation::AsciiExecution;
    use crate::options::TerminalWidthProfile;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_core::OperationPhase;

    #[test]
    fn box_output_admits_extent_before_canvas_materialization() {
        for limit in [
            AsciiResourceLimitId::MaxGridCells,
            AsciiResourceLimitId::MaxDocumentCells,
        ] {
            let rendered = render_full_width_box_with_limit(limit, 27)
                .expect("the exact box output extent should be admitted");
            assert_eq!(rendered.len(), 3);

            let error = render_full_width_box_with_limit(limit, 26)
                .expect_err("the box output extent should exceed the limit");
            assert!(matches!(
                error,
                AsciiError::ResourceLimitExceeded(details)
                    if details.limit == limit && details.actual == 27 && details.max == 26
            ));
        }
    }

    #[test]
    fn box_grid_admission_precedes_label_materialization() {
        let exact_materialized = Cell::new(false);
        prepare_and_materialize_box_with_grid_limit(60, &exact_materialized)
            .expect("the exact 15x4 labeled box extent should be admitted");
        assert!(exact_materialized.get());

        let below_materialized = Cell::new(false);
        let error = prepare_and_materialize_box_with_grid_limit(59, &below_materialized)
            .expect_err("the labeled box extent should exceed the limit by one cell");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGridCells
                    && details.actual == 60
                    && details.max == 59
        ));
        assert!(!below_materialized.get());
    }

    #[test]
    fn box_output_extent_uses_trimmed_content_width() {
        let mut resources = ResourceContext::new(AsciiResourcePolicy::default());
        let mut input_row = blank_line(8, TerminalWidthProfile::Unicode, &resources)
            .expect("the padded input row should fit");
        input_row
            .try_set_role(3, 'x', AsciiColorRole::Text)
            .expect("the retained content should fit");
        let input = vec![input_row];
        let batch = planned_box_output_extent(&input, 0, 3, 8, &resources)
            .expect("the trimmed output extent should be planned");
        let mut extent = SequenceExtentLedger::default();
        let policy = resources.policy();
        let checkpoints = layout_checkpoints(&policy);
        let reservation = extent
            .reserve(batch, &mut resources, &checkpoints)
            .expect("the padded output extent should be admitted");
        let rendered = vec![
            blank_line(3, TerminalWidthProfile::Unicode, &resources)
                .expect("the top box row should fit"),
            blank_line(6, TerminalWidthProfile::Unicode, &resources)
                .expect("the trimmed content row should fit"),
            blank_line(3, TerminalWidthProfile::Unicode, &resources)
                .expect("the bottom box row should fit"),
        ];

        reservation
            .commit(&mut extent, &rendered, &resources)
            .expect("the descriptor should ignore trimmable materialized padding");
    }

    fn prepare_and_materialize_box_with_grid_limit(
        maximum: usize,
        materialized: &Cell<bool>,
    ) -> Result<()> {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, maximum)
            .expect("the box grid limit override should be valid");
        let mut resources = ResourceContext::new(policy);
        let layout = empty_layout();
        let sequence_box = SequenceGroupBox {
            actor_indices: Vec::new(),
            label: Some("one<br>two".to_string()),
            background: None,
            wrap: false,
        };
        let lines = vec![blank_line(4, layout.width_profile, &resources)?];
        let content_width = 8;
        let mut checkpoints = layout_checkpoints(&policy);
        let prepared = prepare_sequence_box(
            &sequence_box,
            &layout,
            content_width,
            &mut resources,
            &mut checkpoints,
        )?;
        let boxes = vec![prepared];
        let canvas_plan = plan_sequence_box_canvas(
            &lines,
            &boxes,
            content_width,
            &mut resources,
            &mut checkpoints,
        )?;
        assert_eq!(canvas_plan.width, 15);
        assert_eq!(canvas_plan.height, 4);
        boxes[0].materialize_label_with_probe(&resources, materialized)
    }

    fn render_full_width_box_with_limit(
        limit: AsciiResourceLimitId,
        maximum: usize,
    ) -> Result<Vec<SequenceLine>> {
        let policy = AsciiResourcePolicy::default()
            .with_limit(limit, maximum)
            .expect("the box output limit override should be valid");
        let mut resources = ResourceContext::new(policy);
        let diagram = AsciiSequenceDiagram {
            title: None,
            participants: Vec::new(),
            lifecycles: Vec::new(),
            boxes: vec![SequenceGroupBox {
                actor_indices: Vec::new(),
                label: None,
                background: None,
                wrap: false,
            }],
            body: crate::sequence::tree::SequenceBody::default(),
        };
        let layout = empty_layout();
        let lines = vec![blank_line(4, layout.width_profile, &resources)?];
        let mut checkpoints = layout_checkpoints(&policy);

        render_sequence_boxes(
            lines,
            &diagram,
            &layout,
            &ascii_chars(),
            &mut resources,
            &mut checkpoints,
        )
    }

    fn layout_checkpoints(policy: &AsciiResourcePolicy) -> SequenceCheckpointCursor<'_> {
        SequenceCheckpointCursor::new(AsciiExecution::standalone(policy), OperationPhase::Layout)
    }

    fn empty_layout() -> SequenceLayout {
        SequenceLayout {
            participant_widths: Vec::new(),
            participant_centers: Vec::new(),
            total_width: 0,
            message_spacing: 1,
            self_message_width: 1,
            width_profile: TerminalWidthProfile::Unicode,
        }
    }

    fn ascii_chars() -> SequenceChars {
        SequenceChars {
            top_left: '+',
            top_right: '+',
            bottom_left: '+',
            bottom_right: '+',
            horizontal: '-',
            vertical: '|',
            active_vertical: '#',
            destroyed_mark: 'x',
            tee_down: '+',
            tee_up: '+',
            tee_right: '+',
            tee_left: '+',
            filled_arrow_right: '>',
            filled_arrow_left: '<',
            solid_line: '-',
            dotted_line: '.',
            self_top_right: '+',
            self_bottom: '+',
            unicode_markers: false,
        }
    }
}
