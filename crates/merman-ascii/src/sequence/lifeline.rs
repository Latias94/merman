use super::SequenceCheckpointCursor;
use super::chars::SequenceChars;
use super::layout::SequenceLayout;
use super::text::{
    SequenceLine, blank_line_with_checkpoints, padded_line_with_checkpoints, trim_right,
};
use crate::color::AsciiColorRole;
use crate::error::Result;
use crate::resource::ResourceContext;

pub(super) fn build_lifeline_line(
    layout: &SequenceLayout,
    chars: &SequenceChars,
    active_counts: &[usize],
    visible_actors: &[bool],
    resources: &ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceLine> {
    let width = resources.checked_grid_add(layout.total_width, 1)?;
    let mut line = blank_line_with_checkpoints(
        width,
        layout.policy.terminal_width_profile,
        resources,
        checkpoints,
    )?;
    for (index, center) in layout.participant_centers.iter().enumerate() {
        checkpoints.tick()?;
        if !visible_actors.get(index).copied().unwrap_or(true) {
            continue;
        }
        line.try_set_role(
            *center,
            lifeline_char(index, chars, active_counts),
            lifeline_role(index, active_counts),
        )?;
    }
    trim_right(line)
}

pub(super) fn retained_lifeline_width(
    layout: &SequenceLayout,
    visible_actors: &[bool],
    resources: &ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<usize> {
    let mut width = 0usize;
    for (index, center) in layout.participant_centers.iter().enumerate() {
        checkpoints.tick()?;
        if visible_actors.get(index).copied().unwrap_or(true) {
            width = width.max(resources.checked_grid_add(*center, 1)?);
        }
    }
    Ok(width)
}

pub(super) fn lifeline_char(index: usize, chars: &SequenceChars, active_counts: &[usize]) -> char {
    if active_counts.get(index).copied().unwrap_or(0) > 0 {
        chars.active_vertical
    } else {
        chars.vertical
    }
}

pub(super) fn lifeline_role(index: usize, active_counts: &[usize]) -> AsciiColorRole {
    if active_counts.get(index).copied().unwrap_or(0) > 0 {
        AsciiColorRole::SequenceActivation
    } else {
        AsciiColorRole::SequenceLifeline
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_overlay_row(
    layout: &SequenceLayout,
    chars: &SequenceChars,
    active_counts: &[usize],
    visible_actors: &[bool],
    left: usize,
    overlay: &SequenceLine,
    resources: &ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceLine> {
    let needed = resources.checked_grid_add(left, overlay.len())?;
    let width = needed.max(resources.checked_grid_add(layout.total_width, 1)?);
    resources.grid_extent(width, 1)?;
    let mut line = padded_line_with_checkpoints(
        build_lifeline_line(
            layout,
            chars,
            active_counts,
            visible_actors,
            resources,
            checkpoints,
        )?,
        width,
        checkpoints,
    )?;
    line.try_write_line_with_checkpoint(left, overlay, resources, || checkpoints.checkpoint())?;
    trim_right(line)
}
