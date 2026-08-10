use crate::error::{AsciiError, Result};
use crate::graph::style::{
    apply_group_declaration, apply_group_declarations, apply_node_declaration,
    apply_node_declarations,
};
use crate::graph::{
    AsciiGraph, GraphDirection, GraphEdgeAttrs, GraphEdgeMarker, GraphGroupKind, GraphGroupStyle,
    GraphNodeCompartments, GraphNodeSemantics, GraphNodeShape, GraphNodeSide,
    GraphNodeSideConstraint, GraphNodeStyle,
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

struct StateDirectionProjection {
    node_by_id: HashMap<String, GraphDirection>,
    group_by_id: HashMap<String, GraphDirection>,
}

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
    let note_side_constraints = note_side_constraints(model, &note_node_parent_by_id)?;
    let direction = parse_state_direction(&model.direction)?;
    let state_directions = state_direction_projection(model, direction)?;
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
        let (label, compartments) = state_node_text(node)?;
        graph.add_node_with_semantics(
            &node.id,
            label,
            state_node_shape(
                node,
                state_directions
                    .node_by_id
                    .get(node.id.as_str())
                    .copied()
                    .unwrap_or_else(|| direction.canonical()),
            )?,
            state_node_style(node),
            GraphNodeSemantics {
                compartments,
                side_constraint: note_side_constraints.get(&node.id).cloned(),
            },
        );
    }

    for node in sorted_group_nodes(model, &group_members, resources)? {
        let members = group_members.get(&node.id).cloned().unwrap_or_default();
        graph.add_group_with_kind_and_style(
            &node.id,
            state_group_title(node),
            state_directions.group_by_id.get(node.id.as_str()).copied(),
            members,
            state_group_kind(node),
            state_group_style(node),
        );
    }

    for edge in &model.edges {
        let mut from = remap_note_endpoint(&edge.start, &note_node_parent_by_id);
        let mut to = remap_note_endpoint(&edge.end, &note_node_parent_by_id);
        if is_note_edge(edge) {
            (from, to) =
                canonical_note_edge_endpoints(from, to, &note_side_constraints, direction)?;
        }
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
        if let Some(position) = node.position.as_deref() {
            charge_text_layout(resources, position)?;
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
    let mut node_ids = HashSet::new();
    node_ids
        .try_reserve(model.nodes.len())
        .map_err(|_| projection_allocation_failed())?;
    for node in &model.nodes {
        if !node_ids.insert(node.id.as_str()) {
            return Err(unsupported("duplicate node ids"));
        }
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

fn note_side_constraints(
    model: &StateDiagramRenderModel,
    note_node_parent_by_id: &HashMap<String, String>,
) -> Result<HashMap<String, GraphNodeSideConstraint>> {
    let mut note_group_by_id = HashMap::<&str, &StateDiagramRenderNode>::new();
    note_group_by_id
        .try_reserve(model.nodes.len())
        .map_err(|_| projection_allocation_failed())?;
    for node in &model.nodes {
        if is_state_note_group(node) {
            note_group_by_id.insert(node.id.as_str(), node);
        }
    }

    let mut constraints = HashMap::new();
    constraints
        .try_reserve(note_group_by_id.len())
        .map_err(|_| projection_allocation_failed())?;
    for edge in &model.edges {
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
                note_group.id.clone(),
                GraphNodeSideConstraint::new(anchor_id, side),
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
    constraints: &HashMap<String, GraphNodeSideConstraint>,
    direction: GraphDirection,
) -> Result<(&'a str, &'a str)> {
    let (note_id, anchor_id, constraint) = match (constraints.get(from), constraints.get(to)) {
        (Some(constraint), None) => (from, to, constraint),
        (None, Some(constraint)) => (to, from, constraint),
        _ => return Err(unsupported("state note edge ownership")),
    };
    if constraint.anchor_id() != anchor_id {
        return Err(unsupported("state note edge ownership"));
    }
    let side = if direction == GraphDirection::RightLeft {
        constraint.side().reversed()
    } else {
        constraint.side()
    };
    Ok(match side {
        GraphNodeSide::Left => (note_id, anchor_id),
        GraphNodeSide::Right => (anchor_id, note_id),
    })
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

fn state_direction_projection(
    model: &StateDiagramRenderModel,
    root_direction: GraphDirection,
) -> Result<StateDirectionProjection> {
    let mut node_by_id = HashMap::<&str, &StateDiagramRenderNode>::new();
    node_by_id
        .try_reserve(model.nodes.len())
        .map_err(|_| projection_allocation_failed())?;
    for node in &model.nodes {
        node_by_id.insert(node.id.as_str(), node);
    }

    let mut node_directions = HashMap::new();
    node_directions
        .try_reserve(model.nodes.len())
        .map_err(|_| projection_allocation_failed())?;
    let mut group_directions = HashMap::new();
    group_directions
        .try_reserve(model.nodes.len())
        .map_err(|_| projection_allocation_failed())?;
    for node in &model.nodes {
        let inherited = nearest_explicit_ancestor_direction(node, &node_by_id, model.nodes.len())?;
        node_directions.insert(node.id.clone(), inherited.unwrap_or(root_direction));

        let group_direction = if node.explicit_dir == Some(true) {
            Some(parse_state_direction(node.dir.as_deref().ok_or_else(
                || unsupported("state explicit direction without value"),
            )?)?)
        } else {
            inherited
        };
        if let Some(group_direction) = group_direction {
            group_directions.insert(node.id.clone(), group_direction);
        }
    }
    Ok(StateDirectionProjection {
        node_by_id: node_directions,
        group_by_id: group_directions,
    })
}

fn nearest_explicit_ancestor_direction(
    node: &StateDiagramRenderNode,
    node_by_id: &HashMap<&str, &StateDiagramRenderNode>,
    node_count: usize,
) -> Result<Option<GraphDirection>> {
    let mut parent_id = node.parent_id.as_deref();
    let mut visited = HashSet::new();
    visited
        .try_reserve(node_count)
        .map_err(|_| projection_allocation_failed())?;
    while let Some(parent) = parent_id {
        if !visited.insert(parent) {
            break;
        }
        let Some(parent_node) = node_by_id.get(parent).copied() else {
            break;
        };
        if parent_node.explicit_dir == Some(true) {
            return parent_node
                .dir
                .as_deref()
                .map(parse_state_direction)
                .transpose();
        }
        parent_id = parent_node.parent_id.as_deref();
    }
    Ok(None)
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

fn state_node_text(
    node: &StateDiagramRenderNode,
) -> Result<(String, Option<GraphNodeCompartments>)> {
    if node.shape != "rectWithTitle" {
        return Ok((state_node_label(node), None));
    }

    let title = state_node_title(node);
    let mut body_lines = Vec::new();
    if let Some(description) = node.description.as_ref() {
        for line in description {
            push_nonempty_label_line(&mut body_lines, line);
        }
    }
    if body_lines.is_empty() {
        return Err(unsupported("state title/body compartments without body"));
    }
    let body = body_lines.join("\n");
    Ok((String::new(), Some(GraphNodeCompartments::new(title, body))))
}

fn state_node_title(node: &StateDiagramRenderNode) -> String {
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
        assert_projection_accepts_exact_work(&model);
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

    fn assert_projection_accepts_exact_work(model: &StateDiagramRenderModel) {
        let unbounded = AsciiResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        let mut measured = ResourceContext::new(unbounded);
        from_state_model_with_context(model, &mut measured)
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
