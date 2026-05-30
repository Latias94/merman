use super::control::{SequenceControlFrame, SequenceControlFrameSeparator};
use super::layout::initial_visible_actors;
use super::model::{AsciiSequenceDiagram, SequenceControlKind, SequenceEvent};
use crate::error::{AsciiError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SequencePlanStep {
    StateOnly,
    Render,
}

#[derive(Debug, Clone)]
pub(super) struct SequenceEventPlan {
    active_counts: Vec<usize>,
    visible_actors: Vec<bool>,
    control_frames: Vec<SequenceControlFrame>,
    active_control_frame: Option<usize>,
}

impl SequenceEventPlan {
    pub(super) fn new(diagram: &AsciiSequenceDiagram) -> Self {
        Self {
            active_counts: vec![0usize; diagram.participants.len()],
            visible_actors: initial_visible_actors(diagram),
            control_frames: Vec::new(),
            active_control_frame: None,
        }
    }

    pub(super) fn active_counts(&self) -> &[usize] {
        &self.active_counts
    }

    pub(super) fn visible_actors(&self) -> &[bool] {
        &self.visible_actors
    }

    pub(super) fn handle_event(
        &mut self,
        event: &SequenceEvent,
        current_row: usize,
    ) -> Result<SequencePlanStep> {
        match event {
            SequenceEvent::ActivationStart { actor, .. } => {
                self.active_counts[*actor] += 1;
                Ok(SequencePlanStep::StateOnly)
            }
            SequenceEvent::ActivationEnd { actor, .. } => {
                let Some(count) = self.active_counts.get_mut(*actor) else {
                    return Err(unsupported("activation actor state"));
                };
                if *count == 0 {
                    return Err(unsupported("activation underflow"));
                }
                *count -= 1;
                Ok(SequencePlanStep::StateOnly)
            }
            SequenceEvent::ControlStart(start) => {
                if self.active_control_frame.is_some() {
                    return Err(unsupported("nested control blocks"));
                }
                self.active_control_frame = Some(self.control_frames.len());
                self.control_frames.push(SequenceControlFrame {
                    kind: start.kind,
                    label: start.label.clone(),
                    start_row: current_row,
                    separators: Vec::new(),
                    end_row: None,
                });
                Ok(SequencePlanStep::StateOnly)
            }
            SequenceEvent::ControlSeparator(separator) => {
                let Some(frame_index) = self.active_control_frame else {
                    return Err(unsupported("control block ordering"));
                };
                let frame = &mut self.control_frames[frame_index];
                if frame.kind != separator.kind {
                    return Err(unsupported("control block ordering"));
                }
                if frame.current_section_start_row() == current_row {
                    return Err(unsupported("empty control block sections"));
                }
                frame.separators.push(SequenceControlFrameSeparator {
                    label: separator.label.clone(),
                    row: current_row,
                });
                Ok(SequencePlanStep::StateOnly)
            }
            SequenceEvent::ControlEnd { kind, .. } => {
                self.end_control_frame(*kind, current_row)?;
                Ok(SequencePlanStep::StateOnly)
            }
            SequenceEvent::Message(_) | SequenceEvent::Note(_) => Ok(SequencePlanStep::Render),
        }
    }

    pub(super) fn record_created_actors(&mut self, actor_indices: &[usize]) {
        for actor in actor_indices {
            if let Some(visible) = self.visible_actors.get_mut(*actor) {
                *visible = true;
            }
        }
    }

    pub(super) fn record_destroyed_actors(&mut self, actor_indices: &[usize]) {
        for actor in actor_indices {
            if let Some(visible) = self.visible_actors.get_mut(*actor) {
                *visible = false;
            }
            if let Some(count) = self.active_counts.get_mut(*actor) {
                *count = 0;
            }
        }
    }

    pub(super) fn finish(self) -> Result<Vec<SequenceControlFrame>> {
        if self.active_control_frame.is_some() {
            return Err(unsupported("control block ordering"));
        }
        Ok(self.control_frames)
    }

    fn end_control_frame(&mut self, kind: SequenceControlKind, current_row: usize) -> Result<()> {
        let Some(frame_index) = self.active_control_frame.take() else {
            return Err(unsupported("control block ordering"));
        };
        let frame = &mut self.control_frames[frame_index];
        if !frame.kind.accepts_end(kind) {
            return Err(unsupported("control block ordering"));
        }
        if frame.current_section_start_row() == current_row {
            return Err(unsupported("empty control block sections"));
        }
        frame.end_row = Some(current_row - 1);
        Ok(())
    }
}

fn unsupported(feature: &'static str) -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "sequence",
        feature,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequence::model::{
        SequenceActorLifecycle, SequenceControlSeparator, SequenceControlStart, SequenceParticipant,
    };

    #[test]
    fn event_plan_tracks_activation_counts() {
        let diagram = diagram(1);
        let mut plan = SequenceEventPlan::new(&diagram);

        assert_eq!(
            plan.handle_event(
                &SequenceEvent::ActivationStart {
                    actor: 0,
                    model_index: 0,
                },
                3,
            )
            .unwrap(),
            SequencePlanStep::StateOnly
        );
        assert_eq!(plan.active_counts(), &[1]);

        plan.handle_event(
            &SequenceEvent::ActivationEnd {
                actor: 0,
                model_index: 1,
            },
            4,
        )
        .unwrap();
        assert_eq!(plan.active_counts(), &[0]);
    }

    #[test]
    fn event_plan_rejects_empty_control_sections() {
        let diagram = diagram(1);
        let mut plan = SequenceEventPlan::new(&diagram);

        plan.handle_event(
            &SequenceEvent::ControlStart(SequenceControlStart {
                model_index: 0,
                kind: SequenceControlKind::Alt,
                label: "choice".to_string(),
            }),
            3,
        )
        .unwrap();

        let error = plan
            .handle_event(
                &SequenceEvent::ControlSeparator(SequenceControlSeparator {
                    model_index: 1,
                    kind: SequenceControlKind::Alt,
                    label: "other".to_string(),
                }),
                3,
            )
            .unwrap_err();

        assert!(matches!(
            error,
            AsciiError::UnsupportedFeature {
                diagram_type: "sequence",
                feature: "empty control block sections",
            }
        ));
    }

    #[test]
    fn event_plan_updates_lifecycle_visibility_and_resets_activation() {
        let mut diagram = diagram(1);
        diagram.lifecycles[0].created_at = Some(0);
        let mut plan = SequenceEventPlan::new(&diagram);
        assert_eq!(plan.visible_actors(), &[false]);

        plan.record_created_actors(&[0]);
        assert_eq!(plan.visible_actors(), &[true]);
        plan.handle_event(
            &SequenceEvent::ActivationStart {
                actor: 0,
                model_index: 1,
            },
            4,
        )
        .unwrap();

        plan.record_destroyed_actors(&[0]);
        assert_eq!(plan.visible_actors(), &[false]);
        assert_eq!(plan.active_counts(), &[0]);
    }

    fn diagram(participant_count: usize) -> AsciiSequenceDiagram {
        AsciiSequenceDiagram {
            participants: (0..participant_count)
                .map(|index| SequenceParticipant {
                    id: format!("p{index}"),
                    label: format!("P{index}"),
                })
                .collect(),
            lifecycles: vec![SequenceActorLifecycle::default(); participant_count],
            boxes: Vec::new(),
            events: Vec::new(),
        }
    }
}
