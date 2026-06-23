use super::{SEQUENCE_ACTOR_WRAP_TEXT_WIDTH, validate::validate_supported_sequence_model};
use crate::error::{AsciiError, Result};
use crate::text::{display_width, split_label_lines, wrap_label_lines};
use merman_core::diagrams::sequence::{
    SequenceDiagramRenderModel, SequenceMessage as CoreSequenceMessage, SequenceMessagePayload,
};
use std::collections::HashMap;

const AUTONUMBER_MESSAGE_TYPE: i32 = 26;
pub(super) const NOTE_MESSAGE_TYPE: i32 = 2;
pub(super) const ACTIVE_START_MESSAGE_TYPE: i32 = 17;
pub(super) const ACTIVE_END_MESSAGE_TYPE: i32 = 18;
const SOLID_FILLED_MESSAGE_TYPE: i32 = 0;
const DOTTED_FILLED_MESSAGE_TYPE: i32 = 1;
const SOLID_CROSS_MESSAGE_TYPE: i32 = 3;
const DOTTED_CROSS_MESSAGE_TYPE: i32 = 4;
const SOLID_OPEN_MESSAGE_TYPE: i32 = 5;
const DOTTED_OPEN_MESSAGE_TYPE: i32 = 6;
const LOOP_START_MESSAGE_TYPE: i32 = 10;
const LOOP_END_MESSAGE_TYPE: i32 = 11;
const ALT_START_MESSAGE_TYPE: i32 = 12;
const ALT_ELSE_MESSAGE_TYPE: i32 = 13;
const ALT_END_MESSAGE_TYPE: i32 = 14;
const OPT_START_MESSAGE_TYPE: i32 = 15;
const OPT_END_MESSAGE_TYPE: i32 = 16;
const PAR_START_MESSAGE_TYPE: i32 = 19;
const PAR_AND_MESSAGE_TYPE: i32 = 20;
const PAR_END_MESSAGE_TYPE: i32 = 21;
const RECT_START_MESSAGE_TYPE: i32 = 22;
const RECT_END_MESSAGE_TYPE: i32 = 23;
const CRITICAL_START_MESSAGE_TYPE: i32 = 27;
const CRITICAL_OPTION_MESSAGE_TYPE: i32 = 28;
const CRITICAL_END_MESSAGE_TYPE: i32 = 29;
const BREAK_START_MESSAGE_TYPE: i32 = 30;
const BREAK_END_MESSAGE_TYPE: i32 = 31;
const PAR_OVER_START_MESSAGE_TYPE: i32 = 32;
const NOTE_PLACEMENT_LEFT_OF: i32 = 0;
const NOTE_PLACEMENT_RIGHT_OF: i32 = 1;
const NOTE_PLACEMENT_OVER: i32 = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AsciiSequenceDiagram {
    pub(super) title: Option<String>,
    pub(super) participants: Vec<SequenceParticipant>,
    pub(super) lifecycles: Vec<SequenceActorLifecycle>,
    pub(super) boxes: Vec<SequenceGroupBox>,
    pub(super) events: Vec<SequenceEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SequenceParticipant {
    pub(super) id: String,
    pub(super) label: SequenceParticipantLabel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SequenceParticipantLabel {
    lines: Vec<String>,
    width: usize,
}

impl SequenceParticipantLabel {
    pub(super) fn from_raw(raw: &str, wrap: bool) -> Self {
        let lines = if wrap {
            wrap_label_lines(raw, SEQUENCE_ACTOR_WRAP_TEXT_WIDTH)
        } else {
            split_label_lines(raw)
        };
        Self::from_lines(lines)
    }

    pub(super) fn lines(&self) -> &[String] {
        &self.lines
    }

    pub(super) fn width(&self) -> usize {
        self.width
    }

    fn from_lines(mut lines: Vec<String>) -> Self {
        if lines.is_empty() {
            lines.push(String::new());
        }
        let width = lines
            .iter()
            .map(|line| display_width(line))
            .max()
            .unwrap_or_default();
        Self { lines, width }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SequenceGroupBox {
    pub(super) actor_indices: Vec<usize>,
    pub(super) label: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct SequenceActorLifecycle {
    pub(super) created_at: Option<usize>,
    pub(super) destroyed_at: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum SequenceEvent {
    Message(SequenceMessage),
    Note(SequenceNote),
    ActivationStart {
        actor: usize,
        model_index: usize,
    },
    ActivationEnd {
        actor: usize,
        model_index: usize,
    },
    ControlStart(SequenceControlStart),
    ControlEnd {
        kind: SequenceControlKind,
        model_index: usize,
    },
    ControlSeparator(SequenceControlSeparator),
}

impl SequenceEvent {
    pub(super) fn model_index(&self) -> usize {
        match self {
            Self::Message(message) => message.model_index,
            Self::Note(note) => note.model_index,
            Self::ActivationStart { model_index, .. } | Self::ActivationEnd { model_index, .. } => {
                *model_index
            }
            Self::ControlStart(start) => start.model_index,
            Self::ControlEnd { model_index, .. } => *model_index,
            Self::ControlSeparator(separator) => separator.model_index,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SequenceControlStart {
    pub(super) model_index: usize,
    pub(super) kind: SequenceControlKind,
    pub(super) label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SequenceControlSeparator {
    pub(super) model_index: usize,
    pub(super) kind: SequenceControlKind,
    pub(super) label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SequenceControlKind {
    Loop,
    Opt,
    Break,
    Alt,
    Par,
    Critical,
    Rect,
    ParOver,
}

impl SequenceControlKind {
    pub(super) fn keyword(self) -> &'static str {
        match self {
            Self::Loop => "loop",
            Self::Opt => "opt",
            Self::Break => "break",
            Self::Alt => "alt",
            Self::Par => "par",
            Self::Critical => "critical",
            Self::Rect => "rect",
            Self::ParOver => "par_over",
        }
    }

    pub(super) fn separator_keyword(self) -> Option<&'static str> {
        match self {
            Self::Alt => Some("else"),
            Self::Par => Some("and"),
            Self::Critical => Some("option"),
            Self::Loop | Self::Opt | Self::Break | Self::Rect | Self::ParOver => None,
        }
    }

    pub(super) fn accepts_end(self, end: Self) -> bool {
        self == end || matches!((self, end), (Self::ParOver, Self::Par))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SequenceMessage {
    pub(super) model_index: usize,
    pub(super) from: usize,
    pub(super) to: usize,
    pub(super) label: String,
    pub(super) wrap: bool,
    pub(super) style: SequenceLineStyle,
    pub(super) arrow: SequenceArrowHead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SequenceLineStyle {
    Solid,
    Dotted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SequenceArrowHead {
    Filled,
    Open,
    Cross,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SequenceNote {
    pub(super) model_index: usize,
    pub(super) from: usize,
    pub(super) to: usize,
    pub(super) label: String,
    pub(super) wrap: bool,
    pub(super) placement: SequenceNotePlacement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SequenceNotePlacement {
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
    let lifecycles = sequence_actor_lifecycles(model, &participant_index)?;
    let mut events = Vec::new();
    let mut autonumber = AutonumberState::default();

    for (model_index, message) in model.messages.iter().enumerate() {
        if consume_autonumber(message, &mut autonumber) {
            continue;
        }

        if let Some(event) = sequence_control_event(message, model_index)? {
            events.push(event);
            continue;
        }

        if matches!(
            message.message_type,
            ACTIVE_START_MESSAGE_TYPE | ACTIVE_END_MESSAGE_TYPE
        ) {
            let actor = message
                .from
                .as_deref()
                .ok_or(AsciiError::UnsupportedFeature {
                    diagram_type: "sequence",
                    feature: "control messages",
                })?;
            let actor = *participant_index
                .get(actor)
                .ok_or(AsciiError::UnsupportedFeature {
                    diagram_type: "sequence",
                    feature: "messages with unknown actors",
                })?;
            let event = if message.message_type == ACTIVE_START_MESSAGE_TYPE {
                SequenceEvent::ActivationStart { actor, model_index }
            } else {
                SequenceEvent::ActivationEnd { actor, model_index }
            };
            events.push(event);
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
            events.push(SequenceEvent::Note(SequenceNote {
                model_index,
                from,
                to,
                label: label.to_string(),
                wrap: message.wrap,
                placement,
            }));
            continue;
        }

        let (style, arrow) = match message.message_type {
            SOLID_FILLED_MESSAGE_TYPE => (SequenceLineStyle::Solid, SequenceArrowHead::Filled),
            DOTTED_FILLED_MESSAGE_TYPE => (SequenceLineStyle::Dotted, SequenceArrowHead::Filled),
            SOLID_CROSS_MESSAGE_TYPE => (SequenceLineStyle::Solid, SequenceArrowHead::Cross),
            DOTTED_CROSS_MESSAGE_TYPE => (SequenceLineStyle::Dotted, SequenceArrowHead::Cross),
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
            model_index,
            from,
            to,
            label,
            wrap: message.wrap,
            style,
            arrow,
        }));
    }

    Ok(AsciiSequenceDiagram {
        title: model
            .title
            .as_ref()
            .filter(|title| !title.is_empty())
            .cloned(),
        participants,
        lifecycles,
        boxes,
        events,
    })
}

fn sequence_control_event(
    message: &CoreSequenceMessage,
    model_index: usize,
) -> Result<Option<SequenceEvent>> {
    let kind = match message.message_type {
        LOOP_START_MESSAGE_TYPE => Some((SequenceControlKind::Loop, true)),
        LOOP_END_MESSAGE_TYPE => Some((SequenceControlKind::Loop, false)),
        ALT_START_MESSAGE_TYPE => Some((SequenceControlKind::Alt, true)),
        ALT_END_MESSAGE_TYPE => Some((SequenceControlKind::Alt, false)),
        OPT_START_MESSAGE_TYPE => Some((SequenceControlKind::Opt, true)),
        OPT_END_MESSAGE_TYPE => Some((SequenceControlKind::Opt, false)),
        PAR_START_MESSAGE_TYPE => Some((SequenceControlKind::Par, true)),
        PAR_END_MESSAGE_TYPE => Some((SequenceControlKind::Par, false)),
        RECT_START_MESSAGE_TYPE => Some((SequenceControlKind::Rect, true)),
        RECT_END_MESSAGE_TYPE => Some((SequenceControlKind::Rect, false)),
        CRITICAL_START_MESSAGE_TYPE => Some((SequenceControlKind::Critical, true)),
        CRITICAL_END_MESSAGE_TYPE => Some((SequenceControlKind::Critical, false)),
        BREAK_START_MESSAGE_TYPE => Some((SequenceControlKind::Break, true)),
        BREAK_END_MESSAGE_TYPE => Some((SequenceControlKind::Break, false)),
        PAR_OVER_START_MESSAGE_TYPE => Some((SequenceControlKind::ParOver, true)),
        _ => None,
    };

    let separator_kind = match message.message_type {
        ALT_ELSE_MESSAGE_TYPE => Some(SequenceControlKind::Alt),
        PAR_AND_MESSAGE_TYPE => Some(SequenceControlKind::Par),
        CRITICAL_OPTION_MESSAGE_TYPE => Some(SequenceControlKind::Critical),
        _ => None,
    };

    let Some((kind, is_start)) = kind else {
        if let Some(kind) = separator_kind {
            ensure_endpointless_control_message(message)?;
            return Ok(Some(SequenceEvent::ControlSeparator(
                SequenceControlSeparator {
                    model_index,
                    kind,
                    label: message.message_text().to_string(),
                },
            )));
        }
        return Ok(None);
    };

    ensure_endpointless_control_message(message)?;

    if is_start {
        Ok(Some(SequenceEvent::ControlStart(SequenceControlStart {
            model_index,
            kind,
            label: message.message_text().to_string(),
        })))
    } else {
        Ok(Some(SequenceEvent::ControlEnd { kind, model_index }))
    }
}

fn ensure_endpointless_control_message(message: &CoreSequenceMessage) -> Result<()> {
    if message.from.is_some() || message.to.is_some() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "control messages",
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
            let raw_label = if actor.description.is_empty() {
                if actor.name.is_empty() {
                    id.clone()
                } else {
                    actor.name.clone()
                }
            } else {
                actor.description.clone()
            };
            let label = SequenceParticipantLabel::from_raw(&raw_label, actor.wrap);
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

fn sequence_actor_lifecycles(
    model: &SequenceDiagramRenderModel,
    participant_index: &HashMap<&str, usize>,
) -> Result<Vec<SequenceActorLifecycle>> {
    let mut lifecycles = vec![SequenceActorLifecycle::default(); participant_index.len()];

    for (actor_id, model_index) in &model.created_actors {
        let actor_index =
            actor_lifecycle_index(participant_index, actor_id, "actor lifecycle actors")?;
        let message =
            actor_lifecycle_message(model, *model_index, "actor lifecycle message indices")?;
        if message.to.as_deref() != Some(actor_id.as_str()) {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "sequence",
                feature: "actor creation messages",
            });
        }
        lifecycles[actor_index].created_at = Some(*model_index);
    }

    for (actor_id, model_index) in &model.destroyed_actors {
        let actor_index =
            actor_lifecycle_index(participant_index, actor_id, "actor lifecycle actors")?;
        let message =
            actor_lifecycle_message(model, *model_index, "actor lifecycle message indices")?;
        if message.from.as_deref() != Some(actor_id.as_str())
            && message.to.as_deref() != Some(actor_id.as_str())
        {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "sequence",
                feature: "actor destruction messages",
            });
        }
        lifecycles[actor_index].destroyed_at = Some(*model_index);
    }

    for lifecycle in &lifecycles {
        if let (Some(created_at), Some(destroyed_at)) =
            (lifecycle.created_at, lifecycle.destroyed_at)
            && destroyed_at <= created_at
        {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "sequence",
                feature: "actor lifecycle order",
            });
        }
    }

    Ok(lifecycles)
}

fn actor_lifecycle_index(
    participant_index: &HashMap<&str, usize>,
    actor_id: &str,
    feature: &'static str,
) -> Result<usize> {
    participant_index
        .get(actor_id)
        .copied()
        .ok_or(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature,
        })
}

fn actor_lifecycle_message<'a>(
    model: &'a SequenceDiagramRenderModel,
    model_index: usize,
    feature: &'static str,
) -> Result<&'a CoreSequenceMessage> {
    model
        .messages
        .get(model_index)
        .ok_or(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature,
        })
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct AutonumberState {
    next: Option<f64>,
    step: f64,
}

impl AutonumberState {
    fn label(&mut self, text: &str) -> String {
        if let Some(next) = self.next {
            let number = format_sequence_number(next);
            let label = if text.is_empty() {
                number
            } else {
                format!("{number}. {text}")
            };
            self.next = Some(round_sequence_number(next + self.step));
            return label;
        }
        text.to_string()
    }
}

fn round_sequence_number(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn format_sequence_number(value: f64) -> String {
    if value.is_finite() {
        value.to_string()
    } else {
        String::new()
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
        state.next = Some(autonumber.start.unwrap_or(1.0));
        state.step = autonumber.step.unwrap_or(1.0);
    } else {
        state.next = None;
        state.step = 1.0;
    }
    true
}
