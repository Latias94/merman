use super::model::AsciiSequenceDiagram;
use super::{BOX_BORDER_WIDTH, BOX_PADDING_LEFT_RIGHT, MIN_BOX_WIDTH};
use crate::options::AsciiRenderOptions;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SequenceLayout {
    pub(super) participant_widths: Vec<usize>,
    pub(super) participant_centers: Vec<usize>,
    pub(super) total_width: usize,
    pub(super) message_spacing: usize,
    pub(super) self_message_width: usize,
}

pub(super) fn calculate_layout(
    diagram: &AsciiSequenceDiagram,
    options: &AsciiRenderOptions,
) -> SequenceLayout {
    let participant_widths = diagram
        .participants
        .iter()
        .map(|participant| (participant.label.width() + BOX_PADDING_LEFT_RIGHT).max(MIN_BOX_WIDTH))
        .collect::<Vec<_>>();

    let mut participant_centers = Vec::with_capacity(diagram.participants.len());
    let mut current_x = 0;
    for (index, width) in participant_widths.iter().enumerate() {
        let box_width = width + BOX_BORDER_WIDTH;
        if index == 0 {
            participant_centers.push(box_width / 2);
            current_x = box_width;
        } else {
            current_x += options.sequence_participant_spacing;
            participant_centers.push(current_x + box_width / 2);
            current_x += box_width;
        }
    }

    let last = participant_widths.len() - 1;
    let total_width = participant_centers[last] + (participant_widths[last] + BOX_BORDER_WIDTH) / 2;

    SequenceLayout {
        participant_widths,
        participant_centers,
        total_width,
        message_spacing: options.sequence_message_spacing.max(1),
        self_message_width: options.sequence_self_message_width,
    }
}

pub(super) fn initial_visible_actors(diagram: &AsciiSequenceDiagram) -> Vec<bool> {
    diagram
        .lifecycles
        .iter()
        .map(|lifecycle| lifecycle.created_at.is_none())
        .collect()
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
) -> Vec<usize> {
    diagram
        .lifecycles
        .iter()
        .enumerate()
        .filter_map(|(actor, lifecycle)| {
            let target = match edge {
                LifecycleEdge::Created => lifecycle.created_at,
                LifecycleEdge::Destroyed => lifecycle.destroyed_at,
            };
            (target == Some(model_index)).then_some(actor)
        })
        .collect()
}

pub(super) fn participant_left(layout: &SequenceLayout, index: usize) -> usize {
    let box_width = layout.participant_widths[index] + BOX_BORDER_WIDTH;
    layout.participant_centers[index] - box_width / 2
}
