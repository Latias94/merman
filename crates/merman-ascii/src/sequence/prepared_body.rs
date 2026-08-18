use super::chars::SequenceChars;
use super::event_paint::{MessageActorState, render_message, render_self_message};
use super::event_plan::{
    PreparedMessageRows, PreparedSelfMessageRows, ensure_message_actors_visible,
    prepare_message_rows, prepare_self_message_rows,
};
use super::layout::{SequenceLayout, participant_left};
use super::lifeline::{build_lifeline_line, retained_lifeline_width};
use super::model::{
    AsciiSequenceDiagram, MaterializedSequenceParticipantLabel, PreparedSequenceParticipantLabel,
    SequenceEvent,
};
use super::notes::{PreparedNoteRows, ensure_note_actors_known, prepare_note_rows, render_note};
use super::text::{
    SequenceBatchExtent, SequenceDocumentExtent, SequenceExtentLedger, SequenceLine,
    SequenceRowFootprint, blank_line_with_checkpoints, padded_line_with_checkpoints, trim_right,
    validate_batch_lines_with_checkpoints,
};
use super::{SequenceActorRenderState, SequenceCheckpointCursor};
use crate::color::AsciiColorRole;
use crate::error::{AsciiError, Result};
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};
use crate::text::display_width_with_profile;

#[derive(Debug, Clone, Copy)]
struct SequenceParticipantRenderModel<'a> {
    diagram: &'a AsciiSequenceDiagram,
    labels: &'a [MaterializedSequenceParticipantLabel],
}

impl<'a> SequenceParticipantRenderModel<'a> {
    fn try_new(
        diagram: &'a AsciiSequenceDiagram,
        labels: &'a [MaterializedSequenceParticipantLabel],
    ) -> Result<Self> {
        if diagram.participants.len() != labels.len() {
            return Err(unsupported("participant label materialization"));
        }
        Ok(Self { diagram, labels })
    }
}

#[derive(Debug)]
pub(super) struct SequencePreparedBody<'diagram> {
    participant_labels: Vec<PreparedSequenceParticipantLabel<'diagram>>,
    batches: Vec<SequencePreparedBatch<'diagram>>,
    footprints: Vec<SequenceRowFootprint>,
    extent: SequenceExtentLedger,
}

#[derive(Debug)]
pub(super) struct SequenceRowStep<'event> {
    pub(super) event: &'event SequenceEvent,
    pub(super) active_counts: Vec<usize>,
    pub(super) visible_actors: Vec<bool>,
    pub(super) created_actors: Vec<usize>,
    pub(super) destroyed_actors: Vec<usize>,
}

#[derive(Debug)]
struct SequencePreparedBatch<'diagram> {
    extent: SequenceBatchExtent,
    kind: SequencePreparedBatchKind<'diagram>,
}

#[derive(Debug)]
enum SequencePreparedBatchKind<'diagram> {
    ParticipantBoxes {
        visible_actors: Vec<bool>,
        frame: ParticipantBoxFrame,
    },
    Lifeline {
        active_counts: Vec<usize>,
        visible_actors: Vec<bool>,
    },
    LifecycleParticipants {
        active_counts: Vec<usize>,
        visible_actors: Vec<bool>,
        actor_indices: Vec<usize>,
    },
    Message {
        message: &'diagram super::model::SequenceMessage,
        active_counts: Vec<usize>,
        visible_actors: Vec<bool>,
        destroyed_actors: Vec<usize>,
        prepared: PreparedMessageRows,
    },
    SelfMessage {
        message: &'diagram super::model::SequenceMessage,
        active_counts: Vec<usize>,
        visible_actors: Vec<bool>,
        destroyed_actors: Vec<usize>,
        prepared: PreparedSelfMessageRows,
    },
    Note {
        note: &'diagram super::model::SequenceNote,
        active_counts: Vec<usize>,
        visible_actors: Vec<bool>,
        prepared: PreparedNoteRows,
    },
}

impl<'diagram> SequencePreparedBody<'diagram> {
    pub(super) fn new(
        diagram: &'diagram AsciiSequenceDiagram,
        layout: &SequenceLayout,
        visible_actors: &[bool],
        resources: &mut ResourceContext,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<Self> {
        let transaction = resources.clone();
        transaction.transaction(|_| {
            Self::new_transactional(diagram, layout, visible_actors, resources, checkpoints)
        })
    }

    fn new_transactional(
        diagram: &'diagram AsciiSequenceDiagram,
        layout: &SequenceLayout,
        visible_actors: &[bool],
        resources: &mut ResourceContext,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<Self> {
        let mut participant_labels = Vec::new();
        participant_labels
            .try_reserve_exact(diagram.participants.len())
            .map_err(|_| allocation_failed())?;
        for participant in &diagram.participants {
            checkpoints.tick()?;
            participant_labels.push(
                participant
                    .label
                    .prepare_materialization(resources, checkpoints)?,
            );
        }

        let mut prepared = Self {
            participant_labels,
            batches: Vec::new(),
            footprints: Vec::new(),
            extent: SequenceExtentLedger::default(),
        };
        let extent =
            participant_box_batch_extent(diagram, layout, visible_actors, resources, checkpoints)?;
        let footprints = uniform_footprints(extent, checkpoints)?;
        prepared.push_batch(
            extent,
            &footprints,
            SequencePreparedBatchKind::ParticipantBoxes {
                visible_actors: try_clone_slice(visible_actors)?,
                frame: ParticipantBoxFrame::Header,
            },
            resources,
            checkpoints,
        )?;
        Ok(prepared)
    }

    pub(super) fn current_row(&self) -> usize {
        self.footprints.len()
    }

    pub(super) fn footprints(&self) -> &[SequenceRowFootprint] {
        &self.footprints
    }

    pub(super) const fn output_extent(&self) -> SequenceDocumentExtent {
        self.extent.output_extent()
    }

    pub(super) fn prepare_step(
        &mut self,
        diagram: &'diagram AsciiSequenceDiagram,
        step: SequenceRowStep<'diagram>,
        layout: &SequenceLayout,
        chars: &SequenceChars,
        resources: &mut ResourceContext,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<()> {
        for _ in 0..layout.message_spacing {
            checkpoints.tick()?;
            self.push_lifeline(
                &step.active_counts,
                &step.visible_actors,
                layout,
                resources,
                checkpoints,
            )?;
        }

        if !step.created_actors.is_empty() {
            let extent = lifecycle_participant_batch_extent(
                diagram,
                layout,
                &step.visible_actors,
                &step.created_actors,
                resources,
                checkpoints,
            )?;
            let footprints = lifecycle_participant_footprints(
                extent,
                layout,
                &step.created_actors,
                resources,
                checkpoints,
            )?;
            self.push_batch(
                extent,
                &footprints,
                SequencePreparedBatchKind::LifecycleParticipants {
                    active_counts: try_clone_slice(&step.active_counts)?,
                    visible_actors: try_clone_slice(&step.visible_actors)?,
                    actor_indices: try_clone_slice(&step.created_actors)?,
                },
                resources,
                checkpoints,
            )?;
        }

        match step.event {
            SequenceEvent::Message(message) => {
                ensure_message_actors_visible(message, &step.visible_actors)?;
                if message.from == message.to {
                    let mut prepared = prepare_self_message_rows(
                        message,
                        layout,
                        chars,
                        &step.visible_actors,
                        resources,
                        checkpoints,
                    )?;
                    let extent = prepared.extent();
                    let footprints = prepared.take_footprints();
                    self.push_batch(
                        extent,
                        &footprints,
                        SequencePreparedBatchKind::SelfMessage {
                            message,
                            active_counts: step.active_counts,
                            visible_actors: step.visible_actors,
                            destroyed_actors: step.destroyed_actors,
                            prepared,
                        },
                        resources,
                        checkpoints,
                    )?;
                } else {
                    let mut prepared = prepare_message_rows(
                        message,
                        layout,
                        &step.visible_actors,
                        resources,
                        checkpoints,
                    )?;
                    let extent = prepared.extent();
                    let footprints = prepared.take_footprints();
                    self.push_batch(
                        extent,
                        &footprints,
                        SequencePreparedBatchKind::Message {
                            message,
                            active_counts: step.active_counts,
                            visible_actors: step.visible_actors,
                            destroyed_actors: step.destroyed_actors,
                            prepared,
                        },
                        resources,
                        checkpoints,
                    )?;
                }
            }
            SequenceEvent::Note(note) => {
                ensure_note_actors_known(note, layout)?;
                let mut prepared =
                    prepare_note_rows(note, layout, &step.visible_actors, resources, checkpoints)?;
                let extent = prepared.extent();
                let footprints = prepared.take_footprints();
                self.push_batch(
                    extent,
                    &footprints,
                    SequencePreparedBatchKind::Note {
                        note,
                        active_counts: step.active_counts,
                        visible_actors: step.visible_actors,
                        prepared,
                    },
                    resources,
                    checkpoints,
                )?;
            }
            SequenceEvent::ActivationStart { .. } | SequenceEvent::ActivationEnd { .. } => {}
        }
        Ok(())
    }

    pub(super) fn finish(
        &mut self,
        actor_state: SequenceActorRenderState<'_>,
        diagram: &'diagram AsciiSequenceDiagram,
        layout: &SequenceLayout,
        mirror_actors: bool,
        resources: &mut ResourceContext,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<()> {
        self.push_lifeline(
            actor_state.active_counts,
            actor_state.visible_actors,
            layout,
            resources,
            checkpoints,
        )?;
        if mirror_actors {
            let extent = participant_box_batch_extent(
                diagram,
                layout,
                actor_state.visible_actors,
                resources,
                checkpoints,
            )?;
            let footprints = uniform_footprints(extent, checkpoints)?;
            self.push_batch(
                extent,
                &footprints,
                SequencePreparedBatchKind::ParticipantBoxes {
                    visible_actors: try_clone_slice(actor_state.visible_actors)?,
                    frame: ParticipantBoxFrame::Mirror,
                },
                resources,
                checkpoints,
            )?;
        }
        Ok(())
    }

    pub(super) fn push_lifeline(
        &mut self,
        active_counts: &[usize],
        visible_actors: &[bool],
        layout: &SequenceLayout,
        resources: &mut ResourceContext,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<()> {
        let extent = lifeline_batch_extent(layout, visible_actors, resources, checkpoints)?;
        let footprints = [SequenceRowFootprint::lifeline(extent.retained_width())];
        self.push_batch(
            extent,
            &footprints,
            SequencePreparedBatchKind::Lifeline {
                active_counts: try_clone_slice(active_counts)?,
                visible_actors: try_clone_slice(visible_actors)?,
            },
            resources,
            checkpoints,
        )
    }

    fn push_batch(
        &mut self,
        extent: SequenceBatchExtent,
        footprints: &[SequenceRowFootprint],
        kind: SequencePreparedBatchKind<'diagram>,
        resources: &mut ResourceContext,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<()> {
        let reservation = self.extent.reserve(extent, resources, checkpoints)?;
        self.batches
            .try_reserve(1)
            .map_err(|_| allocation_failed())?;
        self.footprints
            .try_reserve(footprints.len())
            .map_err(|_| allocation_failed())?;
        reservation.commit_footprints_with_checkpoints(
            &mut self.extent,
            footprints,
            resources,
            checkpoints,
        )?;
        self.footprints.extend_from_slice(footprints);
        self.batches.push(SequencePreparedBatch { extent, kind });
        Ok(())
    }

    pub(super) fn materialization_work_units(
        &self,
        resources: &ResourceContext,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<usize> {
        let mut total = 0usize;
        for label in &self.participant_labels {
            checkpoints.tick()?;
            total = resources.checked_work_add(total, label.materialization_work_units())?;
        }
        for batch in &self.batches {
            checkpoints.tick()?;
            total = resources.checked_work_add(total, batch.materialization_work_units())?;
        }
        Ok(total)
    }

    pub(super) fn materialize(
        self,
        diagram: &AsciiSequenceDiagram,
        layout: &SequenceLayout,
        chars: &SequenceChars,
        resources: &mut ResourceContext,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<Vec<SequenceLine>> {
        let mut participant_labels = Vec::new();
        participant_labels
            .try_reserve_exact(self.participant_labels.len())
            .map_err(|_| allocation_failed())?;
        for label in self.participant_labels {
            checkpoints.tick()?;
            participant_labels.push(label.materialize_after_admission(checkpoints)?);
        }
        let participants = SequenceParticipantRenderModel::try_new(diagram, &participant_labels)?;
        let mut lines = Vec::new();
        lines
            .try_reserve_exact(self.footprints.len())
            .map_err(|_| allocation_failed())?;
        let mut expected_row: usize = 0;
        for batch in self.batches {
            checkpoints.tick()?;
            let batch_height = batch.extent.height();
            let expected_end = expected_row
                .checked_add(batch_height)
                .ok_or_else(|| unsupported("row extent planning"))?;
            let rendered =
                batch.materialize(participants, layout, chars, resources, checkpoints)?;
            let expected = self
                .footprints
                .get(expected_row..expected_end)
                .ok_or_else(|| unsupported("row extent planning"))?;
            for (line, footprint) in rendered.iter().zip(expected) {
                checkpoints.tick()?;
                if line.len() != footprint.retained_width() {
                    return Err(unsupported("row extent planning"));
                }
            }
            expected_row = expected_end;
            lines.extend(rendered);
        }
        if expected_row != self.footprints.len() || lines.len() != self.footprints.len() {
            return Err(unsupported("row extent planning"));
        }
        Ok(lines)
    }
}

impl SequencePreparedBatch<'_> {
    fn materialization_work_units(&self) -> usize {
        match &self.kind {
            SequencePreparedBatchKind::Message { prepared, .. } => {
                prepared.materialization_work_units()
            }
            SequencePreparedBatchKind::SelfMessage { prepared, .. } => {
                prepared.materialization_work_units()
            }
            SequencePreparedBatchKind::Note { prepared, .. } => {
                prepared.materialization_work_units()
            }
            SequencePreparedBatchKind::ParticipantBoxes { .. }
            | SequencePreparedBatchKind::Lifeline { .. }
            | SequencePreparedBatchKind::LifecycleParticipants { .. } => 0,
        }
    }

    fn materialize(
        self,
        participants: SequenceParticipantRenderModel<'_>,
        layout: &SequenceLayout,
        chars: &SequenceChars,
        resources: &mut ResourceContext,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<Vec<SequenceLine>> {
        let lines = match self.kind {
            SequencePreparedBatchKind::ParticipantBoxes {
                visible_actors,
                frame,
            } => render_participant_box_rows(
                participants,
                layout,
                chars,
                &visible_actors,
                frame,
                resources,
                checkpoints,
            )?,
            SequencePreparedBatchKind::Lifeline {
                active_counts,
                visible_actors,
            } => vec![build_lifeline_line(
                layout,
                chars,
                &active_counts,
                &visible_actors,
                resources,
                checkpoints,
            )?],
            SequencePreparedBatchKind::LifecycleParticipants {
                active_counts,
                visible_actors,
                actor_indices,
            } => render_lifecycle_participants(
                participants,
                layout,
                chars,
                SequenceActorRenderState::new(&active_counts, &visible_actors),
                &actor_indices,
                resources,
                checkpoints,
            )?,
            SequencePreparedBatchKind::Message {
                message,
                active_counts,
                visible_actors,
                destroyed_actors,
                prepared,
            } => render_message(
                prepared,
                message,
                layout,
                chars,
                MessageActorState::new(&active_counts, &visible_actors, &destroyed_actors),
                resources,
                checkpoints,
            )?,
            SequencePreparedBatchKind::SelfMessage {
                message,
                active_counts,
                visible_actors,
                destroyed_actors,
                prepared,
            } => render_self_message(
                prepared,
                message,
                layout,
                chars,
                MessageActorState::new(&active_counts, &visible_actors, &destroyed_actors),
                resources,
                checkpoints,
            )?,
            SequencePreparedBatchKind::Note {
                note,
                active_counts,
                visible_actors,
                prepared,
            } => render_note(
                prepared,
                note,
                layout,
                chars,
                SequenceActorRenderState::new(&active_counts, &visible_actors),
                resources,
                checkpoints,
            )?,
        };
        validate_batch_lines_with_checkpoints(self.extent, &lines, resources, checkpoints)?;
        Ok(lines)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParticipantBoxFrame {
    Header,
    Mirror,
}

pub(super) fn lifeline_batch_extent(
    layout: &SequenceLayout,
    visible_actors: &[bool],
    resources: &ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceBatchExtent> {
    let materialized_width = resources.checked_grid_add(layout.total_width, 1)?;
    let retained_width = retained_lifeline_width(layout, visible_actors, resources, checkpoints)?;
    SequenceBatchExtent::uniform(1, materialized_width, retained_width, resources)
}

pub(super) fn participant_box_batch_extent(
    diagram: &AsciiSequenceDiagram,
    layout: &SequenceLayout,
    visible_actors: &[bool],
    resources: &ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceBatchExtent> {
    let height =
        resources.checked_grid_add(participant_label_row_count(diagram, checkpoints)?, 2)?;
    let mut retained_width = 0usize;
    for index in 0..diagram.participants.len() {
        checkpoints.tick()?;
        if visible_actors.get(index).copied().unwrap_or(true) {
            retained_width = retained_width.max(participant_box_right(layout, index, resources)?);
        }
    }
    let materialized_width = resources
        .checked_grid_add(layout.total_width, 1)?
        .max(retained_width);
    SequenceBatchExtent::uniform(height, materialized_width, retained_width, resources)
}

fn lifecycle_participant_batch_extent(
    diagram: &AsciiSequenceDiagram,
    layout: &SequenceLayout,
    visible_actors: &[bool],
    actor_indices: &[usize],
    resources: &ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceBatchExtent> {
    let height =
        resources.checked_grid_add(participant_label_row_count(diagram, checkpoints)?, 2)?;
    let mut retained_width =
        retained_lifeline_width(layout, visible_actors, resources, checkpoints)?;
    for index in actor_indices {
        checkpoints.tick()?;
        retained_width = retained_width.max(participant_box_right(layout, *index, resources)?);
    }
    let materialized_width = resources
        .checked_grid_add(layout.total_width, 1)?
        .max(retained_width);
    SequenceBatchExtent::uniform(height, materialized_width, retained_width, resources)
}

fn uniform_footprints(
    extent: SequenceBatchExtent,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<Vec<SequenceRowFootprint>> {
    let mut footprints = Vec::new();
    footprints
        .try_reserve_exact(extent.height())
        .map_err(|_| allocation_failed())?;
    for _ in 0..extent.height() {
        checkpoints.tick()?;
        footprints.push(SequenceRowFootprint::lifeline(extent.retained_width()));
    }
    Ok(footprints)
}

fn lifecycle_participant_footprints(
    extent: SequenceBatchExtent,
    layout: &SequenceLayout,
    actor_indices: &[usize],
    resources: &ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<Vec<SequenceRowFootprint>> {
    let mut left = None;
    let mut right = 0usize;
    for index in actor_indices {
        checkpoints.tick()?;
        let actor_left = participant_left(layout, *index, resources)?;
        left = Some(left.map_or(actor_left, |current: usize| current.min(actor_left)));
        right = right.max(
            participant_box_right(layout, *index, resources)?
                .checked_sub(1)
                .ok_or_else(|| unsupported("actor lifecycle rows"))?,
        );
    }
    let left = left.ok_or_else(|| unsupported("actor lifecycle rows"))?;
    let footprint = SequenceRowFootprint::with_content(extent.retained_width(), left, right)?;
    let mut footprints = Vec::new();
    footprints
        .try_reserve_exact(extent.height())
        .map_err(|_| allocation_failed())?;
    for _ in 0..extent.height() {
        checkpoints.tick()?;
        footprints.push(footprint);
    }
    Ok(footprints)
}

fn participant_box_right(
    layout: &SequenceLayout,
    index: usize,
    resources: &ResourceContext,
) -> Result<usize> {
    let width = layout
        .participant_widths
        .get(index)
        .copied()
        .ok_or_else(|| unsupported("participant layout"))?;
    let segment_width = resources.checked_grid_add(width, super::BOX_BORDER_WIDTH)?;
    resources.checked_grid_add(participant_left(layout, index, resources)?, segment_width)
}

fn render_participant_box_rows(
    participants: SequenceParticipantRenderModel<'_>,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    visible_actors: &[bool],
    frame: ParticipantBoxFrame,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<Vec<SequenceLine>> {
    let label_rows = participant_label_row_count(participants.diagram, checkpoints)?;
    let rows = participant_box_rows(label_rows, frame, resources, checkpoints)?;
    let width = resources.checked_grid_add(layout.total_width, 1)?;
    resources.grid_extent(width, rows.len())?;
    checkpoints.before_charge()?;
    charge_work_product(
        resources,
        participants.diagram.participants.len(),
        rows.len(),
    )?;
    let mut rendered = Vec::new();
    rendered
        .try_reserve_exact(rows.len())
        .map_err(|_| allocation_failed())?;
    let resource_view: &ResourceContext = resources;
    for row in rows {
        checkpoints.tick()?;
        let mut line =
            blank_line_with_checkpoints(0, layout.width_profile, resource_view, checkpoints)?;
        for index in 0..participants.diagram.participants.len() {
            checkpoints.tick()?;
            if !visible_actors.get(index).copied().unwrap_or(true) {
                continue;
            }
            let left = participant_left(layout, index, resource_view)?;
            let needed = left.saturating_sub(line.len());
            line.try_push_spaces_with_checkpoint(needed, || checkpoints.checkpoint())?;
            let segment = build_participant_box_row(
                participants,
                layout,
                chars,
                ParticipantBoxRowRequest {
                    index,
                    row,
                    label_rows,
                },
                resource_view,
                checkpoints,
            )?;
            line.try_push_line_with_checkpoint(&segment, resource_view, || {
                checkpoints.checkpoint()
            })?;
        }
        rendered.push(line);
    }
    Ok(rendered)
}

fn participant_box_rows(
    label_rows: usize,
    frame: ParticipantBoxFrame,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<Vec<ParticipantBoxRow>> {
    let capacity = resources.checked_grid_add(label_rows, 2)?;
    checkpoints.before_charge()?;
    resources.charge_layout_work(capacity)?;
    resources.grid_extent(1, capacity)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(capacity)
        .map_err(|_| allocation_failed())?;
    rows.push(match frame {
        ParticipantBoxFrame::Header => ParticipantBoxRow::Top,
        ParticipantBoxFrame::Mirror => ParticipantBoxRow::MirrorTop,
    });
    for row in 0..label_rows {
        checkpoints.tick()?;
        rows.push(ParticipantBoxRow::Label(row));
    }
    rows.push(match frame {
        ParticipantBoxFrame::Header => ParticipantBoxRow::Bottom,
        ParticipantBoxFrame::Mirror => ParticipantBoxRow::MirrorBottom,
    });
    Ok(rows)
}

fn participant_label_row_count(
    diagram: &AsciiSequenceDiagram,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<usize> {
    let mut label_rows = 1usize;
    for participant in &diagram.participants {
        checkpoints.tick()?;
        label_rows = label_rows.max(participant.label.line_count());
    }
    Ok(label_rows)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParticipantBoxRow {
    Top,
    Label(usize),
    Bottom,
    MirrorTop,
    MirrorBottom,
}

#[derive(Debug, Clone, Copy)]
struct ParticipantBoxRowRequest {
    index: usize,
    row: ParticipantBoxRow,
    label_rows: usize,
}

fn build_participant_box_row(
    participants: SequenceParticipantRenderModel<'_>,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    request: ParticipantBoxRowRequest,
    resources: &ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceLine> {
    let ParticipantBoxRowRequest {
        index,
        row,
        label_rows,
    } = request;
    let width = layout
        .participant_widths
        .get(index)
        .copied()
        .ok_or_else(|| unsupported("participant layout"))?;
    let total_width = resources.checked_grid_add(width, super::BOX_BORDER_WIDTH)?;
    let mut line =
        blank_line_with_checkpoints(total_width, layout.width_profile, resources, checkpoints)?;
    let center_offset = resources.checked_grid_add(width / 2, 1)?;
    let right = resources.checked_grid_add(width, 1)?;
    match row {
        ParticipantBoxRow::Top | ParticipantBoxRow::MirrorTop => {
            line.try_set_role(0, chars.top_left, AsciiColorRole::SequenceFrame)?;
            for x in 1..=width {
                checkpoints.tick()?;
                let ch = if row == ParticipantBoxRow::MirrorTop && x == center_offset {
                    chars.tee_up
                } else {
                    chars.horizontal
                };
                line.try_set_role(x, ch, AsciiColorRole::SequenceFrame)?;
            }
            line.try_set_role(right, chars.top_right, AsciiColorRole::SequenceFrame)?;
        }
        ParticipantBoxRow::Label(label_row) => {
            let label = participants
                .labels
                .get(index)
                .ok_or_else(|| unsupported("participant label materialization"))?;
            let label_lines = label.lines();
            let row_count = label_lines.len().max(1);
            let top_padding = label_rows.saturating_sub(row_count) / 2;
            let row_label = label_row
                .checked_sub(top_padding)
                .and_then(|index| label_lines.get(index));
            let label_width = row_label
                .map(|line| display_width_with_profile(line, layout.width_profile))
                .unwrap_or(0);
            let left_padding = width
                .checked_sub(label_width)
                .ok_or_else(|| unsupported("participant label width"))?
                / 2;
            line.try_set_role(0, chars.vertical, AsciiColorRole::SequenceFrame)?;
            if let Some(label) = row_label {
                let label_start = resources.checked_grid_add(1, left_padding)?;
                line.try_write_text_role_with_checkpoint(
                    label_start,
                    label,
                    AsciiColorRole::Text,
                    resources,
                    || checkpoints.tick(),
                )?;
            }
            line.try_set_role(right, chars.vertical, AsciiColorRole::SequenceFrame)?;
        }
        ParticipantBoxRow::Bottom | ParticipantBoxRow::MirrorBottom => {
            line.try_set_role(0, chars.bottom_left, AsciiColorRole::SequenceFrame)?;
            for x in 1..=width {
                checkpoints.tick()?;
                let ch = if row == ParticipantBoxRow::Bottom && x == center_offset {
                    chars.tee_down
                } else {
                    chars.horizontal
                };
                line.try_set_role(x, ch, AsciiColorRole::SequenceFrame)?;
            }
            line.try_set_role(right, chars.bottom_right, AsciiColorRole::SequenceFrame)?;
        }
    }
    Ok(line)
}

fn render_lifecycle_participants(
    participants: SequenceParticipantRenderModel<'_>,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    actor_state: SequenceActorRenderState<'_>,
    actor_indices: &[usize],
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<Vec<SequenceLine>> {
    let label_rows = participant_label_row_count(participants.diagram, checkpoints)?;
    let rows = participant_box_rows(
        label_rows,
        ParticipantBoxFrame::Header,
        resources,
        checkpoints,
    )?;
    checkpoints.before_charge()?;
    charge_work_product(resources, actor_indices.len(), rows.len())?;
    let base_width = resources.checked_grid_add(layout.total_width, 1)?;
    resources.grid_extent(base_width, rows.len())?;

    let mut rendered = Vec::new();
    rendered
        .try_reserve_exact(rows.len())
        .map_err(|_| allocation_failed())?;
    for row in rows {
        checkpoints.tick()?;
        let mut width = base_width;
        for index in actor_indices {
            checkpoints.tick()?;
            let segment = build_participant_box_row(
                participants,
                layout,
                chars,
                ParticipantBoxRowRequest {
                    index: *index,
                    row,
                    label_rows,
                },
                resources,
                checkpoints,
            )?;
            let participant_left = participant_left(layout, *index, resources)?;
            let segment_right = resources.checked_grid_add(participant_left, segment.len())?;
            width = width.max(segment_right);
        }
        resources.grid_extent(width, 1)?;
        let mut line = padded_line_with_checkpoints(
            build_lifeline_line(
                layout,
                chars,
                actor_state.active_counts,
                actor_state.visible_actors,
                resources,
                checkpoints,
            )?,
            width,
            checkpoints,
        )?;
        for index in actor_indices {
            checkpoints.tick()?;
            let segment = build_participant_box_row(
                participants,
                layout,
                chars,
                ParticipantBoxRowRequest {
                    index: *index,
                    row,
                    label_rows,
                },
                resources,
                checkpoints,
            )?;
            line.try_write_line_with_checkpoint(
                participant_left(layout, *index, resources)?,
                &segment,
                resources,
                || checkpoints.checkpoint(),
            )?;
        }
        rendered.push(trim_right(line)?);
    }
    Ok(rendered)
}

fn unsupported(feature: &'static str) -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: "sequence",
        feature,
    }
}

fn charge_work_product(resources: &mut ResourceContext, left: usize, right: usize) -> Result<()> {
    resources.charge_layout_work_product(left, right)
}

fn try_clone_slice<T: Copy>(source: &[T]) -> Result<Vec<T>> {
    let mut cloned = Vec::new();
    cloned
        .try_reserve_exact(source.len())
        .map_err(|_| allocation_failed())?;
    cloned.extend_from_slice(source);
    Ok(cloned)
}

fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}
