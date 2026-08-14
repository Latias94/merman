use super::boxes::render_sequence_boxes;
use super::control::{
    SequenceControlBoundaryState, SequenceControlFrame, SequenceControlFrameForest,
    SequenceControlFrameNode, SequenceControlFrameSeparator, SequenceControlFrameTree,
    prepare_sequence_control_frames,
};
use super::layout::{LifecycleEdge, SequenceLayout, initial_visible_actors, lifecycle_actors_at};
use super::model::{AsciiSequenceDiagram, SequenceEvent};
use super::prepared_body::{SequencePreparedBody, SequenceRowStep};
use super::render::SequenceChars;
#[cfg(test)]
use super::text::blank_line;
use super::text::{SequenceLine, blank_line_with_checkpoints, charge_text_work, trim_right};
use super::tree::{SequenceControl, SequenceVisit};
use super::{SequenceActorRenderState, SequenceCheckpointCursor};
use crate::color::AsciiColorMode;
use crate::color::AsciiColorRole;
use crate::error::{AsciiError, Result};
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
use crate::resource::{AsciiResourceLimitPhase, CheckedOutput, ResourceContext};
use crate::terminal::{TerminalCellText, primary_width};
use crate::text::display_width_with_profile;
use merman_core::OperationPhase;

#[derive(Debug)]
struct SequenceRowPlanner<'diagram> {
    active_counts: Vec<usize>,
    visible_actors: Vec<bool>,
    control_frames: Vec<SequenceControlFrame<'diagram>>,
    control_forest: SequenceControlFrameForest,
    active_control_nodes: Vec<usize>,
}

impl<'diagram> SequenceRowPlanner<'diagram> {
    fn new(
        diagram: &'diagram AsciiSequenceDiagram,
        resources: &mut ResourceContext,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<Self> {
        checkpoints.before_charge()?;
        resources.charge_layout_work(diagram.participants.len())?;
        checkpoints.before_charge()?;
        resources.charge_layout_work(diagram.lifecycles.len())?;
        resources.grid_extent(diagram.participants.len().max(diagram.lifecycles.len()), 1)?;

        let mut active_counts = Vec::new();
        active_counts
            .try_reserve_exact(diagram.participants.len())
            .map_err(|_| allocation_failed())?;
        active_counts.resize(diagram.participants.len(), 0);

        let visible_actors = initial_visible_actors(diagram, resources, checkpoints)?;

        Ok(Self {
            active_counts,
            visible_actors,
            control_frames: Vec::new(),
            control_forest: SequenceControlFrameForest {
                nodes: Vec::new(),
                roots: Vec::new(),
            },
            active_control_nodes: Vec::new(),
        })
    }

    fn active_counts(&self) -> &[usize] {
        &self.active_counts
    }

    fn visible_actors(&self) -> &[bool] {
        &self.visible_actors
    }

    fn advance(
        &mut self,
        diagram: &'diagram AsciiSequenceDiagram,
        event: &'diagram SequenceEvent,
        resources: &mut ResourceContext,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<Option<SequenceRowStep<'diagram>>> {
        checkpoints.before_charge()?;
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
            SequenceEvent::Message(_) | SequenceEvent::Note(_) => {
                let model_index = event.model_index();
                checkpoints.before_charge()?;
                charge_work_product(resources, diagram.lifecycles.len(), 2)?;
                let created_actors = lifecycle_actors_at(
                    diagram,
                    model_index,
                    LifecycleEdge::Created,
                    resources,
                    checkpoints,
                )?;
                if !created_actors.is_empty() {
                    self.record_created_actors(&created_actors, checkpoints)?;
                }
                let destroyed_actors = lifecycle_actors_at(
                    diagram,
                    model_index,
                    LifecycleEdge::Destroyed,
                    resources,
                    checkpoints,
                )?;
                checkpoints.before_charge()?;
                resources.charge_layout_work(self.active_counts.len())?;
                checkpoints.before_charge()?;
                resources.charge_layout_work(self.visible_actors.len())?;
                let step = SequenceRowStep {
                    event,
                    active_counts: try_clone_slice(&self.active_counts)?,
                    visible_actors: try_clone_slice(&self.visible_actors)?,
                    created_actors,
                    destroyed_actors,
                };
                self.record_destroyed_actor_visibility(&step.destroyed_actors, checkpoints)?;
                Ok(Some(step))
            }
        }
    }

    fn record_created_actors(
        &mut self,
        actor_indices: &[usize],
        checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<()> {
        for actor in actor_indices {
            checkpoints.tick()?;
            if let Some(visible) = self.visible_actors.get_mut(*actor) {
                *visible = true;
            }
        }
        Ok(())
    }

    fn record_destroyed_actor_visibility(
        &mut self,
        actor_indices: &[usize],
        checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<()> {
        for actor in actor_indices {
            checkpoints.tick()?;
            if let Some(visible) = self.visible_actors.get_mut(*actor) {
                *visible = false;
            }
        }
        Ok(())
    }

    fn enter_control(
        &mut self,
        control: &'diagram SequenceControl,
        depth: usize,
        current_row: usize,
        resources: &mut ResourceContext,
        checkpoints: &SequenceCheckpointCursor<'_>,
    ) -> Result<()> {
        resources.check_nesting_depth(depth)?;
        let frame_index = self.control_frames.len();
        resources.grid_extent(resources.checked_grid_add(frame_index, 1)?, 1)?;
        let node_index = self.control_forest.nodes.len();
        self.control_frames
            .try_reserve(1)
            .map_err(|_| allocation_failed())?;
        self.control_forest
            .nodes
            .try_reserve(1)
            .map_err(|_| allocation_failed())?;
        self.active_control_nodes
            .try_reserve(1)
            .map_err(|_| allocation_failed())?;
        if let Some(parent_index) = self.active_control_nodes.last().copied() {
            self.control_forest
                .nodes
                .get_mut(parent_index)
                .ok_or_else(|| unsupported("control tree"))?
                .children
                .try_reserve(1)
                .map_err(|_| allocation_failed())?;
        } else {
            self.control_forest
                .roots
                .try_reserve(1)
                .map_err(|_| allocation_failed())?;
        }
        let start_boundary = self.capture_boundary(resources, checkpoints)?;
        self.control_frames.push(SequenceControlFrame {
            kind: control.kind,
            label: &control.label,
            background: control.background,
            participant_span: control.participant_span,
            start_boundary,
            start_row: current_row,
            separators: Vec::new(),
            end_boundary: None,
            end_row: None,
        });
        self.control_forest.nodes.push(SequenceControlFrameNode {
            frame_index,
            children: Vec::new(),
            depth,
        });
        if let Some(parent_index) = self.active_control_nodes.last().copied() {
            self.control_forest.nodes[parent_index]
                .children
                .push(node_index);
        } else {
            self.control_forest.roots.push(node_index);
        }
        self.active_control_nodes.push(node_index);
        Ok(())
    }

    fn enter_section(
        &mut self,
        control: &'diagram SequenceControl,
        section_index: usize,
        current_row: usize,
        resources: &mut ResourceContext,
        checkpoints: &SequenceCheckpointCursor<'_>,
    ) -> Result<()> {
        let Some(node_index) = self.active_control_nodes.last().copied() else {
            return Err(unsupported("control tree"));
        };
        let frame_index = self
            .control_forest
            .nodes
            .get(node_index)
            .ok_or_else(|| unsupported("control tree"))?
            .frame_index;
        let frame = self
            .control_frames
            .get_mut(frame_index)
            .ok_or_else(|| unsupported("control tree"))?;
        if section_index == 0 {
            return Ok(());
        }
        let section = control
            .sections
            .get(section_index)
            .and_then(|section| section.separator.as_ref())
            .ok_or_else(|| unsupported("control tree"))?;
        let boundary = SequenceControlBoundaryState::try_capture(
            &self.active_counts,
            &self.visible_actors,
            resources,
            checkpoints,
        )?;
        frame
            .separators
            .try_reserve(1)
            .map_err(|_| allocation_failed())?;
        frame.separators.push(SequenceControlFrameSeparator {
            label: &section.label,
            boundary,
            row: current_row,
        });
        Ok(())
    }

    fn exit_control(
        &mut self,
        current_row: usize,
        resources: &mut ResourceContext,
        checkpoints: &SequenceCheckpointCursor<'_>,
    ) -> Result<()> {
        let node_index = self
            .active_control_nodes
            .pop()
            .ok_or_else(|| unsupported("control tree"))?;
        let frame_index = self
            .control_forest
            .nodes
            .get(node_index)
            .ok_or_else(|| unsupported("control tree"))?
            .frame_index;
        let boundary = self.capture_boundary(resources, checkpoints)?;
        let frame = self
            .control_frames
            .get_mut(frame_index)
            .ok_or_else(|| unsupported("control tree"))?;
        frame.end_boundary = Some(boundary);
        frame.end_row = Some(
            current_row
                .checked_sub(1)
                .ok_or_else(|| unsupported("control row range"))?,
        );
        Ok(())
    }

    fn section_is_empty(&self, current_row: usize) -> Result<bool> {
        let Some(node_index) = self.active_control_nodes.last().copied() else {
            return Err(unsupported("control tree"));
        };
        let frame_index = self.control_forest.nodes[node_index].frame_index;
        let frame = &self.control_frames[frame_index];
        Ok(frame.current_section_start_row() == current_row)
    }

    fn capture_boundary(
        &self,
        resources: &mut ResourceContext,
        checkpoints: &SequenceCheckpointCursor<'_>,
    ) -> Result<SequenceControlBoundaryState> {
        SequenceControlBoundaryState::try_capture(
            &self.active_counts,
            &self.visible_actors,
            resources,
            checkpoints,
        )
    }

    fn finish(self) -> Result<SequenceControlFrameTree<'diagram>> {
        if !self.active_control_nodes.is_empty() {
            return Err(unsupported("control tree"));
        }
        Ok(SequenceControlFrameTree {
            forest: self.control_forest,
            frames: self.control_frames,
        })
    }
}

#[derive(Debug)]
pub(super) struct SequenceRowPlan {
    lines: Vec<SequenceLine>,
}

impl SequenceRowPlan {
    pub(super) fn build(
        diagram: &AsciiSequenceDiagram,
        layout: &SequenceLayout,
        chars: &SequenceChars,
        mirror_actors: bool,
        resources: &mut ResourceContext,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<Self> {
        Self::build_with_materialization_probe(
            diagram,
            layout,
            chars,
            mirror_actors,
            resources,
            checkpoints,
            || {},
        )
    }

    fn build_with_materialization_probe(
        diagram: &AsciiSequenceDiagram,
        layout: &SequenceLayout,
        chars: &SequenceChars,
        mirror_actors: bool,
        resources: &mut ResourceContext,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
        before_materialize: impl FnOnce(),
    ) -> Result<Self> {
        checkpoints.before_charge()?;
        let mut planner = SequenceRowPlanner::new(diagram, resources, checkpoints)?;
        let mut prepared = SequencePreparedBody::new(
            diagram,
            layout,
            planner.visible_actors(),
            resources,
            checkpoints,
        )?;

        diagram
            .body
            .try_visit(resources, checkpoints.execution(), |visit, resources| {
                checkpoints.tick()?;
                match visit {
                    SequenceVisit::Event(event) => {
                        if let Some(step) =
                            planner.advance(diagram, event, resources, checkpoints)?
                        {
                            prepared.prepare_step(
                                diagram,
                                step,
                                layout,
                                chars,
                                resources,
                                checkpoints,
                            )?;
                        }
                    }
                    SequenceVisit::EnterControl { control, depth } => {
                        planner.enter_control(
                            control,
                            depth,
                            prepared.current_row(),
                            resources,
                            checkpoints,
                        )?;
                    }
                    SequenceVisit::EnterSection {
                        control,
                        section_index,
                    } => {
                        if section_index > 0 && planner.section_is_empty(prepared.current_row())? {
                            prepared.push_lifeline(
                                planner.active_counts(),
                                planner.visible_actors(),
                                layout,
                                resources,
                                checkpoints,
                            )?;
                        }
                        planner.enter_section(
                            control,
                            section_index,
                            prepared.current_row(),
                            resources,
                            checkpoints,
                        )?;
                    }
                    SequenceVisit::ExitControl => {
                        if planner.section_is_empty(prepared.current_row())? {
                            prepared.push_lifeline(
                                planner.active_counts(),
                                planner.visible_actors(),
                                layout,
                                resources,
                                checkpoints,
                            )?;
                        }
                        planner.exit_control(prepared.current_row(), resources, checkpoints)?;
                    }
                }
                Ok(())
            })?;
        prepared.finish(
            SequenceActorRenderState::new(planner.active_counts(), planner.visible_actors()),
            diagram,
            layout,
            mirror_actors,
            resources,
            checkpoints,
        )?;

        let control_tree = planner.finish()?;
        let prepared_controls = prepare_sequence_control_frames(
            control_tree,
            prepared.footprints(),
            layout,
            resources,
            checkpoints,
        )?;
        let materialization_work = match prepared_controls.as_ref() {
            Some(control) => {
                let control_work = control.materialization_work_units(resources, checkpoints)?;
                resources.checked_work_add(
                    prepared.materialization_work_units(resources, checkpoints)?,
                    control_work,
                )?
            }
            None => prepared.materialization_work_units(resources, checkpoints)?,
        };
        charge_materialization_work(materialization_work, resources, checkpoints)?;
        checkpoints.tick()?;
        before_materialize();
        let mut lines = prepared.materialize(diagram, layout, chars, resources, checkpoints)?;
        if let Some(control) = prepared_controls {
            checkpoints.tick()?;
            lines = control.materialize(lines, layout, chars, resources, checkpoints)?;
        }

        Ok(Self { lines })
    }

    pub(super) fn render(
        self,
        diagram: &AsciiSequenceDiagram,
        layout: &SequenceLayout,
        chars: &SequenceChars,
        options: &AsciiRenderOptions,
        resources: &mut ResourceContext,
        layout_checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<String> {
        let mut lines = self.lines;
        if !diagram.boxes.is_empty() {
            lines = render_sequence_boxes(
                lines,
                diagram,
                layout,
                chars,
                resources,
                layout_checkpoints,
            )?;
        }
        if let Some(title) = diagram.title.as_deref() {
            prepend_title_line(&mut lines, title, resources, layout_checkpoints)?;
        }
        // Box/title geometry, extent admission, and canvas construction are layout work.  Only
        // after those complete do we bind the shared ledger to Emit for byte/document emission.
        let mut emit_resources = layout_checkpoints
            .execution()
            .resource_context(resources, OperationPhase::Emit);
        let mut emit_checkpoints = layout_checkpoints.next_phase(OperationPhase::Emit);
        finish_sequence_lines(lines, options, &mut emit_resources, &mut emit_checkpoints)
    }
}

fn finish_sequence_lines(
    lines: Vec<SequenceLine>,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<String> {
    if options.color_mode == AsciiColorMode::Plain {
        let document_resources = resources.scoped();
        let mut output = CheckedOutput::new(resources.policy());
        if lines.is_empty() {
            checkpoints.before_charge()?;
            document_resources.charge_layout_work(1)?;
            checkpoints.before_charge()?;
            output.push_char('\n')?;
            return Ok(output.finish());
        }
        for line in lines {
            checkpoints.before_charge()?;
            document_resources.charge_document_cells(line.len())?;
            checkpoints.before_charge()?;
            document_resources.charge_layout_work(line.len().max(1))?;
            write_plain_sequence_line(&line, &mut output, checkpoints)?;
            checkpoints.before_charge()?;
            output.push_char('\n')?;
        }
        return Ok(output.finish());
    }

    if lines.is_empty() {
        return Ok(String::new());
    }

    let deferred = crate::safe_text::DeferredTextRegistry::new();
    crate::canvas::finish_styled_line_iter_with_deferred_resources_with_execution(
        lines.iter(),
        options,
        true,
        resources,
        &deferred,
        checkpoints.execution(),
    )
}

fn write_plain_sequence_line(
    line: &SequenceLine,
    output: &mut CheckedOutput,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<()> {
    let mut offset = 0usize;
    while let Some(cell) = line.surface_cells().get(offset).copied() {
        checkpoints.before_charge()?;
        if let Some(text) = cell.try_output_text(line.surface_arena())? {
            match text {
                TerminalCellText::Scalar(ch) => output.push_char(ch)?,
                TerminalCellText::Grapheme(grapheme) => output.push_str(grapheme)?,
            }
        }
        offset = offset
            .checked_add(primary_width(line.surface_cells(), offset).max(1))
            .ok_or_else(allocation_failed)?;
    }
    Ok(())
}

fn prepend_title_line(
    lines: &mut Vec<SequenceLine>,
    title: &str,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<()> {
    let mut width = 0usize;
    for line in lines.iter() {
        checkpoints.tick()?;
        width = width.max(line.len());
    }
    let width_profile = lines
        .first()
        .map(SequenceLine::width_profile)
        .unwrap_or(TerminalWidthProfile::Unicode);
    charge_text_work(title, width_profile, resources, checkpoints)?;
    let title_width = display_width_with_profile(title, width_profile);
    let height = resources.checked_grid_add(lines.len(), 1)?;
    resources.grid_extent(width.max(title_width), height)?;
    checkpoints.before_charge()?;
    resources.charge_layout_work(title_width.max(1))?;
    lines.try_reserve(1).map_err(|_| allocation_failed())?;
    lines.insert(
        0,
        render_title_line(title, width, width_profile, resources, checkpoints)?,
    );
    Ok(())
}

fn render_title_line(
    title: &str,
    width: usize,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceLine> {
    let title_width = display_width_with_profile(title, width_profile);
    let left = width.saturating_sub(title_width) / 2;
    let mut line = blank_line_with_checkpoints(left, width_profile, resources, checkpoints)?;
    line.try_push_role_text_with_checkpoint(title, AsciiColorRole::Text, || checkpoints.tick())?;
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

fn charge_materialization_work(
    work: usize,
    resources: &mut ResourceContext,
    checkpoints: &SequenceCheckpointCursor<'_>,
) -> Result<()> {
    checkpoints.before_charge()?;
    resources.charge_layout_work(work)
}

fn work_overflow(resources: &ResourceContext) -> AsciiError {
    resources.work_overflow()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::AsciiColorMode;
    use crate::operation::AsciiExecution;
    use crate::options::AsciiRenderOptions;
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use crate::sequence::events::prepare_message_rows;
    use crate::sequence::layout::{calculate_layout, calculate_layout_with_resources};
    use crate::sequence::model::{
        SequenceActorLifecycle, SequenceArrowHead, SequenceCentralDecoration, SequenceControlKind,
        SequenceEvent, SequenceGroupBox, SequenceLineStyle, SequenceMessage,
        SequenceMessageDirection, SequenceParticipant, SequenceParticipantLabel,
    };
    use crate::sequence::prepared_body::{lifeline_batch_extent, participant_box_batch_extent};
    use crate::sequence::text::{SequenceBatchExtent, SequenceExtentLedger};
    use merman_core::{OperationControl, OperationPhase};

    #[test]
    fn event_plan_tracks_activation_counts() {
        let diagram = diagram(1);
        let mut resources = test_resources();
        let policy = resources.policy();
        let mut checkpoints = layout_checkpoints(&policy);
        let mut plan = SequenceRowPlanner::new(&diagram, &mut resources, &mut checkpoints).unwrap();

        assert!(
            plan.advance(
                &diagram,
                &SequenceEvent::ActivationStart {
                    actor: 0,
                    model_index: 0,
                },
                &mut resources,
                &mut checkpoints,
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
                &mut resources,
                &mut checkpoints,
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(plan.active_counts(), &[0]);
    }

    #[test]
    fn event_plan_updates_lifecycle_visibility_without_erasing_activation_state() {
        let mut diagram = diagram(1);
        diagram.lifecycles[0].created_at = Some(1);
        diagram.lifecycles[0].destroyed_at = Some(1);
        let mut resources = test_resources();
        let policy = resources.policy();
        let mut checkpoints = layout_checkpoints(&policy);
        let mut plan = SequenceRowPlanner::new(&diagram, &mut resources, &mut checkpoints).unwrap();
        assert_eq!(plan.visible_actors(), &[false]);

        assert!(
            plan.advance(
                &diagram,
                &SequenceEvent::ActivationStart {
                    actor: 0,
                    model_index: 0,
                },
                &mut resources,
                &mut checkpoints,
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
            source_marker: SequenceArrowHead::None,
            target_marker: SequenceArrowHead::Filled,
            direction: SequenceMessageDirection::Forward,
            central_decoration: SequenceCentralDecoration::None,
        });
        let step = plan
            .advance(&diagram, &message, &mut resources, &mut checkpoints)
            .unwrap()
            .expect("message should produce a row step");

        assert_eq!(step.created_actors, &[0]);
        assert_eq!(step.destroyed_actors, &[0]);
        assert_eq!(step.visible_actors, &[true]);
        assert_eq!(plan.visible_actors(), &[false]);
        assert_eq!(plan.active_counts(), &[1]);

        assert!(
            plan.advance(
                &diagram,
                &SequenceEvent::ActivationEnd {
                    actor: 0,
                    model_index: 2,
                },
                &mut resources,
                &mut checkpoints,
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(plan.active_counts(), &[0]);
    }

    #[test]
    fn message_batch_is_admitted_against_retained_rows_before_rendering() {
        let diagram = diagram(2);
        let base = AsciiRenderOptions::ascii();
        let base_policy = AsciiResourcePolicy::default();
        let layout = calculate_layout(&diagram, &base, &base_policy).unwrap();
        let message = SequenceMessage {
            model_index: 0,
            from: 0,
            to: 1,
            label: "next batch".to_string(),
            wrap: false,
            style: SequenceLineStyle::Solid,
            source_marker: SequenceArrowHead::None,
            target_marker: SequenceArrowHead::Filled,
            direction: SequenceMessageDirection::Forward,
            central_decoration: SequenceCentralDecoration::None,
        };
        let visible_actors = [true, true];
        let mut measuring = ResourceContext::new(base_policy);
        let mut measuring_checkpoints = layout_checkpoints(&base_policy);
        let measured = prepare_message_rows(
            &message,
            &layout,
            &visible_actors,
            &mut measuring,
            &mut measuring_checkpoints,
        )
        .unwrap();
        let width = measured.extent().materialized_width();
        let height = measured.extent().height();
        let batch_cells = width.checked_mul(height).unwrap();

        let limited = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, batch_cells)
            .unwrap();
        let mut resources = ResourceContext::new(limited);
        let mut checkpoints = layout_checkpoints(&limited);
        let mut extent = SequenceExtentLedger::default();
        let retained = SequenceBatchExtent::uniform(height, width, width, &resources).unwrap();
        let reservation = extent
            .reserve(retained, &mut resources, &checkpoints)
            .unwrap();
        let retained_lines = (0..height)
            .map(|_| blank_line(width, layout.width_profile, &resources))
            .collect::<Result<Vec<_>>>()
            .unwrap();
        reservation
            .commit(&mut extent, &retained_lines, &resources)
            .unwrap();

        let prepared = prepare_message_rows(
            &message,
            &layout,
            &visible_actors,
            &mut resources,
            &mut checkpoints,
        )
        .unwrap();
        let materialized = std::cell::Cell::new(false);
        let error = (|| {
            let _reservation = extent.reserve(prepared.extent(), &mut resources, &checkpoints)?;
            prepared.materialize_label_with_probe(&message.label, &resources, &materialized)
        })()
        .expect_err("combined rows must be rejected before render_message allocates its rows");

        assert!(!materialized.get());
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGridCells
                    && details.actual == batch_cells * 2
                    && details.max == batch_cells
        ));
    }

    #[test]
    fn participant_labels_materialize_after_aggregate_box_admission() {
        let diagram = diagram(2);
        let options = AsciiRenderOptions::ascii();
        let policy = AsciiResourcePolicy::default();
        let mut measuring = ResourceContext::new(policy);
        let mut checkpoints = layout_checkpoints(&policy);
        let layout =
            calculate_layout_with_resources(&diagram, &options, &mut measuring, &mut checkpoints)
                .unwrap();
        let visible_actors =
            initial_visible_actors(&diagram, &measuring, &mut checkpoints).unwrap();
        let header = participant_box_batch_extent(
            &diagram,
            &layout,
            &visible_actors,
            &measuring,
            &mut checkpoints,
        )
        .unwrap();
        let lifeline =
            lifeline_batch_extent(&layout, &visible_actors, &measuring, &mut checkpoints).unwrap();
        let total_height = header.height() + lifeline.height();
        let total_width = header
            .materialized_width()
            .max(lifeline.materialized_width());
        let aggregate_cells = total_width.checked_mul(total_height).unwrap();

        let exact_materialized = std::cell::Cell::new(false);
        build_row_plan_with_grid_limit(&diagram, &options, aggregate_cells, &exact_materialized)
            .expect("the exact row-plan grid should be admitted");
        assert!(exact_materialized.get());

        let below_materialized = std::cell::Cell::new(false);
        let error = build_row_plan_with_grid_limit(
            &diagram,
            &options,
            aggregate_cells - 1,
            &below_materialized,
        )
        .expect_err("the row-plan grid should reject its limit minus one");
        assert!(!below_materialized.get());
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGridCells
                    && details.actual == aggregate_cells
                    && details.max == aggregate_cells - 1
        ));
    }

    #[test]
    fn control_row_plan_admits_grid_and_document_limits_before_materialization() {
        let diagram = nested_control_diagram();
        let options = AsciiRenderOptions::ascii();

        for limit in [
            AsciiResourceLimitId::MaxGridCells,
            AsciiResourceLimitId::MaxDocumentCells,
        ] {
            let exact = first_admitted_row_plan_limit(&diagram, &options, limit);
            let exact_materialized = std::cell::Cell::new(false);
            build_row_plan_with_limit(&diagram, &options, limit, exact, &exact_materialized)
                .expect("the exact aggregate row-plan limit should be admitted");
            assert!(exact_materialized.get());

            let below_materialized = std::cell::Cell::new(false);
            let error = build_row_plan_with_limit(
                &diagram,
                &options,
                limit,
                exact - 1,
                &below_materialized,
            )
            .expect_err("the aggregate row-plan limit minus one should be rejected");
            assert!(!below_materialized.get());
            assert!(matches!(
                error,
                AsciiError::ResourceLimitExceeded(details)
                    if details.limit == limit
                        && details.actual == exact
                        && details.max == exact - 1
            ));
        }
    }

    fn build_row_plan_with_grid_limit(
        diagram: &AsciiSequenceDiagram,
        options: &AsciiRenderOptions,
        maximum: usize,
        materialized: &std::cell::Cell<bool>,
    ) -> Result<()> {
        let limited = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, maximum)
            .expect("the test grid limit should be valid");
        let mut resources = ResourceContext::new(limited);
        let mut checkpoints = layout_checkpoints(&limited);
        let layout =
            calculate_layout_with_resources(diagram, options, &mut resources, &mut checkpoints)?;
        let chars = ascii_chars();
        SequenceRowPlan::build_with_materialization_probe(
            diagram,
            &layout,
            &chars,
            false,
            &mut resources,
            &mut checkpoints,
            || materialized.set(true),
        )?;
        Ok(())
    }

    fn build_row_plan_with_limit(
        diagram: &AsciiSequenceDiagram,
        options: &AsciiRenderOptions,
        limit: AsciiResourceLimitId,
        maximum: usize,
        materialized: &std::cell::Cell<bool>,
    ) -> Result<()> {
        let limited = AsciiResourcePolicy::default()
            .with_limit(limit, maximum)
            .expect("the test resource limit should be valid");
        let mut resources = ResourceContext::new(limited);
        let mut checkpoints = layout_checkpoints(&limited);
        let layout =
            calculate_layout_with_resources(diagram, options, &mut resources, &mut checkpoints)?;
        SequenceRowPlan::build_with_materialization_probe(
            diagram,
            &layout,
            &ascii_chars(),
            false,
            &mut resources,
            &mut checkpoints,
            || materialized.set(true),
        )?;
        Ok(())
    }

    fn first_admitted_row_plan_limit(
        diagram: &AsciiSequenceDiagram,
        options: &AsciiRenderOptions,
        limit: AsciiResourceLimitId,
    ) -> usize {
        let mut low = 1usize;
        let mut high = AsciiResourcePolicy::default()
            .value(limit)
            .expect("the test profile should bound sequence resources");
        while low < high {
            let mid = low + (high - low) / 2;
            let materialized = std::cell::Cell::new(false);
            match build_row_plan_with_limit(diagram, options, limit, mid, &materialized) {
                Ok(()) => high = mid,
                Err(AsciiError::ResourceLimitExceeded(details)) if details.limit == limit => {
                    low = mid + 1;
                }
                Err(error) => panic!("unexpected sequence admission error: {error}"),
            }
        }
        low
    }

    fn nested_control_diagram() -> AsciiSequenceDiagram {
        let mut diagram = diagram(2);
        let resources = test_resources();
        let policy = resources.policy();
        let execution = AsciiExecution::standalone(&policy);
        let mut body =
            crate::sequence::tree::SequenceTreeBuilder::new(3, &resources, execution).unwrap();
        body.start_control(
            0,
            SequenceControlKind::Loop,
            "outer".to_string(),
            None,
            &resources,
            execution,
        )
        .unwrap();
        body.start_control(
            1,
            SequenceControlKind::Opt,
            "inner".to_string(),
            None,
            &resources,
            execution,
        )
        .unwrap();
        body.push_event(
            SequenceEvent::Message(SequenceMessage {
                model_index: 2,
                from: 0,
                to: 1,
                label: "work".to_string(),
                wrap: false,
                style: SequenceLineStyle::Solid,
                source_marker: SequenceArrowHead::None,
                target_marker: SequenceArrowHead::Filled,
                direction: SequenceMessageDirection::Forward,
                central_decoration: SequenceCentralDecoration::None,
            }),
            &resources,
            execution,
        )
        .unwrap();
        body.end_control(3, SequenceControlKind::Opt, &resources, execution)
            .unwrap();
        body.end_control(4, SequenceControlKind::Loop, &resources, execution)
            .unwrap();
        diagram.body = body.finish().unwrap();
        diagram
    }

    #[test]
    fn row_plan_wraps_empty_diagram_with_lifeline_and_mirror_rows() {
        let diagram = diagram(2);
        let options = AsciiRenderOptions::ascii().with_sequence_mirror_actors(true);
        let policy = AsciiResourcePolicy::default();
        let layout = calculate_layout(&diagram, &options, &policy).unwrap();
        let mut resources = ResourceContext::new(policy);
        let mut layout_cursor = layout_checkpoints(&policy);
        let plan = SequenceRowPlan::build(
            &diagram,
            &layout,
            &ascii_chars(),
            options.sequence_mirror_actors,
            &mut resources,
            &mut layout_cursor,
        )
        .unwrap();
        let rendered = plan
            .render(
                &diagram,
                &layout,
                &ascii_chars(),
                &options,
                &mut resources,
                &mut layout_cursor,
            )
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
        let policy = AsciiResourcePolicy::default();
        let layout = calculate_layout(&diagram, &options, &policy).unwrap();
        let mut resources = ResourceContext::new(policy);
        let mut layout_cursor = layout_checkpoints(&policy);
        let plan = SequenceRowPlan::build(
            &diagram,
            &layout,
            &ascii_chars(),
            false,
            &mut resources,
            &mut layout_cursor,
        )
        .unwrap();

        let rendered = plan
            .render(
                &diagram,
                &layout,
                &ascii_chars(),
                &options,
                &mut resources,
                &mut layout_cursor,
            )
            .unwrap();

        assert!(rendered.lines().next().unwrap_or("").contains("Timeline"));
    }

    #[test]
    fn row_plan_finalization_uses_the_render_wide_layout_work_ledger() {
        let diagram = diagram(1);
        let options = AsciiRenderOptions::ascii();
        let policy = AsciiResourcePolicy::default();
        let layout = calculate_layout(&diagram, &options, &policy).unwrap();
        let mut resources = ResourceContext::new(policy);
        let mut layout_cursor = layout_checkpoints(&policy);
        let plan = SequenceRowPlan::build(
            &diagram,
            &layout,
            &ascii_chars(),
            false,
            &mut resources,
            &mut layout_cursor,
        )
        .unwrap();
        let before_finalization = resources.layout_work_used();

        plan.render(
            &diagram,
            &layout,
            &ascii_chars(),
            &options,
            &mut resources,
            &mut layout_cursor,
        )
        .unwrap();

        let total_work = resources.layout_work_used();
        assert!(total_work > before_finalization);

        let exact = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, total_work)
            .unwrap();
        let exact_layout = calculate_layout(&diagram, &options, &exact).unwrap();
        let mut exact_resources = ResourceContext::new(exact);
        let mut exact_layout_checkpoints = layout_checkpoints(&exact);
        let exact_plan = SequenceRowPlan::build(
            &diagram,
            &exact_layout,
            &ascii_chars(),
            false,
            &mut exact_resources,
            &mut exact_layout_checkpoints,
        )
        .unwrap();
        exact_plan
            .render(
                &diagram,
                &exact_layout,
                &ascii_chars(),
                &options,
                &mut exact_resources,
                &mut exact_layout_checkpoints,
            )
            .unwrap();
        assert_eq!(exact_resources.layout_work_used(), total_work);

        let below = AsciiResourcePolicy::default()
            .with_limit(
                AsciiResourceLimitId::MaxLayoutWorkUnits,
                total_work.saturating_sub(1),
            )
            .unwrap();
        let below_layout = calculate_layout(&diagram, &options, &below).unwrap();
        let mut below_resources = ResourceContext::new(below);
        let mut below_layout_checkpoints = layout_checkpoints(&below);
        let below_plan = SequenceRowPlan::build(
            &diagram,
            &below_layout,
            &ascii_chars(),
            false,
            &mut below_resources,
            &mut below_layout_checkpoints,
        )
        .unwrap();
        let error = below_plan
            .render(
                &diagram,
                &below_layout,
                &ascii_chars(),
                &options,
                &mut below_resources,
                &mut below_layout_checkpoints,
            )
            .expect_err("finalization must observe prior layout work");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == total_work
                    && details.max == total_work - 1
        ));
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
            let base_policy = AsciiResourcePolicy::default();
            let expected = finish_styled_test_lines(&base, &base_policy)
                .expect("unmodified profile should encode styled sequence rows");

            let exact = AsciiResourcePolicy::default()
                .with_limit(AsciiResourceLimitId::MaxOutputBytes, expected.len())
                .expect("exact output limit should be valid");
            assert_eq!(
                finish_styled_test_lines(&base, &exact).expect("exact output budget should encode"),
                expected
            );

            let below = AsciiResourcePolicy::default()
                .with_limit(
                    AsciiResourceLimitId::MaxOutputBytes,
                    expected.len().saturating_sub(1),
                )
                .expect("limit below encoded output should be valid");
            let error = finish_styled_test_lines(&base, &below)
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

    #[test]
    fn plain_emit_cancellation_precedes_the_limit_minus_one_output_write() {
        let options = AsciiRenderOptions::ascii();
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxOutputBytes, 64)
            .expect("the output limit should be valid");
        let mut resources = ResourceContext::new(policy);
        let line =
            SequenceLine::plain_text_with_profile(&"x".repeat(128), options.terminal_width_profile);
        let control = OperationControl::new();
        control.cancel_after_checkpoints(66);
        let execution = AsciiExecution::new(&control, &policy);
        let mut checkpoints = SequenceCheckpointCursor::new(execution, OperationPhase::Emit);

        let error = finish_sequence_lines(vec![line], &options, &mut resources, &mut checkpoints)
            .expect_err("the 65th byte should observe cancellation before exceeding 64 bytes");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Emit
                    && cancelled.reason == merman_core::CancelReason::Requested
        ));
    }

    #[test]
    fn layout_materialization_cancellation_precedes_the_limit_minus_one_work_charge() {
        const MATERIALIZATION_WORK: usize = 64;
        let policy = AsciiResourcePolicy::default()
            .with_limit(
                AsciiResourceLimitId::MaxLayoutWorkUnits,
                MATERIALIZATION_WORK - 1,
            )
            .expect("the layout-work limit should be valid");
        let mut resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        control.cancel_after_checkpoints(0);
        let execution = AsciiExecution::new(&control, &policy);
        let checkpoints = SequenceCheckpointCursor::new(execution, OperationPhase::Layout);

        let error = charge_materialization_work(MATERIALIZATION_WORK, &mut resources, &checkpoints)
            .expect_err("materialization cancellation should precede the limit-minus-one charge");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == merman_core::CancelReason::Requested
        ));
        assert_eq!(resources.layout_work_used(), 0);
    }

    #[test]
    fn box_geometry_cancellation_is_reported_as_layout_before_emit() {
        let options = AsciiRenderOptions::ascii();
        let policy = AsciiResourcePolicy::default();
        let mut diagram = diagram(1);
        diagram.boxes.push(SequenceGroupBox {
            actor_indices: vec![0],
            label: Some("group".to_string()),
            background: None,
            wrap: false,
        });
        let layout = calculate_layout(&diagram, &options, &policy)
            .expect("the box fixture layout should fit");
        let base_resources = ResourceContext::new(policy);
        let lines = vec![
            blank_line(
                layout.total_width + 1,
                layout.width_profile,
                &base_resources,
            )
            .expect("the planned body line should fit"),
        ];
        let plan = SequenceRowPlan { lines };
        let control = OperationControl::new();
        control.cancel_after_checkpoints(0);
        let execution = AsciiExecution::new(&control, &policy);
        let mut layout_resources =
            execution.resource_context(&base_resources, OperationPhase::Layout);
        let mut checkpoints = SequenceCheckpointCursor::new(execution, OperationPhase::Layout);

        let error = plan
            .render(
                &diagram,
                &layout,
                &ascii_chars(),
                &options,
                &mut layout_resources,
                &mut checkpoints,
            )
            .expect_err("box geometry should observe layout cancellation before emission starts");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == merman_core::CancelReason::Requested
        ));
    }

    fn finish_styled_test_lines(
        options: &AsciiRenderOptions,
        policy: &AsciiResourcePolicy,
    ) -> Result<String> {
        let mut resources = ResourceContext::new(*policy);
        let execution = AsciiExecution::standalone(policy);
        let mut checkpoints = SequenceCheckpointCursor::new(execution, OperationPhase::Emit);
        finish_sequence_lines(
            styled_test_lines(options, *policy),
            options,
            &mut resources,
            &mut checkpoints,
        )
    }

    fn styled_test_lines(
        options: &AsciiRenderOptions,
        policy: AsciiResourcePolicy,
    ) -> Vec<SequenceLine> {
        let mut first = SequenceLine::with_resource_policy(options.terminal_width_profile, policy);
        first
            .try_push_role_text("A<&", AsciiColorRole::Text)
            .expect("styled line should fit");
        let mut second = SequenceLine::with_resource_policy(options.terminal_width_profile, policy);
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
            body: crate::sequence::tree::SequenceBody::default(),
        }
    }

    fn test_resources() -> ResourceContext {
        ResourceContext::new(AsciiResourcePolicy::default())
    }

    fn layout_checkpoints(policy: &AsciiResourcePolicy) -> SequenceCheckpointCursor<'_> {
        SequenceCheckpointCursor::new(AsciiExecution::standalone(policy), OperationPhase::Layout)
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
            solid_line: '-',
            dotted_line: '.',
            self_top_right: '+',
            self_bottom: '+',
            unicode_markers: false,
        }
    }
}
