use super::boxes::render_sequence_boxes;
use super::control::render_sequence_control_frames;
use super::control::{SequenceControlFrame, SequenceControlFrameSeparator};
use super::events::{
    ensure_message_actors_visible, prepare_message_rows, prepare_self_message_rows, render_message,
    render_self_message,
};
use super::layout::{
    LifecycleEdge, SequenceLayout, initial_visible_actors, lifecycle_actors_at, participant_left,
};
use super::model::{AsciiSequenceDiagram, SequenceControlKind, SequenceEvent};
use super::notes::{ensure_note_actors_visible, prepare_note_rows, render_note};
use super::render::{SequenceChars, build_lifeline_line, retained_lifeline_width};
use super::text::{
    SequenceBatchExtent, SequenceExtentLedger, SequenceExtentReservation, SequenceLine, blank_line,
    charge_text_work, padded_line, trim_right,
};
use crate::canvas::finish_styled_lines_with_options;
use crate::color::AsciiColorMode;
use crate::color::AsciiColorRole;
use crate::error::{AsciiError, Result};
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
use crate::resource::{AsciiResourceLimitPhase, CheckedOutput, ResourceContext};
use crate::text::display_width_with_profile;

#[derive(Debug)]
struct SequenceRowStep<'event> {
    event: &'event SequenceEvent,
    active_counts: Vec<usize>,
    visible_actors: Vec<bool>,
    created_actors: Vec<usize>,
    destroyed_actors: Vec<usize>,
}

#[derive(Debug)]
struct SequenceRowPlanner {
    active_counts: Vec<usize>,
    visible_actors: Vec<bool>,
    control_frames: Vec<SequenceControlFrame>,
    active_control_frames: Vec<usize>,
}

impl SequenceRowPlanner {
    fn new(diagram: &AsciiSequenceDiagram, resources: &mut ResourceContext) -> Result<Self> {
        resources.charge_layout_work(diagram.participants.len())?;
        resources.charge_layout_work(diagram.lifecycles.len())?;
        resources.grid_extent(diagram.participants.len().max(diagram.lifecycles.len()), 1)?;

        let mut active_counts = Vec::new();
        active_counts
            .try_reserve_exact(diagram.participants.len())
            .map_err(|_| allocation_failed())?;
        active_counts.resize(diagram.participants.len(), 0);

        let visible_actors = initial_visible_actors(diagram, resources)?;

        Ok(Self {
            active_counts,
            visible_actors,
            control_frames: Vec::new(),
            active_control_frames: Vec::new(),
        })
    }

    fn active_counts(&self) -> &[usize] {
        &self.active_counts
    }

    fn visible_actors(&self) -> &[bool] {
        &self.visible_actors
    }

    fn advance<'event>(
        &mut self,
        diagram: &AsciiSequenceDiagram,
        event: &'event SequenceEvent,
        current_row: usize,
        resources: &mut ResourceContext,
    ) -> Result<Option<SequenceRowStep<'event>>> {
        resources.charge_layout_work(1)?;
        match event {
            SequenceEvent::ActivationStart { actor, .. } => {
                let Some(count) = self.active_counts.get_mut(*actor) else {
                    return Err(unsupported("activation actor state"));
                };
                *count = count
                    .checked_add(1)
                    .ok_or_else(|| work_overflow(resources))?;
                Ok(None)
            }
            SequenceEvent::ActivationEnd { actor, .. } => {
                let Some(count) = self.active_counts.get_mut(*actor) else {
                    return Err(unsupported("activation actor state"));
                };
                if *count == 0 {
                    return Err(unsupported("activation underflow"));
                }
                *count -= 1;
                Ok(None)
            }
            SequenceEvent::ControlStart(start) => {
                charge_text_work(&start.label, TerminalWidthProfile::Unicode, resources)?;
                let depth = self
                    .active_control_frames
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| nesting_overflow(resources))?;
                resources.check_nesting_depth(depth)?;
                let frame_index = self.control_frames.len();
                let frame_count = resources.checked_grid_add(frame_index, 1)?;
                resources.grid_extent(frame_count, 1)?;
                let label = try_clone_string(&start.label)?;
                self.control_frames
                    .try_reserve(1)
                    .map_err(|_| allocation_failed())?;
                self.active_control_frames
                    .try_reserve(1)
                    .map_err(|_| allocation_failed())?;
                self.control_frames.push(SequenceControlFrame {
                    kind: start.kind,
                    label,
                    background: start.background,
                    start_row: current_row,
                    separators: Vec::new(),
                    end_row: None,
                });
                self.active_control_frames.push(frame_index);
                Ok(None)
            }
            SequenceEvent::ControlSeparator(separator) => {
                charge_text_work(&separator.label, TerminalWidthProfile::Unicode, resources)?;
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
                let separator_count = resources.checked_grid_add(frame.separators.len(), 1)?;
                resources.grid_extent(separator_count, 1)?;
                let label = try_clone_string(&separator.label)?;
                frame
                    .separators
                    .try_reserve(1)
                    .map_err(|_| allocation_failed())?;
                frame.separators.push(SequenceControlFrameSeparator {
                    label,
                    row: current_row,
                });
                Ok(None)
            }
            SequenceEvent::ControlEnd { kind, .. } => {
                self.end_control_frame(*kind, current_row)?;
                Ok(None)
            }
            SequenceEvent::Message(_) | SequenceEvent::Note(_) => {
                let model_index = event.model_index();
                charge_work_product(resources, diagram.lifecycles.len(), 2)?;
                let created_actors =
                    lifecycle_actors_at(diagram, model_index, LifecycleEdge::Created, resources)?;
                if !created_actors.is_empty() {
                    self.record_created_actors(&created_actors);
                }
                let destroyed_actors =
                    lifecycle_actors_at(diagram, model_index, LifecycleEdge::Destroyed, resources)?;
                resources.charge_layout_work(self.active_counts.len())?;
                resources.charge_layout_work(self.visible_actors.len())?;
                let step = SequenceRowStep {
                    event,
                    active_counts: try_clone_slice(&self.active_counts)?,
                    visible_actors: try_clone_slice(&self.visible_actors)?,
                    created_actors,
                    destroyed_actors,
                };
                self.record_destroyed_actors(&step.destroyed_actors);
                Ok(Some(step))
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
            let end_row = current_row
                .checked_sub(1)
                .ok_or_else(|| unsupported("control block ordering"))?;
            frame.end_row = Some(end_row);
        }
        self.active_control_frames.pop();
        Ok(())
    }
}

#[derive(Debug)]
struct SequenceRowEmitter<'diagram, 'layout, 'chars> {
    diagram: &'diagram AsciiSequenceDiagram,
    layout: &'layout SequenceLayout,
    chars: &'chars SequenceChars,
    lines: Vec<SequenceLine>,
    extent: SequenceExtentLedger,
}

impl<'diagram, 'layout, 'chars> SequenceRowEmitter<'diagram, 'layout, 'chars> {
    fn new(
        diagram: &'diagram AsciiSequenceDiagram,
        layout: &'layout SequenceLayout,
        chars: &'chars SequenceChars,
        visible_actors: &[bool],
        resources: &mut ResourceContext,
    ) -> Result<Self> {
        let batch = participant_box_batch_extent(diagram, layout, visible_actors, resources)?;
        let mut extent = SequenceExtentLedger::default();
        let reservation = extent.reserve(batch, resources)?;
        let lines = render_participant_box_rows(
            diagram,
            layout,
            chars,
            visible_actors,
            ParticipantBoxFrame::Header,
            resources,
        )?;
        reservation.commit(&mut extent, &lines, resources)?;
        Ok(Self {
            diagram,
            layout,
            chars,
            lines,
            extent,
        })
    }

    fn current_row(&self) -> usize {
        self.lines.len()
    }

    fn emit_step(
        &mut self,
        step: &SequenceRowStep<'_>,
        resources: &mut ResourceContext,
    ) -> Result<()> {
        self.emit_message_spacing(step, resources)?;
        self.emit_created_actors(step, resources)?;
        self.emit_message_or_note(step, resources)
    }

    fn emit_message_spacing(
        &mut self,
        step: &SequenceRowStep<'_>,
        resources: &mut ResourceContext,
    ) -> Result<()> {
        for _ in 0..self.layout.message_spacing {
            let batch = lifeline_batch_extent(self.layout, &step.visible_actors, resources)?;
            let reservation = self.reserve_batch(batch, resources)?;
            let line = self.lifeline_line(step, resources)?;
            self.commit_line(reservation, line, resources)?;
        }
        Ok(())
    }

    fn emit_created_actors(
        &mut self,
        step: &SequenceRowStep<'_>,
        resources: &mut ResourceContext,
    ) -> Result<()> {
        if step.created_actors.is_empty() {
            return Ok(());
        }

        let batch = lifecycle_participant_batch_extent(
            self.diagram,
            self.layout,
            &step.visible_actors,
            &step.created_actors,
            resources,
        )?;
        let reservation = self.reserve_batch(batch, resources)?;
        let lines = render_lifecycle_participants(
            self.diagram,
            self.layout,
            self.chars,
            &step.active_counts,
            &step.visible_actors,
            &step.created_actors,
            resources,
        )?;
        self.commit_lines(reservation, lines, resources)
    }

    fn emit_message_or_note(
        &mut self,
        step: &SequenceRowStep<'_>,
        resources: &mut ResourceContext,
    ) -> Result<()> {
        match step.event {
            SequenceEvent::Message(message) => {
                ensure_message_actors_visible(message, &step.visible_actors)?;
                if message.from == message.to {
                    let prepared = prepare_self_message_rows(
                        message,
                        self.layout,
                        &step.visible_actors,
                        resources,
                    )?;
                    let reservation = self.reserve_batch(prepared.extent(), resources)?;
                    let lines = render_self_message(
                        prepared,
                        message,
                        self.layout,
                        self.chars,
                        &step.active_counts,
                        &step.visible_actors,
                        &step.destroyed_actors,
                        resources,
                    )?;
                    self.commit_lines(reservation, lines, resources)?;
                } else {
                    let prepared = prepare_message_rows(
                        message,
                        self.layout,
                        &step.visible_actors,
                        resources,
                    )?;
                    let reservation = self.reserve_batch(prepared.extent(), resources)?;
                    let lines = render_message(
                        prepared,
                        message,
                        self.layout,
                        self.chars,
                        &step.active_counts,
                        &step.visible_actors,
                        &step.destroyed_actors,
                        resources,
                    )?;
                    self.commit_lines(reservation, lines, resources)?;
                }
            }
            SequenceEvent::Note(note) => {
                ensure_note_actors_visible(note, &step.visible_actors)?;
                let prepared =
                    prepare_note_rows(note, self.layout, &step.visible_actors, resources)?;
                let reservation = self.reserve_batch(prepared.extent(), resources)?;
                let lines = render_note(
                    prepared,
                    self.layout,
                    self.chars,
                    &step.active_counts,
                    &step.visible_actors,
                    resources,
                )?;
                self.commit_lines(reservation, lines, resources)?;
            }
            SequenceEvent::ActivationStart { .. }
            | SequenceEvent::ActivationEnd { .. }
            | SequenceEvent::ControlStart(_)
            | SequenceEvent::ControlEnd { .. }
            | SequenceEvent::ControlSeparator(_) => {}
        }
        Ok(())
    }

    fn finish(
        mut self,
        planner: &SequenceRowPlanner,
        mirror_actors: bool,
        resources: &mut ResourceContext,
    ) -> Result<Vec<SequenceLine>> {
        let batch = lifeline_batch_extent(self.layout, planner.visible_actors(), resources)?;
        let reservation = self.reserve_batch(batch, resources)?;
        let lifeline =
            self.lifeline_line_state(planner.active_counts(), planner.visible_actors(), resources)?;
        self.commit_line(reservation, lifeline, resources)?;
        if mirror_actors {
            let batch = participant_box_batch_extent(
                self.diagram,
                self.layout,
                planner.visible_actors(),
                resources,
            )?;
            let reservation = self.reserve_batch(batch, resources)?;
            let lines = render_participant_box_rows(
                self.diagram,
                self.layout,
                self.chars,
                planner.visible_actors(),
                ParticipantBoxFrame::Mirror,
                resources,
            )?;
            self.commit_lines(reservation, lines, resources)?;
        }
        Ok(self.lines)
    }

    fn lifeline_line(
        &self,
        step: &SequenceRowStep<'_>,
        resources: &ResourceContext,
    ) -> Result<SequenceLine> {
        self.lifeline_line_state(&step.active_counts, &step.visible_actors, resources)
    }

    fn lifeline_line_state(
        &self,
        active_counts: &[usize],
        visible_actors: &[bool],
        resources: &ResourceContext,
    ) -> Result<SequenceLine> {
        build_lifeline_line(
            self.layout,
            self.chars,
            active_counts,
            visible_actors,
            resources,
        )
    }

    fn reserve_batch(
        &mut self,
        batch: SequenceBatchExtent,
        resources: &mut ResourceContext,
    ) -> Result<SequenceExtentReservation> {
        let reservation = self.extent.reserve(batch, resources)?;
        self.lines
            .try_reserve(batch.height())
            .map_err(|_| allocation_failed())?;
        Ok(reservation)
    }

    fn commit_lines(
        &mut self,
        reservation: SequenceExtentReservation,
        lines: Vec<SequenceLine>,
        resources: &ResourceContext,
    ) -> Result<()> {
        reservation.commit(&mut self.extent, &lines, resources)?;
        self.lines.extend(lines);
        Ok(())
    }

    fn commit_line(
        &mut self,
        reservation: SequenceExtentReservation,
        line: SequenceLine,
        resources: &ResourceContext,
    ) -> Result<()> {
        reservation.commit(&mut self.extent, std::slice::from_ref(&line), resources)?;
        self.lines.push(line);
        Ok(())
    }
}

#[derive(Debug)]
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
        resources: &mut ResourceContext,
    ) -> Result<Self> {
        let mut planner = SequenceRowPlanner::new(diagram, resources)?;
        let mut emitter =
            SequenceRowEmitter::new(diagram, layout, chars, planner.visible_actors(), resources)?;

        for event in &diagram.events {
            if let Some(step) = planner.advance(diagram, event, emitter.current_row(), resources)? {
                emitter.emit_step(&step, resources)?;
            }
        }

        Ok(Self {
            lines: emitter.finish(&planner, mirror_actors, resources)?,
            control_frames: planner.finish()?,
        })
    }

    pub(super) fn render(
        self,
        diagram: &AsciiSequenceDiagram,
        layout: &SequenceLayout,
        chars: &SequenceChars,
        options: &AsciiRenderOptions,
        resources: &mut ResourceContext,
    ) -> Result<String> {
        let mut lines = self.lines;
        if !self.control_frames.is_empty() {
            lines = render_sequence_control_frames(lines, &self.control_frames, chars, resources)?;
        }
        if !diagram.boxes.is_empty() {
            lines = render_sequence_boxes(lines, diagram, layout, chars, resources)?;
        }
        if let Some(title) = diagram.title.as_deref() {
            prepend_title_line(&mut lines, title, resources)?;
        }
        finish_sequence_lines(lines, options)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParticipantBoxFrame {
    Header,
    Mirror,
}

fn lifeline_batch_extent(
    layout: &SequenceLayout,
    visible_actors: &[bool],
    resources: &ResourceContext,
) -> Result<SequenceBatchExtent> {
    let materialized_width = resources.checked_grid_add(layout.total_width, 1)?;
    let retained_width = retained_lifeline_width(layout, visible_actors, resources)?;
    SequenceBatchExtent::uniform(1, materialized_width, retained_width, resources)
}

fn participant_box_batch_extent(
    diagram: &AsciiSequenceDiagram,
    layout: &SequenceLayout,
    visible_actors: &[bool],
    resources: &ResourceContext,
) -> Result<SequenceBatchExtent> {
    let height = resources.checked_grid_add(participant_label_row_count(diagram), 2)?;
    let retained_width = (0..diagram.participants.len())
        .filter(|index| visible_actors.get(*index).copied().unwrap_or(true))
        .try_fold(0usize, |width, index| {
            Ok::<usize, AsciiError>(width.max(participant_box_right(layout, index, resources)?))
        })?;
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
) -> Result<SequenceBatchExtent> {
    let height = resources.checked_grid_add(participant_label_row_count(diagram), 2)?;
    let retained_width = actor_indices.iter().try_fold(
        retained_lifeline_width(layout, visible_actors, resources)?,
        |width, index| {
            Ok::<usize, AsciiError>(width.max(participant_box_right(layout, *index, resources)?))
        },
    )?;
    let materialized_width = resources
        .checked_grid_add(layout.total_width, 1)?
        .max(retained_width);
    SequenceBatchExtent::uniform(height, materialized_width, retained_width, resources)
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
    diagram: &AsciiSequenceDiagram,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    visible_actors: &[bool],
    frame: ParticipantBoxFrame,
    resources: &mut ResourceContext,
) -> Result<Vec<SequenceLine>> {
    let rows = participant_box_rows(diagram, frame, resources)?;
    let width = resources.checked_grid_add(layout.total_width, 1)?;
    resources.grid_extent(width, rows.len())?;
    charge_work_product(resources, diagram.participants.len(), rows.len())?;
    let mut rendered = Vec::new();
    rendered
        .try_reserve_exact(rows.len())
        .map_err(|_| allocation_failed())?;
    let resource_view: &ResourceContext = resources;
    for row in rows {
        rendered.push(build_participant_line(
            diagram,
            layout,
            visible_actors,
            resource_view,
            |index| build_participant_box_row(diagram, layout, chars, index, row, resource_view),
        )?);
    }
    Ok(rendered)
}

fn participant_box_rows(
    diagram: &AsciiSequenceDiagram,
    frame: ParticipantBoxFrame,
    resources: &mut ResourceContext,
) -> Result<Vec<ParticipantBoxRow>> {
    let label_rows = participant_label_row_count(diagram);
    let capacity = resources.checked_grid_add(label_rows, 2)?;
    resources.charge_layout_work(capacity)?;
    resources.grid_extent(1, capacity)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(capacity)
        .map_err(|_| allocation_failed())?;
    rows.push(match frame {
        ParticipantBoxFrame::Header => ParticipantBoxRow::Top,
        ParticipantBoxFrame::Mirror => ParticipantBoxRow::MirrorTop,
    });
    rows.extend((0..label_rows).map(ParticipantBoxRow::Label));
    rows.push(match frame {
        ParticipantBoxFrame::Header => ParticipantBoxRow::Bottom,
        ParticipantBoxFrame::Mirror => ParticipantBoxRow::MirrorBottom,
    });
    Ok(rows)
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
    resources: &ResourceContext,
    draw: impl Fn(usize) -> Result<SequenceLine>,
) -> Result<SequenceLine> {
    let mut line = blank_line(0, layout.width_profile, resources)?;
    for index in 0..diagram.participants.len() {
        if !visible_actors.get(index).copied().unwrap_or(true) {
            continue;
        }
        let left = participant_left(layout, index, resources)?;
        let needed = left.saturating_sub(line.len());
        line.try_push_spaces(needed)?;
        line.try_push_line(&draw(index)?)?;
    }
    Ok(line)
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
    resources: &ResourceContext,
) -> Result<SequenceLine> {
    let width = layout
        .participant_widths
        .get(index)
        .copied()
        .ok_or_else(|| unsupported("participant layout"))?;
    let total_width = resources.checked_grid_add(width, super::BOX_BORDER_WIDTH)?;
    let mut line = blank_line(total_width, layout.width_profile, resources)?;
    let center_offset = resources.checked_grid_add(width / 2, 1)?;
    let right = resources.checked_grid_add(width, 1)?;
    match row {
        ParticipantBoxRow::Top | ParticipantBoxRow::MirrorTop => {
            line.try_set_role(0, chars.top_left, AsciiColorRole::SequenceFrame)?;
            for x in 1..=width {
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
            let label = &diagram
                .participants
                .get(index)
                .ok_or_else(|| unsupported("participant layout"))?
                .label;
            let label_lines = label.lines();
            let row_count = label_lines.len().max(1);
            let top_padding = (participant_label_row_count(diagram).saturating_sub(row_count)) / 2;
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
                line.try_write_text_role(label_start, label, AsciiColorRole::Text)?;
            }
            line.try_set_role(right, chars.vertical, AsciiColorRole::SequenceFrame)?;
        }
        ParticipantBoxRow::Bottom | ParticipantBoxRow::MirrorBottom => {
            line.try_set_role(0, chars.bottom_left, AsciiColorRole::SequenceFrame)?;
            for x in 1..=width {
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
    diagram: &AsciiSequenceDiagram,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    active_counts: &[usize],
    visible_actors: &[bool],
    actor_indices: &[usize],
    resources: &mut ResourceContext,
) -> Result<Vec<SequenceLine>> {
    let rows = participant_box_rows(diagram, ParticipantBoxFrame::Header, resources)?;
    charge_work_product(resources, actor_indices.len(), rows.len())?;
    let base_width = resources.checked_grid_add(layout.total_width, 1)?;
    resources.grid_extent(base_width, rows.len())?;

    let mut rendered = Vec::new();
    rendered
        .try_reserve_exact(rows.len())
        .map_err(|_| allocation_failed())?;
    for row in rows {
        let mut width = base_width;
        for index in actor_indices {
            let segment =
                build_participant_box_row(diagram, layout, chars, *index, row, resources)?;
            let participant_left = participant_left(layout, *index, resources)?;
            let segment_right = resources.checked_grid_add(participant_left, segment.len())?;
            width = width.max(segment_right);
        }
        resources.grid_extent(width, 1)?;
        let mut line = padded_line(
            build_lifeline_line(layout, chars, active_counts, visible_actors, resources)?,
            width,
        )?;
        for index in actor_indices {
            let segment =
                build_participant_box_row(diagram, layout, chars, *index, row, resources)?;
            line.try_write_line(participant_left(layout, *index, resources)?, &segment)?;
        }
        rendered.push(trim_right(line)?);
    }
    Ok(rendered)
}

fn finish_sequence_lines(lines: Vec<SequenceLine>, options: &AsciiRenderOptions) -> Result<String> {
    if options.color_mode == AsciiColorMode::Plain {
        let resources = ResourceContext::new(options.resources);
        let mut output = CheckedOutput::new(options.resources);
        if lines.is_empty() {
            output.push_char('\n')?;
            return Ok(output.finish());
        }
        for line in lines {
            resources.charge_document_cells(line.len())?;
            line.try_write_plain_to(&mut output)?;
            output.push_char('\n')?;
        }
        return Ok(output.finish());
    }

    if lines.is_empty() {
        return Ok(String::new());
    }

    finish_styled_lines_with_options(&lines, options, true)
}

fn prepend_title_line(
    lines: &mut Vec<SequenceLine>,
    title: &str,
    resources: &mut ResourceContext,
) -> Result<()> {
    let width = lines.iter().map(SequenceLine::len).max().unwrap_or(0);
    let width_profile = lines
        .first()
        .map(SequenceLine::width_profile)
        .unwrap_or(TerminalWidthProfile::Unicode);
    charge_text_work(title, width_profile, resources)?;
    let title_width = display_width_with_profile(title, width_profile);
    let height = resources.checked_grid_add(lines.len(), 1)?;
    resources.grid_extent(width.max(title_width), height)?;
    resources.charge_layout_work(title_width.max(1))?;
    lines.try_reserve(1).map_err(|_| allocation_failed())?;
    lines.insert(
        0,
        render_title_line(title, width, width_profile, resources)?,
    );
    Ok(())
}

fn render_title_line(
    title: &str,
    width: usize,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<SequenceLine> {
    let title_width = display_width_with_profile(title, width_profile);
    let left = width.saturating_sub(title_width) / 2;
    let mut line = blank_line(left, width_profile, resources)?;
    line.try_push_role_text(title, AsciiColorRole::Text)?;
    trim_right(line)
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

fn work_overflow(resources: &ResourceContext) -> AsciiError {
    resources.work_overflow()
}

fn nesting_overflow(resources: &ResourceContext) -> AsciiError {
    resources.nesting_overflow()
}

fn try_clone_slice<T: Copy>(source: &[T]) -> Result<Vec<T>> {
    let mut cloned = Vec::new();
    cloned
        .try_reserve_exact(source.len())
        .map_err(|_| allocation_failed())?;
    cloned.extend_from_slice(source);
    Ok(cloned)
}

fn try_clone_string(source: &str) -> Result<String> {
    let mut cloned = String::new();
    cloned
        .try_reserve_exact(source.len())
        .map_err(|_| allocation_failed())?;
    cloned.push_str(source);
    Ok(cloned)
}

fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::AsciiColorMode;
    use crate::options::AsciiRenderOptions;
    use crate::resource::AsciiResourceLimitId;
    use crate::sequence::layout::calculate_layout;
    use crate::sequence::model::{
        SequenceActorLifecycle, SequenceArrowHead, SequenceControlSeparator, SequenceControlStart,
        SequenceEvent, SequenceLineStyle, SequenceMessage, SequenceParticipant,
        SequenceParticipantLabel,
    };

    #[test]
    fn event_plan_tracks_activation_counts() {
        let diagram = diagram(1);
        let mut resources = test_resources();
        let mut plan = SequenceRowPlanner::new(&diagram, &mut resources).unwrap();

        assert!(
            plan.advance(
                &diagram,
                &SequenceEvent::ActivationStart {
                    actor: 0,
                    model_index: 0,
                },
                3,
                &mut resources,
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(plan.active_counts(), &[1]);

        assert!(
            plan.advance(
                &diagram,
                &SequenceEvent::ActivationEnd {
                    actor: 0,
                    model_index: 1,
                },
                4,
                &mut resources,
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(plan.active_counts(), &[0]);
    }

    #[test]
    fn event_plan_rejects_empty_control_sections() {
        let diagram = diagram(1);
        let mut resources = test_resources();
        let mut plan = SequenceRowPlanner::new(&diagram, &mut resources).unwrap();

        assert!(
            plan.advance(
                &diagram,
                &SequenceEvent::ControlStart(SequenceControlStart {
                    model_index: 0,
                    kind: SequenceControlKind::Alt,
                    label: "choice".to_string(),
                    background: None,
                }),
                3,
                &mut resources,
            )
            .unwrap()
            .is_none()
        );

        let error = plan
            .advance(
                &diagram,
                &SequenceEvent::ControlSeparator(SequenceControlSeparator {
                    model_index: 1,
                    kind: SequenceControlKind::Alt,
                    label: "other".to_string(),
                }),
                3,
                &mut resources,
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
        let mut resources = test_resources();
        let mut plan = SequenceRowPlanner::new(&diagram, &mut resources).unwrap();

        assert!(
            plan.advance(
                &diagram,
                &SequenceEvent::ControlStart(SequenceControlStart {
                    model_index: 0,
                    kind: SequenceControlKind::Loop,
                    label: "outer".to_string(),
                    background: None,
                }),
                3,
                &mut resources,
            )
            .unwrap()
            .is_none()
        );
        assert!(
            plan.advance(
                &diagram,
                &SequenceEvent::ControlStart(SequenceControlStart {
                    model_index: 1,
                    kind: SequenceControlKind::Opt,
                    label: "inner".to_string(),
                    background: None,
                }),
                3,
                &mut resources,
            )
            .unwrap()
            .is_none()
        );
        assert!(
            plan.advance(
                &diagram,
                &SequenceEvent::ControlEnd {
                    kind: SequenceControlKind::Opt,
                    model_index: 2,
                },
                4,
                &mut resources,
            )
            .unwrap()
            .is_none()
        );
        assert!(
            plan.advance(
                &diagram,
                &SequenceEvent::ControlEnd {
                    kind: SequenceControlKind::Loop,
                    model_index: 3,
                },
                4,
                &mut resources,
            )
            .unwrap()
            .is_none()
        );

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
    fn event_plan_rejects_control_nesting_before_push() {
        let diagram = diagram(1);
        let options = AsciiRenderOptions::ascii()
            .with_resource_limit(AsciiResourceLimitId::MaxNestingDepth, 1)
            .unwrap();
        let mut resources = ResourceContext::new(options.resources);
        let mut plan = SequenceRowPlanner::new(&diagram, &mut resources).unwrap();

        plan.advance(
            &diagram,
            &SequenceEvent::ControlStart(SequenceControlStart {
                model_index: 0,
                kind: SequenceControlKind::Loop,
                label: "outer".to_string(),
                background: None,
            }),
            3,
            &mut resources,
        )
        .unwrap();

        let error = plan
            .advance(
                &diagram,
                &SequenceEvent::ControlStart(SequenceControlStart {
                    model_index: 1,
                    kind: SequenceControlKind::Opt,
                    label: "inner".to_string(),
                    background: None,
                }),
                3,
                &mut resources,
            )
            .unwrap_err();

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxNestingDepth
                    && details.actual == 2
                    && details.max == 1
        ));
        assert_eq!(plan.active_control_frames.len(), 1);
        assert_eq!(plan.control_frames.len(), 1);
    }

    #[test]
    fn event_plan_updates_lifecycle_visibility_and_resets_activation() {
        let mut diagram = diagram(1);
        diagram.lifecycles[0].created_at = Some(1);
        diagram.lifecycles[0].destroyed_at = Some(1);
        let mut resources = test_resources();
        let mut plan = SequenceRowPlanner::new(&diagram, &mut resources).unwrap();
        assert_eq!(plan.visible_actors(), &[false]);

        assert!(
            plan.advance(
                &diagram,
                &SequenceEvent::ActivationStart {
                    actor: 0,
                    model_index: 0,
                },
                3,
                &mut resources,
            )
            .unwrap()
            .is_none()
        );

        let message = SequenceEvent::Message(SequenceMessage {
            model_index: 1,
            from: 0,
            to: 0,
            label: "done".to_string(),
            wrap: false,
            style: SequenceLineStyle::Solid,
            arrow: SequenceArrowHead::Filled,
        });
        let step = plan
            .advance(&diagram, &message, 4, &mut resources)
            .unwrap()
            .expect("message should produce a row step");

        assert_eq!(step.created_actors, &[0]);
        assert_eq!(step.destroyed_actors, &[0]);
        assert_eq!(step.visible_actors, &[true]);
        assert_eq!(plan.visible_actors(), &[false]);
        assert_eq!(plan.active_counts(), &[0]);
    }

    #[test]
    fn message_batch_is_admitted_against_retained_rows_before_rendering() {
        let diagram = diagram(2);
        let base = AsciiRenderOptions::ascii();
        let layout = calculate_layout(&diagram, &base).unwrap();
        let message = SequenceMessage {
            model_index: 0,
            from: 0,
            to: 1,
            label: "next batch".to_string(),
            wrap: false,
            style: SequenceLineStyle::Solid,
            arrow: SequenceArrowHead::Filled,
        };
        let visible_actors = [true, true];
        let mut measuring = ResourceContext::new(base.resources);
        let measured =
            prepare_message_rows(&message, &layout, &visible_actors, &mut measuring).unwrap();
        let width = measured.extent().materialized_width();
        let height = measured.extent().height();
        let batch_cells = width.checked_mul(height).unwrap();

        let limited = base
            .with_resource_limit(AsciiResourceLimitId::MaxGridCells, batch_cells)
            .unwrap();
        let mut resources = ResourceContext::new(limited.resources);
        let mut extent = SequenceExtentLedger::default();
        let retained = SequenceBatchExtent::uniform(height, width, width, &resources).unwrap();
        let reservation = extent.reserve(retained, &mut resources).unwrap();
        let retained_lines = (0..height)
            .map(|_| blank_line(width, layout.width_profile, &resources))
            .collect::<Result<Vec<_>>>()
            .unwrap();
        reservation
            .commit(&mut extent, &retained_lines, &resources)
            .unwrap();

        let prepared =
            prepare_message_rows(&message, &layout, &visible_actors, &mut resources).unwrap();
        let error = extent
            .reserve(prepared.extent(), &mut resources)
            .expect_err("combined rows must be rejected before render_message allocates its rows");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGridCells
                    && details.actual == batch_cells * 2
                    && details.max == batch_cells
        ));
    }

    #[test]
    fn row_plan_wraps_empty_diagram_with_lifeline_and_mirror_rows() {
        let diagram = diagram(2);
        let options = AsciiRenderOptions::ascii().with_sequence_mirror_actors(true);
        let layout = calculate_layout(&diagram, &options).unwrap();
        let mut resources = ResourceContext::new(options.resources);
        let plan = SequenceRowPlan::build(
            &diagram,
            &layout,
            &ascii_chars(),
            options.sequence_mirror_actors,
            &mut resources,
        )
        .unwrap();
        let rendered = plan
            .render(&diagram, &layout, &ascii_chars(), &options, &mut resources)
            .unwrap();
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
        let layout = calculate_layout(&diagram, &options).unwrap();
        let mut resources = ResourceContext::new(options.resources);
        let plan = SequenceRowPlan::build(&diagram, &layout, &ascii_chars(), false, &mut resources)
            .unwrap();

        let rendered = plan
            .render(&diagram, &layout, &ascii_chars(), &options, &mut resources)
            .unwrap();

        assert!(rendered.lines().next().unwrap_or("").contains("Timeline"));
    }

    #[test]
    fn styled_sequence_rows_stream_through_one_output_budget_in_every_mode() {
        for mode in [
            AsciiColorMode::Plain,
            AsciiColorMode::Ansi16,
            AsciiColorMode::Ansi256,
            AsciiColorMode::TrueColor,
            AsciiColorMode::Html,
        ] {
            let base = AsciiRenderOptions::unicode().with_color_mode(mode);
            let expected = finish_sequence_lines(styled_test_lines(&base), &base)
                .expect("unmodified profile should encode styled sequence rows");

            let exact = base
                .with_resource_limit(AsciiResourceLimitId::MaxOutputBytes, expected.len())
                .expect("exact output limit should be valid");
            assert_eq!(
                finish_sequence_lines(styled_test_lines(&exact), &exact)
                    .expect("exact output budget should encode"),
                expected
            );

            let below = base
                .with_resource_limit(
                    AsciiResourceLimitId::MaxOutputBytes,
                    expected.len().saturating_sub(1),
                )
                .expect("limit below encoded output should be valid");
            let error = finish_sequence_lines(styled_test_lines(&below), &below)
                .expect_err("aggregate output budget must reject before partial output escapes");
            assert!(matches!(
                error,
                AsciiError::ResourceLimitExceeded(details)
                    if details.limit == AsciiResourceLimitId::MaxOutputBytes
                        && details.actual > details.max
                        && details.max == expected.len() - 1
            ));
        }
    }

    fn styled_test_lines(options: &AsciiRenderOptions) -> Vec<SequenceLine> {
        let mut first =
            SequenceLine::with_resource_policy(options.terminal_width_profile, options.resources);
        first
            .try_push_role_text("A<&", AsciiColorRole::Text)
            .expect("styled line should fit");
        let mut second =
            SequenceLine::with_resource_policy(options.terminal_width_profile, options.resources);
        second
            .try_push_role_text("B👩🏽‍💻", AsciiColorRole::EdgeArrow)
            .expect("styled line should fit");
        vec![first, second]
    }

    fn diagram(participant_count: usize) -> AsciiSequenceDiagram {
        AsciiSequenceDiagram {
            title: None,
            participants: (0..participant_count)
                .map(|index| SequenceParticipant {
                    id: format!("p{index}"),
                    label: SequenceParticipantLabel::from_raw(
                        &format!("P{index}"),
                        false,
                        TerminalWidthProfile::Unicode,
                    ),
                })
                .collect(),
            lifecycles: vec![SequenceActorLifecycle::default(); participant_count],
            boxes: Vec::new(),
            events: Vec::new(),
        }
    }

    fn test_resources() -> ResourceContext {
        ResourceContext::new(AsciiRenderOptions::ascii().resources)
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
