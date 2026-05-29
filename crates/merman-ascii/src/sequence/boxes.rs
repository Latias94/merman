use super::{BOX_BORDER_WIDTH, SEQUENCE_BOX_CONTENT_OFFSET, SEQUENCE_BOX_LABEL_MARGIN};
use crate::text::display_width;

use super::layout::SequenceLayout;
use super::model::{AsciiSequenceDiagram, SequenceGroupBox};
use super::render::SequenceChars;
use super::text::trim_right;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SequenceGroupBoxBounds {
    left: usize,
    right: usize,
}

pub(super) fn render_sequence_boxes(
    lines: Vec<String>,
    diagram: &AsciiSequenceDiagram,
    layout: &SequenceLayout,
    chars: &SequenceChars,
) -> Vec<String> {
    let bounds = diagram
        .boxes
        .iter()
        .map(|sequence_box| sequence_box_bounds(sequence_box, layout))
        .collect::<Vec<_>>();
    let content_width = lines
        .iter()
        .map(|line| line.chars().count() + SEQUENCE_BOX_CONTENT_OFFSET)
        .max()
        .unwrap_or(0);
    let box_width = bounds
        .iter()
        .map(|bounds| bounds.right + 1)
        .max()
        .unwrap_or(0);
    let width = content_width.max(box_width);

    let mut canvas = Vec::with_capacity(lines.len() + 2);
    canvas.push(vec![' '; width]);
    for line in lines {
        let mut row = Vec::with_capacity(width);
        row.extend(std::iter::repeat_n(' ', SEQUENCE_BOX_CONTENT_OFFSET));
        row.extend(line.chars());
        if row.len() < width {
            row.extend(std::iter::repeat_n(' ', width - row.len()));
        }
        canvas.push(row);
    }
    canvas.push(vec![' '; width]);

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
    canvas: &mut [Vec<char>],
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
        canvas[top][x] = chars.horizontal;
        canvas[bottom][x] = chars.horizontal;
    }
    canvas[top][bounds.left] = chars.top_left;
    canvas[top][bounds.right] = chars.top_right;
    canvas[bottom][bounds.left] = chars.bottom_left;
    canvas[bottom][bounds.right] = chars.bottom_right;

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
                canvas[top][index] = ch;
            }
        }
    }
}

fn draw_background_vertical(row: &mut [char], index: usize, vertical: char) {
    // Mermaid boxes are background regions; do not corrupt foreground labels or frames.
    if row[index] == ' ' {
        row[index] = vertical;
    }
}
