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
const NOTE_MESSAGE_TYPE: i32 = 2;
const SOLID_FILLED_MESSAGE_TYPE: i32 = 0;
const DOTTED_FILLED_MESSAGE_TYPE: i32 = 1;
const SOLID_OPEN_MESSAGE_TYPE: i32 = 5;
const DOTTED_OPEN_MESSAGE_TYPE: i32 = 6;
const NOTE_PLACEMENT_LEFT_OF: i32 = 0;
const NOTE_PLACEMENT_RIGHT_OF: i32 = 1;
const NOTE_PLACEMENT_OVER: i32 = 2;
const NOTE_SIDE_GAP: usize = 2;
const SEQUENCE_BOX_CONTENT_OFFSET: usize = 1;
const SEQUENCE_BOX_LABEL_MARGIN: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AsciiSequenceDiagram {
    participants: Vec<SequenceParticipant>,
    boxes: Vec<SequenceGroupBox>,
    events: Vec<SequenceEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SequenceParticipant {
    id: String,
    label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SequenceGroupBox {
    actor_indices: Vec<usize>,
    label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SequenceEvent {
    Message(SequenceMessage),
    Note(SequenceNote),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SequenceMessage {
    from: usize,
    to: usize,
    label: String,
    style: SequenceLineStyle,
    arrow: SequenceArrowHead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SequenceLineStyle {
    Solid,
    Dotted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SequenceArrowHead {
    Filled,
    Open,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SequenceNote {
    from: usize,
    to: usize,
    label: String,
    placement: SequenceNotePlacement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SequenceNotePlacement {
    LeftOf,
    RightOf,
    Over,
}

impl SequenceNotePlacement {
    fn from_model(value: Option<i32>) -> Result<Self> {
        match value.unwrap_or(NOTE_PLACEMENT_OVER) {
            NOTE_PLACEMENT_LEFT_OF => Ok(Self::LeftOf),
            NOTE_PLACEMENT_RIGHT_OF => Ok(Self::RightOf),
            NOTE_PLACEMENT_OVER => Ok(Self::Over),
            _ => Err(AsciiError::UnsupportedFeature {
                diagram_type: "sequence",
                feature: "note placement",
            }),
        }
    }
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
    filled_arrow_right: char,
    filled_arrow_left: char,
    open_arrow_right: char,
    open_arrow_left: char,
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

    fn arrow_right(self, arrow: SequenceArrowHead) -> char {
        match arrow {
            SequenceArrowHead::Filled => self.filled_arrow_right,
            SequenceArrowHead::Open => self.open_arrow_right,
        }
    }

    fn arrow_left(self, arrow: SequenceArrowHead) -> char {
        match arrow {
            SequenceArrowHead::Filled => self.filled_arrow_left,
            SequenceArrowHead::Open => self.open_arrow_left,
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
    let boxes = sequence_boxes(model, &participant_index)?;
    let mut events = Vec::new();
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

        if message.message_type == NOTE_MESSAGE_TYPE {
            let placement = SequenceNotePlacement::from_model(message.placement)?;
            let label = message.message_text();
            if label.contains(['\r', '\n']) {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type: "sequence",
                    feature: "multiline notes",
                });
            }
            events.push(SequenceEvent::Note(SequenceNote {
                from,
                to,
                label: label.to_string(),
                placement,
            }));
            continue;
        }

        let (style, arrow) = match message.message_type {
            SOLID_FILLED_MESSAGE_TYPE => (SequenceLineStyle::Solid, SequenceArrowHead::Filled),
            DOTTED_FILLED_MESSAGE_TYPE => (SequenceLineStyle::Dotted, SequenceArrowHead::Filled),
            SOLID_OPEN_MESSAGE_TYPE => (SequenceLineStyle::Solid, SequenceArrowHead::Open),
            DOTTED_OPEN_MESSAGE_TYPE => (SequenceLineStyle::Dotted, SequenceArrowHead::Open),
            _ => {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type: "sequence",
                    feature: "message types",
                });
            }
        };
        let label = autonumber.label(message.message_text());

        events.push(SequenceEvent::Message(SequenceMessage {
            from,
            to,
            label,
            style,
            arrow,
        }));
    }

    Ok(AsciiSequenceDiagram {
        participants,
        boxes,
        events,
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

    if model.notes.iter().any(|note| note.wrap)
        || model
            .messages
            .iter()
            .any(|message| message.message_type == NOTE_MESSAGE_TYPE && message.wrap)
    {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "wrapped notes",
        });
    }

    let note_message_count = model
        .messages
        .iter()
        .filter(|message| message.message_type == NOTE_MESSAGE_TYPE)
        .count();
    if !model.notes.is_empty() && note_message_count < model.notes.len() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "notes without drawable messages",
        });
    }

    if model.boxes.iter().any(|sequence_box| sequence_box.wrap) {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "wrapped boxes",
        });
    }

    if model
        .boxes
        .iter()
        .any(|sequence_box| sequence_box.actor_keys.is_empty())
    {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "empty boxes",
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
        .any(|message| message.message_type != NOTE_MESSAGE_TYPE && message.placement.is_some())
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

fn sequence_boxes(
    model: &SequenceDiagramRenderModel,
    participant_index: &HashMap<&str, usize>,
) -> Result<Vec<SequenceGroupBox>> {
    model
        .boxes
        .iter()
        .map(|sequence_box| {
            let actor_indices = sequence_box
                .actor_keys
                .iter()
                .map(|actor_key| {
                    participant_index.get(actor_key.as_str()).copied().ok_or(
                        AsciiError::UnsupportedFeature {
                            diagram_type: "sequence",
                            feature: "boxes with unknown actors",
                        },
                    )
                })
                .collect::<Result<Vec<_>>>()?;

            let label = sequence_box
                .name
                .as_ref()
                .filter(|name| !name.is_empty())
                .cloned();

            Ok(SequenceGroupBox {
                actor_indices,
                label,
            })
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

    for event in &diagram.events {
        for _ in 0..layout.message_spacing {
            lines.push(build_lifeline(&layout, &chars));
        }

        match event {
            SequenceEvent::Message(message) => {
                if message.from == message.to {
                    lines.extend(render_self_message(message, &layout, &chars));
                } else {
                    lines.extend(render_message(message, &layout, &chars));
                }
            }
            SequenceEvent::Note(note) => lines.extend(render_note(note, &layout, &chars)),
        }
    }

    lines.push(build_lifeline(&layout, &chars));
    if !diagram.boxes.is_empty() {
        lines = render_sequence_boxes(lines, diagram, &layout, &chars);
    }
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
        line[to - 1] = chars.arrow_right(message.arrow);
        line[to] = chars.vertical;
    } else {
        line[to] = chars.vertical;
        line[to + 1] = chars.arrow_left(message.arrow);
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
    bottom[center + 1] = chars.arrow_left(message.arrow);
    for offset in 2..(width - 1) {
        bottom[center + offset] = chars.horizontal;
    }
    bottom[center + width - 1] = chars.self_bottom;
    lines.push(trim_right(bottom));

    lines
}

fn render_note(note: &SequenceNote, layout: &SequenceLayout, chars: &SequenceChars) -> Vec<String> {
    let label_width = display_width(&note.label);
    let mut inner_width = (label_width + BOX_PADDING_LEFT_RIGHT).max(MIN_BOX_WIDTH);
    let from = layout.participant_centers[note.from];
    let to = layout.participant_centers[note.to];

    let left = match note.placement {
        SequenceNotePlacement::LeftOf => {
            let total_width = inner_width + BOX_BORDER_WIDTH;
            from.saturating_sub(total_width + NOTE_SIDE_GAP)
        }
        SequenceNotePlacement::RightOf => from + NOTE_SIDE_GAP,
        SequenceNotePlacement::Over => {
            if from == to {
                let total_width = inner_width + BOX_BORDER_WIDTH;
                from.saturating_sub(total_width / 2)
            } else {
                let span_left = from.min(to).saturating_sub(1);
                let span_inner_width = from.abs_diff(to) + 1;
                inner_width = inner_width.max(span_inner_width);
                span_left
            }
        }
    };

    let label_left_padding = (inner_width - label_width) / 2;
    let label_right_padding = inner_width - label_left_padding - label_width;
    let top = format!(
        "{}{}{}",
        chars.top_left,
        chars.horizontal.to_string().repeat(inner_width),
        chars.top_right
    );
    let middle = format!(
        "{}{}{}{}{}",
        chars.vertical,
        " ".repeat(label_left_padding),
        note.label,
        " ".repeat(label_right_padding),
        chars.vertical
    );
    let bottom = format!(
        "{}{}{}",
        chars.bottom_left,
        chars.horizontal.to_string().repeat(inner_width),
        chars.bottom_right
    );

    vec![
        render_overlay_row(layout, chars, left, &top),
        render_overlay_row(layout, chars, left, &middle),
        render_overlay_row(layout, chars, left, &bottom),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SequenceGroupBoxBounds {
    left: usize,
    right: usize,
}

fn render_sequence_boxes(
    lines: Vec<String>,
    diagram: &AsciiSequenceDiagram,
    layout: &SequenceLayout,
    chars: &SequenceChars,
) -> Vec<String> {
    let bounds = diagram
        .boxes
        .iter()
        .map(|sequence_box| sequence_box_bounds(sequence_box, layout))
        .collect::<Vec<_>>();
    let content_width = lines
        .iter()
        .map(|line| line.chars().count() + SEQUENCE_BOX_CONTENT_OFFSET)
        .max()
        .unwrap_or(0);
    let box_width = bounds
        .iter()
        .map(|bounds| bounds.right + 1)
        .max()
        .unwrap_or(0);
    let width = content_width.max(box_width);

    let mut canvas = Vec::with_capacity(lines.len() + 2);
    canvas.push(vec![' '; width]);
    for line in lines {
        let mut row = Vec::with_capacity(width);
        row.extend(std::iter::repeat_n(' ', SEQUENCE_BOX_CONTENT_OFFSET));
        row.extend(line.chars());
        if row.len() < width {
            row.extend(std::iter::repeat_n(' ', width - row.len()));
        }
        canvas.push(row);
    }
    canvas.push(vec![' '; width]);

    for (sequence_box, bounds) in diagram.boxes.iter().zip(bounds) {
        draw_sequence_box(&mut canvas, sequence_box, bounds, chars);
    }

    canvas.into_iter().map(trim_right).collect()
}

fn sequence_box_bounds(
    sequence_box: &SequenceGroupBox,
    layout: &SequenceLayout,
) -> SequenceGroupBoxBounds {
    let mut left = usize::MAX;
    let mut right = 0;

    for actor_index in &sequence_box.actor_indices {
        let box_width = layout.participant_widths[*actor_index] + BOX_BORDER_WIDTH;
        let participant_left = layout.participant_centers[*actor_index] - box_width / 2;
        let participant_right = participant_left + box_width - 1;
        left = left.min((participant_left + SEQUENCE_BOX_CONTENT_OFFSET).saturating_sub(1));
        right = right.max(participant_right + SEQUENCE_BOX_CONTENT_OFFSET + 1);
    }

    if let Some(label) = &sequence_box.label {
        let label_right = left + display_width(label) + 2 * SEQUENCE_BOX_LABEL_MARGIN;
        right = right.max(label_right);
    }

    SequenceGroupBoxBounds { left, right }
}

fn draw_sequence_box(
    canvas: &mut [Vec<char>],
    sequence_box: &SequenceGroupBox,
    bounds: SequenceGroupBoxBounds,
    chars: &SequenceChars,
) {
    if canvas.is_empty() || bounds.left >= bounds.right {
        return;
    }

    let top = 0;
    let bottom = canvas.len() - 1;

    for x in bounds.left..=bounds.right {
        canvas[top][x] = chars.horizontal;
        canvas[bottom][x] = chars.horizontal;
    }
    canvas[top][bounds.left] = chars.top_left;
    canvas[top][bounds.right] = chars.top_right;
    canvas[bottom][bounds.left] = chars.bottom_left;
    canvas[bottom][bounds.right] = chars.bottom_right;

    for row in canvas.iter_mut().take(bottom).skip(top + 1) {
        row[bounds.left] = chars.vertical;
        row[bounds.right] = chars.vertical;
    }

    if let Some(label) = &sequence_box.label {
        let label = format!(" {label} ");
        let start = bounds.left + SEQUENCE_BOX_LABEL_MARGIN;
        for (offset, ch) in label.chars().enumerate() {
            let index = start + offset;
            if index < bounds.right {
                canvas[top][index] = ch;
            }
        }
    }
}

fn render_overlay_row(
    layout: &SequenceLayout,
    chars: &SequenceChars,
    left: usize,
    text: &str,
) -> String {
    let needed = left + text.chars().count();
    let mut line = padded_line(build_lifeline(layout, chars), needed);
    write_text(&mut line, left, text);
    trim_right(line)
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
