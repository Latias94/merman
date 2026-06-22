//! Linear-segments node placement.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p4nodes/LinearSegmentsNodePlacer.java

use super::simple::place_nodes_simple;
use crate::graph::{LGraph, LNodeKind};

pub fn place_nodes_linear_segments(graph: &mut LGraph) {
    if graph.layers.is_empty() {
        return;
    }

    place_nodes_simple(graph);

    let segments = discover_segments(graph);
    for segment in segments {
        if segment.len() < 2 {
            continue;
        }

        let target_center = segment
            .iter()
            .map(|node_index| node_center_y(graph, *node_index))
            .sum::<f64>()
            / segment.len() as f64;

        for node_index in segment {
            let node = &graph.layerless_nodes[node_index];
            graph.layerless_nodes[node_index].position.y =
                target_center - node.margin.top - node.size.height / 2.0;
        }
    }
}

fn discover_segments(graph: &LGraph) -> Vec<Vec<usize>> {
    let mut visited = vec![false; graph.layerless_nodes.len()];
    let mut segments = Vec::new();

    for layer in &graph.layers {
        for &node_index in &layer.nodes {
            if visited[node_index] || !is_segment_node(graph, node_index) {
                continue;
            }

            let mut stack = vec![node_index];
            visited[node_index] = true;
            let mut segment = Vec::new();

            while let Some(current) = stack.pop() {
                segment.push(current);
                for neighbor in segment_neighbors(graph, current) {
                    if visited[neighbor] {
                        continue;
                    }
                    visited[neighbor] = true;
                    stack.push(neighbor);
                }
            }

            segment.sort_by_key(|node| {
                (
                    graph.layerless_nodes[*node]
                        .layer_index
                        .unwrap_or(usize::MAX),
                    graph
                        .layers
                        .iter()
                        .enumerate()
                        .find_map(|(layer_index, layer)| {
                            layer
                                .nodes
                                .iter()
                                .position(|candidate| candidate == node)
                                .map(|position| (layer_index, position))
                        })
                        .unwrap_or((usize::MAX, usize::MAX)),
                )
            });
            segments.push(segment);
        }
    }

    segments
}

fn segment_neighbors(graph: &LGraph, node_index: usize) -> Vec<usize> {
    let mut neighbors = Vec::new();
    let node = &graph.layerless_nodes[node_index];

    for port in &node.ports {
        for edge_index in &port.outgoing_edges {
            if let Some(other) = graph.edges.get(*edge_index).map(|edge| edge.target.node) {
                push_segment_neighbor(graph, node_index, other, &mut neighbors);
            }
        }
        for edge_index in &port.incoming_edges {
            if let Some(other) = graph.edges.get(*edge_index).map(|edge| edge.source.node) {
                push_segment_neighbor(graph, node_index, other, &mut neighbors);
            }
        }
    }

    neighbors
}

fn push_segment_neighbor(graph: &LGraph, current: usize, neighbor: usize, out: &mut Vec<usize>) {
    if current == neighbor {
        return;
    }
    if graph.layerless_nodes[current].layer_index == graph.layerless_nodes[neighbor].layer_index {
        return;
    }
    if is_segment_node(graph, neighbor) {
        out.push(neighbor);
    }
}

fn is_segment_node(graph: &LGraph, node_index: usize) -> bool {
    matches!(
        graph.layerless_nodes[node_index].kind,
        LNodeKind::LongEdge | LNodeKind::NorthSouthPort
    )
}

fn node_center_y(graph: &LGraph, node_index: usize) -> f64 {
    let node = &graph.layerless_nodes[node_index];
    node.position.y + node.margin.top + node.size.height / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::importer::{ElkInputEdge, ElkInputGraph, ElkInputNode, import_graph};
    use crate::intermediate::split_long_edges;
    use crate::options::{ElkDirection, LayeredOptions};
    use crate::p2layers::layer_network_simplex;

    fn node(id: &str) -> ElkInputNode {
        ElkInputNode {
            id: id.to_string(),
            width: 80.0,
            height: 20.0,
            parent: None,
            direction: None,
            hierarchy_handling: None,
            layer_constraint: None,
            port_constraints: None,
            node_label_placement: crate::options::NodeLabelPlacement::Fixed,
            nested_spacing_base: None,
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
            inside_self_loops_yo: false,
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
        }
    }

    #[test]
    fn linear_segments_placer_aligns_dummy_chain() {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
            nodes: vec![node("A"), node("B"), node("C"), node("D")],
            edges: vec![
                edge("A-B", "A", "B"),
                edge("B-C", "B", "C"),
                edge("C-D", "C", "D"),
                edge("A-D", "A", "D"),
            ],
        })
        .unwrap();
        layer_network_simplex(&mut graph);
        split_long_edges(&mut graph);

        let dummies = graph
            .layerless_nodes
            .iter()
            .enumerate()
            .filter_map(|(index, node)| (node.kind == LNodeKind::LongEdge).then_some(index))
            .collect::<Vec<_>>();
        assert!(dummies.len() >= 2);

        place_nodes_linear_segments(&mut graph);

        let expected = node_center_y(&graph, dummies[0]);
        for dummy in dummies {
            assert!((node_center_y(&graph, dummy) - expected).abs() < 1e-6);
        }
    }
}
