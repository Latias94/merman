//! Phase 2 layering processors.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p2layers/NetworkSimplexLayerer.java
//! - https://github.com/eclipse-elk/elk/tree/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.common/src/org/eclipse/elk/alg/common/networksimplex

use std::collections::{HashMap, VecDeque};

use crate::common::networksimplex::{NGraph, NetworkSimplex};
use crate::graph::{LGraph, LayeredEdge};

const ITER_LIMIT_FACTOR: usize = 4;

pub fn layer_network_simplex(graph: &mut LGraph) {
    graph.clear_layers();
    let nodes = (0..graph.layerless_nodes.len()).collect::<Vec<_>>();
    if nodes.is_empty() {
        return;
    }

    let connected_components = connected_components(graph, &nodes);
    let mut previous_layering_node_counts = None;

    for component in connected_components.iter() {
        let iter_limit = graph.options.thoroughness
            * ITER_LIMIT_FACTOR
            * (component.len() as f64).sqrt() as usize;
        let mut ngraph = initialize(graph, component);

        NetworkSimplex::for_graph(&mut ngraph)
            .with_iteration_limit(iter_limit)
            .with_previous_layering(previous_layering_node_counts.clone())
            .with_balancing(true)
            .execute();

        for n_node in ngraph.active_nodes().iter().copied() {
            let Some(l_node) = ngraph.nodes[n_node].origin else {
                continue;
            };
            graph.set_node_layer(l_node, ngraph.nodes[n_node].layer as usize);
        }

        if connected_components.len() > 1 {
            previous_layering_node_counts =
                Some(graph.layers.iter().map(|layer| layer.nodes.len()).collect());
        }
    }
}

fn connected_components(graph: &LGraph, nodes: &[usize]) -> Vec<Vec<usize>> {
    let mut visited = vec![false; graph.layerless_nodes.len()];
    let mut components: VecDeque<Vec<usize>> = VecDeque::new();

    for node in nodes.iter().copied() {
        if visited[node] {
            continue;
        }

        let mut component = Vec::new();
        connected_components_dfs(graph, node, &mut visited, &mut component);
        if components
            .front()
            .map(|front| front.len() < component.len())
            .unwrap_or(true)
        {
            components.push_front(component);
        } else {
            components.push_back(component);
        }
    }

    components.into_iter().collect()
}

fn connected_components_dfs(
    graph: &LGraph,
    node: usize,
    visited: &mut [bool],
    component: &mut Vec<usize>,
) {
    visited[node] = true;
    component.push(node);

    for edge_index in graph.node_connected_edges(node) {
        let Some(opposite) = opposite_node(graph, node, edge_index) else {
            continue;
        };
        if !visited[opposite] {
            connected_components_dfs(graph, opposite, visited, component);
        }
    }
}

fn initialize(graph: &LGraph, nodes: &[usize]) -> NGraph {
    let mut ngraph = NGraph::new();
    let mut node_map = HashMap::new();

    for l_node in nodes {
        let n_node = ngraph.add_node(Some(*l_node));
        node_map.insert(*l_node, n_node);
    }

    for l_node in nodes {
        for edge_index in graph.node_outgoing_edges(*l_node) {
            let edge = &graph.edges[edge_index];
            if edge.source.node == edge.target.node {
                continue;
            }

            let Some(source) = node_map.get(&edge.source.node).copied() else {
                continue;
            };
            let Some(target) = node_map.get(&edge.target.node).copied() else {
                continue;
            };
            ngraph.add_edge(
                Some(edge_index),
                source,
                target,
                priority_shortness_weight(edge),
                1,
            );
        }
    }

    ngraph
}

fn opposite_node(graph: &LGraph, node: usize, edge_index: usize) -> Option<usize> {
    let edge = graph.edges.get(edge_index)?;
    if edge.source.node == node {
        Some(edge.target.node)
    } else if edge.target.node == node {
        Some(edge.source.node)
    } else {
        None
    }
}

fn priority_shortness_weight(edge: &LayeredEdge) -> f64 {
    edge.priority_shortness.max(1) as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::importer::{ElkInputEdge, ElkInputGraph, ElkInputNode, import_graph};
    use crate::options::{ElkDirection, LayeredOptions};
    use crate::p1cycles::break_cycles_greedy;

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
    fn network_simplex_layerer_assigns_forward_layers() {
        let mut graph = graph(
            vec![node("A"), node("B"), node("C")],
            vec![edge("A-B", "A", "B"), edge("B-C", "B", "C")],
        );

        layer_network_simplex(&mut graph);

        assert_eq!(graph.layers.len(), 3);
        assert_layer_order(&graph, "A", "B");
        assert_layer_order(&graph, "B", "C");
    }

    #[test]
    fn network_simplex_layerer_ignores_self_loops() {
        let mut graph = graph(
            vec![node("A"), node("B")],
            vec![edge("A-A", "A", "A"), edge("A-B", "A", "B")],
        );

        layer_network_simplex(&mut graph);

        assert_layer_order(&graph, "A", "B");
    }

    #[test]
    fn network_simplex_layerer_processes_connected_components() {
        let mut graph = graph(
            vec![node("A"), node("B"), node("C"), node("D")],
            vec![edge("A-B", "A", "B"), edge("C-D", "C", "D")],
        );

        layer_network_simplex(&mut graph);

        assert_layer_order(&graph, "A", "B");
        assert_layer_order(&graph, "C", "D");
        assert_eq!(graph.layers[0].nodes.len(), 2);
    }

    #[test]
    fn greedy_cycle_breaker_output_can_be_layered_by_network_simplex() {
        let mut graph = graph(
            vec![node("A"), node("B"), node("C")],
            vec![
                edge("A-B", "A", "B"),
                edge("B-C", "B", "C"),
                edge("C-A", "C", "A"),
            ],
        );

        break_cycles_greedy(&mut graph);
        layer_network_simplex(&mut graph);

        for edge in &graph.edges {
            if edge.source.node == edge.target.node {
                continue;
            }
            let source_layer = graph.layerless_nodes[edge.source.node].layer_index.unwrap();
            let target_layer = graph.layerless_nodes[edge.target.node].layer_index.unwrap();
            assert!(
                target_layer > source_layer,
                "edge {} should point from an earlier layer to a later layer",
                edge.id
            );
        }
    }

    fn assert_layer_order(graph: &LGraph, source: &str, target: &str) {
        let source = graph
            .layerless_nodes
            .iter()
            .find(|node| node.id == source)
            .unwrap();
        let target = graph
            .layerless_nodes
            .iter()
            .find(|node| node.id == target)
            .unwrap();
        assert!(target.layer_index.unwrap() > source.layer_index.unwrap());
    }
}
