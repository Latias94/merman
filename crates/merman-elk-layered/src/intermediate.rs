//! Intermediate layered processors.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/ReversedEdgeRestorer.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/LongEdgeSplitter.java

use crate::graph::{
    EdgeLabelPlacement, LGraph, LNode, LNodeKind, LPoint, LayeredEdge, PortSide, PortType,
    reverse_edge,
};
use crate::options::PortConstraints;

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
        thickness,
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
    use crate::options::{ElkDirection, LayeredOptions};
    use crate::p2layers::layer_network_simplex;

    fn node(id: &str) -> ElkInputNode {
        ElkInputNode {
            id: id.to_string(),
            width: 80.0,
            height: 40.0,
            parent: None,
            direction: None,
            hierarchy_handling: None,
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
}
