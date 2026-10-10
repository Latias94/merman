use super::SequenceCheckpointCursor;
use super::chars::SequenceChars;
use super::event_plan::{effective_self_message_width, message_label_plan};
use super::model::{AsciiSequenceDiagram, SequenceEvent, SequenceMessage};
use super::{BOX_BORDER_WIDTH, BOX_PADDING_LEFT_RIGHT, MIN_BOX_WIDTH};
use crate::error::{AsciiError, Result};
#[cfg(test)]
use crate::operation::AsciiExecution;
#[cfg(test)]
use crate::options::AsciiRenderOptions;
use crate::options::SequenceLayoutPolicy;
#[cfg(test)]
use crate::resource::AsciiResourcePolicy;
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};
use crate::safe_text::NormalizedLabelPlan;
#[cfg(test)]
use merman_core::OperationPhase;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub(super) struct SequenceLayout {
    pub(super) participant_widths: Vec<usize>,
    pub(super) participant_centers: Vec<usize>,
    pub(super) total_width: usize,
    pub(super) policy: SequenceLayoutPolicy,
    pub(super) message_labels: HashMap<usize, NormalizedLabelPlan>,
}

/// A label owns one adjacent-lifeline interval, even when its arrow spans several actors.
#[derive(Debug, Clone, Copy)]
pub(super) struct SequenceLabelHost {
    pub(super) start: usize,
    end: usize,
}

impl SequenceLabelHost {
    pub(super) fn width(self) -> usize {
        self.end - self.start
    }

    pub(super) fn verify_width(self, width: usize, resources: &ResourceContext) -> Result<()> {
        if resources.checked_grid_add(self.start, width)? > self.end {
            return Err(invalid_message_label_host());
        }
        Ok(())
    }
}

// One blank terminal cell keeps text separate from the next lifeline.
const MESSAGE_LABEL_RIGHT_CLEARANCE: usize = 1;

impl SequenceLayout {
    pub(super) fn message_label_host(
        &self,
        message: &SequenceMessage,
        resources: &ResourceContext,
    ) -> Result<SequenceLabelHost> {
        let actor = message.from.min(message.to);
        let left = self
            .participant_centers
            .get(actor)
            .copied()
            .ok_or_else(invalid_message_label_host)?;
        let right = self
            .participant_centers
            .get(actor + 1)
            .copied()
            .unwrap_or(self.total_width);
        let start = resources.checked_grid_add(left, self.policy.message_label_left_margin)?;
        let end = right
            .checked_sub(MESSAGE_LABEL_RIGHT_CLEARANCE)
            .filter(|end| *end >= start)
            .ok_or_else(invalid_message_label_host)?;
        Ok(SequenceLabelHost { start, end })
    }
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
    mut policy: SequenceLayoutPolicy,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceLayout> {
    checkpoints.before_charge()?;
    charge_work_product(resources, diagram.participants.len(), 3)?;
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

    let mut interval_widths = Vec::new();
    interval_widths
        .try_reserve_exact(participant_widths.len())
        .map_err(|_| AsciiError::allocation_failed(AsciiResourceLimitPhase::Layout.as_str()))?;
    for (index, width) in participant_widths.iter().enumerate() {
        checkpoints.tick()?;
        let box_width = resources.checked_grid_add(*width, BOX_BORDER_WIDTH)?;
        let distance = if let Some(next) = participant_widths.get(index + 1) {
            let next_box_width = resources.checked_grid_add(*next, BOX_BORDER_WIDTH)?;
            resources.checked_grid_add(
                resources
                    .checked_grid_add(box_width - box_width / 2, policy.participant_spacing)?,
                next_box_width / 2,
            )?
        } else {
            box_width / 2
        };
        interval_widths.push(distance);
    }

    let mut message_labels = HashMap::new();
    let chars = SequenceChars::for_charset(policy.structural_charset);
    diagram
        .body
        .try_for_each_event(checkpoints, |event, checkpoints| {
            checkpoints.before_charge()?;
            resources.charge_layout_work(1)?;
            let SequenceEvent::Message(message) = event else {
                return Ok(());
            };
            if message.from >= participant_widths.len() || message.to >= participant_widths.len() {
                return Err(invalid_message_label_host());
            }
            let actor = message.from.min(message.to);
            let has_next_actor = actor + 1 < participant_widths.len();
            let preferred_label_width = if has_next_actor {
                interval_widths[actor]
                    .saturating_sub(policy.message_label_left_margin)
                    .saturating_sub(MESSAGE_LABEL_RIGHT_CLEARANCE)
            } else {
                resources.checked_grid_add(
                    effective_self_message_width(message, policy, &chars),
                    policy.message_label_overflow_buffer,
                )?
            }
            .max(1);
            let label = message_label_plan(
                message,
                preferred_label_width,
                policy.terminal_width_profile,
                resources,
                checkpoints,
            )?;
            if let Some(plan) = label {
                checkpoints.before_charge()?;
                plan.check_materialization_limits(resources)?;
                let required = resources.checked_grid_add(
                    resources.checked_grid_add(
                        policy.message_label_left_margin,
                        plan.metrics().max_width,
                    )?,
                    MESSAGE_LABEL_RIGHT_CLEARANCE,
                )?;
                interval_widths[actor] = interval_widths[actor].max(required);
                if !message.wrap {
                    // Reuse the exact normalization plan after assigning final actor centers.
                    let count = resources.checked_grid_add(message_labels.len(), 1)?;
                    resources.grid_extent(count, 1)?;
                    resources.charge_layout_work(1)?;
                    message_labels.try_reserve(1).map_err(|_| {
                        AsciiError::allocation_failed(AsciiResourceLimitPhase::Layout.as_str())
                    })?;
                    if message_labels.insert(message.model_index, plan).is_some() {
                        return Err(invalid_message_label_host());
                    }
                }
            }
            if message.from == message.to {
                let loop_width = effective_self_message_width(message, policy, &chars);
                let required =
                    resources.checked_grid_add(loop_width, MESSAGE_LABEL_RIGHT_CLEARANCE)?;
                interval_widths[actor] = interval_widths[actor].max(required);
            }
            Ok(())
        })?;

    let mut participant_centers = Vec::new();
    participant_centers
        .try_reserve_exact(diagram.participants.len())
        .map_err(|_| AsciiError::AllocationFailed {
            phase: AsciiResourceLimitPhase::Layout.as_str(),
        })?;
    for (index, width) in participant_widths.iter().enumerate() {
        checkpoints.tick()?;
        let center = if index == 0 {
            resources.checked_grid_add(*width, BOX_BORDER_WIDTH)? / 2
        } else {
            resources
                .checked_grid_add(participant_centers[index - 1], interval_widths[index - 1])?
        };
        participant_centers.push(center);
    }

    let Some((&last_center, &last_interval)) =
        participant_centers.last().zip(interval_widths.last())
    else {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "no participants",
        });
    };
    let total_width = resources.checked_grid_add(last_center, last_interval)?;
    resources.grid_extent(resources.checked_grid_add(total_width, 1)?, 1)?;

    policy.message_spacing = policy.message_spacing.max(1);
    Ok(SequenceLayout {
        participant_widths,
        participant_centers,
        total_width,
        policy,
        message_labels,
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

fn invalid_message_label_host() -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "sequence",
        feature: "message label host",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::options::TerminalWidthProfile;
    use crate::resource::{AsciiResourceLimitCause, AsciiResourceLimitId};
    use crate::sequence::model::{
        SequenceActorLifecycle, SequenceArrowHead, SequenceCentralDecoration, SequenceLineStyle,
        SequenceMessageDirection, SequenceParticipant, SequenceParticipantLabel,
    };
    use crate::sequence::render::render_sequence_diagram_with_execution;
    use crate::sequence::tree::SequenceTreeBuilder;
    use merman_core::{CancelReason, OperationControl};
    use unicode_segmentation::UnicodeSegmentation;

    const FIRST_LABEL: &str = "abcdefghijklmno";
    const REPLY_LABEL: &str = "reply";
    // Two participant scans, two event admissions, two cache entries, and three units per
    // ASCII label byte: source preflight, source segment, and normalized output segment.
    const EXACT_MESSAGE_PLANNING_WORK: usize = 2 * 3 + 2 + 2 + 3 * (15 + 5);
    // The second label's transaction rolls back, retaining the first plan and the next event.
    const WORK_BEFORE_SECOND_LABEL: usize = 2 * 3 + 1 + 3 * 15 + 1 + 1;
    const SEEDED_WORK: usize = 7;
    const SEEDED_DOCUMENT_CELLS: usize = 11;

    #[test]
    fn early_message_planning_admits_exact_work_and_rejects_limit_minus_one() {
        let diagram = two_message_diagram(REPLY_LABEL);
        let options = AsciiRenderOptions::ascii();
        let exact_policy = AsciiResourcePolicy::default()
            .with_limit(
                AsciiResourceLimitId::MaxLayoutWorkUnits,
                EXACT_MESSAGE_PLANNING_WORK,
            )
            .unwrap();
        let mut exact_resources = ResourceContext::new(exact_policy);
        let mut exact_checkpoints = SequenceCheckpointCursor::new(
            AsciiExecution::for_test(&exact_policy),
            OperationPhase::Layout,
        );

        let layout = calculate_layout_with_resources(
            &diagram,
            &options,
            &mut exact_resources,
            &mut exact_checkpoints,
        )
        .expect("the independently derived early-planning work limit should fit exactly");

        assert_eq!(
            exact_resources.layout_work_used(),
            EXACT_MESSAGE_PLANNING_WORK
        );
        assert_eq!(exact_resources.document_cells_used(), 0);
        assert_eq!(layout.participant_centers, [2, 20]);
        assert_eq!(layout.total_width, 22);
        assert_eq!(layout.message_labels.len(), 2);
        assert_eq!(layout.message_labels[&0].metrics().max_width, 15);
        assert_eq!(layout.message_labels[&1].metrics().max_width, 5);

        let below_policy = exact_policy
            .with_limit(
                AsciiResourceLimitId::MaxLayoutWorkUnits,
                EXACT_MESSAGE_PLANNING_WORK - 1,
            )
            .unwrap();
        let mut below_resources = ResourceContext::new(below_policy);
        let mut below_checkpoints = SequenceCheckpointCursor::new(
            AsciiExecution::for_test(&below_policy),
            OperationPhase::Layout,
        );
        let error = calculate_layout_with_resources(
            &diagram,
            &options,
            &mut below_resources,
            &mut below_checkpoints,
        )
        .expect_err("the second cache admission must reject before assigning actor centers");

        assert_work_ceiling(
            error,
            EXACT_MESSAGE_PLANNING_WORK,
            EXACT_MESSAGE_PLANNING_WORK - 1,
            below_policy,
        );
        assert_eq!(
            below_resources.layout_work_used(),
            EXACT_MESSAGE_PLANNING_WORK - 1
        );
        assert_eq!(below_resources.document_cells_used(), 0);
    }

    #[test]
    fn early_message_planning_work_rejection_rolls_back_the_render_ledger() {
        let diagram = two_message_diagram(REPLY_LABEL);
        let original_diagram = diagram.clone();
        let maximum = SEEDED_WORK + EXACT_MESSAGE_PLANNING_WORK - 1;
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, maximum)
            .unwrap();
        let mut resources = seeded_resources(policy);
        let control = OperationControl::new();
        let error = render_sequence_diagram_with_execution(
            &diagram,
            None,
            &AsciiRenderOptions::ascii(),
            &mut resources,
            AsciiExecution::new(&control, &policy),
        )
        .expect_err("the render should reject the second early-plan cache admission");

        assert_work_ceiling(error, maximum + 1, maximum, policy);
        assert_eq!(resources.layout_work_used(), SEEDED_WORK);
        assert_eq!(resources.document_cells_used(), SEEDED_DOCUMENT_CELLS);
        assert_eq!(diagram, original_diagram);
    }

    #[test]
    fn early_message_planning_cancels_inside_the_second_label_and_rolls_back_render() {
        let multi_codepoint_grapheme = "\u{301}".repeat(128);
        assert_eq!(multi_codepoint_grapheme.graphemes(true).count(), 1);
        assert_eq!(multi_codepoint_grapheme.chars().count(), 128);

        for (name, second_label) in [
            ("ordinary label", "z".repeat(128)),
            ("one multi-codepoint grapheme", multi_codepoint_grapheme),
        ] {
            let diagram = two_message_diagram(&second_label);
            let original_diagram = diagram.clone();
            let policy = AsciiResourcePolicy::default();
            let base_resources = seeded_resources(policy);
            let control = OperationControl::new();
            // The short first label and its cache admission complete before this checkpoint.
            // The second label still has many normalized segments left, including when all
            // of its source codepoints belong to one zero-width grapheme.
            control.cancel_after_checkpoints(250);
            let execution = AsciiExecution::new(&control, &policy);
            let mut resources = execution.resource_context(&base_resources, OperationPhase::Layout);
            let mut checkpoints = SequenceCheckpointCursor::new(execution, OperationPhase::Layout);
            let error = calculate_layout_with_resources(
                &diagram,
                &AsciiRenderOptions::ascii(),
                &mut resources,
                &mut checkpoints,
            )
            .expect_err("cancellation must stop the second label before centers are returned");

            assert_layout_cancellation(error, name);
            assert_eq!(
                base_resources.layout_work_used(),
                SEEDED_WORK + WORK_BEFORE_SECOND_LABEL,
                "{name}: the first label must have completed before the second label rolled back",
            );
            assert_eq!(base_resources.document_cells_used(), SEEDED_DOCUMENT_CELLS);
            assert_eq!(diagram, original_diagram);

            let mut render_resources = seeded_resources(policy);
            let render_control = OperationControl::new();
            render_control.cancel_after_checkpoints(250);
            let error = render_sequence_diagram_with_execution(
                &diagram,
                None,
                &AsciiRenderOptions::ascii(),
                &mut render_resources,
                AsciiExecution::new(&render_control, &policy),
            )
            .expect_err("the render must observe the same in-planning cancellation");

            assert_layout_cancellation(error, name);
            assert_eq!(render_resources.layout_work_used(), SEEDED_WORK, "{name}");
            assert_eq!(
                render_resources.document_cells_used(),
                SEEDED_DOCUMENT_CELLS,
                "{name}",
            );
            assert_eq!(diagram, original_diagram);
        }
    }

    fn two_message_diagram(second_label: &str) -> AsciiSequenceDiagram {
        let policy = AsciiResourcePolicy::default();
        let resources = ResourceContext::new(policy);
        let execution = AsciiExecution::for_test(&policy);
        let mut builder = SequenceTreeBuilder::new(2, &resources, execution).unwrap();
        for (model_index, label) in [FIRST_LABEL, second_label].into_iter().enumerate() {
            builder
                .push_event(
                    SequenceEvent::Message(SequenceMessage {
                        model_index,
                        from: model_index,
                        to: 1 - model_index,
                        label: label.to_string(),
                        wrap: false,
                        style: SequenceLineStyle::Solid,
                        source_marker: SequenceArrowHead::None,
                        target_marker: SequenceArrowHead::Filled,
                        direction: SequenceMessageDirection::Forward,
                        central_decoration: SequenceCentralDecoration::None,
                    }),
                    &resources,
                    execution,
                )
                .unwrap();
        }
        AsciiSequenceDiagram {
            participants: ["A", "B"]
                .into_iter()
                .map(|id| SequenceParticipant {
                    id: id.to_string(),
                    label: SequenceParticipantLabel::from_raw(
                        id,
                        false,
                        TerminalWidthProfile::Unicode,
                    ),
                })
                .collect(),
            lifecycles: vec![SequenceActorLifecycle::default(); 2],
            boxes: Vec::new(),
            body: builder.finish().unwrap(),
        }
    }

    fn seeded_resources(policy: AsciiResourcePolicy) -> ResourceContext {
        let resources = ResourceContext::new(policy);
        resources.charge_layout_work(SEEDED_WORK).unwrap();
        resources
            .charge_document_cells(SEEDED_DOCUMENT_CELLS)
            .unwrap();
        resources
    }

    fn assert_work_ceiling(
        error: AsciiError,
        actual: usize,
        maximum: usize,
        policy: AsciiResourcePolicy,
    ) {
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.phase() == AsciiResourceLimitPhase::LayoutWork
                    && details.cause == AsciiResourceLimitCause::Ceiling
                    && details.profile == policy.profile()
                    && details.actual == actual
                    && details.max == maximum
        ));
    }

    fn assert_layout_cancellation(error: AsciiError, name: &str) {
        assert!(
            matches!(
                error,
                AsciiError::Cancelled(cancelled)
                    if cancelled.phase == OperationPhase::Layout
                        && cancelled.reason == CancelReason::Requested
            ),
            "{name}: {error}",
        );
    }
}
