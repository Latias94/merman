use super::boxes::render_sequence_boxes;
use super::control::render_sequence_control_frames;
use super::control::{SequenceControlFrame, SequenceControlFrameSeparator};
use super::events::{ensure_message_actors_visible, render_message, render_self_message};
use super::layout::{
    LifecycleEdge, SequenceLayout, initial_visible_actors, lifecycle_actors_at, participant_left,
};
use super::model::{AsciiSequenceDiagram, SequenceControlKind, SequenceEvent};
use super::notes::{ensure_note_actors_visible, render_note};
use super::render::{SequenceChars, build_lifeline_line};
use super::text::{SequenceLine, padded_line, trim_right};
use crate::canvas::Canvas;
use crate::color::AsciiColorMode;
use crate::color::AsciiColorRole;
use crate::error::{AsciiError, Result};
use crate::options::AsciiRenderOptions;
use crate::text::display_width;

#[derive(Debug, Clone, PartialEq, Eq)]
enum SequenceEventEffect {
    StateOnly,
    Render {
        created_actors: Vec<usize>,
        destroyed_actors: Vec<usize>,
    },
}

impl SequenceEventEffect {
    fn created_actors(&self) -> &[usize] {
        match self {
            Self::StateOnly => &[],
            Self::Render { created_actors, .. } => created_actors,
        }
    }

    fn destroyed_actors(&self) -> &[usize] {
        match self {
            Self::StateOnly => &[],
            Self::Render {
                destroyed_actors, ..
            } => destroyed_actors,
        }
    }
}

#[derive(Debug, Clone)]
struct SequenceEventPlan {
    active_counts: Vec<usize>,
    visible_actors: Vec<bool>,
    control_frames: Vec<SequenceControlFrame>,
    active_control_frames: Vec<usize>,
}

impl SequenceEventPlan {
    fn new(diagram: &AsciiSequenceDiagram) -> Self {
        Self {
            active_counts: vec![0usize; diagram.participants.len()],
            visible_actors: initial_visible_actors(diagram),
            control_frames: Vec::new(),
            active_control_frames: Vec::new(),
        }
    }

    fn active_counts(&self) -> &[usize] {
        &self.active_counts
    }

    fn visible_actors(&self) -> &[bool] {
        &self.visible_actors
    }

    fn advance(
        &mut self,
        diagram: &AsciiSequenceDiagram,
        event: &SequenceEvent,
        current_row: usize,
    ) -> Result<SequenceEventEffect> {
        match event {
            SequenceEvent::ActivationStart { actor, .. } => {
                self.active_counts[*actor] += 1;
                Ok(SequenceEventEffect::StateOnly)
            }
            SequenceEvent::ActivationEnd { actor, .. } => {
                let Some(count) = self.active_counts.get_mut(*actor) else {
                    return Err(unsupported("activation actor state"));
                };
                if *count == 0 {
                    return Err(unsupported("activation underflow"));
                }
                *count -= 1;
                Ok(SequenceEventEffect::StateOnly)
            }
            SequenceEvent::ControlStart(start) => {
                let frame_index = self.control_frames.len();
                self.control_frames.push(SequenceControlFrame {
                    kind: start.kind,
                    label: start.label.clone(),
                    background: start.background,
                    start_row: current_row,
                    separators: Vec::new(),
                    end_row: None,
                });
                self.active_control_frames.push(frame_index);
                Ok(SequenceEventEffect::StateOnly)
            }
            SequenceEvent::ControlSeparator(separator) => {
                let Some(frame_index) = self.active_control_frames.last().copied() else {
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
                Ok(SequenceEventEffect::StateOnly)
            }
            SequenceEvent::ControlEnd { kind, .. } => {
                self.end_control_frame(*kind, current_row)?;
                Ok(SequenceEventEffect::StateOnly)
            }
            SequenceEvent::Message(_) | SequenceEvent::Note(_) => {
                let model_index = event.model_index();
                let created_actors =
                    lifecycle_actors_at(diagram, model_index, LifecycleEdge::Created);
                if !created_actors.is_empty() {
                    self.record_created_actors(&created_actors);
                }
                let destroyed_actors =
                    lifecycle_actors_at(diagram, model_index, LifecycleEdge::Destroyed);
                Ok(SequenceEventEffect::Render {
                    created_actors,
                    destroyed_actors,
                })
            }
        }
    }

    fn record_created_actors(&mut self, actor_indices: &[usize]) {
        for actor in actor_indices {
            if let Some(visible) = self.visible_actors.get_mut(*actor) {
                *visible = true;
            }
        }
    }

    fn record_destroyed_actors(&mut self, actor_indices: &[usize]) {
        for actor in actor_indices {
            if let Some(visible) = self.visible_actors.get_mut(*actor) {
                *visible = false;
            }
            if let Some(count) = self.active_counts.get_mut(*actor) {
                *count = 0;
            }
        }
    }

    fn complete(&mut self, effect: SequenceEventEffect) {
        if let SequenceEventEffect::Render {
            destroyed_actors, ..
        } = effect
        {
            self.record_destroyed_actors(&destroyed_actors);
        }
    }

    fn finish(self) -> Result<Vec<SequenceControlFrame>> {
        if !self.active_control_frames.is_empty() {
            return Err(unsupported("control block ordering"));
        }
        Ok(self.control_frames)
    }

    fn end_control_frame(&mut self, kind: SequenceControlKind, current_row: usize) -> Result<()> {
        let Some(frame_index) = self.active_control_frames.last().copied() else {
            return Err(unsupported("control block ordering"));
        };

        {
            let frame = &mut self.control_frames[frame_index];
            if !frame.kind.accepts_end(kind) {
                return Err(unsupported("control block ordering"));
            }
            if frame.current_section_start_row() == current_row {
                return Err(unsupported("empty control block sections"));
            }
            frame.end_row = Some(current_row - 1);
        }
        self.active_control_frames.pop();
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub(super) struct SequenceRowPlan {
    lines: Vec<SequenceLine>,
    control_frames: Vec<SequenceControlFrame>,
}

impl SequenceRowPlan {
    pub(super) fn build(
        diagram: &AsciiSequenceDiagram,
        layout: &SequenceLayout,
        chars: &SequenceChars,
        mirror_actors: bool,
    ) -> Result<Self> {
        let mut lines = Vec::new();
        let mut event_plan = SequenceEventPlan::new(diagram);

        lines.extend(render_participant_box_rows(
            diagram,
            layout,
            chars,
            event_plan.visible_actors(),
            ParticipantBoxFrame::Header,
        ));

        for event in &diagram.events {
            let effect = event_plan.advance(diagram, event, lines.len())?;
            if matches!(effect, SequenceEventEffect::StateOnly) {
                continue;
            }

            for _ in 0..layout.message_spacing {
                lines.push(build_lifeline_line(
                    layout,
                    chars,
                    event_plan.active_counts(),
                    event_plan.visible_actors(),
                ));
            }

            if !effect.created_actors().is_empty() {
                lines.extend(render_lifecycle_participants(
                    diagram,
                    layout,
                    chars,
                    event_plan.active_counts(),
                    event_plan.visible_actors(),
                    effect.created_actors(),
                ));
            }

            match event {
                SequenceEvent::Message(message) => {
                    ensure_message_actors_visible(message, event_plan.visible_actors())?;
                    if message.from == message.to {
                        lines.extend(render_self_message(
                            message,
                            layout,
                            chars,
                            event_plan.active_counts(),
                            event_plan.visible_actors(),
                            effect.destroyed_actors(),
                        ));
                    } else {
                        lines.extend(render_message(
                            message,
                            layout,
                            chars,
                            event_plan.active_counts(),
                            event_plan.visible_actors(),
                            effect.destroyed_actors(),
                        ));
                    }
                }
                SequenceEvent::Note(note) => {
                    ensure_note_actors_visible(note, event_plan.visible_actors())?;
                    lines.extend(render_note(
                        note,
                        layout,
                        chars,
                        event_plan.active_counts(),
                        event_plan.visible_actors(),
                    ));
                }
                SequenceEvent::ActivationStart { .. }
                | SequenceEvent::ActivationEnd { .. }
                | SequenceEvent::ControlStart(_)
                | SequenceEvent::ControlEnd { .. }
                | SequenceEvent::ControlSeparator(_) => {}
            }

            event_plan.complete(effect);
        }

        lines.push(build_lifeline_line(
            layout,
            chars,
            event_plan.active_counts(),
            event_plan.visible_actors(),
        ));
        if mirror_actors {
            lines.extend(render_participant_box_rows(
                diagram,
                layout,
                chars,
                event_plan.visible_actors(),
                ParticipantBoxFrame::Mirror,
            ));
        }

        Ok(Self {
            lines,
            control_frames: event_plan.finish()?,
        })
    }

    pub(super) fn render(
        self,
        diagram: &AsciiSequenceDiagram,
        layout: &SequenceLayout,
        chars: &SequenceChars,
        options: &AsciiRenderOptions,
    ) -> String {
        let mut lines = self.lines;
        if !self.control_frames.is_empty() {
            lines = render_sequence_control_frames(lines, &self.control_frames, chars);
        }
        if !diagram.boxes.is_empty() {
            lines = render_sequence_boxes(lines, diagram, layout, chars);
        }
        if let Some(title) = diagram.title.as_deref() {
            prepend_title_line(&mut lines, title);
        }
        finish_sequence_lines(lines, options)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParticipantBoxFrame {
    Header,
    Mirror,
}

fn render_participant_box_rows(
    diagram: &AsciiSequenceDiagram,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    visible_actors: &[bool],
    frame: ParticipantBoxFrame,
) -> Vec<SequenceLine> {
    participant_box_rows(diagram, frame)
        .into_iter()
        .map(|row| {
            build_participant_line(diagram, layout, visible_actors, |index| {
                build_participant_box_row(diagram, layout, chars, index, row)
            })
        })
        .collect()
}

fn participant_box_rows(
    diagram: &AsciiSequenceDiagram,
    frame: ParticipantBoxFrame,
) -> Vec<ParticipantBoxRow> {
    let mut rows = Vec::with_capacity(participant_label_row_count(diagram) + 2);
    rows.push(match frame {
        ParticipantBoxFrame::Header => ParticipantBoxRow::Top,
        ParticipantBoxFrame::Mirror => ParticipantBoxRow::MirrorTop,
    });
    rows.extend((0..participant_label_row_count(diagram)).map(ParticipantBoxRow::Label));
    rows.push(match frame {
        ParticipantBoxFrame::Header => ParticipantBoxRow::Bottom,
        ParticipantBoxFrame::Mirror => ParticipantBoxRow::MirrorBottom,
    });
    rows
}

fn participant_label_row_count(diagram: &AsciiSequenceDiagram) -> usize {
    diagram
        .participants
        .iter()
        .map(|participant| participant.label.lines().len())
        .max()
        .unwrap_or(1)
        .max(1)
}

fn build_participant_line(
    diagram: &AsciiSequenceDiagram,
    layout: &SequenceLayout,
    visible_actors: &[bool],
    draw: impl Fn(usize) -> SequenceLine,
) -> SequenceLine {
    let mut line = SequenceLine::blank(0);
    for index in 0..diagram.participants.len() {
        if !visible_actors.get(index).copied().unwrap_or(true) {
            continue;
        }
        let left = participant_left(layout, index);
        let needed = left.saturating_sub(line.len());
        line.push_spaces(needed);
        line.push_line(&draw(index));
    }
    line
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParticipantBoxRow {
    Top,
    Label(usize),
    Bottom,
    MirrorTop,
    MirrorBottom,
}

fn build_participant_box_row(
    diagram: &AsciiSequenceDiagram,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    index: usize,
    row: ParticipantBoxRow,
) -> SequenceLine {
    let width = layout.participant_widths[index];
    let total_width = width + super::BOX_BORDER_WIDTH;
    let mut line = SequenceLine::blank(total_width);
    match row {
        ParticipantBoxRow::Top | ParticipantBoxRow::MirrorTop => {
            line.set_role(0, chars.top_left, AsciiColorRole::SequenceFrame);
            for x in 1..=width {
                let ch = if row == ParticipantBoxRow::MirrorTop && x == (width / 2) + 1 {
                    chars.tee_up
                } else {
                    chars.horizontal
                };
                line.set_role(x, ch, AsciiColorRole::SequenceFrame);
            }
            line.set_role(width + 1, chars.top_right, AsciiColorRole::SequenceFrame);
        }
        ParticipantBoxRow::Label(label_row) => {
            let label = &diagram.participants[index].label;
            let label_lines = label.lines();
            let row_count = label_lines.len().max(1);
            let top_padding = (participant_label_row_count(diagram).saturating_sub(row_count)) / 2;
            let row_label = label_row
                .checked_sub(top_padding)
                .and_then(|index| label_lines.get(index));
            let label_width = row_label.map(|line| display_width(line)).unwrap_or(0);
            let left_padding = (width - label_width) / 2;
            line.set_role(0, chars.vertical, AsciiColorRole::SequenceFrame);
            if let Some(label) = row_label {
                line.write_text_role(1 + left_padding, label, AsciiColorRole::Text);
            }
            line.set_role(width + 1, chars.vertical, AsciiColorRole::SequenceFrame);
        }
        ParticipantBoxRow::Bottom | ParticipantBoxRow::MirrorBottom => {
            line.set_role(0, chars.bottom_left, AsciiColorRole::SequenceFrame);
            for x in 1..=width {
                let ch = if row == ParticipantBoxRow::Bottom && x == (width / 2) + 1 {
                    chars.tee_down
                } else {
                    chars.horizontal
                };
                line.set_role(x, ch, AsciiColorRole::SequenceFrame);
            }
            line.set_role(width + 1, chars.bottom_right, AsciiColorRole::SequenceFrame);
        }
    }
    line
}

fn render_lifecycle_participants(
    diagram: &AsciiSequenceDiagram,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    active_counts: &[usize],
    visible_actors: &[bool],
    actor_indices: &[usize],
) -> Vec<SequenceLine> {
    participant_box_rows(diagram, ParticipantBoxFrame::Header)
        .into_iter()
        .map(|row| {
            let width = actor_indices
                .iter()
                .map(|index| {
                    let segment = build_participant_box_row(diagram, layout, chars, *index, row);
                    participant_left(layout, *index) + segment.len()
                })
                .max()
                .unwrap_or(layout.total_width + 1)
                .max(layout.total_width + 1);
            let mut line = padded_line(
                build_lifeline_line(layout, chars, active_counts, visible_actors),
                width,
            );
            for index in actor_indices {
                let segment = build_participant_box_row(diagram, layout, chars, *index, row);
                line.write_line(participant_left(layout, *index), &segment);
            }
            trim_right(line)
        })
        .collect()
}

fn finish_sequence_lines(lines: Vec<SequenceLine>, options: &AsciiRenderOptions) -> String {
    if options.color_mode == AsciiColorMode::Plain {
        return lines
            .into_iter()
            .map(SequenceLine::into_text)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
    }

    if lines.is_empty() {
        return String::new();
    }

    let width = lines.iter().map(SequenceLine::len).max().unwrap_or(0);
    if width == 0 {
        return "\n".repeat(lines.len());
    }

    let mut canvas = Canvas::new(width, lines.len());
    for (y, line) in lines.iter().enumerate() {
        line.write_to(&mut canvas, y);
    }

    canvas.finish_trimmed_with_options(options)
}

fn prepend_title_line(lines: &mut Vec<SequenceLine>, title: &str) {
    let width = lines.iter().map(SequenceLine::len).max().unwrap_or(0);
    lines.insert(0, render_title_line(title, width));
}

fn render_title_line(title: &str, width: usize) -> SequenceLine {
    let title_width = display_width(title);
    let left = width.saturating_sub(title_width) / 2;
    let mut line = SequenceLine::blank(left);
    line.push_role_text(title, AsciiColorRole::Text);
    trim_right(line)
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
    use crate::options::AsciiRenderOptions;
    use crate::sequence::layout::calculate_layout;
    use crate::sequence::model::{
        SequenceActorLifecycle, SequenceArrowHead, SequenceControlSeparator, SequenceControlStart,
        SequenceEvent, SequenceLineStyle, SequenceMessage, SequenceParticipant,
        SequenceParticipantLabel,
    };

    #[test]
    fn event_plan_tracks_activation_counts() {
        let diagram = diagram(1);
        let mut plan = SequenceEventPlan::new(&diagram);

        assert_eq!(
            plan.advance(
                &diagram,
                &SequenceEvent::ActivationStart {
                    actor: 0,
                    model_index: 0,
                },
                3,
            )
            .unwrap(),
            SequenceEventEffect::StateOnly
        );
        assert_eq!(plan.active_counts(), &[1]);

        plan.advance(
            &diagram,
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

        plan.advance(
            &diagram,
            &SequenceEvent::ControlStart(SequenceControlStart {
                model_index: 0,
                kind: SequenceControlKind::Alt,
                label: "choice".to_string(),
                background: None,
            }),
            3,
        )
        .unwrap();

        let error = plan
            .advance(
                &diagram,
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
    fn event_plan_tracks_nested_control_frames() {
        let diagram = diagram(2);
        let mut plan = SequenceEventPlan::new(&diagram);

        plan.advance(
            &diagram,
            &SequenceEvent::ControlStart(SequenceControlStart {
                model_index: 0,
                kind: SequenceControlKind::Loop,
                label: "outer".to_string(),
                background: None,
            }),
            3,
        )
        .unwrap();
        plan.advance(
            &diagram,
            &SequenceEvent::ControlStart(SequenceControlStart {
                model_index: 1,
                kind: SequenceControlKind::Opt,
                label: "inner".to_string(),
                background: None,
            }),
            3,
        )
        .unwrap();
        plan.advance(
            &diagram,
            &SequenceEvent::ControlEnd {
                kind: SequenceControlKind::Opt,
                model_index: 2,
            },
            4,
        )
        .unwrap();
        plan.advance(
            &diagram,
            &SequenceEvent::ControlEnd {
                kind: SequenceControlKind::Loop,
                model_index: 3,
            },
            4,
        )
        .unwrap();

        let frames = plan.finish().unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].kind, SequenceControlKind::Loop);
        assert_eq!(frames[0].start_row, 3);
        assert_eq!(frames[0].end_row, Some(3));
        assert_eq!(frames[1].kind, SequenceControlKind::Opt);
        assert_eq!(frames[1].start_row, 3);
        assert_eq!(frames[1].end_row, Some(3));
    }

    #[test]
    fn event_plan_updates_lifecycle_visibility_and_resets_activation() {
        let mut diagram = diagram(1);
        diagram.lifecycles[0].created_at = Some(1);
        diagram.lifecycles[0].destroyed_at = Some(1);
        let mut plan = SequenceEventPlan::new(&diagram);
        assert_eq!(plan.visible_actors(), &[false]);

        let effect = plan
            .advance(
                &diagram,
                &SequenceEvent::ActivationStart {
                    actor: 0,
                    model_index: 0,
                },
                3,
            )
            .unwrap();
        assert_eq!(effect, SequenceEventEffect::StateOnly);

        let effect = plan
            .advance(
                &diagram,
                &SequenceEvent::Message(SequenceMessage {
                    model_index: 1,
                    from: 0,
                    to: 0,
                    label: "done".to_string(),
                    wrap: false,
                    style: SequenceLineStyle::Solid,
                    arrow: SequenceArrowHead::Filled,
                }),
                4,
            )
            .unwrap();

        assert_eq!(effect.created_actors(), &[0]);
        assert_eq!(effect.destroyed_actors(), &[0]);
        assert_eq!(plan.visible_actors(), &[true]);
        plan.complete(effect);
        assert_eq!(plan.visible_actors(), &[false]);
        assert_eq!(plan.active_counts(), &[0]);
    }

    #[test]
    fn row_plan_wraps_empty_diagram_with_lifeline_and_mirror_rows() {
        let diagram = diagram(2);
        let options = AsciiRenderOptions::ascii().with_sequence_mirror_actors(true);
        let layout = calculate_layout(&diagram, &options);
        let plan = SequenceRowPlan::build(
            &diagram,
            &layout,
            &ascii_chars(),
            options.sequence_mirror_actors,
        )
        .unwrap();
        let rendered = plan.render(&diagram, &layout, &ascii_chars(), &options);
        let rendered = rendered.lines().map(str::to_string).collect::<Vec<_>>();
        assert_eq!(rendered.len(), 7);
        assert!(rendered[0].starts_with('+'));
        assert!(rendered[1].contains("P0"));
        assert!(rendered[1].contains("P1"));
        assert!(rendered[3].contains('|'));
        assert!(rendered[4].starts_with('+'));
        assert!(rendered[5].contains("P0"));
        assert!(rendered[6].starts_with('+'));
    }

    #[test]
    fn row_plan_renders_title_before_content() {
        let mut diagram = diagram(1);
        diagram.title = Some("Timeline".to_string());
        let options = AsciiRenderOptions::ascii();
        let layout = calculate_layout(&diagram, &options);
        let plan = SequenceRowPlan::build(&diagram, &layout, &ascii_chars(), false).unwrap();

        let rendered = plan.render(&diagram, &layout, &ascii_chars(), &options);

        assert!(rendered.lines().next().unwrap_or("").contains("Timeline"));
    }

    fn diagram(participant_count: usize) -> AsciiSequenceDiagram {
        AsciiSequenceDiagram {
            title: None,
            participants: (0..participant_count)
                .map(|index| SequenceParticipant {
                    id: format!("p{index}"),
                    label: SequenceParticipantLabel::from_raw(&format!("P{index}"), false),
                })
                .collect(),
            lifecycles: vec![SequenceActorLifecycle::default(); participant_count],
            boxes: Vec::new(),
            events: Vec::new(),
        }
    }

    fn ascii_chars() -> SequenceChars {
        SequenceChars {
            top_left: '+',
            top_right: '+',
            bottom_left: '+',
            bottom_right: '+',
            horizontal: '-',
            vertical: '|',
            active_vertical: '#',
            destroyed_mark: 'x',
            tee_down: '+',
            tee_up: '+',
            tee_right: '+',
            tee_left: '+',
            filled_arrow_right: '>',
            filled_arrow_left: '<',
            open_arrow_right: '>',
            open_arrow_left: '<',
            solid_line: '-',
            dotted_line: '.',
            self_top_right: '+',
            self_bottom: '+',
        }
    }
}
