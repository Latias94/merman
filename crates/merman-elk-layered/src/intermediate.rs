//! Intermediate layered processors.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/ReversedEdgeRestorer.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/EdgeAndLayerConstraintEdgeReverser.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/LayerConstraintPreprocessor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/LayerConstraintPostprocessor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/LongEdgeSplitter.java

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
        graph.layers.push(crate::graph::Layer { nodes: Vec::new() });
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
        graph.layers.push(crate::graph::Layer { nodes: Vec::new() });
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
}
