//! Network-simplex node placement.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p4nodes/NetworkSimplexPlacer.java

use super::vertical_spacing;
use crate::common::networksimplex::{NGraph, NetworkSimplex};
use crate::graph::{LGraph, LNodeKind};

pub fn place_nodes_network_simplex(graph: &mut LGraph) {
    if graph.layers.is_empty() {
        return;
    }

    let mut auxiliary = NGraph::new();
    let root = auxiliary.add_node(None);
    let mut node_map = vec![None; graph.layerless_nodes.len()];

    for layer in &graph.layers {
        for node_index in layer.nodes.iter().copied() {
            if graph.layerless_nodes[node_index].hidden {
                continue;
            }
            let nnode = auxiliary.add_node(Some(node_index));
            node_map[node_index] = Some(nnode);
            auxiliary.add_edge(None, root, nnode, 0.0, 0);
        }
    }

    for layer in &graph.layers {
        for pair in layer.nodes.windows(2) {
            let previous = pair[0];
            let current = pair[1];
            let (Some(previous_aux), Some(current_aux)) = (node_map[previous], node_map[current])
            else {
                continue;
            };

            auxiliary.add_edge(
                None,
                previous_aux,
                current_aux,
                0.0,
                spacing_delta(graph, previous, current),
            );
        }
    }

    add_edge_alignment_constraints(graph, &mut auxiliary, &node_map);

    let iteration_limit = graph
        .options
        .thoroughness
        .saturating_mul(auxiliary.nodes.len().max(1));
    NetworkSimplex::for_graph(&mut auxiliary)
        .with_iteration_limit(iteration_limit)
        .with_balancing(false)
        .execute();

    for nnode in auxiliary.active_nodes().iter().copied() {
        let Some(node_index) = auxiliary.nodes[nnode].origin else {
            continue;
        };
        graph.layerless_nodes[node_index].position.y = auxiliary.nodes[nnode].layer as f64;
    }
}

fn spacing_delta(graph: &LGraph, previous: usize, current: usize) -> i32 {
    let previous_node = &graph.layerless_nodes[previous];
    let current_node = &graph.layerless_nodes[current];
    ceil_i32(
        previous_node.size.height
            + previous_node.margin.bottom
            + vertical_spacing(graph, previous, current)
            + current_node.margin.top,
    )
}

fn add_edge_alignment_constraints(
    graph: &LGraph,
    auxiliary: &mut NGraph,
    node_map: &[Option<usize>],
) {
    if graph.options.node_placement_favor_straight_edges == Some(false) {
        return;
    }

    for (edge_index, edge) in graph.edges.iter().enumerate() {
        if edge.source.node == edge.target.node
            || graph.layerless_nodes[edge.source.node].layer_index
                == graph.layerless_nodes[edge.target.node].layer_index
        {
            continue;
        }
        let (Some(source_aux), Some(target_aux)) =
            (node_map[edge.source.node], node_map[edge.target.node])
        else {
            continue;
        };
        if source_aux == target_aux {
            continue;
        }

        let edge_aux = auxiliary.add_node(None);
        let source_anchor = port_y(graph, edge.source.node, edge.source.port);
        let target_anchor = port_y(graph, edge.target.node, edge.target.port);
        let source_delta = (target_anchor - source_anchor).max(0.0).round();
        let target_delta = (source_anchor - target_anchor).max(0.0).round();
        let weight = edge.priority_straightness.max(1) as f64
            * edge_type_weight(
                graph.layerless_nodes[edge.source.node].kind,
                graph.layerless_nodes[edge.target.node].kind,
            );

        auxiliary.add_edge(
            Some(edge_index),
            edge_aux,
            source_aux,
            weight,
            ceil_i32(source_delta),
        );
        auxiliary.add_edge(
            Some(edge_index),
            edge_aux,
            target_aux,
            weight,
            ceil_i32(target_delta),
        );
    }
}

fn port_y(graph: &LGraph, node: usize, port: usize) -> f64 {
    graph
        .layerless_nodes
        .get(node)
        .and_then(|node| node.ports.get(port))
        .map(|port| port.position.y + port.anchor.y)
        .unwrap_or(0.0)
}

fn edge_type_weight(source: LNodeKind, target: LNodeKind) -> f64 {
    match (source, target) {
        (LNodeKind::Normal, LNodeKind::Normal) => 4.0,
        (LNodeKind::Normal, _) | (_, LNodeKind::Normal) => 8.0,
        _ => 32.0,
    }
}

fn ceil_i32(value: f64) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    value.ceil().clamp(0.0, i32::MAX as f64) as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::LMargin;
    use crate::importer::{ElkInputEdge, ElkInputGraph, ElkInputNode, import_graph};
    use crate::options::{ElkDirection, LayeredOptions};

    fn node(id: &str, width: f64, height: f64) -> ElkInputNode {
        ElkInputNode {
            id: id.to_string(),
            width,
            height,
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
    fn network_simplex_placer_respects_layer_order_and_spacing() {
        let mut graph = graph(
            vec![
                node("A", 80.0, 30.0),
                node("B", 80.0, 20.0),
                node("C", 80.0, 40.0),
            ],
            vec![edge("A-C", "A", "C"), edge("B-C", "B", "C")],
        );
        graph.set_node_layer(0, 0);
        graph.set_node_layer(1, 0);
        graph.set_node_layer(2, 1);
        graph.layerless_nodes[0].margin = LMargin {
            top: 2.0,
            bottom: 3.0,
            ..LMargin::default()
        };
        graph.layerless_nodes[1].margin = LMargin {
            top: 5.0,
            bottom: 7.0,
            ..LMargin::default()
        };

        place_nodes_network_simplex(&mut graph);

        let required_gap = graph.layerless_nodes[0].size.height
            + graph.layerless_nodes[0].margin.bottom
            + vertical_spacing(&graph, 0, 1)
            + graph.layerless_nodes[1].margin.top;
        assert!(
            graph.layerless_nodes[1].position.y - graph.layerless_nodes[0].position.y
                >= required_gap
        );
        assert!(
            graph
                .layerless_nodes
                .iter()
                .all(|node| node.position.y.is_finite())
        );
    }

    #[test]
    fn network_simplex_placer_handles_empty_layers() {
        let mut graph = graph(vec![node("A", 80.0, 30.0)], vec![]);
        graph.set_node_layer(0, 1);

        place_nodes_network_simplex(&mut graph);

        assert!(graph.layerless_nodes[0].position.y.is_finite());
    }
}
