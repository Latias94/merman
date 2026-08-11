use super::model::{
    AsciiGraph, GraphDirection, GraphEdgeAttrs, GraphEdgeMarker, GraphEdgeStroke,
    GraphNodeSemantics,
};
use super::shape::resolve_flowchart_node_shape;
use super::style::{resolve_edge_style, resolve_group_style, resolve_node_style};
use crate::AsciiDirection;
use crate::error::{AsciiError, Result};
use crate::options::AsciiRenderOptions;
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use crate::safe_text::charge_text_layout;
use crate::text::normalize_optional_text;
use merman_core::diagrams::flowchart::{
    FlowEdgeMarker as CoreFlowEdgeMarker, FlowEdgeStroke as CoreFlowEdgeStroke,
    FlowEdgeVisibility as CoreFlowEdgeVisibility, FlowchartModel,
};
use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;

pub(crate) fn from_flowchart_model(
    model: &FlowchartModel,
    options: &AsciiRenderOptions,
    resources: &mut ResourceContext,
) -> Result<AsciiGraph> {
    debug_assert_eq!(resources.policy(), options.resources);
    let memberships = preflight_flowchart_projection(model, resources)?;
    validate_supported_flowchart_model(model, &memberships, resources)?;

    let direction = if let Some(direction) = model.direction.as_deref() {
        parse_direction(direction)?
    } else {
        match options.default_direction {
            AsciiDirection::LeftRight => GraphDirection::LeftRight,
            AsciiDirection::TopDown => GraphDirection::TopDown,
        }
    };
    let mut graph = AsciiGraph::new(direction);
    let wrap_width = NonZeroUsize::new(options.flowchart_node_label_wrap_width).ok_or(
        AsciiError::InvalidOption {
            field: "flowchart_node_label_wrap_width",
            message: "must be greater than 0",
        },
    )?;
    graph.wrap_node_labels_at(wrap_width);
    graph.try_reserve_projection(model.nodes.len(), model.edges.len(), model.subgraphs.len())?;

    for node in &model.nodes {
        if memberships.is_group_id(&node.id) {
            continue;
        }
        let resolved_shape = resolve_flowchart_node_shape(node.layout_shape.as_deref(), direction)?;
        let id = try_clone_projection_string(&node.id)?;
        let label = try_clone_projection_string(
            resolved_shape.projected_label(node.label.as_deref().unwrap_or(&node.id)),
        )?;
        graph.add_node_with_semantics(
            id,
            label,
            resolved_shape.shape,
            resolve_node_style(model, node),
            GraphNodeSemantics::default(),
        );
    }

    for edge in &model.edges {
        let from = try_clone_projection_string(&edge.from)?;
        let to = try_clone_projection_string(&edge.to)?;
        graph.add_edge_with_attrs(
            from,
            to,
            GraphEdgeAttrs {
                id: Some(try_clone_projection_string(&edge.id)?),
                is_user_defined_id: edge.is_user_defined_id,
                label: edge
                    .label
                    .as_deref()
                    .and_then(|label| normalize_optional_text(Some(label))),
                stroke: parse_flow_edge_stroke(edge.stroke_kind, edge.visibility),
                start_marker: parse_flow_edge_marker(edge.start_marker),
                end_marker: parse_flow_edge_marker(edge.end_marker),
                length: edge.length,
                style: resolve_edge_style(model, edge),
            },
        );
    }

    for subgraph in &model.subgraphs {
        let mut members = Vec::new();
        members
            .try_reserve_exact(subgraph.nodes.len())
            .map_err(|_| projection_allocation_failed())?;
        for member in &subgraph.nodes {
            members.push(try_clone_projection_string(member)?);
        }
        graph.add_group_with_style(
            try_clone_projection_string(&subgraph.id)?,
            try_clone_projection_string(&subgraph.title)?,
            subgraph
                .dir
                .as_deref()
                .filter(|_| subgraph.has_explicit_dir)
                .map(parse_direction)
                .transpose()?,
            members,
            resolve_group_style(model, subgraph),
        );
    }

    Ok(graph)
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
    resources: &mut ResourceContext,
) -> Result<FlowchartMembershipIndex<'a>> {
    resources.charge_layout_work(1)?;
    if let Some(direction) = model.direction.as_deref() {
        charge_text_layout(resources, direction)?;
    }

    for node in &model.nodes {
        resources.charge_layout_work(1)?;
        charge_text_layout(resources, &node.id)?;
        charge_text_layout(resources, node.label.as_deref().unwrap_or(&node.id))?;
        if let Some(shape) = node.layout_shape.as_deref() {
            charge_text_layout(resources, shape)?;
        }
        for declaration in node.classes.iter().chain(&node.styles) {
            resources.charge_layout_work(1)?;
            charge_text_layout(resources, declaration)?;
        }
    }

    for edge in &model.edges {
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
        if let Some(label) = edge.label.as_deref() {
            charge_text_layout(resources, label)?;
        }
        for declaration in edge.classes.iter().chain(&edge.style) {
            resources.charge_layout_work(1)?;
            charge_text_layout(resources, declaration)?;
        }
    }

    for subgraph in &model.subgraphs {
        resources.charge_layout_work(1)?;
        charge_text_layout(resources, &subgraph.id)?;
        charge_text_layout(resources, &subgraph.title)?;
        for member in &subgraph.nodes {
            resources.charge_layout_work(1)?;
            charge_text_layout(resources, member)?;
        }
        for declaration in subgraph.classes.iter().chain(&subgraph.styles) {
            resources.charge_layout_work(1)?;
            charge_text_layout(resources, declaration)?;
        }
    }

    for (class_name, declarations) in &model.class_defs {
        resources.charge_layout_work(1)?;
        charge_text_layout(resources, class_name)?;
        for declaration in declarations {
            resources.charge_layout_work(1)?;
            charge_text_layout(resources, declaration)?;
        }
    }

    let memberships = FlowchartMembershipIndex::try_new(model, resources)?;
    preflight_subgraph_nesting(model, &memberships, resources)?;
    Ok(memberships)
}

#[derive(Debug)]
struct FlowchartMembershipIndex<'a> {
    parent_group_by_member: HashMap<&'a str, usize>,
    group_ids: HashSet<&'a str>,
}

impl<'a> FlowchartMembershipIndex<'a> {
    fn try_new(model: &'a FlowchartModel, resources: &mut ResourceContext) -> Result<Self> {
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
        for (parent_index, subgraph) in model.subgraphs.iter().enumerate() {
            if !group_ids.insert(subgraph.id.as_str()) {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type: "flowchart",
                    feature: "duplicate subgraph ids",
                });
            }
            for member in &subgraph.nodes {
                // Preserve the former first-match parent semantics without rescanning candidates.
                parent_group_by_member
                    .entry(member.as_str())
                    .or_insert(parent_index);
            }
        }

        Ok(Self {
            parent_group_by_member,
            group_ids,
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
    resources: &mut ResourceContext,
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

    for start_index in 0..model.subgraphs.len() {
        if states[start_index] == NestingVisitState::Complete {
            continue;
        }
        path.clear();
        let mut current_index = start_index;
        let mut base_depth;
        loop {
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
    resources: &mut ResourceContext,
) -> Result<()> {
    for member_id in memberships.member_ids() {
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
    for node in &model.nodes {
        resources.charge_layout_work(1)?;
        if !node_ids.insert(node.id.as_str()) {
            return Err(AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature: "duplicate node ids",
            });
        }
    }
    for edge in &model.edges {
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

    for member_id in memberships.member_ids() {
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
        let mut exact_resources = ResourceContext::new(exact_policy);
        let memberships = FlowchartMembershipIndex::try_new(&model, &mut exact_resources)
            .expect("exact membership construction work should pass");
        preflight_subgraph_nesting(&model, &memberships, &mut exact_resources)
            .expect("exact nesting traversal work should pass");
        assert_eq!(exact_resources.layout_work_used(), exact_work);

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact_work - 1)
            .expect("max-minus-one layout-work limit should be valid");
        let mut below_resources = ResourceContext::new(below_policy);
        let memberships = FlowchartMembershipIndex::try_new(&model, &mut below_resources)
            .expect("membership construction should fit below the combined work boundary");
        let error = preflight_subgraph_nesting(&model, &memberships, &mut below_resources)
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
        let mut resources = ResourceContext::new(options.resources);

        let error = from_flowchart_model(&model, &options, &mut resources)
            .expect_err("duplicate node ids must not select different layout and route entries");
        assert!(matches!(
            error,
            AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature: "duplicate node ids"
            }
        ));
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
        let mut resources = ResourceContext::new(options.resources);

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
        let mut resources = ResourceContext::new(options.resources);

        let graph = from_flowchart_model(&model, &options, &mut resources)
            .expect("subgraph endpoint projection should remain supported");

        assert!(graph.nodes.iter().all(|node| node.id != "TOP"));
        assert_eq!(graph.groups.len(), 1);
        assert_eq!(graph.groups[0].id, "TOP");
        assert_eq!(graph.edges[0].to, "TOP");
        assert_eq!(graph.edges[1].from, "TOP");
    }
}
