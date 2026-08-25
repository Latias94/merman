use crate::error::{AsciiError, Result};
use crate::graph::style::prepare_state_style;
use crate::graph::{
    AsciiGraph, DeferredGraphLabelSectionPlan, DeferredGraphNodeLabelPlan, GraphDirection,
    GraphEdgeAttrs, GraphEdgeMarker, GraphGroupKind, GraphGroupStyle, GraphNodeSemantics,
    GraphNodeShape, GraphNodeSide, GraphNodeSideConstraint, GraphNodeStyle,
};
use crate::operation::AsciiExecution;
use crate::options::TerminalWidthProfile;
#[cfg(test)]
use crate::resource::AsciiResourcePolicy;
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use crate::safe_text::{
    NormalizedTrimmedTextPlan, charge_text_layout, try_plan_normalized_trimmed_text,
};
use merman_core::diagrams::state::{
    StateDiagramRenderEdge, StateDiagramRenderModel, StateDiagramRenderNode,
};
use std::collections::HashMap;

const STATE_DIAGRAM_TYPE: &str = "state";
const STATE_NODE_PROJECTION_WORK_UNITS: usize = 10;
const STATE_EDGE_PROJECTION_WORK_UNITS: usize = 2;
const STATE_TEXT_COPY_CHUNK_BYTES: usize = 8 * 1024;

#[derive(Debug)]
struct StateParentProjection {
    parent_by_index: Vec<Option<usize>>,
    depth_by_index: Vec<usize>,
    parent_first_indices: Vec<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StateParentVisit {
    Pending,
    Active,
    Complete,
}

struct StateDirectionProjection {
    node_by_index: Vec<GraphDirection>,
    group_by_index: Vec<Option<GraphDirection>>,
}

struct StateStylePlan {
    node_by_index: Vec<GraphNodeStyle>,
    group_by_index: Vec<GraphGroupStyle>,
}

impl StateStylePlan {
    fn try_new(
        model: &StateDiagramRenderModel,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<Self> {
        let mut node_by_index = Vec::new();
        node_by_index
            .try_reserve_exact(model.nodes.len())
            .map_err(|_| projection_allocation_failed())?;
        let mut group_by_index = Vec::new();
        group_by_index
            .try_reserve_exact(model.nodes.len())
            .map_err(|_| projection_allocation_failed())?;

        for (index, node) in model.nodes.iter().enumerate() {
            checkpoint_projection(execution, index)?;
            let prepared = prepare_state_style(
                node.css_compiled_styles
                    .iter()
                    .chain(&node.css_styles)
                    .map(String::as_str)
                    .chain(std::iter::once(node.label_style.as_str())),
                resources,
                execution,
            )?;
            let mut node_style = GraphNodeStyle::default();
            prepared.apply_node(&mut node_style);
            node_by_index.push(node_style);
            let mut group_style = GraphGroupStyle::default();
            prepared.apply_group(&mut group_style);
            group_by_index.push(group_style);
        }

        Ok(Self {
            node_by_index,
            group_by_index,
        })
    }
}

struct StateGroupOrderEntry<'a> {
    node_index: usize,
    depth: usize,
    node: &'a StateDiagramRenderNode,
}

type StateGroupMembers<'a> = HashMap<&'a str, Vec<&'a str>>;
type StateNoteParentIndex<'a> = HashMap<&'a str, &'a str>;
type StateNoteSideConstraints<'a> = HashMap<&'a str, StateNoteSideConstraint<'a>>;

#[derive(Clone, Copy)]
struct StateNoteSideConstraint<'a> {
    anchor_id: &'a str,
    side: GraphNodeSide,
}

struct StateProjectionTextPlan<'a> {
    nodes: Vec<Option<DeferredGraphNodeLabelPlan<'a>>>,
    group_titles: HashMap<&'a str, DeferredGraphLabelSectionPlan<'a>>,
    edge_labels: Vec<Option<NormalizedTrimmedTextPlan<'a>>>,
}

#[cfg(test)]
pub(crate) fn from_state_model_with_resources(
    model: &StateDiagramRenderModel,
    policy: AsciiResourcePolicy,
) -> Result<AsciiGraph> {
    let mut resources = ResourceContext::new(policy);
    from_state_model_with_context_and_execution(
        model,
        TerminalWidthProfile::Unicode,
        &mut resources,
        AsciiExecution::for_test(&policy),
    )
}

pub(crate) fn from_state_model_with_context_and_execution(
    model: &StateDiagramRenderModel,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<AsciiGraph> {
    execution.rebind_resource_context(resources, merman_core::OperationPhase::Semantic);
    resources.transaction(|resources| {
        from_state_model_transactional(model, width_profile, resources, execution)
    })
}

fn from_state_model_transactional(
    model: &StateDiagramRenderModel,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<AsciiGraph> {
    preflight_state_projection_text(model, resources, execution)?;
    let projection_work = state_projection_work(model, resources, execution)?;
    resources.charge_layout_work(projection_work)?;
    let style_plan = StateStylePlan::try_new(model, resources, execution)?;
    let parent_projection = validate_supported_state_model(model, resources, execution)?;

    let direction = parse_state_direction(&model.direction)?;
    let group_members = group_members_by_id(model, execution)?;
    let note_node_parent_by_id = note_node_parent_by_id(model, execution)?;
    let note_side_constraints = note_side_constraints(model, &note_node_parent_by_id, execution)?;
    let StateProjectionTextPlan {
        nodes: node_text_plans,
        mut group_titles,
        edge_labels,
    } = plan_state_texts(
        model,
        &group_members,
        &note_node_parent_by_id,
        &note_side_constraints,
        width_profile,
        resources,
        execution,
    )?;
    let state_directions =
        state_direction_projection(model, direction, &parent_projection, resources, execution)?;
    let mut graph = AsciiGraph::new_for_diagram(STATE_DIAGRAM_TYPE, direction);
    graph.try_reserve_projection(model.nodes.len(), model.edges.len(), model.nodes.len())?;
    graph.use_incoming_edge_roots();

    for (index, (node, text_plan)) in model.nodes.iter().zip(node_text_plans).enumerate() {
        checkpoint_projection(execution, index)?;
        if is_group_container(node, &group_members) {
            continue;
        }
        if is_state_note_node(node) {
            continue;
        }
        let text = text_plan
            .ok_or_else(|| unsupported("missing state node label plan"))?
            .materialize_after_admission(resources)?;
        let side_constraint = materialize_note_side_constraint(
            note_side_constraints.get(node.id.as_str()),
            resources,
        )?;
        graph.add_node_with_prepared_text(
            materialize_state_text(&node.id, resources)?,
            text,
            state_node_shape(
                node,
                state_directions
                    .node_by_index
                    .get(index)
                    .copied()
                    .unwrap_or_else(|| direction.canonical()),
            )?,
            style_plan
                .node_by_index
                .get(index)
                .copied()
                .ok_or_else(|| unsupported("missing state node style plan"))?,
            GraphNodeSemantics { side_constraint },
        );
    }

    for (index, group) in sorted_group_nodes(
        model,
        &group_members,
        &parent_projection,
        resources,
        execution,
    )?
    .into_iter()
    .enumerate()
    {
        checkpoint_projection(execution, index)?;
        let StateGroupOrderEntry {
            node_index, node, ..
        } = group;
        let members = materialize_group_members(
            group_members.get(node.id.as_str()).map(Vec::as_slice),
            resources,
            execution,
        )?;
        let title = if state_group_title_is_empty(node) {
            String::new()
        } else {
            group_titles
                .remove(node.id.as_str())
                .ok_or_else(|| unsupported("missing state group title plan"))?
                .materialize_normalized_after_admission(resources)?
        };
        graph.add_group_with_kind_and_style(
            materialize_state_text(&node.id, resources)?,
            title,
            state_directions
                .group_by_index
                .get(node_index)
                .copied()
                .flatten(),
            members,
            state_group_kind(node),
            style_plan
                .group_by_index
                .get(node_index)
                .copied()
                .ok_or_else(|| unsupported("missing state group style plan"))?,
        );
    }

    for (index, (edge, label_plan)) in model.edges.iter().zip(edge_labels).enumerate() {
        checkpoint_projection(execution, index)?;
        let mut from = remap_note_endpoint(&edge.start, &note_node_parent_by_id);
        let mut to = remap_note_endpoint(&edge.end, &note_node_parent_by_id);
        if is_note_edge(edge) {
            (from, to) =
                canonical_note_edge_endpoints(from, to, &note_side_constraints, direction)?;
        }
        graph.add_edge_with_attrs(
            materialize_state_text(from, resources)?,
            materialize_state_text(to, resources)?,
            GraphEdgeAttrs {
                label: label_plan
                    .map(|plan| {
                        plan.materialize_after_admission_with_checkpoint(|iteration| {
                            checkpoint_projection(execution, iteration)
                        })
                    })
                    .transpose()?,
                end_marker: edge_marker(edge),
                ..GraphEdgeAttrs::default()
            },
        );
    }

    Ok(graph)
}

fn checkpoint_projection(execution: AsciiExecution<'_>, iteration: usize) -> Result<()> {
    execution.checkpoint_loop(merman_core::OperationPhase::Semantic, iteration)
}

fn preflight_state_projection_text(
    model: &StateDiagramRenderModel,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    charge_text_layout(resources, &model.direction)?;
    for (index, node) in model.nodes.iter().enumerate() {
        checkpoint_projection(execution, index)?;
        charge_text_layout(resources, &node.id)?;
        if let Some(parent_id) = node.parent_id.as_deref() {
            charge_text_layout(resources, parent_id)?;
        }
        if let Some(position) = node.position.as_deref() {
            charge_text_layout(resources, position)?;
        }
    }
    for (index, edge) in model.edges.iter().enumerate() {
        checkpoint_projection(execution, index)?;
        charge_text_layout(resources, &edge.start)?;
        charge_text_layout(resources, &edge.end)?;
    }
    Ok(())
}

fn plan_state_texts<'a>(
    model: &'a StateDiagramRenderModel,
    group_members: &StateGroupMembers<'a>,
    note_node_parent_by_id: &StateNoteParentIndex<'a>,
    note_side_constraints: &StateNoteSideConstraints<'a>,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<StateProjectionTextPlan<'a>> {
    let mut nodes = Vec::new();
    nodes
        .try_reserve_exact(model.nodes.len())
        .map_err(|_| projection_allocation_failed())?;
    let mut group_titles = HashMap::new();
    group_titles
        .try_reserve(model.nodes.len())
        .map_err(|_| projection_allocation_failed())?;
    let mut work_units = 0usize;
    let mut retained_document_cells = 0usize;
    let mut planned_document_cells = 0usize;
    let mut output_bytes = 0usize;

    for (index, node) in model.nodes.iter().enumerate() {
        checkpoint_projection(execution, index)?;
        if is_group_container(node, group_members) {
            nodes.push(None);
            work_units = include_state_copy_work(work_units, &node.id, resources)?;
            for (member_index, member) in group_members
                .get(node.id.as_str())
                .into_iter()
                .flat_map(|members| members.iter().copied())
                .enumerate()
            {
                checkpoint_projection(execution, member_index)?;
                work_units = include_state_copy_work(work_units, member, resources)?;
            }
            if state_group_title_is_empty(node) {
                continue;
            }
            let title = DeferredGraphLabelSectionPlan::try_joined(
                state_label_fragments(node).chain(state_description_fragments(node)),
                Some(node.id.as_str()),
                STATE_DIAGRAM_TYPE,
                width_profile,
                resources,
            )?
            .ok_or_else(|| unsupported("state group without title fallback"))?;
            work_units = resources.checked_work_add(
                work_units,
                title.normalized_materialization_work_units(resources)?,
            )?;
            retained_document_cells = checked_state_projection_metric_add(
                resources,
                AsciiResourceLimitId::MaxDocumentCells,
                retained_document_cells,
                title.document_cells(),
            )?;
            output_bytes = checked_state_projection_metric_add(
                resources,
                AsciiResourceLimitId::MaxOutputBytes,
                output_bytes,
                title.normalized_joined_bytes(resources)?,
            )?;
            group_titles.insert(node.id.as_str(), title);
            continue;
        }
        if is_state_note_node(node) {
            nodes.push(None);
            continue;
        }
        work_units = include_state_copy_work(work_units, &node.id, resources)?;
        if let Some(constraint) = note_side_constraints.get(node.id.as_str()) {
            work_units = include_state_copy_work(work_units, constraint.anchor_id, resources)?;
        }
        let plan = state_node_text_plan(node, width_profile, resources)?;
        work_units =
            resources.checked_work_add(work_units, plan.source_materialization_work_units())?;
        planned_document_cells = checked_state_projection_metric_add(
            resources,
            AsciiResourceLimitId::MaxDocumentCells,
            planned_document_cells,
            plan.document_cells(),
        )?;
        output_bytes = checked_state_projection_metric_add(
            resources,
            AsciiResourceLimitId::MaxOutputBytes,
            output_bytes,
            plan.materialized_bytes(),
        )?;
        nodes.push(Some(plan));
    }

    let mut edge_labels = Vec::new();
    edge_labels
        .try_reserve_exact(model.edges.len())
        .map_err(|_| projection_allocation_failed())?;
    for (index, edge) in model.edges.iter().enumerate() {
        checkpoint_projection(execution, index)?;
        let from = remap_note_endpoint(&edge.start, note_node_parent_by_id);
        let to = remap_note_endpoint(&edge.end, note_node_parent_by_id);
        work_units = include_state_copy_work(work_units, from, resources)?;
        work_units = include_state_copy_work(work_units, to, resources)?;
        let label = try_plan_normalized_trimmed_text(&edge.label, width_profile, resources)?;
        if let Some(label) = label {
            work_units =
                resources.checked_work_add(work_units, label.materialization_work_units())?;
            let metrics = label.metrics();
            retained_document_cells = checked_state_projection_metric_add(
                resources,
                AsciiResourceLimitId::MaxDocumentCells,
                retained_document_cells,
                metrics.document_cells,
            )?;
            output_bytes = checked_state_projection_metric_add(
                resources,
                AsciiResourceLimitId::MaxOutputBytes,
                output_bytes,
                metrics.materialized_bytes,
            )?;
        }
        edge_labels.push(label);
    }

    let admitted_document_cells = checked_state_projection_metric_add(
        resources,
        AsciiResourceLimitId::MaxDocumentCells,
        retained_document_cells,
        planned_document_cells,
    )?;
    resources.check_usage(work_units, admitted_document_cells)?;
    resources.check(AsciiResourceLimitId::MaxOutputBytes, output_bytes)?;
    resources.charge_usage(work_units, retained_document_cells)?;
    Ok(StateProjectionTextPlan {
        nodes,
        group_titles,
        edge_labels,
    })
}

fn include_state_copy_work(
    work_units: usize,
    value: &str,
    resources: &ResourceContext,
) -> Result<usize> {
    resources.checked_work_add(work_units, value.len())
}

fn state_node_text_plan<'a>(
    node: &'a StateDiagramRenderNode,
    width_profile: TerminalWidthProfile,
    resources: &ResourceContext,
) -> Result<DeferredGraphNodeLabelPlan<'a>> {
    if is_state_pseudo_shape(node.shape.as_str()) {
        return DeferredGraphNodeLabelPlan::single(
            DeferredGraphLabelSectionPlan::try_single(
                "",
                None,
                STATE_DIAGRAM_TYPE,
                width_profile,
                resources,
            )?,
            None,
            STATE_DIAGRAM_TYPE,
            width_profile,
            resources,
        );
    }

    if node.shape == "rectWithTitle" {
        let title = DeferredGraphLabelSectionPlan::try_joined(
            state_label_fragments(node),
            Some(node.id.as_str()),
            STATE_DIAGRAM_TYPE,
            width_profile,
            resources,
        )?
        .ok_or_else(|| unsupported("state title/body compartments without title"))?;
        let body = DeferredGraphLabelSectionPlan::try_joined(
            state_description_fragments(node),
            None,
            STATE_DIAGRAM_TYPE,
            width_profile,
            resources,
        )?
        .ok_or_else(|| unsupported("state title/body compartments without body"))?;
        return DeferredGraphNodeLabelPlan::compartmented(
            title,
            body,
            STATE_DIAGRAM_TYPE,
            width_profile,
            resources,
        );
    }

    let label = DeferredGraphLabelSectionPlan::try_joined(
        state_label_fragments(node).chain(state_description_fragments(node)),
        Some(node.id.as_str()),
        STATE_DIAGRAM_TYPE,
        width_profile,
        resources,
    )?
    .ok_or_else(|| unsupported("state node without label fallback"))?;
    DeferredGraphNodeLabelPlan::single(label, None, STATE_DIAGRAM_TYPE, width_profile, resources)
}

fn state_label_fragments(node: &StateDiagramRenderNode) -> impl Iterator<Item = &str> {
    node.label.iter().flat_map(|label| {
        label.as_str().into_iter().chain(
            label
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|item| item.as_str()),
        )
    })
}

fn state_description_fragments(node: &StateDiagramRenderNode) -> impl Iterator<Item = &str> {
    node.description.iter().flatten().map(String::as_str)
}

fn checked_state_projection_metric_add(
    resources: &ResourceContext,
    limit: AsciiResourceLimitId,
    left: usize,
    right: usize,
) -> Result<usize> {
    left.checked_add(right)
        .ok_or_else(|| resources.overflow(limit))
}

fn state_projection_work(
    model: &StateDiagramRenderModel,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<usize> {
    let mut authored_items = 0usize;
    for (index, node) in model.nodes.iter().enumerate() {
        checkpoint_projection(execution, index)?;
        let label_items = node
            .label
            .as_ref()
            .map(|label| label.as_array().map_or(1, |items| items.len()))
            .unwrap_or_default();
        let node_items = label_items
            .checked_add(node.description.as_ref().map_or(0, |items| items.len()))
            .and_then(|items| items.checked_add(usize::from(node.position.is_some())))
            .ok_or_else(|| {
                resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
            })?;
        authored_items = authored_items.checked_add(node_items).ok_or_else(|| {
            resources
                .policy()
                .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
        })?;
    }
    let node_containers = model
        .nodes
        .len()
        .checked_mul(STATE_NODE_PROJECTION_WORK_UNITS)
        .ok_or_else(|| {
            resources
                .policy()
                .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
        })?;
    let edge_containers = model
        .edges
        .len()
        .checked_mul(STATE_EDGE_PROJECTION_WORK_UNITS)
        .ok_or_else(|| {
            resources
                .policy()
                .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
        })?;
    node_containers
        .checked_add(edge_containers)
        .and_then(|work| work.checked_add(authored_items))
        .ok_or_else(|| {
            resources
                .policy()
                .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
        })
}

fn projection_allocation_failed() -> AsciiError {
    AsciiError::AllocationFailed {
        phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
    }
}

fn validate_supported_state_model(
    model: &StateDiagramRenderModel,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<StateParentProjection> {
    let mut node_index_by_id = HashMap::new();
    node_index_by_id
        .try_reserve(model.nodes.len())
        .map_err(|_| projection_allocation_failed())?;
    for (index, node) in model.nodes.iter().enumerate() {
        checkpoint_projection(execution, index)?;
        if node_index_by_id.insert(node.id.as_str(), index).is_some() {
            return Err(unsupported("duplicate node ids"));
        }
        validate_supported_state_node(node, execution)?;
    }
    let parent_projection =
        StateParentProjection::try_new(model, &node_index_by_id, resources, execution)?;

    for (index, edge) in model.edges.iter().enumerate() {
        checkpoint_projection(execution, index)?;
        if !node_index_by_id.contains_key(edge.start.as_str())
            || !node_index_by_id.contains_key(edge.end.as_str())
        {
            return Err(unsupported("edges with missing endpoint nodes"));
        }
        if !edge.arrow_type_end.is_empty()
            && !matches!(
                edge.arrow_type_end.as_str(),
                "arrow_barb" | "arrow_barb_neo"
            )
        {
            return Err(unsupported("state arrow types"));
        }
    }

    Ok(parent_projection)
}

impl StateParentProjection {
    fn try_new(
        model: &StateDiagramRenderModel,
        node_index_by_id: &HashMap<&str, usize>,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<Self> {
        let mut parent_by_index = Vec::new();
        parent_by_index
            .try_reserve_exact(model.nodes.len())
            .map_err(|_| projection_allocation_failed())?;
        for (node_index, node) in model.nodes.iter().enumerate() {
            checkpoint_projection(execution, node_index)?;
            let parent_index = node
                .parent_id
                .as_deref()
                .map(|parent_id| {
                    node_index_by_id
                        .get(parent_id)
                        .copied()
                        .ok_or_else(|| unsupported("unknown state parent ids"))
                })
                .transpose()?;
            if let Some(parent_index) = parent_index {
                let parent = model
                    .nodes
                    .get(parent_index)
                    .ok_or_else(|| unsupported("state parent index"))?;
                if !parent.is_group {
                    return Err(unsupported("state parents that are not groups"));
                }
            }
            parent_by_index.push(parent_index);
        }

        let (depth_by_index, parent_first_indices) =
            cache_state_parent_depths(&parent_by_index, resources, execution)?;
        Ok(Self {
            parent_by_index,
            depth_by_index,
            parent_first_indices,
        })
    }
}

fn cache_state_parent_depths(
    parent_by_index: &[Option<usize>],
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<(Vec<usize>, Vec<usize>)> {
    let mut visit_by_index = Vec::new();
    visit_by_index
        .try_reserve_exact(parent_by_index.len())
        .map_err(|_| projection_allocation_failed())?;
    visit_by_index.resize(parent_by_index.len(), StateParentVisit::Pending);

    let mut depth_by_index = Vec::new();
    depth_by_index
        .try_reserve_exact(parent_by_index.len())
        .map_err(|_| projection_allocation_failed())?;
    depth_by_index.resize(parent_by_index.len(), 0usize);

    let mut parent_first_indices = Vec::new();
    parent_first_indices
        .try_reserve_exact(parent_by_index.len())
        .map_err(|_| projection_allocation_failed())?;
    let mut path = Vec::new();
    path.try_reserve_exact(parent_by_index.len())
        .map_err(|_| projection_allocation_failed())?;

    // Reuse one path while each node advances Pending -> Active -> Complete exactly once.
    for start_index in 0..parent_by_index.len() {
        checkpoint_projection(execution, start_index)?;
        if visit_by_index.get(start_index) == Some(&StateParentVisit::Complete) {
            continue;
        }

        path.clear();
        let mut current_index = Some(start_index);
        while let Some(node_index) = current_index {
            let visit = visit_by_index
                .get(node_index)
                .copied()
                .ok_or_else(|| unsupported("state parent index"))?;
            match visit {
                StateParentVisit::Complete => break,
                StateParentVisit::Active => {
                    return Err(unsupported("cyclic state parent ids"));
                }
                StateParentVisit::Pending => {
                    let slot = visit_by_index
                        .get_mut(node_index)
                        .ok_or_else(|| unsupported("state parent index"))?;
                    *slot = StateParentVisit::Active;
                    path.push(node_index);
                    // Stop an over-depth chain before walking the remainder of its parents.
                    resources.check_nesting_depth(path.len())?;

                    let parent_index = parent_by_index
                        .get(node_index)
                        .copied()
                        .ok_or_else(|| unsupported("state parent index"))?;
                    if parent_index.is_some() {
                        checkpoint_projection_before_charge(execution)?;
                        resources.charge_layout_work(1)?;
                    }
                    current_index = parent_index;
                }
            }
        }

        while let Some(node_index) = path.pop() {
            let parent_index = parent_by_index
                .get(node_index)
                .copied()
                .ok_or_else(|| unsupported("state parent index"))?;
            let depth = match parent_index {
                Some(parent_index) => depth_by_index
                    .get(parent_index)
                    .copied()
                    .ok_or_else(|| unsupported("state parent index"))?
                    .checked_add(1)
                    .ok_or_else(|| resources.nesting_overflow())?,
                None => 0,
            };
            let nesting_depth = depth
                .checked_add(1)
                .ok_or_else(|| resources.nesting_overflow())?;
            resources.check_nesting_depth(nesting_depth)?;

            *depth_by_index
                .get_mut(node_index)
                .ok_or_else(|| unsupported("state parent index"))? = depth;
            *visit_by_index
                .get_mut(node_index)
                .ok_or_else(|| unsupported("state parent index"))? = StateParentVisit::Complete;
            parent_first_indices.push(node_index);
        }
    }

    Ok((depth_by_index, parent_first_indices))
}

fn validate_supported_state_node(
    node: &StateDiagramRenderNode,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    if let Some(label) = node.label.as_ref()
        && !label.is_string()
    {
        if let Some(items) = label.as_array()
            && !items.is_empty()
        {
            for (index, item) in items.iter().enumerate() {
                checkpoint_projection(execution, index)?;
                if !item.is_string() {
                    return Err(unsupported("state labels with unsupported values"));
                }
            }
        } else {
            return Err(unsupported("state labels with unsupported values"));
        }
    }
    if is_state_note_group(node) {
        if !matches!(node.position.as_deref(), Some("left of" | "right of")) {
            return Err(unsupported("state note positions"));
        }
        return Ok(());
    }
    if is_state_note_node(node) {
        if node
            .position
            .as_deref()
            .is_some_and(|position| !matches!(position, "left of" | "right of"))
        {
            return Err(unsupported("state note positions"));
        }
        return Ok(());
    }
    if node.position.is_some() {
        return Err(unsupported("state node positions"));
    }
    if is_state_divider_group(node) {
        return Ok(());
    }
    state_node_shape(node, GraphDirection::TopDown)?;
    Ok(())
}

fn unsupported(feature: &'static str) -> AsciiError {
    AsciiError::UnsupportedFeature {
        diagram_type: STATE_DIAGRAM_TYPE,
        feature,
    }
}

fn group_members_by_id<'a>(
    model: &'a StateDiagramRenderModel,
    execution: AsciiExecution<'_>,
) -> Result<StateGroupMembers<'a>> {
    let mut members = StateGroupMembers::new();
    members
        .try_reserve(model.nodes.len())
        .map_err(|_| projection_allocation_failed())?;
    for (index, node) in model.nodes.iter().enumerate() {
        checkpoint_projection(execution, index)?;
        let Some(parent_id) = node.parent_id.as_ref() else {
            continue;
        };
        let group_members = members.entry(parent_id.as_str()).or_default();
        group_members
            .try_reserve(1)
            .map_err(|_| projection_allocation_failed())?;
        group_members.push(node.id.as_str());
    }
    Ok(members)
}

fn materialize_group_members(
    members: Option<&[&str]>,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Vec<String>> {
    let members = members.unwrap_or_default();
    resources.checkpoint()?;
    let mut owned = Vec::new();
    owned
        .try_reserve_exact(members.len())
        .map_err(|_| projection_allocation_failed())?;
    for (index, member) in members.iter().copied().enumerate() {
        checkpoint_projection(execution, index)?;
        owned.push(materialize_state_text(member, resources)?);
    }
    Ok(owned)
}

fn materialize_state_text(value: &str, resources: &ResourceContext) -> Result<String> {
    resources.checkpoint()?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(value.len())
        .map_err(|_| projection_allocation_failed())?;
    let mut start = 0usize;
    while start < value.len() {
        resources.checkpoint()?;
        let mut end = start
            .saturating_add(STATE_TEXT_COPY_CHUNK_BYTES)
            .min(value.len());
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        owned.push_str(&value[start..end]);
        start = end;
    }
    Ok(owned)
}

fn materialize_note_side_constraint(
    constraint: Option<&StateNoteSideConstraint<'_>>,
    resources: &ResourceContext,
) -> Result<Option<GraphNodeSideConstraint>> {
    constraint
        .map(|constraint| {
            materialize_state_text(constraint.anchor_id, resources)
                .map(|anchor_id| GraphNodeSideConstraint::new(anchor_id, constraint.side))
        })
        .transpose()
}

fn note_node_parent_by_id<'a>(
    model: &'a StateDiagramRenderModel,
    execution: AsciiExecution<'_>,
) -> Result<StateNoteParentIndex<'a>> {
    let mut parents = HashMap::new();
    parents
        .try_reserve(model.nodes.len())
        .map_err(|_| projection_allocation_failed())?;
    for (index, node) in model.nodes.iter().enumerate() {
        checkpoint_projection(execution, index)?;
        if !is_state_note_node(node) {
            continue;
        }
        let Some(parent_id) = node.parent_id.as_ref() else {
            continue;
        };
        parents.insert(node.id.as_str(), parent_id.as_str());
    }
    Ok(parents)
}

fn note_side_constraints<'a>(
    model: &'a StateDiagramRenderModel,
    note_node_parent_by_id: &StateNoteParentIndex<'a>,
    execution: AsciiExecution<'_>,
) -> Result<StateNoteSideConstraints<'a>> {
    let mut note_group_by_id = HashMap::<&str, &StateDiagramRenderNode>::new();
    note_group_by_id
        .try_reserve(model.nodes.len())
        .map_err(|_| projection_allocation_failed())?;
    for (index, node) in model.nodes.iter().enumerate() {
        checkpoint_projection(execution, index)?;
        if is_state_note_group(node) {
            note_group_by_id.insert(node.id.as_str(), node);
        }
    }

    let mut constraints = HashMap::new();
    constraints
        .try_reserve(note_group_by_id.len())
        .map_err(|_| projection_allocation_failed())?;
    for (index, edge) in model.edges.iter().enumerate() {
        checkpoint_projection(execution, index)?;
        if !is_note_edge(edge) {
            continue;
        }
        let from = remap_note_endpoint(&edge.start, note_node_parent_by_id);
        let to = remap_note_endpoint(&edge.end, note_node_parent_by_id);
        let (note_group, anchor_id) = match (
            note_group_by_id.get(from).copied(),
            note_group_by_id.get(to).copied(),
        ) {
            (Some(note_group), None) => (note_group, to),
            (None, Some(note_group)) => (note_group, from),
            _ => return Err(unsupported("state note edge ownership")),
        };
        let side = match note_group.position.as_deref() {
            Some("left of") => GraphNodeSide::Left,
            Some("right of") => GraphNodeSide::Right,
            _ => return Err(unsupported("state note positions")),
        };
        if constraints
            .insert(
                note_group.id.as_str(),
                StateNoteSideConstraint { anchor_id, side },
            )
            .is_some()
        {
            return Err(unsupported("state note groups with multiple owners"));
        }
    }
    if constraints.len() != note_group_by_id.len() {
        return Err(unsupported("state note groups without exactly one owner"));
    }
    Ok(constraints)
}

fn canonical_note_edge_endpoints<'a>(
    from: &'a str,
    to: &'a str,
    constraints: &StateNoteSideConstraints<'a>,
    direction: GraphDirection,
) -> Result<(&'a str, &'a str)> {
    let (note_id, anchor_id, constraint) = match (constraints.get(from), constraints.get(to)) {
        (Some(constraint), None) => (from, to, constraint),
        (None, Some(constraint)) => (to, from, constraint),
        _ => return Err(unsupported("state note edge ownership")),
    };
    if constraint.anchor_id != anchor_id {
        return Err(unsupported("state note edge ownership"));
    }
    let side = if direction == GraphDirection::RightLeft {
        constraint.side.reversed()
    } else {
        constraint.side
    };
    Ok(match side {
        GraphNodeSide::Left => (note_id, anchor_id),
        GraphNodeSide::Right => (anchor_id, note_id),
    })
}

fn sorted_group_nodes<'a>(
    model: &'a StateDiagramRenderModel,
    group_members: &StateGroupMembers<'a>,
    parent_projection: &StateParentProjection,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<Vec<StateGroupOrderEntry<'a>>> {
    let mut groups = Vec::new();
    groups
        .try_reserve(model.nodes.len())
        .map_err(|_| projection_allocation_failed())?;
    for (node_index, node) in model.nodes.iter().enumerate() {
        checkpoint_projection(execution, node_index)?;
        if !is_group_container(node, group_members) {
            continue;
        }
        let depth = parent_projection
            .depth_by_index
            .get(node_index)
            .copied()
            .ok_or_else(|| unsupported("state parent index"))?;
        groups.push(StateGroupOrderEntry {
            node_index,
            depth,
            node,
        });
    }
    charge_state_group_sort_work(groups.len(), resources, execution)?;
    groups.sort_by_key(|group| std::cmp::Reverse(group.depth));
    Ok(groups)
}

fn charge_state_group_sort_work(
    len: usize,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    if len <= 1 {
        return Ok(());
    }
    let comparison_levels = usize::BITS as usize - (len - 1).leading_zeros() as usize;
    // Account two numeric key visits per comparison level before entering the sort.
    let key_visits = resources.checked_work_mul(len, comparison_levels)?;
    let sort_work = resources.checked_work_mul(key_visits, 2)?;
    checkpoint_projection_before_charge(execution)?;
    resources.charge_layout_work(sort_work)
}

fn state_direction_projection(
    model: &StateDiagramRenderModel,
    root_direction: GraphDirection,
    parent_projection: &StateParentProjection,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<StateDirectionProjection> {
    let mut explicit_by_index = Vec::new();
    explicit_by_index
        .try_reserve_exact(model.nodes.len())
        .map_err(|_| projection_allocation_failed())?;
    for (index, node) in model.nodes.iter().enumerate() {
        checkpoint_projection(execution, index)?;
        let explicit = if node.explicit_dir == Some(true) {
            Some(parse_state_direction(node.dir.as_deref().ok_or_else(
                || unsupported("state explicit direction without value"),
            )?)?)
        } else {
            None
        };
        explicit_by_index.push(explicit);
    }

    let mut inherited_by_index = Vec::new();
    inherited_by_index
        .try_reserve_exact(model.nodes.len())
        .map_err(|_| projection_allocation_failed())?;
    inherited_by_index.resize(model.nodes.len(), None);
    // Validation cached a parent-first order, so every inherited value is already available here.
    for (iteration, node_index) in parent_projection
        .parent_first_indices
        .iter()
        .copied()
        .enumerate()
    {
        checkpoint_projection(execution, iteration)?;
        let parent_index = parent_projection
            .parent_by_index
            .get(node_index)
            .copied()
            .ok_or_else(|| unsupported("state parent index"))?;
        let inherited = match parent_index {
            Some(parent_index) => {
                checkpoint_projection_before_charge(execution)?;
                resources.charge_layout_work(1)?;
                explicit_by_index
                    .get(parent_index)
                    .copied()
                    .flatten()
                    .or_else(|| inherited_by_index.get(parent_index).copied().flatten())
            }
            None => None,
        };
        *inherited_by_index
            .get_mut(node_index)
            .ok_or_else(|| unsupported("state parent index"))? = inherited;
    }

    let mut node_by_index = Vec::new();
    node_by_index
        .try_reserve_exact(model.nodes.len())
        .map_err(|_| projection_allocation_failed())?;
    let mut group_by_index = Vec::new();
    group_by_index
        .try_reserve_exact(model.nodes.len())
        .map_err(|_| projection_allocation_failed())?;
    for (index, (explicit, inherited)) in explicit_by_index
        .into_iter()
        .zip(inherited_by_index)
        .enumerate()
    {
        checkpoint_projection(execution, index)?;
        node_by_index.push(inherited.unwrap_or(root_direction));
        group_by_index.push(explicit.or(inherited));
    }
    Ok(StateDirectionProjection {
        node_by_index,
        group_by_index,
    })
}

fn checkpoint_projection_before_charge(execution: AsciiExecution<'_>) -> Result<()> {
    execution.checkpoint(merman_core::OperationPhase::Semantic)
}

fn is_group_container(
    node: &StateDiagramRenderNode,
    group_members: &StateGroupMembers<'_>,
) -> bool {
    if is_state_note_group(node) {
        return false;
    }
    node.is_group
        && group_members
            .get(node.id.as_str())
            .is_some_and(|members| !members.is_empty())
}

fn state_node_shape(
    node: &StateDiagramRenderNode,
    direction: GraphDirection,
) -> Result<GraphNodeShape> {
    match node.shape.as_str() {
        "rect" => Ok(GraphNodeShape::Rect),
        "rectWithTitle" => Ok(GraphNodeShape::StateWithTitle),
        "roundedWithTitle" | "noteGroup" => Ok(GraphNodeShape::Rounded),
        "stateStart" => Ok(GraphNodeShape::StateStart),
        "stateEnd" => Ok(GraphNodeShape::StateEnd),
        "fork" | "join" => match direction.canonical() {
            GraphDirection::LeftRight => Ok(GraphNodeShape::ForkJoinVertical),
            GraphDirection::TopDown => Ok(GraphNodeShape::ForkJoinHorizontal),
            GraphDirection::RightLeft | GraphDirection::BottomTop => unreachable!(),
        },
        "choice" => Ok(GraphNodeShape::Choice),
        _ => Err(unsupported("state node shapes")),
    }
}

fn state_group_kind(node: &StateDiagramRenderNode) -> GraphGroupKind {
    if is_state_divider_group(node) {
        GraphGroupKind::Divider
    } else {
        GraphGroupKind::Container
    }
}

fn is_state_note_group(node: &StateDiagramRenderNode) -> bool {
    node.shape == "noteGroup"
}

fn is_state_note_node(node: &StateDiagramRenderNode) -> bool {
    node.shape == "note"
}

fn is_state_divider_group(node: &StateDiagramRenderNode) -> bool {
    node.shape == "divider"
}

fn state_group_title_is_empty(node: &StateDiagramRenderNode) -> bool {
    is_state_divider_group(node) || is_state_pseudo_shape(node.shape.as_str())
}

fn is_state_pseudo_shape(shape: &str) -> bool {
    matches!(
        shape,
        "stateStart" | "stateEnd" | "fork" | "join" | "choice"
    )
}

fn edge_marker(edge: &StateDiagramRenderEdge) -> GraphEdgeMarker {
    if is_note_edge(edge) {
        GraphEdgeMarker::Open
    } else {
        GraphEdgeMarker::Point
    }
}

fn is_note_edge(edge: &StateDiagramRenderEdge) -> bool {
    edge.classes
        .split_whitespace()
        .any(|class| class == "note-edge")
}

fn remap_note_endpoint<'a>(
    endpoint: &'a str,
    note_node_parent_by_id: &StateNoteParentIndex<'a>,
) -> &'a str {
    note_node_parent_by_id
        .get(endpoint)
        .copied()
        .unwrap_or(endpoint)
}

fn parse_state_direction(direction: &str) -> Result<GraphDirection> {
    match direction.trim() {
        "LR" => Ok(GraphDirection::LeftRight),
        "RL" => Ok(GraphDirection::RightLeft),
        "TB" | "TD" => Ok(GraphDirection::TopDown),
        "BT" => Ok(GraphDirection::BottomTop),
        _ => Err(unsupported("unsupported state directions")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_core::resources::ResourceProfile;
    use merman_core::{CancelReason, OperationControl, OperationPhase};

    #[test]
    fn state_projection_cancellation_precedes_the_first_resource_debit() {
        let model = StateDiagramRenderModel {
            direction: "TB".to_string(),
            nodes: vec![state_node("a")],
            ..StateDiagramRenderModel::default()
        };
        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .expect("one work unit should be a valid limit");
        let mut resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        control.cancel();

        let error = from_state_model_with_context_and_execution(
            &model,
            TerminalWidthProfile::Unicode,
            &mut resources,
            AsciiExecution::new(&control, &policy),
        )
        .expect_err("cancellation should win before state projection work exhaustion");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Semantic
                    && cancelled.reason == CancelReason::Requested
        ));
        assert_eq!(resources.layout_work_used(), 0);
    }

    #[test]
    fn state_projection_accepts_exact_work_and_rejects_max_minus_one() {
        let model = StateDiagramRenderModel {
            direction: "TB".to_string(),
            nodes: vec![state_node("a")],
            ..StateDiagramRenderModel::default()
        };
        assert_projection_accepts_exact_work(&model);
    }

    #[test]
    fn state_style_bytes_are_admitted_at_the_exact_work_boundary() {
        let mut node = state_node("a");
        node.css_compiled_styles = vec!["fill:#112233".to_string()];
        node.css_styles = vec!["fill:transparent".to_string()];
        node.label_style = "border:#445566".to_string();
        let model = StateDiagramRenderModel {
            direction: "TB".to_string(),
            nodes: vec![node],
            ..StateDiagramRenderModel::default()
        };
        assert_projection_accepts_exact_work(&model);
    }

    #[test]
    fn state_projection_admits_fallback_text_before_owned_source_materialization() {
        let mut group = state_node("pseudo-group");
        group.shape = "choice".to_string();
        group.is_group = true;
        let mut node = state_node("AB");
        node.parent_id = Some(group.id.clone());
        let model = StateDiagramRenderModel {
            direction: "TB".to_string(),
            nodes: vec![group, node],
            ..StateDiagramRenderModel::default()
        };
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxDocumentCells, 1)
            .expect("one document cell should be a valid limit");
        let mut resources = ResourceContext::new(policy);

        let error = from_state_model_with_context_and_execution(
            &model,
            TerminalWidthProfile::Unicode,
            &mut resources,
            AsciiExecution::for_test(&policy),
        )
        .expect_err("the aggregate node-label bound must reject before source ownership");

        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxDocumentCells
                    && details.actual == 2
                    && details.max == 1
        ));
        assert_eq!(resources.layout_work_used(), 0);
        assert_eq!(resources.document_cells_used(), 0);
    }

    #[test]
    fn state_note_constraint_projection_accepts_exact_work_and_rejects_max_minus_one() {
        let mut note_group = state_node("note-group");
        note_group.shape = "noteGroup".to_string();
        note_group.is_group = true;
        note_group.node_type = Some("group".to_string());
        note_group.position = Some("right of".to_string());
        let mut note = state_node("note");
        note.shape = "note".to_string();
        note.parent_id = Some("note-group".to_string());
        note.position = Some("right of".to_string());
        let model = StateDiagramRenderModel {
            direction: "TB".to_string(),
            nodes: vec![state_node("a"), note_group, note],
            edges: vec![StateDiagramRenderEdge {
                id: "note-edge".to_string(),
                start: "a".to_string(),
                end: "note".to_string(),
                classes: "transition note-edge".to_string(),
                arrow_type_end: String::new(),
                label: String::new(),
            }],
            ..StateDiagramRenderModel::default()
        };
        assert_projection_accepts_exact_work(&model);
    }

    #[test]
    fn large_flat_state_model_keeps_ancestor_work_zero() {
        const NODE_COUNT: usize = 4_096;

        let model = StateDiagramRenderModel {
            direction: "TB".to_string(),
            nodes: (0..NODE_COUNT)
                .map(|index| state_node(&format!("state-{index}")))
                .collect(),
            ..StateDiagramRenderModel::default()
        };
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);

        let validation_resources = ResourceContext::new(unbounded);
        let execution = AsciiExecution::for_test(&unbounded);
        let parent_projection =
            validate_supported_state_model(&model, &validation_resources, execution)
                .expect("large flat state model should pass parent projection");
        assert_eq!(parent_projection.parent_by_index.len(), NODE_COUNT);
        assert!(
            parent_projection
                .parent_by_index
                .iter()
                .all(Option::is_none)
        );
        assert!(
            parent_projection
                .depth_by_index
                .iter()
                .all(|depth| *depth == 0)
        );
        assert_eq!(validation_resources.layout_work_used(), 0);

        let group_members = group_members_by_id(&model, execution)
            .expect("large flat state model should project group members");
        let sorting_resources = ResourceContext::new(unbounded);
        let groups = sorted_group_nodes(
            &model,
            &group_members,
            &parent_projection,
            &sorting_resources,
            execution,
        )
        .expect("large flat state model should sort groups");
        assert!(groups.is_empty());
        assert_eq!(sorting_resources.layout_work_used(), 0);

        let direction_resources = ResourceContext::new(unbounded);
        let projection = state_direction_projection(
            &model,
            GraphDirection::TopDown,
            &parent_projection,
            &direction_resources,
            execution,
        )
        .expect("large flat state model should project directions");
        assert_eq!(projection.node_by_index.len(), NODE_COUNT);
        assert!(projection.group_by_index.iter().all(Option::is_none));
        assert_eq!(direction_resources.layout_work_used(), 0);
        assert_eq!(
            state_projection_work(&model, &direction_resources, execution)
                .expect("large flat state projection work should fit"),
            NODE_COUNT * STATE_NODE_PROJECTION_WORK_UNITS
        );
    }

    #[test]
    fn state_parent_projection_rejects_nesting_before_full_chain_work() {
        const NODE_COUNT: usize = 64;
        const MAX_NESTING_DEPTH: usize = 8;

        let id_prefix = "x".repeat(512);
        let mut nodes = Vec::new();
        for index in 0..NODE_COUNT {
            let mut node = state_node(&format!("{id_prefix}-{index}"));
            node.is_group = index + 1 < NODE_COUNT;
            if index > 0 {
                node.parent_id = Some(format!("{id_prefix}-{}", index - 1));
            }
            nodes.push(node);
        }
        nodes.reverse();
        let model = StateDiagramRenderModel {
            direction: "TB".to_string(),
            nodes,
            ..StateDiagramRenderModel::default()
        };
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxNestingDepth, MAX_NESTING_DEPTH)
            .expect("the test nesting limit should be valid");
        let resources = ResourceContext::new(policy);
        let execution = AsciiExecution::for_test(&policy);

        let error = validate_supported_state_model(&model, &resources, execution)
            .expect_err("the parent projection should reject the first over-depth node");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxNestingDepth
                    && details.actual == MAX_NESTING_DEPTH + 1
                    && details.max == MAX_NESTING_DEPTH
        ));
        assert!(resources.layout_work_used() <= MAX_NESTING_DEPTH);
    }

    #[test]
    fn deep_state_parent_chain_caches_semantics_with_bounded_work() {
        const NODE_COUNT: usize = 64;

        let mut nodes = Vec::new();
        for index in 0..NODE_COUNT {
            let mut node = state_node(&format!("state-{index}"));
            node.is_group = index + 1 < NODE_COUNT;
            if index == 0 {
                node.explicit_dir = Some(true);
                node.dir = Some("LR".to_string());
            } else {
                node.parent_id = Some(format!("state-{}", index - 1));
            }
            nodes.push(node);
        }
        nodes.reverse();
        let model = StateDiagramRenderModel {
            direction: "TB".to_string(),
            nodes,
            ..StateDiagramRenderModel::default()
        };
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);

        let validation_resources = ResourceContext::new(unbounded);
        let execution = AsciiExecution::for_test(&unbounded);
        let parent_projection =
            validate_supported_state_model(&model, &validation_resources, execution)
                .expect("deep valid state parent chain should cache its parents");
        let leaf_index = model
            .nodes
            .iter()
            .position(|node| node.id == "state-63")
            .expect("the chain leaf should remain in the model");
        assert_eq!(
            parent_projection.depth_by_index.get(leaf_index).copied(),
            Some(NODE_COUNT - 1)
        );
        assert_eq!(parent_projection.parent_first_indices.len(), NODE_COUNT);
        assert!(validation_resources.layout_work_used() < NODE_COUNT);

        let group_members = group_members_by_id(&model, execution)
            .expect("deep valid state parent chain should project group members");
        let sorting_resources = ResourceContext::new(unbounded);
        let groups = sorted_group_nodes(
            &model,
            &group_members,
            &parent_projection,
            &sorting_resources,
            execution,
        )
        .expect("deep valid state parent chain should sort groups");
        assert_eq!(groups.len(), NODE_COUNT - 1);
        assert_eq!(
            groups.first().map(|group| group.node.id.as_str()),
            Some("state-62")
        );
        assert_eq!(
            groups.last().map(|group| group.node.id.as_str()),
            Some("state-0")
        );
        let group_count = NODE_COUNT - 1;
        let comparison_levels = usize::BITS as usize - (group_count - 1).leading_zeros() as usize;
        let sort_work_upper_bound = group_count * comparison_levels * 2;
        assert!(sorting_resources.layout_work_used() <= sort_work_upper_bound);

        let direction_resources = ResourceContext::new(unbounded);
        let projection = state_direction_projection(
            &model,
            GraphDirection::TopDown,
            &parent_projection,
            &direction_resources,
            execution,
        )
        .expect("deep valid state parent chain should project directions");
        let deepest_group_index = model
            .nodes
            .iter()
            .position(|node| node.id == "state-62")
            .expect("the deepest group should remain in the model");
        assert_eq!(
            projection.node_by_index.get(leaf_index),
            Some(&GraphDirection::LeftRight)
        );
        assert_eq!(
            projection.group_by_index.get(deepest_group_index),
            Some(&Some(GraphDirection::LeftRight))
        );
        assert!(direction_resources.layout_work_used() < NODE_COUNT);

        assert_projection_accepts_exact_work(&model);
    }

    fn assert_projection_accepts_exact_work(model: &StateDiagramRenderModel) {
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut measured = ResourceContext::new(unbounded);
        from_state_model_with_context_and_execution(
            model,
            TerminalWidthProfile::Unicode,
            &mut measured,
            AsciiExecution::for_test(&unbounded),
        )
        .expect("unbounded state projection should succeed");
        let exact = measured.layout_work_used();
        assert!(exact > 1);

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact)
            .expect("exact state projection work limit should be valid");
        from_state_model_with_resources(model, exact_policy)
            .expect("exact state projection work limit should pass");

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact - 1)
            .expect("max-minus-one state projection work limit should be valid");
        let error = from_state_model_with_resources(model, below_policy)
            .expect_err("max-minus-one state projection work limit should fail");
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a layout-work resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
        assert_eq!(details.actual, exact);
        assert_eq!(details.max, exact - 1);
    }

    fn state_node(id: &str) -> StateDiagramRenderNode {
        StateDiagramRenderNode {
            id: id.to_string(),
            label_style: String::new(),
            label: None,
            description: None,
            dom_id: String::new(),
            is_group: false,
            node_type: None,
            parent_id: None,
            css_classes: String::new(),
            css_compiled_styles: Vec::new(),
            css_styles: Vec::new(),
            dir: None,
            explicit_dir: None,
            padding: None,
            rx: None,
            ry: None,
            shape: "rect".to_string(),
            position: None,
        }
    }
}
