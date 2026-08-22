use super::chars::SequenceChars;
use super::control::{
    PreparedSequenceControlFrames, SequenceControlBoundary, SequenceControlFrame,
    SequenceControlFrameForest, SequenceControlFrameNode, SequenceControlFrameSeparator,
    SequenceControlFrameTree, prepare_sequence_control_frames,
};
use super::layout::{LifecycleEdge, SequenceLayout, initial_visible_actors, lifecycle_actors_at};
use super::lifeline::retained_lifeline_width;
use super::model::{AsciiSequenceDiagram, SequenceEvent};
use super::prepared_body::{SequencePreparedBody, SequenceRowStep};
use super::row_document::SequenceRowDocument;
use super::text::SequenceDocumentPlan;
use super::tree::{SequenceControl, SequenceVisit};
use super::{SequenceActorRenderState, SequenceCheckpointCursor};
use crate::error::{AsciiError, Result};
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};

#[derive(Debug)]
struct SequenceRowPlanner<'diagram> {
    active_counts: Vec<usize>,
    visible_actors: Vec<bool>,
    control_frames: Vec<SequenceControlFrame<'diagram>>,
    control_forest: SequenceControlFrameForest,
    active_control_nodes: Vec<usize>,
}

#[derive(Debug)]
struct SequenceRowTransition<'diagram> {
    event: &'diagram SequenceEvent,
    created_actors: Vec<usize>,
    destroyed_actors: Vec<usize>,
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
    ) -> Result<Option<SequenceRowTransition<'diagram>>> {
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
                Ok(Some(SequenceRowTransition {
                    event,
                    created_actors,
                    destroyed_actors,
                }))
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
        boundary_width: usize,
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
        let start_boundary = self.capture_boundary(boundary_width, resources, checkpoints)?;
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
        boundary_width: usize,
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
        let boundary = SequenceControlBoundary::try_capture(
            &self.active_counts,
            &self.visible_actors,
            boundary_width,
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
        boundary_width: usize,
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
        let boundary = self.capture_boundary(boundary_width, resources, checkpoints)?;
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
        retained_width: usize,
        resources: &mut ResourceContext,
        checkpoints: &SequenceCheckpointCursor<'_>,
    ) -> Result<SequenceControlBoundary> {
        SequenceControlBoundary::try_capture(
            &self.active_counts,
            &self.visible_actors,
            retained_width,
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

pub(super) struct PreparedSequenceRowPlan<'diagram> {
    body: SequencePreparedBody<'diagram>,
    controls: Option<PreparedSequenceControlFrames<'diagram>>,
}

impl PreparedSequenceRowPlan<'_> {
    pub(super) fn output_plan(&self) -> SequenceDocumentPlan<'_> {
        self.controls.as_ref().map_or_else(
            || self.body.output_plan(),
            |controls| controls.output_plan(),
        )
    }

    pub(super) fn materialize(
        self,
        diagram: &AsciiSequenceDiagram,
        layout: &SequenceLayout,
        chars: &SequenceChars,
        resources: &mut ResourceContext,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<SequenceRowDocument> {
        materialize_sequence_row_plan(self, diagram, layout, chars, resources, checkpoints)
    }
}

pub(super) fn prepare_sequence_row_document<'diagram>(
    diagram: &'diagram AsciiSequenceDiagram,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    mirror_actors: bool,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<PreparedSequenceRowPlan<'diagram>> {
    prepare_sequence_row_plan(
        diagram,
        layout,
        chars,
        mirror_actors,
        resources,
        checkpoints,
    )
}

fn prepare_sequence_row_plan<'diagram>(
    diagram: &'diagram AsciiSequenceDiagram,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    mirror_actors: bool,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<PreparedSequenceRowPlan<'diagram>> {
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
                    if let Some(transition) =
                        planner.advance(diagram, event, resources, checkpoints)?
                    {
                        prepared.prepare_step(
                            diagram,
                            SequenceRowStep {
                                event: transition.event,
                                active_counts: planner.active_counts(),
                                visible_actors: planner.visible_actors(),
                                created_actors: &transition.created_actors,
                                destroyed_actors: &transition.destroyed_actors,
                            },
                            layout,
                            chars,
                            resources,
                            checkpoints,
                        )?;
                        planner.record_destroyed_actor_visibility(
                            &transition.destroyed_actors,
                            checkpoints,
                        )?;
                    }
                }
                SequenceVisit::EnterControl { control, depth } => {
                    let boundary_width = retained_lifeline_width(
                        layout,
                        planner.visible_actors(),
                        resources,
                        checkpoints,
                    )?;
                    planner.enter_control(
                        control,
                        depth,
                        prepared.current_row(),
                        boundary_width,
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
                    let boundary_width = if section_index == 0 {
                        0
                    } else {
                        retained_lifeline_width(
                            layout,
                            planner.visible_actors(),
                            resources,
                            checkpoints,
                        )?
                    };
                    planner.enter_section(
                        control,
                        section_index,
                        prepared.current_row(),
                        boundary_width,
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
                    let boundary_width = retained_lifeline_width(
                        layout,
                        planner.visible_actors(),
                        resources,
                        checkpoints,
                    )?;
                    planner.exit_control(
                        prepared.current_row(),
                        boundary_width,
                        resources,
                        checkpoints,
                    )?;
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
    Ok(PreparedSequenceRowPlan {
        body: prepared,
        controls: prepared_controls,
    })
}

fn materialize_sequence_row_plan(
    prepared: PreparedSequenceRowPlan<'_>,
    diagram: &AsciiSequenceDiagram,
    layout: &SequenceLayout,
    chars: &SequenceChars,
    resources: &mut ResourceContext,
    checkpoints: &mut SequenceCheckpointCursor<'_>,
) -> Result<SequenceRowDocument> {
    let mut lines = prepared
        .body
        .materialize(diagram, layout, chars, resources, checkpoints)?;
    if let Some(control) = prepared.controls {
        checkpoints.tick()?;
        lines = control.materialize(lines, layout, chars, resources, checkpoints)?;
    }

    Ok(SequenceRowDocument::new(lines))
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

fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(AsciiResourceLimitPhase::LayoutWork.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operation::AsciiExecution;
    use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
    use crate::resource::{AsciiResourceLimitId, AsciiResourcePolicy};
    use crate::sequence::layout::{calculate_layout, calculate_layout_with_resources};
    use crate::sequence::model::{
        SequenceActorLifecycle, SequenceArrowHead, SequenceCentralDecoration, SequenceControlKind,
        SequenceEvent, SequenceGroupBox, SequenceLineStyle, SequenceMessage,
        SequenceMessageDirection, SequenceParticipant, SequenceParticipantLabel,
    };
    use crate::sequence::notes::apply_note_gutters;
    use crate::sequence::prepared_body::{lifeline_batch_extent, participant_box_batch_extent};
    use crate::sequence::render::render_sequence_diagram_with_execution;
    use crate::sequence::row_document::{
        PreparedSequenceDocument, PreparedSequenceTitle, prepare_sequence_document,
    };
    use crate::sequence::text::{
        SequenceDocumentExtent, SequenceDocumentPlan, SequenceExtentLedger, SequenceRetainedRowRun,
        SequenceRetainedRows,
    };
    use merman_core::{OperationControl, OperationPhase};

    fn build_prepared_row_document<'diagram>(
        diagram: &'diagram AsciiSequenceDiagram,
        title: Option<PreparedSequenceTitle<'diagram>>,
        layout: &SequenceLayout,
        chars: &SequenceChars,
        mirror_actors: bool,
        resources: &mut ResourceContext,
        checkpoints: &mut SequenceCheckpointCursor<'_>,
    ) -> Result<(SequenceRowDocument, PreparedSequenceDocument<'diagram>)> {
        let prepared = prepare_sequence_row_document(
            diagram,
            layout,
            chars,
            mirror_actors,
            resources,
            checkpoints,
        )?;
        let document = prepare_sequence_document(
            diagram,
            title,
            prepared.output_plan(),
            layout,
            resources,
            checkpoints,
        )?;
        let rows = prepared.materialize(diagram, layout, chars, resources, checkpoints)?;
        Ok((rows, document))
    }

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
        let transition = plan
            .advance(&diagram, &message, &mut resources, &mut checkpoints)
            .unwrap()
            .expect("message should produce a row step");

        assert_eq!(transition.created_actors, &[0]);
        assert_eq!(transition.destroyed_actors, &[0]);
        assert_eq!(plan.visible_actors(), &[true]);
        assert_eq!(plan.active_counts(), &[1]);
        plan.record_destroyed_actor_visibility(&transition.destroyed_actors, &mut checkpoints)
            .unwrap();
        assert_eq!(plan.visible_actors(), &[false]);

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
    fn aggregate_box_grid_admission_accepts_exact_and_rejects_n_minus_one() {
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

        build_row_plan_with_grid_limit(&diagram, &options, aggregate_cells)
            .expect("the exact row-plan grid should be admitted");

        let error = build_row_plan_with_grid_limit(&diagram, &options, aggregate_cells - 1)
            .expect_err("the row-plan grid should reject its limit minus one");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGridCells
                    && details.actual == aggregate_cells
                    && details.max == aggregate_cells - 1
        ));
    }

    #[test]
    fn hidden_lifeline_work_admission_counts_materialized_cells_and_participant_scan() {
        const MATERIALIZED_WIDTH: usize = 101;
        const PARTICIPANT_COUNT: usize = 2;
        const EXACT_WORK: usize = MATERIALIZED_WIDTH + PARTICIPANT_COUNT;

        let layout = SequenceLayout {
            participant_widths: vec![3, 3],
            participant_centers: vec![2, 98],
            total_width: MATERIALIZED_WIDTH - 1,
            message_spacing: 1,
            self_message_width: 5,
            width_profile: TerminalWidthProfile::Unicode,
        };
        let visible_actors = [true, false];

        let exact_policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, EXACT_WORK)
            .expect("the exact lifeline work limit should be valid");
        let mut exact_resources = ResourceContext::new(exact_policy);
        let mut exact_checkpoints = layout_checkpoints(&exact_policy);
        let exact_extent = lifeline_batch_extent(
            &layout,
            &visible_actors,
            &exact_resources,
            &mut exact_checkpoints,
        )
        .expect("the hidden-participant lifeline extent should fit");
        assert_eq!(exact_extent.materialized_width(), MATERIALIZED_WIDTH);
        assert_eq!(exact_extent.retained_width(), 3);
        SequenceExtentLedger::default()
            .reserve(exact_extent, &mut exact_resources, &exact_checkpoints)
            .expect("materialized cells plus the participant scan should fit exactly");
        assert_eq!(exact_resources.layout_work_used(), EXACT_WORK);

        let below_policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, EXACT_WORK - 1)
            .expect("the lifeline work limit minus one should be valid");
        let mut below_resources = ResourceContext::new(below_policy);
        let mut below_checkpoints = layout_checkpoints(&below_policy);
        let below_extent = lifeline_batch_extent(
            &layout,
            &visible_actors,
            &below_resources,
            &mut below_checkpoints,
        )
        .expect("extent planning should precede aggregate work admission");
        let error = SequenceExtentLedger::default()
            .reserve(below_extent, &mut below_resources, &below_checkpoints)
            .expect_err("the materialization work limit minus one must reject before painting");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxLayoutWorkUnits
                    && details.actual == EXACT_WORK
                    && details.max == EXACT_WORK - 1
        ));
        assert_eq!(below_resources.layout_work_used(), 0);
    }

    #[test]
    fn final_message_batch_reserves_before_footprint_and_state_materialization() {
        let diagram = diagram(2);
        let options = AsciiRenderOptions::ascii();
        let base_policy = AsciiResourcePolicy::default();
        let mut layout = calculate_layout(&diagram, &options, &base_policy).unwrap();
        layout.message_spacing = 0;
        let event = SequenceEvent::Message(SequenceMessage {
            model_index: 0,
            from: 0,
            to: 1,
            label: "first<br>second".to_string(),
            wrap: false,
            style: SequenceLineStyle::Solid,
            source_marker: SequenceArrowHead::None,
            target_marker: SequenceArrowHead::Filled,
            direction: SequenceMessageDirection::Forward,
            central_decoration: SequenceCentralDecoration::None,
        });
        let active_counts = [0, 0];
        let visible_actors = [true, true];

        let prepares_with_limit = |maximum| {
            let policy = AsciiResourcePolicy::default()
                .with_limit(AsciiResourceLimitId::MaxGridCells, maximum)
                .expect("the message grid limit should be valid");
            let mut resources = ResourceContext::new(policy);
            let mut checkpoints = layout_checkpoints(&policy);
            let mut body = SequencePreparedBody::new(
                &diagram,
                &layout,
                &visible_actors,
                &mut resources,
                &mut checkpoints,
            )?;
            let rows_before = body.current_row();
            body.prepare_step(
                &diagram,
                SequenceRowStep {
                    event: &event,
                    active_counts: &active_counts,
                    visible_actors: &visible_actors,
                    created_actors: &[],
                    destroyed_actors: &[],
                },
                &layout,
                &ascii_chars(),
                &mut resources,
                &mut checkpoints,
            )?;
            Ok::<_, AsciiError>((rows_before, body.current_row()))
        };

        let mut low = 1usize;
        let mut high = base_policy
            .value(AsciiResourceLimitId::MaxGridCells)
            .expect("the default policy should bound the grid");
        while low < high {
            let mid = low + (high - low) / 2;
            match prepares_with_limit(mid) {
                Ok(_) => high = mid,
                Err(AsciiError::ResourceLimitExceeded(details))
                    if details.limit == AsciiResourceLimitId::MaxGridCells =>
                {
                    low = mid + 1;
                }
                Err(error) => panic!("unexpected message batch admission error: {error}"),
            }
        }
        let exact = low;

        let (rows_before, exact_rows) = prepares_with_limit(exact)
            .expect("the exact aggregate message batch should be admitted");
        assert_eq!(exact_rows - rows_before, 2);

        let exact_policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, exact)
            .expect("the exact message grid limit should be valid");
        let mut exact_resources = ResourceContext::new(exact_policy);
        let mut exact_checkpoints = layout_checkpoints(&exact_policy);
        let mut exact_body = SequencePreparedBody::new(
            &diagram,
            &layout,
            &visible_actors,
            &mut exact_resources,
            &mut exact_checkpoints,
        )
        .expect("the participant header should fit the exact limit");
        exact_body
            .prepare_step(
                &diagram,
                SequenceRowStep {
                    event: &event,
                    active_counts: &active_counts,
                    visible_actors: &visible_actors,
                    created_actors: &[],
                    destroyed_actors: &[],
                },
                &layout,
                &ascii_chars(),
                &mut exact_resources,
                &mut exact_checkpoints,
            )
            .expect("the exact message batch should prepare");
        let exact_lines = exact_body
            .materialize(
                &diagram,
                &layout,
                &ascii_chars(),
                &mut exact_resources,
                &mut exact_checkpoints,
            )
            .expect("the admitted message batch should materialize");
        assert!(exact_lines.iter().any(|line| line.text().contains("first")));
        assert!(
            exact_lines
                .iter()
                .any(|line| line.text().contains("second"))
        );

        let below = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, exact - 1)
            .expect("the message N-1 grid limit should be valid");
        let mut resources = ResourceContext::new(below);
        let mut checkpoints = layout_checkpoints(&below);
        let mut body = SequencePreparedBody::new(
            &diagram,
            &layout,
            &visible_actors,
            &mut resources,
            &mut checkpoints,
        )
        .expect("the participant header should fit below the final aggregate limit");
        let rows_before = body.current_row();
        let work_before = resources.layout_work_used();
        let document_before = resources.document_cells_used();

        let error = body
            .prepare_step(
                &diagram,
                SequenceRowStep {
                    event: &event,
                    active_counts: &active_counts,
                    visible_actors: &visible_actors,
                    created_actors: &[],
                    destroyed_actors: &[],
                },
                &layout,
                &ascii_chars(),
                &mut resources,
                &mut checkpoints,
            )
            .expect_err("the final message batch should reject its aggregate grid N-1");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGridCells
                    && details.actual == exact
                    && details.max == exact - 1
        ));
        assert_eq!(body.current_row(), rows_before);
        assert_eq!(resources.layout_work_used(), work_before);
        assert_eq!(resources.document_cells_used(), document_before);
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
            build_row_plan_with_limit(&diagram, &options, limit, exact)
                .expect("the exact aggregate row-plan limit should be admitted");

            let error = build_row_plan_with_limit(&diagram, &options, limit, exact - 1)
                .expect_err("the aggregate row-plan limit minus one should be rejected");
            assert!(matches!(
                error,
                AsciiError::ResourceLimitExceeded(details)
                    if details.limit == limit
                        && details.actual == exact
                        && details.max == exact - 1
            ));
        }
    }

    #[test]
    fn final_document_cells_are_admitted_before_row_materialization() {
        let options = AsciiRenderOptions::ascii();
        let mut boxed = diagram(1);
        boxed.boxes.push(SequenceGroupBox {
            actor_indices: vec![0],
            label: Some("group".to_string()),
            background: None,
            wrap: false,
        });
        let control_box = local_control_box_diagram();
        let cases = [
            ("body", diagram(1), "Timeline"),
            ("control", nested_control_diagram(), "Timeline"),
            ("box", boxed, "Timeline"),
            ("control+box", control_box, "Timeline"),
            ("trailing-space title", diagram(1), "Timeline   "),
        ];

        for (name, diagram, title) in cases {
            let (content, output) = prepare_document_extents_without_materialization(
                &diagram,
                title,
                &options,
                AsciiResourcePolicy::default(),
            )
            .unwrap_or_else(|error| panic!("{name} plan should fit the default policy: {error}"));
            let exact = output.document_cells();
            let title_cells = exact
                .checked_sub(content.document_cells())
                .expect("the title must add retained document cells");
            assert!(title_cells > 0, "{name} title should retain visible cells");
            assert!(
                content.document_cells() < exact,
                "{name} body/control/box should fit the combined limit minus one"
            );
            assert!(
                title_cells < exact,
                "{name} title should fit the combined limit minus one"
            );

            let exact_policy = AsciiResourcePolicy::default()
                .with_limit(AsciiResourceLimitId::MaxDocumentCells, exact)
                .expect("the exact document-cell limit should be valid");
            let admitted = prepare_document_extents_without_materialization(
                &diagram,
                title,
                &options,
                exact_policy,
            )
            .unwrap_or_else(|error| panic!("{name} exact document plan should pass: {error}"));
            assert_eq!(admitted.1, output);

            let below_policy = AsciiResourcePolicy::default()
                .with_limit(AsciiResourceLimitId::MaxDocumentCells, exact - 1)
                .expect("the limit below the final document should be valid");
            let error = prepare_document_extents_without_materialization(
                &diagram,
                title,
                &options,
                below_policy,
            )
            .expect_err("the combined document must reject before row materialization is called");
            assert!(matches!(
                error,
                AsciiError::ResourceLimitExceeded(details)
                    if details.limit == AsciiResourceLimitId::MaxDocumentCells
                        && details.actual == exact
                        && details.max == exact - 1
            ));
        }
    }

    #[test]
    fn title_and_control_box_plans_match_public_materialization() {
        let options = AsciiRenderOptions::ascii();
        let cases = [
            ("trailing-space title", diagram(1), "Timeline   "),
            ("blank title", diagram(1), "   "),
            ("control+box", local_control_box_diagram(), "Timeline"),
        ];

        for (name, diagram, title) in cases {
            let (_, planned) = prepare_document_extents_without_materialization(
                &diagram,
                title,
                &options,
                AsciiResourcePolicy::default(),
            )
            .unwrap_or_else(|error| panic!("{name} plan should fit: {error}"));
            let exact_policy = AsciiResourcePolicy::default()
                .with_limit(
                    AsciiResourceLimitId::MaxDocumentCells,
                    planned.document_cells(),
                )
                .expect("the exact document-cell limit should be valid");
            let mut resources = ResourceContext::new(exact_policy);
            let rendered = render_sequence_diagram_with_execution(
                &diagram,
                Some(title),
                &options,
                &mut resources,
                AsciiExecution::for_test(&exact_policy),
            )
            .unwrap_or_else(|error| {
                panic!("{name} should materialize at the exact limit: {error}")
            });
            let actual_cells = rendered.lines().map(str::len).sum::<usize>();
            assert_eq!(actual_cells, planned.document_cells(), "{name}");

            let first_line = rendered.lines().next().unwrap_or_default();
            if title.trim().is_empty() {
                assert!(first_line.is_empty(), "{name}");
            } else {
                assert_eq!(first_line.trim_end(), first_line, "{name}");
            }
        }
    }

    fn build_row_plan_with_grid_limit(
        diagram: &AsciiSequenceDiagram,
        options: &AsciiRenderOptions,
        maximum: usize,
    ) -> Result<()> {
        let limited = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, maximum)
            .expect("the test grid limit should be valid");
        let mut resources = ResourceContext::new(limited);
        let mut checkpoints = layout_checkpoints(&limited);
        let layout =
            calculate_layout_with_resources(diagram, options, &mut resources, &mut checkpoints)?;
        let chars = ascii_chars();
        prepare_sequence_row_document(
            diagram,
            &layout,
            &chars,
            false,
            &mut resources,
            &mut checkpoints,
        )?;
        Ok(())
    }

    fn prepare_document_extents_without_materialization(
        diagram: &AsciiSequenceDiagram,
        title: &str,
        options: &AsciiRenderOptions,
        policy: AsciiResourcePolicy,
    ) -> Result<(SequenceDocumentExtent, SequenceDocumentExtent)> {
        let resources = ResourceContext::new(policy);
        let execution = AsciiExecution::for_test(&policy);
        let mut layout_resources = execution.resource_context(&resources, OperationPhase::Layout);
        let mut checkpoints = SequenceCheckpointCursor::new(execution, OperationPhase::Layout);
        let title = crate::sequence::row_document::prepare_sequence_title(
            Some(title),
            options.terminal_width_profile,
            &mut layout_resources,
            &mut checkpoints,
        )?;
        let mut layout = calculate_layout_with_resources(
            diagram,
            options,
            &mut layout_resources,
            &mut checkpoints,
        )?;
        apply_note_gutters(
            diagram,
            &mut layout,
            &mut layout_resources,
            &mut checkpoints,
        )?;
        let chars = ascii_chars();
        let rows = prepare_sequence_row_document(
            diagram,
            &layout,
            &chars,
            options.sequence_mirror_actors,
            &mut layout_resources,
            &mut checkpoints,
        )?;
        let document = prepare_sequence_document(
            diagram,
            title,
            rows.output_plan(),
            &layout,
            &mut layout_resources,
            &mut checkpoints,
        )?;
        Ok((document.content_extent(), document.output_extent()))
    }

    fn build_row_plan_with_limit(
        diagram: &AsciiSequenceDiagram,
        options: &AsciiRenderOptions,
        limit: AsciiResourceLimitId,
        maximum: usize,
    ) -> Result<()> {
        let limited = AsciiResourcePolicy::default()
            .with_limit(limit, maximum)
            .expect("the test resource limit should be valid");
        let mut resources = ResourceContext::new(limited);
        let mut checkpoints = layout_checkpoints(&limited);
        let layout =
            calculate_layout_with_resources(diagram, options, &mut resources, &mut checkpoints)?;
        prepare_sequence_row_document(
            diagram,
            &layout,
            &ascii_chars(),
            false,
            &mut resources,
            &mut checkpoints,
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
            match build_row_plan_with_limit(diagram, options, limit, mid) {
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
        let execution = AsciiExecution::for_test(&policy);
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

    fn local_control_box_diagram() -> AsciiSequenceDiagram {
        let mut diagram = diagram(2);
        diagram.lifecycles[1].created_at = Some(3);
        diagram.boxes.push(SequenceGroupBox {
            actor_indices: vec![0],
            label: Some("local".to_string()),
            background: None,
            wrap: false,
        });
        let resources = test_resources();
        let policy = resources.policy();
        let execution = AsciiExecution::for_test(&policy);
        let mut body =
            crate::sequence::tree::SequenceTreeBuilder::new(3, &resources, execution).unwrap();
        body.start_control(
            0,
            SequenceControlKind::Loop,
            "local".to_string(),
            None,
            &resources,
            execution,
        )
        .unwrap();
        body.push_event(
            SequenceEvent::Message(SequenceMessage {
                model_index: 1,
                from: 0,
                to: 0,
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
        body.end_control(2, SequenceControlKind::Loop, &resources, execution)
            .unwrap();
        body.push_event(
            SequenceEvent::Message(SequenceMessage {
                model_index: 3,
                from: 0,
                to: 1,
                label: "create".to_string(),
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
        let chars = ascii_chars();
        let (plan, document) = build_prepared_row_document(
            &diagram,
            None,
            &layout,
            &chars,
            options.sequence_mirror_actors,
            &mut resources,
            &mut layout_cursor,
        )
        .unwrap();
        let rendered = plan
            .render(
                document,
                &layout,
                &chars,
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
        let diagram = diagram(1);
        let options = AsciiRenderOptions::ascii();
        let policy = AsciiResourcePolicy::default();
        let layout = calculate_layout(&diagram, &options, &policy).unwrap();
        let mut resources = ResourceContext::new(policy);
        let mut layout_cursor = layout_checkpoints(&policy);
        let title = crate::sequence::row_document::prepare_sequence_title(
            Some("Timeline"),
            options.terminal_width_profile,
            &mut resources,
            &mut layout_cursor,
        )
        .unwrap();
        let chars = ascii_chars();
        let (plan, document) = build_prepared_row_document(
            &diagram,
            title,
            &layout,
            &chars,
            false,
            &mut resources,
            &mut layout_cursor,
        )
        .unwrap();

        let rendered = plan
            .render(
                document,
                &layout,
                &chars,
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
        let chars = ascii_chars();
        let (plan, document) = build_prepared_row_document(
            &diagram,
            None,
            &layout,
            &chars,
            false,
            &mut resources,
            &mut layout_cursor,
        )
        .unwrap();
        let before_finalization = resources.layout_work_used();

        plan.render(
            document,
            &layout,
            &chars,
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
        let exact_chars = ascii_chars();
        let (exact_plan, exact_document) = build_prepared_row_document(
            &diagram,
            None,
            &exact_layout,
            &exact_chars,
            false,
            &mut exact_resources,
            &mut exact_layout_checkpoints,
        )
        .unwrap();
        exact_plan
            .render(
                exact_document,
                &exact_layout,
                &exact_chars,
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
        let below_chars = ascii_chars();
        let (below_plan, below_document) = build_prepared_row_document(
            &diagram,
            None,
            &below_layout,
            &below_chars,
            false,
            &mut below_resources,
            &mut below_layout_checkpoints,
        )
        .unwrap();
        let error = below_plan
            .render(
                below_document,
                &below_layout,
                &below_chars,
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
        let control = OperationControl::new();
        control.cancel_after_checkpoints(0);
        let execution = AsciiExecution::new(&control, &policy);
        let mut layout_resources =
            execution.resource_context(&base_resources, OperationPhase::Layout);
        let mut checkpoints = SequenceCheckpointCursor::new(execution, OperationPhase::Layout);

        let retained_rows = [SequenceRetainedRowRun::new(layout.total_width + 1, 1)];
        let error = prepare_sequence_document(
            &diagram,
            None,
            SequenceDocumentPlan::new(
                SequenceDocumentExtent::new(layout.total_width + 1, 1, layout.total_width + 1),
                SequenceRetainedRows::Runs(&retained_rows),
            ),
            &layout,
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

    fn diagram(participant_count: usize) -> AsciiSequenceDiagram {
        AsciiSequenceDiagram {
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
        SequenceCheckpointCursor::new(AsciiExecution::for_test(policy), OperationPhase::Layout)
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
