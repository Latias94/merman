use super::{
    BOX_BORDER_WIDTH, SEQUENCE_BOX_CONTENT_OFFSET, SEQUENCE_BOX_LABEL_MARGIN,
    SEQUENCE_BOX_WRAP_TEXT_WIDTH,
};
use crate::color::{AsciiColorRole, AsciiRgb};
use crate::error::{AsciiError, Result};
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};
use crate::text::{
    display_width_with_profile, split_label_lines, truncate_display_width_with_profile,
    wrap_label_lines_with_profile,
};

use super::layout::SequenceLayout;
use super::model::{AsciiSequenceDiagram, SequenceGroupBox};
use super::render::SequenceChars;
use super::text::{
    SequenceBatchExtent, SequenceExtentLedger, SequenceLine, blank_line, charge_text_work,
    trim_right,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SequenceGroupBoxBounds {
    left: usize,
    right: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreparedSequenceGroupBox {
    bounds: SequenceGroupBoxBounds,
    label_lines: Vec<String>,
    background: Option<AsciiRgb>,
}

pub(super) fn render_sequence_boxes(
    lines: Vec<SequenceLine>,
    diagram: &AsciiSequenceDiagram,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    resources: &mut ResourceContext,
) -> Result<Vec<SequenceLine>> {
    let horizontal_padding = resources.checked_grid_mul(2, SEQUENCE_BOX_CONTENT_OFFSET)?;
    let mut content_width = 0;
    for line in &lines {
        content_width =
            content_width.max(resources.checked_grid_add(line.len(), horizontal_padding)?);
    }

    resources.charge_layout_work(diagram.boxes.len())?;
    resources.grid_extent(diagram.boxes.len(), 1)?;
    let mut boxes = Vec::new();
    boxes
        .try_reserve_exact(diagram.boxes.len())
        .map_err(|_| allocation_failed())?;
    for sequence_box in &diagram.boxes {
        boxes.push(prepare_sequence_box(
            sequence_box,
            layout,
            content_width,
            resources,
        )?);
    }
    let label_extra_rows = boxes
        .iter()
        .map(|sequence_box| sequence_box.label_lines.len().saturating_sub(1))
        .max()
        .unwrap_or(0);
    let mut box_width = 0;
    for sequence_box in &boxes {
        box_width = box_width.max(resources.checked_grid_add(sequence_box.bounds.right, 1)?);
    }
    let width = content_width.max(box_width);
    let height = resources.checked_grid_add(
        resources.checked_grid_add(lines.len(), label_extra_rows)?,
        2,
    )?;
    resources.grid_extent(width, height)?;
    charge_work_product(resources, width, height)?;
    let output_batch =
        planned_box_output_extent(&lines, label_extra_rows, box_width, width, resources)?;
    let mut output_extent = SequenceExtentLedger::default();
    let output_reservation = output_extent.reserve(output_batch, resources)?;

    let mut canvas = Vec::new();
    canvas
        .try_reserve_exact(height)
        .map_err(|_| allocation_failed())?;
    canvas.push(blank_line(width, layout.width_profile, resources)?);
    for _ in 0..label_extra_rows {
        canvas.push(blank_line(width, layout.width_profile, resources)?);
    }
    for line in lines {
        let mut row = blank_line(0, layout.width_profile, resources)?;
        row.try_push_spaces(SEQUENCE_BOX_CONTENT_OFFSET)?;
        row.try_push_line(&line)?;
        row.try_push_spaces(SEQUENCE_BOX_CONTENT_OFFSET)?;
        row.try_pad_to(width)?;
        canvas.push(row);
    }
    canvas.push(blank_line(width, layout.width_profile, resources)?);

    for sequence_box in boxes {
        draw_sequence_box(&mut canvas, sequence_box, chars, resources)?;
    }

    let mut rendered = Vec::new();
    rendered
        .try_reserve_exact(canvas.len())
        .map_err(|_| allocation_failed())?;
    for row in canvas {
        rendered.push(trim_right(row)?);
    }
    output_reservation.commit(&mut output_extent, &rendered, resources)?;
    Ok(rendered)
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
            .checked_grid_add(SEQUENCE_BOX_CONTENT_OFFSET, line.len())
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

fn prepare_sequence_box(
    sequence_box: &SequenceGroupBox,
    layout: &SequenceLayout,
    content_width: usize,
    resources: &mut ResourceContext,
) -> Result<PreparedSequenceGroupBox> {
    let mut bounds = sequence_box_bounds(sequence_box, layout, content_width, resources)?;
    let label_margin = resources.checked_grid_mul(2, SEQUENCE_BOX_LABEL_MARGIN)?;
    let label_left = resources.checked_grid_add(bounds.left, label_margin)?;
    let label_width = bounds.right.saturating_sub(label_left).max(1);
    let label_lines =
        sequence_box_label_lines(sequence_box, label_width, layout.width_profile, resources)?;

    if let Some(max_label_width) = label_lines
        .iter()
        .map(|line| display_width_with_profile(line, layout.width_profile))
        .max()
    {
        let label_right = resources.checked_grid_add(
            resources.checked_grid_add(bounds.left, max_label_width)?,
            label_margin,
        )?;
        bounds.right = bounds.right.max(label_right);
    }

    Ok(PreparedSequenceGroupBox {
        bounds,
        label_lines,
        background: sequence_box.background,
    })
}

fn sequence_box_bounds(
    sequence_box: &SequenceGroupBox,
    layout: &SequenceLayout,
    content_width: usize,
    resources: &ResourceContext,
) -> Result<SequenceGroupBoxBounds> {
    if sequence_box.actor_indices.is_empty() {
        return sequence_box_full_width_bounds(
            content_width,
            sequence_box,
            layout.width_profile,
            resources,
        );
    }

    sequence_box_actor_bounds(sequence_box, layout, content_width, resources)
}

fn sequence_box_full_width_bounds(
    content_width: usize,
    sequence_box: &SequenceGroupBox,
    width_profile: crate::options::TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<SequenceGroupBoxBounds> {
    let label_width = if let Some(label) = sequence_box.label.as_ref() {
        resources.checked_grid_add(
            display_width_with_profile(label, width_profile),
            resources.checked_grid_mul(2, SEQUENCE_BOX_LABEL_MARGIN)?,
        )?
    } else {
        0
    };
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
) -> Result<SequenceGroupBoxBounds> {
    let mut left = usize::MAX;
    let mut right = 0;

    for actor_index in &sequence_box.actor_indices {
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

fn sequence_box_label_lines(
    sequence_box: &SequenceGroupBox,
    label_width: usize,
    width_profile: crate::options::TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<Vec<String>> {
    let Some(label) = &sequence_box.label else {
        return Ok(Vec::new());
    };
    charge_text_work(label, width_profile, resources)?;

    if sequence_box.wrap {
        Ok(wrap_label_lines_with_profile(
            label,
            label_width.max(SEQUENCE_BOX_WRAP_TEXT_WIDTH),
            width_profile,
        ))
    } else {
        Ok(split_label_lines(label))
    }
}

fn draw_sequence_box(
    canvas: &mut [SequenceLine],
    sequence_box: PreparedSequenceGroupBox,
    chars: &SequenceChars,
    resources: &mut ResourceContext,
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
    charge_work_product(resources, span_width, canvas.len())?;

    paint_sequence_box_background(canvas, bounds, sequence_box.background);

    for x in bounds.left..=bounds.right {
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
        draw_background_vertical(row, bounds.left, chars.vertical)?;
        draw_background_vertical(row, bounds.right, chars.vertical)?;
    }

    for (line_index, line) in sequence_box.label_lines.iter().enumerate() {
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
) {
    let Some(background) = background else {
        return;
    };

    for row in canvas {
        for x in bounds.left..=bounds.right {
            row.set_background_color_if_unset(x, background);
        }
    }
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
    use super::*;
    use crate::options::TerminalWidthProfile;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};

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
            events: Vec::new(),
        };
        let layout = SequenceLayout {
            participant_widths: Vec::new(),
            participant_centers: Vec::new(),
            total_width: 0,
            message_spacing: 1,
            self_message_width: 1,
            width_profile: TerminalWidthProfile::Unicode,
        };
        let lines = vec![blank_line(4, layout.width_profile, &resources)?];

        render_sequence_boxes(lines, &diagram, &layout, &ascii_chars(), &mut resources)
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
