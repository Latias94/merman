use crate::error::{AsciiError, Result};
use crate::options::{AsciiCharset, AsciiRenderOptions};
use crate::text::display_width;
use merman_core::diagrams::sequence::{
    SequenceDiagramRenderModel, SequenceMessage as CoreSequenceMessage, SequenceMessagePayload,
};
use std::collections::HashMap;

const BOX_PADDING_LEFT_RIGHT: usize = 2;
const MIN_BOX_WIDTH: usize = 3;
const BOX_BORDER_WIDTH: usize = 2;
const LABEL_LEFT_MARGIN: usize = 2;
const LABEL_BUFFER_SPACE: usize = 10;
const AUTONUMBER_MESSAGE_TYPE: i32 = 26;
const SOLID_MESSAGE_TYPE: i32 = 0;
const DOTTED_MESSAGE_TYPE: i32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AsciiSequenceDiagram {
    participants: Vec<SequenceParticipant>,
    messages: Vec<SequenceMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SequenceParticipant {
    id: String,
    label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SequenceMessage {
    from: usize,
    to: usize,
    label: String,
    style: SequenceLineStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SequenceLineStyle {
    Solid,
    Dotted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SequenceChars {
    top_left: char,
    top_right: char,
    bottom_left: char,
    bottom_right: char,
    horizontal: char,
    vertical: char,
    tee_down: char,
    tee_right: char,
    tee_left: char,
    arrow_right: char,
    arrow_left: char,
    solid_line: char,
    dotted_line: char,
    self_top_right: char,
    self_bottom: char,
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
                tee_down: '+',
                tee_right: '+',
                tee_left: '+',
                arrow_right: '>',
                arrow_left: '<',
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
                tee_down: '┬',
                tee_right: '├',
                tee_left: '┤',
                arrow_right: '►',
                arrow_left: '◄',
                solid_line: '─',
                dotted_line: '┈',
                self_top_right: '┐',
                self_bottom: '┘',
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SequenceLayout {
    participant_widths: Vec<usize>,
    participant_centers: Vec<usize>,
    total_width: usize,
    message_spacing: usize,
    self_message_width: usize,
}

pub(crate) fn from_sequence_model(
    model: &SequenceDiagramRenderModel,
) -> Result<AsciiSequenceDiagram> {
    validate_supported_sequence_model(model)?;

    let participants = sequence_participants(model);
    if participants.is_empty() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "no participants",
        });
    }

    let participant_index = participants
        .iter()
        .enumerate()
        .map(|(index, participant)| (participant.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let mut messages = Vec::new();
    let mut autonumber = AutonumberState::default();

    for message in &model.messages {
        if consume_autonumber(message, &mut autonumber) {
            continue;
        }

        let from = message
            .from
            .as_deref()
            .ok_or(AsciiError::UnsupportedFeature {
                diagram_type: "sequence",
                feature: "control messages",
            })?;
        let to = message
            .to
            .as_deref()
            .ok_or(AsciiError::UnsupportedFeature {
                diagram_type: "sequence",
                feature: "control messages",
            })?;

        let from = *participant_index
            .get(from)
            .ok_or(AsciiError::UnsupportedFeature {
                diagram_type: "sequence",
                feature: "messages with unknown actors",
            })?;
        let to = *participant_index
            .get(to)
            .ok_or(AsciiError::UnsupportedFeature {
                diagram_type: "sequence",
                feature: "messages with unknown actors",
            })?;

        let style = match message.message_type {
            SOLID_MESSAGE_TYPE => SequenceLineStyle::Solid,
            DOTTED_MESSAGE_TYPE => SequenceLineStyle::Dotted,
            _ => {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type: "sequence",
                    feature: "message types",
                });
            }
        };
        let label = autonumber.label(message.message_text());

        messages.push(SequenceMessage {
            from,
            to,
            label,
            style,
        });
    }

    Ok(AsciiSequenceDiagram {
        participants,
        messages,
    })
}

fn validate_supported_sequence_model(model: &SequenceDiagramRenderModel) -> Result<()> {
    if model.title.is_some() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "diagram titles",
        });
    }

    if model
        .actors
        .values()
        .any(|actor| actor.actor_type != "participant")
    {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "actor participant shapes",
        });
    }

    if model.actors.values().any(|actor| actor.wrap) {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "wrapped actor labels",
        });
    }

    if model
        .actors
        .values()
        .any(|actor| !actor.links.is_empty() || !actor.properties.is_empty())
    {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "actor links/properties",
        });
    }

    if !model.notes.is_empty() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "notes",
        });
    }

    if !model.boxes.is_empty() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "boxes",
        });
    }

    if !model.created_actors.is_empty() || !model.destroyed_actors.is_empty() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "actor create/destroy",
        });
    }

    if model.messages.iter().any(|message| message.activate) {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "activations",
        });
    }

    if model
        .messages
        .iter()
        .any(|message| message.placement.is_some())
    {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "message placement",
        });
    }

    if model.messages.iter().any(|message| message.wrap) {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "wrapped messages",
        });
    }

    Ok(())
}

fn sequence_participants(model: &SequenceDiagramRenderModel) -> Vec<SequenceParticipant> {
    let ids = if model.actor_order.is_empty() {
        model.actors.keys().cloned().collect::<Vec<_>>()
    } else {
        model.actor_order.clone()
    };

    ids.into_iter()
        .filter_map(|id| {
            let actor = model.actors.get(&id)?;
            let label = if actor.description.is_empty() {
                if actor.name.is_empty() {
                    id.clone()
                } else {
                    actor.name.clone()
                }
            } else {
                actor.description.clone()
            };
            Some(SequenceParticipant { id, label })
        })
        .collect()
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct AutonumberState {
    next: Option<i64>,
    step: i64,
}

impl AutonumberState {
    fn label(&mut self, text: &str) -> String {
        if let Some(next) = self.next {
            let label = if text.is_empty() {
                next.to_string()
            } else {
                format!("{next}. {text}")
            };
            self.next = Some(next + self.step);
            return label;
        }
        text.to_string()
    }
}

fn consume_autonumber(message: &CoreSequenceMessage, state: &mut AutonumberState) -> bool {
    let SequenceMessagePayload::Autonumber(autonumber) = &message.message else {
        return false;
    };

    if message.message_type != AUTONUMBER_MESSAGE_TYPE {
        return false;
    }

    if autonumber.visible {
        state.next = Some(autonumber.start.unwrap_or(1));
        state.step = autonumber.step.unwrap_or(1);
    } else {
        state.next = None;
        state.step = 1;
    }
    true
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

    lines.push(build_participant_line(diagram, &layout, |index| {
        format!(
            "{}{}{}",
            chars.top_left,
            chars
                .horizontal
                .to_string()
                .repeat(layout.participant_widths[index]),
            chars.top_right
        )
    }));
    lines.push(build_participant_line(diagram, &layout, |index| {
        let width = layout.participant_widths[index];
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
    }));
    lines.push(build_participant_line(diagram, &layout, |index| {
        let width = layout.participant_widths[index];
        format!(
            "{}{}{}{}{}",
            chars.bottom_left,
            chars.horizontal.to_string().repeat(width / 2),
            chars.tee_down,
            chars.horizontal.to_string().repeat(width - width / 2 - 1),
            chars.bottom_right
        )
    }));

    for message in &diagram.messages {
        for _ in 0..layout.message_spacing {
            lines.push(build_lifeline(&layout, &chars));
        }

        if message.from == message.to {
            lines.extend(render_self_message(message, &layout, &chars));
        } else {
            lines.extend(render_message(message, &layout, &chars));
        }
    }

    lines.push(build_lifeline(&layout, &chars));
    Ok(lines.join("\n") + "\n")
}

fn calculate_layout(
    diagram: &AsciiSequenceDiagram,
    options: &AsciiRenderOptions,
) -> SequenceLayout {
    let participant_widths = diagram
        .participants
        .iter()
        .map(|participant| {
            (display_width(&participant.label) + BOX_PADDING_LEFT_RIGHT).max(MIN_BOX_WIDTH)
        })
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

fn build_participant_line(
    diagram: &AsciiSequenceDiagram,
    layout: &SequenceLayout,
    draw: impl Fn(usize) -> String,
) -> String {
    let mut line = String::new();
    for index in 0..diagram.participants.len() {
        let box_width = layout.participant_widths[index] + BOX_BORDER_WIDTH;
        let left = layout.participant_centers[index] - box_width / 2;
        let needed = left.saturating_sub(line.chars().count());
        line.push_str(&" ".repeat(needed));
        line.push_str(&draw(index));
    }
    line
}

fn build_lifeline(layout: &SequenceLayout, chars: &SequenceChars) -> String {
    let mut line = vec![' '; layout.total_width + 1];
    for center in &layout.participant_centers {
        if *center < line.len() {
            line[*center] = chars.vertical;
        }
    }
    trim_right(line)
}

fn render_message(
    message: &SequenceMessage,
    layout: &SequenceLayout,
    chars: &SequenceChars,
) -> Vec<String> {
    let mut lines = Vec::new();
    let from = layout.participant_centers[message.from];
    let to = layout.participant_centers[message.to];

    if !message.label.is_empty() {
        let start = from.min(to) + LABEL_LEFT_MARGIN;
        let label_width = display_width(&message.label);
        let width = layout
            .total_width
            .max(start + label_width)
            .saturating_add(LABEL_BUFFER_SPACE);
        let mut line = padded_line(build_lifeline(layout, chars), width);
        write_text(&mut line, start, &message.label);
        lines.push(trim_right(line));
    }

    let mut line = build_lifeline(layout, chars).chars().collect::<Vec<_>>();
    let style = match message.style {
        SequenceLineStyle::Solid => chars.solid_line,
        SequenceLineStyle::Dotted => chars.dotted_line,
    };

    if from < to {
        line[from] = chars.tee_right;
        for cell in line.iter_mut().take(to).skip(from + 1) {
            *cell = style;
        }
        line[to - 1] = chars.arrow_right;
        line[to] = chars.vertical;
    } else {
        line[to] = chars.vertical;
        line[to + 1] = chars.arrow_left;
        for cell in line.iter_mut().take(from).skip(to + 2) {
            *cell = style;
        }
        line[from] = chars.tee_left;
    }
    lines.push(trim_right(line));
    lines
}

fn render_self_message(
    message: &SequenceMessage,
    layout: &SequenceLayout,
    chars: &SequenceChars,
) -> Vec<String> {
    let mut lines = Vec::new();
    let center = layout.participant_centers[message.from];
    let width = layout.self_message_width;

    if !message.label.is_empty() {
        let start = center + LABEL_LEFT_MARGIN;
        let needed = start + display_width(&message.label) + LABEL_BUFFER_SPACE;
        let mut line = ensure_self_width(build_lifeline(layout, chars), layout, needed);
        write_text(&mut line, start, &message.label);
        lines.push(trim_right(line));
    }

    let mut top = ensure_self_width(build_lifeline(layout, chars), layout, 0);
    top[center] = chars.tee_right;
    for offset in 1..width {
        top[center + offset] = chars.horizontal;
    }
    top[center + width - 1] = chars.self_top_right;
    lines.push(trim_right(top));

    let mut middle = ensure_self_width(build_lifeline(layout, chars), layout, 0);
    middle[center + width - 1] = chars.vertical;
    lines.push(trim_right(middle));

    let mut bottom = ensure_self_width(build_lifeline(layout, chars), layout, 0);
    bottom[center] = chars.vertical;
    bottom[center + 1] = chars.arrow_left;
    for offset in 2..(width - 1) {
        bottom[center + offset] = chars.horizontal;
    }
    bottom[center + width - 1] = chars.self_bottom;
    lines.push(trim_right(bottom));

    lines
}

fn padded_line(line: String, width: usize) -> Vec<char> {
    let mut line = line.chars().collect::<Vec<_>>();
    if line.len() < width {
        line.extend(std::iter::repeat_n(' ', width - line.len()));
    }
    line
}

fn ensure_self_width(line: String, layout: &SequenceLayout, needed: usize) -> Vec<char> {
    let width = (layout.total_width + layout.self_message_width + 1).max(needed);
    padded_line(line, width)
}

fn write_text(line: &mut [char], start: usize, text: &str) {
    for (offset, ch) in text.chars().enumerate() {
        let index = start + offset;
        if index < line.len() {
            line[index] = ch;
        }
    }
}

fn trim_right(mut line: Vec<char>) -> String {
    while line.last() == Some(&' ') {
        line.pop();
    }
    line.into_iter().collect()
}
