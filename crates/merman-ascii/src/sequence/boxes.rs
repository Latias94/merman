use super::{
    BOX_BORDER_WIDTH, SEQUENCE_BOX_CONTENT_OFFSET, SEQUENCE_BOX_LABEL_MARGIN,
    SEQUENCE_BOX_WRAP_TEXT_WIDTH,
};
use crate::color::{AsciiColorRole, AsciiRgb};
use crate::text::{
    display_width_with_profile, split_label_lines, truncate_display_width_with_profile,
    wrap_label_lines_with_profile,
};

use super::layout::SequenceLayout;
use super::model::{AsciiSequenceDiagram, SequenceGroupBox};
use super::render::SequenceChars;
use super::text::{SequenceLine, trim_right};

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
) -> Vec<SequenceLine> {
    let content_width = lines
        .iter()
        .map(|line| line.len() + 2 * SEQUENCE_BOX_CONTENT_OFFSET)
        .max()
        .unwrap_or(0);
    let boxes = diagram
        .boxes
        .iter()
        .map(|sequence_box| prepare_sequence_box(sequence_box, layout, content_width))
        .collect::<Vec<_>>();
    let label_extra_rows = boxes
        .iter()
        .map(|sequence_box| sequence_box.label_lines.len().saturating_sub(1))
        .max()
        .unwrap_or(0);
    let box_width = boxes
        .iter()
        .map(|sequence_box| sequence_box.bounds.right + 1)
        .max()
        .unwrap_or(0);
    let width = content_width.max(box_width);

    let mut canvas = Vec::with_capacity(lines.len() + label_extra_rows + 2);
    canvas.push(SequenceLine::blank_with_profile(
        width,
        layout.width_profile,
    ));
    for _ in 0..label_extra_rows {
        canvas.push(SequenceLine::blank_with_profile(
            width,
            layout.width_profile,
        ));
    }
    for line in lines {
        let mut row = SequenceLine::blank_with_profile(0, layout.width_profile);
        row.push_spaces(SEQUENCE_BOX_CONTENT_OFFSET);
        row.push_line(&line);
        row.push_spaces(SEQUENCE_BOX_CONTENT_OFFSET);
        row.pad_to(width);
        canvas.push(row);
    }
    canvas.push(SequenceLine::blank_with_profile(
        width,
        layout.width_profile,
    ));

    for sequence_box in boxes {
        draw_sequence_box(&mut canvas, sequence_box, chars);
    }

    canvas.into_iter().map(trim_right).collect()
}

fn prepare_sequence_box(
    sequence_box: &SequenceGroupBox,
    layout: &SequenceLayout,
    content_width: usize,
) -> PreparedSequenceGroupBox {
    let mut bounds = sequence_box_bounds(sequence_box, layout, content_width);
    let label_width = bounds
        .right
        .saturating_sub(bounds.left + 2 * SEQUENCE_BOX_LABEL_MARGIN)
        .max(1);
    let label_lines = sequence_box_label_lines(sequence_box, label_width, layout.width_profile);

    if let Some(max_label_width) = label_lines
        .iter()
        .map(|line| display_width_with_profile(line, layout.width_profile))
        .max()
    {
        let label_right = bounds.left + max_label_width + 2 * SEQUENCE_BOX_LABEL_MARGIN;
        bounds.right = bounds.right.max(label_right);
    }

    PreparedSequenceGroupBox {
        bounds,
        label_lines,
        background: sequence_box.background,
    }
}

fn sequence_box_bounds(
    sequence_box: &SequenceGroupBox,
    layout: &SequenceLayout,
    content_width: usize,
) -> SequenceGroupBoxBounds {
    if sequence_box.actor_indices.is_empty() {
        return sequence_box_full_width_bounds(content_width, sequence_box, layout.width_profile);
    }

    sequence_box_actor_bounds(sequence_box, layout, content_width)
}

fn sequence_box_full_width_bounds(
    content_width: usize,
    sequence_box: &SequenceGroupBox,
    width_profile: crate::options::TerminalWidthProfile,
) -> SequenceGroupBoxBounds {
    let label_width = sequence_box
        .label
        .as_ref()
        .map(|label| {
            display_width_with_profile(label, width_profile) + 2 * SEQUENCE_BOX_LABEL_MARGIN
        })
        .unwrap_or(0);
    let right = content_width
        .max(label_width)
        .max(SEQUENCE_BOX_LABEL_MARGIN * 2 + 1);

    SequenceGroupBoxBounds { left: 0, right }
}

fn sequence_box_actor_bounds(
    sequence_box: &SequenceGroupBox,
    layout: &SequenceLayout,
    content_width: usize,
) -> SequenceGroupBoxBounds {
    let mut left = usize::MAX;
    let mut right = 0;

    for actor_index in &sequence_box.actor_indices {
        let box_width = layout.participant_widths[*actor_index] + BOX_BORDER_WIDTH;
        let participant_left = layout.participant_centers[*actor_index] - box_width / 2;
        let participant_right = participant_left + box_width - 1;
        left = left.min(participant_left);
        right = right.max(participant_right + SEQUENCE_BOX_CONTENT_OFFSET + 1);
    }

    if sequence_box.actor_indices.len() == layout.participant_widths.len() {
        right = right.max(content_width);
    }

    SequenceGroupBoxBounds { left, right }
}

fn sequence_box_label_lines(
    sequence_box: &SequenceGroupBox,
    label_width: usize,
    width_profile: crate::options::TerminalWidthProfile,
) -> Vec<String> {
    let Some(label) = &sequence_box.label else {
        return Vec::new();
    };

    if sequence_box.wrap {
        wrap_label_lines_with_profile(
            label,
            label_width.max(SEQUENCE_BOX_WRAP_TEXT_WIDTH),
            width_profile,
        )
    } else {
        split_label_lines(label)
    }
}

fn draw_sequence_box(
    canvas: &mut [SequenceLine],
    sequence_box: PreparedSequenceGroupBox,
    chars: &SequenceChars,
) {
    let bounds = sequence_box.bounds;
    if canvas.is_empty() || bounds.left >= bounds.right {
        return;
    }

    let top = 0;
    let bottom = canvas.len() - 1;

    paint_sequence_box_background(canvas, bounds, top, bottom, sequence_box.background);

    for x in bounds.left..=bounds.right {
        canvas[top].set_role(x, chars.horizontal, AsciiColorRole::SequenceFrame);
        canvas[bottom].set_role(x, chars.horizontal, AsciiColorRole::SequenceFrame);
    }
    canvas[top].set_role(bounds.left, chars.top_left, AsciiColorRole::SequenceFrame);
    canvas[top].set_role(bounds.right, chars.top_right, AsciiColorRole::SequenceFrame);
    canvas[bottom].set_role(
        bounds.left,
        chars.bottom_left,
        AsciiColorRole::SequenceFrame,
    );
    canvas[bottom].set_role(
        bounds.right,
        chars.bottom_right,
        AsciiColorRole::SequenceFrame,
    );

    for row in canvas.iter_mut().take(bottom).skip(top + 1) {
        draw_background_vertical(row, bounds.left, chars.vertical);
        draw_background_vertical(row, bounds.right, chars.vertical);
    }

    for (line_index, line) in sequence_box.label_lines.iter().enumerate() {
        let Some(row) = canvas.get_mut(line_index) else {
            break;
        };
        draw_sequence_box_label(row, line, bounds);
    }
}

fn paint_sequence_box_background(
    canvas: &mut [SequenceLine],
    bounds: SequenceGroupBoxBounds,
    top: usize,
    bottom: usize,
    background: Option<AsciiRgb>,
) {
    let Some(background) = background else {
        return;
    };

    for row in canvas.iter_mut().take(bottom + 1).skip(top) {
        for x in bounds.left..=bounds.right {
            row.set_background_color_if_unset(x, background);
        }
    }
}

fn draw_sequence_box_label(row: &mut SequenceLine, label: &str, bounds: SequenceGroupBoxBounds) {
    let label = format!(" {label} ");
    let index = bounds.left + SEQUENCE_BOX_LABEL_MARGIN;
    let available = bounds.right.saturating_sub(index);
    let label = truncate_display_width_with_profile(&label, available, row.width_profile());
    row.write_text_role(index, &label, AsciiColorRole::Text);
}

fn draw_background_vertical(row: &mut SequenceLine, index: usize, vertical: char) {
    // Mermaid boxes are background regions; do not corrupt foreground labels or frames.
    if row.get(index) == Some(' ') {
        row.set_role(index, vertical, AsciiColorRole::SequenceFrame);
    }
}
