use super::{
    BOX_BORDER_WIDTH, BOX_PADDING_LEFT_RIGHT, MIN_BOX_WIDTH, NOTE_SIDE_GAP, NOTE_WRAP_TEXT_WIDTH,
};
use crate::color::AsciiColorRole;
use crate::error::{AsciiError, Result};
use crate::text::{display_width, wrap_display_lines};

use super::layout::SequenceLayout;
use super::model::{SequenceNote, SequenceNotePlacement};
use super::render::{SequenceChars, render_overlay_row};
use super::text::SequenceLine;

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
) -> Vec<SequenceLine> {
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

    let mut rows = Vec::with_capacity(label_lines.len() + 2);
    rows.push(note_border_row(
        chars.top_left,
        chars.top_right,
        chars.horizontal,
        inner_width,
    ));
    for line in label_lines {
        let line_width = display_width(&line);
        let left_padding = (inner_width - line_width) / 2;
        let mut row = SequenceLine::blank(inner_width + BOX_BORDER_WIDTH);
        row.set_role(0, chars.vertical, AsciiColorRole::SequenceFrame);
        row.write_text_role(1 + left_padding, &line, AsciiColorRole::Text);
        row.set_role(
            inner_width + 1,
            chars.vertical,
            AsciiColorRole::SequenceFrame,
        );
        rows.push(row);
    }
    rows.push(note_border_row(
        chars.bottom_left,
        chars.bottom_right,
        chars.horizontal,
        inner_width,
    ));

    rows.into_iter()
        .map(|row| render_overlay_row(layout, chars, active_counts, visible_actors, left, &row))
        .collect()
}

fn note_border_row(left: char, right: char, horizontal: char, inner_width: usize) -> SequenceLine {
    let mut row = SequenceLine::blank(inner_width + BOX_BORDER_WIDTH);
    row.set_role(0, left, AsciiColorRole::SequenceFrame);
    for x in 1..=inner_width {
        row.set_role(x, horizontal, AsciiColorRole::SequenceFrame);
    }
    row.set_role(inner_width + 1, right, AsciiColorRole::SequenceFrame);
    row
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
