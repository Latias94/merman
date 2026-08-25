use super::tree::{SequenceBody, SequenceTreeBuilder};
use super::{
    SEQUENCE_ACTOR_WRAP_TEXT_WIDTH, SequenceCheckpointCursor, charge_sequence_projection_text,
    lifecycle::resolve_actor_lifecycles, projection_allocation_failed, try_plan_sequence_label,
    try_plan_sequence_projection_label, validate::validate_supported_sequence_model,
};
use crate::color::AsciiRgb;
use crate::error::{AsciiError, Result};
use crate::operation::AsciiExecution;
use crate::options::TerminalWidthProfile;
use crate::resource::{AsciiResourceLimitId, ResourceContext};
use crate::safe_text::LabelBreakPolicy;
use crate::style_color::{CssColor, parse_css_color, parse_css_color_value};
use merman_core::OperationPhase;
use merman_core::diagrams::sequence::{
    SequenceCentralDecoration as CoreSequenceCentralDecoration, SequenceControlRole,
    SequenceDiagramRenderModel, SequenceMessage as CoreSequenceMessage,
    SequenceMessageDirection as CoreSequenceMessageDirection,
    SequenceMessageKind as CoreSequenceMessageKind,
    SequenceMessageMarker as CoreSequenceMessageMarker, SequenceMessagePayload,
    SequenceMessageStroke as CoreSequenceMessageStroke,
};
use std::collections::HashMap;

pub(super) use super::lifecycle::SequenceActorLifecycle;
pub(super) use merman_core::diagrams::sequence::{SequenceControlKind, SequenceNotePlacement};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AsciiSequenceDiagram {
    pub(super) participants: Vec<SequenceParticipant>,
    pub(super) lifecycles: Vec<SequenceActorLifecycle>,
    pub(super) boxes: Vec<SequenceGroupBox>,
    pub(super) body: SequenceBody,
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
        execution: AsciiExecution<'_>,
    ) -> Result<Self> {
        let transaction = resources.clone();
        transaction.transaction(|_| {
            let wrap_width = wrap.then_some(SEQUENCE_ACTOR_WRAP_TEXT_WIDTH);
            let plan = try_plan_sequence_projection_label(
                raw,
                width_profile,
                false,
                wrap_width,
                LabelBreakPolicy::MermaidLabelBreaks,
                resources,
                execution,
            )?
            .expect("non-trimmed labels always retain one row");
            execution.checkpoint(OperationPhase::Semantic)?;
            plan.check_materialization_limits(resources)?;
            let metrics = plan.metrics();
            execution.checkpoint(OperationPhase::Semantic)?;
            resources.grid_extent(metrics.max_width.max(1), metrics.line_count)?;
            execution.checkpoint(OperationPhase::Semantic)?;
            resources.check(
                AsciiResourceLimitId::MaxDocumentCells,
                metrics.document_cells,
            )?;
            execution.checkpoint(OperationPhase::Semantic)?;
            resources.check(AsciiResourceLimitId::MaxOutputBytes, raw.len())?;
            Ok(Self {
                raw: try_clone_projection_string(raw, execution)?,
                wrap_width,
                width_profile,
                metrics: SequenceParticipantLabelMetrics {
                    materialized_bytes: metrics.materialized_bytes,
                    document_cells: metrics.document_cells,
                    line_count: metrics.line_count,
                    max_width: metrics.max_width,
                },
            })
        })
    }

    #[cfg(test)]
    pub(super) fn from_raw(raw: &str, wrap: bool, width_profile: TerminalWidthProfile) -> Self {
        let policy = crate::resource::AsciiResourcePolicy::for_profile(
            merman_core::resources::ResourceProfile::UnboundedForTrustedInput,
        );
        let mut resources = ResourceContext::new(policy);
        let execution = AsciiExecution::for_test(&policy);
        Self::try_from_raw(raw, wrap, width_profile, &mut resources, execution)
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
        checkpoints: &SequenceCheckpointCursor<'_>,
    ) -> Result<PreparedSequenceParticipantLabel<'_>> {
        let plan = try_plan_sequence_label(
            &self.raw,
            self.width_profile,
            false,
            self.wrap_width,
            LabelBreakPolicy::MermaidLabelBreaks,
            resources,
            checkpoints,
        )?
        .expect("non-trimmed labels always retain one row");
        checkpoints.before_charge()?;
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
        checkpoints: &SequenceCheckpointCursor<'_>,
    ) -> Result<MaterializedSequenceParticipantLabel> {
        checkpoints.checkpoint()?;
        let materialized = self
            .plan
            .materialize_after_admission_with_checkpoint(&self.label.raw, || {
                checkpoints.checkpoint()
            });
        checkpoints.checkpoint()?;
        let (lines, width) = materialized?.into_parts();
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
    ActivationStart { actor: usize, model_index: usize },
    ActivationEnd { actor: usize, model_index: usize },
}

impl SequenceEvent {
    pub(super) fn model_index(&self) -> usize {
        match self {
            Self::Message(message) => message.model_index,
            Self::Note(note) => note.model_index,
            Self::ActivationStart { model_index, .. } | Self::ActivationEnd { model_index, .. } => {
                *model_index
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SequenceControlRecord {
    Start {
        model_index: usize,
        kind: SequenceControlKind,
        label: String,
        background: Option<AsciiRgb>,
    },
    Separator {
        model_index: usize,
        kind: SequenceControlKind,
        label: String,
    },
    End {
        model_index: usize,
        kind: SequenceControlKind,
    },
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

fn preflight_sequence_projection(
    model: &SequenceDiagramRenderModel,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    execution.checkpoint(OperationPhase::Semantic)?;
    resources.charge_layout_work(1)?;
    if let Some(title) = model.title.as_deref() {
        charge_sequence_projection_text(resources, title, execution)?;
    }

    for actor_id in &model.actor_order {
        execution.checkpoint(OperationPhase::Semantic)?;
        resources.charge_layout_work(1)?;
        charge_sequence_projection_text(resources, actor_id, execution)?;
    }
    for (actor_id, actor) in &model.actors {
        execution.checkpoint(OperationPhase::Semantic)?;
        resources.charge_layout_work(1)?;
        charge_sequence_projection_text(resources, actor_id, execution)?;
        charge_sequence_projection_text(resources, &actor.name, execution)?;
        charge_sequence_projection_text(resources, &actor.description, execution)?;
    }
    for sequence_box in &model.boxes {
        execution.checkpoint(OperationPhase::Semantic)?;
        resources.charge_layout_work(1)?;
        charge_sequence_projection_text(resources, &sequence_box.fill, execution)?;
        if let Some(name) = sequence_box.name.as_deref() {
            charge_sequence_projection_text(resources, name, execution)?;
        }
        for actor_key in &sequence_box.actor_keys {
            execution.checkpoint(OperationPhase::Semantic)?;
            resources.charge_layout_work(1)?;
            charge_sequence_projection_text(resources, actor_key, execution)?;
        }
    }

    for message in &model.messages {
        execution.checkpoint(OperationPhase::Semantic)?;
        resources.charge_layout_work(1)?;
        if let Some(from) = message.from.as_deref() {
            charge_sequence_projection_text(resources, from, execution)?;
        }
        if let Some(to) = message.to.as_deref() {
            charge_sequence_projection_text(resources, to, execution)?;
        }
        charge_sequence_projection_text(resources, message.message_text(), execution)?;
    }

    for actor_id in model
        .created_actors
        .keys()
        .chain(model.destroyed_actors.keys())
    {
        execution.checkpoint(OperationPhase::Semantic)?;
        resources.charge_layout_work(1)?;
        charge_sequence_projection_text(resources, actor_id, execution)?;
    }
    Ok(())
}

pub(crate) fn from_sequence_model(
    model: &SequenceDiagramRenderModel,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<AsciiSequenceDiagram> {
    // Keep one render-wide ledger while binding every semantic admission to the caller's
    // operation.  The public facade creates the base ledger before entering this module; this
    // view shares its counters, but makes every charge observe semantic cancellation.
    let semantic_resources = execution.resource_context(resources, OperationPhase::Semantic);
    semantic_resources.transaction(|semantic_resources| {
        let mut semantic_resources = semantic_resources.clone();
        from_sequence_model_transactional(model, width_profile, &mut semantic_resources, execution)
    })
}

fn from_sequence_model_transactional(
    model: &SequenceDiagramRenderModel,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<AsciiSequenceDiagram> {
    preflight_sequence_projection(model, resources, execution)?;
    validate_supported_sequence_model(model, execution)?;

    let participants = sequence_participants(model, width_profile, resources, execution)?;
    if participants.is_empty() {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "no participants",
        });
    }

    execution.checkpoint(OperationPhase::Semantic)?;
    let mut participant_index = HashMap::new();
    participant_index
        .try_reserve(participants.len())
        .map_err(|_| projection_allocation_failed())?;
    for (index, participant) in participants.iter().enumerate() {
        execution.checkpoint(OperationPhase::Semantic)?;
        participant_index.insert(participant.id.as_str(), index);
    }
    let boxes = sequence_boxes(model, &participant_index, execution)?;
    let lifecycles = resolve_actor_lifecycles(model, &participant_index, resources, execution)?;
    let mut body = SequenceTreeBuilder::new(model.messages.len(), resources, execution)?;
    let mut autonumber = AutonumberState::default();

    for (model_index, message) in model.messages.iter().enumerate() {
        execution.checkpoint(OperationPhase::Semantic)?;
        let semantic_kind = message.semantic_kind();
        if consume_autonumber(message, &mut autonumber) {
            continue;
        }

        if semantic_kind == CoreSequenceMessageKind::CentralDecorationRecord {
            continue;
        }

        if let Some(record) = sequence_control_record(message, model_index, execution)? {
            match record {
                SequenceControlRecord::Start {
                    model_index,
                    kind,
                    label,
                    background,
                } => {
                    body.start_control(model_index, kind, label, background, resources, execution)?
                }
                SequenceControlRecord::Separator {
                    model_index,
                    kind,
                    label,
                } => body.start_section(model_index, kind, label, resources, execution)?,
                SequenceControlRecord::End { model_index, kind } => {
                    body.end_control(model_index, kind, resources, execution)?
                }
            }
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
            body.push_event(event, resources, execution)?;
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
            let placement = message
                .note_placement()
                .ok_or(AsciiError::UnsupportedFeature {
                    diagram_type: "sequence",
                    feature: "note placement",
                })?;
            let label = message.message_text();
            body.push_event(
                SequenceEvent::Note(SequenceNote {
                    model_index,
                    from,
                    to,
                    label: try_clone_projection_string(label, execution)?,
                    wrap: message.wrap,
                    placement,
                }),
                resources,
                execution,
            )?;
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
        let label = autonumber.label(message.message_text(), execution)?;

        body.push_event(
            SequenceEvent::Message(SequenceMessage {
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
            }),
            resources,
            execution,
        )?;
    }

    Ok(AsciiSequenceDiagram {
        participants,
        lifecycles,
        boxes,
        body: body.finish()?,
    })
}

fn sequence_control_record(
    message: &CoreSequenceMessage,
    model_index: usize,
    execution: AsciiExecution<'_>,
) -> Result<Option<SequenceControlRecord>> {
    let Some(semantics) = message.control_semantics() else {
        return Ok(None);
    };

    ensure_endpointless_control_message(message)?;

    match semantics.role {
        SequenceControlRole::Start => {
            let raw_label = message.message_text();
            let (label, background) =
                sequence_control_start_label(semantics.kind, raw_label, execution)?;
            Ok(Some(SequenceControlRecord::Start {
                model_index,
                kind: semantics.kind,
                label,
                background,
            }))
        }
        SequenceControlRole::Separator => Ok(Some(SequenceControlRecord::Separator {
            model_index,
            kind: semantics.kind,
            label: try_clone_projection_string(message.message_text(), execution)?,
        })),
        SequenceControlRole::End => Ok(Some(SequenceControlRecord::End {
            model_index,
            kind: semantics.kind,
        })),
    }
}

fn sequence_control_start_label(
    kind: SequenceControlKind,
    raw_label: &str,
    execution: AsciiExecution<'_>,
) -> Result<(String, Option<AsciiRgb>)> {
    if kind != SequenceControlKind::Rect {
        return Ok((try_clone_projection_string(raw_label, execution)?, None));
    }

    Ok(match parse_css_color_value(raw_label) {
        Some(CssColor::Rgb(color)) => (String::new(), Some(color)),
        Some(CssColor::Transparent) => (String::new(), None),
        None => (try_clone_projection_string(raw_label, execution)?, None),
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
    execution: AsciiExecution<'_>,
) -> Result<Vec<SequenceParticipant>> {
    let expected = if model.actor_order.is_empty() {
        model.actors.len()
    } else {
        model.actor_order.len()
    };
    execution.checkpoint(OperationPhase::Semantic)?;
    let mut participants = Vec::new();
    participants
        .try_reserve_exact(expected)
        .map_err(|_| projection_allocation_failed())?;

    if model.actor_order.is_empty() {
        for id in model.actors.keys() {
            execution.checkpoint(OperationPhase::Semantic)?;
            push_sequence_participant(
                &mut participants,
                model,
                id,
                width_profile,
                resources,
                execution,
            )?;
        }
    } else {
        for id in &model.actor_order {
            execution.checkpoint(OperationPhase::Semantic)?;
            push_sequence_participant(
                &mut participants,
                model,
                id,
                width_profile,
                resources,
                execution,
            )?;
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
    execution: AsciiExecution<'_>,
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
    let label = SequenceParticipantLabel::try_from_raw(
        raw_label,
        actor.wrap,
        width_profile,
        resources,
        execution,
    )?;
    participants.push(SequenceParticipant {
        id: try_clone_projection_string(id, execution)?,
        label,
    });
    Ok(())
}

fn sequence_boxes(
    model: &SequenceDiagramRenderModel,
    participant_index: &HashMap<&str, usize>,
    execution: AsciiExecution<'_>,
) -> Result<Vec<SequenceGroupBox>> {
    execution.checkpoint(OperationPhase::Semantic)?;
    let mut boxes = Vec::new();
    boxes
        .try_reserve_exact(model.boxes.len())
        .map_err(|_| projection_allocation_failed())?;
    for sequence_box in &model.boxes {
        execution.checkpoint(OperationPhase::Semantic)?;
        let mut actor_indices = Vec::new();
        actor_indices
            .try_reserve_exact(sequence_box.actor_keys.len())
            .map_err(|_| projection_allocation_failed())?;
        for actor_key in &sequence_box.actor_keys {
            execution.checkpoint(OperationPhase::Semantic)?;
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
            .map(|name| try_clone_projection_string(name, execution))
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

fn try_clone_projection_string(value: &str, execution: AsciiExecution<'_>) -> Result<String> {
    execution.checkpoint(OperationPhase::Semantic)?;
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
    fn label(&mut self, text: &str, execution: AsciiExecution<'_>) -> Result<String> {
        execution.checkpoint(OperationPhase::Semantic)?;
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
        try_clone_projection_string(text, execution)
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
    use super::{SequenceParticipantLabel, from_sequence_model};
    use crate::error::AsciiError;
    use crate::operation::AsciiExecution;
    use crate::options::TerminalWidthProfile;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy, ResourceContext};
    use merman_core::resources::ResourceProfile;
    use merman_core::{
        Engine, OperationControl, OperationPhase, ParseOptions, RenderSemanticModel,
    };

    fn parse_sequence_model(
        source: &str,
    ) -> merman_core::diagrams::sequence::SequenceDiagramRenderModel {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("sequence cancellation fixture should parse")
            .expect("sequence cancellation fixture should be detected");
        match parsed.into_parts().1 {
            RenderSemanticModel::Sequence(model) => model,
            other => panic!("expected sequence model, got {}", other.kind()),
        }
    }

    #[test]
    fn sequence_projection_cancellation_precedes_the_next_semantic_work_charge() {
        let model = parse_sequence_model("sequenceDiagram\nparticipant A\nparticipant B\n");
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .expect("one semantic work unit should be a valid limit");
        let mut resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        control.cancel_after_checkpoints(1);

        let error = from_sequence_model(
            &model,
            TerminalWidthProfile::Unicode,
            &mut resources,
            AsciiExecution::new(&control, &policy),
        )
        .expect_err("scheduled semantic cancellation should beat the limit-minus-one charge");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Semantic
                    && cancelled.reason == merman_core::CancelReason::Requested
        ));
        assert_eq!(resources.layout_work_used(), 0);
    }

    #[test]
    fn sequence_projection_failure_restores_the_complete_shared_ledger() {
        let model = parse_sequence_model("sequenceDiagram\nparticipant A as AB\n");
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, 1)
            .expect("one grid cell should be a valid limit");
        let mut resources = ResourceContext::new(policy);
        resources
            .charge_layout_work(5)
            .expect("the pre-existing work debit should fit");
        resources
            .charge_document_cells(3)
            .expect("the pre-existing document debit should fit");
        let control = OperationControl::new();

        let error = from_sequence_model(
            &model,
            TerminalWidthProfile::Unicode,
            &mut resources,
            AsciiExecution::new(&control, &policy),
        )
        .expect_err("the participant label should exceed the one-cell grid limit");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGridCells
                    && details.actual == 2
                    && details.max == 1
        ));
        assert_eq!(resources.layout_work_used(), 5);
        assert_eq!(resources.document_cells_used(), 3);
    }

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
            AsciiExecution::for_test(&unbounded),
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
            AsciiExecution::for_test(&exact_policy),
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
            AsciiExecution::for_test(&below_policy),
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

    #[test]
    fn participant_label_admission_restores_a_nonzero_shared_ledger() {
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, 1)
            .expect("one grid cell should be a valid limit");
        let base_resources = ResourceContext::new(policy);
        base_resources
            .charge_layout_work(5)
            .expect("the pre-existing work debit should fit");
        base_resources
            .charge_document_cells(3)
            .expect("the pre-existing document debit should fit");
        let control = OperationControl::new();
        let execution = AsciiExecution::new(&control, &policy);
        let mut resources = execution.resource_context(&base_resources, OperationPhase::Semantic);

        let error = SequenceParticipantLabel::try_from_raw(
            "AB",
            false,
            TerminalWidthProfile::Unicode,
            &mut resources,
            execution,
        )
        .expect_err("the post-plan grid admission should reject two cells");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGridCells
                    && details.actual == 2
                    && details.max == 1
        ));
        assert_eq!(base_resources.layout_work_used(), 5);
        assert_eq!(base_resources.document_cells_used(), 3);
    }
}
