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

use crate::graph::{
    EdgeLabelPlacement, LGraph, LNode, LNodeKind, LPoint, LayeredEdge, PortSide, PortType,
    reverse_edge,
};
use crate::options::{LayerConstraint, PortConstraints};

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
    use crate::graph::LLabel;
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
}
