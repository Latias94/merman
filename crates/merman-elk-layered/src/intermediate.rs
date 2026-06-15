//! Intermediate layered processors.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/ReversedEdgeRestorer.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/EdgeAndLayerConstraintEdgeReverser.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/LayerConstraintPreprocessor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/LayerConstraintPostprocessor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/LongEdgeSplitter.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/LongEdgeJoiner.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/LayerSizeAndGraphHeightCalculator.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/LabelDummyInserter.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/LabelDummySwitcher.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/LabelSideSelector.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/LabelDummyRemover.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/HierarchicalPortConstraintProcessor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/HierarchicalPortDummySizeProcessor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/HierarchicalPortPositionProcessor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/HierarchicalPortOrthogonalEdgeRouter.java

use std::collections::VecDeque;

use crate::graph::{
    EdgeLabelPlacement, HorizontalLabelAlignment, LGraph, LLabel, LNode, LNodeKind, LPoint, LSize,
    LabelCellLayout, LabelSide, LayeredEdge, PortRef, PortSide, PortType, VerticalLabelAlignment,
    reverse_edge,
};
use crate::options::{
    Alignment, EdgeLabelSideSelection, ElkDirection, LayerConstraint, PortConstraints,
};
use crate::p5edges::orthogonal::{RoutingDirection, route_edges};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IntermediateError {
    #[error(
        "node `{node_id}` has layer constraint FIRST_SEPARATE but has incoming edge `{edge_id}`"
    )]
    FirstSeparateIncomingEdge { node_id: String, edge_id: String },
    #[error(
        "node `{node_id}` has layer constraint LAST_SEPARATE but has outgoing edge `{edge_id}`"
    )]
    LastSeparateOutgoingEdge { node_id: String, edge_id: String },
    #[error("node `{node_id}` has layer constraint FIRST but has incoming edge `{edge_id}`")]
    FirstIncomingEdge { node_id: String, edge_id: String },
    #[error("node `{node_id}` has layer constraint LAST but has outgoing edge `{edge_id}`")]
    LastOutgoingEdge { node_id: String, edge_id: String },
}

pub type IntermediateResult<T> = Result<T, IntermediateError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EdgeReversalPortType {
    Input,
    Output,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HiddenNodeConnections {
    None,
    FirstSeparate,
    LastSeparate,
    Both,
}

impl HiddenNodeConnections {
    fn combine(self, layer_constraint: LayerConstraint) -> Self {
        match self {
            Self::None => {
                if layer_constraint == LayerConstraint::FirstSeparate {
                    Self::FirstSeparate
                } else {
                    Self::LastSeparate
                }
            }
            Self::FirstSeparate => {
                if layer_constraint == LayerConstraint::FirstSeparate {
                    Self::FirstSeparate
                } else {
                    Self::Both
                }
            }
            Self::LastSeparate => {
                if layer_constraint == LayerConstraint::FirstSeparate {
                    Self::Both
                } else {
                    Self::LastSeparate
                }
            }
            Self::Both => Self::Both,
        }
    }
}

pub fn restore_reversed_edges(graph: &mut LGraph) {
    for layer_index in 0..graph.layers.len() {
        let nodes = graph.layers[layer_index].nodes.clone();
        for node_index in nodes {
            let Some(node) = graph.layerless_nodes.get(node_index) else {
                continue;
            };
            let port_count = node.ports.len();

            for port_index in 0..port_count {
                let outgoing_edges = graph.layerless_nodes[node_index].ports[port_index]
                    .outgoing_edges
                    .clone();
                for edge_index in outgoing_edges {
                    if graph.edges[edge_index].reversed {
                        reverse_edge(graph, edge_index, false);
                    }
                }
            }
        }
    }
}

pub fn reverse_edges_for_edge_and_layer_constraints(graph: &mut LGraph) {
    let mut remaining_nodes = Vec::new();

    for node in 0..graph.layerless_nodes.len() {
        let layer_constraint = graph.layerless_nodes[node].layer_constraint;
        if let Some(target_port_type) = target_port_type_for_layer_constraint(layer_constraint) {
            reverse_node_edges(graph, node, layer_constraint, target_port_type);
        } else {
            remaining_nodes.push(node);
        }
    }

    for node in remaining_nodes {
        if should_reverse_all_fixed_side_edges(graph, node) {
            let layer_constraint = graph.layerless_nodes[node].layer_constraint;
            reverse_node_edges(graph, node, layer_constraint, EdgeReversalPortType::All);
        }
    }
}

fn target_port_type_for_layer_constraint(
    layer_constraint: LayerConstraint,
) -> Option<EdgeReversalPortType> {
    match layer_constraint {
        LayerConstraint::First | LayerConstraint::FirstSeparate => {
            Some(EdgeReversalPortType::Output)
        }
        LayerConstraint::Last | LayerConstraint::LastSeparate => Some(EdgeReversalPortType::Input),
        LayerConstraint::None => None,
    }
}

fn should_reverse_all_fixed_side_edges(graph: &LGraph, node: usize) -> bool {
    let lnode = &graph.layerless_nodes[node];
    if !lnode.port_constraints.is_side_fixed() || lnode.ports.is_empty() {
        return false;
    }

    for port in &lnode.ports {
        let reversed_port = match port.side {
            PortSide::East => port.net_flow() > 0,
            PortSide::West => port.net_flow() < 0,
            PortSide::North | PortSide::South | PortSide::Undefined => false,
        };
        if !reversed_port {
            return false;
        }

        for edge in &port.outgoing_edges {
            let target_layer_constraint =
                graph.layerless_nodes[graph.edges[*edge].target.node].layer_constraint;
            if matches!(
                target_layer_constraint,
                LayerConstraint::Last | LayerConstraint::LastSeparate
            ) {
                return false;
            }
        }
        for edge in &port.incoming_edges {
            let source_layer_constraint =
                graph.layerless_nodes[graph.edges[*edge].source.node].layer_constraint;
            if matches!(
                source_layer_constraint,
                LayerConstraint::First | LayerConstraint::FirstSeparate
            ) {
                return false;
            }
        }
    }

    true
}

fn reverse_node_edges(
    graph: &mut LGraph,
    node: usize,
    node_layer_constraint: LayerConstraint,
    target_port_type: EdgeReversalPortType,
) {
    let port_count = graph.layerless_nodes[node].ports.len();
    for port in 0..port_count {
        if matches!(
            target_port_type,
            EdgeReversalPortType::Input | EdgeReversalPortType::All
        ) {
            let outgoing_edges = graph.layerless_nodes[node].ports[port]
                .outgoing_edges
                .clone();
            for edge in outgoing_edges {
                if can_reverse_outgoing_edge(graph, node_layer_constraint, edge) {
                    reverse_edge(graph, edge, true);
                }
            }
        }

        if matches!(
            target_port_type,
            EdgeReversalPortType::Output | EdgeReversalPortType::All
        ) {
            let incoming_edges = graph.layerless_nodes[node].ports[port]
                .incoming_edges
                .clone();
            for edge in incoming_edges {
                if can_reverse_incoming_edge(graph, node_layer_constraint, edge) {
                    reverse_edge(graph, edge, true);
                }
            }
        }
    }
}

fn can_reverse_outgoing_edge(
    graph: &LGraph,
    source_node_layer_constraint: LayerConstraint,
    edge: usize,
) -> bool {
    let Some(edge) = graph.edges.get(edge) else {
        return false;
    };
    if edge.reversed {
        return false;
    }

    let target_node = edge.target.node;
    if source_node_layer_constraint == LayerConstraint::Last
        && graph.layerless_nodes[target_node].kind == LNodeKind::Label
    {
        return false;
    }

    graph.layerless_nodes[target_node].layer_constraint != LayerConstraint::LastSeparate
}

fn can_reverse_incoming_edge(
    graph: &LGraph,
    target_node_layer_constraint: LayerConstraint,
    edge: usize,
) -> bool {
    let Some(edge) = graph.edges.get(edge) else {
        return false;
    };
    if edge.reversed {
        return false;
    }

    let source_node = edge.source.node;
    if target_node_layer_constraint == LayerConstraint::First
        && graph.layerless_nodes[source_node].kind == LNodeKind::Label
    {
        return false;
    }

    graph.layerless_nodes[source_node].layer_constraint != LayerConstraint::FirstSeparate
}

pub fn preprocess_layer_constraints(graph: &mut LGraph) -> IntermediateResult<()> {
    graph.hidden_nodes.clear();
    let mut hidden_connections = vec![HiddenNodeConnections::None; graph.layerless_nodes.len()];
    let nodes = (0..graph.layerless_nodes.len()).collect::<Vec<_>>();

    for node in nodes {
        if is_relevant_layer_constraint_node(graph, node) {
            hide_layer_constraint_node(graph, node, &mut hidden_connections)?;
            graph.layerless_nodes[node].hidden = true;
            graph.layerless_nodes[node].layer_index = None;
            graph.hidden_nodes.push(node);
        }
    }

    Ok(())
}

fn is_relevant_layer_constraint_node(graph: &LGraph, node: usize) -> bool {
    matches!(
        graph.layerless_nodes[node].layer_constraint,
        LayerConstraint::FirstSeparate | LayerConstraint::LastSeparate
    )
}

fn hide_layer_constraint_node(
    graph: &mut LGraph,
    node: usize,
    hidden_connections: &mut [HiddenNodeConnections],
) -> IntermediateResult<()> {
    ensure_no_unacceptable_separate_edges(graph, node)?;
    let connected_edges = graph.node_connected_edges(node);

    for edge in connected_edges {
        hide_layer_constraint_edge(graph, node, edge, hidden_connections);
    }

    Ok(())
}

fn hide_layer_constraint_edge(
    graph: &mut LGraph,
    node: usize,
    edge: usize,
    hidden_connections: &mut [HiddenNodeConnections],
) {
    let is_outgoing = graph.edges[edge].source.node == node;
    let opposite_port = if is_outgoing {
        graph.edges[edge].target
    } else {
        graph.edges[edge].source
    };

    if is_outgoing {
        graph.detach_edge_target(edge);
    } else {
        graph.detach_edge_source(edge);
    }
    graph.edges[edge].original_opposite_port = Some(opposite_port);

    update_opposite_node_layer_constraints(graph, node, opposite_port.node, hidden_connections);
}

fn update_opposite_node_layer_constraints(
    graph: &mut LGraph,
    hidden_node: usize,
    opposite_node: usize,
    hidden_connections: &mut [HiddenNodeConnections],
) {
    if graph.layerless_nodes[opposite_node].layer_constraint_explicit {
        return;
    }

    hidden_connections[opposite_node] = hidden_connections[opposite_node]
        .combine(graph.layerless_nodes[hidden_node].layer_constraint);

    if !graph.node_connected_edges(opposite_node).is_empty() {
        return;
    }

    match hidden_connections[opposite_node] {
        HiddenNodeConnections::FirstSeparate => {
            graph.layerless_nodes[opposite_node].layer_constraint = LayerConstraint::First;
            graph.layerless_nodes[opposite_node].layer_constraint_explicit = true;
        }
        HiddenNodeConnections::LastSeparate => {
            graph.layerless_nodes[opposite_node].layer_constraint = LayerConstraint::Last;
            graph.layerless_nodes[opposite_node].layer_constraint_explicit = true;
        }
        HiddenNodeConnections::None | HiddenNodeConnections::Both => {}
    }
}

fn ensure_no_unacceptable_separate_edges(graph: &LGraph, node: usize) -> IntermediateResult<()> {
    match graph.layerless_nodes[node].layer_constraint {
        LayerConstraint::FirstSeparate => {
            for edge in graph.node_incoming_edges(node) {
                if !is_acceptable_separate_incident_edge(graph, edge) {
                    return Err(IntermediateError::FirstSeparateIncomingEdge {
                        node_id: graph.layerless_nodes[node].id.clone(),
                        edge_id: graph.edges[edge].id.clone(),
                    });
                }
            }
        }
        LayerConstraint::LastSeparate => {
            for edge in graph.node_outgoing_edges(node) {
                if !is_acceptable_separate_incident_edge(graph, edge) {
                    return Err(IntermediateError::LastSeparateOutgoingEdge {
                        node_id: graph.layerless_nodes[node].id.clone(),
                        edge_id: graph.edges[edge].id.clone(),
                    });
                }
            }
        }
        _ => {}
    }

    Ok(())
}

fn is_acceptable_separate_incident_edge(graph: &LGraph, edge: usize) -> bool {
    graph.layerless_nodes[graph.edges[edge].source.node].kind == LNodeKind::ExternalPort
        && graph.layerless_nodes[graph.edges[edge].target.node].kind == LNodeKind::ExternalPort
}

pub fn postprocess_layer_constraints(graph: &mut LGraph) -> IntermediateResult<()> {
    if !graph.layers.is_empty() {
        move_first_and_last_nodes(graph)?;
    }

    if !graph.hidden_nodes.is_empty() {
        restore_hidden_layer_constraint_nodes(graph);
    }

    Ok(())
}

fn move_first_and_last_nodes(graph: &mut LGraph) -> IntermediateResult<()> {
    let first_layer = 0usize;
    let last_layer = graph.layers.len() - 1;
    let mut first_label_nodes = Vec::new();
    let mut last_label_nodes = Vec::new();

    let layered_nodes = graph
        .layers
        .iter()
        .flat_map(|layer| layer.nodes.iter().copied())
        .collect::<Vec<_>>();

    for node in layered_nodes {
        match graph.layerless_nodes[node].layer_constraint {
            LayerConstraint::First => {
                ensure_no_incoming_except_label_dummies(graph, node)?;
                graph.set_node_layer(node, first_layer);
                first_label_nodes.extend(adjacent_label_dummies(graph, node, true));
            }
            LayerConstraint::Last => {
                ensure_no_outgoing_except_label_dummies(graph, node)?;
                graph.set_node_layer(node, last_layer);
                last_label_nodes.extend(adjacent_label_dummies(graph, node, false));
            }
            _ => {}
        }
    }

    graph.compact_empty_layers();

    if !first_label_nodes.is_empty() {
        graph.insert_layer(0);
        for node in first_label_nodes {
            graph.set_node_layer(node, 0);
        }
    }

    if !last_label_nodes.is_empty() {
        let layer = graph.layers.len();
        graph.layers.push(crate::graph::Layer::new());
        for node in last_label_nodes {
            graph.set_node_layer(node, layer);
        }
    }

    Ok(())
}

fn adjacent_label_dummies(graph: &LGraph, node: usize, incoming: bool) -> Vec<usize> {
    let edges = if incoming {
        graph.node_incoming_edges(node)
    } else {
        graph.node_outgoing_edges(node)
    };

    edges
        .into_iter()
        .filter_map(|edge| {
            let candidate = if incoming {
                graph.edges[edge].source.node
            } else {
                graph.edges[edge].target.node
            };
            (graph.layerless_nodes[candidate].kind == LNodeKind::Label).then_some(candidate)
        })
        .collect()
}

fn restore_hidden_layer_constraint_nodes(graph: &mut LGraph) {
    let hidden_nodes = graph.hidden_nodes.clone();
    let mut first_separate_nodes = Vec::new();
    let mut last_separate_nodes = Vec::new();

    for node in hidden_nodes {
        graph.layerless_nodes[node].hidden = false;
        match graph.layerless_nodes[node].layer_constraint {
            LayerConstraint::FirstSeparate => first_separate_nodes.push(node),
            LayerConstraint::LastSeparate => last_separate_nodes.push(node),
            _ => {}
        }

        restore_hidden_node_edges(graph, node);
    }

    if !first_separate_nodes.is_empty() {
        graph.insert_layer(0);
        for node in first_separate_nodes {
            graph.set_node_layer(node, 0);
        }
    }

    if !last_separate_nodes.is_empty() {
        let layer = graph.layers.len();
        graph.layers.push(crate::graph::Layer::new());
        for node in last_separate_nodes {
            graph.set_node_layer(node, layer);
        }
    }

    graph.hidden_nodes.clear();
}

fn restore_hidden_node_edges(graph: &mut LGraph, node: usize) {
    for edge in graph.node_connected_edges(node) {
        if graph.edge_source_attached(edge) && graph.edge_target_attached(edge) {
            continue;
        }

        let Some(original_opposite_port) = graph.edges[edge].original_opposite_port else {
            continue;
        };

        if !graph.edge_target_attached(edge) {
            graph.set_edge_target(edge, original_opposite_port);
        } else if !graph.edge_source_attached(edge) {
            graph.set_edge_source(edge, original_opposite_port);
        }
    }
}

fn ensure_no_incoming_except_label_dummies(graph: &LGraph, node: usize) -> IntermediateResult<()> {
    for edge in graph.node_incoming_edges(node) {
        if graph.layerless_nodes[graph.edges[edge].source.node].kind != LNodeKind::Label {
            return Err(IntermediateError::FirstIncomingEdge {
                node_id: graph.layerless_nodes[node].id.clone(),
                edge_id: graph.edges[edge].id.clone(),
            });
        }
    }
    Ok(())
}

fn ensure_no_outgoing_except_label_dummies(graph: &LGraph, node: usize) -> IntermediateResult<()> {
    for edge in graph.node_outgoing_edges(node) {
        if graph.layerless_nodes[graph.edges[edge].target.node].kind != LNodeKind::Label {
            return Err(IntermediateError::LastOutgoingEdge {
                node_id: graph.layerless_nodes[node].id.clone(),
                edge_id: graph.edges[edge].id.clone(),
            });
        }
    }
    Ok(())
}

pub fn insert_label_dummies(graph: &mut LGraph) {
    let edge_indices = graph
        .edges
        .iter()
        .enumerate()
        .filter(|(_, edge)| edge.source.node != edge.target.node)
        .filter(|(_, edge)| {
            edge.labels
                .iter()
                .any(|label| label.placement == EdgeLabelPlacement::Center)
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();

    for edge_index in edge_indices {
        if !graph.edge_source_attached(edge_index) || !graph.edge_target_attached(edge_index) {
            continue;
        }

        let thickness = graph.edges[edge_index].thickness.max(0.0);
        graph.edges[edge_index].thickness = thickness;
        let original_source = graph.edges[edge_index].source;
        let original_target = graph.edges[edge_index].target;
        let dummy = create_label_dummy_node(graph, edge_index, thickness);
        if split_edge(graph, edge_index, dummy).is_none() {
            continue;
        }

        graph.layerless_nodes[dummy].long_edge_source = Some(original_source);
        graph.layerless_nodes[dummy].long_edge_target = Some(original_target);
        move_center_labels_to_dummy(graph, edge_index, dummy);
        size_label_dummy(graph, dummy, thickness);
    }
}

fn create_label_dummy_node(graph: &mut LGraph, edge_index: usize, thickness: f64) -> usize {
    let dummy_index = graph.layerless_nodes.len();
    let mut dummy = LNode::new(format!("label:{edge_index}:{dummy_index}"), 0.0, 0.0, None);
    dummy.kind = LNodeKind::Label;
    dummy.origin_edge = Some(edge_index);
    dummy.port_constraints = PortConstraints::FixedPos;
    graph.layerless_nodes.push(dummy);
    graph.layerless_nodes[dummy_index].size.height = thickness.max(0.0);
    dummy_index
}

fn move_center_labels_to_dummy(graph: &mut LGraph, edge_index: usize, dummy: usize) {
    let mut retained = Vec::new();
    let mut moved = Vec::new();

    for label in std::mem::take(&mut graph.edges[edge_index].labels) {
        if label.placement == EdgeLabelPlacement::Center {
            moved.push(label);
        } else {
            retained.push(label);
        }
    }

    graph.edges[edge_index].labels = retained;
    graph.layerless_nodes[dummy].labels = moved;
}

fn size_label_dummy(graph: &mut LGraph, dummy: usize, thickness: f64) {
    let edge_label_spacing = graph.options.spacing.edge_label;
    let label_label_spacing = graph.options.spacing.label_label;
    let mut width = 0.0;
    let mut height = 0.0;

    if graph.options.direction.is_vertical() {
        for label in &graph.layerless_nodes[dummy].labels {
            width += label.size.width + label_label_spacing;
            height = f64::max(height, label.size.height);
        }
        if !graph.layerless_nodes[dummy].labels.is_empty() {
            width -= label_label_spacing;
        }
        height += edge_label_spacing + thickness;
    } else {
        for label in &graph.layerless_nodes[dummy].labels {
            width = f64::max(width, label.size.width);
            height += label.size.height + label_label_spacing;
        }
        if !graph.layerless_nodes[dummy].labels.is_empty() {
            height -= label_label_spacing;
        }
        height += edge_label_spacing + thickness;
    }

    graph.layerless_nodes[dummy].size.width = width.max(0.0);
    graph.layerless_nodes[dummy].size.height = height.max(thickness);
}

pub fn split_long_edges(graph: &mut LGraph) {
    if graph.layers.len() <= 2 {
        return;
    }

    let mut layer_index = 0usize;
    while layer_index + 1 < graph.layers.len() {
        let next_layer_index = layer_index + 1;
        let mut node_position = 0usize;

        while node_position < graph.layers[layer_index].nodes.len() {
            let node_index = graph.layers[layer_index].nodes[node_position];
            let outgoing_edges = graph.node_outgoing_edges(node_index);

            for edge_index in outgoing_edges {
                let target_node = graph.edges[edge_index].target.node;
                let Some(target_layer_index) = graph.layerless_nodes[target_node].layer_index
                else {
                    continue;
                };

                if target_layer_index != layer_index && target_layer_index != next_layer_index {
                    let dummy = create_long_edge_dummy_node(graph, next_layer_index, edge_index);
                    split_edge(graph, edge_index, dummy);
                }
            }

            node_position += 1;
        }

        layer_index += 1;
    }
}

pub fn join_long_edges(graph: &mut LGraph) {
    let add_unnecessary_bendpoints = graph.options.unnecessary_bendpoints;

    for layer_index in 0..graph.layers.len() {
        let mut node_position = 0usize;

        while node_position < graph.layers[layer_index].nodes.len() {
            let node_index = graph.layers[layer_index].nodes[node_position];

            if graph.layerless_nodes[node_index].kind == LNodeKind::LongEdge {
                join_long_edge_at(graph, node_index, add_unnecessary_bendpoints);
                graph.layers[layer_index].nodes.remove(node_position);
            } else {
                node_position += 1;
            }
        }
    }
}

pub fn switch_label_dummies(graph: &mut LGraph) {
    let label_dummies = graph
        .layers
        .iter()
        .flat_map(|layer| layer.nodes.iter().copied())
        .filter(|node| graph.layerless_nodes[*node].kind == LNodeKind::Label)
        .collect::<Vec<_>>();

    for label_dummy in label_dummies {
        if let Some(info) = LabelDummyInfo::new(graph, label_dummy) {
            let target_layer = find_center_layer_target_id(graph, &info);
            assign_label_dummy_layer(graph, &info, target_layer);
            update_long_edge_before_label_dummy_info(graph, info.label_dummy);
        }
    }
}

pub fn select_label_sides(graph: &mut LGraph) {
    match graph.options.edge_label_side_selection {
        EdgeLabelSideSelection::AlwaysUp => same_side(graph, LabelSide::Above),
        EdgeLabelSideSelection::AlwaysDown => same_side(graph, LabelSide::Below),
        EdgeLabelSideSelection::DirectionUp => based_on_direction(graph, LabelSide::Above),
        EdgeLabelSideSelection::DirectionDown => based_on_direction(graph, LabelSide::Below),
        EdgeLabelSideSelection::SmartUp => smart(graph, LabelSide::Above),
        EdgeLabelSideSelection::SmartDown => smart(graph, LabelSide::Below),
    }
}

pub fn preprocess_end_labels(graph: &mut LGraph) {
    let edge_label_spacing = graph.options.spacing.edge_label;
    let label_label_spacing = graph.options.spacing.label_label;
    let vertical_layout = graph.options.direction.is_vertical();
    let nodes = graph
        .layers
        .iter()
        .flat_map(|layer| layer.nodes.iter().copied())
        .collect::<Vec<_>>();

    for node in nodes {
        if !matches!(
            graph.layerless_nodes[node].kind,
            LNodeKind::Normal | LNodeKind::ExternalPort
        ) {
            continue;
        }
        gather_end_labels_for_node(graph, node);
        place_end_label_cells_for_node(
            graph,
            node,
            label_label_spacing,
            edge_label_spacing,
            vertical_layout,
        );
    }
}

fn same_side(graph: &mut LGraph, label_side: LabelSide) {
    for layer_index in 0..graph.layers.len() {
        let nodes = graph.layers[layer_index].nodes.clone();
        for node in nodes {
            if graph.layerless_nodes[node].kind == LNodeKind::Label {
                apply_label_side(graph, node, label_side);
            }

            let outgoing_edges = graph.node_outgoing_edges(node);
            for edge in outgoing_edges {
                apply_label_side_to_edge(graph, edge, label_side);
            }
        }
    }
}

fn based_on_direction(graph: &mut LGraph, side_for_rightward_edges: LabelSide) {
    for layer_index in 0..graph.layers.len() {
        let nodes = graph.layers[layer_index].nodes.clone();
        for node in nodes {
            if graph.layerless_nodes[node].kind == LNodeKind::Label {
                let side = if does_label_dummy_point_right(graph, node) {
                    side_for_rightward_edges
                } else {
                    side_for_rightward_edges.opposite()
                };
                apply_label_side(graph, node, side);
            }

            let outgoing_edges = graph.node_outgoing_edges(node);
            for edge in outgoing_edges {
                let side = if !graph.edges[edge].reversed {
                    side_for_rightward_edges
                } else {
                    side_for_rightward_edges.opposite()
                };
                apply_label_side_to_edge(graph, edge, side);
            }
        }
    }
}

fn smart(graph: &mut LGraph, default_side: LabelSide) {
    let mut dummy_node_queue = VecDeque::new();

    for layer_index in 0..graph.layers.len() {
        let nodes = graph.layers[layer_index].nodes.clone();
        let mut top_group = true;
        let mut label_dummies_in_queue = 0usize;

        for node in nodes {
            match graph.layerless_nodes[node].kind {
                LNodeKind::Label => {
                    label_dummies_in_queue += 1;
                    dummy_node_queue.push_back(node);
                }
                LNodeKind::LongEdge => {
                    dummy_node_queue.push_back(node);
                }
                LNodeKind::Normal => {
                    smart_for_regular_node(graph, node, default_side);
                    if !dummy_node_queue.is_empty() {
                        smart_for_consecutive_dummy_node_run(
                            graph,
                            &mut dummy_node_queue,
                            label_dummies_in_queue,
                            top_group,
                            false,
                            default_side,
                        );
                    }
                    top_group = false;
                    label_dummies_in_queue = 0;
                }
                _ => {
                    if !dummy_node_queue.is_empty() {
                        smart_for_consecutive_dummy_node_run(
                            graph,
                            &mut dummy_node_queue,
                            label_dummies_in_queue,
                            top_group,
                            false,
                            default_side,
                        );
                    }
                    top_group = false;
                    label_dummies_in_queue = 0;
                }
            }
        }

        if !dummy_node_queue.is_empty() {
            smart_for_consecutive_dummy_node_run(
                graph,
                &mut dummy_node_queue,
                label_dummies_in_queue,
                top_group,
                true,
                default_side,
            );
        }
    }
}

fn smart_for_consecutive_dummy_node_run(
    graph: &mut LGraph,
    dummy_nodes: &mut VecDeque<usize>,
    label_dummy_count: usize,
    top_group: bool,
    bottom_group: bool,
    default_side: LabelSide,
) {
    debug_assert!(!dummy_nodes.is_empty());

    if top_group
        && (!bottom_group || dummy_nodes.len() > 1)
        && label_dummy_count == 1
        && dummy_nodes
            .front()
            .is_some_and(|node| graph.layerless_nodes[*node].kind == LNodeKind::Label)
    {
        if let Some(node) = dummy_nodes.front().copied() {
            apply_label_side(graph, node, LabelSide::Above);
        }
    } else if bottom_group
        && (!top_group || dummy_nodes.len() > 1)
        && label_dummy_count == 1
        && dummy_nodes
            .back()
            .is_some_and(|node| graph.layerless_nodes[*node].kind == LNodeKind::Label)
    {
        if let Some(node) = dummy_nodes.back().copied() {
            apply_label_side(graph, node, LabelSide::Below);
        }
    } else if dummy_nodes.len() == 2 {
        if let Some(node) = dummy_nodes.pop_front() {
            apply_label_side(graph, node, LabelSide::Above);
        }
        if let Some(node) = dummy_nodes.pop_front() {
            apply_label_side(graph, node, LabelSide::Below);
        }
    } else {
        apply_for_dummy_node_run_with_simple_loops(graph, dummy_nodes, default_side);
    }

    dummy_nodes.clear();
}

fn apply_for_dummy_node_run_with_simple_loops(
    graph: &mut LGraph,
    dummy_nodes: &VecDeque<usize>,
    default_side: LabelSide,
) {
    let mut label_dummy_run = Vec::with_capacity(dummy_nodes.len());
    let mut prev_long_edge_source = None;
    let mut prev_long_edge_target = None;

    for current_dummy in dummy_nodes.iter().copied() {
        let curr_long_edge_source = get_long_edge_end_node(graph, current_dummy, true);
        let curr_long_edge_target = get_long_edge_end_node(graph, current_dummy, false);

        if prev_long_edge_source != curr_long_edge_source
            || prev_long_edge_target != curr_long_edge_target
        {
            apply_label_sides_to_label_dummy_run(graph, &mut label_dummy_run, default_side);
            prev_long_edge_source = curr_long_edge_source;
            prev_long_edge_target = curr_long_edge_target;
        }

        label_dummy_run.push(current_dummy);
    }

    apply_label_sides_to_label_dummy_run(graph, &mut label_dummy_run, default_side);
}

fn get_long_edge_end_node(graph: &LGraph, label_dummy: usize, source: bool) -> Option<usize> {
    let port = if source {
        graph.layerless_nodes[label_dummy].long_edge_source
    } else {
        graph.layerless_nodes[label_dummy].long_edge_target
    }?;
    Some(port.node)
}

fn apply_label_sides_to_label_dummy_run(
    graph: &mut LGraph,
    label_dummy_run: &mut Vec<usize>,
    default_side: LabelSide,
) {
    if label_dummy_run.is_empty() {
        return;
    }

    if label_dummy_run.len() == 2 {
        if let Some(node) = label_dummy_run.first().copied() {
            apply_label_side(graph, node, LabelSide::Above);
        }
        if let Some(node) = label_dummy_run.get(1).copied() {
            apply_label_side(graph, node, LabelSide::Below);
        }
    } else {
        for dummy_node in label_dummy_run.iter().copied() {
            apply_label_side(graph, dummy_node, default_side);
        }
    }

    label_dummy_run.clear();
}

fn smart_for_regular_node(graph: &mut LGraph, node: usize, default_side: LabelSide) {
    let mut end_label_queue = VecDeque::new();
    let mut current_port_side = None;
    let port_count = graph.layerless_nodes[node].ports.len();

    for port_index in 0..port_count {
        let port_side = graph.layerless_nodes[node].ports[port_index].side;
        if Some(port_side) != current_port_side {
            if !end_label_queue.is_empty() {
                smart_for_regular_node_port_end_labels(
                    graph,
                    node,
                    &mut end_label_queue,
                    current_port_side,
                    default_side,
                );
            }

            end_label_queue.clear();
            current_port_side = Some(port_side);
        }

        if port_has_incident_end_labels(graph, node, port_index) {
            end_label_queue.push_back(port_index);
        }
    }

    if !end_label_queue.is_empty() {
        smart_for_regular_node_port_end_labels(
            graph,
            node,
            &mut end_label_queue,
            current_port_side,
            default_side,
        );
    }
}

fn smart_for_regular_node_port_end_labels(
    graph: &mut LGraph,
    node: usize,
    end_label_queue: &mut VecDeque<usize>,
    port_side: Option<PortSide>,
    default_side: LabelSide,
) {
    debug_assert!(!end_label_queue.is_empty());
    let Some(port_side) = port_side else {
        return;
    };

    if end_label_queue.len() == 2 {
        if matches!(port_side, PortSide::North | PortSide::East) {
            if let Some(port_index) = end_label_queue.pop_front() {
                apply_label_side_to_port_labels(graph, node, port_index, LabelSide::Above);
            }
            if let Some(port_index) = end_label_queue.pop_front() {
                apply_label_side_to_port_labels(graph, node, port_index, LabelSide::Below);
            }
        } else {
            if let Some(port_index) = end_label_queue.pop_front() {
                apply_label_side_to_port_labels(graph, node, port_index, LabelSide::Below);
            }
            if let Some(port_index) = end_label_queue.pop_front() {
                apply_label_side_to_port_labels(graph, node, port_index, LabelSide::Above);
            }
        }
    } else {
        for port_index in end_label_queue.iter().copied() {
            apply_label_side_to_port_labels(graph, node, port_index, default_side);
        }
    }

    end_label_queue.clear();
}

fn apply_label_side_to_port_labels(
    graph: &mut LGraph,
    node: usize,
    port_index: usize,
    side: LabelSide,
) {
    for edge in incident_end_label_edges_for_port(graph, node, port_index) {
        let placement = end_label_placement_for_port(graph, edge, node, port_index);
        for label in &mut graph.edges[edge].labels {
            if Some(label.placement) == placement {
                label.label_side = Some(side);
            }
        }
    }
    for label in &mut graph.layerless_nodes[node].ports[port_index].labels {
        label.label_side = Some(side);
    }
}

fn apply_label_side_to_edge(graph: &mut LGraph, edge: usize, side: LabelSide) {
    for label in &mut graph.edges[edge].labels {
        label.label_side = Some(side);
    }
}

fn port_has_incident_end_labels(graph: &LGraph, node: usize, port_index: usize) -> bool {
    incident_end_label_edges_for_port(graph, node, port_index)
        .into_iter()
        .any(|edge| {
            let placement = end_label_placement_for_port(graph, edge, node, port_index);
            graph.edges[edge]
                .labels
                .iter()
                .any(|label| Some(label.placement) == placement)
        })
}

fn incident_end_label_edges_for_port(graph: &LGraph, node: usize, port_index: usize) -> Vec<usize> {
    let port = &graph.layerless_nodes[node].ports[port_index];
    port.incoming_edges
        .iter()
        .chain(port.outgoing_edges.iter())
        .copied()
        .collect()
}

fn end_label_placement_for_port(
    graph: &LGraph,
    edge: usize,
    node: usize,
    port_index: usize,
) -> Option<EdgeLabelPlacement> {
    if graph.edges[edge].source.node == node && graph.edges[edge].source.port == port_index {
        Some(EdgeLabelPlacement::Tail)
    } else if graph.edges[edge].target.node == node && graph.edges[edge].target.port == port_index {
        Some(EdgeLabelPlacement::Head)
    } else {
        None
    }
}

fn gather_end_labels_for_node(graph: &mut LGraph, node: usize) {
    let port_count = graph.layerless_nodes[node].ports.len();
    for port_index in 0..port_count {
        graph.layerless_nodes[node].ports[port_index].labels.clear();
        graph.layerless_nodes[node].ports[port_index].end_label_cell = None;
        let incident_edges = incident_end_label_edges_for_port(graph, node, port_index);
        for edge in incident_edges {
            let Some(placement) = end_label_placement_for_port(graph, edge, node, port_index)
            else {
                continue;
            };

            let mut index = 0usize;
            while index < graph.edges[edge].labels.len() {
                if graph.edges[edge].labels[index].placement == placement {
                    let mut label = graph.edges[edge].labels.remove(index);
                    if label.end_label_edge.is_none() {
                        label.end_label_edge = Some(edge);
                    }
                    graph.layerless_nodes[node].ports[port_index]
                        .labels
                        .push(label);
                } else {
                    index += 1;
                }
            }
        }
    }
}

fn place_end_label_cells_for_node(
    graph: &mut LGraph,
    node: usize,
    label_label_spacing: f64,
    edge_label_spacing: f64,
    vertical_layout: bool,
) {
    let port_count = graph.layerless_nodes[node].ports.len();
    for port_index in 0..port_count {
        if graph.layerless_nodes[node].ports[port_index]
            .labels
            .is_empty()
        {
            continue;
        }
        let cell = create_end_label_cell(
            &graph.layerless_nodes[node].ports[port_index].labels,
            label_label_spacing,
            vertical_layout,
        );
        graph.layerless_nodes[node].ports[port_index].end_label_cell = Some(cell);
        place_end_label_cell_for_port(graph, node, port_index, edge_label_spacing);
    }

    if vertical_layout {
        remove_end_label_overlaps(graph, node, PortSide::East, 2.0 * label_label_spacing);
        remove_end_label_overlaps(graph, node, PortSide::West, 2.0 * label_label_spacing);
    } else {
        remove_end_label_overlaps(graph, node, PortSide::North, 2.0 * label_label_spacing);
        remove_end_label_overlaps(graph, node, PortSide::South, 2.0 * label_label_spacing);
    }

    update_end_label_node_margin(graph, node);
}

fn create_end_label_cell(
    labels: &[LLabel],
    label_label_spacing: f64,
    vertical_layout: bool,
) -> LabelCellLayout {
    let horizontal_layout = !vertical_layout;
    let size = label_cell_size(labels, label_label_spacing, horizontal_layout);
    LabelCellLayout {
        position: LPoint::default(),
        size,
        horizontal_layout,
        horizontal_alignment: HorizontalLabelAlignment::Center,
        vertical_alignment: VerticalLabelAlignment::Center,
    }
}

fn label_cell_size(labels: &[LLabel], label_label_spacing: f64, horizontal_layout: bool) -> LSize {
    let mut width: f64 = 0.0;
    let mut height: f64 = 0.0;

    if horizontal_layout {
        for label in labels {
            width += label.size.width + label_label_spacing;
            height = height.max(label.size.height);
        }
        if !labels.is_empty() {
            width -= label_label_spacing;
        }
    } else {
        for label in labels {
            width = width.max(label.size.width);
            height += label.size.height + label_label_spacing;
        }
        if !labels.is_empty() {
            height -= label_label_spacing;
        }
    }

    LSize { width, height }
}

fn place_end_label_cell_for_port(
    graph: &mut LGraph,
    node: usize,
    port_index: usize,
    edge_label_spacing: f64,
) {
    let node_size = graph.layerless_nodes[node].size;
    let node_margin = graph.layerless_nodes[node].margin;
    let port = graph.layerless_nodes[node].ports[port_index].clone();
    let Some(mut cell) = port.end_label_cell else {
        return;
    };
    let port_anchor = LPoint {
        x: port.position.x + port.anchor.x,
        y: port.position.y + port.anchor.y,
    };
    let max_edge_thickness = max_incident_edge_thickness(graph, &port);
    let label_side = port
        .labels
        .first()
        .and_then(|label| label.label_side)
        .unwrap_or(LabelSide::Below);

    match port.side {
        PortSide::North => {
            cell.vertical_alignment = VerticalLabelAlignment::Bottom;
            cell.position.y = -node_margin.top - edge_label_spacing - cell.size.height;
            if label_side == LabelSide::Above {
                cell.horizontal_alignment = HorizontalLabelAlignment::Right;
                cell.position.x =
                    port_anchor.x - max_edge_thickness - edge_label_spacing - cell.size.width;
            } else {
                cell.horizontal_alignment = HorizontalLabelAlignment::Left;
                cell.position.x = port_anchor.x + max_edge_thickness + edge_label_spacing;
            }
        }
        PortSide::East => {
            cell.horizontal_alignment = HorizontalLabelAlignment::Left;
            cell.position.x = node_size.width + node_margin.right + edge_label_spacing;
            if label_side == LabelSide::Above {
                cell.vertical_alignment = VerticalLabelAlignment::Bottom;
                cell.position.y =
                    port_anchor.y - max_edge_thickness - edge_label_spacing - cell.size.height;
            } else {
                cell.vertical_alignment = VerticalLabelAlignment::Top;
                cell.position.y = port_anchor.y + max_edge_thickness + edge_label_spacing;
            }
        }
        PortSide::South => {
            cell.vertical_alignment = VerticalLabelAlignment::Top;
            cell.position.y = node_size.height + node_margin.bottom + edge_label_spacing;
            if label_side == LabelSide::Above {
                cell.horizontal_alignment = HorizontalLabelAlignment::Right;
                cell.position.x =
                    port_anchor.x - max_edge_thickness - edge_label_spacing - cell.size.width;
            } else {
                cell.horizontal_alignment = HorizontalLabelAlignment::Left;
                cell.position.x = port_anchor.x + max_edge_thickness + edge_label_spacing;
            }
        }
        PortSide::West => {
            cell.horizontal_alignment = HorizontalLabelAlignment::Right;
            cell.position.x = -node_margin.left - edge_label_spacing - cell.size.width;
            if label_side == LabelSide::Above {
                cell.vertical_alignment = VerticalLabelAlignment::Bottom;
                cell.position.y =
                    port_anchor.y - max_edge_thickness - edge_label_spacing - cell.size.height;
            } else {
                cell.vertical_alignment = VerticalLabelAlignment::Top;
                cell.position.y = port_anchor.y + max_edge_thickness + edge_label_spacing;
            }
        }
        PortSide::Undefined => {}
    }

    graph.layerless_nodes[node].ports[port_index].end_label_cell = Some(cell);
}

fn max_incident_edge_thickness(graph: &LGraph, port: &crate::graph::LPort) -> f64 {
    port.incoming_edges
        .iter()
        .chain(port.outgoing_edges.iter())
        .filter_map(|edge| graph.edges.get(*edge).map(|edge| edge.thickness.max(0.0)))
        .fold(0.0, f64::max)
}

fn remove_end_label_overlaps(graph: &mut LGraph, node: usize, port_side: PortSide, gap: f64) {
    let mut ports = graph.layerless_nodes[node]
        .ports
        .iter()
        .enumerate()
        .filter_map(|(port, port_data)| {
            (port_data.side == port_side && port_data.end_label_cell.is_some()).then_some(port)
        })
        .collect::<Vec<_>>();

    if ports.len() <= 1 {
        return;
    }

    match port_side {
        PortSide::North => {
            ports.sort_by(|a, b| {
                graph.layerless_nodes[node].ports[*b]
                    .end_label_cell
                    .unwrap()
                    .position
                    .y
                    .total_cmp(
                        &graph.layerless_nodes[node].ports[*a]
                            .end_label_cell
                            .unwrap()
                            .position
                            .y,
                    )
            });
            let mut limit = f64::INFINITY;
            for port in ports {
                let mut cell = graph.layerless_nodes[node].ports[port]
                    .end_label_cell
                    .unwrap();
                let max_y = if limit.is_finite() {
                    limit - gap - cell.size.height
                } else {
                    cell.position.y
                };
                cell.position.y = cell.position.y.min(max_y);
                limit = cell.position.y;
                graph.layerless_nodes[node].ports[port].end_label_cell = Some(cell);
            }
        }
        PortSide::South => {
            ports.sort_by(|a, b| {
                graph.layerless_nodes[node].ports[*a]
                    .end_label_cell
                    .unwrap()
                    .position
                    .y
                    .total_cmp(
                        &graph.layerless_nodes[node].ports[*b]
                            .end_label_cell
                            .unwrap()
                            .position
                            .y,
                    )
            });
            let mut limit = f64::NEG_INFINITY;
            for port in ports {
                let mut cell = graph.layerless_nodes[node].ports[port]
                    .end_label_cell
                    .unwrap();
                let min_y = if limit.is_finite() {
                    limit + gap
                } else {
                    cell.position.y
                };
                cell.position.y = cell.position.y.max(min_y);
                limit = cell.position.y + cell.size.height;
                graph.layerless_nodes[node].ports[port].end_label_cell = Some(cell);
            }
        }
        PortSide::East => {
            ports.sort_by(|a, b| {
                graph.layerless_nodes[node].ports[*a]
                    .end_label_cell
                    .unwrap()
                    .position
                    .x
                    .total_cmp(
                        &graph.layerless_nodes[node].ports[*b]
                            .end_label_cell
                            .unwrap()
                            .position
                            .x,
                    )
            });
            let mut limit = f64::NEG_INFINITY;
            for port in ports {
                let mut cell = graph.layerless_nodes[node].ports[port]
                    .end_label_cell
                    .unwrap();
                let min_x = if limit.is_finite() {
                    limit + gap
                } else {
                    cell.position.x
                };
                cell.position.x = cell.position.x.max(min_x);
                limit = cell.position.x + cell.size.width;
                graph.layerless_nodes[node].ports[port].end_label_cell = Some(cell);
            }
        }
        PortSide::West => {
            ports.sort_by(|a, b| {
                graph.layerless_nodes[node].ports[*b]
                    .end_label_cell
                    .unwrap()
                    .position
                    .x
                    .total_cmp(
                        &graph.layerless_nodes[node].ports[*a]
                            .end_label_cell
                            .unwrap()
                            .position
                            .x,
                    )
            });
            let mut limit = f64::INFINITY;
            for port in ports {
                let mut cell = graph.layerless_nodes[node].ports[port]
                    .end_label_cell
                    .unwrap();
                let max_x = if limit.is_finite() {
                    limit - gap - cell.size.width
                } else {
                    cell.position.x
                };
                cell.position.x = cell.position.x.min(max_x);
                limit = cell.position.x;
                graph.layerless_nodes[node].ports[port].end_label_cell = Some(cell);
            }
        }
        PortSide::Undefined => {}
    }
}

fn update_end_label_node_margin(graph: &mut LGraph, node: usize) {
    let node_size = graph.layerless_nodes[node].size;
    let node_margin = graph.layerless_nodes[node].margin;
    let mut min_x = -node_margin.left;
    let mut min_y = -node_margin.top;
    let mut max_x = node_size.width + node_margin.right;
    let mut max_y = node_size.height + node_margin.bottom;

    for port in &graph.layerless_nodes[node].ports {
        if let Some(cell) = port.end_label_cell {
            min_x = min_x.min(cell.position.x);
            min_y = min_y.min(cell.position.y);
            max_x = max_x.max(cell.position.x + cell.size.width);
            max_y = max_y.max(cell.position.y + cell.size.height);
        }
    }

    graph.layerless_nodes[node].margin.left = (-min_x).max(0.0);
    graph.layerless_nodes[node].margin.top = (-min_y).max(0.0);
    graph.layerless_nodes[node].margin.right =
        (max_x - graph.layerless_nodes[node].margin.left - node_size.width).max(0.0);
    graph.layerless_nodes[node].margin.bottom =
        (max_y - graph.layerless_nodes[node].margin.top - node_size.height).max(0.0);
}

pub fn sort_end_labels(graph: &mut LGraph) {
    let edge_sort_keys = (0..graph.edges.len())
        .map(|edge| end_label_sort_key(graph, edge))
        .collect::<Vec<_>>();

    for node in 0..graph.layerless_nodes.len() {
        let port_count = graph.layerless_nodes[node].ports.len();
        for port in 0..port_count {
            if graph.layerless_nodes[node].ports[port].labels.len() < 2 {
                continue;
            }
            graph.layerless_nodes[node].ports[port]
                .labels
                .sort_by(|a, b| {
                    let a_key = a
                        .end_label_edge
                        .and_then(|edge| edge_sort_keys.get(edge).copied())
                        .unwrap_or((usize::MAX, usize::MAX, usize::MAX));
                    let b_key = b
                        .end_label_edge
                        .and_then(|edge| edge_sort_keys.get(edge).copied())
                        .unwrap_or((usize::MAX, usize::MAX, usize::MAX));
                    a_key.cmp(&b_key)
                });
        }
    }
}

fn end_label_sort_key(graph: &LGraph, edge: usize) -> (usize, usize, usize) {
    let edge = &graph.edges[edge];
    let source_port = edge.source.port;
    let target_node = edge.target.node;
    let target_port = usize::MAX.saturating_sub(edge.target.port);
    (source_port, target_node, target_port)
}

pub fn postprocess_end_labels(graph: &mut LGraph) {
    for node in 0..graph.layerless_nodes.len() {
        if !matches!(
            graph.layerless_nodes[node].kind,
            LNodeKind::Normal | LNodeKind::ExternalPort
        ) {
            continue;
        }
        let node_position = graph.layerless_nodes[node].position;
        let port_count = graph.layerless_nodes[node].ports.len();
        for port_index in 0..port_count {
            let Some(cell) = graph.layerless_nodes[node].ports[port_index].end_label_cell else {
                continue;
            };
            apply_end_label_cell_layout(graph, node, port_index, node_position, cell);
            graph.layerless_nodes[node].ports[port_index].end_label_cell = None;
        }
    }
}

fn apply_end_label_cell_layout(
    graph: &mut LGraph,
    node: usize,
    port_index: usize,
    node_position: LPoint,
    cell: LabelCellLayout,
) {
    let label_label_spacing = graph.options.spacing.label_label;
    let mut labels = std::mem::take(&mut graph.layerless_nodes[node].ports[port_index].labels);
    let mut cursor = cell.position;
    cursor.x += node_position.x;
    cursor.y += node_position.y;

    for label in &mut labels {
        if cell.horizontal_layout {
            label.position.x = cursor.x;
            label.position.y = aligned_y(cursor.y, cell.size.height, label.size.height, cell);
            cursor.x += label.size.width + label_label_spacing;
        } else {
            label.position.x = aligned_x(cursor.x, cell.size.width, label.size.width, cell);
            label.position.y = cursor.y;
            cursor.y += label.size.height + label_label_spacing;
        }
    }

    for label in labels {
        let target_edge = label.end_label_edge.unwrap_or_else(|| {
            let port = &graph.layerless_nodes[node].ports[port_index];
            port.incoming_edges
                .first()
                .or_else(|| port.outgoing_edges.first())
                .copied()
                .unwrap_or(usize::MAX)
        });
        if let Some(edge) = graph.edges.get_mut(target_edge) {
            edge.labels.push(label);
        }
    }
}

fn aligned_x(x: f64, cell_width: f64, label_width: f64, cell: LabelCellLayout) -> f64 {
    match cell.horizontal_alignment {
        HorizontalLabelAlignment::Left => x,
        HorizontalLabelAlignment::Center => x + (cell_width - label_width) / 2.0,
        HorizontalLabelAlignment::Right => x + cell_width - label_width,
    }
}

fn aligned_y(y: f64, cell_height: f64, label_height: f64, cell: LabelCellLayout) -> f64 {
    match cell.vertical_alignment {
        VerticalLabelAlignment::Top => y,
        VerticalLabelAlignment::Center => y + (cell_height - label_height) / 2.0,
        VerticalLabelAlignment::Bottom => y + cell_height - label_height,
    }
}

pub fn remove_label_dummies(graph: &mut LGraph) {
    for layer_index in 0..graph.layers.len() {
        let mut node_position = 0usize;

        while node_position < graph.layers[layer_index].nodes.len() {
            let node = graph.layers[layer_index].nodes[node_position];
            if graph.layerless_nodes[node].kind == LNodeKind::Label {
                place_label_dummy_labels(graph, node);
                let add_unnecessary_bendpoints =
                    graph.options.edge_routing == crate::options::EdgeRouting::Polyline;
                join_long_edge_at(graph, node, add_unnecessary_bendpoints);
                graph.layers[layer_index].nodes.remove(node_position);
            } else {
                node_position += 1;
            }
        }
    }
}

#[derive(Debug, Clone)]
struct LabelDummyInfo {
    label_dummy: usize,
    left_long_edge_dummies: Vec<usize>,
    right_long_edge_dummies: Vec<usize>,
    leftmost_layer_id: usize,
    rightmost_layer_id: usize,
}

impl LabelDummyInfo {
    fn new(graph: &LGraph, label_dummy: usize) -> Option<Self> {
        let left_long_edge_dummies = gather_left_long_edge_dummies(graph, label_dummy)?;
        let right_long_edge_dummies = gather_right_long_edge_dummies(graph, label_dummy)?;
        let leftmost_layer_id = left_long_edge_dummies
            .first()
            .copied()
            .unwrap_or(label_dummy);
        let leftmost_layer_id = graph.layerless_nodes[leftmost_layer_id].layer_index?;
        let rightmost_layer_id = right_long_edge_dummies
            .last()
            .copied()
            .unwrap_or(label_dummy);
        let rightmost_layer_id = graph.layerless_nodes[rightmost_layer_id].layer_index?;

        Some(Self {
            label_dummy,
            left_long_edge_dummies,
            right_long_edge_dummies,
            leftmost_layer_id,
            rightmost_layer_id,
        })
    }

    fn dummy_at_offset(&self, offset: usize) -> Option<usize> {
        if offset < self.left_long_edge_dummies.len() {
            self.left_long_edge_dummies.get(offset).copied()
        } else if offset == self.left_long_edge_dummies.len() {
            Some(self.label_dummy)
        } else {
            self.right_long_edge_dummies
                .get(offset - self.left_long_edge_dummies.len() - 1)
                .copied()
        }
    }
}

fn gather_left_long_edge_dummies(graph: &LGraph, label_dummy: usize) -> Option<Vec<usize>> {
    let mut out = Vec::new();
    let mut current = label_dummy;

    loop {
        let incoming = graph.node_incoming_edges(current).into_iter().next()?;
        let source = graph.edges[incoming].source.node;
        if graph.layerless_nodes[source].kind == LNodeKind::LongEdge {
            out.push(source);
            current = source;
        } else {
            break;
        }
    }

    out.reverse();
    Some(out)
}

fn gather_right_long_edge_dummies(graph: &LGraph, label_dummy: usize) -> Option<Vec<usize>> {
    let mut out = Vec::new();
    let mut current = label_dummy;

    loop {
        let outgoing = graph.node_outgoing_edges(current).into_iter().next()?;
        let target = graph.edges[outgoing].target.node;
        if graph.layerless_nodes[target].kind == LNodeKind::LongEdge {
            out.push(target);
            current = target;
        } else {
            break;
        }
    }

    Some(out)
}

fn find_center_layer_target_id(graph: &LGraph, info: &LabelDummyInfo) -> usize {
    let layer_width_sums = compute_layer_width_sums(graph, info);
    let threshold = layer_width_sums.last().copied().unwrap_or(0.0) / 2.0;
    for (offset, width_sum) in layer_width_sums.iter().enumerate() {
        if *width_sum >= threshold {
            return info.leftmost_layer_id + offset;
        }
    }
    info.leftmost_layer_id + info.left_long_edge_dummies.len()
}

fn compute_layer_width_sums(graph: &LGraph, info: &LabelDummyInfo) -> Vec<f64> {
    let total = info.rightmost_layer_id - info.leftmost_layer_id + 1;
    let edge_node_spacing = graph.options.spacing.edge_node_between_layers * 2.0;
    let node_node_spacing = graph.options.spacing.node_node_between_layers;
    let min_space_between_layers = edge_node_spacing.max(node_node_spacing);
    let mut width_sums = Vec::with_capacity(total);
    let mut current_sum = -min_space_between_layers;

    for layer in info.leftmost_layer_id..=info.rightmost_layer_id {
        current_sum += graph.layers[layer].size.width + min_space_between_layers;
        width_sums.push(current_sum);
    }

    width_sums
}

fn assign_label_dummy_layer(graph: &mut LGraph, info: &LabelDummyInfo, target_layer: usize) {
    let current_layer = graph.layerless_nodes[info.label_dummy]
        .layer_index
        .unwrap_or(target_layer);
    if current_layer == target_layer {
        return;
    }

    let Some(target_dummy) = info.dummy_at_offset(target_layer - info.leftmost_layer_id) else {
        return;
    };
    if graph.layerless_nodes[target_dummy].kind != LNodeKind::LongEdge {
        return;
    }
    swap_label_and_long_edge_dummies(graph, info.label_dummy, target_dummy);
}

fn swap_label_and_long_edge_dummies(
    graph: &mut LGraph,
    label_dummy: usize,
    long_edge_dummy: usize,
) {
    let Some(label_layer) = graph.layerless_nodes[label_dummy].layer_index else {
        return;
    };
    let Some(long_edge_layer) = graph.layerless_nodes[long_edge_dummy].layer_index else {
        return;
    };
    let Some(label_position) = graph.layers[label_layer]
        .nodes
        .iter()
        .position(|node| *node == label_dummy)
    else {
        return;
    };
    let Some(long_edge_position) = graph.layers[long_edge_layer]
        .nodes
        .iter()
        .position(|node| *node == long_edge_dummy)
    else {
        return;
    };

    let Some(label_input) = first_port_on_side(graph, label_dummy, PortSide::West) else {
        return;
    };
    let Some(label_output) = first_port_on_side(graph, label_dummy, PortSide::East) else {
        return;
    };
    let Some(long_input) = first_port_on_side(graph, long_edge_dummy, PortSide::West) else {
        return;
    };
    let Some(long_output) = first_port_on_side(graph, long_edge_dummy, PortSide::East) else {
        return;
    };

    let label_input_ref = PortRef {
        node: label_dummy,
        port: label_input,
    };
    let label_output_ref = PortRef {
        node: label_dummy,
        port: label_output,
    };
    let long_input_ref = PortRef {
        node: long_edge_dummy,
        port: long_input,
    };
    let long_output_ref = PortRef {
        node: long_edge_dummy,
        port: long_output,
    };
    let label_incoming = graph.layerless_nodes[label_dummy].ports[label_input]
        .incoming_edges
        .clone();
    let label_outgoing = graph.layerless_nodes[label_dummy].ports[label_output]
        .outgoing_edges
        .clone();
    let long_incoming = graph.layerless_nodes[long_edge_dummy].ports[long_input]
        .incoming_edges
        .clone();
    let long_outgoing = graph.layerless_nodes[long_edge_dummy].ports[long_output]
        .outgoing_edges
        .clone();

    graph.layers[long_edge_layer].nodes[long_edge_position] = label_dummy;
    graph.layerless_nodes[label_dummy].layer_index = Some(long_edge_layer);
    for edge in long_incoming {
        graph.set_edge_target(edge, label_input_ref);
    }
    for edge in long_outgoing {
        graph.set_edge_source(edge, label_output_ref);
    }

    graph.layers[label_layer].nodes[label_position] = long_edge_dummy;
    graph.layerless_nodes[long_edge_dummy].layer_index = Some(label_layer);
    for edge in label_incoming {
        graph.set_edge_target(edge, long_input_ref);
    }
    for edge in label_outgoing {
        graph.set_edge_source(edge, long_output_ref);
    }
}

fn update_long_edge_before_label_dummy_info(graph: &mut LGraph, label_dummy: usize) {
    let mut current = label_dummy;
    loop {
        let Some(incoming) = graph.node_incoming_edges(current).into_iter().next() else {
            break;
        };
        let source = graph.edges[incoming].source.node;
        if graph.layerless_nodes[source].kind != LNodeKind::LongEdge {
            break;
        }
        graph.layerless_nodes[source].long_edge_has_label_dummies = true;
        current = source;
    }
}

fn apply_label_side(graph: &mut LGraph, label_dummy: usize, side: LabelSide) {
    if graph.layerless_nodes[label_dummy].kind != LNodeKind::Label {
        return;
    }

    let effective_side = if graph.layerless_nodes[label_dummy]
        .labels
        .iter()
        .all(|label| label.inline)
    {
        LabelSide::Inline
    } else {
        side
    };

    graph.layerless_nodes[label_dummy].label_side = effective_side;
    for label in &mut graph.layerless_nodes[label_dummy].labels {
        label.label_side = Some(effective_side);
    }

    if effective_side == LabelSide::Below {
        return;
    }

    let origin_edge = graph.layerless_nodes[label_dummy]
        .origin_edge
        .and_then(|edge| graph.edges.get(edge))
        .map(|edge| edge.thickness.max(0.0))
        .unwrap_or(0.0);
    let port_y = match effective_side {
        LabelSide::Above => {
            graph.layerless_nodes[label_dummy].size.height - (origin_edge / 2.0).ceil()
        }
        LabelSide::Inline => {
            let edge_label_spacing = graph.options.spacing.edge_label;
            let port_y =
                (graph.layerless_nodes[label_dummy].size.height - edge_label_spacing - origin_edge)
                    .ceil()
                    / 2.0;
            graph.layerless_nodes[label_dummy].size.height -= edge_label_spacing;
            graph.layerless_nodes[label_dummy].size.height -= origin_edge;
            port_y
        }
        LabelSide::Below => 0.0,
    };

    for port in &mut graph.layerless_nodes[label_dummy].ports {
        port.position.y = port_y;
    }
}

fn does_label_dummy_point_right(graph: &LGraph, label_dummy: usize) -> bool {
    let incoming = graph.node_incoming_edges(label_dummy).into_iter().next();
    let outgoing = graph.node_outgoing_edges(label_dummy).into_iter().next();

    incoming
        .map(|edge| !graph.edges[edge].reversed)
        .unwrap_or(false)
        || outgoing
            .map(|edge| !graph.edges[edge].reversed)
            .unwrap_or(false)
}

fn place_label_dummy_labels(graph: &mut LGraph, label_dummy: usize) {
    let Some(origin_edge) = graph.layerless_nodes[label_dummy].origin_edge else {
        return;
    };
    let thickness = graph.edges[origin_edge].thickness.max(0.0);
    let edge_label_spacing = graph.options.spacing.edge_label;
    let label_label_spacing = graph.options.spacing.label_label;
    let labels_below_edge = graph.layerless_nodes[label_dummy].label_side == LabelSide::Below;
    let inline = graph.layerless_nodes[label_dummy]
        .labels
        .iter()
        .all(|label| label.inline);
    let mut label_position = graph.layerless_nodes[label_dummy].position;

    if labels_below_edge {
        label_position.y += thickness + edge_label_spacing;
    }

    let label_space = LPoint {
        x: graph.layerless_nodes[label_dummy].size.width,
        y: graph.layerless_nodes[label_dummy].size.height
            + if inline {
                0.0
            } else {
                -thickness - edge_label_spacing
            },
    };
    let mut labels = std::mem::take(&mut graph.layerless_nodes[label_dummy].labels);

    if graph.options.direction.is_vertical() {
        place_labels_for_vertical_layout(
            &mut labels,
            label_position,
            label_label_spacing,
            label_space,
            labels_below_edge,
            graph.options.direction,
        );
    } else {
        place_labels_for_horizontal_layout(
            &mut labels,
            label_position,
            label_label_spacing,
            label_space,
        );
    }

    graph.edges[origin_edge].labels.extend(labels);
}

fn place_labels_for_horizontal_layout(
    labels: &mut [LLabel],
    mut label_position: LPoint,
    label_spacing: f64,
    label_space: LPoint,
) {
    for label in labels {
        label.position.x = label_position.x + (label_space.x - label.size.width) / 2.0;
        label.position.y = label_position.y;
        label_position.y += label.size.height + label_spacing;
    }
}

fn place_labels_for_vertical_layout(
    labels: &mut [LLabel],
    mut label_position: LPoint,
    label_spacing: f64,
    label_space: LPoint,
    left_aligned: bool,
    direction: ElkDirection,
) {
    let inline = labels.iter().all(|label| label.inline);

    if direction == ElkDirection::Up {
        labels.reverse();
    }

    for label in &mut *labels {
        label.position.x = label_position.x;
        if inline {
            label.position.y = label_position.y + (label_space.y - label.size.height) / 2.0;
        } else if left_aligned {
            label.position.y = label_position.y;
        } else {
            label.position.y = label_position.y + label_space.y - label.size.height;
        }
        label_position.x += label.size.width + label_spacing;
    }

    if direction == ElkDirection::Up {
        labels.reverse();
    }
}

pub fn join_long_edge_at(
    graph: &mut LGraph,
    long_edge_dummy: usize,
    add_unnecessary_bendpoints: bool,
) {
    let Some(input_port) = first_port_on_side(graph, long_edge_dummy, PortSide::West) else {
        return;
    };
    let Some(output_port) = first_port_on_side(graph, long_edge_dummy, PortSide::East) else {
        return;
    };

    let mut input_port_edges = graph.layerless_nodes[long_edge_dummy].ports[input_port]
        .incoming_edges
        .clone();
    let mut output_port_edges = graph.layerless_nodes[long_edge_dummy].ports[output_port]
        .outgoing_edges
        .clone();

    while !input_port_edges.is_empty() && !output_port_edges.is_empty() {
        let surviving_edge = input_port_edges.remove(0);
        let dropped_edge = output_port_edges.remove(0);
        join_long_edge_pair(
            graph,
            long_edge_dummy,
            surviving_edge,
            dropped_edge,
            add_unnecessary_bendpoints,
        );
    }
}

fn first_port_on_side(graph: &LGraph, node: usize, side: PortSide) -> Option<usize> {
    graph.layerless_nodes[node]
        .ports
        .iter()
        .position(|port| port.side == side)
}

fn join_long_edge_pair(
    graph: &mut LGraph,
    long_edge_dummy: usize,
    surviving_edge: usize,
    dropped_edge: usize,
    add_unnecessary_bendpoints: bool,
) {
    let dropped_target = graph.edges[dropped_edge].target;
    let dropped_target_index = graph.layerless_nodes[dropped_target.node].ports
        [dropped_target.port]
        .incoming_edges
        .iter()
        .position(|edge| *edge == dropped_edge);
    let dropped_bend_points = graph.edges[dropped_edge].bend_points.clone();
    let dropped_labels = std::mem::take(&mut graph.edges[dropped_edge].labels);

    graph.set_edge_target_at(surviving_edge, dropped_target, dropped_target_index);
    graph.detach_edge(dropped_edge);

    if add_unnecessary_bendpoints {
        let unnecessary_bendpoint = long_edge_dummy_anchor(graph, long_edge_dummy);
        graph.edges[surviving_edge]
            .bend_points
            .push(unnecessary_bendpoint);
    }
    graph.edges[surviving_edge]
        .bend_points
        .extend(dropped_bend_points);
    graph.edges[surviving_edge].labels.extend(dropped_labels);
}

fn long_edge_dummy_anchor(graph: &LGraph, node: usize) -> LPoint {
    let port = &graph.layerless_nodes[node].ports[0];
    LPoint {
        x: graph.layerless_nodes[node].position.x + port.position.x + port.anchor.x,
        y: graph.layerless_nodes[node].position.y + port.position.y + port.anchor.y,
    }
}

pub fn calculate_layer_sizes_and_graph_height(graph: &mut LGraph) {
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut found_nodes = false;
    let ports_surrounding = graph.options.spacing.ports_surrounding;

    for layer in &mut graph.layers {
        layer.size.width = 0.0;
        layer.size.height = 0.0;

        if layer.nodes.is_empty() {
            continue;
        }

        found_nodes = true;

        for node_index in &layer.nodes {
            let node = &graph.layerless_nodes[*node_index];
            layer.size.width = layer
                .size
                .width
                .max(node.size.width + node.margin.left + node.margin.right);
        }

        let first_node = &graph.layerless_nodes[layer.nodes[0]];
        let mut top = first_node.position.y - first_node.margin.top;
        if first_node.kind == LNodeKind::ExternalPort {
            top -= ports_surrounding.top;
        }

        let last_node = &graph.layerless_nodes[layer.nodes[layer.nodes.len() - 1]];
        let mut bottom = last_node.position.y + last_node.size.height + last_node.margin.bottom;
        if last_node.kind == LNodeKind::ExternalPort {
            bottom += ports_surrounding.bottom;
        }

        layer.size.height = bottom - top;
        min_y = min_y.min(top);
        max_y = max_y.max(bottom);
    }

    if !found_nodes {
        min_y = 0.0;
        max_y = 0.0;
    }

    graph.size.height = max_y - min_y;
    graph.offset.y -= min_y;
}

pub fn process_hierarchical_port_dummy_sizes(graph: &mut LGraph) {
    let delta = graph.options.spacing.edge_edge_between_layers * 2.0;

    for layer_index in 0..graph.layers.len() {
        let (northern_dummies, southern_dummies) =
            collect_north_south_external_port_dummies(graph, layer_index);
        set_hierarchical_port_dummy_widths(graph, &northern_dummies, true, delta);
        set_hierarchical_port_dummy_widths(graph, &southern_dummies, false, delta);
    }
}

fn collect_north_south_external_port_dummies(
    graph: &LGraph,
    layer_index: usize,
) -> (Vec<usize>, Vec<usize>) {
    let mut northern_dummies = Vec::new();
    let mut southern_dummies = Vec::new();

    for node_index in &graph.layers[layer_index].nodes {
        let node = &graph.layerless_nodes[*node_index];
        if node.kind != LNodeKind::ExternalPort {
            continue;
        }

        match node.external_port_side {
            PortSide::North => northern_dummies.push(*node_index),
            PortSide::South => southern_dummies.push(*node_index),
            PortSide::Undefined | PortSide::East | PortSide::West => {}
        }
    }

    (northern_dummies, southern_dummies)
}

fn set_hierarchical_port_dummy_widths(
    graph: &mut LGraph,
    nodes: &[usize],
    top_down: bool,
    delta: f64,
) {
    let mut current_width = if top_down {
        0.0
    } else {
        delta * nodes.len().saturating_sub(1) as f64
    };
    let step = if top_down { delta } else { -delta };

    for node_index in nodes {
        let node = &mut graph.layerless_nodes[*node_index];
        node.node_alignment = Alignment::Center;
        node.size.width = current_width;

        for port in &mut node.ports {
            if port.side == PortSide::East {
                port.position.x = current_width;
            }
        }

        current_width += step;
    }
}

pub fn process_hierarchical_port_positions(graph: &mut LGraph) {
    if !(graph.options.port_constraints.is_ratio_fixed()
        || graph.options.port_constraints.is_pos_fixed())
        || graph.layers.is_empty()
    {
        return;
    }

    process_hierarchical_port_positions_for_layer(graph, 0);
    if graph.layers.len() > 1 {
        process_hierarchical_port_positions_for_layer(graph, graph.layers.len() - 1);
    }
}

fn process_hierarchical_port_positions_for_layer(graph: &mut LGraph, layer_index: usize) {
    let graph_height = actual_graph_height(graph);
    let nodes = graph.layers[layer_index].nodes.clone();

    for node in nodes {
        if graph.layerless_nodes[node].kind != LNodeKind::ExternalPort {
            continue;
        }
        if !matches!(
            graph.layerless_nodes[node].external_port_side,
            PortSide::East | PortSide::West
        ) {
            continue;
        }

        let mut final_y_coordinate = graph.layerless_nodes[node].port_ratio_or_position;
        if graph.options.port_constraints.is_ratio_fixed() {
            final_y_coordinate *= graph_height;
        }

        let port_anchor_y = first_port_anchor_y(graph, node);
        graph.layerless_nodes[node].position.y = final_y_coordinate - port_anchor_y;
        border_to_content_area_y(graph, node);
    }
}

fn actual_graph_height(graph: &LGraph) -> f64 {
    graph.size.height + graph.padding.top + graph.padding.bottom
}

fn first_port_anchor_y(graph: &LGraph, node: usize) -> f64 {
    graph.layerless_nodes[node]
        .ports
        .first()
        .map(|port| port.position.y)
        .unwrap_or(0.0)
}

fn border_to_content_area_y(graph: &mut LGraph, node: usize) {
    graph.layerless_nodes[node].position.y -= graph.padding.top + graph.offset.y;
}

pub fn process_hierarchical_port_orthogonal_edges(graph: &mut LGraph) {
    if graph.layers.is_empty() {
        return;
    }

    let north_south_dummies = restore_north_south_dummies(graph);
    set_north_south_dummy_coordinates(graph, &north_south_dummies);
    route_north_south_dummy_edges(graph, &north_south_dummies);
    remove_temporary_north_south_dummies(graph);
    fix_hierarchical_port_coordinates(graph);
    correct_hierarchical_port_slanted_edge_segments(graph);
}

fn restore_north_south_dummies(graph: &mut LGraph) -> Vec<usize> {
    let restored_dummies = graph.replaced_external_port_dummies.clone();
    if restored_dummies.is_empty() {
        return restored_dummies;
    }

    for dummy in &restored_dummies {
        restore_north_south_dummy(graph, *dummy);
    }

    let replacement_nodes = graph
        .layers
        .iter()
        .flat_map(|layer| layer.nodes.iter().copied())
        .filter(|node| {
            graph.layerless_nodes[*node].kind == LNodeKind::ExternalPort
                && graph.layerless_nodes[*node]
                    .replaced_external_port_dummy
                    .is_some()
        })
        .collect::<Vec<_>>();

    for replacement in replacement_nodes {
        if let Some(original) = graph.layerless_nodes[replacement].replaced_external_port_dummy {
            connect_replacement_dummy_to_original(graph, replacement, original);
        }
    }

    let last_layer = graph.layers.len() - 1;
    for dummy in &restored_dummies {
        graph.set_node_layer(*dummy, last_layer);
    }

    restored_dummies
}

fn restore_north_south_dummy(graph: &mut LGraph, dummy: usize) {
    let external_side = graph.layerless_nodes[dummy].external_port_side;
    let Some(port) = graph.layerless_nodes[dummy].ports.first_mut() else {
        return;
    };

    port.side = match external_side {
        PortSide::North => PortSide::South,
        PortSide::South => PortSide::North,
        side => side,
    };
    port.port_type = PortType::Output;
}

fn connect_replacement_dummy_to_original(graph: &mut LGraph, replacement: usize, original: usize) {
    if graph.layerless_nodes[replacement].ports.iter().any(|port| {
        port.side == graph.layerless_nodes[replacement].external_port_side
            && port.outgoing_edges.iter().any(|edge| {
                graph.edges[*edge].target.node == original && graph.edges[*edge].target.port == 0
            })
    }) {
        return;
    }

    let external_side = graph.layerless_nodes[replacement].external_port_side;
    let Some(origin_port) = graph.add_port(
        replacement,
        PortType::Output,
        external_side,
        Default::default(),
    ) else {
        return;
    };

    let edge = LayeredEdge {
        id: format!(
            "{}->{}:hierarchical-origin",
            graph.layerless_nodes[replacement].id, graph.layerless_nodes[original].id
        ),
        source: origin_port,
        target: PortRef {
            node: original,
            port: 0,
        },
        source_node_id: graph.layerless_nodes[replacement].id.clone(),
        target_node_id: graph.layerless_nodes[original].id.clone(),
        labels: Vec::new(),
        minlen: 1,
        reversed: false,
        bend_points: Vec::new(),
        model_order: None,
        priority_direction: 0,
        priority_shortness: 0,
        priority_straightness: 0,
        thickness: 0.0,
        original_opposite_port: None,
    };
    graph.add_edge(edge);
}

fn set_north_south_dummy_coordinates(graph: &mut LGraph, north_south_dummies: &[usize]) {
    if north_south_dummies.is_empty() {
        return;
    }

    let graph_width = graph.size.width + graph.padding.left + graph.padding.right;
    let north_y = -graph.padding.top - graph.offset.y;
    let south_y = graph.size.height + graph.padding.top + graph.padding.bottom - graph.offset.y;
    let mut northern_dummies = Vec::new();
    let mut southern_dummies = Vec::new();

    for dummy in north_south_dummies {
        match graph.options.port_constraints {
            PortConstraints::Free | PortConstraints::FixedSide | PortConstraints::FixedOrder => {
                calculate_north_south_dummy_position(graph, *dummy);
            }
            PortConstraints::FixedRatio => {
                graph.layerless_nodes[*dummy].position.x = graph_width
                    * graph.layerless_nodes[*dummy].port_ratio_or_position
                    - first_port_anchor_x(graph, *dummy);
                border_to_content_area_x(graph, *dummy);
            }
            PortConstraints::FixedPos => {
                graph.layerless_nodes[*dummy].position.x = graph.layerless_nodes[*dummy]
                    .port_ratio_or_position
                    - first_port_anchor_x(graph, *dummy);
                border_to_content_area_x(graph, *dummy);
                let required_width = graph.layerless_nodes[*dummy].position.x
                    + graph.layerless_nodes[*dummy].size.width / 2.0;
                graph.size.width = graph.size.width.max(required_width);
            }
            PortConstraints::Undefined => {
                calculate_north_south_dummy_position(graph, *dummy);
            }
        }

        match graph.layerless_nodes[*dummy].external_port_side {
            PortSide::North => {
                graph.layerless_nodes[*dummy].position.y = north_y;
                northern_dummies.push(*dummy);
            }
            PortSide::South => {
                graph.layerless_nodes[*dummy].position.y = south_y;
                southern_dummies.push(*dummy);
            }
            PortSide::Undefined | PortSide::East | PortSide::West => {}
        }
    }

    match graph.options.port_constraints {
        PortConstraints::Free | PortConstraints::FixedSide | PortConstraints::Undefined => {
            ensure_unique_north_south_positions(graph, &mut northern_dummies);
            ensure_unique_north_south_positions(graph, &mut southern_dummies);
        }
        PortConstraints::FixedOrder => {
            restore_north_south_dummy_order(graph, &mut northern_dummies);
            restore_north_south_dummy_order(graph, &mut southern_dummies);
        }
        PortConstraints::FixedRatio | PortConstraints::FixedPos => {}
    }
}

fn calculate_north_south_dummy_position(graph: &mut LGraph, dummy: usize) {
    let Some(dummy_port) = graph.layerless_nodes[dummy].ports.first() else {
        graph.layerless_nodes[dummy].position.x = 0.0;
        return;
    };

    let connected_ports = dummy_port
        .incoming_edges
        .iter()
        .filter_map(|edge| graph.edges.get(*edge).map(|edge| edge.source))
        .chain(
            dummy_port
                .outgoing_edges
                .iter()
                .filter_map(|edge| graph.edges.get(*edge).map(|edge| edge.target)),
        )
        .collect::<Vec<_>>();

    if connected_ports.is_empty() {
        graph.layerless_nodes[dummy].position.x = 0.0;
        return;
    }

    let pos_sum = connected_ports
        .iter()
        .map(|port_ref| absolute_port_anchor(graph, *port_ref).x)
        .sum::<f64>();
    graph.layerless_nodes[dummy].position.x =
        pos_sum / connected_ports.len() as f64 - first_port_anchor_x(graph, dummy);
}

fn first_port_anchor_x(graph: &LGraph, node: usize) -> f64 {
    graph.layerless_nodes[node]
        .ports
        .first()
        .map(|port| port.position.x)
        .unwrap_or(0.0)
}

fn border_to_content_area_x(graph: &mut LGraph, node: usize) {
    graph.layerless_nodes[node].position.x -= graph.padding.left + graph.offset.x;
}

fn ensure_unique_north_south_positions(graph: &mut LGraph, dummies: &mut [usize]) {
    dummies.sort_by(|left, right| {
        graph.layerless_nodes[*left]
            .position
            .x
            .total_cmp(&graph.layerless_nodes[*right].position.x)
    });
    assign_ascending_north_south_coordinates(graph, dummies);
}

fn restore_north_south_dummy_order(graph: &mut LGraph, dummies: &mut [usize]) {
    dummies.sort_by(|left, right| {
        graph.layerless_nodes[*left]
            .port_ratio_or_position
            .total_cmp(&graph.layerless_nodes[*right].port_ratio_or_position)
    });
    assign_ascending_north_south_coordinates(graph, dummies);
}

fn assign_ascending_north_south_coordinates(graph: &mut LGraph, dummies: &[usize]) {
    let Some(first) = dummies.first() else {
        return;
    };

    let spacing = graph.options.spacing.port_port;
    let first_node = &graph.layerless_nodes[*first];
    let mut next_valid_coordinate =
        first_node.position.x + first_node.size.width + first_node.margin.right + spacing;

    for dummy in dummies.iter().skip(1) {
        let current = &graph.layerless_nodes[*dummy];
        let delta = current.position.x - current.margin.left - next_valid_coordinate;
        if delta < 0.0 {
            graph.layerless_nodes[*dummy].position.x -= delta;
        }

        let current = &graph.layerless_nodes[*dummy];
        graph.size.width = graph
            .size
            .width
            .max(current.position.x + current.size.width);
        next_valid_coordinate =
            current.position.x + current.size.width + current.margin.right + spacing;
    }
}

fn route_north_south_dummy_edges(graph: &mut LGraph, north_south_dummies: &[usize]) {
    let mut northern_source_layer = Vec::new();
    let mut northern_target_layer = Vec::new();
    let mut southern_source_layer = Vec::new();
    let mut southern_target_layer = Vec::new();

    for dummy in north_south_dummies {
        match graph.layerless_nodes[*dummy].external_port_side {
            PortSide::North => {
                push_unique(&mut northern_target_layer, *dummy);
                for edge in graph.node_incoming_edges(*dummy) {
                    push_unique(&mut northern_source_layer, graph.edges[edge].source.node);
                }
            }
            PortSide::South => {
                push_unique(&mut southern_target_layer, *dummy);
                for edge in graph.node_incoming_edges(*dummy) {
                    push_unique(&mut southern_source_layer, graph.edges[edge].source.node);
                }
            }
            PortSide::Undefined | PortSide::East | PortSide::West => {}
        }
    }

    let node_spacing = graph.options.spacing.node_node;
    let edge_spacing = graph.options.spacing.edge_edge;

    if !northern_source_layer.is_empty() {
        let slots = route_edges(
            graph,
            RoutingDirection::SouthToNorth,
            Some(&northern_source_layer),
            Some(&northern_target_layer),
            -node_spacing - graph.offset.y,
            edge_spacing,
        );
        if slots > 0 {
            let routing_height = node_spacing + slots.saturating_sub(1) as f64 * edge_spacing;
            graph.offset.y += routing_height;
            graph.size.height += routing_height;
        }
    }

    if !southern_source_layer.is_empty() {
        let slots = route_edges(
            graph,
            RoutingDirection::NorthToSouth,
            Some(&southern_source_layer),
            Some(&southern_target_layer),
            graph.size.height + node_spacing - graph.offset.y,
            edge_spacing,
        );
        if slots > 0 {
            graph.size.height += node_spacing + slots.saturating_sub(1) as f64 * edge_spacing;
        }
    }
}

fn push_unique(nodes: &mut Vec<usize>, node: usize) {
    if !nodes.contains(&node) {
        nodes.push(node);
    }
}

fn remove_temporary_north_south_dummies(graph: &mut LGraph) {
    let temporary_nodes = graph
        .layers
        .iter()
        .flat_map(|layer| layer.nodes.iter().copied())
        .filter(|node| {
            graph.layerless_nodes[*node].kind == LNodeKind::ExternalPort
                && graph.layerless_nodes[*node]
                    .replaced_external_port_dummy
                    .is_some()
        })
        .collect::<Vec<_>>();

    for node in &temporary_nodes {
        remove_temporary_north_south_dummy(graph, *node);
    }

    for node in temporary_nodes {
        graph.remove_node_from_layer(node);
    }
}

fn remove_temporary_north_south_dummy(graph: &mut LGraph, node: usize) {
    let Some(node_in_port) = first_port_on_side(graph, node, PortSide::West) else {
        return;
    };
    let Some(node_out_port) = first_port_on_side(graph, node, PortSide::East) else {
        return;
    };
    let Some(node_origin_port) = graph.layerless_nodes[node]
        .ports
        .iter()
        .position(|port| !matches!(port.side, PortSide::West | PortSide::East))
    else {
        return;
    };
    let Some(node_to_origin_edge) = graph.layerless_nodes[node].ports[node_origin_port]
        .outgoing_edges
        .first()
        .copied()
    else {
        return;
    };
    let Some(replaced_dummy) = graph.layerless_nodes[node].replaced_external_port_dummy else {
        return;
    };

    let mut incoming_edge_bend_points = graph.edges[node_to_origin_edge].bend_points.clone();
    incoming_edge_bend_points.insert(
        0,
        node_relative_port_position(graph, node, node_origin_port),
    );

    let mut outgoing_edge_bend_points = graph.edges[node_to_origin_edge].bend_points.clone();
    outgoing_edge_bend_points.reverse();
    outgoing_edge_bend_points.push(node_relative_port_position(graph, node, node_origin_port));

    let replaced_dummy_port = PortRef {
        node: replaced_dummy,
        port: 0,
    };

    let incoming_edges = graph.layerless_nodes[node].ports[node_in_port]
        .incoming_edges
        .clone();
    for edge in incoming_edges {
        graph.set_edge_target(edge, replaced_dummy_port);
        graph.edges[edge]
            .bend_points
            .extend(incoming_edge_bend_points.iter().copied());
    }

    let outgoing_edges = graph.layerless_nodes[node].ports[node_out_port]
        .outgoing_edges
        .clone();
    for edge in outgoing_edges {
        graph.set_edge_source(edge, replaced_dummy_port);
        graph.edges[edge]
            .bend_points
            .splice(0..0, outgoing_edge_bend_points.iter().copied());
    }

    graph.detach_edge(node_to_origin_edge);
}

fn node_relative_port_position(graph: &LGraph, node: usize, port: usize) -> LPoint {
    LPoint {
        x: graph.layerless_nodes[node].position.x
            + graph.layerless_nodes[node].ports[port].position.x,
        y: graph.layerless_nodes[node].position.y
            + graph.layerless_nodes[node].ports[port].position.y,
    }
}

fn fix_hierarchical_port_coordinates(graph: &mut LGraph) {
    fix_hierarchical_port_coordinates_for_layer(graph, 0);
    fix_hierarchical_port_coordinates_for_layer(graph, graph.layers.len() - 1);
}

fn fix_hierarchical_port_coordinates_for_layer(graph: &mut LGraph, layer_index: usize) {
    let nodes = graph.layers[layer_index].nodes.clone();
    let graph_actual_height = actual_graph_height(graph);
    let mut new_actual_graph_height = graph_actual_height;

    for node in &nodes {
        if graph.layerless_nodes[*node].kind != LNodeKind::ExternalPort {
            continue;
        }

        match graph.layerless_nodes[*node].external_port_side {
            PortSide::East => {
                graph.layerless_nodes[*node].position.x =
                    graph.size.width + graph.padding.right - graph.offset.x;
            }
            PortSide::West => {
                graph.layerless_nodes[*node].position.x = -graph.offset.x - graph.padding.left;
            }
            PortSide::North | PortSide::South | PortSide::Undefined => {}
        }

        if matches!(
            graph.layerless_nodes[*node].external_port_side,
            PortSide::East | PortSide::West
        ) {
            let required_actual_graph_height =
                fix_hierarchical_port_east_west_y_coordinate(graph, *node, graph_actual_height);
            new_actual_graph_height = new_actual_graph_height.max(required_actual_graph_height);
        }
    }

    graph.size.height += new_actual_graph_height - graph_actual_height;

    for node in nodes {
        if graph.layerless_nodes[node].kind != LNodeKind::ExternalPort {
            continue;
        }

        match graph.layerless_nodes[node].external_port_side {
            PortSide::North => {
                graph.layerless_nodes[node].position.y = -graph.offset.y - graph.padding.top;
            }
            PortSide::South => {
                graph.layerless_nodes[node].position.y =
                    graph.size.height + graph.padding.bottom - graph.offset.y;
            }
            PortSide::East | PortSide::West | PortSide::Undefined => {}
        }
    }
}

fn fix_hierarchical_port_east_west_y_coordinate(
    graph: &mut LGraph,
    node: usize,
    graph_actual_height: f64,
) -> f64 {
    let mut required_actual_graph_height = 0.0;

    if graph.options.port_constraints.is_ratio_fixed() {
        graph.layerless_nodes[node].position.y = graph_actual_height
            * graph.layerless_nodes[node].port_ratio_or_position
            - first_port_anchor_y(graph, node);
        required_actual_graph_height = graph.layerless_nodes[node].position.y
            + graph.layerless_nodes[node].external_port_size.height;
        border_to_content_area_y(graph, node);
    } else if graph.options.port_constraints.is_pos_fixed() {
        graph.layerless_nodes[node].position.y =
            graph.layerless_nodes[node].port_ratio_or_position - first_port_anchor_y(graph, node);
        required_actual_graph_height = graph.layerless_nodes[node].position.y
            + graph.layerless_nodes[node].external_port_size.height;
        border_to_content_area_y(graph, node);
    }

    required_actual_graph_height
}

fn correct_hierarchical_port_slanted_edge_segments(graph: &mut LGraph) {
    correct_hierarchical_port_slanted_edge_segments_for_layer(graph, 0);
    correct_hierarchical_port_slanted_edge_segments_for_layer(graph, graph.layers.len() - 1);
}

fn correct_hierarchical_port_slanted_edge_segments_for_layer(
    graph: &mut LGraph,
    layer_index: usize,
) {
    let nodes = graph.layers[layer_index].nodes.clone();

    for node in nodes {
        if graph.layerless_nodes[node].kind != LNodeKind::ExternalPort
            || !matches!(
                graph.layerless_nodes[node].external_port_side,
                PortSide::East | PortSide::West
            )
        {
            continue;
        }

        let connected_edges = graph.node_connected_edges(node);
        for edge in connected_edges {
            if graph.edges[edge].bend_points.is_empty() {
                continue;
            }

            let source = graph.edges[edge].source;
            if source.node == node {
                let source_y = absolute_port_anchor(graph, source).y;
                if let Some(first) = graph.edges[edge].bend_points.first_mut() {
                    first.y = source_y;
                }
            }

            let target = graph.edges[edge].target;
            if target.node == node {
                let target_y = absolute_port_anchor(graph, target).y;
                if let Some(last) = graph.edges[edge].bend_points.last_mut() {
                    last.y = target_y;
                }
            }
        }
    }
}

fn absolute_port_anchor(graph: &LGraph, port_ref: PortRef) -> LPoint {
    let node = &graph.layerless_nodes[port_ref.node];
    let port = &node.ports[port_ref.port];
    LPoint {
        x: node.position.x + port.position.x + port.anchor.x,
        y: node.position.y + port.position.y + port.anchor.y,
    }
}

pub fn process_hierarchical_port_constraints(graph: &mut LGraph) {
    process_east_west_hierarchical_port_constraints(graph);
    process_north_south_hierarchical_port_constraints(graph);
}

fn process_east_west_hierarchical_port_constraints(graph: &mut LGraph) {
    if !graph.options.port_constraints.is_order_fixed() || graph.layers.is_empty() {
        return;
    }

    process_east_west_hierarchical_port_constraints_for_layer(graph, 0);
    if graph.layers.len() > 1 {
        process_east_west_hierarchical_port_constraints_for_layer(graph, graph.layers.len() - 1);
    }
}

fn process_east_west_hierarchical_port_constraints_for_layer(
    graph: &mut LGraph,
    layer_index: usize,
) {
    let mut nodes = graph.layers[layer_index].nodes.clone();
    nodes.sort_by(|left, right| {
        let left_node = &graph.layerless_nodes[*left];
        let right_node = &graph.layerless_nodes[*right];

        match (
            left_node.kind == LNodeKind::ExternalPort,
            right_node.kind == LNodeKind::ExternalPort,
        ) {
            (_, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            (true, true) => left_node
                .port_ratio_or_position
                .total_cmp(&right_node.port_ratio_or_position),
        }
    });

    let mut last_hierarchical_dummy: Option<usize> = None;
    for node in nodes {
        if graph.layerless_nodes[node].kind != LNodeKind::ExternalPort {
            break;
        }
        if !matches!(
            graph.layerless_nodes[node].external_port_side,
            PortSide::West | PortSide::East
        ) {
            continue;
        }

        if let Some(previous) = last_hierarchical_dummy {
            graph.layerless_nodes[previous]
                .in_layer_successor_constraints
                .push(node);
        }
        last_hierarchical_dummy = Some(node);
    }
}

fn process_north_south_hierarchical_port_constraints(graph: &mut LGraph) {
    if !graph.options.port_constraints.is_side_fixed() {
        return;
    }

    let original_layer_count = graph.layers.len();
    let mut new_dummy_nodes = vec![Vec::<usize>::new(); original_layer_count + 2];
    let mut prev_maps = vec![Vec::<(usize, usize)>::new(); original_layer_count + 2];
    let mut next_maps = vec![Vec::<(usize, usize)>::new(); original_layer_count + 2];
    let mut original_external_port_dummies = Vec::new();

    for layer_index in 0..original_layer_count {
        let current_nodes = graph.layers[layer_index].nodes.clone();

        for current_node in current_nodes {
            if is_north_south_external_port_dummy(graph, current_node) {
                original_external_port_dummies.push(current_node);
                continue;
            }

            let incoming_edges = graph.node_incoming_edges(current_node);
            for edge in incoming_edges {
                let source_node = graph.edges[edge].source.node;
                if !is_north_south_external_port_dummy(graph, source_node) {
                    continue;
                }

                let replacement = replacement_dummy_for_original(
                    graph,
                    source_node,
                    layer_index,
                    &mut prev_maps,
                    &mut new_dummy_nodes,
                );
                graph.set_edge_source(
                    edge,
                    PortRef {
                        node: replacement,
                        port: 1,
                    },
                );
            }

            let outgoing_edges = graph.node_outgoing_edges(current_node);
            for edge in outgoing_edges {
                let target_node = graph.edges[edge].target.node;
                if !is_north_south_external_port_dummy(graph, target_node) {
                    continue;
                }

                let replacement = replacement_dummy_for_original(
                    graph,
                    target_node,
                    layer_index + 2,
                    &mut next_maps,
                    &mut new_dummy_nodes,
                );
                graph.set_edge_target(
                    edge,
                    PortRef {
                        node: replacement,
                        port: 0,
                    },
                );
            }
        }
    }

    for (virtual_layer, nodes) in new_dummy_nodes.into_iter().enumerate() {
        if nodes.is_empty() {
            continue;
        }

        let layer_index = if virtual_layer == 0 {
            graph.insert_layer(0);
            0
        } else if virtual_layer == original_layer_count + 1 {
            graph.layers.push(Default::default());
            graph.layers.len() - 1
        } else {
            virtual_layer - 1
        };

        for node in nodes {
            graph.set_node_layer(node, layer_index);
        }
    }

    for original_dummy in original_external_port_dummies {
        graph.remove_node_from_layer(original_dummy);
        if !graph
            .replaced_external_port_dummies
            .contains(&original_dummy)
        {
            graph.replaced_external_port_dummies.push(original_dummy);
        }
    }
}

fn is_north_south_external_port_dummy(graph: &LGraph, node: usize) -> bool {
    let node = &graph.layerless_nodes[node];
    node.kind == LNodeKind::ExternalPort
        && matches!(node.external_port_side, PortSide::North | PortSide::South)
}

fn replacement_dummy_for_original(
    graph: &mut LGraph,
    original_dummy: usize,
    virtual_layer: usize,
    maps: &mut [Vec<(usize, usize)>],
    new_dummy_nodes: &mut [Vec<usize>],
) -> usize {
    if let Some((_, replacement)) = maps[virtual_layer]
        .iter()
        .find(|(original, _)| *original == original_dummy)
    {
        return *replacement;
    }

    let replacement = create_hierarchical_port_replacement_dummy(graph, original_dummy);
    maps[virtual_layer].push((original_dummy, replacement));
    new_dummy_nodes[virtual_layer].push(replacement);
    replacement
}

fn create_hierarchical_port_replacement_dummy(graph: &mut LGraph, original_dummy: usize) -> usize {
    let original = graph.layerless_nodes[original_dummy].clone();
    let node_index = graph.layerless_nodes.len();
    let mut replacement = LNode::new(
        format!("{}:replacement:{node_index}", original.id),
        original.size.width,
        original.size.height,
        None,
    );
    replacement.kind = LNodeKind::ExternalPort;
    replacement.margin = original.margin;
    replacement.padding = original.padding;
    replacement.external_port_side = original.external_port_side;
    replacement.external_port_size = original.external_port_size;
    replacement.replaced_external_port_dummy = Some(original_dummy);
    replacement.port_constraints = PortConstraints::FixedPos;
    replacement.node_alignment = Alignment::Center;
    graph.layerless_nodes.push(replacement);

    graph
        .add_port(
            node_index,
            PortType::Input,
            PortSide::West,
            Default::default(),
        )
        .expect("replacement node was just inserted");
    graph
        .add_port(
            node_index,
            PortType::Output,
            PortSide::East,
            Default::default(),
        )
        .expect("replacement node was just inserted");
    node_index
}

fn create_long_edge_dummy_node(
    graph: &mut LGraph,
    target_layer_index: usize,
    edge_to_split: usize,
) -> usize {
    let dummy_index = graph.layerless_nodes.len();
    let mut dummy = LNode::new(
        format!("longEdge:{edge_to_split}:{target_layer_index}"),
        0.0,
        0.0,
        None,
    );
    dummy.kind = LNodeKind::LongEdge;
    dummy.origin_edge = Some(edge_to_split);
    dummy.port_constraints = PortConstraints::FixedPos;
    graph.layerless_nodes.push(dummy);
    graph.set_node_layer(dummy_index, target_layer_index);
    dummy_index
}

pub fn split_edge(graph: &mut LGraph, edge_index: usize, dummy_node: usize) -> Option<usize> {
    let old_edge_target = graph.edges.get(edge_index)?.target;
    let thickness = graph.edges[edge_index].thickness.max(0.0);
    graph.edges[edge_index].thickness = thickness;
    graph.layerless_nodes[dummy_node].size.height = thickness;
    let port_position = LPoint {
        x: 0.0,
        y: (thickness / 2.0).floor(),
    };

    let dummy_input = graph.add_port(dummy_node, PortType::Input, PortSide::West, port_position)?;
    let dummy_output =
        graph.add_port(dummy_node, PortType::Output, PortSide::East, port_position)?;

    if !graph.set_edge_target(edge_index, dummy_input) {
        return None;
    }

    let old_segment = graph.edges[edge_index].clone();
    let dummy_edge = LayeredEdge {
        id: old_segment.id.clone(),
        source: dummy_output,
        target: old_edge_target,
        source_node_id: old_segment.source_node_id.clone(),
        target_node_id: old_segment.target_node_id.clone(),
        labels: Vec::new(),
        minlen: old_segment.minlen,
        reversed: old_segment.reversed,
        bend_points: Vec::new(),
        model_order: old_segment.model_order,
        priority_direction: old_segment.priority_direction,
        priority_shortness: old_segment.priority_shortness,
        priority_straightness: old_segment.priority_straightness,
        thickness,
        original_opposite_port: old_segment.original_opposite_port,
    };
    let dummy_edge_index = graph.add_edge(dummy_edge)?;

    set_dummy_node_properties(graph, dummy_node, edge_index, dummy_edge_index);
    move_head_labels(graph, edge_index, dummy_edge_index);

    Some(dummy_edge_index)
}

fn set_dummy_node_properties(
    graph: &mut LGraph,
    dummy_node: usize,
    in_edge: usize,
    out_edge: usize,
) {
    let in_edge_source_node = graph.edges[in_edge].source.node;
    let out_edge_target_node = graph.edges[out_edge].target.node;

    if graph.layerless_nodes[in_edge_source_node].kind == LNodeKind::LongEdge {
        graph.layerless_nodes[dummy_node].long_edge_source =
            graph.layerless_nodes[in_edge_source_node].long_edge_source;
        graph.layerless_nodes[dummy_node].long_edge_target =
            graph.layerless_nodes[in_edge_source_node].long_edge_target;
        graph.layerless_nodes[dummy_node].long_edge_has_label_dummies =
            graph.layerless_nodes[in_edge_source_node].long_edge_has_label_dummies;
    } else if graph.layerless_nodes[in_edge_source_node].kind == LNodeKind::Label {
        graph.layerless_nodes[dummy_node].long_edge_source =
            graph.layerless_nodes[in_edge_source_node].long_edge_source;
        graph.layerless_nodes[dummy_node].long_edge_target =
            graph.layerless_nodes[in_edge_source_node].long_edge_target;
        graph.layerless_nodes[dummy_node].long_edge_has_label_dummies = true;
    } else if graph.layerless_nodes[out_edge_target_node].kind == LNodeKind::Label {
        graph.layerless_nodes[dummy_node].long_edge_source =
            graph.layerless_nodes[out_edge_target_node].long_edge_source;
        graph.layerless_nodes[dummy_node].long_edge_target =
            graph.layerless_nodes[out_edge_target_node].long_edge_target;
        graph.layerless_nodes[dummy_node].long_edge_has_label_dummies = true;
    } else {
        graph.layerless_nodes[dummy_node].long_edge_source = Some(graph.edges[in_edge].source);
        graph.layerless_nodes[dummy_node].long_edge_target = Some(graph.edges[out_edge].target);
    }
}

fn move_head_labels(graph: &mut LGraph, old_edge: usize, new_edge: usize) {
    let mut moved = Vec::new();
    let mut index = 0usize;

    while index < graph.edges[old_edge].labels.len() {
        if graph.edges[old_edge].labels[index].placement == EdgeLabelPlacement::Head {
            let mut label = graph.edges[old_edge].labels.remove(index);
            if label.end_label_edge.is_none() {
                label.end_label_edge = Some(old_edge);
            }
            moved.push(label);
        } else {
            index += 1;
        }
    }

    graph.edges[new_edge].labels.extend(moved);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{LLabel, LSize, PortType};
    use crate::importer::{ElkInputEdge, ElkInputGraph, ElkInputLabel, ElkInputNode, import_graph};
    use crate::options::{ElkDirection, LayerConstraint, LayeredOptions};
    use crate::p2layers::layer_network_simplex;

    fn node(id: &str) -> ElkInputNode {
        ElkInputNode {
            id: id.to_string(),
            width: 80.0,
            height: 40.0,
            parent: None,
            direction: None,
            hierarchy_handling: None,
            layer_constraint: None,
            label: None,
        }
    }

    fn edge(id: &str, source: &str, target: &str) -> ElkInputEdge {
        ElkInputEdge {
            id: id.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            label: None,
            minlen: 1,
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
        }
    }

    fn graph(nodes: Vec<ElkInputNode>, edges: Vec<ElkInputEdge>) -> LGraph {
        import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
            nodes,
            edges,
        })
        .unwrap()
    }

    #[test]
    fn edge_and_layer_constraint_reverser_makes_first_nodes_outgoing_only() {
        let mut first = node("start");
        first.layer_constraint = Some(LayerConstraint::First);
        let mut graph = graph(vec![node("A"), first], vec![edge("A-start", "A", "start")]);
        let start = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "start")
            .unwrap();

        reverse_edges_for_edge_and_layer_constraints(&mut graph);

        assert!(graph.edges[0].reversed);
        assert_eq!(graph.edges[0].source.node, start);
        assert!(graph.node_incoming_edges(start).is_empty());
        assert_eq!(graph.node_outgoing_edges(start), vec![0]);
    }

    #[test]
    fn edge_and_layer_constraint_reverser_makes_last_nodes_incoming_only() {
        let mut last = node("end");
        last.layer_constraint = Some(LayerConstraint::Last);
        let mut graph = graph(vec![last, node("A")], vec![edge("end-A", "end", "A")]);
        let end = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "end")
            .unwrap();

        reverse_edges_for_edge_and_layer_constraints(&mut graph);

        assert!(graph.edges[0].reversed);
        assert_eq!(graph.edges[0].target.node, end);
        assert_eq!(graph.node_incoming_edges(end), vec![0]);
        assert!(graph.node_outgoing_edges(end).is_empty());
    }

    #[test]
    fn long_edge_splitter_makes_layering_proper() {
        let mut graph = graph(
            vec![node("A"), node("B"), node("C"), node("D")],
            vec![
                edge("A-B", "A", "B"),
                edge("B-C", "B", "C"),
                edge("C-D", "C", "D"),
                edge("A-D", "A", "D"),
            ],
        );

        layer_network_simplex(&mut graph);
        split_long_edges(&mut graph);

        assert!(
            graph
                .layerless_nodes
                .iter()
                .any(|node| node.kind == LNodeKind::LongEdge)
        );
        for edge in &graph.edges {
            let source_layer = graph.layerless_nodes[edge.source.node].layer_index.unwrap();
            let target_layer = graph.layerless_nodes[edge.target.node].layer_index.unwrap();
            assert_eq!(
                target_layer,
                source_layer + 1,
                "edge {} should connect adjacent layers",
                edge.id
            );
        }
    }

    #[test]
    fn split_edge_moves_head_labels_to_new_segment_and_records_origin_ports() {
        let mut head = ElkInputLabel::center("head", 20.0, 10.0);
        head.placement = EdgeLabelPlacement::Head;
        let mut long = edge("A-C", "A", "C");
        long.label = Some(head);

        let mut graph = graph(
            vec![node("A"), node("B"), node("C")],
            vec![edge("A-B", "A", "B"), edge("B-C", "B", "C"), long],
        );

        layer_network_simplex(&mut graph);
        let long_edge = graph
            .edges
            .iter()
            .position(|edge| edge.id == "A-C")
            .unwrap();
        let original_source = graph.edges[long_edge].source;
        let original_target = graph.edges[long_edge].target;

        split_long_edges(&mut graph);

        let dummy = graph
            .layerless_nodes
            .iter()
            .position(|node| node.kind == LNodeKind::LongEdge)
            .unwrap();
        assert_eq!(graph.layerless_nodes[dummy].origin_edge, Some(long_edge));
        assert_eq!(
            graph.layerless_nodes[dummy].long_edge_source,
            Some(original_source)
        );
        assert_eq!(
            graph.layerless_nodes[dummy].long_edge_target,
            Some(original_target)
        );

        assert!(graph.edges[long_edge].labels.is_empty());
        let moved_label_edge = graph
            .edges
            .iter()
            .enumerate()
            .find(|(_, edge)| edge.labels.iter().any(|label| label.text == "head"))
            .map(|(index, _)| index)
            .unwrap();
        assert_ne!(moved_label_edge, long_edge);
        assert_eq!(
            graph.edges[moved_label_edge].labels[0].end_label_edge,
            Some(long_edge)
        );
    }

    #[test]
    fn label_dummy_inserter_splits_center_label_edges_and_moves_labels_to_dummy() {
        let mut center = ElkInputLabel::center("choice", 30.0, 12.0);
        center.placement = EdgeLabelPlacement::Center;
        let mut labelled = edge("A-B", "A", "B");
        labelled.label = Some(center);
        let mut graph = graph(vec![node("A"), node("B")], vec![labelled]);
        let edge_index = graph
            .edges
            .iter()
            .position(|edge| edge.id == "A-B")
            .unwrap();
        let original_source = graph.edges[edge_index].source;
        let original_target = graph.edges[edge_index].target;

        insert_label_dummies(&mut graph);

        let dummy = graph
            .layerless_nodes
            .iter()
            .position(|node| node.kind == LNodeKind::Label)
            .unwrap();
        assert_eq!(graph.layerless_nodes[dummy].origin_edge, Some(edge_index));
        assert_eq!(
            graph.layerless_nodes[dummy].long_edge_source,
            Some(original_source)
        );
        assert_eq!(
            graph.layerless_nodes[dummy].long_edge_target,
            Some(original_target)
        );
        assert_eq!(graph.layerless_nodes[dummy].labels.len(), 1);
        assert_eq!(graph.layerless_nodes[dummy].labels[0].text, "choice");
        assert!(graph.edges[edge_index].labels.is_empty());
        assert_eq!(graph.node_incoming_edges(dummy).len(), 1);
        assert_eq!(graph.node_outgoing_edges(dummy).len(), 1);
    }

    #[test]
    fn label_dummy_remover_restores_center_labels_to_origin_edge() {
        let mut center = ElkInputLabel::center("choice", 30.0, 12.0);
        center.placement = EdgeLabelPlacement::Center;
        let mut labelled = edge("A-C", "A", "C");
        labelled.label = Some(center);
        let mut graph = graph(
            vec![node("A"), node("B"), node("C")],
            vec![edge("A-B", "A", "B"), edge("B-C", "B", "C"), labelled],
        );
        let edge_index = graph
            .edges
            .iter()
            .position(|edge| edge.id == "A-C")
            .unwrap();

        insert_label_dummies(&mut graph);
        layer_network_simplex(&mut graph);
        split_long_edges(&mut graph);
        switch_label_dummies(&mut graph);
        select_label_sides(&mut graph);
        let dummy = graph
            .layerless_nodes
            .iter()
            .position(|node| node.kind == LNodeKind::Label)
            .unwrap();
        graph.layerless_nodes[dummy].position = LPoint { x: 20.0, y: 40.0 };

        remove_label_dummies(&mut graph);

        assert!(!graph.layers.iter().any(|layer| {
            layer
                .nodes
                .iter()
                .any(|node| graph.layerless_nodes[*node].kind == LNodeKind::Label)
        }));
        assert_eq!(graph.edges[edge_index].labels.len(), 1);
        assert_eq!(graph.edges[edge_index].labels[0].text, "choice");
        assert!(
            !graph.edge_source_attached(
                graph
                    .edges
                    .iter()
                    .enumerate()
                    .find(|(index, edge)| *index != edge_index && edge.id == "A-C")
                    .map(|(index, _)| index)
                    .unwrap()
            )
        );
    }

    #[test]
    fn end_label_preprocessor_moves_head_labels_to_port_cell_and_expands_margin() {
        let mut head = ElkInputLabel::center("head", 24.0, 10.0);
        head.placement = EdgeLabelPlacement::Head;
        head.inline = false;
        let mut labelled = edge("A-B", "A", "B");
        labelled.label = Some(head);
        let mut graph = graph(vec![node("A"), node("B")], vec![labelled]);

        layer_network_simplex(&mut graph);
        crate::p3order::process_port_sides(&mut graph);
        crate::p4nodes::calculate_label_and_node_sizes(&mut graph);
        crate::p4nodes::calculate_innermost_node_margins(&mut graph);
        select_label_sides(&mut graph);
        let target = graph.edges[0].target.node;
        let target_port = graph.edges[0].target.port;
        let old_left_margin = graph.layerless_nodes[target].margin.left;

        preprocess_end_labels(&mut graph);

        assert!(graph.edges[0].labels.is_empty());
        let port = &graph.layerless_nodes[target].ports[target_port];
        assert_eq!(port.labels.len(), 1);
        assert_eq!(port.labels[0].text, "head");
        assert_eq!(port.labels[0].end_label_edge, Some(0));
        assert!(port.end_label_cell.is_some());
        assert!(graph.layerless_nodes[target].margin.left > old_left_margin);
    }

    #[test]
    fn label_dummy_remover_centers_vertical_inline_labels_in_label_space() {
        let mut center = ElkInputLabel::center("choice", 30.0, 12.0);
        center.placement = EdgeLabelPlacement::Center;
        let mut labelled = edge("A-C", "A", "C");
        labelled.label = Some(center);
        let mut graph = graph(
            vec![node("A"), node("B"), node("C")],
            vec![edge("A-B", "A", "B"), edge("B-C", "B", "C"), labelled],
        );
        let edge_index = graph
            .edges
            .iter()
            .position(|edge| edge.id == "A-C")
            .unwrap();

        insert_label_dummies(&mut graph);
        layer_network_simplex(&mut graph);
        split_long_edges(&mut graph);
        switch_label_dummies(&mut graph);
        select_label_sides(&mut graph);
        let dummy = graph
            .layerless_nodes
            .iter()
            .position(|node| node.kind == LNodeKind::Label)
            .unwrap();
        graph.layerless_nodes[dummy].position = LPoint { x: 20.0, y: 40.0 };
        graph.layerless_nodes[dummy].size.height = 14.0;
        graph.layerless_nodes[dummy].labels.push(LLabel {
            text: "short".to_string(),
            size: LSize {
                width: 12.0,
                height: 4.0,
            },
            position: LPoint::default(),
            placement: EdgeLabelPlacement::Center,
            inline: true,
            label_side: None,
            end_label_edge: None,
        });

        remove_label_dummies(&mut graph);

        let labels = &graph.edges[edge_index].labels;
        assert_eq!(labels.len(), 2);
        assert_eq!(labels[0].position.y, 41.0);
        assert_eq!(labels[1].position.y, 45.0);
    }

    #[test]
    fn long_edge_joiner_merges_split_segments_and_removes_dummy_from_layer() {
        let mut graph = graph(
            vec![node("A"), node("B"), node("C")],
            vec![
                edge("A-B", "A", "B"),
                edge("B-C", "B", "C"),
                edge("A-C", "A", "C"),
            ],
        );
        graph.options.unnecessary_bendpoints = true;
        layer_network_simplex(&mut graph);

        let long_edge = graph
            .edges
            .iter()
            .position(|edge| edge.id == "A-C")
            .unwrap();
        split_long_edges(&mut graph);

        let dummy = graph
            .layerless_nodes
            .iter()
            .position(|node| node.kind == LNodeKind::LongEdge)
            .unwrap();
        graph.layerless_nodes[dummy].position = LPoint { x: 50.0, y: 70.0 };
        graph.edges[long_edge]
            .bend_points
            .push(LPoint { x: 10.0, y: 20.0 });
        let dropped = graph
            .edges
            .iter()
            .enumerate()
            .find(|(index, edge)| *index != long_edge && edge.id == "A-C")
            .map(|(index, _)| index)
            .unwrap();
        graph.edges[dropped]
            .bend_points
            .push(LPoint { x: 90.0, y: 100.0 });
        let mut label = LLabel::new("head", 20.0, 10.0);
        label.placement = EdgeLabelPlacement::Head;
        graph.edges[dropped].labels.push(label);
        let original_target = graph.edges[dropped].target;
        let tail_edge = graph
            .edges
            .iter()
            .enumerate()
            .find(|(index, edge)| *index != long_edge && *index != dropped && edge.id == "A-B")
            .map(|(index, _)| index)
            .unwrap();
        graph.set_edge_target(tail_edge, original_target);
        graph.layerless_nodes[original_target.node].ports[original_target.port].incoming_edges =
            vec![dropped, tail_edge];

        join_long_edges(&mut graph);

        assert_eq!(graph.edges[long_edge].target, original_target);
        assert!(graph.edge_source_attached(long_edge));
        assert!(graph.edge_target_attached(long_edge));
        assert!(!graph.edge_source_attached(dropped));
        assert!(!graph.edge_target_attached(dropped));
        assert!(
            !graph
                .layers
                .iter()
                .any(|layer| layer.nodes.contains(&dummy))
        );
        assert_eq!(
            graph.edges[long_edge].bend_points,
            vec![
                LPoint { x: 10.0, y: 20.0 },
                LPoint { x: 50.0, y: 70.0 },
                LPoint { x: 90.0, y: 100.0 }
            ]
        );
        assert_eq!(graph.edges[long_edge].labels.len(), 1);
        assert!(graph.edges[dropped].labels.is_empty());
        assert_eq!(
            graph.layerless_nodes[original_target.node].ports[original_target.port].incoming_edges,
            vec![long_edge, tail_edge]
        );
    }

    #[test]
    fn layer_constraint_preprocessor_hides_separate_nodes_before_layering() {
        let mut first = node("start");
        first.layer_constraint = Some(LayerConstraint::FirstSeparate);
        let mut graph = graph(
            vec![first, node("A"), node("B")],
            vec![edge("start-A", "start", "A"), edge("A-B", "A", "B")],
        );
        let edge_index = graph
            .edges
            .iter()
            .position(|edge| edge.id == "start-A")
            .unwrap();

        preprocess_layer_constraints(&mut graph).unwrap();

        let hidden = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "start")
            .unwrap();
        let a = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "A")
            .unwrap();
        assert!(graph.layerless_nodes[hidden].hidden);
        assert_eq!(graph.hidden_nodes, vec![hidden]);
        assert_eq!(
            graph.layerless_nodes[a].layer_constraint,
            LayerConstraint::None
        );
        assert!(
            !graph.edge_target_attached(edge_index),
            "preprocessor should detach the opposite target endpoint"
        );
        assert!(graph.edge_source_attached(edge_index));
    }

    #[test]
    fn layer_constraint_preprocessor_constrains_isolated_opposite_node() {
        let mut first = node("start");
        first.layer_constraint = Some(LayerConstraint::FirstSeparate);
        let mut graph = graph(vec![first, node("A")], vec![edge("start-A", "start", "A")]);

        preprocess_layer_constraints(&mut graph).unwrap();

        let a = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "A")
            .unwrap();
        assert_eq!(
            graph.layerless_nodes[a].layer_constraint,
            LayerConstraint::First
        );
    }

    #[test]
    fn layer_constraint_preprocessor_keeps_explicit_opposite_constraint() {
        let mut first = node("start");
        first.layer_constraint = Some(LayerConstraint::FirstSeparate);
        let mut explicit = node("A");
        explicit.layer_constraint = Some(LayerConstraint::None);
        let mut graph = graph(vec![first, explicit], vec![edge("start-A", "start", "A")]);

        preprocess_layer_constraints(&mut graph).unwrap();

        let a = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "A")
            .unwrap();
        assert!(graph.layerless_nodes[a].layer_constraint_explicit);
        assert_eq!(
            graph.layerless_nodes[a].layer_constraint,
            LayerConstraint::None
        );
    }

    #[test]
    fn layer_constraint_preprocessor_keeps_node_connected_to_both_separate_sides_unconstrained() {
        let mut first = node("start");
        first.layer_constraint = Some(LayerConstraint::FirstSeparate);
        let mut last = node("end");
        last.layer_constraint = Some(LayerConstraint::LastSeparate);
        let mut graph = graph(
            vec![first, node("A"), last],
            vec![edge("start-A", "start", "A"), edge("A-end", "A", "end")],
        );

        preprocess_layer_constraints(&mut graph).unwrap();

        let a = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "A")
            .unwrap();
        assert_eq!(
            graph.layerless_nodes[a].layer_constraint,
            LayerConstraint::None
        );
    }

    #[test]
    fn layer_constraint_postprocessor_restores_hidden_nodes_in_separate_layer() {
        let mut first = node("start");
        first.layer_constraint = Some(LayerConstraint::FirstSeparate);
        let mut graph = graph(
            vec![first, node("A"), node("B")],
            vec![edge("start-A", "start", "A"), edge("A-B", "A", "B")],
        );
        let edge_index = graph
            .edges
            .iter()
            .position(|edge| edge.id == "start-A")
            .unwrap();

        preprocess_layer_constraints(&mut graph).unwrap();
        layer_network_simplex(&mut graph);
        postprocess_layer_constraints(&mut graph).unwrap();

        let start = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "start")
            .unwrap();
        let a = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "A")
            .unwrap();
        assert!(!graph.layerless_nodes[start].hidden);
        assert!(graph.hidden_nodes.is_empty());
        assert_eq!(graph.layerless_nodes[start].layer_index, Some(0));
        assert!(
            graph.layerless_nodes[a].layer_index.unwrap()
                > graph.layerless_nodes[start].layer_index.unwrap()
        );
        assert!(graph.edge_source_attached(edge_index));
        assert!(graph.edge_target_attached(edge_index));
    }

    #[test]
    fn layer_constraint_postprocessor_restores_hidden_incoming_edge_source() {
        let mut last = node("end");
        last.layer_constraint = Some(LayerConstraint::LastSeparate);
        let mut graph = graph(vec![node("A"), last], vec![edge("A-end", "A", "end")]);
        let edge_index = graph
            .edges
            .iter()
            .position(|edge| edge.id == "A-end")
            .unwrap();

        preprocess_layer_constraints(&mut graph).unwrap();
        assert!(!graph.edge_source_attached(edge_index));
        assert!(graph.edge_target_attached(edge_index));

        layer_network_simplex(&mut graph);
        postprocess_layer_constraints(&mut graph).unwrap();

        let end = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "end")
            .unwrap();
        assert_eq!(
            graph.layerless_nodes[end].layer_index,
            Some(graph.layers.len() - 1)
        );
        assert!(graph.edge_source_attached(edge_index));
        assert!(graph.edge_target_attached(edge_index));
    }

    #[test]
    fn layer_constraint_preprocessor_rejects_invalid_separate_edges() {
        let mut first = node("start");
        first.layer_constraint = Some(LayerConstraint::FirstSeparate);
        let mut graph = graph(vec![node("A"), first], vec![edge("A-start", "A", "start")]);

        let err = preprocess_layer_constraints(&mut graph).unwrap_err();
        assert!(matches!(
            err,
            IntermediateError::FirstSeparateIncomingEdge { .. }
        ));
    }

    #[test]
    fn layer_constraint_preprocessor_allows_external_port_incident_exception() {
        let mut first = node("start");
        first.layer_constraint = Some(LayerConstraint::FirstSeparate);
        let mut graph = graph(vec![node("A"), first], vec![edge("A-start", "A", "start")]);
        for node in &mut graph.layerless_nodes {
            node.kind = LNodeKind::ExternalPort;
        }

        preprocess_layer_constraints(&mut graph).unwrap();
    }

    #[test]
    fn layer_constraint_postprocessor_rejects_invalid_first_incoming_edge() {
        let mut first = node("start");
        first.layer_constraint = Some(LayerConstraint::First);
        let mut graph = graph(vec![node("A"), first], vec![edge("A-start", "A", "start")]);
        layer_network_simplex(&mut graph);

        let err = postprocess_layer_constraints(&mut graph).unwrap_err();
        assert!(matches!(err, IntermediateError::FirstIncomingEdge { .. }));
    }

    #[test]
    fn layer_size_calculator_sets_layer_sizes_graph_height_and_vertical_offset() {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::default(),
            nodes: vec![node("A"), node("B")],
            edges: Vec::new(),
        })
        .unwrap();
        graph.layerless_nodes[0].size.width = 30.0;
        graph.layerless_nodes[0].size.height = 10.0;
        graph.layerless_nodes[0].position.y = -5.0;
        graph.layerless_nodes[0].margin.top = 3.0;
        graph.layerless_nodes[0].margin.right = 7.0;
        graph.layerless_nodes[0].margin.bottom = 11.0;
        graph.layerless_nodes[0].margin.left = 5.0;
        graph.layerless_nodes[1].size.width = 40.0;
        graph.layerless_nodes[1].size.height = 20.0;
        graph.layerless_nodes[1].position.y = 25.0;
        graph.layerless_nodes[1].margin.top = 2.0;
        graph.layerless_nodes[1].margin.right = 3.0;
        graph.layerless_nodes[1].margin.bottom = 4.0;
        graph.layerless_nodes[1].margin.left = 1.0;
        graph.layers.push(crate::graph::Layer {
            nodes: vec![0, 1],
            size: Default::default(),
        });

        calculate_layer_sizes_and_graph_height(&mut graph);

        assert_eq!(graph.layers[0].size.width, 44.0);
        assert_eq!(graph.layers[0].size.height, 57.0);
        assert_eq!(graph.size.height, 57.0);
        assert_eq!(graph.offset.y, 8.0);
    }

    #[test]
    fn layer_size_calculator_includes_surrounding_spacing_for_external_port_bounds() {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::default(),
            nodes: vec![node("port")],
            edges: Vec::new(),
        })
        .unwrap();
        graph.options.spacing.ports_surrounding.top = 6.0;
        graph.options.spacing.ports_surrounding.bottom = 8.0;
        graph.layerless_nodes[0].kind = LNodeKind::ExternalPort;
        graph.layerless_nodes[0].size.height = 12.0;
        graph.layerless_nodes[0].position.y = 10.0;
        graph.layerless_nodes[0].margin.top = 1.0;
        graph.layerless_nodes[0].margin.bottom = 2.0;
        graph.layers.push(crate::graph::Layer {
            nodes: vec![0],
            size: Default::default(),
        });

        calculate_layer_sizes_and_graph_height(&mut graph);

        assert_eq!(graph.layers[0].size.height, 29.0);
        assert_eq!(graph.size.height, 29.0);
        assert_eq!(graph.offset.y, -3.0);
    }

    #[test]
    fn hierarchical_port_constraint_processor_orders_east_west_dummies() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        graph.options.port_constraints = PortConstraints::FixedOrder;
        let west_late = push_external_dummy(&mut graph, "west-late", PortSide::West, 0.8);
        let west_early = push_external_dummy(&mut graph, "west-early", PortSide::West, 0.2);
        let normal = push_normal_node(&mut graph, "A");

        graph.set_node_layer(west_late, 0);
        graph.set_node_layer(west_early, 0);
        graph.set_node_layer(normal, 0);

        process_hierarchical_port_constraints(&mut graph);

        assert_eq!(
            graph.layerless_nodes[west_early].in_layer_successor_constraints,
            vec![west_late]
        );
        assert!(
            graph.layerless_nodes[west_late]
                .in_layer_successor_constraints
                .is_empty()
        );
    }

    #[test]
    fn hierarchical_port_constraint_processor_replaces_north_south_dummies() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        graph.options.port_constraints = PortConstraints::FixedPos;
        let north = push_external_dummy(&mut graph, "north", PortSide::North, 5.0);
        let a = push_normal_node(&mut graph, "A");
        let b = push_normal_node(&mut graph, "B");

        graph.set_node_layer(a, 0);
        graph.set_node_layer(north, 0);
        graph.set_node_layer(b, 1);

        let in_edge = add_test_edge(&mut graph, "north-A", north, 0, a, 0);
        let out_edge = add_test_edge(&mut graph, "B-north", b, 0, north, 0);

        process_hierarchical_port_constraints(&mut graph);

        assert_eq!(graph.layerless_nodes[north].layer_index, None);
        assert_eq!(graph.replaced_external_port_dummies, vec![north]);
        let replacements = graph
            .layerless_nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| {
                (node.replaced_external_port_dummy == Some(north)).then_some(index)
            })
            .collect::<Vec<_>>();
        assert_eq!(replacements.len(), 2);
        assert_eq!(graph.edges[in_edge].source.node, replacements[0]);
        assert_eq!(graph.edges[in_edge].source.port, 1);
        assert_eq!(graph.edges[out_edge].target.node, replacements[1]);
        assert_eq!(graph.edges[out_edge].target.port, 0);
        assert!(graph.edge_source_attached(in_edge));
        assert!(graph.edge_target_attached(out_edge));
        assert_eq!(
            graph.layerless_nodes[replacements[0]].port_constraints,
            PortConstraints::FixedPos
        );
        assert_eq!(
            graph.layerless_nodes[replacements[0]].node_alignment,
            Alignment::Center
        );
    }

    #[test]
    fn hierarchical_port_dummy_size_processor_sizes_north_south_dummies_per_layer() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        graph.options.spacing.edge_edge_between_layers = 7.0;

        let north_a = push_replaced_north_south_dummy(&mut graph, "north-a", PortSide::North);
        let north_b = push_replaced_north_south_dummy(&mut graph, "north-b", PortSide::North);
        let south_a = push_replaced_north_south_dummy(&mut graph, "south-a", PortSide::South);
        let south_b = push_replaced_north_south_dummy(&mut graph, "south-b", PortSide::South);

        graph.set_node_layer(north_a, 0);
        graph.set_node_layer(north_b, 0);
        graph.set_node_layer(south_a, 0);
        graph.set_node_layer(south_b, 0);

        process_hierarchical_port_dummy_sizes(&mut graph);

        assert_eq!(
            graph.layerless_nodes[north_a].node_alignment,
            Alignment::Center
        );
        assert_eq!(
            graph.layerless_nodes[north_b].node_alignment,
            Alignment::Center
        );
        assert_eq!(
            graph.layerless_nodes[south_a].node_alignment,
            Alignment::Center
        );
        assert_eq!(
            graph.layerless_nodes[south_b].node_alignment,
            Alignment::Center
        );
        assert_eq!(graph.layerless_nodes[north_a].size.width, 0.0);
        assert_eq!(graph.layerless_nodes[north_b].size.width, 14.0);
        assert_eq!(graph.layerless_nodes[south_a].size.width, 14.0);
        assert_eq!(graph.layerless_nodes[south_b].size.width, 0.0);
        assert_eq!(graph.layerless_nodes[north_a].ports[0].position.x, 0.0);
        assert_eq!(graph.layerless_nodes[north_b].ports[0].position.x, 0.0);
        assert_eq!(graph.layerless_nodes[north_b].ports[1].position.x, 14.0);
        assert_eq!(graph.layerless_nodes[south_a].ports[1].position.x, 14.0);
    }

    #[test]
    fn hierarchical_port_position_processor_sets_fixed_ratio_east_west_y_coordinates() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        graph.options.port_constraints = PortConstraints::FixedRatio;
        graph.size.height = 100.0;
        graph.padding.top = 10.0;
        graph.padding.bottom = 20.0;
        graph.offset.y = 7.0;

        let west = push_external_dummy(&mut graph, "west", PortSide::West, 0.25);
        let east = push_external_dummy(&mut graph, "east", PortSide::East, 0.5);
        let north = push_external_dummy(&mut graph, "north", PortSide::North, 0.75);
        graph.layerless_nodes[west].ports[0].position.y = 3.0;
        graph.layerless_nodes[east].ports[0].position.y = 4.0;
        graph.layerless_nodes[north].ports[0].position.y = 5.0;

        graph.set_node_layer(west, 0);
        graph.set_node_layer(north, 0);
        graph.set_node_layer(east, 1);

        process_hierarchical_port_positions(&mut graph);

        assert_eq!(graph.layerless_nodes[west].position.y, 12.5);
        assert_eq!(graph.layerless_nodes[east].position.y, 44.0);
        assert_eq!(graph.layerless_nodes[north].position.y, 0.0);
    }

    #[test]
    fn hierarchical_port_position_processor_sets_fixed_pos_only_on_border_layers() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        graph.options.port_constraints = PortConstraints::FixedPos;
        graph.padding.top = 6.0;
        graph.offset.y = -2.0;

        let first = push_external_dummy(&mut graph, "first", PortSide::West, 30.0);
        let middle = push_external_dummy(&mut graph, "middle", PortSide::West, 90.0);
        let last = push_external_dummy(&mut graph, "last", PortSide::East, 50.0);
        graph.layerless_nodes[first].ports[0].position.y = 1.0;
        graph.layerless_nodes[middle].ports[0].position.y = 2.0;
        graph.layerless_nodes[last].ports[0].position.y = 3.0;

        graph.set_node_layer(first, 0);
        graph.set_node_layer(middle, 1);
        graph.set_node_layer(last, 2);

        process_hierarchical_port_positions(&mut graph);

        assert_eq!(graph.layerless_nodes[first].position.y, 25.0);
        assert_eq!(graph.layerless_nodes[middle].position.y, 0.0);
        assert_eq!(graph.layerless_nodes[last].position.y, 43.0);
    }

    #[test]
    fn hierarchical_port_orthogonal_router_tail_fixes_east_west_coordinates_and_slanted_segments() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        graph.options.port_constraints = PortConstraints::FixedPos;
        graph.size.width = 100.0;
        graph.size.height = 40.0;
        graph.padding.left = 5.0;
        graph.padding.right = 7.0;
        graph.padding.top = 3.0;
        graph.padding.bottom = 11.0;
        graph.offset.x = 2.0;
        graph.offset.y = 4.0;

        let west = push_external_dummy(&mut graph, "west", PortSide::West, 22.0);
        let east = push_external_dummy(&mut graph, "east", PortSide::East, 70.0);
        let middle = push_normal_node(&mut graph, "A");
        graph.layerless_nodes[west].external_port_size.height = 6.0;
        graph.layerless_nodes[east].external_port_size.height = 8.0;
        graph.layerless_nodes[west].ports[0].position.y = 2.0;
        graph.layerless_nodes[east].ports[0].position.y = 5.0;

        graph.set_node_layer(west, 0);
        graph.set_node_layer(middle, 1);
        graph.set_node_layer(east, 2);

        let west_edge = add_test_edge(&mut graph, "west-A", west, 0, middle, 0);
        graph.edges[west_edge].bend_points = vec![LPoint { x: 10.0, y: 999.0 }];
        let east_edge = add_test_edge(&mut graph, "A-east", middle, 0, east, 0);
        graph.edges[east_edge].bend_points = vec![LPoint { x: 90.0, y: 999.0 }];

        process_hierarchical_port_orthogonal_edges(&mut graph);

        assert_eq!(graph.layerless_nodes[west].position.x, -7.0);
        assert_eq!(graph.layerless_nodes[east].position.x, 105.0);
        assert_eq!(graph.layerless_nodes[west].position.y, 13.0);
        assert_eq!(graph.layerless_nodes[east].position.y, 58.0);
        assert_eq!(graph.size.height, 59.0);
        assert_eq!(graph.edges[west_edge].bend_points[0].y, 15.0);
        assert_eq!(graph.edges[east_edge].bend_points[0].y, 63.0);
    }

    #[test]
    fn hierarchical_port_orthogonal_router_tail_sets_north_south_border_y_coordinates() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        graph.size.height = 80.0;
        graph.padding.top = 6.0;
        graph.padding.bottom = 9.0;
        graph.offset.y = 4.0;

        let north = push_replaced_north_south_dummy(&mut graph, "north", PortSide::North);
        let south = push_replaced_north_south_dummy(&mut graph, "south", PortSide::South);
        graph.set_node_layer(north, 0);
        graph.set_node_layer(south, 1);

        process_hierarchical_port_orthogonal_edges(&mut graph);

        assert_eq!(graph.layerless_nodes[north].position.y, -10.0);
        assert_eq!(graph.layerless_nodes[south].position.y, 85.0);
    }

    #[test]
    fn hierarchical_port_orthogonal_router_restores_and_removes_north_dummy_replacements() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        graph.options.port_constraints = PortConstraints::FixedSide;
        graph.options.spacing.node_node = 20.0;
        graph.options.spacing.edge_edge = 10.0;
        graph.size.height = 50.0;

        let north = push_external_dummy(&mut graph, "north", PortSide::North, 0.0);
        graph.layerless_nodes[north].size.width = 4.0;
        graph.layerless_nodes[north].ports[0].position = LPoint { x: 2.0, y: 0.0 };
        let source = push_normal_node(&mut graph, "A");
        graph.layerless_nodes[source].position = LPoint { x: 50.0, y: 30.0 };
        let source_port = graph
            .add_port(
                source,
                PortType::Output,
                PortSide::North,
                LPoint { x: 30.0, y: 0.0 },
            )
            .unwrap();

        graph.set_node_layer(source, 0);
        graph.set_node_layer(north, 0);
        let edge = graph
            .add_edge(LayeredEdge {
                id: "A-north".to_string(),
                source: source_port,
                target: PortRef {
                    node: north,
                    port: 0,
                },
                source_node_id: "A".to_string(),
                target_node_id: "north".to_string(),
                labels: Vec::new(),
                minlen: 1,
                reversed: false,
                bend_points: Vec::new(),
                model_order: None,
                priority_direction: 0,
                priority_shortness: 0,
                priority_straightness: 0,
                thickness: 0.0,
                original_opposite_port: None,
            })
            .unwrap();

        process_hierarchical_port_constraints(&mut graph);
        let replacement = graph
            .layerless_nodes
            .iter()
            .enumerate()
            .find_map(|(index, node)| {
                (node.replaced_external_port_dummy == Some(north)).then_some(index)
            })
            .unwrap();
        process_hierarchical_port_orthogonal_edges(&mut graph);

        assert_eq!(graph.layerless_nodes[north].layer_index, Some(1));
        assert_eq!(graph.layerless_nodes[replacement].layer_index, None);
        assert_eq!(graph.layerless_nodes[north].ports[0].side, PortSide::South);
        assert_eq!(graph.edges[edge].target.node, north);
        assert_eq!(graph.edges[edge].target.port, 0);
        assert!(graph.edge_source_attached(edge));
        assert!(graph.edge_target_attached(edge));
        let detached_edge = graph
            .edges
            .iter()
            .position(|edge| edge.id.ends_with(":hierarchical-origin"))
            .unwrap();
        assert!(!graph.edge_source_attached(detached_edge));
        assert!(!graph.edge_target_attached(detached_edge));
    }

    #[test]
    fn hierarchical_port_orthogonal_router_places_fixed_ratio_north_south_dummies() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        graph.options.port_constraints = PortConstraints::FixedRatio;
        graph.size.width = 100.0;
        graph.padding.left = 10.0;
        graph.padding.right = 30.0;
        graph.offset.x = 4.0;

        let north = push_external_dummy(&mut graph, "north", PortSide::North, 0.25);
        let south = push_external_dummy(&mut graph, "south", PortSide::South, 0.5);
        graph.layerless_nodes[north].ports[0].position.x = 2.0;
        graph.layerless_nodes[south].ports[0].position.x = 3.0;
        graph.replaced_external_port_dummies = vec![north, south];
        graph.layers.push(crate::graph::Layer {
            nodes: vec![],
            size: Default::default(),
        });

        process_hierarchical_port_orthogonal_edges(&mut graph);

        assert_eq!(graph.layerless_nodes[north].position.x, 19.0);
        assert_eq!(graph.layerless_nodes[south].position.x, 53.0);
    }

    #[test]
    fn hierarchical_port_orthogonal_router_routes_fixed_pos_north_edges_above_graph() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        graph.options.port_constraints = PortConstraints::FixedPos;
        graph.options.spacing.node_node = 20.0;
        graph.options.spacing.edge_edge = 10.0;
        graph.size.height = 50.0;

        let north = push_external_dummy(&mut graph, "north", PortSide::North, 5.0);
        graph.layerless_nodes[north].size.width = 4.0;
        graph.layerless_nodes[north].ports[0].position = LPoint { x: 2.0, y: 0.0 };
        let source = push_normal_node(&mut graph, "A");
        let source_port = graph
            .add_port(
                source,
                PortType::Output,
                PortSide::North,
                LPoint { x: 30.0, y: 0.0 },
            )
            .unwrap();

        graph.set_node_layer(source, 0);
        graph.set_node_layer(north, 0);
        graph
            .add_edge(LayeredEdge {
                id: "A-north".to_string(),
                source: source_port,
                target: PortRef {
                    node: north,
                    port: 0,
                },
                source_node_id: "A".to_string(),
                target_node_id: "north".to_string(),
                labels: Vec::new(),
                minlen: 1,
                reversed: false,
                bend_points: Vec::new(),
                model_order: None,
                priority_direction: 0,
                priority_shortness: 0,
                priority_straightness: 0,
                thickness: 0.0,
                original_opposite_port: None,
            })
            .unwrap();

        process_hierarchical_port_constraints(&mut graph);
        process_hierarchical_port_orthogonal_edges(&mut graph);

        assert_eq!(graph.offset.y, 20.0);
        assert!(graph.size.height > 50.0);
        let origin_edge = graph
            .edges
            .iter()
            .find(|edge| edge.id.ends_with(":hierarchical-origin"))
            .unwrap();
        assert!(!origin_edge.bend_points.is_empty());
    }

    #[test]
    fn hierarchical_port_orthogonal_router_restores_fixed_order_north_south_spacing() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        graph.options.port_constraints = PortConstraints::FixedOrder;
        graph.options.spacing.port_port = 7.0;

        let late = push_external_dummy(&mut graph, "late", PortSide::North, 0.8);
        let early = push_external_dummy(&mut graph, "early", PortSide::North, 0.2);
        graph.layerless_nodes[late].position.x = 0.0;
        graph.layerless_nodes[early].position.x = 0.0;
        graph.layerless_nodes[late].size.width = 5.0;
        graph.layerless_nodes[early].size.width = 5.0;
        graph.replaced_external_port_dummies = vec![late, early];
        graph.layers.push(crate::graph::Layer {
            nodes: vec![],
            size: Default::default(),
        });

        process_hierarchical_port_orthogonal_edges(&mut graph);

        assert_eq!(graph.layerless_nodes[early].position.x, 0.0);
        assert_eq!(graph.layerless_nodes[late].position.x, 12.0);
    }

    fn push_replaced_north_south_dummy(
        graph: &mut LGraph,
        id: &str,
        external_side: PortSide,
    ) -> usize {
        let node = graph.layerless_nodes.len();
        let mut dummy = LNode::new(id, 0.0, 0.0, None);
        dummy.kind = LNodeKind::ExternalPort;
        dummy.external_port_side = external_side;
        graph.layerless_nodes.push(dummy);
        graph
            .add_port(node, PortType::Input, PortSide::West, Default::default())
            .unwrap();
        graph
            .add_port(node, PortType::Output, PortSide::East, Default::default())
            .unwrap();
        node
    }

    fn push_external_dummy(
        graph: &mut LGraph,
        id: &str,
        external_side: PortSide,
        ratio_or_position: f64,
    ) -> usize {
        let node = graph.layerless_nodes.len();
        let mut dummy = LNode::new(id, 0.0, 0.0, None);
        dummy.kind = LNodeKind::ExternalPort;
        dummy.external_port_side = external_side;
        dummy.port_ratio_or_position = ratio_or_position;
        graph.layerless_nodes.push(dummy);
        graph
            .add_port(node, PortType::Input, PortSide::West, Default::default())
            .unwrap();
        node
    }

    fn push_normal_node(graph: &mut LGraph, id: &str) -> usize {
        let node = graph.layerless_nodes.len();
        graph.layerless_nodes.push(LNode::new(id, 80.0, 40.0, None));
        graph
            .add_port(node, PortType::Input, PortSide::West, Default::default())
            .unwrap();
        node
    }

    fn add_test_edge(
        graph: &mut LGraph,
        id: &str,
        source_node: usize,
        source_port: usize,
        target_node: usize,
        target_port: usize,
    ) -> usize {
        graph
            .add_edge(LayeredEdge {
                id: id.to_string(),
                source: PortRef {
                    node: source_node,
                    port: source_port,
                },
                target: PortRef {
                    node: target_node,
                    port: target_port,
                },
                source_node_id: graph.layerless_nodes[source_node].id.clone(),
                target_node_id: graph.layerless_nodes[target_node].id.clone(),
                labels: Vec::new(),
                minlen: 1,
                reversed: false,
                bend_points: Vec::new(),
                model_order: None,
                priority_direction: 0,
                priority_shortness: 0,
                priority_straightness: 0,
                thickness: 0.0,
                original_opposite_port: None,
            })
            .unwrap()
    }
}
