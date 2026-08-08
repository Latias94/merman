use super::model::AsciiSequenceDiagram;
use super::text::charge_text_work;
use super::{BOX_BORDER_WIDTH, BOX_PADDING_LEFT_RIGHT, MIN_BOX_WIDTH};
use crate::error::{AsciiError, Result};
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};

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
) -> Result<SequenceLayout> {
    let mut resources = ResourceContext::new(options.resources);
    calculate_layout_with_resources(diagram, options, &mut resources)
}

pub(super) fn calculate_layout_with_resources(
    diagram: &AsciiSequenceDiagram,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<SequenceLayout> {
    charge_work_product(resources, diagram.participants.len(), 2)?;
    resources.grid_extent(diagram.participants.len(), 1)?;
    for participant in &diagram.participants {
        for line in participant.label.lines() {
            charge_text_work(line, options.terminal_width_profile, resources)?;
        }
    }

    let mut participant_widths = Vec::new();
    participant_widths
        .try_reserve_exact(diagram.participants.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::Layout.as_str(),
        })?;
    for participant in &diagram.participants {
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
        let box_width = resources.checked_grid_add(*width, BOX_BORDER_WIDTH)?;
        if index == 0 {
            participant_centers.push(box_width / 2);
            current_x = box_width;
        } else {
            current_x =
                resources.checked_grid_add(current_x, options.sequence_participant_spacing)?;
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
        message_spacing: options.sequence_message_spacing.max(1),
        self_message_width: options.sequence_self_message_width,
        width_profile: options.terminal_width_profile,
    })
}

fn charge_work_product(resources: &mut ResourceContext, left: usize, right: usize) -> Result<()> {
    resources.charge_layout_work_product(left, right)
}

pub(super) fn initial_visible_actors(
    diagram: &AsciiSequenceDiagram,
    resources: &ResourceContext,
) -> Result<Vec<bool>> {
    resources.grid_extent(diagram.lifecycles.len(), 1)?;
    let mut visible = Vec::new();
    visible
        .try_reserve_exact(diagram.lifecycles.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    for lifecycle in &diagram.lifecycles {
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
) -> Result<Vec<usize>> {
    resources.grid_extent(diagram.lifecycles.len(), 1)?;
    let mut actors = Vec::new();
    actors
        .try_reserve_exact(diagram.lifecycles.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
        })?;
    for (actor, lifecycle) in diagram.lifecycles.iter().enumerate() {
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
