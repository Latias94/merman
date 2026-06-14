//! Intermediate layered processors.
//!
//! Source reference:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/ReversedEdgeRestorer.java

use crate::graph::{LGraph, reverse_edge};

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
