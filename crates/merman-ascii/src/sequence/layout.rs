use super::SequenceCheckpointCursor;
use super::model::AsciiSequenceDiagram;
use super::{BOX_BORDER_WIDTH, BOX_PADDING_LEFT_RIGHT, MIN_BOX_WIDTH};
use crate::error::{AsciiError, Result};
#[cfg(test)]
use crate::operation::AsciiExecution;
#[cfg(test)]
use crate::options::AsciiRenderOptions;
use crate::options::{SequenceLayoutPolicy, TerminalWidthProfile};
#[cfg(test)]
use crate::resource::AsciiResourcePolicy;
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};
#[cfg(test)]
use merman_core::OperationPhase;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SequenceLayout {
    pub(super) participant_widths: Vec<usize>,
    pub(super) participant_centers: Vec<usize>,
    pub(super) total_width: usize,
    pub(super) message_spacing: usize,
    pub(super) self_message_width: usize,
    pub(super) width_profile: TerminalWidthProfile,
}

#[cfg(test)]
pub(super) fn calculate_layout(
    diagram: &AsciiSequenceDiagram,
    options: &AsciiRenderOptions,
    policy: &AsciiResourcePolicy,
) -> Result<SequenceLayout> {
    let mut resources = ResourceContext::new(*policy);
    let mut checkpoints =
        SequenceCheckpointCursor::new(AsciiExecution::for_test(policy), OperationPhase::Layout);
    calculate_layout_with_resources(diagram, options, &mut resources, &mut checkpoints)
}

#[cfg(test)]
pub(super) fn calculate_layout_with_resources(
    diagram: &AsciiSequenceDiagram,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceLayout> {
    calculate_layout_with_policy(diagram, options.sequence_layout(), resources, checkpoints)
}

pub(super) fn calculate_layout_with_policy(
    diagram: &AsciiSequenceDiagram,
    layout: SequenceLayoutPolicy,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceLayout> {
    checkpoints.before_charge()?;
    charge_work_product(resources, diagram.participants.len(), 2)?;
    resources.grid_extent(diagram.participants.len(), 1)?;

    let mut participant_widths = Vec::new();
    participant_widths
        .try_reserve_exact(diagram.participants.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::Layout.as_str(),
        })?;
    for participant in &diagram.participants {
        checkpoints.tick()?;
        let width = resources
            .checked_grid_add(participant.label.width(), BOX_PADDING_LEFT_RIGHT)?
            .max(MIN_BOX_WIDTH);
        participant_widths.push(width);
    }

    let mut participant_centers = Vec::new();
    participant_centers
        .try_reserve_exact(diagram.participants.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::Layout.as_str(),
        })?;
    let mut current_x = 0;
    for (index, width) in participant_widths.iter().enumerate() {
        checkpoints.tick()?;
        let box_width = resources.checked_grid_add(*width, BOX_BORDER_WIDTH)?;
        if index == 0 {
            participant_centers.push(box_width / 2);
            current_x = box_width;
        } else {
            current_x = resources.checked_grid_add(current_x, layout.participant_spacing)?;
            participant_centers.push(resources.checked_grid_add(current_x, box_width / 2)?);
            current_x = resources.checked_grid_add(current_x, box_width)?;
        }
    }

    let Some((&last_center, &last_width)) =
        participant_centers.last().zip(participant_widths.last())
    else {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "no participants",
        });
    };
    let last_box_width = resources.checked_grid_add(last_width, BOX_BORDER_WIDTH)?;
    let total_width = resources.checked_grid_add(last_center, last_box_width / 2)?;
    resources.grid_extent(resources.checked_grid_add(total_width, 1)?, 1)?;

    Ok(SequenceLayout {
        participant_widths,
        participant_centers,
        total_width,
        message_spacing: layout.message_spacing.max(1),
        self_message_width: layout.self_message_width,
        width_profile: layout.terminal_width_profile,
    })
}

fn charge_work_product(resources: &mut ResourceContext, left: usize, right: usize) -> Result<()> {
    resources.charge_layout_work_product(left, right)
}

pub(super) fn initial_visible_actors(
    diagram: &AsciiSequenceDiagram,
    resources: &ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<Vec<bool>> {
    resources.grid_extent(diagram.lifecycles.len(), 1)?;
    let mut visible = Vec::new();
    visible
        .try_reserve_exact(diagram.lifecycles.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    for lifecycle in &diagram.lifecycles {
        checkpoints.tick()?;
        visible.push(lifecycle.created_at.is_none());
    }
    Ok(visible)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LifecycleEdge {
    Created,
    Destroyed,
}

pub(super) fn lifecycle_actors_at(
    diagram: &AsciiSequenceDiagram,
    model_index: usize,
    edge: LifecycleEdge,
    resources: &ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<Vec<usize>> {
    resources.grid_extent(diagram.lifecycles.len(), 1)?;
    let mut actors = Vec::new();
    actors
        .try_reserve_exact(diagram.lifecycles.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    for (actor, lifecycle) in diagram.lifecycles.iter().enumerate() {
        checkpoints.tick()?;
        let target = match edge {
            LifecycleEdge::Created => lifecycle.created_at,
            LifecycleEdge::Destroyed => lifecycle.destroyed_at,
        };
        if target == Some(model_index) {
            actors.push(actor);
        }
    }
    Ok(actors)
}

pub(super) fn participant_left(
    layout: &SequenceLayout,
    index: usize,
    resources: &ResourceContext,
) -> Result<usize> {
    let width = layout
        .participant_widths
        .get(index)
        .copied()
        .ok_or_else(invalid_participant_geometry)?;
    let center = layout
        .participant_centers
        .get(index)
        .copied()
        .ok_or_else(invalid_participant_geometry)?;
    let box_width = resources.checked_grid_add(width, BOX_BORDER_WIDTH)?;
    center
        .checked_sub(box_width / 2)
        .ok_or_else(invalid_participant_geometry)
}

fn invalid_participant_geometry() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "sequence",
        feature: "participant geometry",
    }
}
