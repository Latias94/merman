use super::{
    LABEL_BUFFER_SPACE, LABEL_LEFT_MARGIN, SequenceCheckpointCursor, try_plan_sequence_label,
};
use crate::color::AsciiColorRole;
use crate::error::{AsciiError, Result};
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};
use crate::safe_text::{LabelBreakPolicy, NormalizedLabelPlan};
use crate::text::display_width_with_profile;

use super::layout::SequenceLayout;
use super::model::{
    SequenceArrowHead, SequenceCentralDecoration, SequenceLineStyle, SequenceMessage,
    SequenceMessageDirection,
};
use super::render::{
    SequenceChars, build_lifeline_line, lifeline_char, lifeline_role, retained_lifeline_width,
};
use super::text::{
    SequenceBatchExtent, SequenceLine, SequenceRowFootprint, padded_line_with_checkpoints,
    trim_right, validate_batch_footprints_with_checkpoints, write_text_role,
};

#[derive(Debug)]
pub(super) struct PreparedMessageRows {
    label_plan: Option<NormalizedLabelPlan>,
    extent: SequenceBatchExtent,
    footprints: Vec<SequenceRowFootprint>,
}

#[derive(Debug)]
pub(super) struct PreparedSelfMessageRows {
    label_plan: Option<NormalizedLabelPlan>,
    extent: SequenceBatchExtent,
    geometry: SelfMessageGeometry,
    footprints: Vec<SequenceRowFootprint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SelfMessageGeometry {
    width: usize,
    loop_right: usize,
    loop_needed: usize,
    arrow_x: usize,
    materialized_width: usize,
}

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

impl PreparedMessageRows {
    pub(super) const fn extent(&self) -> SequenceBatchExtent {
        self.extent
    }

    pub(super) fn take_footprints(&mut self) -> Vec<SequenceRowFootprint> {
        std::mem::take(&mut self.footprints)
    }

    pub(super) fn materialization_work_units(&self) -> usize {
        self.label_plan
            .map_or(0, NormalizedLabelPlan::materialization_work_units)
    }

    #[cfg(test)]
    pub(super) fn materialize_label_with_probe(
        &self,
        raw: &str,
        resources: &ResourceContext,
        materialized: &std::cell::Cell<bool>,
    ) -> Result<()> {
        if let Some(plan) = self.label_plan {
            plan.materialize_with_probe(raw, resources, materialized)?;
        }
        Ok(())
    }
}

impl PreparedSelfMessageRows {
    pub(super) const fn extent(&self) -> SequenceBatchExtent {
        self.extent
    }

    pub(super) fn take_footprints(&mut self) -> Vec<SequenceRowFootprint> {
        std::mem::take(&mut self.footprints)
    }

    pub(super) fn materialization_work_units(&self) -> usize {
        self.label_plan
            .map_or(0, NormalizedLabelPlan::materialization_work_units)
    }
}

impl SelfMessageGeometry {
    fn try_new(
        message: &SequenceMessage,
        layout: &SequenceLayout,
        chars: &SequenceChars,
        resources: &ResourceContext,
    ) -> Result<Self> {
        let center = layout.participant_centers[message.from];
        let width = effective_self_message_width(message, layout, chars);
        let loop_right_offset = width.checked_sub(1).ok_or_else(invalid_message_geometry)?;
        let loop_right = resources.checked_grid_add(center, loop_right_offset)?;
        Ok(Self {
            width,
            loop_right,
            loop_needed: resources.checked_grid_add(loop_right, 1)?,
            arrow_x: resources.checked_grid_add(center, 1)?,
            materialized_width: resources
                .checked_grid_add(resources.checked_grid_add(layout.total_width, width)?, 1)?,
        })
    }

    fn pad_line(
        self,
        line: SequenceLine,
        needed: usize,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<SequenceLine> {
        padded_line_with_checkpoints(line, self.materialized_width.max(needed), checkpoints)
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

fn message_label_plan(
    message: &SequenceMessage,
    max_width: usize,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
    checkpoints: &SequenceCheckpointCursor<'_>,
) -> Result<Option<NormalizedLabelPlan>> {
    if message.label.is_empty() {
        return Ok(None);
    }

    let (wrap_width, break_policy) = if message.wrap {
        (Some(max_width), LabelBreakPolicy::StructuralParagraphs)
    } else {
        (None, LabelBreakPolicy::VisibleLine)
    };
    try_plan_sequence_label(
        &message.label,
        width_profile,
        false,
        wrap_width,
        break_policy,
        resources,
        checkpoints,
    )
}

pub(super) fn prepare_message_rows(
    message: &SequenceMessage,
    layout: &SequenceLayout,
    visible_actors: &[bool],
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<PreparedMessageRows> {
    let transaction = resources.clone();
    transaction.transaction(|_| {
        prepare_message_rows_transactional(message, layout, visible_actors, resources, checkpoints)
    })
}

fn prepare_message_rows_transactional(
    message: &SequenceMessage,
    layout: &SequenceLayout,
    visible_actors: &[bool],
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<PreparedMessageRows> {
    let from = layout.participant_centers[message.from];
    let to = layout.participant_centers[message.to];
    let label_plan = message_label_plan(
        message,
        from.abs_diff(to).saturating_sub(LABEL_LEFT_MARGIN),
        layout.width_profile,
        resources,
        checkpoints,
    )?;
    if let Some(plan) = label_plan {
        checkpoints.before_charge()?;
        plan.check_materialization_limits(resources)?;
    }
    let label_metrics = label_plan.map(NormalizedLabelPlan::metrics);
    let row_count =
        resources.checked_grid_add(label_metrics.map_or(0, |metrics| metrics.line_count), 1)?;
    let start = resources.checked_grid_add(from.min(to), LABEL_LEFT_MARGIN)?;
    let mut max_width = resources.checked_grid_add(layout.total_width, 1)?;
    if let Some(metrics) = label_metrics {
        let label_right = resources.checked_grid_add(start, metrics.max_width)?;
        let label_width =
            resources.checked_grid_add(layout.total_width.max(label_right), LABEL_BUFFER_SPACE)?;
        max_width = max_width.max(label_width);
    }
    checkpoints.before_charge()?;
    resources.grid_extent(max_width, row_count)?;
    checkpoints.before_charge()?;
    charge_row_work(resources, max_width, row_count)?;

    let lifeline_width = retained_lifeline_width(layout, visible_actors, resources, checkpoints)?;
    let mut extent = SequenceBatchExtent::with_materialized_width(max_width);
    let mut footprints = Vec::new();
    footprints
        .try_reserve_exact(row_count)
        .map_err(|_| allocation_failed())?;
    if let Some(plan) = label_plan {
        let visited = plan.try_visit_row_metrics_with_checkpoint(
            &message.label,
            resources,
            || checkpoints.checkpoint(),
            |row| {
                let label_right = resources.checked_grid_add(start, row.retained_width)?;
                let retained_width = lifeline_width.max(label_right);
                extent.try_push_line_length(retained_width, resources)?;
                footprints.push(if row.retained_width == 0 {
                    SequenceRowFootprint::lifeline(retained_width)
                } else {
                    SequenceRowFootprint::with_content(
                        retained_width,
                        start,
                        label_right
                            .checked_sub(1)
                            .ok_or_else(invalid_message_geometry)?,
                    )?
                });
                Ok(())
            },
        );
        checkpoints.before_charge()?;
        visited?;
    }
    extent.try_push_line_length(lifeline_width, resources)?;
    footprints.push(SequenceRowFootprint::with_content(
        lifeline_width,
        from.min(to),
        from.max(to),
    )?);
    validate_batch_footprints_with_checkpoints(extent, &footprints, resources, checkpoints)?;

    Ok(PreparedMessageRows {
        label_plan,
        extent,
        footprints,
    })
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
    let PreparedMessageRows {
        label_plan,
        extent,
        footprints: _,
    } = prepared;
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
    let start = resources.checked_grid_add(from.min(to), LABEL_LEFT_MARGIN)?;

    let mut lines = Vec::new();
    lines
        .try_reserve_exact(row_count)
        .map_err(|_| allocation_failed())?;

    for label in label_lines {
        checkpoints.tick()?;
        let label_width = display_width_with_profile(&label, layout.width_profile);
        let label_right = resources.checked_grid_add(start, label_width)?;
        let width =
            resources.checked_grid_add(layout.total_width.max(label_right), LABEL_BUFFER_SPACE)?;
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

pub(super) fn prepare_self_message_rows(
    message: &SequenceMessage,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    visible_actors: &[bool],
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<PreparedSelfMessageRows> {
    let transaction = resources.clone();
    transaction.transaction(|_| {
        prepare_self_message_rows_transactional(
            message,
            layout,
            chars,
            visible_actors,
            resources,
            checkpoints,
        )
    })
}

fn prepare_self_message_rows_transactional(
    message: &SequenceMessage,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    visible_actors: &[bool],
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<PreparedSelfMessageRows> {
    let center = layout.participant_centers[message.from];
    let geometry = SelfMessageGeometry::try_new(message, layout, chars, resources)?;
    let label_wrap_width = resources.checked_grid_add(geometry.width, LABEL_BUFFER_SPACE)?;
    let label_plan = message_label_plan(
        message,
        label_wrap_width,
        layout.width_profile,
        resources,
        checkpoints,
    )?;
    if let Some(plan) = label_plan {
        checkpoints.before_charge()?;
        plan.check_materialization_limits(resources)?;
    }
    let label_metrics = label_plan.map(NormalizedLabelPlan::metrics);
    let row_count =
        resources.checked_grid_add(label_metrics.map_or(0, |metrics| metrics.line_count), 3)?;
    let start = resources.checked_grid_add(center, LABEL_LEFT_MARGIN)?;
    let mut max_width = geometry.materialized_width;
    if let Some(metrics) = label_metrics {
        let label_right = resources.checked_grid_add(start, metrics.max_width)?;
        max_width = max_width.max(resources.checked_grid_add(label_right, LABEL_BUFFER_SPACE)?);
    }
    checkpoints.before_charge()?;
    resources.grid_extent(max_width, row_count)?;
    checkpoints.before_charge()?;
    charge_row_work(resources, max_width, row_count)?;

    let lifeline_width = retained_lifeline_width(layout, visible_actors, resources, checkpoints)?;
    let message_row_width = lifeline_width.max(geometry.loop_needed);
    let mut extent = SequenceBatchExtent::with_materialized_width(max_width);
    let mut footprints = Vec::new();
    footprints
        .try_reserve_exact(row_count)
        .map_err(|_| allocation_failed())?;
    if let Some(plan) = label_plan {
        let visited = plan.try_visit_row_metrics_with_checkpoint(
            &message.label,
            resources,
            || checkpoints.checkpoint(),
            |row| {
                let label_right = resources.checked_grid_add(start, row.retained_width)?;
                let retained_width = lifeline_width.max(label_right);
                extent.try_push_line_length(retained_width, resources)?;
                footprints.push(if row.retained_width == 0 {
                    SequenceRowFootprint::lifeline(retained_width)
                } else {
                    SequenceRowFootprint::with_content(
                        retained_width,
                        start,
                        label_right
                            .checked_sub(1)
                            .ok_or_else(invalid_message_geometry)?,
                    )?
                });
                Ok(())
            },
        );
        checkpoints.before_charge()?;
        visited?;
    }
    for _ in 0..3 {
        checkpoints.tick()?;
        extent.try_push_line_length(message_row_width, resources)?;
        footprints.push(SequenceRowFootprint::with_content(
            message_row_width,
            center,
            geometry.loop_right,
        )?);
    }
    validate_batch_footprints_with_checkpoints(extent, &footprints, resources, checkpoints)?;

    Ok(PreparedSelfMessageRows {
        label_plan,
        extent,
        geometry,
        footprints,
    })
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
    let PreparedSelfMessageRows {
        label_plan,
        extent,
        geometry,
        footprints: _,
    } = prepared;
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
    let start = resources.checked_grid_add(center, LABEL_LEFT_MARGIN)?;

    let mut lines = Vec::new();
    lines
        .try_reserve_exact(row_count)
        .map_err(|_| allocation_failed())?;

    for label in label_lines {
        checkpoints.tick()?;
        let label_right = resources.checked_grid_add(
            start,
            display_width_with_profile(&label, layout.width_profile),
        )?;
        let needed = resources.checked_grid_add(label_right, LABEL_BUFFER_SPACE)?;
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

fn effective_self_message_width(
    message: &SequenceMessage,
    layout: &SequenceLayout,
    chars: &SequenceChars,
) -> usize {
    let has_filled_half_stem = [message.source_marker, message.target_marker]
        .into_iter()
        .filter_map(|marker| chars.arrow_left(marker))
        .any(|glyph| glyph.lineward_stem.is_some());
    if has_filled_half_stem {
        layout.self_message_width.max(4)
    } else {
        layout.self_message_width
    }
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

fn charge_row_work(resources: &mut ResourceContext, width: usize, height: usize) -> Result<()> {
    let work = resources.checked_work_mul(width, height)?;
    resources.charge_layout_work(work)
}

fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

fn invalid_message_geometry() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "sequence",
        feature: "message geometry",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::AsciiExecution;
    use crate::options::AsciiRenderOptions;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use crate::sequence::text::blank_line;
    use merman_core::OperationPhase;

    fn narrow_self_layout() -> SequenceLayout {
        SequenceLayout {
            participant_widths: vec![3],
            participant_centers: vec![2],
            total_width: 5,
            message_spacing: 5,
            self_message_width: 2,
            width_profile: TerminalWidthProfile::Unicode,
        }
    }

    fn filled_half_self_message() -> SequenceMessage {
        SequenceMessage {
            model_index: 0,
            from: 0,
            to: 0,
            label: String::new(),
            wrap: false,
            style: SequenceLineStyle::Solid,
            source_marker: SequenceArrowHead::None,
            target_marker: SequenceArrowHead::FilledHalfTop,
            direction: SequenceMessageDirection::Forward,
            central_decoration: SequenceCentralDecoration::None,
        }
    }

    #[test]
    fn narrow_filled_half_self_message_uses_one_exact_geometry_for_admission_and_paint() {
        let layout = narrow_self_layout();
        let message = filled_half_self_message();
        let options = AsciiRenderOptions::ascii();
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, 30)
            .unwrap();
        let chars = SequenceChars::for_options(&options);
        let mut resources = ResourceContext::new(policy);
        let mut checkpoints = SequenceCheckpointCursor::new(
            AsciiExecution::standalone(&policy),
            OperationPhase::Layout,
        );
        let prepared = prepare_self_message_rows(
            &message,
            &layout,
            &chars,
            &[true],
            &mut resources,
            &mut checkpoints,
        )
        .expect("the exact 10x3 self-message extent should be admitted");

        assert_eq!(prepared.extent().materialized_width(), 10);
        assert_eq!(prepared.extent().height(), 3);
        assert_eq!(prepared.geometry.materialized_width, 10);
        let padded = prepared
            .geometry
            .pad_line(
                blank_line(6, layout.width_profile, &resources).unwrap(),
                prepared.geometry.loop_needed,
                &mut checkpoints,
            )
            .unwrap();
        assert_eq!(padded.len(), prepared.geometry.materialized_width);

        let lines = render_self_message(
            prepared,
            &message,
            &layout,
            &chars,
            MessageActorState::new(&[0], &[true], &[]),
            &mut resources,
            &mut checkpoints,
        )
        .expect("the admitted self-message should paint from the same geometry");
        assert_eq!(lines.len(), 3);
        assert!(lines.iter().any(|line| line.text().contains("/|")));

        let below = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, 29)
            .unwrap();
        let mut resources = ResourceContext::new(below);
        let mut checkpoints = SequenceCheckpointCursor::new(
            AsciiExecution::standalone(&below),
            OperationPhase::Layout,
        );
        let error = prepare_self_message_rows(
            &message,
            &layout,
            &SequenceChars::for_options(&options),
            &[true],
            &mut resources,
            &mut checkpoints,
        )
        .expect_err("the 10x3 self-message must reject a 29-cell grid limit");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGridCells
                    && details.actual == 30
                    && details.max == 29
        ));
    }
}
