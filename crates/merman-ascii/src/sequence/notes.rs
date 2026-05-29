use super::{
    BOX_BORDER_WIDTH, BOX_PADDING_LEFT_RIGHT, MIN_BOX_WIDTH, NOTE_SIDE_GAP, NOTE_WRAP_TEXT_WIDTH,
};
use crate::error::{AsciiError, Result};
use crate::text::{display_width, wrap_display_lines};

use super::layout::SequenceLayout;
use super::model::{SequenceNote, SequenceNotePlacement};
use super::render::{SequenceChars, render_overlay_row};

pub(super) fn ensure_note_actors_visible(
    note: &SequenceNote,
    visible_actors: &[bool],
) -> Result<()> {
    if visible_actors.get(note.from).copied().unwrap_or(false)
        && visible_actors.get(note.to).copied().unwrap_or(false)
    {
        return Ok(());
    }

    Err(AsciiError::UnsupportedFeature {
        diagram_type: "sequence",
        feature: "actor lifecycle visibility",
    })
}

pub(super) fn render_note(
    note: &SequenceNote,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    active_counts: &[usize],
    visible_actors: &[bool],
) -> Vec<String> {
    let label_lines = note_label_lines(note, layout);
    let label_width = label_lines
        .iter()
        .map(|line| display_width(line))
        .max()
        .unwrap_or(0);
    let mut inner_width = (label_width + BOX_PADDING_LEFT_RIGHT).max(MIN_BOX_WIDTH);
    let from = layout.participant_centers[note.from];
    let to = layout.participant_centers[note.to];

    let left = match note.placement {
        SequenceNotePlacement::LeftOf => {
            let total_width = inner_width + BOX_BORDER_WIDTH;
            from.saturating_sub(total_width + NOTE_SIDE_GAP)
        }
        SequenceNotePlacement::RightOf => from + NOTE_SIDE_GAP,
        SequenceNotePlacement::Over => {
            if from == to {
                let total_width = inner_width + BOX_BORDER_WIDTH;
                from.saturating_sub(total_width / 2)
            } else {
                let span_left = from.min(to).saturating_sub(1);
                let span_inner_width = from.abs_diff(to) + 1;
                inner_width = inner_width.max(span_inner_width);
                span_left
            }
        }
    };

    let top = format!(
        "{}{}{}",
        chars.top_left,
        chars.horizontal.to_string().repeat(inner_width),
        chars.top_right
    );
    let bottom = format!(
        "{}{}{}",
        chars.bottom_left,
        chars.horizontal.to_string().repeat(inner_width),
        chars.bottom_right
    );

    let mut rows = Vec::with_capacity(label_lines.len() + 2);
    rows.push(top);
    for line in label_lines {
        let line_width = display_width(&line);
        let left_padding = (inner_width - line_width) / 2;
        let right_padding = inner_width - left_padding - line_width;
        rows.push(format!(
            "{}{}{}{}{}",
            chars.vertical,
            " ".repeat(left_padding),
            line,
            " ".repeat(right_padding),
            chars.vertical
        ));
    }
    rows.push(bottom);

    rows.into_iter()
        .map(|row| render_overlay_row(layout, chars, active_counts, visible_actors, left, &row))
        .collect()
}

fn note_label_lines(note: &SequenceNote, layout: &SequenceLayout) -> Vec<String> {
    if note.label.is_empty() {
        return vec![String::new()];
    }

    if !note.wrap {
        return vec![note.label.clone()];
    }

    let span_width =
        layout.participant_centers[note.from].abs_diff(layout.participant_centers[note.to]);
    wrap_display_lines(&note.label, span_width.max(NOTE_WRAP_TEXT_WIDTH))
}
