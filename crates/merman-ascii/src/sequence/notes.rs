use super::{
    BOX_BORDER_WIDTH, BOX_PADDING_LEFT_RIGHT, MIN_BOX_WIDTH, SequenceActorRenderState,
    SequenceCheckpointCursor, try_plan_sequence_label,
};
use crate::color::AsciiColorRole;
use crate::error::{AsciiError, Result};
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};
use crate::safe_text::{LabelBreakPolicy, NormalizedLabelPlan};
use crate::text::display_width_with_profile;

use super::chars::SequenceChars;
use super::layout::SequenceLayout;
use super::lifeline::{render_overlay_row, retained_lifeline_width};
use super::model::{AsciiSequenceDiagram, SequenceEvent, SequenceNote, SequenceNotePlacement};
use super::text::{
    SequenceBatchExtent, SequenceFootprintRun, SequenceLine, SequenceRowFootprint,
    blank_line_with_checkpoints,
};

#[derive(Debug)]
pub(super) struct PreparedNoteRows {
    label_plan: NormalizedLabelPlan,
    inner_width: usize,
    left: usize,
    extent: SequenceBatchExtent,
    footprints: SequenceFootprintRun,
}

impl PreparedNoteRows {
    pub(super) const fn extent(&self) -> SequenceBatchExtent {
        self.extent
    }

    pub(super) const fn materialization_work_units(&self) -> usize {
        self.label_plan.materialization_work_units()
    }

    pub(super) fn append_footprints(
        &self,
        footprints: &mut Vec<SequenceRowFootprint>,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<()> {
        self.footprints.append_to(footprints, checkpoints)
    }
}

pub(super) fn ensure_note_actors_known(note: &SequenceNote, layout: &SequenceLayout) -> Result<()> {
    if layout.participant_centers.get(note.from).is_some()
        && layout.participant_centers.get(note.to).is_some()
    {
        return Ok(());
    }

    Err(AsciiError::UnsupportedFeature {
        diagram_type: "sequence",
        feature: "note actors",
    })
}

pub(super) fn apply_note_gutters(
    diagram: &AsciiSequenceDiagram,
    layout: &mut SequenceLayout,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<()> {
    let mut left_gutter = 0;
    diagram
        .body
        .try_for_each_event(checkpoints, |event, checkpoints| {
            let SequenceEvent::Note(note) = event else {
                return Ok(());
            };
            checkpoints.before_charge()?;
            resources.charge_layout_work(1)?;
            let from = layout
                .participant_centers
                .get(note.from)
                .copied()
                .ok_or_else(invalid_note_geometry)?;
            layout
                .participant_centers
                .get(note.to)
                .ok_or_else(invalid_note_geometry)?;
            let inner_width = note_inner_width(note, layout, resources, checkpoints)?;
            let note_width = resources.checked_grid_add(inner_width, BOX_BORDER_WIDTH)?;
            let required_anchor_offset = match note.placement {
                SequenceNotePlacement::LeftOf => {
                    resources.checked_grid_add(note_width, layout.policy.note_side_gutter)?
                }
                SequenceNotePlacement::Over if note.from == note.to => note_width / 2,
                SequenceNotePlacement::Over => 1,
                SequenceNotePlacement::RightOf => 0,
            };
            if required_anchor_offset > from {
                left_gutter = left_gutter.max(required_anchor_offset - from);
            }
            Ok(())
        })?;

    if left_gutter == 0 {
        return Ok(());
    }

    checkpoints.before_charge()?;
    resources.charge_layout_work(layout.participant_centers.len())?;
    for center in &mut layout.participant_centers {
        checkpoints.tick()?;
        *center = resources.checked_grid_add(*center, left_gutter)?;
    }
    layout.total_width = resources.checked_grid_add(layout.total_width, left_gutter)?;
    resources.grid_extent(resources.checked_grid_add(layout.total_width, 1)?, 1)?;
    Ok(())
}

pub(super) fn prepare_note_rows(
    note: &SequenceNote,
    layout: &SequenceLayout,
    visible_actors: &[bool],
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<PreparedNoteRows> {
    let transaction = resources.clone();
    transaction.transaction(|_| {
        prepare_note_rows_transactional(note, layout, visible_actors, resources, checkpoints)
    })
}

fn prepare_note_rows_transactional(
    note: &SequenceNote,
    layout: &SequenceLayout,
    visible_actors: &[bool],
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<PreparedNoteRows> {
    let label_plan = note_label_plan(note, layout, resources, checkpoints)?;
    checkpoints.before_charge()?;
    label_plan.check_materialization_limits(resources)?;
    let label_metrics = label_plan.metrics();
    let mut inner_width = resources
        .checked_grid_add(label_metrics.max_width, BOX_PADDING_LEFT_RIGHT)?
        .max(MIN_BOX_WIDTH);
    let from = layout
        .participant_centers
        .get(note.from)
        .copied()
        .ok_or_else(invalid_note_geometry)?;
    let to = layout
        .participant_centers
        .get(note.to)
        .copied()
        .ok_or_else(invalid_note_geometry)?;

    let left = match note.placement {
        SequenceNotePlacement::LeftOf => {
            let total_width = resources.checked_grid_add(inner_width, BOX_BORDER_WIDTH)?;
            from.checked_sub(
                resources.checked_grid_add(total_width, layout.policy.note_side_gutter)?,
            )
            .ok_or_else(invalid_note_geometry)?
        }
        SequenceNotePlacement::RightOf => {
            resources.checked_grid_add(from, layout.policy.note_side_gutter)?
        }
        SequenceNotePlacement::Over => {
            if from == to {
                let total_width = resources.checked_grid_add(inner_width, BOX_BORDER_WIDTH)?;
                from.checked_sub(total_width / 2)
                    .ok_or_else(invalid_note_geometry)?
            } else {
                let span_left = from
                    .min(to)
                    .checked_sub(1)
                    .ok_or_else(invalid_note_geometry)?;
                let span_inner_width = resources.checked_grid_add(from.abs_diff(to), 1)?;
                inner_width = inner_width.max(span_inner_width);
                span_left
            }
        }
    };

    let row_count = resources.checked_grid_add(label_metrics.line_count, 2)?;
    let note_width = resources.checked_grid_add(inner_width, BOX_BORDER_WIDTH)?;
    let overlay_right = resources.checked_grid_add(left, note_width)?;
    let max_width = resources
        .checked_grid_add(layout.total_width, 1)?
        .max(overlay_right);
    checkpoints.before_charge()?;
    resources.grid_extent(max_width, row_count)?;
    checkpoints.before_charge()?;
    charge_note_work(resources, max_width, row_count)?;

    let retained_width =
        retained_lifeline_width(layout, visible_actors, resources, checkpoints)?.max(overlay_right);
    let extent = SequenceBatchExtent::uniform(row_count, max_width, retained_width, resources)?;
    let content_right = overlay_right
        .checked_sub(1)
        .ok_or_else(invalid_note_geometry)?;
    let footprint = SequenceRowFootprint::with_content(retained_width, left, content_right)?;
    Ok(PreparedNoteRows {
        label_plan,
        inner_width,
        left,
        extent,
        footprints: SequenceFootprintRun::new(footprint, row_count),
    })
}

pub(super) fn render_note(
    prepared: PreparedNoteRows,
    note: &SequenceNote,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    actor_state: SequenceActorRenderState<'_>,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<Vec<SequenceLine>> {
    let PreparedNoteRows {
        label_plan,
        inner_width,
        left,
        extent,
        footprints: _,
    } = prepared;
    checkpoints.checkpoint()?;
    let materialized = label_plan
        .materialize_after_admission_with_checkpoint(&note.label, || checkpoints.checkpoint());
    checkpoints.checkpoint()?;
    let label_lines = materialized?.into_parts().0;
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
        layout.policy.terminal_width_profile,
        resources,
        checkpoints,
    )?);
    for line in label_lines {
        checkpoints.tick()?;
        let line_width = display_width_with_profile(&line, layout.policy.terminal_width_profile);
        let left_padding = inner_width
            .checked_sub(line_width)
            .ok_or_else(invalid_note_geometry)?
            / 2;
        let mut row = blank_line_with_checkpoints(
            note_width,
            layout.policy.terminal_width_profile,
            resources,
            checkpoints,
        )?;
        row.try_set_role(0, chars.vertical, AsciiColorRole::SequenceFrame)?;
        row.try_write_text_role_with_checkpoint(
            resources.checked_grid_add(1, left_padding)?,
            &line,
            AsciiColorRole::Text,
            resources,
            || checkpoints.tick(),
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
        layout.policy.terminal_width_profile,
        resources,
        checkpoints,
    )?);

    let mut rendered = Vec::new();
    rendered
        .try_reserve_exact(row_count)
        .map_err(|_| allocation_failed())?;
    for row in rows {
        checkpoints.tick()?;
        rendered.push(render_overlay_row(
            layout,
            chars,
            actor_state.active_counts,
            actor_state.visible_actors,
            left,
            &row,
            resources,
            checkpoints,
        )?);
    }
    Ok(rendered)
}

fn charge_note_work(resources: &mut ResourceContext, width: usize, height: usize) -> Result<()> {
    let work = resources.checked_work_mul(width, height)?;
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
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceLine> {
    let total_width = resources.checked_grid_add(inner_width, BOX_BORDER_WIDTH)?;
    let mut row = blank_line_with_checkpoints(total_width, width_profile, resources, checkpoints)?;
    row.try_set_role(0, left, AsciiColorRole::SequenceFrame)?;
    for x in 1..=inner_width {
        checkpoints.tick()?;
        row.try_set_role(x, horizontal, AsciiColorRole::SequenceFrame)?;
    }
    row.try_set_role(
        resources.checked_grid_add(inner_width, 1)?,
        right,
        AsciiColorRole::SequenceFrame,
    )?;
    Ok(row)
}

fn note_inner_width(
    note: &SequenceNote,
    layout: &SequenceLayout,
    resources: &ResourceContext,
    checkpoints: &SequenceCheckpointCursor<'_>,
) -> Result<usize> {
    let label_width = note_label_plan(note, layout, resources, checkpoints)?
        .metrics()
        .max_width;
    let mut inner_width = resources
        .checked_grid_add(label_width, BOX_PADDING_LEFT_RIGHT)?
        .max(MIN_BOX_WIDTH);
    if note.placement == SequenceNotePlacement::Over && note.from != note.to {
        let from = layout
            .participant_centers
            .get(note.from)
            .copied()
            .ok_or_else(invalid_note_geometry)?;
        let to = layout
            .participant_centers
            .get(note.to)
            .copied()
            .ok_or_else(invalid_note_geometry)?;
        inner_width = inner_width.max(resources.checked_grid_add(from.abs_diff(to), 1)?);
    }
    Ok(inner_width)
}

fn note_label_plan(
    note: &SequenceNote,
    layout: &SequenceLayout,
    resources: &ResourceContext,
    checkpoints: &SequenceCheckpointCursor<'_>,
) -> Result<NormalizedLabelPlan> {
    let wrap_width = if note.wrap {
        let from = layout
            .participant_centers
            .get(note.from)
            .copied()
            .ok_or_else(invalid_note_geometry)?;
        let to = layout
            .participant_centers
            .get(note.to)
            .copied()
            .ok_or_else(invalid_note_geometry)?;
        Some(from.abs_diff(to).max(layout.policy.note_wrap_width))
    } else {
        None
    };
    try_plan_sequence_label(
        &note.label,
        layout.policy.terminal_width_profile,
        false,
        wrap_width,
        LabelBreakPolicy::MermaidLabelBreaks,
        resources,
        checkpoints,
    )
    .and_then(|plan| plan.ok_or_else(invalid_note_geometry))
}
