use crate::error::{AsciiError, Result};
use crate::graph::style::{
    apply_group_declaration, apply_group_declarations, apply_node_declaration,
    apply_node_declarations,
};
use crate::graph::{
    AsciiGraph, GraphDirection, GraphEdgeAttrs, GraphEdgeMarker, GraphGroupKind, GraphGroupStyle,
    GraphNodeShape, GraphNodeStyle,
};
#[cfg(test)]
use crate::resource::AsciiResourcePolicy;
use crate::resource::{AsciiResourceLimitId, AsciiResourceLimitPhase, ResourceContext};
use crate::safe_text::charge_text_layout;
use crate::text::normalize_optional_text;
use merman_core::diagrams::state::{
    StateDiagramRenderEdge, StateDiagramRenderModel, StateDiagramRenderNode,
};
use std::collections::{HashMap, HashSet};

const STATE_DIAGRAM_TYPE: &str = "state";
const STATE_NODE_PROJECTION_WORK_UNITS: usize = 10;
const STATE_EDGE_PROJECTION_WORK_UNITS: usize = 2;

#[cfg(test)]
pub(crate) fn from_state_model_with_resources(
    model: &StateDiagramRenderModel,
    policy: AsciiResourcePolicy,
) -> Result<AsciiGraph> {
    let mut resources = ResourceContext::new(policy);
    from_state_model_with_context(model, &mut resources)
}

pub(crate) fn from_state_model_with_context(
    model: &StateDiagramRenderModel,
    resources: &mut ResourceContext,
) -> Result<AsciiGraph> {
    preflight_state_projection_text(model, resources)?;
    let projection_work = state_projection_work(model, resources)?;
    resources.charge_layout_work(projection_work)?;
    validate_supported_state_model(model)?;

    let group_members = group_members_by_id(model)?;
    let note_node_parent_by_id = note_node_parent_by_id(model)?;
    let direction = parse_state_direction(&model.direction)?;
    let state_node_direction_by_id = state_node_direction_by_id(model, direction)?;
    let mut graph = AsciiGraph::new_for_diagram(STATE_DIAGRAM_TYPE, direction);
    graph.try_reserve_projection(model.nodes.len(), model.edges.len(), model.nodes.len())?;
    graph.use_incoming_edge_roots();

    for node in &model.nodes {
        if is_group_container(node, &group_members) {
            continue;
        }
        if is_state_note_node(node) {
            continue;
        }
        graph.add_node_with_shape_and_style(
            &node.id,
            state_node_label(node),
            state_node_shape(
                node,
                state_node_direction_by_id
                    .get(node.id.as_str())
                    .copied()
                    .unwrap_or_else(|| direction.canonical()),
            )?,
            state_node_style(node),
        );
    }

    for node in sorted_group_nodes(model, &group_members, resources)? {
        let members = group_members.get(&node.id).cloned().unwrap_or_default();
        graph.add_group_with_kind_and_style(
            &node.id,
            state_group_title(node),
            node.dir
                .as_deref()
                .filter(|_| node.explicit_dir == Some(true))
                .map(parse_state_direction)
                .transpose()?,
            members,
            state_group_kind(node),
            state_group_style(node),
        );
    }

    for edge in &model.edges {
        let from = remap_note_endpoint(&edge.start, &note_node_parent_by_id);
        let to = remap_note_endpoint(&edge.end, &note_node_parent_by_id);
        graph.add_edge_with_attrs(
            from,
            to,
            GraphEdgeAttrs {
                label: edge_label(&edge.label),
                end_marker: edge_marker(edge),
                ..GraphEdgeAttrs::default()
            },
        );
    }

    Ok(graph)
}

fn preflight_state_projection_text(
    model: &StateDiagramRenderModel,
    resources: &mut ResourceContext,
) -> Result<()> {
    charge_text_layout(resources, &model.direction)?;
    for node in &model.nodes {
        charge_text_layout(resources, &node.id)?;
        if let Some(parent_id) = node.parent_id.as_deref() {
            charge_text_layout(resources, parent_id)?;
        }
        if let Some(label) = node.label.as_ref() {
            if let Some(label) = label.as_str() {
                charge_text_layout(resources, label)?;
            } else if let Some(items) = label.as_array() {
                for item in items {
                    if let Some(line) = item.as_str() {
                        charge_text_layout(resources, line)?;
                    }
                }
            }
        }
        if let Some(description) = node.description.as_ref() {
            for line in description {
                charge_text_layout(resources, line)?;
            }
        }
    }
    for edge in &model.edges {
        charge_text_layout(resources, &edge.start)?;
        charge_text_layout(resources, &edge.end)?;
        charge_text_layout(resources, &edge.label)?;
    }
    Ok(())
}

fn state_projection_work(
    model: &StateDiagramRenderModel,
    resources: &ResourceContext,
) -> Result<usize> {
    let mut authored_items = 0usize;
    for node in &model.nodes {
        let label_items = node
            .label
            .as_ref()
            .map(|label| label.as_array().map_or(1, |items| items.len()))
            .unwrap_or_default();
        let node_items = label_items
            .checked_add(node.description.as_ref().map_or(0, |items| items.len()))
            .and_then(|items| items.checked_add(node.css_compiled_styles.len()))
            .and_then(|items| items.checked_add(node.css_styles.len()))
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
    let node_pairs = model
        .nodes
        .len()
        .checked_mul(model.nodes.len())
        .ok_or_else(|| {
            resources
                .policy()
                .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
        })?;
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
    node_pairs
        .checked_add(node_containers)
        .and_then(|work| work.checked_add(edge_containers))
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

fn validate_supported_state_model(model: &StateDiagramRenderModel) -> Result<()> {
    for node in &model.nodes {
        validate_supported_state_node(node)?;
    }

    for edge in &model.edges {
        if !edge.arrow_type_end.is_empty()
            && !matches!(
                edge.arrow_type_end.as_str(),
                "arrow_barb" | "arrow_barb_neo"
            )
        {
            return Err(unsupported("state arrow types"));
        }
    }

    Ok(())
}

fn validate_supported_state_node(node: &StateDiagramRenderNode) -> Result<()> {
    if is_state_note_node(node) || is_state_note_group(node) {
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

fn group_members_by_id(model: &StateDiagramRenderModel) -> Result<HashMap<String, Vec<String>>> {
    let mut members = HashMap::<String, Vec<String>>::new();
    members
        .try_reserve(model.nodes.len())
        .map_err(|_| projection_allocation_failed())?;
    for node in &model.nodes {
        let Some(parent_id) = node.parent_id.as_ref() else {
            continue;
        };
        let group_members = members.entry(parent_id.clone()).or_default();
        group_members
            .try_reserve(1)
            .map_err(|_| projection_allocation_failed())?;
        group_members.push(node.id.clone());
    }
    Ok(members)
}

fn note_node_parent_by_id(model: &StateDiagramRenderModel) -> Result<HashMap<String, String>> {
    let mut parents = HashMap::new();
    parents
        .try_reserve(model.nodes.len())
        .map_err(|_| projection_allocation_failed())?;
    for node in &model.nodes {
        if !is_state_note_node(node) {
            continue;
        }
        let Some(parent_id) = node.parent_id.as_ref() else {
            continue;
        };
        parents.insert(node.id.clone(), parent_id.clone());
    }
    Ok(parents)
}

fn sorted_group_nodes<'a>(
    model: &'a StateDiagramRenderModel,
    group_members: &HashMap<String, Vec<String>>,
    resources: &ResourceContext,
) -> Result<Vec<&'a StateDiagramRenderNode>> {
    let mut parent_by_id = HashMap::new();
    parent_by_id
        .try_reserve(model.nodes.len())
        .map_err(|_| projection_allocation_failed())?;
    for node in &model.nodes {
        parent_by_id.insert(node.id.as_str(), node.parent_id.as_deref());
    }

    let mut depth_by_id = HashMap::new();
    depth_by_id
        .try_reserve(model.nodes.len())
        .map_err(|_| projection_allocation_failed())?;
    for node in &model.nodes {
        depth_by_id.insert(
            node.id.as_str(),
            node_depth(node, &parent_by_id, model.nodes.len(), resources)?,
        );
    }

    let mut groups = Vec::new();
    groups
        .try_reserve(model.nodes.len())
        .map_err(|_| projection_allocation_failed())?;
    groups.extend(
        model
            .nodes
            .iter()
            .filter(|node| is_group_container(node, group_members)),
    );
    groups.sort_by_key(|node| {
        std::cmp::Reverse(
            depth_by_id
                .get(node.id.as_str())
                .copied()
                .unwrap_or_default(),
        )
    });
    Ok(groups)
}

fn node_depth(
    node: &StateDiagramRenderNode,
    parent_by_id: &HashMap<&str, Option<&str>>,
    node_count: usize,
    resources: &ResourceContext,
) -> Result<usize> {
    let mut depth = 0usize;
    let mut seen = HashSet::new();
    seen.try_reserve(node_count)
        .map_err(|_| projection_allocation_failed())?;
    let mut parent = node.parent_id.as_deref();
    resources.check_nesting_depth(1)?;

    while let Some(parent_id) = parent {
        if !seen.insert(parent_id) {
            break;
        }
        depth = depth.checked_add(1).ok_or_else(|| {
            resources
                .policy()
                .overflow(AsciiResourceLimitId::MaxNestingDepth)
        })?;
        let nesting_depth = depth.checked_add(1).ok_or_else(|| {
            resources
                .policy()
                .overflow(AsciiResourceLimitId::MaxNestingDepth)
        })?;
        resources.check_nesting_depth(nesting_depth)?;
        parent = parent_by_id.get(parent_id).copied().flatten();
    }

    Ok(depth)
}

fn state_node_direction_by_id(
    model: &StateDiagramRenderModel,
    root_direction: GraphDirection,
) -> Result<HashMap<String, GraphDirection>> {
    let mut node_by_id = HashMap::<&str, &StateDiagramRenderNode>::new();
    node_by_id
        .try_reserve(model.nodes.len())
        .map_err(|_| projection_allocation_failed())?;
    for node in &model.nodes {
        node_by_id.insert(node.id.as_str(), node);
    }

    let mut directions = HashMap::new();
    directions
        .try_reserve(model.nodes.len())
        .map_err(|_| projection_allocation_failed())?;
    for node in &model.nodes {
        let mut direction = root_direction;
        let mut parent_id = node.parent_id.as_deref();
        let mut visited = HashSet::new();
        visited
            .try_reserve(model.nodes.len())
            .map_err(|_| projection_allocation_failed())?;
        while let Some(parent) = parent_id {
            if !visited.insert(parent) {
                break;
            }
            let Some(parent_node) = node_by_id.get(parent).copied() else {
                break;
            };
            if parent_node.explicit_dir == Some(true) {
                if let Some(parent_direction) = parent_node.dir.as_deref() {
                    direction = parse_state_direction(parent_direction)?;
                }
                break;
            }
            parent_id = parent_node.parent_id.as_deref();
        }
        directions.insert(node.id.clone(), direction);
    }
    Ok(directions)
}

fn is_group_container(
    node: &StateDiagramRenderNode,
    group_members: &HashMap<String, Vec<String>>,
) -> bool {
    if is_state_note_group(node) {
        return false;
    }
    node.is_group
        && group_members
            .get(&node.id)
            .is_some_and(|members| !members.is_empty())
}

fn state_node_shape(
    node: &StateDiagramRenderNode,
    direction: GraphDirection,
) -> Result<GraphNodeShape> {
    match node.shape.as_str() {
        "rect" | "rectWithTitle" => Ok(GraphNodeShape::Rect),
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

fn state_node_style(node: &StateDiagramRenderNode) -> GraphNodeStyle {
    let mut style = GraphNodeStyle::default();
    apply_node_declarations(&mut style, &node.css_compiled_styles);
    apply_node_declarations(&mut style, &node.css_styles);
    apply_node_declaration(&mut style, &node.label_style);
    style
}

fn state_group_style(node: &StateDiagramRenderNode) -> GraphGroupStyle {
    let mut style = GraphGroupStyle::default();
    apply_group_declarations(&mut style, &node.css_compiled_styles);
    apply_group_declarations(&mut style, &node.css_styles);
    apply_group_declaration(&mut style, &node.label_style);
    style
}

fn state_group_kind(node: &StateDiagramRenderNode) -> GraphGroupKind {
    if is_state_divider_group(node) {
        GraphGroupKind::Divider
    } else {
        GraphGroupKind::Container
    }
}

fn state_group_title(node: &StateDiagramRenderNode) -> String {
    if is_state_divider_group(node) {
        String::new()
    } else {
        state_node_label(node)
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

fn state_node_label(node: &StateDiagramRenderNode) -> String {
    if is_state_pseudo_shape(node.shape.as_str()) {
        return String::new();
    }

    let mut lines = Vec::new();
    if let Some(label) = node.label.as_ref() {
        if let Some(label) = label.as_str() {
            push_nonempty_label_line(&mut lines, label);
        } else if let Some(items) = label.as_array() {
            for item in items {
                if let Some(line) = item.as_str() {
                    push_nonempty_label_line(&mut lines, line);
                }
            }
        }
    }
    if let Some(description) = node.description.as_ref() {
        for line in description {
            push_nonempty_label_line(&mut lines, line);
        }
    }

    if lines.is_empty() {
        node.id.clone()
    } else {
        lines.join("\n")
    }
}

fn is_state_pseudo_shape(shape: &str) -> bool {
    matches!(
        shape,
        "stateStart" | "stateEnd" | "fork" | "join" | "choice"
    )
}

fn push_nonempty_label_line(lines: &mut Vec<String>, line: &str) {
    if let Some(line) = normalize_optional_text(Some(line)) {
        lines.push(line);
    }
}

fn edge_label(label: &str) -> Option<String> {
    normalize_optional_text(Some(label))
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
    note_node_parent_by_id: &'a HashMap<String, String>,
) -> &'a str {
    note_node_parent_by_id
        .get(endpoint)
        .map(String::as_str)
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

    #[test]
    fn state_projection_accepts_exact_work_and_rejects_max_minus_one() {
        let model = StateDiagramRenderModel {
            direction: "TB".to_string(),
            nodes: vec![state_node("a")],
            ..StateDiagramRenderModel::default()
        };
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut measured = ResourceContext::new(unbounded);
        from_state_model_with_context(&model, &mut measured)
            .expect("unbounded state projection should succeed");
        let exact = measured.layout_work_used();
        assert!(exact > 1);

        let exact_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact)
            .expect("exact state projection work limit should be valid");
        from_state_model_with_resources(&model, exact_policy)
            .expect("exact state projection work limit should pass");

        let below_policy = unbounded
            .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, exact - 1)
            .expect("max-minus-one state projection work limit should be valid");
        let error = from_state_model_with_resources(&model, below_policy)
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
