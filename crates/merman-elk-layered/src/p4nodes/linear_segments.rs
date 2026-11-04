//! Linear-segments node placement.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p4nodes/LinearSegmentsNodePlacer.java

use super::simple::place_nodes_simple;
use super::vertical_spacing;
use crate::graph::{LGraph, LNodeKind};

pub fn place_nodes_linear_segments(graph: &mut LGraph) {
    if graph.layers.is_empty() {
        return;
    }

    place_nodes_simple(graph);

    let mut layer_positions = vec![None; graph.layerless_nodes.len()];
    for (layer_index, layer) in graph.layers.iter().enumerate() {
        for (position, &node_index) in layer.nodes.iter().enumerate() {
            layer_positions[node_index] = Some((layer_index, position));
        }
    }

    let segments = discover_segments(graph, &layer_positions);
    for segment in segments {
        if segment.len() < 2 {
            continue;
        }

        let Some((min_center, max_center)) =
            segment_center_bounds(graph, &layer_positions, &segment)
        else {
            // Keep the feasible simple placement when this segment cannot be aligned as a whole.
            continue;
        };

        // Simple placement gives nonnegative centers. An online mean keeps a long segment's
        // finite centers finite even when their sum would exceed f64::MAX.
        let target_center = segment
            .iter()
            .enumerate()
            .fold(0.0, |mean, (index, node_index)| {
                mean + (node_center_y(graph, *node_index) - mean) / (index + 1) as f64
            })
            .clamp(min_center, max_center);

        for node_index in segment {
            let node = &graph.layerless_nodes[node_index];
            graph.layerless_nodes[node_index].position.y = target_center - node.size.height / 2.0;
        }
    }
}

fn segment_center_bounds(
    graph: &LGraph,
    layer_positions: &[Option<(usize, usize)>],
    segment: &[usize],
) -> Option<(f64, f64)> {
    let mut min_center: f64 = 0.0;
    let mut max_center = f64::INFINITY;
    let mut previous_layer = None;

    // LinearSegmentsNodePlacer.postProcess uses the current neighbors' margin envelopes and
    // vertical spacing. Intersect those bounds before moving any node of this segment.
    for &node_index in segment {
        let (layer_index, position) = layer_positions[node_index]?;
        if previous_layer == Some(layer_index) {
            // A branched component can contain several nodes in one layer; aligning it would
            // collapse their required separation. Preserve their existing feasible placement.
            return None;
        }
        previous_layer = Some(layer_index);

        let layer = &graph.layers[layer_index];
        let node = &graph.layerless_nodes[node_index];
        let half_height = node.size.height / 2.0;
        let above = if position == 0 {
            node.margin.top + half_height
        } else {
            let neighbor_index = layer.nodes[position - 1];
            let neighbor = &graph.layerless_nodes[neighbor_index];
            neighbor.position.y
                + neighbor.size.height
                + neighbor.margin.bottom
                + vertical_spacing(graph, neighbor_index, node_index)
                + node.margin.top
                + half_height
        };
        min_center = min_center.max(above);

        if let Some(&neighbor_index) = layer.nodes.get(position + 1) {
            let neighbor = &graph.layerless_nodes[neighbor_index];
            let below = neighbor.position.y
                - neighbor.margin.top
                - vertical_spacing(graph, node_index, neighbor_index)
                - node.margin.bottom
                - half_height;
            max_center = max_center.min(below);
        }

        if min_center > max_center {
            return None;
        }
    }

    Some((min_center, max_center))
}

fn discover_segments(
    graph: &LGraph,
    layer_positions: &[Option<(usize, usize)>],
) -> Vec<Vec<usize>> {
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

            segment.sort_unstable_by_key(|node| {
                layer_positions[*node].unwrap_or((usize::MAX, usize::MAX))
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
    node.position.y + node.size.height / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::LMargin;
    use crate::importer::{ElkInputEdge, ElkInputGraph, ElkInputNode, import_graph};
    use crate::intermediate::split_long_edges;
    use crate::options::{ElkDirection, LayeredOptions, SpacingOptions};
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
            port_alignment: None,
            port_constraints: None,
            node_label_placement: crate::options::NodeLabelPlacement::Fixed,
            node_flexibility: crate::options::NodeFlexibility::None,
            ports_surrounding: None,
            nested_options: None,
            label: None,
        }
    }

    fn edge(id: &str, source: &str, target: &str) -> ElkInputEdge {
        ElkInputEdge {
            id: id.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            labels: Vec::new(),
            minlen: 1,
            inside_self_loops_yo: false,
            model_order: None,
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
        }
    }

    fn layered_graph(layers: &[&[&str]], connections: &[(&str, &str)]) -> LGraph {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".into(),
            options: LayeredOptions {
                direction: ElkDirection::Right,
                spacing: SpacingOptions {
                    node_node: 31.0,
                    edge_node: 17.0,
                    edge_edge: 7.0,
                    label_node: 11.0,
                    ..Default::default()
                },
                ..Default::default()
            },
            nodes: layers
                .iter()
                .flat_map(|layer| layer.iter().map(|id| node(id)))
                .collect(),
            edges: connections
                .iter()
                .enumerate()
                .map(|(index, (source, target))| edge(&format!("e{index}"), source, target))
                .collect(),
        })
        .unwrap();
        let mut node_index = 0;
        for (layer_index, layer) in layers.iter().enumerate() {
            for _ in *layer {
                graph.set_node_layer(node_index, layer_index);
                node_index += 1;
            }
        }
        graph
    }

    fn assert_layer_spacing(graph: &LGraph) {
        for layer in &graph.layers {
            if let Some(&first) = layer.nodes.first() {
                let node = &graph.layerless_nodes[first];
                assert!(
                    node.position.y >= node.margin.top,
                    "{} lost its top margin",
                    node.id
                );
            }
            for pair in layer.nodes.windows(2) {
                let above = &graph.layerless_nodes[pair[0]];
                let below = &graph.layerless_nodes[pair[1]];
                let gap = below.position.y
                    - below.margin.top
                    - (above.position.y + above.size.height + above.margin.bottom);
                let spacing = super::super::vertical_spacing(graph, pair[0], pair[1]);
                assert!(
                    gap >= spacing,
                    "{} -> {}: gap={gap}, spacing={spacing}",
                    above.id,
                    below.id
                );
            }
        }
    }

    #[test]
    fn linear_segments_placer_respects_predecessor_margins_and_edge_node_spacing() {
        let mut graph = layered_graph(&[&["above", "a"], &["b"]], &[("a", "b")]);
        graph.layerless_nodes[0].size.height = 40.0;
        graph.layerless_nodes[0].margin = LMargin {
            top: 3.0,
            bottom: 7.0,
            ..Default::default()
        };
        graph.layerless_nodes[1].kind = LNodeKind::LongEdge;
        graph.layerless_nodes[1].size.height = 4.0;
        graph.layerless_nodes[1].margin = LMargin {
            top: 5.0,
            bottom: 6.0,
            ..Default::default()
        };
        graph.layerless_nodes[2].kind = LNodeKind::NorthSouthPort;
        graph.layerless_nodes[2].size.height = 10.0;
        graph.layerless_nodes[2].margin = LMargin {
            top: 2.0,
            bottom: 8.0,
            ..Default::default()
        };

        place_nodes_linear_segments(&mut graph);

        // The predecessor requires a.y >= 3 + 40 + 7 + 17 + 5 = 72.
        assert_eq!(graph.layerless_nodes[1].position.y, 72.0);
        assert_eq!(graph.layerless_nodes[2].position.y, 69.0);
        assert_eq!(
            graph.layerless_nodes[1].position.y + 2.0,
            graph.layerless_nodes[2].position.y + 5.0
        );
        assert_eq!(graph.layers[0].nodes, [0, 1]);
        assert_layer_spacing(&graph);
    }

    #[test]
    fn linear_segments_placer_respects_successor_margins_and_label_node_spacing() {
        let mut graph = layered_graph(&[&["a", "below"], &["b"]], &[("a", "b")]);
        graph.layerless_nodes[0].kind = LNodeKind::NorthSouthPort;
        graph.layerless_nodes[0].size.height = 4.0;
        graph.layerless_nodes[0].margin = LMargin {
            top: 5.0,
            bottom: 6.0,
            ..Default::default()
        };
        graph.layerless_nodes[1].kind = LNodeKind::Label;
        graph.layerless_nodes[1].size.height = 40.0;
        graph.layerless_nodes[1].margin = LMargin {
            top: 7.0,
            bottom: 3.0,
            ..Default::default()
        };
        graph.layerless_nodes[2].kind = LNodeKind::LongEdge;
        graph.layerless_nodes[2].size.height = 10.0;
        graph.layerless_nodes[2].margin = LMargin {
            top: 2.0,
            bottom: 8.0,
            ..Default::default()
        };

        place_nodes_linear_segments(&mut graph);

        // The successor requires a.y <= 33 - 7 - 11 - 4 - 6 = 5.
        assert_eq!(graph.layerless_nodes[0].position.y, 5.0);
        assert_eq!(graph.layerless_nodes[2].position.y, 2.0);
        assert_eq!(
            graph.layerless_nodes[0].position.y + 2.0,
            graph.layerless_nodes[2].position.y + 5.0
        );
        assert_eq!(graph.layers[0].nodes, [0, 1]);
        assert_layer_spacing(&graph);
    }

    #[test]
    fn linear_segments_placer_preserves_feasible_positions_when_center_intersection_is_empty() {
        let mut graph = layered_graph(&[&["above", "a"], &["b", "below"]], &[("a", "b")]);
        graph.layerless_nodes[0].size.height = 40.0;
        graph.layerless_nodes[0].margin = LMargin {
            top: 3.0,
            bottom: 7.0,
            ..Default::default()
        };
        graph.layerless_nodes[1].kind = LNodeKind::LongEdge;
        graph.layerless_nodes[1].size.height = 4.0;
        graph.layerless_nodes[1].margin = LMargin {
            top: 5.0,
            bottom: 6.0,
            ..Default::default()
        };
        graph.layerless_nodes[2].kind = LNodeKind::NorthSouthPort;
        graph.layerless_nodes[2].size.height = 10.0;
        graph.layerless_nodes[2].margin = LMargin {
            top: 2.0,
            bottom: 8.0,
            ..Default::default()
        };
        graph.layerless_nodes[3].kind = LNodeKind::Label;
        graph.layerless_nodes[3].size.height = 100.0;
        graph.layerless_nodes[3].margin = LMargin {
            top: 9.0,
            bottom: 1.0,
            ..Default::default()
        };
        let mut expected = graph.clone();
        place_nodes_simple(&mut expected);

        place_nodes_linear_segments(&mut graph);

        // a needs a center >= 103.5, while b needs a center <= 7.
        assert_eq!(graph.layerless_nodes[1].position.y, 101.5);
        assert_eq!(graph.layerless_nodes[2].position.y, 2.0);
        assert_eq!(graph, expected);
        assert_layer_spacing(&graph);
    }

    #[test]
    fn linear_segments_placer_uses_current_positions_of_previously_aligned_segments() {
        let mut graph = layered_graph(
            &[&["a0", "tail0"], &["a1", "b1"], &["b2", "tail2"]],
            &[("a0", "a1"), ("b1", "b2")],
        );
        for index in [0, 2, 3, 4] {
            graph.layerless_nodes[index].kind = LNodeKind::LongEdge;
            graph.layerless_nodes[index].size.height = 2.0;
        }
        graph.layerless_nodes[1].size.height = 100.0;
        graph.layerless_nodes[5].size.height = 80.0;

        place_nodes_linear_segments(&mut graph);

        // Moving a1 from 54 to 0 opens b1's feasible center interval [10, 11].
        assert_eq!(graph.layerless_nodes[0].position.y, 0.0);
        assert_eq!(graph.layerless_nodes[2].position.y, 0.0);
        assert_eq!(graph.layerless_nodes[3].position.y, 10.0);
        assert_eq!(graph.layerless_nodes[4].position.y, 10.0);
        assert_eq!(graph.layers[1].nodes, [2, 3]);
        assert_layer_spacing(&graph);
    }

    #[test]
    fn linear_segments_placer_does_not_collapse_branched_nodes_in_the_same_layer() {
        let mut graph = layered_graph(&[&["a"], &["b", "c"]], &[("a", "b"), ("a", "c")]);
        for node in &mut graph.layerless_nodes {
            node.kind = LNodeKind::LongEdge;
            node.size.height = 2.0;
        }

        place_nodes_linear_segments(&mut graph);

        assert_eq!(graph.layers[1].nodes, [1, 2]);
        assert_layer_spacing(&graph);
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
        layer_network_simplex(&mut graph).unwrap();
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
