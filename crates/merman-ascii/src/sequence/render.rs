use super::boxes::render_sequence_boxes;
use super::control::{
    SequenceControlFrame, SequenceControlFrameSeparator, render_sequence_control_frames,
};
use super::events::{ensure_message_actors_visible, render_message, render_self_message};
use super::layout::{
    LifecycleEdge, SequenceLayout, calculate_layout, initial_visible_actors, lifecycle_actors_at,
    participant_left,
};
use super::model::{AsciiSequenceDiagram, SequenceArrowHead, SequenceEvent};
use super::notes::{ensure_note_actors_visible, render_note};
use super::text::{padded_line, trim_right, write_text};
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
    let mut active_counts = vec![0usize; diagram.participants.len()];
    let mut visible_actors = initial_visible_actors(diagram);
    let mut control_frames = Vec::<SequenceControlFrame>::new();
    let mut active_control_frame = None;

    lines.push(build_participant_line(
        diagram,
        &layout,
        &visible_actors,
        |index| participant_box_segment(diagram, &layout, &chars, index, ParticipantBoxRow::Top),
    ));
    lines.push(build_participant_line(
        diagram,
        &layout,
        &visible_actors,
        |index| participant_box_segment(diagram, &layout, &chars, index, ParticipantBoxRow::Label),
    ));
    lines.push(build_participant_line(
        diagram,
        &layout,
        &visible_actors,
        |index| participant_box_segment(diagram, &layout, &chars, index, ParticipantBoxRow::Bottom),
    ));

    for event in &diagram.events {
        match event {
            SequenceEvent::ActivationStart { actor, .. } => {
                active_counts[*actor] += 1;
                continue;
            }
            SequenceEvent::ActivationEnd { actor, .. } => {
                let Some(count) = active_counts.get_mut(*actor) else {
                    return Err(AsciiError::UnsupportedFeature {
                        diagram_type: "sequence",
                        feature: "activation actor state",
                    });
                };
                if *count == 0 {
                    return Err(AsciiError::UnsupportedFeature {
                        diagram_type: "sequence",
                        feature: "activation underflow",
                    });
                }
                *count -= 1;
                continue;
            }
            SequenceEvent::ControlStart(start) => {
                if active_control_frame.is_some() {
                    return Err(AsciiError::UnsupportedFeature {
                        diagram_type: "sequence",
                        feature: "nested control blocks",
                    });
                }
                active_control_frame = Some(control_frames.len());
                control_frames.push(SequenceControlFrame {
                    kind: start.kind,
                    label: start.label.clone(),
                    start_row: lines.len(),
                    separators: Vec::new(),
                    end_row: None,
                });
                continue;
            }
            SequenceEvent::ControlSeparator(separator) => {
                let Some(frame_index) = active_control_frame else {
                    return Err(AsciiError::UnsupportedFeature {
                        diagram_type: "sequence",
                        feature: "control block ordering",
                    });
                };
                let frame = &mut control_frames[frame_index];
                if frame.kind != separator.kind {
                    return Err(AsciiError::UnsupportedFeature {
                        diagram_type: "sequence",
                        feature: "control block ordering",
                    });
                }
                if frame.current_section_start_row() == lines.len() {
                    return Err(AsciiError::UnsupportedFeature {
                        diagram_type: "sequence",
                        feature: "empty control block sections",
                    });
                }
                frame.separators.push(SequenceControlFrameSeparator {
                    label: separator.label.clone(),
                    row: lines.len(),
                });
                continue;
            }
            SequenceEvent::ControlEnd { kind, .. } => {
                let Some(frame_index) = active_control_frame.take() else {
                    return Err(AsciiError::UnsupportedFeature {
                        diagram_type: "sequence",
                        feature: "control block ordering",
                    });
                };
                let frame = &mut control_frames[frame_index];
                if !frame.kind.accepts_end(*kind) {
                    return Err(AsciiError::UnsupportedFeature {
                        diagram_type: "sequence",
                        feature: "control block ordering",
                    });
                }
                if frame.current_section_start_row() == lines.len() {
                    return Err(AsciiError::UnsupportedFeature {
                        diagram_type: "sequence",
                        feature: "empty control block sections",
                    });
                }
                frame.end_row = Some(lines.len() - 1);
                continue;
            }
            SequenceEvent::Message(_) | SequenceEvent::Note(_) => {}
        }

        for _ in 0..layout.message_spacing {
            lines.push(build_lifeline(
                &layout,
                &chars,
                &active_counts,
                &visible_actors,
            ));
        }

        let model_index = event.model_index();
        let created_actors = lifecycle_actors_at(diagram, model_index, LifecycleEdge::Created);
        if !created_actors.is_empty() {
            lines.extend(render_lifecycle_participants(
                diagram,
                &layout,
                &chars,
                &active_counts,
                &visible_actors,
                &created_actors,
            ));
            for actor in &created_actors {
                visible_actors[*actor] = true;
            }
        }

        let destroyed_actors = lifecycle_actors_at(diagram, model_index, LifecycleEdge::Destroyed);

        match event {
            SequenceEvent::Message(message) => {
                ensure_message_actors_visible(message, &visible_actors)?;
                if message.from == message.to {
                    lines.extend(render_self_message(
                        message,
                        &layout,
                        &chars,
                        &active_counts,
                        &visible_actors,
                        &destroyed_actors,
                    ));
                } else {
                    lines.extend(render_message(
                        message,
                        &layout,
                        &chars,
                        &active_counts,
                        &visible_actors,
                        &destroyed_actors,
                    ));
                }
            }
            SequenceEvent::Note(note) => {
                ensure_note_actors_visible(note, &visible_actors)?;
                lines.extend(render_note(
                    note,
                    &layout,
                    &chars,
                    &active_counts,
                    &visible_actors,
                ));
            }
            SequenceEvent::ActivationStart { .. }
            | SequenceEvent::ActivationEnd { .. }
            | SequenceEvent::ControlStart(_)
            | SequenceEvent::ControlEnd { .. }
            | SequenceEvent::ControlSeparator(_) => {}
        }

        for actor in destroyed_actors {
            visible_actors[actor] = false;
            active_counts[actor] = 0;
        }
    }

    lines.push(build_lifeline(
        &layout,
        &chars,
        &active_counts,
        &visible_actors,
    ));
    if active_control_frame.is_some() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "control block ordering",
        });
    }
    if !control_frames.is_empty() {
        lines = render_sequence_control_frames(lines, &control_frames, &chars);
    }
    if !diagram.boxes.is_empty() {
        lines = render_sequence_boxes(lines, diagram, &layout, &chars);
    }
    Ok(lines.join("\n") + "\n")
}

fn build_participant_line(
    diagram: &AsciiSequenceDiagram,
    layout: &SequenceLayout,
    visible_actors: &[bool],
    draw: impl Fn(usize) -> String,
) -> String {
    let mut line = String::new();
    for index in 0..diagram.participants.len() {
        if !visible_actors.get(index).copied().unwrap_or(true) {
            continue;
        }
        let left = participant_left(layout, index);
        let needed = left.saturating_sub(line.chars().count());
        line.push_str(&" ".repeat(needed));
        line.push_str(&draw(index));
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
) -> String {
    let width = layout.participant_widths[index];
    match row {
        ParticipantBoxRow::Top => {
            format!(
                "{}{}{}",
                chars.top_left,
                chars.horizontal.to_string().repeat(width),
                chars.top_right
            )
        }
        ParticipantBoxRow::Label => {
            let label = &diagram.participants[index].label;
            let label_width = display_width(label);
            let left_padding = (width - label_width) / 2;
            format!(
                "{}{}{}{}{}",
                chars.vertical,
                " ".repeat(left_padding),
                label,
                " ".repeat(width - left_padding - label_width),
                chars.vertical
            )
        }
        ParticipantBoxRow::Bottom => {
            format!(
                "{}{}{}{}{}",
                chars.bottom_left,
                chars.horizontal.to_string().repeat(width / 2),
                chars.tee_down,
                chars.horizontal.to_string().repeat(width - width / 2 - 1),
                chars.bottom_right
            )
        }
    }
}

fn render_lifecycle_participants(
    diagram: &AsciiSequenceDiagram,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    active_counts: &[usize],
    visible_actors: &[bool],
    actor_indices: &[usize],
) -> Vec<String> {
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
                    + participant_box_segment(diagram, layout, chars, *index, row)
                        .chars()
                        .count()
            })
            .max()
            .unwrap_or(layout.total_width + 1)
            .max(layout.total_width + 1);
        let mut line = padded_line(
            build_lifeline(layout, chars, active_counts, visible_actors),
            width,
        );
        for index in actor_indices {
            let segment = participant_box_segment(diagram, layout, chars, *index, row);
            write_text(&mut line, participant_left(layout, *index), &segment);
        }
        trim_right(line)
    })
    .collect()
}

pub(super) fn build_lifeline(
    layout: &SequenceLayout,
    chars: &SequenceChars,
    active_counts: &[usize],
    visible_actors: &[bool],
) -> String {
    let mut line = vec![' '; layout.total_width + 1];
    for (index, center) in layout.participant_centers.iter().enumerate() {
        if !visible_actors.get(index).copied().unwrap_or(true) {
            continue;
        }
        if *center < line.len() {
            line[*center] = lifeline_char(index, chars, active_counts);
        }
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

pub(super) fn render_overlay_row(
    layout: &SequenceLayout,
    chars: &SequenceChars,
    active_counts: &[usize],
    visible_actors: &[bool],
    left: usize,
    text: &str,
) -> String {
    let needed = left + text.chars().count();
    let mut line = padded_line(
        build_lifeline(layout, chars, active_counts, visible_actors),
        needed,
    );
    write_text(&mut line, left, text);
    trim_right(line)
}
