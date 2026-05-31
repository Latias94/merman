use super::boxes::render_sequence_boxes;
use super::control::render_sequence_control_frames;
use super::events::{ensure_message_actors_visible, render_message, render_self_message};
use super::layout::{
    LifecycleEdge, SequenceLayout, calculate_layout, lifecycle_actors_at, participant_left,
};
use super::model::{AsciiSequenceDiagram, SequenceArrowHead, SequenceEvent};
use super::notes::{ensure_note_actors_visible, render_note};
use super::plan::{SequenceEventPlan, SequencePlanStep};
use super::text::{SequenceLine, padded_line, trim_right};
use crate::canvas::Canvas;
use crate::color::{AsciiColorMode, AsciiColorRole};
use crate::error::{AsciiError, Result};
use crate::options::{AsciiCharset, AsciiRenderOptions};
use crate::text::display_width;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SequenceChars {
    pub(super) top_left: char,
    pub(super) top_right: char,
    pub(super) bottom_left: char,
    pub(super) bottom_right: char,
    pub(super) horizontal: char,
    pub(super) vertical: char,
    pub(super) active_vertical: char,
    pub(super) destroyed_mark: char,
    pub(super) tee_down: char,
    pub(super) tee_right: char,
    pub(super) tee_left: char,
    pub(super) filled_arrow_right: char,
    pub(super) filled_arrow_left: char,
    pub(super) open_arrow_right: char,
    pub(super) open_arrow_left: char,
    pub(super) solid_line: char,
    pub(super) dotted_line: char,
    pub(super) self_top_right: char,
    pub(super) self_bottom: char,
}

impl SequenceChars {
    fn for_options(options: &AsciiRenderOptions) -> Self {
        match options.charset {
            AsciiCharset::Ascii => Self {
                top_left: '+',
                top_right: '+',
                bottom_left: '+',
                bottom_right: '+',
                horizontal: '-',
                vertical: '|',
                active_vertical: '#',
                destroyed_mark: 'x',
                tee_down: '+',
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
            },
            AsciiCharset::Unicode => Self {
                top_left: '┌',
                top_right: '┐',
                bottom_left: '└',
                bottom_right: '┘',
                horizontal: '─',
                vertical: '│',
                active_vertical: '┃',
                destroyed_mark: '×',
                tee_down: '┬',
                tee_right: '├',
                tee_left: '┤',
                filled_arrow_right: '►',
                filled_arrow_left: '◄',
                open_arrow_right: '>',
                open_arrow_left: '<',
                solid_line: '─',
                dotted_line: '┈',
                self_top_right: '┐',
                self_bottom: '┘',
            },
        }
    }

    pub(super) fn arrow_right(self, arrow: SequenceArrowHead) -> char {
        match arrow {
            SequenceArrowHead::Filled => self.filled_arrow_right,
            SequenceArrowHead::Open => self.open_arrow_right,
            SequenceArrowHead::Cross => self.destroyed_mark,
        }
    }

    pub(super) fn arrow_left(self, arrow: SequenceArrowHead) -> char {
        match arrow {
            SequenceArrowHead::Filled => self.filled_arrow_left,
            SequenceArrowHead::Open => self.open_arrow_left,
            SequenceArrowHead::Cross => self.destroyed_mark,
        }
    }
}

pub(crate) fn render_sequence_diagram(
    diagram: &AsciiSequenceDiagram,
    options: &AsciiRenderOptions,
) -> Result<String> {
    options.validate()?;
    if diagram.participants.is_empty() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "no participants",
        });
    }

    let chars = SequenceChars::for_options(options);
    let layout = calculate_layout(diagram, options);
    let mut lines = Vec::new();
    let mut event_plan = SequenceEventPlan::new(diagram);

    lines.push(build_participant_line(
        diagram,
        &layout,
        event_plan.visible_actors(),
        |index| participant_box_segment(diagram, &layout, &chars, index, ParticipantBoxRow::Top),
    ));
    lines.push(build_participant_line(
        diagram,
        &layout,
        event_plan.visible_actors(),
        |index| participant_box_segment(diagram, &layout, &chars, index, ParticipantBoxRow::Label),
    ));
    lines.push(build_participant_line(
        diagram,
        &layout,
        event_plan.visible_actors(),
        |index| participant_box_segment(diagram, &layout, &chars, index, ParticipantBoxRow::Bottom),
    ));

    for event in &diagram.events {
        if event_plan.handle_event(event, lines.len())? == SequencePlanStep::StateOnly {
            continue;
        }

        for _ in 0..layout.message_spacing {
            lines.push(build_lifeline_line(
                &layout,
                &chars,
                event_plan.active_counts(),
                event_plan.visible_actors(),
            ));
        }

        let model_index = event.model_index();
        let created_actors = lifecycle_actors_at(diagram, model_index, LifecycleEdge::Created);
        if !created_actors.is_empty() {
            lines.extend(render_lifecycle_participants(
                diagram,
                &layout,
                &chars,
                event_plan.active_counts(),
                event_plan.visible_actors(),
                &created_actors,
            ));
            event_plan.record_created_actors(&created_actors);
        }

        let destroyed_actors = lifecycle_actors_at(diagram, model_index, LifecycleEdge::Destroyed);

        match event {
            SequenceEvent::Message(message) => {
                ensure_message_actors_visible(message, event_plan.visible_actors())?;
                if message.from == message.to {
                    lines.extend(render_self_message(
                        message,
                        &layout,
                        &chars,
                        event_plan.active_counts(),
                        event_plan.visible_actors(),
                        &destroyed_actors,
                    ));
                } else {
                    lines.extend(render_message(
                        message,
                        &layout,
                        &chars,
                        event_plan.active_counts(),
                        event_plan.visible_actors(),
                        &destroyed_actors,
                    ));
                }
            }
            SequenceEvent::Note(note) => {
                ensure_note_actors_visible(note, event_plan.visible_actors())?;
                lines.extend(render_note(
                    note,
                    &layout,
                    &chars,
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

        event_plan.record_destroyed_actors(&destroyed_actors);
    }

    lines.push(build_lifeline_line(
        &layout,
        &chars,
        event_plan.active_counts(),
        event_plan.visible_actors(),
    ));
    let control_frames = event_plan.finish()?;
    if !control_frames.is_empty() {
        lines = render_sequence_control_frames(lines, &control_frames, &chars);
    }
    if !diagram.boxes.is_empty() {
        lines = render_sequence_boxes(lines, diagram, &layout, &chars);
    }
    if let Some(title) = diagram.title.as_deref() {
        prepend_title_line(&mut lines, title);
    }
    Ok(finish_sequence_lines(lines, options))
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
    Label,
    Bottom,
}

fn participant_box_segment(
    diagram: &AsciiSequenceDiagram,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    index: usize,
    row: ParticipantBoxRow,
) -> SequenceLine {
    let width = layout.participant_widths[index];
    let total_width = width + 2;
    let mut line = SequenceLine::blank(total_width);
    match row {
        ParticipantBoxRow::Top => {
            line.set_role(0, chars.top_left, AsciiColorRole::SequenceFrame);
            for x in 1..=width {
                line.set_role(x, chars.horizontal, AsciiColorRole::SequenceFrame);
            }
            line.set_role(width + 1, chars.top_right, AsciiColorRole::SequenceFrame);
        }
        ParticipantBoxRow::Label => {
            let label = &diagram.participants[index].label;
            let label_width = display_width(label);
            let left_padding = (width - label_width) / 2;
            line.set_role(0, chars.vertical, AsciiColorRole::SequenceFrame);
            line.write_text_role(1 + left_padding, label, AsciiColorRole::Text);
            line.set_role(width + 1, chars.vertical, AsciiColorRole::SequenceFrame);
        }
        ParticipantBoxRow::Bottom => {
            line.set_role(0, chars.bottom_left, AsciiColorRole::SequenceFrame);
            for x in 1..=width {
                let ch = if x == (width / 2) + 1 {
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
    [
        ParticipantBoxRow::Top,
        ParticipantBoxRow::Label,
        ParticipantBoxRow::Bottom,
    ]
    .into_iter()
    .map(|row| {
        let width = actor_indices
            .iter()
            .map(|index| {
                participant_left(layout, *index)
                    + participant_box_segment(diagram, layout, chars, *index, row).len()
            })
            .max()
            .unwrap_or(layout.total_width + 1)
            .max(layout.total_width + 1);
        let mut line = padded_line(
            build_lifeline_line(layout, chars, active_counts, visible_actors),
            width,
        );
        for index in actor_indices {
            let segment = participant_box_segment(diagram, layout, chars, *index, row);
            line.write_line(participant_left(layout, *index), &segment);
        }
        trim_right(line)
    })
    .collect()
}

pub(super) fn build_lifeline_line(
    layout: &SequenceLayout,
    chars: &SequenceChars,
    active_counts: &[usize],
    visible_actors: &[bool],
) -> SequenceLine {
    let mut line = SequenceLine::blank(layout.total_width + 1);
    for (index, center) in layout.participant_centers.iter().enumerate() {
        if !visible_actors.get(index).copied().unwrap_or(true) {
            continue;
        }
        line.set_role(
            *center,
            lifeline_char(index, chars, active_counts),
            lifeline_role(index, active_counts),
        );
    }
    trim_right(line)
}

pub(super) fn lifeline_char(index: usize, chars: &SequenceChars, active_counts: &[usize]) -> char {
    if active_counts.get(index).copied().unwrap_or(0) > 0 {
        chars.active_vertical
    } else {
        chars.vertical
    }
}

pub(super) fn lifeline_role(index: usize, active_counts: &[usize]) -> AsciiColorRole {
    if active_counts.get(index).copied().unwrap_or(0) > 0 {
        AsciiColorRole::SequenceActivation
    } else {
        AsciiColorRole::SequenceLifeline
    }
}

pub(super) fn render_overlay_row(
    layout: &SequenceLayout,
    chars: &SequenceChars,
    active_counts: &[usize],
    visible_actors: &[bool],
    left: usize,
    overlay: &SequenceLine,
) -> SequenceLine {
    let needed = left + overlay.len();
    let mut line = padded_line(
        build_lifeline_line(layout, chars, active_counts, visible_actors),
        needed,
    );
    line.write_line(left, overlay);
    trim_right(line)
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
