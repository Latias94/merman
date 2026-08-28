use super::{SequenceCheckpointCursor, try_plan_sequence_label};
use crate::error::{AsciiError, Result};
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};
use crate::safe_text::{LabelBreakPolicy, NormalizedLabelPlan};

use super::chars::SequenceChars;
use super::layout::SequenceLayout;
use super::lifeline::retained_lifeline_width;
use super::model::SequenceMessage;
use super::text::{
    SequenceBatchExtent, SequenceLine, SequenceRowFootprint, padded_line_with_checkpoints,
};

#[derive(Debug)]
pub(super) struct PreparedMessageRows {
    label_plan: Option<NormalizedLabelPlan>,
    extent: SequenceBatchExtent,
    label_start: usize,
    lifeline_width: usize,
    message_footprint: SequenceRowFootprint,
}

#[derive(Debug)]
pub(super) struct PreparedSelfMessageRows {
    label_plan: Option<NormalizedLabelPlan>,
    extent: SequenceBatchExtent,
    geometry: SelfMessageGeometry,
    label_start: usize,
    lifeline_width: usize,
    message_footprint: SequenceRowFootprint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SelfMessageGeometry {
    pub(super) width: usize,
    pub(super) loop_right: usize,
    pub(super) loop_needed: usize,
    pub(super) arrow_x: usize,
    pub(super) materialized_width: usize,
}

#[derive(Debug, Clone, Copy)]
struct MessageFootprintPlan {
    label_plan: Option<NormalizedLabelPlan>,
    extent: SequenceBatchExtent,
    label_start: usize,
    lifeline_width: usize,
    message_footprint: SequenceRowFootprint,
    message_rows: usize,
}

impl PreparedMessageRows {
    pub(super) const fn extent(&self) -> SequenceBatchExtent {
        self.extent
    }

    pub(super) fn materialization_work_units(&self) -> usize {
        self.label_plan
            .map_or(0, NormalizedLabelPlan::materialization_work_units)
    }

    pub(super) fn into_render_parts(self) -> (Option<NormalizedLabelPlan>, SequenceBatchExtent) {
        (self.label_plan, self.extent)
    }

    pub(super) fn append_footprints(
        &self,
        raw: &str,
        resources: &ResourceContext,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
        footprints: &mut Vec<SequenceRowFootprint>,
    ) -> Result<()> {
        MessageFootprintPlan {
            label_plan: self.label_plan,
            extent: self.extent,
            label_start: self.label_start,
            lifeline_width: self.lifeline_width,
            message_footprint: self.message_footprint,
            message_rows: 1,
        }
        .append(raw, resources, checkpoints, footprints)
    }
}

impl PreparedSelfMessageRows {
    pub(super) const fn extent(&self) -> SequenceBatchExtent {
        self.extent
    }

    pub(super) fn materialization_work_units(&self) -> usize {
        self.label_plan
            .map_or(0, NormalizedLabelPlan::materialization_work_units)
    }

    pub(super) fn into_render_parts(
        self,
    ) -> (
        Option<NormalizedLabelPlan>,
        SequenceBatchExtent,
        SelfMessageGeometry,
    ) {
        (self.label_plan, self.extent, self.geometry)
    }

    pub(super) fn append_footprints(
        &self,
        raw: &str,
        resources: &ResourceContext,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
        footprints: &mut Vec<SequenceRowFootprint>,
    ) -> Result<()> {
        MessageFootprintPlan {
            label_plan: self.label_plan,
            extent: self.extent,
            label_start: self.label_start,
            lifeline_width: self.lifeline_width,
            message_footprint: self.message_footprint,
            message_rows: 3,
        }
        .append(raw, resources, checkpoints, footprints)
    }
}

impl MessageFootprintPlan {
    fn append(
        self,
        raw: &str,
        resources: &ResourceContext,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
        footprints: &mut Vec<SequenceRowFootprint>,
    ) -> Result<()> {
        footprints
            .try_reserve(self.extent.height())
            .map_err(|_| allocation_failed())?;
        if let Some(plan) = self.label_plan {
            let visited = plan.try_visit_row_metrics_with_checkpoint(
                raw,
                resources,
                || checkpoints.checkpoint(),
                |row| {
                    let label_right =
                        resources.checked_grid_add(self.label_start, row.retained_width)?;
                    let retained_width = self.lifeline_width.max(label_right);
                    footprints.push(if row.retained_width == 0 {
                        SequenceRowFootprint::lifeline(retained_width)
                    } else {
                        SequenceRowFootprint::with_content(
                            retained_width,
                            self.label_start,
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
        for _ in 0..self.message_rows {
            checkpoints.tick()?;
            footprints.push(self.message_footprint);
        }
        Ok(())
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

    pub(super) fn pad_line(
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
        from.abs_diff(to)
            .saturating_sub(layout.policy.message_label_left_margin),
        layout.policy.terminal_width_profile,
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
    let start =
        resources.checked_grid_add(from.min(to), layout.policy.message_label_left_margin)?;
    let mut max_width = resources.checked_grid_add(layout.total_width, 1)?;
    if let Some(metrics) = label_metrics {
        let label_right = resources.checked_grid_add(start, metrics.max_width)?;
        let label_width = resources.checked_grid_add(
            layout.total_width.max(label_right),
            layout.policy.message_label_overflow_buffer,
        )?;
        max_width = max_width.max(label_width);
    }
    checkpoints.before_charge()?;
    resources.grid_extent(max_width, row_count)?;
    checkpoints.before_charge()?;
    charge_row_work(resources, max_width, row_count)?;

    let lifeline_width = retained_lifeline_width(layout, visible_actors, resources, checkpoints)?;
    let mut extent = SequenceBatchExtent::with_materialized_width(max_width);
    if let Some(plan) = label_plan {
        let visited = plan.try_visit_row_metrics_with_checkpoint(
            &message.label,
            resources,
            || checkpoints.checkpoint(),
            |row| {
                let label_right = resources.checked_grid_add(start, row.retained_width)?;
                let retained_width = lifeline_width.max(label_right);
                extent.try_push_line_length(retained_width, resources)?;
                Ok(())
            },
        );
        checkpoints.before_charge()?;
        visited?;
    }
    extent.try_push_line_length(lifeline_width, resources)?;
    let message_footprint =
        SequenceRowFootprint::with_content(lifeline_width, from.min(to), from.max(to))?;

    Ok(PreparedMessageRows {
        label_plan,
        extent,
        label_start: start,
        lifeline_width,
        message_footprint,
    })
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
    let label_wrap_width =
        resources.checked_grid_add(geometry.width, layout.policy.message_label_overflow_buffer)?;
    let label_plan = message_label_plan(
        message,
        label_wrap_width,
        layout.policy.terminal_width_profile,
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
    let start = resources.checked_grid_add(center, layout.policy.message_label_left_margin)?;
    let mut max_width = geometry.materialized_width;
    if let Some(metrics) = label_metrics {
        let label_right = resources.checked_grid_add(start, metrics.max_width)?;
        max_width = max_width.max(
            resources.checked_grid_add(label_right, layout.policy.message_label_overflow_buffer)?,
        );
    }
    checkpoints.before_charge()?;
    resources.grid_extent(max_width, row_count)?;
    checkpoints.before_charge()?;
    charge_row_work(resources, max_width, row_count)?;

    let lifeline_width = retained_lifeline_width(layout, visible_actors, resources, checkpoints)?;
    let message_row_width = lifeline_width.max(geometry.loop_needed);
    let mut extent = SequenceBatchExtent::with_materialized_width(max_width);
    if let Some(plan) = label_plan {
        let visited = plan.try_visit_row_metrics_with_checkpoint(
            &message.label,
            resources,
            || checkpoints.checkpoint(),
            |row| {
                let label_right = resources.checked_grid_add(start, row.retained_width)?;
                let retained_width = lifeline_width.max(label_right);
                extent.try_push_line_length(retained_width, resources)?;
                Ok(())
            },
        );
        checkpoints.before_charge()?;
        visited?;
    }
    for _ in 0..3 {
        checkpoints.tick()?;
        extent.try_push_line_length(message_row_width, resources)?;
    }
    let message_footprint =
        SequenceRowFootprint::with_content(message_row_width, center, geometry.loop_right)?;

    Ok(PreparedSelfMessageRows {
        label_plan,
        extent,
        geometry,
        label_start: start,
        lifeline_width,
        message_footprint,
    })
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
        layout.policy.self_message_width.max(4)
    } else {
        layout.policy.self_message_width
    }
}

fn charge_row_work(resources: &mut ResourceContext, width: usize, height: usize) -> Result<()> {
    let work = resources.checked_work_mul(width, height)?;
    resources.charge_layout_work(work)
}

fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

pub(super) fn invalid_message_geometry() -> AsciiError {
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
    use crate::sequence::event_paint::{MessageActorState, render_self_message};
    use crate::sequence::model::{
        SequenceArrowHead, SequenceCentralDecoration, SequenceLineStyle, SequenceMessageDirection,
    };
    use crate::sequence::text::blank_line;
    use merman_core::OperationPhase;

    fn narrow_self_layout() -> SequenceLayout {
        let mut policy = AsciiRenderOptions::unicode().sequence_layout();
        policy.message_spacing = 5;
        policy.self_message_width = 2;
        SequenceLayout {
            participant_widths: vec![3],
            participant_centers: vec![2],
            total_width: 5,
            policy,
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
            AsciiExecution::for_test(&policy),
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
                blank_line(6, layout.policy.terminal_width_profile, &resources).unwrap(),
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
        let mut checkpoints =
            SequenceCheckpointCursor::new(AsciiExecution::for_test(&below), OperationPhase::Layout);
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
