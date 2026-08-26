use super::SequenceCheckpointCursor;
use super::chars::SequenceChars;
use super::event_plan::{PreparedMessageRows, PreparedSelfMessageRows, invalid_message_geometry};
use super::layout::SequenceLayout;
use super::lifeline::{build_lifeline_line, lifeline_char, lifeline_role};
use super::model::{
    SequenceArrowHead, SequenceCentralDecoration, SequenceLineStyle, SequenceMessage,
    SequenceMessageDirection,
};
use super::text::{SequenceLine, padded_line_with_checkpoints, trim_right, write_text_role};
use crate::color::AsciiColorRole;
use crate::error::{AsciiError, Result};
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};
use crate::text::display_width_with_profile;

#[derive(Debug, Clone, Copy)]
pub(super) struct MessageActorState<'a> {
    active_counts: &'a [usize],
    visible_actors: &'a [bool],
    destroyed_actors: &'a [usize],
}

impl<'a> MessageActorState<'a> {
    pub(super) const fn new(
        active_counts: &'a [usize],
        visible_actors: &'a [bool],
        destroyed_actors: &'a [usize],
    ) -> Self {
        Self {
            active_counts,
            visible_actors,
            destroyed_actors,
        }
    }
}

pub(super) fn render_message(
    prepared: PreparedMessageRows,
    message: &SequenceMessage,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    actor_state: MessageActorState<'_>,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<Vec<SequenceLine>> {
    let MessageActorState {
        active_counts,
        visible_actors,
        destroyed_actors,
    } = actor_state;
    let (label_plan, extent) = prepared.into_render_parts();
    let label_lines = match label_plan {
        Some(plan) => {
            checkpoints.checkpoint()?;
            let materialized = plan
                .materialize_after_admission_with_checkpoint(&message.label, || {
                    checkpoints.checkpoint()
                });
            checkpoints.checkpoint()?;
            materialized?.into_parts().0
        }
        None => Vec::new(),
    };
    let row_count = extent.height();
    let from = layout.participant_centers[message.from];
    let to = layout.participant_centers[message.to];
    let start =
        resources.checked_grid_add(from.min(to), layout.policy.message_label_left_margin)?;

    let mut lines = Vec::new();
    lines
        .try_reserve_exact(row_count)
        .map_err(|_| allocation_failed())?;

    for label in label_lines {
        checkpoints.tick()?;
        let label_width = display_width_with_profile(&label, layout.policy.terminal_width_profile);
        let label_right = resources.checked_grid_add(start, label_width)?;
        let width = resources.checked_grid_add(
            layout.total_width.max(label_right),
            layout.policy.message_label_overflow_buffer,
        )?;
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
        write_text_role(
            &mut line,
            start,
            &label,
            AsciiColorRole::EdgeLabel,
            resources,
            checkpoints,
        )?;
        lines.push(trim_right(line)?);
    }

    let mut line = build_lifeline_line(
        layout,
        chars,
        active_counts,
        visible_actors,
        resources,
        checkpoints,
    )?;
    let style = match message.style {
        SequenceLineStyle::Solid => chars.solid_line,
        SequenceLineStyle::Dotted => chars.dotted_line,
    };
    validate_message_direction(message)?;

    if from < to {
        let line_start = resources.checked_grid_add(from, 1)?;
        let source_marker_x = line_start;
        let target_marker_x = to.checked_sub(1).ok_or_else(invalid_message_geometry)?;
        if destroyed_actors.contains(&message.from) {
            line.try_set_role(from, chars.destroyed_mark, AsciiColorRole::EdgeArrow)?;
        } else {
            line.try_set_role(from, chars.tee_right, AsciiColorRole::Junction)?;
        }
        for x in line_start..to {
            checkpoints.tick()?;
            line.try_set_role(x, style, AsciiColorRole::EdgeLine)?;
        }
        paint_endpoint_marker(
            &mut line,
            source_marker_x,
            message.source_marker,
            false,
            destroyed_actors.contains(&message.from),
            style,
            chars,
        )?;
        paint_endpoint_marker(
            &mut line,
            target_marker_x,
            message.target_marker,
            true,
            destroyed_actors.contains(&message.to),
            style,
            chars,
        )?;
        if destroyed_actors.contains(&message.to) {
            line.try_set_role(to, chars.destroyed_mark, AsciiColorRole::EdgeArrow)?;
        } else {
            line.try_set_role(
                to,
                lifeline_char(message.to, chars, active_counts),
                lifeline_role(message.to, active_counts),
            )?;
        }
        paint_central_decorations(&mut line, message, from, to, destroyed_actors, chars)?;
    } else {
        let target_marker_x = resources.checked_grid_add(to, 1)?;
        let line_start = resources.checked_grid_add(to, 2)?;
        let source_marker_x = from.checked_sub(1).ok_or_else(invalid_message_geometry)?;
        if destroyed_actors.contains(&message.to) {
            line.try_set_role(to, chars.destroyed_mark, AsciiColorRole::EdgeArrow)?;
        } else {
            line.try_set_role(
                to,
                lifeline_char(message.to, chars, active_counts),
                lifeline_role(message.to, active_counts),
            )?;
        }
        line.try_set_role(target_marker_x, style, AsciiColorRole::EdgeLine)?;
        for x in line_start..from {
            checkpoints.tick()?;
            line.try_set_role(x, style, AsciiColorRole::EdgeLine)?;
        }
        paint_endpoint_marker(
            &mut line,
            target_marker_x,
            message.target_marker,
            false,
            destroyed_actors.contains(&message.to),
            style,
            chars,
        )?;
        paint_endpoint_marker(
            &mut line,
            source_marker_x,
            message.source_marker,
            true,
            destroyed_actors.contains(&message.from),
            style,
            chars,
        )?;
        if destroyed_actors.contains(&message.from) {
            line.try_set_role(from, chars.destroyed_mark, AsciiColorRole::EdgeArrow)?;
        } else {
            line.try_set_role(from, chars.tee_left, AsciiColorRole::Junction)?;
        }
        paint_central_decorations(&mut line, message, from, to, destroyed_actors, chars)?;
    }
    lines.push(trim_right(line)?);
    Ok(lines)
}

pub(super) fn render_self_message(
    prepared: PreparedSelfMessageRows,
    message: &SequenceMessage,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    actor_state: MessageActorState<'_>,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<Vec<SequenceLine>> {
    let MessageActorState {
        active_counts,
        visible_actors,
        destroyed_actors,
    } = actor_state;
    let (label_plan, extent, geometry) = prepared.into_render_parts();
    let label_lines = match label_plan {
        Some(plan) => {
            checkpoints.checkpoint()?;
            let materialized = plan
                .materialize_after_admission_with_checkpoint(&message.label, || {
                    checkpoints.checkpoint()
                });
            checkpoints.checkpoint()?;
            materialized?.into_parts().0
        }
        None => Vec::new(),
    };
    let row_count = extent.height();
    let center = layout.participant_centers[message.from];
    let start = resources.checked_grid_add(center, layout.policy.message_label_left_margin)?;

    let mut lines = Vec::new();
    lines
        .try_reserve_exact(row_count)
        .map_err(|_| allocation_failed())?;

    for label in label_lines {
        checkpoints.tick()?;
        let label_right = resources.checked_grid_add(
            start,
            display_width_with_profile(&label, layout.policy.terminal_width_profile),
        )?;
        let needed =
            resources.checked_grid_add(label_right, layout.policy.message_label_overflow_buffer)?;
        let mut line = geometry.pad_line(
            build_lifeline_line(
                layout,
                chars,
                active_counts,
                visible_actors,
                resources,
                checkpoints,
            )?,
            needed,
            checkpoints,
        )?;
        write_text_role(
            &mut line,
            start,
            &label,
            AsciiColorRole::EdgeLabel,
            resources,
            checkpoints,
        )?;
        lines.push(trim_right(line)?);
    }

    let mut top = geometry.pad_line(
        build_lifeline_line(
            layout,
            chars,
            active_counts,
            visible_actors,
            resources,
            checkpoints,
        )?,
        geometry.loop_needed,
        checkpoints,
    )?;
    let style = match message.style {
        SequenceLineStyle::Solid => chars.solid_line,
        SequenceLineStyle::Dotted => chars.dotted_line,
    };
    validate_message_direction(message)?;
    top.try_set_role(center, chars.tee_right, AsciiColorRole::Junction)?;
    for offset in 1..geometry.width {
        checkpoints.tick()?;
        top.try_set_role(
            resources.checked_grid_add(center, offset)?,
            style,
            AsciiColorRole::EdgeLine,
        )?;
    }
    top.try_set_role(
        geometry.loop_right,
        chars.self_top_right,
        AsciiColorRole::EdgeLine,
    )?;
    paint_endpoint_marker(
        &mut top,
        geometry.arrow_x,
        message.source_marker,
        false,
        destroyed_actors.contains(&message.from),
        style,
        chars,
    )?;
    if has_source_central_decoration(message.central_decoration)
        && !destroyed_actors.contains(&message.from)
    {
        top.try_set_role(
            center,
            chars.central_decoration(),
            AsciiColorRole::EdgeArrow,
        )?;
    }
    lines.push(trim_right(top)?);

    let mut middle = geometry.pad_line(
        build_lifeline_line(
            layout,
            chars,
            active_counts,
            visible_actors,
            resources,
            checkpoints,
        )?,
        geometry.loop_needed,
        checkpoints,
    )?;
    middle.try_set_role(
        geometry.loop_right,
        chars.vertical,
        AsciiColorRole::EdgeLine,
    )?;
    lines.push(trim_right(middle)?);

    let mut bottom = geometry.pad_line(
        build_lifeline_line(
            layout,
            chars,
            active_counts,
            visible_actors,
            resources,
            checkpoints,
        )?,
        geometry.loop_needed,
        checkpoints,
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
    bottom.try_set_role(geometry.arrow_x, style, AsciiColorRole::EdgeLine)?;
    for offset in 2..geometry.width - 1 {
        checkpoints.tick()?;
        bottom.try_set_role(
            resources.checked_grid_add(center, offset)?,
            style,
            AsciiColorRole::EdgeLine,
        )?;
    }
    bottom.try_set_role(
        geometry.loop_right,
        chars.self_bottom,
        AsciiColorRole::EdgeLine,
    )?;
    paint_endpoint_marker(
        &mut bottom,
        geometry.arrow_x,
        message.target_marker,
        false,
        destroyed_actors.contains(&message.to),
        style,
        chars,
    )?;
    if has_target_central_decoration(message.central_decoration)
        && !destroyed_actors.contains(&message.to)
    {
        bottom.try_set_role(
            center,
            chars.central_decoration(),
            AsciiColorRole::EdgeArrow,
        )?;
    }
    lines.push(trim_right(bottom)?);

    Ok(lines)
}

fn validate_message_direction(message: &SequenceMessage) -> Result<()> {
    let valid = match message.direction {
        // Mermaid's SOLID_OPEN/DOTTED_OPEN line types are authored forward signals with no
        // endpoint marker, so forward direction cannot require a target marker.
        SequenceMessageDirection::Forward => message.source_marker == SequenceArrowHead::None,
        SequenceMessageDirection::Reverse => {
            message.source_marker != SequenceArrowHead::None
                && message.target_marker == SequenceArrowHead::None
        }
        SequenceMessageDirection::Bidirectional => {
            message.source_marker != SequenceArrowHead::None
                && message.target_marker != SequenceArrowHead::None
        }
    };
    if valid {
        Ok(())
    } else {
        Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "message marker direction",
        })
    }
}

fn paint_endpoint_marker(
    line: &mut SequenceLine,
    x: usize,
    marker: SequenceArrowHead,
    points_right: bool,
    endpoint_destroyed: bool,
    style: char,
    chars: &SequenceChars,
) -> Result<()> {
    if endpoint_destroyed && marker == SequenceArrowHead::Cross {
        line.try_set_role(x, style, AsciiColorRole::EdgeLine)?;
        return Ok(());
    }

    let glyph = if points_right {
        chars.arrow_right(marker)
    } else {
        chars.arrow_left(marker)
    };
    if let Some(glyph) = glyph {
        line.try_set_role(x, glyph.tip, AsciiColorRole::EdgeArrow)?;
        if let Some(stem) = glyph.lineward_stem {
            let stem_x = if points_right {
                x.checked_sub(1).ok_or_else(invalid_message_geometry)?
            } else {
                x.checked_add(1).ok_or_else(invalid_message_geometry)?
            };
            line.try_set_role(stem_x, stem, AsciiColorRole::EdgeArrow)?;
        }
    }
    Ok(())
}

fn paint_central_decorations(
    line: &mut SequenceLine,
    message: &SequenceMessage,
    source_x: usize,
    target_x: usize,
    destroyed_actors: &[usize],
    chars: &SequenceChars,
) -> Result<()> {
    if has_source_central_decoration(message.central_decoration)
        && !destroyed_actors.contains(&message.from)
    {
        line.try_set_role(
            source_x,
            chars.central_decoration(),
            AsciiColorRole::EdgeArrow,
        )?;
    }
    if has_target_central_decoration(message.central_decoration)
        && !destroyed_actors.contains(&message.to)
    {
        line.try_set_role(
            target_x,
            chars.central_decoration(),
            AsciiColorRole::EdgeArrow,
        )?;
    }
    Ok(())
}

fn has_source_central_decoration(decoration: SequenceCentralDecoration) -> bool {
    matches!(
        decoration,
        SequenceCentralDecoration::Source | SequenceCentralDecoration::Both
    )
}

fn has_target_central_decoration(decoration: SequenceCentralDecoration) -> bool {
    matches!(
        decoration,
        SequenceCentralDecoration::Target | SequenceCentralDecoration::Both
    )
}

fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}
