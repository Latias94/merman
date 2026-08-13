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
use super::text::{SequenceLine, blank_line, charge_text_work, trim_right};
use super::tree::{SequenceControl, SequenceVisit};
use crate::canvas::finish_styled_lines_with_resources;
use crate::color::AsciiColorMode;
use crate::color::AsciiColorRole;
use crate::error::{AsciiError, Result};
use crate::operation::AsciiExecution;
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
use crate::resource::{AsciiResourceLimitPhase, CheckedOutput, ResourceContext};
use crate::text::display_width_with_profile;

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
    ) -> Result<Self> {
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
    ) -> Result<Option<SequenceRowStep<'diagram>>> {
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
                self.record_destroyed_actor_visibility(&step.destroyed_actors);
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

    fn record_destroyed_actor_visibility(&mut self, actor_indices: &[usize]) {
        for actor in actor_indices {
            if let Some(visible) = self.visible_actors.get_mut(*actor) {
                *visible = false;
            }
        }
    }

    fn enter_control(
        &mut self,
        control: &'diagram SequenceControl,
        depth: usize,
        current_row: usize,
        resources: &mut ResourceContext,
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
        let start_boundary = self.capture_boundary(resources)?;
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

    fn exit_control(&mut self, current_row: usize, resources: &mut ResourceContext) -> Result<()> {
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
        let boundary = self.capture_boundary(resources)?;
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
    ) -> Result<SequenceControlBoundaryState> {
        SequenceControlBoundaryState::try_capture(
            &self.active_counts,
            &self.visible_actors,
            resources,
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
    ) -> Result<Self> {
        Self::build_with_materialization_probe(
            diagram,
            layout,
            chars,
            mirror_actors,
            resources,
            || {},
        )
    }

    fn build_with_materialization_probe(
        diagram: &AsciiSequenceDiagram,
        layout: &SequenceLayout,
        chars: &SequenceChars,
        mirror_actors: bool,
        resources: &mut ResourceContext,
        before_materialize: impl FnOnce(),
    ) -> Result<Self> {
        Self::build_controlled(
            diagram,
            layout,
            chars,
            mirror_actors,
            resources,
            None,
            before_materialize,
        )
    }

    pub(super) fn build_with_execution(
        diagram: &AsciiSequenceDiagram,
        layout: &SequenceLayout,
        chars: &SequenceChars,
        mirror_actors: bool,
        resources: &mut ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<Self> {
        Self::build_controlled(
            diagram,
            layout,
            chars,
            mirror_actors,
            resources,
            Some(execution),
            || {},
        )
    }

    fn build_controlled(
        diagram: &AsciiSequenceDiagram,
        layout: &SequenceLayout,
        chars: &SequenceChars,
        mirror_actors: bool,
        resources: &mut ResourceContext,
        execution: Option<AsciiExecution<'_>>,
        before_materialize: impl FnOnce(),
    ) -> Result<Self> {
        if let Some(execution) = execution {
            execution.checkpoint(merman_core::OperationPhase::Layout)?;
        }
        let mut planner = SequenceRowPlanner::new(diagram, resources)?;
        let mut prepared =
            SequencePreparedBody::new(diagram, layout, planner.visible_actors(), resources)?;

        diagram.body.try_visit(resources, |visit, resources| {
            if let Some(execution) = execution {
                execution.checkpoint(merman_core::OperationPhase::Layout)?;
            }
            match visit {
                SequenceVisit::Event(event) => {
                    if let Some(step) = planner.advance(diagram, event, resources)? {
                        prepared.prepare_step(diagram, step, layout, chars, resources)?;
                    }
                }
                SequenceVisit::EnterControl { control, depth } => {
                    planner.enter_control(control, depth, prepared.current_row(), resources)?;
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
                        )?;
                    }
                    planner.enter_section(
                        control,
                        section_index,
                        prepared.current_row(),
                        resources,
                    )?;
                }
                SequenceVisit::ExitControl => {
                    if planner.section_is_empty(prepared.current_row())? {
                        prepared.push_lifeline(
                            planner.active_counts(),
                            planner.visible_actors(),
                            layout,
                            resources,
                        )?;
                    }
                    planner.exit_control(prepared.current_row(), resources)?;
                }
            }
            Ok(())
        })?;
        prepared.finish(
            planner.active_counts(),
            planner.visible_actors(),
            diagram,
            layout,
            mirror_actors,
            resources,
        )?;

        let prepared_controls = prepare_sequence_control_frames(
            planner.finish()?,
            prepared.footprints(),
            layout,
            resources,
        )?;
        let materialization_work = match prepared_controls.as_ref() {
            Some(control) => resources.checked_work_add(
                prepared.materialization_work_units(resources)?,
                control.materialization_work_units(resources)?,
            )?,
            None => prepared.materialization_work_units(resources)?,
        };
        resources.charge_layout_work(materialization_work)?;
        if let Some(execution) = execution {
            execution.checkpoint(merman_core::OperationPhase::Layout)?;
        }
        before_materialize();
        let mut lines = prepared.materialize(diagram, layout, chars, resources)?;
        if let Some(control) = prepared_controls {
            if let Some(execution) = execution {
                execution.checkpoint(merman_core::OperationPhase::Layout)?;
            }
            lines = control.materialize(lines, layout, chars, resources)?;
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
    ) -> Result<String> {
        let mut lines = self.lines;
        if !diagram.boxes.is_empty() {
            lines = render_sequence_boxes(lines, diagram, layout, chars, resources)?;
        }
        if let Some(title) = diagram.title.as_deref() {
            prepend_title_line(&mut lines, title, resources)?;
        }
        finish_sequence_lines(lines, options, resources)
    }

    pub(super) fn render_with_execution(
        self,
        diagram: &AsciiSequenceDiagram,
        layout: &SequenceLayout,
        chars: &SequenceChars,
        options: &AsciiRenderOptions,
        resources: &mut ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<String> {
        let mut lines = self.lines;
        execution.checkpoint(merman_core::OperationPhase::Emit)?;
        if !diagram.boxes.is_empty() {
            lines = render_sequence_boxes(lines, diagram, layout, chars, resources)?;
        }
        execution.checkpoint(merman_core::OperationPhase::Emit)?;
        if let Some(title) = diagram.title.as_deref() {
            prepend_title_line(&mut lines, title, resources)?;
        }
        finish_sequence_lines_with_execution(lines, options, resources, execution)
    }
}

fn finish_sequence_lines(
    lines: Vec<SequenceLine>,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<String> {
    if options.color_mode == AsciiColorMode::Plain {
        let document_resources = resources.scoped();
        let mut output = CheckedOutput::new(options.resources);
        if lines.is_empty() {
            document_resources.charge_layout_work(1)?;
            output.push_char('\n')?;
            return Ok(output.finish());
        }
        for line in lines {
            document_resources.charge_document_cells(line.len())?;
            document_resources.charge_layout_work(line.len().max(1))?;
            line.try_write_plain_to(&mut output)?;
            output.push_char('\n')?;
        }
        return Ok(output.finish());
    }

    if lines.is_empty() {
        return Ok(String::new());
    }

    finish_styled_lines_with_resources(&lines, options, true, resources)
}

fn finish_sequence_lines_with_execution(
    lines: Vec<SequenceLine>,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<String> {
    let width = lines.iter().map(SequenceLine::len).max().unwrap_or(0);
    let grid_cells = width.saturating_mul(lines.len());
    execution.admit_grid(grid_cells)?;
    for _ in &lines {
        execution.checkpoint(merman_core::OperationPhase::Emit)?;
    }
    finish_sequence_lines(lines, options, resources)
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
    use crate::options::AsciiRenderOptions;
    use crate::resource::AsciiResourceLimitId;
    use crate::sequence::events::prepare_message_rows;
    use crate::sequence::layout::{calculate_layout, calculate_layout_with_resources};
    use crate::sequence::model::{
        SequenceActorLifecycle, SequenceArrowHead, SequenceCentralDecoration, SequenceControlKind,
        SequenceEvent, SequenceLineStyle, SequenceMessage, SequenceMessageDirection,
        SequenceParticipant, SequenceParticipantLabel,
    };
    use crate::sequence::prepared_body::{lifeline_batch_extent, participant_box_batch_extent};
    use crate::sequence::text::{SequenceBatchExtent, SequenceExtentLedger};

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
                &mut resources,
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
        let mut plan = SequenceRowPlanner::new(&diagram, &mut resources).unwrap();
        assert_eq!(plan.visible_actors(), &[false]);

        assert!(
            plan.advance(
                &diagram,
                &SequenceEvent::ActivationStart {
                    actor: 0,
                    model_index: 0,
                },
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
            source_marker: SequenceArrowHead::None,
            target_marker: SequenceArrowHead::Filled,
            direction: SequenceMessageDirection::Forward,
            central_decoration: SequenceCentralDecoration::None,
        });
        let step = plan
            .advance(&diagram, &message, &mut resources)
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
        let layout = calculate_layout(&diagram, &base).unwrap();
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
        let materialized = std::cell::Cell::new(false);
        let error = (|| {
            let _reservation = extent.reserve(prepared.extent(), &mut resources)?;
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
        let mut measuring = ResourceContext::new(options.resources);
        let layout = calculate_layout_with_resources(&diagram, &options, &mut measuring).unwrap();
        let visible_actors = initial_visible_actors(&diagram, &measuring).unwrap();
        let header =
            participant_box_batch_extent(&diagram, &layout, &visible_actors, &measuring).unwrap();
        let lifeline = lifeline_batch_extent(&layout, &visible_actors, &measuring).unwrap();
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
        let limited =
            (*options).with_resource_limit(AsciiResourceLimitId::MaxGridCells, maximum)?;
        let mut resources = ResourceContext::new(limited.resources);
        let layout = calculate_layout_with_resources(diagram, &limited, &mut resources)?;
        let chars = ascii_chars();
        SequenceRowPlan::build_with_materialization_probe(
            diagram,
            &layout,
            &chars,
            false,
            &mut resources,
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
        let limited = (*options).with_resource_limit(limit, maximum)?;
        let mut resources = ResourceContext::new(limited.resources);
        let layout = calculate_layout_with_resources(diagram, &limited, &mut resources)?;
        SequenceRowPlan::build_with_materialization_probe(
            diagram,
            &layout,
            &ascii_chars(),
            false,
            &mut resources,
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
        let mut high = options
            .resources
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
        let mut body = crate::sequence::tree::SequenceTreeBuilder::new(3, &resources).unwrap();
        body.start_control(
            0,
            SequenceControlKind::Loop,
            "outer".to_string(),
            None,
            &resources,
        )
        .unwrap();
        body.start_control(
            1,
            SequenceControlKind::Opt,
            "inner".to_string(),
            None,
            &resources,
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
        )
        .unwrap();
        body.end_control(3, SequenceControlKind::Opt, &resources)
            .unwrap();
        body.end_control(4, SequenceControlKind::Loop, &resources)
            .unwrap();
        diagram.body = body.finish().unwrap();
        diagram
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
    fn row_plan_finalization_uses_the_render_wide_layout_work_ledger() {
        let diagram = diagram(1);
        let options = AsciiRenderOptions::ascii();
        let layout = calculate_layout(&diagram, &options).unwrap();
        let mut resources = ResourceContext::new(options.resources);
        let plan = SequenceRowPlan::build(&diagram, &layout, &ascii_chars(), false, &mut resources)
            .unwrap();
        let before_finalization = resources.layout_work_used();

        plan.render(&diagram, &layout, &ascii_chars(), &options, &mut resources)
            .unwrap();

        let total_work = resources.layout_work_used();
        assert!(total_work > before_finalization);

        let exact = options
            .with_resource_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, total_work)
            .unwrap();
        let exact_layout = calculate_layout(&diagram, &exact).unwrap();
        let mut exact_resources = ResourceContext::new(exact.resources);
        let exact_plan = SequenceRowPlan::build(
            &diagram,
            &exact_layout,
            &ascii_chars(),
            false,
            &mut exact_resources,
        )
        .unwrap();
        exact_plan
            .render(
                &diagram,
                &exact_layout,
                &ascii_chars(),
                &exact,
                &mut exact_resources,
            )
            .unwrap();
        assert_eq!(exact_resources.layout_work_used(), total_work);

        let below = options
            .with_resource_limit(
                AsciiResourceLimitId::MaxLayoutWorkUnits,
                total_work.saturating_sub(1),
            )
            .unwrap();
        let below_layout = calculate_layout(&diagram, &below).unwrap();
        let mut below_resources = ResourceContext::new(below.resources);
        let below_plan = SequenceRowPlan::build(
            &diagram,
            &below_layout,
            &ascii_chars(),
            false,
            &mut below_resources,
        )
        .unwrap();
        let error = below_plan
            .render(
                &diagram,
                &below_layout,
                &ascii_chars(),
                &below,
                &mut below_resources,
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
            let expected = finish_styled_test_lines(&base)
                .expect("unmodified profile should encode styled sequence rows");

            let exact = base
                .with_resource_limit(AsciiResourceLimitId::MaxOutputBytes, expected.len())
                .expect("exact output limit should be valid");
            assert_eq!(
                finish_styled_test_lines(&exact).expect("exact output budget should encode"),
                expected
            );

            let below = base
                .with_resource_limit(
                    AsciiResourceLimitId::MaxOutputBytes,
                    expected.len().saturating_sub(1),
                )
                .expect("limit below encoded output should be valid");
            let error = finish_styled_test_lines(&below)
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

    fn finish_styled_test_lines(options: &AsciiRenderOptions) -> Result<String> {
        let mut resources = ResourceContext::new(options.resources);
        finish_sequence_lines(styled_test_lines(options), options, &mut resources)
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
            body: crate::sequence::tree::SequenceBody::default(),
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
            solid_line: '-',
            dotted_line: '.',
            self_top_right: '+',
            self_bottom: '+',
            unicode_markers: false,
        }
    }
}
