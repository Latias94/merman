use super::{
    BOX_BORDER_WIDTH, BOX_PADDING_LEFT_RIGHT, MIN_BOX_WIDTH, NOTE_SIDE_GAP, NOTE_WRAP_TEXT_WIDTH,
};
use crate::color::AsciiColorRole;
use crate::error::{AsciiError, Result};
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use crate::text::{display_width_with_profile, split_label_lines, wrap_label_lines_with_profile};

use super::layout::SequenceLayout;
use super::model::{SequenceNote, SequenceNotePlacement};
use super::render::{SequenceChars, render_overlay_row, retained_lifeline_width};
use super::text::{SequenceBatchExtent, SequenceLine, blank_line, charge_text_work};

#[derive(Debug)]
pub(super) struct PreparedNoteRows {
    label_lines: Vec<String>,
    inner_width: usize,
    left: usize,
    extent: SequenceBatchExtent,
}

impl PreparedNoteRows {
    pub(super) const fn extent(&self) -> SequenceBatchExtent {
        self.extent
    }
}

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

pub(super) fn prepare_note_rows(
    note: &SequenceNote,
    layout: &SequenceLayout,
    visible_actors: &[bool],
    resources: &mut ResourceContext,
) -> Result<PreparedNoteRows> {
    let label_lines = note_label_lines(note, layout, resources)?;
    let label_width = label_lines
        .iter()
        .map(|line| display_width_with_profile(line, layout.width_profile))
        .max()
        .unwrap_or(0);
    let mut inner_width = resources
        .checked_grid_add(label_width, BOX_PADDING_LEFT_RIGHT)?
        .max(MIN_BOX_WIDTH);
    let from = layout.participant_centers[note.from];
    let to = layout.participant_centers[note.to];

    let left = match note.placement {
        SequenceNotePlacement::LeftOf => {
            let total_width = resources.checked_grid_add(inner_width, BOX_BORDER_WIDTH)?;
            from.saturating_sub(resources.checked_grid_add(total_width, NOTE_SIDE_GAP)?)
        }
        SequenceNotePlacement::RightOf => resources.checked_grid_add(from, NOTE_SIDE_GAP)?,
        SequenceNotePlacement::Over => {
            if from == to {
                let total_width = resources.checked_grid_add(inner_width, BOX_BORDER_WIDTH)?;
                from.saturating_sub(total_width / 2)
            } else {
                let span_left = from.min(to).saturating_sub(1);
                let span_inner_width = resources.checked_grid_add(from.abs_diff(to), 1)?;
                inner_width = inner_width.max(span_inner_width);
                span_left
            }
        }
    };

    let row_count = resources.checked_grid_add(label_lines.len(), 2)?;
    let note_width = resources.checked_grid_add(inner_width, BOX_BORDER_WIDTH)?;
    let overlay_right = resources.checked_grid_add(left, note_width)?;
    let max_width = resources
        .checked_grid_add(layout.total_width, 1)?
        .max(overlay_right);
    resources.grid_extent(max_width, row_count)?;
    charge_note_work(resources, max_width, row_count)?;

    let retained_width =
        retained_lifeline_width(layout, visible_actors, resources)?.max(overlay_right);
    let extent = SequenceBatchExtent::uniform(row_count, max_width, retained_width, resources)?;
    Ok(PreparedNoteRows {
        label_lines,
        inner_width,
        left,
        extent,
    })
}

pub(super) fn render_note(
    prepared: PreparedNoteRows,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    active_counts: &[usize],
    visible_actors: &[bool],
    resources: &mut ResourceContext,
) -> Result<Vec<SequenceLine>> {
    let PreparedNoteRows {
        label_lines,
        inner_width,
        left,
        extent,
    } = prepared;
    let row_count = extent.height();
    let note_width = resources.checked_grid_add(inner_width, BOX_BORDER_WIDTH)?;

    let mut rows = Vec::new();
    rows.try_reserve_exact(row_count)
        .map_err(|_| allocation_failed())?;
    rows.push(note_border_row(
        chars.top_left,
        chars.top_right,
        chars.horizontal,
        inner_width,
        layout.width_profile,
        resources,
    )?);
    for line in label_lines {
        let line_width = display_width_with_profile(&line, layout.width_profile);
        let left_padding = inner_width
            .checked_sub(line_width)
            .ok_or_else(invalid_note_geometry)?
            / 2;
        let mut row = blank_line(note_width, layout.width_profile, resources)?;
        row.try_set_role(0, chars.vertical, AsciiColorRole::SequenceFrame)?;
        row.try_write_text_role(
            resources.checked_grid_add(1, left_padding)?,
            &line,
            AsciiColorRole::Text,
        )?;
        row.try_set_role(
            resources.checked_grid_add(inner_width, 1)?,
            chars.vertical,
            AsciiColorRole::SequenceFrame,
        )?;
        rows.push(row);
    }
    rows.push(note_border_row(
        chars.bottom_left,
        chars.bottom_right,
        chars.horizontal,
        inner_width,
        layout.width_profile,
        resources,
    )?);

    let mut rendered = Vec::new();
    rendered
        .try_reserve_exact(row_count)
        .map_err(|_| allocation_failed())?;
    for row in rows {
        rendered.push(render_overlay_row(
            layout,
            chars,
            active_counts,
            visible_actors,
            left,
            &row,
            resources,
        )?);
    }
    Ok(rendered)
}

fn charge_note_work(resources: &mut ResourceContext, width: usize, height: usize) -> Result<()> {
    let work = width.checked_mul(height).ok_or_else(|| {
        resources
            .policy()
            .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
    })?;
    resources.charge_layout_work(work)
}

fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

fn invalid_note_geometry() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "sequence",
        feature: "note geometry",
    }
}

fn note_border_row(
    left: char,
    right: char,
    horizontal: char,
    inner_width: usize,
    width_profile: crate::options::TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<SequenceLine> {
    let total_width = resources.checked_grid_add(inner_width, BOX_BORDER_WIDTH)?;
    let mut row = blank_line(total_width, width_profile, resources)?;
    row.try_set_role(0, left, AsciiColorRole::SequenceFrame)?;
    for x in 1..=inner_width {
        row.try_set_role(x, horizontal, AsciiColorRole::SequenceFrame)?;
    }
    row.try_set_role(
        resources.checked_grid_add(inner_width, 1)?,
        right,
        AsciiColorRole::SequenceFrame,
    )?;
    Ok(row)
}

fn note_label_lines(
    note: &SequenceNote,
    layout: &SequenceLayout,
    resources: &mut ResourceContext,
) -> Result<Vec<String>> {
    charge_text_work(&note.label, layout.width_profile, resources)?;
    if !note.wrap {
        return Ok(split_label_lines(&note.label));
    }

    let span_width =
        layout.participant_centers[note.from].abs_diff(layout.participant_centers[note.to]);
    Ok(wrap_label_lines_with_profile(
        &note.label,
        span_width.max(NOTE_WRAP_TEXT_WIDTH),
        layout.width_profile,
    ))
}
