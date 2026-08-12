use super::{
    SEQUENCE_ACTOR_WRAP_TEXT_WIDTH, lifecycle::resolve_actor_lifecycles,
    projection_allocation_failed, validate::validate_supported_sequence_model,
};
use crate::color::AsciiRgb;
use crate::error::{AsciiError, Result};
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use crate::safe_text::{
    LabelBreakPolicy, charge_text_layout, try_plan_normalized_label_lines_with_policy,
};
use crate::style_color::{CssColor, parse_css_color, parse_css_color_value};
use merman_core::diagrams::sequence::{
    SequenceCentralDecoration as CoreSequenceCentralDecoration, SequenceDiagramRenderModel,
    SequenceMessage as CoreSequenceMessage,
    SequenceMessageDirection as CoreSequenceMessageDirection,
    SequenceMessageKind as CoreSequenceMessageKind,
    SequenceMessageMarker as CoreSequenceMessageMarker, SequenceMessagePayload,
    SequenceMessageStroke as CoreSequenceMessageStroke,
};
use std::collections::HashMap;

pub(super) use super::lifecycle::SequenceActorLifecycle;

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
    raw: String,
    wrap_width: Option<usize>,
    width_profile: TerminalWidthProfile,
    metrics: SequenceParticipantLabelMetrics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SequenceParticipantLabelMetrics {
    materialized_bytes: usize,
    document_cells: usize,
    line_count: usize,
    max_width: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MaterializedSequenceParticipantLabel {
    lines: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PreparedSequenceParticipantLabel<'a> {
    label: &'a SequenceParticipantLabel,
    plan: crate::safe_text::NormalizedLabelPlan,
}

impl SequenceParticipantLabel {
    pub(super) fn try_from_raw(
        raw: &str,
        wrap: bool,
        width_profile: TerminalWidthProfile,
        resources: &mut ResourceContext,
    ) -> Result<Self> {
        let wrap_width = wrap.then_some(SEQUENCE_ACTOR_WRAP_TEXT_WIDTH);
        let plan = try_plan_normalized_label_lines_with_policy(
            raw,
            width_profile,
            false,
            wrap_width,
            LabelBreakPolicy::MermaidLabelBreaks,
            resources,
        )?
        .expect("non-trimmed labels always retain one row");
        plan.check_materialization_limits(resources)?;
        let metrics = plan.metrics();
        resources.grid_extent(metrics.max_width.max(1), metrics.line_count)?;
        resources.check(
            AsciiResourceLimitId::MaxDocumentCells,
            metrics.document_cells,
        )?;
        resources.check(AsciiResourceLimitId::MaxOutputBytes, raw.len())?;
        Ok(Self {
            raw: try_clone_projection_string(raw)?,
            wrap_width,
            width_profile,
            metrics: SequenceParticipantLabelMetrics {
                materialized_bytes: metrics.materialized_bytes,
                document_cells: metrics.document_cells,
                line_count: metrics.line_count,
                max_width: metrics.max_width,
            },
        })
    }

    #[cfg(test)]
    pub(super) fn from_raw(raw: &str, wrap: bool, width_profile: TerminalWidthProfile) -> Self {
        let policy = crate::resource::AsciiResourcePolicy::for_profile(
            merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
        );
        let mut resources = ResourceContext::new(policy);
        Self::try_from_raw(raw, wrap, width_profile, &mut resources)
            .expect("test participant label should fit the unbounded resource policy")
    }

    pub(super) fn line_count(&self) -> usize {
        self.metrics.line_count
    }

    pub(super) fn width(&self) -> usize {
        self.metrics.max_width
    }

    pub(super) fn prepare_materialization(
        &self,
        resources: &ResourceContext,
    ) -> Result<PreparedSequenceParticipantLabel<'_>> {
        let plan = try_plan_normalized_label_lines_with_policy(
            &self.raw,
            self.width_profile,
            false,
            self.wrap_width,
            LabelBreakPolicy::MermaidLabelBreaks,
            resources,
        )?
        .expect("non-trimmed labels always retain one row");
        plan.check_materialization_limits(resources)?;
        let metrics = plan.metrics();
        if metrics.materialized_bytes != self.metrics.materialized_bytes
            || metrics.document_cells != self.metrics.document_cells
            || metrics.line_count != self.metrics.line_count
            || metrics.max_width != self.metrics.max_width
        {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "sequence",
                feature: "participant label plan",
            });
        }
        Ok(PreparedSequenceParticipantLabel { label: self, plan })
    }
}

impl PreparedSequenceParticipantLabel<'_> {
    pub(super) const fn materialization_work_units(self) -> usize {
        self.plan.materialization_work_units()
    }

    pub(super) fn materialize_after_admission(
        self,
    ) -> Result<MaterializedSequenceParticipantLabel> {
        let (lines, width) = self
            .plan
            .materialize_after_admission(&self.label.raw)?
            .into_parts();
        if lines.len() != self.label.metrics.line_count || width != self.label.metrics.max_width {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "sequence",
                feature: "participant label plan",
            });
        }
        Ok(MaterializedSequenceParticipantLabel { lines })
    }
}

impl MaterializedSequenceParticipantLabel {
    pub(super) fn lines(&self) -> &[String] {
        &self.lines
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SequenceGroupBox {
    pub(super) actor_indices: Vec<usize>,
    pub(super) label: Option<String>,
    pub(super) background: Option<AsciiRgb>,
    pub(super) wrap: bool,
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
    pub(super) background: Option<AsciiRgb>,
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
    pub(super) source_marker: SequenceArrowHead,
    pub(super) target_marker: SequenceArrowHead,
    pub(super) direction: SequenceMessageDirection,
    pub(super) central_decoration: SequenceCentralDecoration,
}

pub(super) type SequenceLineStyle = CoreSequenceMessageStroke;
pub(super) type SequenceArrowHead = CoreSequenceMessageMarker;
pub(super) type SequenceMessageDirection = CoreSequenceMessageDirection;
pub(super) type SequenceCentralDecoration = CoreSequenceCentralDecoration;

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

fn preflight_sequence_projection(
    model: &SequenceDiagramRenderModel,
    resources: &mut ResourceContext,
) -> Result<()> {
    resources.charge_layout_work(1)?;
    if let Some(title) = model.title.as_deref() {
        charge_text_layout(resources, title)?;
    }

    for actor_id in &model.actor_order {
        resources.charge_layout_work(1)?;
        charge_text_layout(resources, actor_id)?;
    }
    for (actor_id, actor) in &model.actors {
        resources.charge_layout_work(1)?;
        charge_text_layout(resources, actor_id)?;
        charge_text_layout(resources, &actor.name)?;
        charge_text_layout(resources, &actor.description)?;
    }
    for sequence_box in &model.boxes {
        resources.charge_layout_work(1)?;
        charge_text_layout(resources, &sequence_box.fill)?;
        if let Some(name) = sequence_box.name.as_deref() {
            charge_text_layout(resources, name)?;
        }
        for actor_key in &sequence_box.actor_keys {
            resources.charge_layout_work(1)?;
            charge_text_layout(resources, actor_key)?;
        }
    }

    let mut nesting_depth = 0usize;
    for message in &model.messages {
        resources.charge_layout_work(1)?;
        if let Some(from) = message.from.as_deref() {
            charge_text_layout(resources, from)?;
        }
        if let Some(to) = message.to.as_deref() {
            charge_text_layout(resources, to)?;
        }
        charge_text_layout(resources, message.message_text())?;

        if is_control_start_message(message.message_type) {
            nesting_depth = nesting_depth.checked_add(1).ok_or_else(|| {
                resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxNestingDepth)
            })?;
            resources.check_nesting_depth(nesting_depth)?;
        } else if is_control_end_message(message.message_type) {
            nesting_depth = nesting_depth.saturating_sub(1);
        }
    }

    for actor_id in model
        .created_actors
        .keys()
        .chain(model.destroyed_actors.keys())
    {
        resources.charge_layout_work(1)?;
        charge_text_layout(resources, actor_id)?;
    }
    Ok(())
}

fn is_control_start_message(message_type: i32) -> bool {
    matches!(
        message_type,
        LOOP_START_MESSAGE_TYPE
            | ALT_START_MESSAGE_TYPE
            | OPT_START_MESSAGE_TYPE
            | PAR_START_MESSAGE_TYPE
            | RECT_START_MESSAGE_TYPE
            | CRITICAL_START_MESSAGE_TYPE
            | BREAK_START_MESSAGE_TYPE
            | PAR_OVER_START_MESSAGE_TYPE
    )
}

fn is_control_end_message(message_type: i32) -> bool {
    matches!(
        message_type,
        LOOP_END_MESSAGE_TYPE
            | ALT_END_MESSAGE_TYPE
            | OPT_END_MESSAGE_TYPE
            | PAR_END_MESSAGE_TYPE
            | RECT_END_MESSAGE_TYPE
            | CRITICAL_END_MESSAGE_TYPE
            | BREAK_END_MESSAGE_TYPE
    )
}

pub(crate) fn from_sequence_model(
    model: &SequenceDiagramRenderModel,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<AsciiSequenceDiagram> {
    preflight_sequence_projection(model, resources)?;
    validate_supported_sequence_model(model)?;

    let participants = sequence_participants(model, width_profile, resources)?;
    if participants.is_empty() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "no participants",
        });
    }

    let mut participant_index = HashMap::new();
    participant_index
        .try_reserve(participants.len())
        .map_err(|_| projection_allocation_failed())?;
    for (index, participant) in participants.iter().enumerate() {
        participant_index.insert(participant.id.as_str(), index);
    }
    let boxes = sequence_boxes(model, &participant_index)?;
    let lifecycles = resolve_actor_lifecycles(model, &participant_index, resources)?;
    let mut events = Vec::new();
    events
        .try_reserve_exact(model.messages.len())
        .map_err(|_| projection_allocation_failed())?;
    let mut autonumber = AutonumberState::default();

    for (model_index, message) in model.messages.iter().enumerate() {
        let semantic_kind = message.semantic_kind();
        if consume_autonumber(message, &mut autonumber) {
            continue;
        }

        if semantic_kind == CoreSequenceMessageKind::CentralDecorationRecord {
            continue;
        }

        if let Some(event) = sequence_control_event(message, model_index)? {
            events.push(event);
            continue;
        }

        if matches!(
            semantic_kind,
            CoreSequenceMessageKind::ActivationStart | CoreSequenceMessageKind::ActivationEnd
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
            let event = if semantic_kind == CoreSequenceMessageKind::ActivationStart {
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

        if semantic_kind == CoreSequenceMessageKind::Note {
            let placement = SequenceNotePlacement::from_model(message.placement)?;
            let label = message.message_text();
            events.push(SequenceEvent::Note(SequenceNote {
                model_index,
                from,
                to,
                label: try_clone_projection_string(label)?,
                wrap: message.wrap,
                placement,
            }));
            continue;
        }

        if semantic_kind != CoreSequenceMessageKind::Signal {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "sequence",
                feature: "message types",
            });
        }

        let semantics = message
            .signal_semantics()
            .ok_or(AsciiError::UnsupportedFeature {
                diagram_type: "sequence",
                feature: "message types",
            })?;
        let central_decoration =
            message
                .central_decoration()
                .ok_or(AsciiError::UnsupportedFeature {
                    diagram_type: "sequence",
                    feature: "central connection types",
                })?;
        let label = autonumber.label(message.message_text())?;

        events.push(SequenceEvent::Message(SequenceMessage {
            model_index,
            from,
            to,
            label,
            wrap: message.wrap,
            style: semantics.stroke,
            source_marker: semantics.source_marker,
            target_marker: semantics.target_marker,
            direction: semantics.direction,
            central_decoration,
        }));
    }

    Ok(AsciiSequenceDiagram {
        title: model
            .title
            .as_deref()
            .filter(|title| !title.is_empty())
            .map(try_clone_projection_string)
            .transpose()?,
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
                    label: try_clone_projection_string(message.message_text())?,
                },
            )));
        }
        return Ok(None);
    };

    ensure_endpointless_control_message(message)?;

    if is_start {
        let raw_label = message.message_text();
        let (label, background) = sequence_control_start_label(kind, raw_label)?;
        Ok(Some(SequenceEvent::ControlStart(SequenceControlStart {
            model_index,
            kind,
            label,
            background,
        })))
    } else {
        Ok(Some(SequenceEvent::ControlEnd { kind, model_index }))
    }
}

fn sequence_control_start_label(
    kind: SequenceControlKind,
    raw_label: &str,
) -> Result<(String, Option<AsciiRgb>)> {
    if kind != SequenceControlKind::Rect {
        return Ok((try_clone_projection_string(raw_label)?, None));
    }

    Ok(match parse_css_color_value(raw_label) {
        Some(CssColor::Rgb(color)) => (String::new(), Some(color)),
        Some(CssColor::Transparent) => (String::new(), None),
        None => (try_clone_projection_string(raw_label)?, None),
    })
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

fn sequence_participants(
    model: &SequenceDiagramRenderModel,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<Vec<SequenceParticipant>> {
    let expected = if model.actor_order.is_empty() {
        model.actors.len()
    } else {
        model.actor_order.len()
    };
    let mut participants = Vec::new();
    participants
        .try_reserve_exact(expected)
        .map_err(|_| projection_allocation_failed())?;

    if model.actor_order.is_empty() {
        for id in model.actors.keys() {
            push_sequence_participant(&mut participants, model, id, width_profile, resources)?;
        }
    } else {
        for id in &model.actor_order {
            push_sequence_participant(&mut participants, model, id, width_profile, resources)?;
        }
    }

    Ok(participants)
}

fn push_sequence_participant(
    participants: &mut Vec<SequenceParticipant>,
    model: &SequenceDiagramRenderModel,
    id: &str,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> Result<()> {
    let actor = model.actors.get(id).ok_or(AsciiError::UnsupportedFeature {
        diagram_type: "sequence",
        feature: "actor order",
    })?;
    let raw_label = if actor.description.is_empty() {
        if actor.name.is_empty() {
            id
        } else {
            &actor.name
        }
    } else {
        &actor.description
    };
    let label =
        SequenceParticipantLabel::try_from_raw(raw_label, actor.wrap, width_profile, resources)?;
    participants.push(SequenceParticipant {
        id: try_clone_projection_string(id)?,
        label,
    });
    Ok(())
}

fn sequence_boxes(
    model: &SequenceDiagramRenderModel,
    participant_index: &HashMap<&str, usize>,
) -> Result<Vec<SequenceGroupBox>> {
    let mut boxes = Vec::new();
    boxes
        .try_reserve_exact(model.boxes.len())
        .map_err(|_| projection_allocation_failed())?;
    for sequence_box in &model.boxes {
        let mut actor_indices = Vec::new();
        actor_indices
            .try_reserve_exact(sequence_box.actor_keys.len())
            .map_err(|_| projection_allocation_failed())?;
        for actor_key in &sequence_box.actor_keys {
            actor_indices.push(participant_index.get(actor_key.as_str()).copied().ok_or(
                AsciiError::UnsupportedFeature {
                    diagram_type: "sequence",
                    feature: "boxes with unknown actors",
                },
            )?);
        }

        let label = sequence_box
            .name
            .as_deref()
            .filter(|name| !name.is_empty())
            .map(try_clone_projection_string)
            .transpose()?;

        boxes.push(SequenceGroupBox {
            actor_indices,
            label,
            background: parse_css_color(&sequence_box.fill),
            wrap: sequence_box.wrap,
        });
    }
    Ok(boxes)
}

fn try_clone_projection_string(value: &str) -> Result<String> {
    let mut output = String::new();
    output
        .try_reserve_exact(value.len())
        .map_err(|_| projection_allocation_failed())?;
    output.push_str(value);
    Ok(output)
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct AutonumberState {
    visible: bool,
    next: f64,
    step: f64,
}

impl Default for AutonumberState {
    fn default() -> Self {
        Self {
            visible: false,
            next: 1.0,
            step: 1.0,
        }
    }
}

impl AutonumberState {
    fn label(&mut self, text: &str) -> Result<String> {
        let next = self.next;
        self.next = round_sequence_number(next + self.step);

        if self.visible {
            let number = format_sequence_number(next);
            let label = if text.is_empty() {
                number
            } else {
                let capacity = number
                    .len()
                    .checked_add(2)
                    .and_then(|value| value.checked_add(text.len()))
                    .ok_or_else(projection_allocation_failed)?;
                let mut label = String::new();
                label
                    .try_reserve_exact(capacity)
                    .map_err(|_| projection_allocation_failed())?;
                label.push_str(&number);
                label.push_str(". ");
                label.push_str(text);
                label
            };
            return Ok(label);
        }
        try_clone_projection_string(text)
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

    if message.semantic_kind() != CoreSequenceMessageKind::Autonumber {
        return false;
    }

    if let Some(start) = autonumber.start {
        state.next = start;
    }
    if let Some(step) = autonumber.step {
        state.step = step;
    }
    state.visible = autonumber.visible;
    true
}

#[cfg(test)]
mod tests {
    use super::SequenceParticipantLabel;
    use crate::error::AsciiError;
    use crate::options::TerminalWidthProfile;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy, ResourceContext};
    use merman_core::resources::ResourceProfile;

    #[test]
    fn participant_label_checks_its_grid_extent_before_projection_clone() {
        let raw = "Alpha Beta<br><br>Gamma Delta";
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut measured_resources = ResourceContext::new(unbounded);
        let measured = SequenceParticipantLabel::try_from_raw(
            raw,
            true,
            TerminalWidthProfile::Unicode,
            &mut measured_resources,
        )
        .expect("unbounded participant-label plan should pass");
        let required_cells = measured.metrics.max_width * measured.metrics.line_count;
        assert!(required_cells > 0);

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxGridCells, required_cells)
            .expect("exact participant-label grid limit should be valid");
        let mut exact_resources = ResourceContext::new(exact_policy);
        SequenceParticipantLabel::try_from_raw(
            raw,
            true,
            TerminalWidthProfile::Unicode,
            &mut exact_resources,
        )
        .expect("exact participant-label grid extent should pass");

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxGridCells, required_cells - 1)
            .expect("max-minus-one participant-label grid limit should be valid");
        let mut below_resources = ResourceContext::new(below_policy);
        let error = SequenceParticipantLabel::try_from_raw(
            raw,
            true,
            TerminalWidthProfile::Unicode,
            &mut below_resources,
        )
        .expect_err("max-minus-one participant-label grid extent should reject");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGridCells
                    && details.actual == required_cells
                    && details.max == required_cells - 1
        ));
    }
}
