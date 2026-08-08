use super::{LABEL_BUFFER_SPACE, LABEL_LEFT_MARGIN};
use crate::color::AsciiColorRole;
use crate::error::{AsciiError, Result};
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use crate::text::{display_width_with_profile, wrap_display_lines_with_profile};

use super::layout::SequenceLayout;
use super::model::{SequenceArrowHead, SequenceLineStyle, SequenceMessage};
use super::render::{
    SequenceChars, build_lifeline_line, lifeline_char, lifeline_role, retained_lifeline_width,
};
use super::text::{
    SequenceBatchExtent, SequenceLine, charge_text_work, ensure_self_width, padded_line,
    trim_right, write_text_role,
};

#[derive(Debug)]
pub(super) struct PreparedMessageRows {
    label_lines: Vec<String>,
    extent: SequenceBatchExtent,
}

impl PreparedMessageRows {
    pub(super) const fn extent(&self) -> SequenceBatchExtent {
        self.extent
    }
}

pub(super) fn ensure_message_actors_visible(
    message: &SequenceMessage,
    visible_actors: &[bool],
) -> Result<()> {
    if visible_actors.get(message.from).copied().unwrap_or(false)
        && visible_actors.get(message.to).copied().unwrap_or(false)
    {
        return Ok(());
    }

    Err(AsciiError::UnsupportedFeature {
        diagram_type: "sequence",
        feature: "actor lifecycle visibility",
    })
}

fn message_label_lines(
    message: &SequenceMessage,
    max_width: usize,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<Vec<String>> {
    charge_text_work(&message.label, width_profile, resources)?;
    if message.label.is_empty() {
        Ok(Vec::new())
    } else if message.wrap {
        Ok(wrap_display_lines_with_profile(
            &message.label,
            max_width,
            width_profile,
        ))
    } else {
        let mut lines = Vec::new();
        lines.try_reserve(1).map_err(|_| allocation_failed())?;
        lines.push(try_clone_string(&message.label)?);
        Ok(lines)
    }
}

pub(super) fn prepare_message_rows(
    message: &SequenceMessage,
    layout: &SequenceLayout,
    visible_actors: &[bool],
    resources: &mut ResourceContext,
) -> Result<PreparedMessageRows> {
    let from = layout.participant_centers[message.from];
    let to = layout.participant_centers[message.to];
    let label_lines = message_label_lines(
        message,
        from.abs_diff(to).saturating_sub(LABEL_LEFT_MARGIN),
        layout.width_profile,
        resources,
    )?;
    let row_count = resources.checked_grid_add(label_lines.len(), 1)?;
    let start = resources.checked_grid_add(from.min(to), LABEL_LEFT_MARGIN)?;
    let mut max_width = resources.checked_grid_add(layout.total_width, 1)?;
    for label in &label_lines {
        let label_right = resources.checked_grid_add(
            start,
            display_width_with_profile(label, layout.width_profile),
        )?;
        let label_width =
            resources.checked_grid_add(layout.total_width.max(label_right), LABEL_BUFFER_SPACE)?;
        max_width = max_width.max(label_width);
    }
    resources.grid_extent(max_width, row_count)?;
    charge_row_work(resources, max_width, row_count)?;

    let lifeline_width = retained_lifeline_width(layout, visible_actors, resources)?;
    let extent = SequenceBatchExtent::try_from_line_lengths(
        max_width,
        label_lines
            .iter()
            .map(|label| {
                resources
                    .checked_grid_add(start, retained_label_width(label, layout.width_profile))
                    .map(|label_right| lifeline_width.max(label_right))
            })
            .chain(std::iter::once(Ok(lifeline_width))),
        resources,
    )?;

    Ok(PreparedMessageRows {
        label_lines,
        extent,
    })
}

pub(super) fn render_message(
    prepared: PreparedMessageRows,
    message: &SequenceMessage,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    active_counts: &[usize],
    visible_actors: &[bool],
    destroyed_actors: &[usize],
    resources: &mut ResourceContext,
) -> Result<Vec<SequenceLine>> {
    let PreparedMessageRows {
        label_lines,
        extent,
    } = prepared;
    let row_count = extent.height();
    let from = layout.participant_centers[message.from];
    let to = layout.participant_centers[message.to];
    let start = resources.checked_grid_add(from.min(to), LABEL_LEFT_MARGIN)?;

    let mut lines = Vec::new();
    lines
        .try_reserve_exact(row_count)
        .map_err(|_| allocation_failed())?;

    for label in label_lines {
        let label_width = display_width_with_profile(&label, layout.width_profile);
        let label_right = resources.checked_grid_add(start, label_width)?;
        let width =
            resources.checked_grid_add(layout.total_width.max(label_right), LABEL_BUFFER_SPACE)?;
        let mut line = padded_line(
            build_lifeline_line(layout, chars, active_counts, visible_actors, resources)?,
            width,
        )?;
        write_text_role(&mut line, start, &label, AsciiColorRole::EdgeLabel)?;
        lines.push(trim_right(line)?);
    }

    let mut line = build_lifeline_line(layout, chars, active_counts, visible_actors, resources)?;
    let style = match message.style {
        SequenceLineStyle::Solid => chars.solid_line,
        SequenceLineStyle::Dotted => chars.dotted_line,
    };

    if from < to {
        let line_start = resources.checked_grid_add(from, 1)?;
        let arrow_x = to.checked_sub(1).ok_or_else(invalid_message_geometry)?;
        if destroyed_actors.contains(&message.from) {
            line.try_set_role(from, chars.destroyed_mark, AsciiColorRole::EdgeArrow)?;
        } else {
            line.try_set_role(from, chars.tee_right, AsciiColorRole::Junction)?;
        }
        for x in line_start..to {
            line.try_set_role(x, style, AsciiColorRole::EdgeLine)?;
        }
        if destroyed_actors.contains(&message.to) && message.arrow == SequenceArrowHead::Cross {
            line.try_set_role(arrow_x, style, AsciiColorRole::EdgeLine)?;
        } else {
            line.try_set_role(
                arrow_x,
                chars.arrow_right(message.arrow),
                AsciiColorRole::EdgeArrow,
            )?;
        }
        if destroyed_actors.contains(&message.to) {
            line.try_set_role(to, chars.destroyed_mark, AsciiColorRole::EdgeArrow)?;
        } else {
            line.try_set_role(
                to,
                lifeline_char(message.to, chars, active_counts),
                lifeline_role(message.to, active_counts),
            )?;
        }
    } else {
        let arrow_x = resources.checked_grid_add(to, 1)?;
        let line_start = resources.checked_grid_add(to, 2)?;
        if destroyed_actors.contains(&message.to) {
            line.try_set_role(to, chars.destroyed_mark, AsciiColorRole::EdgeArrow)?;
        } else {
            line.try_set_role(
                to,
                lifeline_char(message.to, chars, active_counts),
                lifeline_role(message.to, active_counts),
            )?;
        }
        if destroyed_actors.contains(&message.to) && message.arrow == SequenceArrowHead::Cross {
            line.try_set_role(arrow_x, style, AsciiColorRole::EdgeLine)?;
        } else {
            line.try_set_role(
                arrow_x,
                chars.arrow_left(message.arrow),
                AsciiColorRole::EdgeArrow,
            )?;
        }
        for x in line_start..from {
            line.try_set_role(x, style, AsciiColorRole::EdgeLine)?;
        }
        if destroyed_actors.contains(&message.from) {
            line.try_set_role(from, chars.destroyed_mark, AsciiColorRole::EdgeArrow)?;
        } else {
            line.try_set_role(from, chars.tee_left, AsciiColorRole::Junction)?;
        }
    }
    lines.push(trim_right(line)?);
    Ok(lines)
}

pub(super) fn prepare_self_message_rows(
    message: &SequenceMessage,
    layout: &SequenceLayout,
    visible_actors: &[bool],
    resources: &mut ResourceContext,
) -> Result<PreparedMessageRows> {
    let center = layout.participant_centers[message.from];
    let width = layout.self_message_width;
    let label_wrap_width = resources.checked_grid_add(width, LABEL_BUFFER_SPACE)?;
    let label_lines =
        message_label_lines(message, label_wrap_width, layout.width_profile, resources)?;
    let row_count = resources.checked_grid_add(label_lines.len(), 3)?;
    let base_width = resources.checked_grid_add(
        resources.checked_grid_add(layout.total_width, layout.self_message_width)?,
        1,
    )?;
    let start = resources.checked_grid_add(center, LABEL_LEFT_MARGIN)?;
    let mut max_width = base_width;
    for label in &label_lines {
        let label_right = resources.checked_grid_add(
            start,
            display_width_with_profile(label, layout.width_profile),
        )?;
        max_width = max_width.max(resources.checked_grid_add(label_right, LABEL_BUFFER_SPACE)?);
    }
    resources.grid_extent(max_width, row_count)?;
    charge_row_work(resources, max_width, row_count)?;

    let lifeline_width = retained_lifeline_width(layout, visible_actors, resources)?;
    let loop_right_offset = width.checked_sub(1).ok_or_else(invalid_message_geometry)?;
    let loop_right = resources.checked_grid_add(center, loop_right_offset)?;
    let message_row_width = lifeline_width.max(resources.checked_grid_add(loop_right, 1)?);
    let extent = SequenceBatchExtent::try_from_line_lengths(
        max_width,
        label_lines
            .iter()
            .map(|label| {
                resources
                    .checked_grid_add(start, retained_label_width(label, layout.width_profile))
                    .map(|label_right| lifeline_width.max(label_right))
            })
            .chain([message_row_width; 3].into_iter().map(Ok)),
        resources,
    )?;

    Ok(PreparedMessageRows {
        label_lines,
        extent,
    })
}

pub(super) fn render_self_message(
    prepared: PreparedMessageRows,
    message: &SequenceMessage,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    active_counts: &[usize],
    visible_actors: &[bool],
    destroyed_actors: &[usize],
    resources: &mut ResourceContext,
) -> Result<Vec<SequenceLine>> {
    let PreparedMessageRows {
        label_lines,
        extent,
    } = prepared;
    let row_count = extent.height();
    let center = layout.participant_centers[message.from];
    let width = layout.self_message_width;
    let start = resources.checked_grid_add(center, LABEL_LEFT_MARGIN)?;

    let mut lines = Vec::new();
    lines
        .try_reserve_exact(row_count)
        .map_err(|_| allocation_failed())?;

    for label in label_lines {
        let label_right = resources.checked_grid_add(
            start,
            display_width_with_profile(&label, layout.width_profile),
        )?;
        let needed = resources.checked_grid_add(label_right, LABEL_BUFFER_SPACE)?;
        let mut line = ensure_self_width(
            build_lifeline_line(layout, chars, active_counts, visible_actors, resources)?,
            layout,
            needed,
            resources,
        )?;
        write_text_role(&mut line, start, &label, AsciiColorRole::EdgeLabel)?;
        lines.push(trim_right(line)?);
    }

    let mut top = ensure_self_width(
        build_lifeline_line(layout, chars, active_counts, visible_actors, resources)?,
        layout,
        0,
        resources,
    )?;
    let loop_right_offset = width.checked_sub(1).ok_or_else(invalid_message_geometry)?;
    let loop_right = resources.checked_grid_add(center, loop_right_offset)?;
    let arrow_x = resources.checked_grid_add(center, 1)?;
    top.try_set_role(center, chars.tee_right, AsciiColorRole::Junction)?;
    for offset in 1..width {
        top.try_set_role(
            resources.checked_grid_add(center, offset)?,
            chars.horizontal,
            AsciiColorRole::EdgeLine,
        )?;
    }
    top.try_set_role(loop_right, chars.self_top_right, AsciiColorRole::EdgeLine)?;
    lines.push(trim_right(top)?);

    let mut middle = ensure_self_width(
        build_lifeline_line(layout, chars, active_counts, visible_actors, resources)?,
        layout,
        0,
        resources,
    )?;
    middle.try_set_role(loop_right, chars.vertical, AsciiColorRole::EdgeLine)?;
    lines.push(trim_right(middle)?);

    let mut bottom = ensure_self_width(
        build_lifeline_line(layout, chars, active_counts, visible_actors, resources)?,
        layout,
        0,
        resources,
    )?;
    if destroyed_actors.contains(&message.from) {
        bottom.try_set_role(center, chars.destroyed_mark, AsciiColorRole::EdgeArrow)?;
    } else {
        bottom.try_set_role(
            center,
            lifeline_char(message.from, chars, active_counts),
            lifeline_role(message.from, active_counts),
        )?;
    }
    bottom.try_set_role(
        arrow_x,
        chars.arrow_left(message.arrow),
        AsciiColorRole::EdgeArrow,
    )?;
    for offset in 2..loop_right_offset {
        bottom.try_set_role(
            resources.checked_grid_add(center, offset)?,
            chars.horizontal,
            AsciiColorRole::EdgeLine,
        )?;
    }
    bottom.try_set_role(loop_right, chars.self_bottom, AsciiColorRole::EdgeLine)?;
    lines.push(trim_right(bottom)?);

    Ok(lines)
}

fn charge_row_work(resources: &mut ResourceContext, width: usize, height: usize) -> Result<()> {
    let work = width.checked_mul(height).ok_or_else(|| {
        resources
            .policy()
            .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
    })?;
    resources.charge_layout_work(work)
}

fn retained_label_width(label: &str, width_profile: TerminalWidthProfile) -> usize {
    display_width_with_profile(label.trim_end_matches(' '), width_profile)
}

fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

fn try_clone_string(source: &str) -> Result<String> {
    let mut cloned = String::new();
    cloned
        .try_reserve_exact(source.len())
        .map_err(|_| allocation_failed())?;
    cloned.push_str(source);
    Ok(cloned)
}

fn invalid_message_geometry() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "sequence",
        feature: "message geometry",
    }
}
