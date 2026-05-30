use super::{BOX_BORDER_WIDTH, SEQUENCE_BOX_CONTENT_OFFSET, SEQUENCE_BOX_LABEL_MARGIN};
use crate::color::AsciiColorRole;
use crate::text::display_width;

use super::layout::SequenceLayout;
use super::model::{AsciiSequenceDiagram, SequenceGroupBox};
use super::render::SequenceChars;
use super::text::{SequenceLine, trim_right};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SequenceGroupBoxBounds {
    left: usize,
    right: usize,
}

pub(super) fn render_sequence_boxes(
    lines: Vec<SequenceLine>,
    diagram: &AsciiSequenceDiagram,
    layout: &SequenceLayout,
    chars: &SequenceChars,
) -> Vec<SequenceLine> {
    let bounds = diagram
        .boxes
        .iter()
        .map(|sequence_box| sequence_box_bounds(sequence_box, layout))
        .collect::<Vec<_>>();
    let content_width = lines
        .iter()
        .map(|line| line.len() + SEQUENCE_BOX_CONTENT_OFFSET)
        .max()
        .unwrap_or(0);
    let box_width = bounds
        .iter()
        .map(|bounds| bounds.right + 1)
        .max()
        .unwrap_or(0);
    let width = content_width.max(box_width);

    let mut canvas = Vec::with_capacity(lines.len() + 2);
    canvas.push(SequenceLine::blank(width));
    for line in lines {
        let mut row = SequenceLine::blank(0);
        row.push_spaces(SEQUENCE_BOX_CONTENT_OFFSET);
        row.push_line(&line);
        row.pad_to(width);
        canvas.push(row);
    }
    canvas.push(SequenceLine::blank(width));

    for (sequence_box, bounds) in diagram.boxes.iter().zip(bounds) {
        draw_sequence_box(&mut canvas, sequence_box, bounds, chars);
    }

    canvas.into_iter().map(trim_right).collect()
}

fn sequence_box_bounds(
    sequence_box: &SequenceGroupBox,
    layout: &SequenceLayout,
) -> SequenceGroupBoxBounds {
    let mut left = usize::MAX;
    let mut right = 0;

    for actor_index in &sequence_box.actor_indices {
        let box_width = layout.participant_widths[*actor_index] + BOX_BORDER_WIDTH;
        let participant_left = layout.participant_centers[*actor_index] - box_width / 2;
        let participant_right = participant_left + box_width - 1;
        left = left.min((participant_left + SEQUENCE_BOX_CONTENT_OFFSET).saturating_sub(1));
        right = right.max(participant_right + SEQUENCE_BOX_CONTENT_OFFSET + 1);
    }

    if let Some(label) = &sequence_box.label {
        let label_right = left + display_width(label) + 2 * SEQUENCE_BOX_LABEL_MARGIN;
        right = right.max(label_right);
    }

    SequenceGroupBoxBounds { left, right }
}

fn draw_sequence_box(
    canvas: &mut [SequenceLine],
    sequence_box: &SequenceGroupBox,
    bounds: SequenceGroupBoxBounds,
    chars: &SequenceChars,
) {
    if canvas.is_empty() || bounds.left >= bounds.right {
        return;
    }

    let top = 0;
    let bottom = canvas.len() - 1;

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

    if let Some(label) = &sequence_box.label {
        let label = format!(" {label} ");
        let start = bounds.left + SEQUENCE_BOX_LABEL_MARGIN;
        for (offset, ch) in label.chars().enumerate() {
            let index = start + offset;
            if index < bounds.right {
                canvas[top].set_role(index, ch, AsciiColorRole::Text);
            }
        }
    }
}

fn draw_background_vertical(row: &mut SequenceLine, index: usize, vertical: char) {
    // Mermaid boxes are background regions; do not corrupt foreground labels or frames.
    if row.get(index) == Some(' ') {
        row.set_role(index, vertical, AsciiColorRole::SequenceFrame);
    }
}
