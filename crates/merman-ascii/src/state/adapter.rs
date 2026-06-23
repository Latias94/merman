use crate::error::{AsciiError, Result};
use crate::graph::style::{
    apply_group_declaration, apply_group_declarations, apply_node_declaration,
    apply_node_declarations,
};
use crate::graph::{
    AsciiGraph, GraphDirection, GraphEdgeArrow, GraphEdgeAttrs, GraphGroupKind, GraphGroupStyle,
    GraphNodeShape, GraphNodeStyle,
};
use merman_core::diagrams::state::{
    StateDiagramRenderEdge, StateDiagramRenderModel, StateDiagramRenderNode,
};
use std::collections::{HashMap, HashSet};

const STATE_DIAGRAM_TYPE: &str = "state";

pub(crate) fn from_state_model(model: &StateDiagramRenderModel) -> Result<AsciiGraph> {
    validate_supported_state_model(model)?;

    let group_members = group_members_by_id(model);
    let note_node_parent_by_id = note_node_parent_by_id(model);
    let direction = parse_state_direction(&model.direction)?;
    let state_node_direction_by_id = state_node_direction_by_id(model, direction)?;
    let mut graph = AsciiGraph::new_for_diagram(STATE_DIAGRAM_TYPE, direction);
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

    for node in sorted_group_nodes(model, &group_members) {
        let members = group_members.get(&node.id).cloned().unwrap_or_default();
        graph.add_group_with_kind_and_style(
            &node.id,
            state_group_title(node),
            node.dir
                .as_deref()
                .map(parse_state_direction)
                .transpose()?
                .map(GraphDirection::canonical),
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
                arrow: edge_arrow(edge),
                ..GraphEdgeAttrs::default()
            },
        );
    }

    Ok(graph)
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

fn group_members_by_id(model: &StateDiagramRenderModel) -> HashMap<String, Vec<String>> {
    let mut members = HashMap::<String, Vec<String>>::new();
    for node in &model.nodes {
        let Some(parent_id) = node.parent_id.as_ref() else {
            continue;
        };
        members
            .entry(parent_id.clone())
            .or_default()
            .push(node.id.clone());
    }
    members
}

fn note_node_parent_by_id(model: &StateDiagramRenderModel) -> HashMap<String, String> {
    model
        .nodes
        .iter()
        .filter(|node| is_state_note_node(node))
        .filter_map(|node| {
            let parent_id = node.parent_id.as_ref()?;
            Some((node.id.clone(), parent_id.clone()))
        })
        .collect()
}

fn sorted_group_nodes<'a>(
    model: &'a StateDiagramRenderModel,
    group_members: &HashMap<String, Vec<String>>,
) -> Vec<&'a StateDiagramRenderNode> {
    let parent_by_id = model
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node.parent_id.as_deref()))
        .collect::<HashMap<_, _>>();
    let mut groups = model
        .nodes
        .iter()
        .filter(|node| is_group_container(node, group_members))
        .collect::<Vec<_>>();
    groups.sort_by_key(|node| std::cmp::Reverse(node_depth(node, &parent_by_id)));
    groups
}

fn node_depth(node: &StateDiagramRenderNode, parent_by_id: &HashMap<&str, Option<&str>>) -> usize {
    let mut depth = 0;
    let mut seen = HashSet::new();
    let mut parent = node.parent_id.as_deref();

    while let Some(parent_id) = parent {
        if !seen.insert(parent_id) {
            break;
        }
        depth += 1;
        parent = parent_by_id.get(parent_id).copied().flatten();
    }

    depth
}

fn state_node_direction_by_id(
    model: &StateDiagramRenderModel,
    fallback_direction: GraphDirection,
) -> Result<HashMap<String, GraphDirection>> {
    let mut group_direction_by_id = HashMap::<&str, GraphDirection>::new();
    for node in &model.nodes {
        let Some(direction) = node.dir.as_deref() else {
            continue;
        };
        group_direction_by_id.insert(
            node.id.as_str(),
            parse_state_direction(direction)?.canonical(),
        );
    }

    Ok(model
        .nodes
        .iter()
        .map(|node| {
            let direction = node
                .parent_id
                .as_deref()
                .and_then(|parent_id| group_direction_by_id.get(parent_id).copied())
                .unwrap_or_else(|| fallback_direction.canonical());
            (node.id.clone(), direction)
        })
        .collect())
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
    let line = line.trim();
    if !line.is_empty() {
        lines.push(line.to_string());
    }
}

fn edge_label(label: &str) -> Option<String> {
    let label = label.trim();
    (!label.is_empty()).then(|| label.to_string())
}

fn edge_arrow(edge: &StateDiagramRenderEdge) -> GraphEdgeArrow {
    if is_note_edge(edge) {
        GraphEdgeArrow::Open
    } else {
        GraphEdgeArrow::Point
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
