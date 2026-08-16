use super::model::{
    AsciiGraph, GraphDirection, GraphEdgeAttrs, GraphEdgeMarker, GraphEdgeStroke,
    GraphNodeSemantics,
};
use super::shape::{ResolvedGraphNodeShape, resolve_flowchart_node_shape};
use super::style::{resolve_edge_style, resolve_group_style, resolve_node_style};
use crate::AsciiDirection;
use crate::error::{AsciiError, Result};
use crate::operation::AsciiExecution;
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use crate::safe_text::{
    NormalizedTextPlan, NormalizedTrimmedTextPlan, charge_text_layout, try_plan_normalized_text,
    try_plan_normalized_trimmed_text,
};
use merman_core::diagrams::flowchart::{
    FlowEdgeMarker as CoreFlowEdgeMarker, FlowEdgeStroke as CoreFlowEdgeStroke,
    FlowEdgeVisibility as CoreFlowEdgeVisibility, FlowchartModel,
};
use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;

#[cfg(test)]
pub(crate) fn from_flowchart_model(
    model: &FlowchartModel,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<AsciiGraph> {
    let policy = resources.policy();
    let control = merman_core::OperationControl::new();
    from_flowchart_model_with_execution(
        model,
        options,
        resources,
        AsciiExecution::new(&control, &policy),
    )
}

pub(crate) fn from_flowchart_model_with_execution(
    model: &FlowchartModel,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<AsciiGraph> {
    execution.rebind_resource_context(resources, merman_core::OperationPhase::Semantic);
    resources.transaction(|resources| {
        from_flowchart_model_transactional(model, options, resources, execution)
    })
}

fn from_flowchart_model_transactional(
    model: &FlowchartModel,
    options: &AsciiRenderOptions,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<AsciiGraph> {
    let memberships = preflight_flowchart_projection(model, resources, execution)?;
    validate_supported_flowchart_model(model, &memberships, resources, execution)?;

    let direction = if let Some(direction) = model.direction.as_deref() {
        parse_direction(direction)?
    } else {
        match options.default_direction {
            AsciiDirection::LeftRight => GraphDirection::LeftRight,
            AsciiDirection::TopDown => GraphDirection::TopDown,
        }
    };
    let wrap_width = NonZeroUsize::new(options.flowchart_node_label_wrap_width).ok_or(
        AsciiError::InvalidOption {
            field: "flowchart_node_label_wrap_width",
            message: "must be greater than 0",
        },
    )?;
    let projection_plan = FlowchartProjectionPlan::try_new(
        model,
        &memberships,
        direction,
        options.terminal_width_profile,
        resources,
        execution,
    )?;

    let FlowchartProjectionPlan {
        nodes,
        edge_labels,
        groups,
    } = projection_plan;
    debug_assert_eq!(model.nodes.len(), nodes.len());
    debug_assert_eq!(model.edges.len(), edge_labels.len());
    debug_assert_eq!(memberships.canonical_group_indices().len(), groups.len());

    let mut graph = AsciiGraph::new(direction);
    graph.wrap_node_labels_at(wrap_width);
    graph.try_reserve_projection(
        model.nodes.len(),
        model.edges.len(),
        memberships.canonical_group_indices().len(),
    )?;

    for (index, (node, node_plan)) in model.nodes.iter().zip(nodes).enumerate() {
        checkpoint_projection(execution, index)?;
        let Some(node_plan) = node_plan else {
            continue;
        };
        let id = try_clone_projection_string(&node.id)?;
        let label = try_clone_projection_string(node_plan.label)?;
        graph.add_node_with_semantics(
            id,
            label,
            node_plan.shape.shape,
            resolve_node_style(model, node),
            GraphNodeSemantics::default(),
        );
    }

    for (index, (edge, label_plan)) in model.edges.iter().zip(edge_labels).enumerate() {
        checkpoint_projection(execution, index)?;
        let from = try_clone_projection_string(&edge.from)?;
        let to = try_clone_projection_string(&edge.to)?;
        graph.add_edge_with_attrs(
            from,
            to,
            GraphEdgeAttrs {
                id: Some(try_clone_projection_string(&edge.id)?),
                is_user_defined_id: edge.is_user_defined_id,
                label: label_plan
                    .map(|plan| {
                        plan.materialize_after_admission_with_checkpoint(|iteration| {
                            checkpoint_projection(execution, iteration)
                        })
                    })
                    .transpose()?,
                stroke: parse_flow_edge_stroke(edge.stroke_kind, edge.visibility),
                start_marker: parse_flow_edge_marker(edge.start_marker),
                end_marker: parse_flow_edge_marker(edge.end_marker),
                length: edge.length,
                style: resolve_edge_style(model, edge),
            },
        );
    }

    for (index, (group_plan, (canonical_index, canonical_members))) in groups
        .into_iter()
        .zip(memberships.canonical_groups())
        .enumerate()
    {
        checkpoint_projection(execution, index)?;
        debug_assert_eq!(group_plan.canonical_index, canonical_index);
        let subgraph = &model.subgraphs[canonical_index];
        let mut members = Vec::new();
        members
            .try_reserve_exact(canonical_members.len())
            .map_err(|_| projection_allocation_failed())?;
        for (member_index, member) in canonical_members.iter().enumerate() {
            checkpoint_projection(execution, member_index)?;
            members.push(try_clone_projection_string(member)?);
        }
        graph.add_group_with_style(
            try_clone_projection_string(&subgraph.id)?,
            group_plan
                .title
                .materialize_after_admission_with_checkpoint(|iteration| {
                    checkpoint_projection(execution, iteration)
                })?,
            group_plan.direction,
            members,
            resolve_group_style(model, subgraph),
        );
    }

    Ok(graph)
}

fn checkpoint_projection(execution: AsciiExecution<'_>, iteration: usize) -> Result<()> {
    execution.checkpoint_loop(merman_core::OperationPhase::Semantic, iteration)
}

fn parse_flow_edge_marker(marker: CoreFlowEdgeMarker) -> GraphEdgeMarker {
    match marker {
        CoreFlowEdgeMarker::None => GraphEdgeMarker::Open,
        CoreFlowEdgeMarker::Point => GraphEdgeMarker::Point,
        CoreFlowEdgeMarker::Circle => GraphEdgeMarker::Circle,
        CoreFlowEdgeMarker::Cross => GraphEdgeMarker::Cross,
    }
}

fn parse_flow_edge_stroke(
    stroke: CoreFlowEdgeStroke,
    visibility: CoreFlowEdgeVisibility,
) -> GraphEdgeStroke {
    if matches!(visibility, CoreFlowEdgeVisibility::Invisible) {
        return GraphEdgeStroke::Invisible;
    }
    match stroke {
        CoreFlowEdgeStroke::Normal => GraphEdgeStroke::Normal,
        CoreFlowEdgeStroke::Dotted => GraphEdgeStroke::Dotted,
        CoreFlowEdgeStroke::Thick => GraphEdgeStroke::Thick,
    }
}

fn preflight_flowchart_projection<'a>(
    model: &'a FlowchartModel,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<FlowchartMembershipIndex<'a>> {
    resources.charge_layout_work(1)?;
    if let Some(direction) = model.direction.as_deref() {
        charge_text_layout(resources, direction)?;
    }

    for (index, node) in model.nodes.iter().enumerate() {
        checkpoint_projection(execution, index)?;
        resources.charge_layout_work(1)?;
        charge_text_layout(resources, &node.id)?;
        if let Some(shape) = node.layout_shape.as_deref() {
            charge_text_layout(resources, shape)?;
        }
        for (declaration_index, declaration) in node.classes.iter().chain(&node.styles).enumerate()
        {
            checkpoint_projection(execution, declaration_index)?;
            resources.charge_layout_work(1)?;
            charge_text_layout(resources, declaration)?;
        }
    }

    for (index, edge) in model.edges.iter().enumerate() {
        checkpoint_projection(execution, index)?;
        resources.charge_layout_work(1)?;
        charge_text_layout(resources, &edge.id)?;
        charge_text_layout(resources, &edge.from)?;
        charge_text_layout(resources, &edge.to)?;
        charge_text_layout(resources, &edge.arrow)?;
        if let Some(edge_type) = edge.edge_type.as_deref() {
            charge_text_layout(resources, edge_type)?;
        }
        if let Some(stroke) = edge.stroke.as_deref() {
            charge_text_layout(resources, stroke)?;
        }
        for (declaration_index, declaration) in edge.classes.iter().chain(&edge.style).enumerate() {
            checkpoint_projection(execution, declaration_index)?;
            resources.charge_layout_work(1)?;
            charge_text_layout(resources, declaration)?;
        }
    }

    for (index, subgraph) in model.subgraphs.iter().enumerate() {
        checkpoint_projection(execution, index)?;
        resources.charge_layout_work(1)?;
        charge_text_layout(resources, &subgraph.id)?;
        for (member_index, member) in subgraph.nodes.iter().enumerate() {
            checkpoint_projection(execution, member_index)?;
            resources.charge_layout_work(1)?;
            charge_text_layout(resources, member)?;
        }
        for (declaration_index, declaration) in
            subgraph.classes.iter().chain(&subgraph.styles).enumerate()
        {
            checkpoint_projection(execution, declaration_index)?;
            resources.charge_layout_work(1)?;
            charge_text_layout(resources, declaration)?;
        }
    }

    for (index, (class_name, declarations)) in model.class_defs.iter().enumerate() {
        checkpoint_projection(execution, index)?;
        resources.charge_layout_work(1)?;
        charge_text_layout(resources, class_name)?;
        for (declaration_index, declaration) in declarations.iter().enumerate() {
            checkpoint_projection(execution, declaration_index)?;
            resources.charge_layout_work(1)?;
            charge_text_layout(resources, declaration)?;
        }
    }

    let memberships = FlowchartMembershipIndex::try_new(model, resources, execution)?;
    preflight_subgraph_nesting(model, &memberships, resources, execution)?;
    Ok(memberships)
}

#[derive(Debug)]
struct FlowchartMembershipIndex<'a> {
    parent_group_by_member: HashMap<&'a str, usize>,
    group_ids: HashSet<&'a str>,
    canonical_group_indices: Vec<usize>,
    canonical_group_members: Vec<Vec<&'a str>>,
}

impl<'a> FlowchartMembershipIndex<'a> {
    fn try_new(
        model: &'a FlowchartModel,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<Self> {
        let member_count = model.subgraphs.iter().try_fold(0usize, |total, group| {
            resources.checked_work_add(total, group.nodes.len())
        })?;
        let construction_work = resources.checked_work_add(model.subgraphs.len(), member_count)?;
        resources.charge_layout_work(construction_work)?;

        let mut parent_group_by_member = HashMap::new();
        parent_group_by_member
            .try_reserve(member_count)
            .map_err(|_| projection_allocation_failed())?;
        let mut group_ids = HashSet::new();
        group_ids
            .try_reserve(model.subgraphs.len())
            .map_err(|_| projection_allocation_failed())?;
        let mut canonical_slot_by_group_id = HashMap::new();
        canonical_slot_by_group_id
            .try_reserve(model.subgraphs.len())
            .map_err(|_| projection_allocation_failed())?;
        let mut canonical_group_indices = Vec::new();
        canonical_group_indices
            .try_reserve(model.subgraphs.len())
            .map_err(|_| projection_allocation_failed())?;
        let mut canonical_group_members = Vec::new();
        canonical_group_members
            .try_reserve(model.subgraphs.len())
            .map_err(|_| projection_allocation_failed())?;
        let mut canonical_group_member_ids = Vec::new();
        canonical_group_member_ids
            .try_reserve(model.subgraphs.len())
            .map_err(|_| projection_allocation_failed())?;
        for (parent_index, subgraph) in model.subgraphs.iter().enumerate() {
            checkpoint_projection(execution, parent_index)?;
            let group_id = subgraph.id.as_str();
            let (canonical_slot, needs_member_reserve) =
                match canonical_slot_by_group_id.entry(group_id) {
                    std::collections::hash_map::Entry::Occupied(entry) => (*entry.get(), true),
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        let canonical_slot = canonical_group_indices.len();
                        entry.insert(canonical_slot);
                        group_ids.insert(group_id);
                        canonical_group_indices.push(parent_index);
                        let mut members = Vec::new();
                        members
                            .try_reserve_exact(subgraph.nodes.len())
                            .map_err(|_| projection_allocation_failed())?;
                        canonical_group_members.push(members);
                        let mut member_ids = HashSet::new();
                        member_ids
                            .try_reserve(subgraph.nodes.len())
                            .map_err(|_| projection_allocation_failed())?;
                        canonical_group_member_ids.push(member_ids);
                        (canonical_slot, false)
                    }
                };
            if needs_member_reserve {
                canonical_group_members[canonical_slot]
                    .try_reserve(subgraph.nodes.len())
                    .map_err(|_| projection_allocation_failed())?;
                canonical_group_member_ids[canonical_slot]
                    .try_reserve(subgraph.nodes.len())
                    .map_err(|_| projection_allocation_failed())?;
            }
            let canonical_index = canonical_group_indices[canonical_slot];
            for (member_index, member) in subgraph.nodes.iter().enumerate() {
                checkpoint_projection(execution, member_index)?;
                // Preserve the former first-match parent semantics without rescanning candidates.
                parent_group_by_member
                    .entry(member.as_str())
                    .or_insert(canonical_index);
                if canonical_group_member_ids[canonical_slot].insert(member.as_str()) {
                    canonical_group_members[canonical_slot].push(member.as_str());
                }
            }
        }

        Ok(Self {
            parent_group_by_member,
            group_ids,
            canonical_group_indices,
            canonical_group_members,
        })
    }

    fn parent_group_index(&self, member_id: &str) -> Option<usize> {
        self.parent_group_by_member.get(member_id).copied()
    }

    fn member_ids(&self) -> impl Iterator<Item = &'a str> + '_ {
        self.parent_group_by_member.keys().copied()
    }

    fn is_group_id(&self, endpoint_id: &str) -> bool {
        self.group_ids.contains(endpoint_id)
    }

    fn canonical_group_indices(&self) -> &[usize] {
        &self.canonical_group_indices
    }

    fn canonical_groups(&self) -> impl Iterator<Item = (usize, &[&'a str])> + '_ {
        self.canonical_group_indices
            .iter()
            .copied()
            .zip(self.canonical_group_members.iter().map(Vec::as_slice))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NestingVisitState {
    Unvisited,
    Visiting,
    Complete,
}

fn preflight_subgraph_nesting(
    model: &FlowchartModel,
    memberships: &FlowchartMembershipIndex<'_>,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    // Every group is inspected once by the outer pass and resolved at most once.
    let traversal_work = resources.checked_work_mul(model.subgraphs.len(), 2)?;
    resources.charge_layout_work(traversal_work)?;

    let mut states = Vec::new();
    states
        .try_reserve_exact(model.subgraphs.len())
        .map_err(|_| projection_allocation_failed())?;
    states.resize(model.subgraphs.len(), NestingVisitState::Unvisited);

    let mut depths = Vec::new();
    depths
        .try_reserve_exact(model.subgraphs.len())
        .map_err(|_| projection_allocation_failed())?;
    depths.resize(model.subgraphs.len(), 0usize);

    let mut path = Vec::new();
    path.try_reserve(model.subgraphs.len())
        .map_err(|_| projection_allocation_failed())?;

    for (start_ordinal, &start_index) in memberships.canonical_group_indices().iter().enumerate() {
        checkpoint_projection(execution, start_ordinal)?;
        if states[start_index] == NestingVisitState::Complete {
            continue;
        }
        path.clear();
        let mut current_index = start_index;
        let mut base_depth;
        let mut traversal_step = 0usize;
        loop {
            checkpoint_projection(execution, traversal_step)?;
            traversal_step = traversal_step.saturating_add(1);
            match states[current_index] {
                NestingVisitState::Complete => {
                    base_depth = depths[current_index];
                    break;
                }
                NestingVisitState::Visiting => {
                    let cycle_rejection_depth =
                        model.subgraphs.len().checked_add(1).ok_or_else(|| {
                            resources
                                .policy()
                                .overflow(AsciiResourceLimitId::MaxNestingDepth)
                        })?;
                    resources.check_nesting_depth(cycle_rejection_depth)?;
                    return Err(AsciiError::UnsupportedFeature {
                        diagram_type: "flowchart",
                        feature: "cyclic subgraph membership",
                    });
                }
                NestingVisitState::Unvisited => {}
            }

            states[current_index] = NestingVisitState::Visiting;
            path.push(current_index);
            let minimum_depth = path.len();
            resources.check_nesting_depth(minimum_depth)?;

            let current_id = model.subgraphs[current_index].id.as_str();
            let Some(parent_index) = memberships.parent_group_index(current_id) else {
                base_depth = 0;
                break;
            };
            current_index = parent_index;
        }

        while let Some(group_index) = path.pop() {
            base_depth = base_depth.checked_add(1).ok_or_else(|| {
                resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxNestingDepth)
            })?;
            resources.check_nesting_depth(base_depth)?;
            depths[group_index] = base_depth;
            states[group_index] = NestingVisitState::Complete;
        }
    }
    Ok(())
}

#[derive(Debug)]
struct FlowchartProjectionPlan<'a> {
    nodes: Vec<Option<FlowchartNodeProjectionPlan<'a>>>,
    edge_labels: Vec<Option<NormalizedTrimmedTextPlan<'a>>>,
    groups: Vec<FlowchartGroupProjectionPlan<'a>>,
}

#[derive(Debug, Clone, Copy)]
struct FlowchartNodeProjectionPlan<'a> {
    label: &'a str,
    shape: ResolvedGraphNodeShape,
}

#[derive(Debug, Clone, Copy)]
struct FlowchartGroupProjectionPlan<'a> {
    canonical_index: usize,
    title: NormalizedTextPlan<'a>,
    direction: Option<GraphDirection>,
}

#[derive(Debug, Default)]
struct ProjectionMaterializationAdmission {
    work_units: usize,
    document_cells: usize,
    output_bytes: usize,
}

impl ProjectionMaterializationAdmission {
    fn include_copy(&mut self, value: &str, resources: &ResourceContext) -> Result<()> {
        self.work_units = resources.checked_work_add(self.work_units, value.len())?;
        Ok(())
    }

    fn include_visible_text(
        &mut self,
        plan: NormalizedTextPlan<'_>,
        resources: &ResourceContext,
    ) -> Result<()> {
        self.work_units =
            resources.checked_work_add(self.work_units, plan.materialization_work_units())?;
        let metrics = plan.metrics();
        self.document_cells = checked_projection_metric_add(
            resources,
            AsciiResourceLimitId::MaxDocumentCells,
            self.document_cells,
            metrics.document_cells,
        )?;
        self.output_bytes = checked_projection_metric_add(
            resources,
            AsciiResourceLimitId::MaxOutputBytes,
            self.output_bytes,
            metrics.materialized_bytes,
        )?;
        Ok(())
    }

    fn admit(self, resources: &ResourceContext) -> Result<()> {
        resources.check_usage(self.work_units, self.document_cells)?;
        resources.check(AsciiResourceLimitId::MaxOutputBytes, self.output_bytes)?;
        resources.charge_usage(self.work_units, self.document_cells)
    }
}

impl<'a> FlowchartProjectionPlan<'a> {
    fn try_new(
        model: &'a FlowchartModel,
        memberships: &FlowchartMembershipIndex<'_>,
        direction: GraphDirection,
        width_profile: TerminalWidthProfile,
        resources: &ResourceContext,
        execution: AsciiExecution<'_>,
    ) -> Result<Self> {
        let plan_entries = resources.checked_work_add(
            resources.checked_work_add(model.nodes.len(), model.edges.len())?,
            memberships.canonical_group_indices().len(),
        )?;
        resources.charge_layout_work(plan_entries)?;

        let mut admission = ProjectionMaterializationAdmission::default();
        let mut nodes = Vec::new();
        nodes
            .try_reserve_exact(model.nodes.len())
            .map_err(|_| projection_allocation_failed())?;
        for (index, node) in model.nodes.iter().enumerate() {
            checkpoint_projection(execution, index)?;
            if memberships.is_group_id(&node.id) {
                nodes.push(None);
                continue;
            }
            let shape = resolve_flowchart_node_shape(node.layout_shape.as_deref(), direction)?;
            let projected_label = shape.projected_label(node.label.as_deref().unwrap_or(&node.id));
            // Validate terminal text before allocating the projection, but retain the authored
            // text so the graph label planner remains the sole owner of normalization and
            // wrapping. Materializing the visible escape here would erase its atomic segment
            // boundary and allow a later wrap pass to split `\u{HEX}` across rows.
            let _label = try_plan_normalized_text(projected_label, width_profile, resources)?;
            admission.include_copy(&node.id, resources)?;
            admission.include_copy(projected_label, resources)?;
            nodes.push(Some(FlowchartNodeProjectionPlan {
                label: projected_label,
                shape,
            }));
        }

        let mut edge_labels = Vec::new();
        edge_labels
            .try_reserve_exact(model.edges.len())
            .map_err(|_| projection_allocation_failed())?;
        for (index, edge) in model.edges.iter().enumerate() {
            checkpoint_projection(execution, index)?;
            admission.include_copy(&edge.from, resources)?;
            admission.include_copy(&edge.to, resources)?;
            admission.include_copy(&edge.id, resources)?;
            let label_plan = match edge.label.as_deref() {
                Some(label) => try_plan_normalized_trimmed_text(label, width_profile, resources)?,
                None => None,
            };
            if let Some(label_plan) = label_plan {
                admission.include_visible_text(label_plan, resources)?;
            }
            edge_labels.push(label_plan);
        }

        let mut groups = Vec::new();
        groups
            .try_reserve_exact(memberships.canonical_group_indices().len())
            .map_err(|_| projection_allocation_failed())?;
        for (index, (canonical_index, canonical_members)) in
            memberships.canonical_groups().enumerate()
        {
            checkpoint_projection(execution, index)?;
            let subgraph = &model.subgraphs[canonical_index];
            admission.include_copy(&subgraph.id, resources)?;
            let title = try_plan_normalized_text(&subgraph.title, width_profile, resources)?;
            admission.include_visible_text(title, resources)?;
            for (member_index, member) in canonical_members.iter().enumerate() {
                checkpoint_projection(execution, member_index)?;
                admission.include_copy(member, resources)?;
            }
            let direction = subgraph
                .dir
                .as_deref()
                .filter(|_| subgraph.has_explicit_dir)
                .map(parse_direction)
                .transpose()?;
            groups.push(FlowchartGroupProjectionPlan {
                canonical_index,
                title,
                direction,
            });
        }

        admission.admit(resources)?;

        Ok(Self {
            nodes,
            edge_labels,
            groups,
        })
    }
}

fn checked_projection_metric_add(
    resources: &ResourceContext,
    limit: AsciiResourceLimitId,
    left: usize,
    right: usize,
) -> Result<usize> {
    left.checked_add(right)
        .ok_or_else(|| resources.policy().overflow(limit))
}

fn try_clone_projection_string(value: &str) -> Result<String> {
    let mut output = String::new();
    output
        .try_reserve_exact(value.len())
        .map_err(|_| projection_allocation_failed())?;
    output.push_str(value);
    Ok(output)
}

fn projection_allocation_failed() -> AsciiError {
    AsciiError::AllocationFailed {
        phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
    }
}

fn parse_direction(direction: &str) -> Result<GraphDirection> {
    match direction {
        "LR" => Ok(GraphDirection::LeftRight),
        "RL" => Ok(GraphDirection::RightLeft),
        "TB" | "TD" => Ok(GraphDirection::TopDown),
        "BT" => Ok(GraphDirection::BottomTop),
        _ => Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "unsupported graph directions",
        }),
    }
}

fn validate_supported_flowchart_model(
    model: &FlowchartModel,
    memberships: &FlowchartMembershipIndex<'_>,
    resources: &ResourceContext,
    execution: AsciiExecution<'_>,
) -> Result<()> {
    for (index, member_id) in memberships.member_ids().enumerate() {
        checkpoint_projection(execution, index)?;
        resources.charge_layout_work(1)?;
        if member_id.contains('\n') {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature: "subgraph member ids with line breaks",
            });
        }
    }

    let mut node_ids = HashSet::new();
    node_ids
        .try_reserve(model.nodes.len())
        .map_err(|_| projection_allocation_failed())?;
    for (index, node) in model.nodes.iter().enumerate() {
        checkpoint_projection(execution, index)?;
        resources.charge_layout_work(1)?;
        if !node_ids.insert(node.id.as_str()) {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature: "duplicate node ids",
            });
        }
    }
    for (index, edge) in model.edges.iter().enumerate() {
        checkpoint_projection(execution, index)?;
        resources.charge_layout_work(1)?;
        if (!node_ids.contains(edge.from.as_str()) && !memberships.is_group_id(&edge.from))
            || (!node_ids.contains(edge.to.as_str()) && !memberships.is_group_id(&edge.to))
        {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature: "edges with missing endpoint nodes",
            });
        }
    }

    for (index, member_id) in memberships.member_ids().enumerate() {
        checkpoint_projection(execution, index)?;
        resources.charge_layout_work(1)?;
        if !node_ids.contains(member_id) && !memberships.is_group_id(member_id) {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature: "subgraphs with missing member nodes",
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::AsciiResourcePolicy;
    use merman_core::diagrams::flowchart::{
        FlowEdge, FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility, FlowNode, FlowSubgraph,
    };
    use merman_core::resources::ResourceProfile;
    use merman_core::{OperationControl, OperationPhase};

    fn flow_node(id: &str) -> FlowNode {
        FlowNode {
            id: id.to_string(),
            label: Some(id.to_string()),
            label_type: None,
            layout_shape: None,
            shape: None,
            icon: None,
            form: None,
            pos: None,
            img: None,
            constraint: None,
            asset_width: None,
            asset_height: None,
            classes: Vec::new(),
            styles: Vec::new(),
            link: None,
            link_target: None,
            have_callback: false,
        }
    }

    fn model_with_edge(edge: FlowEdge) -> FlowchartModel {
        FlowchartModel {
            keyword: "flowchart".to_string(),
            acc_descr: None,
            acc_title: None,
            class_defs: Default::default(),
            direction: Some("LR".to_string()),
            edge_defaults: None,
            vertex_calls: Vec::new(),
            nodes: vec![flow_node("A"), flow_node("B")],
            edges: vec![edge],
            subgraphs: Vec::new(),
            tooltips: Default::default(),
            warning_facts: Vec::new(),
        }
    }

    fn flow_edge(from: &str, to: &str) -> FlowEdge {
        FlowEdge {
            id: format!("edge-{from}-{to}"),
            from: from.to_string(),
            to: to.to_string(),
            label: None,
            label_type: None,
            edge_type: Some("arrow_point".to_string()),
            arrow: "-->".to_string(),
            start_marker: FlowEdgeMarker::None,
            end_marker: FlowEdgeMarker::Point,
            is_user_defined_id: false,
            stroke: Some("normal".to_string()),
            stroke_kind: FlowEdgeStroke::Normal,
            visibility: FlowEdgeVisibility::Visible,
            interpolate: None,
            classes: Vec::new(),
            style: Vec::new(),
            animate: None,
            animation: None,
            length: 1,
        }
    }

    fn nested_subgraph_model(group_count: usize) -> FlowchartModel {
        let mut member = "node".to_string();
        let mut subgraphs = Vec::with_capacity(group_count);
        for index in 0..group_count {
            let id = format!("group-{index}");
            subgraphs.push(FlowSubgraph {
                id: id.clone(),
                title: format!("Group {index}"),
                dir: None,
                has_explicit_dir: false,
                label_type: None,
                classes: Vec::new(),
                styles: Vec::new(),
                nodes: vec![member],
            });
            member = id;
        }

        FlowchartModel {
            keyword: "flowchart".to_string(),
            acc_descr: None,
            acc_title: None,
            class_defs: Default::default(),
            direction: None,
            edge_defaults: None,
            vertex_calls: Vec::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
            subgraphs,
            tooltips: Default::default(),
            warning_facts: Vec::new(),
        }
    }

    #[test]
    fn flowchart_projection_admits_exact_and_rejects_n_minus_one_atomically() {
        const PRIOR_WORK: usize = 11;
        const PRIOR_DOCUMENT_CELLS: usize = 2;
        const EXPECTED_DOCUMENT_CELLS: usize = 16;
        const EXPECTED_OUTPUT_BYTES: usize = 16;
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut edge = flow_edge("A", "B");
        edge.label = Some("  边\u{7}  ".to_string());
        let mut model = model_with_edge(edge);
        model.nodes[0].label = Some("节点\u{1b}".to_string());
        model.nodes[1].label = Some("🧭".to_string());
        model.subgraphs.push(FlowSubgraph {
            id: "G".to_string(),
            title: "组\u{7}".to_string(),
            dir: None,
            has_explicit_dir: false,
            label_type: None,
            classes: Vec::new(),
            styles: Vec::new(),
            nodes: vec!["A".to_string()],
        });

        let assert_visible_text = |graph: &AsciiGraph| {
            assert_eq!(graph.nodes[0].label, "节点\u{1b}");
            assert_eq!(graph.nodes[1].label, "🧭");
            assert_eq!(graph.edges[0].label.as_deref(), Some("边\\u{7}"));
            assert_eq!(graph.groups[0].title, "组\\u{7}");
        };

        let mut measuring_resources = ResourceContext::new(unbounded);
        measuring_resources
            .charge_layout_work(PRIOR_WORK)
            .expect("prior layout work should fit the measuring policy");
        measuring_resources
            .charge_document_cells(PRIOR_DOCUMENT_CELLS)
            .expect("prior document cells should fit the measuring policy");
        let measured = from_flowchart_model(
            &model,
            &AsciiRenderOptions::default(),
            &mut measuring_resources,
        )
        .expect("unbounded projection measurement should succeed");
        assert_visible_text(&measured);
        let expected_layout_work = measuring_resources.layout_work_used();
        assert_eq!(
            measuring_resources.document_cells_used(),
            EXPECTED_DOCUMENT_CELLS
        );

        let exact_limits = [
            (
                AsciiResourceLimitId::MaxLayoutWorkUnits,
                expected_layout_work,
                "layout work",
            ),
            (
                AsciiResourceLimitId::MaxDocumentCells,
                EXPECTED_DOCUMENT_CELLS,
                "document cells",
            ),
            (
                AsciiResourceLimitId::MaxOutputBytes,
                EXPECTED_OUTPUT_BYTES,
                "output bytes",
            ),
        ];
        for &(limit, exact, description) in &exact_limits {
            let exact_policy = unbounded
                .with_limit(limit, exact)
                .expect("exact projection limit should be valid");
            let mut exact_resources = ResourceContext::new(exact_policy);
            exact_resources
                .charge_layout_work(PRIOR_WORK)
                .expect("prior layout work should fit the exact policy");
            exact_resources
                .charge_document_cells(PRIOR_DOCUMENT_CELLS)
                .expect("prior document cells should fit the exact policy");
            let graph =
                from_flowchart_model(&model, &AsciiRenderOptions::default(), &mut exact_resources)
                    .unwrap_or_else(|error| {
                        panic!("exact {description} limit should admit: {error:?}")
                    });
            assert_visible_text(&graph);
            assert_eq!(exact_resources.layout_work_used(), expected_layout_work);
            assert_eq!(
                exact_resources.document_cells_used(),
                EXPECTED_DOCUMENT_CELLS
            );
        }

        for &(limit, exact, description) in &exact_limits {
            let below_policy = unbounded
                .with_limit(limit, exact - 1)
                .expect("max-minus-one projection limit should be valid");
            let mut below_resources = ResourceContext::new(below_policy);
            below_resources
                .charge_layout_work(PRIOR_WORK)
                .expect("prior layout work should fit below the combined boundary");
            below_resources
                .charge_document_cells(PRIOR_DOCUMENT_CELLS)
                .expect("prior document cells should fit below the combined boundary");
            let error = match from_flowchart_model(
                &model,
                &AsciiRenderOptions::default(),
                &mut below_resources,
            ) {
                Ok(_) => panic!("max-minus-one {description} limit unexpectedly admitted"),
                Err(error) => error,
            };

            assert!(matches!(
                error,
                AsciiError::ResourceLimitExceeded(details)
                    if details.limit == limit
                        && details.max == exact - 1
                        && details.actual == exact
            ));
            assert_eq!(below_resources.layout_work_used(), PRIOR_WORK);
            assert_eq!(below_resources.document_cells_used(), PRIOR_DOCUMENT_CELLS);
        }
    }

    #[test]
    fn flowchart_label_plan_uses_the_shared_controlled_ledger() {
        let policy = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, 1)
            .expect("minimum layout-work limit should be valid");
        let mut resources = ResourceContext::new(policy);
        let control = OperationControl::new();
        control.cancel();
        let execution = AsciiExecution::new(&control, &policy);
        execution.rebind_resource_context(&mut resources, OperationPhase::Semantic);

        let error = try_plan_normalized_trimmed_text(
            "long edge label",
            TerminalWidthProfile::Unicode,
            &resources,
        )
        .expect_err("cancelled label planning must not report the competing work limit");

        assert!(matches!(
            error,
            AsciiError::Cancelled(cancelled)
                if cancelled.phase == OperationPhase::Semantic
        ));
        assert_eq!(resources.layout_work_used(), 0);
        assert_eq!(resources.document_cells_used(), 0);
    }

    #[test]
    fn membership_index_and_nesting_accept_exact_work_and_reject_max_minus_one() {
        const GROUP_COUNT: usize = 64;
        let model = nested_subgraph_model(GROUP_COUNT);
        let member_count = model
            .subgraphs
            .iter()
            .map(|group| group.nodes.len())
            .sum::<usize>();
        let exact_work = GROUP_COUNT * 3 + member_count;
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .expect("exact layout-work limit should be valid");
        let exact_resources = ResourceContext::new(exact_policy);
        let exact_control = OperationControl::new();
        let exact_execution = AsciiExecution::new(&exact_control, &exact_policy);
        let memberships =
            FlowchartMembershipIndex::try_new(&model, &exact_resources, exact_execution)
                .expect("exact membership construction work should pass");
        preflight_subgraph_nesting(&model, &memberships, &exact_resources, exact_execution)
            .expect("exact nesting traversal work should pass");
        assert_eq!(exact_resources.layout_work_used(), exact_work);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work - 1)
            .expect("max-minus-one layout-work limit should be valid");
        let below_resources = ResourceContext::new(below_policy);
        let below_control = OperationControl::new();
        let below_execution = AsciiExecution::new(&below_control, &below_policy);
        let memberships =
            FlowchartMembershipIndex::try_new(&model, &below_resources, below_execution)
                .expect("membership construction should fit below the combined work boundary");
        let error =
            preflight_subgraph_nesting(&model, &memberships, &below_resources, below_execution)
                .expect_err("max-minus-one work should fail before nesting-state allocation");
        let AsciiError::ResourceLimitExceeded(details) = error else {
            panic!("expected a layout-work resource error, got {error:?}");
        };
        assert_eq!(details.limit, AsciiResourceLimitId::MaxLayoutWorkUnits);
        assert_eq!(details.actual, exact_work);
        assert_eq!(details.max, exact_work - 1);
        assert_eq!(
            below_resources.layout_work_used(),
            GROUP_COUNT + member_count
        );
    }

    #[test]
    fn duplicate_flowchart_node_ids_are_rejected_before_graph_projection() {
        let mut model = model_with_edge(flow_edge("A", "B"));
        model.nodes.push(flow_node("A"));
        let options = AsciiRenderOptions::ascii();
        let mut resources = ResourceContext::new(AsciiResourcePolicy::default());

        resources
            .charge_usage(7, 3)
            .expect("the resource ledger should accept prior usage");

        let error = from_flowchart_model(&model, &options, &mut resources)
            .expect_err("duplicate node ids must not select different layout and route entries");
        assert!(matches!(
            error,
            AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature: "duplicate node ids"
            }
        ));
        assert_eq!(resources.layout_work_used(), 7);
        assert_eq!(resources.document_cells_used(), 3);
    }

    #[test]
    fn flowchart_projection_retains_edge_identity_provenance_and_marker_semantics() {
        let model = model_with_edge(FlowEdge {
            id: "edge-A-B".to_string(),
            from: "A".to_string(),
            to: "B".to_string(),
            label: None,
            label_type: None,
            edge_type: Some("arrow_cross".to_string()),
            arrow: "o--x".to_string(),
            start_marker: FlowEdgeMarker::Circle,
            end_marker: FlowEdgeMarker::Cross,
            is_user_defined_id: true,
            stroke: Some("invisible".to_string()),
            stroke_kind: FlowEdgeStroke::Normal,
            visibility: FlowEdgeVisibility::Invisible,
            interpolate: None,
            classes: Vec::new(),
            style: Vec::new(),
            animate: None,
            animation: None,
            length: 1,
        });
        let options = AsciiRenderOptions::ascii();
        let mut resources = ResourceContext::new(AsciiResourcePolicy::default());

        let graph = from_flowchart_model(&model, &options, &mut resources).unwrap();
        let edge = &graph.edges[0];

        assert_eq!(edge.id.as_deref(), Some("edge-A-B"));
        assert!(edge.is_user_defined_id);
        assert_eq!(edge.start_marker, GraphEdgeMarker::Circle);
        assert_eq!(edge.end_marker, GraphEdgeMarker::Cross);
        assert_eq!(edge.stroke, GraphEdgeStroke::Invisible);
    }

    #[test]
    fn flowchart_projection_assigns_colliding_endpoint_ids_to_subgraphs() {
        let model = FlowchartModel {
            keyword: "flowchart".to_string(),
            acc_descr: None,
            acc_title: None,
            class_defs: Default::default(),
            direction: Some("LR".to_string()),
            edge_defaults: None,
            vertex_calls: Vec::new(),
            nodes: vec![
                flow_node("A"),
                flow_node("TOP"),
                flow_node("member"),
                flow_node("B"),
            ],
            edges: vec![flow_edge("A", "TOP"), flow_edge("TOP", "B")],
            subgraphs: vec![FlowSubgraph {
                id: "TOP".to_string(),
                title: "Top Group".to_string(),
                dir: Some("TB".to_string()),
                has_explicit_dir: true,
                label_type: None,
                classes: Vec::new(),
                styles: Vec::new(),
                nodes: vec!["member".to_string()],
            }],
            tooltips: Default::default(),
            warning_facts: Vec::new(),
        };
        let options = AsciiRenderOptions::ascii();
        let mut resources = ResourceContext::new(AsciiResourcePolicy::default());

        let graph = from_flowchart_model(&model, &options, &mut resources)
            .expect("subgraph endpoint projection should remain supported");

        assert!(graph.nodes.iter().all(|node| node.id != "TOP"));
        assert_eq!(graph.groups.len(), 1);
        assert_eq!(graph.groups[0].id, "TOP");
        assert_eq!(graph.edges[0].to, "TOP");
        assert_eq!(graph.edges[1].from, "TOP");
    }
}
