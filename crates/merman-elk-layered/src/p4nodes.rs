//! Phase 4 node-placement prerequisites.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/InLayerConstraintProcessor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/LabelAndNodeSizeProcessor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/InnermostNodeMarginCalculator.java

pub mod bk;
pub mod linear_segments;
pub mod network_simplex;
pub mod simple;

use crate::common::nodespacing;
use crate::graph::{InLayerConstraint, LGraph, LNodeKind};

pub use bk::place_nodes_brandes_koepf;
pub use linear_segments::place_nodes_linear_segments;
pub use network_simplex::place_nodes_network_simplex;
pub use simple::place_nodes_simple;

pub(crate) fn vertical_spacing(graph: &LGraph, first: usize, second: usize) -> f64 {
    use LNodeKind::*;

    match (
        graph.layerless_nodes[first].kind,
        graph.layerless_nodes[second].kind,
    ) {
        (Normal, Normal) | (Normal, Label) | (Label, Normal) => graph.options.spacing.node_node,
        (Normal, LongEdge)
        | (LongEdge, Normal)
        | (LongEdge, Label)
        | (Label, LongEdge)
        | (BreakingPoint, Normal)
        | (Normal, BreakingPoint)
        | (BreakingPoint, Label)
        | (Label, BreakingPoint)
        | (BreakingPoint, LongEdge)
        | (LongEdge, BreakingPoint) => graph.options.spacing.edge_node,
        (LongEdge, LongEdge)
        | (LongEdge, NorthSouthPort)
        | (NorthSouthPort, LongEdge)
        | (LongEdge, ExternalPort)
        | (ExternalPort, LongEdge)
        | (Label, Label)
        | (BreakingPoint, BreakingPoint) => graph.options.spacing.edge_edge,
        (Normal, NorthSouthPort)
        | (NorthSouthPort, Normal)
        | (Normal, ExternalPort)
        | (ExternalPort, Normal) => graph.options.spacing.edge_node,
        (NorthSouthPort, NorthSouthPort)
        | (NorthSouthPort, ExternalPort)
        | (ExternalPort, NorthSouthPort) => graph.options.spacing.edge_edge,
        (NorthSouthPort, Label) | (Label, NorthSouthPort) => graph.options.spacing.label_node,
        (ExternalPort, ExternalPort) => graph.options.spacing.port_port,
        (ExternalPort, Label) | (Label, ExternalPort) => graph.options.spacing.label_port_vertical,
        (BreakingPoint, ExternalPort)
        | (ExternalPort, BreakingPoint)
        | (BreakingPoint, NorthSouthPort)
        | (NorthSouthPort, BreakingPoint) => graph.options.spacing.edge_edge,
    }
}

pub fn process_in_layer_constraints(graph: &mut LGraph) {
    for layer_index in 0..graph.layers.len() {
        let nodes = graph.layers[layer_index].nodes.clone();
        let mut top_nodes = Vec::new();
        let mut middle_nodes = Vec::new();
        let mut bottom_nodes = Vec::new();

        for node in nodes {
            match graph.layerless_nodes[node].in_layer_constraint {
                InLayerConstraint::Top => top_nodes.push(node),
                InLayerConstraint::Bottom => bottom_nodes.push(node),
                InLayerConstraint::None => middle_nodes.push(node),
            }
        }

        top_nodes.extend(middle_nodes);
        top_nodes.extend(bottom_nodes);
        graph.layers[layer_index].nodes = top_nodes;
    }
}

pub fn calculate_label_and_node_sizes(graph: &mut LGraph) {
    let layered_nodes = graph
        .layers
        .iter()
        .flat_map(|layer| layer.nodes.iter().copied())
        .collect::<Vec<_>>();

    for node in layered_nodes {
        if graph.layerless_nodes[node].kind == LNodeKind::Normal {
            nodespacing::calculate_label_and_node_sizes(graph, [node]);
        }
    }
}

pub fn calculate_innermost_node_margins(graph: &mut LGraph) {
    let layered_nodes = graph
        .layers
        .iter()
        .flat_map(|layer| layer.nodes.iter().copied())
        .collect::<Vec<_>>();

    nodespacing::calculate_node_margins(graph, layered_nodes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::PortSide;
    use crate::importer::{ElkInputEdge, ElkInputGraph, ElkInputNode, import_graph};
    use crate::options::{ElkDirection, LayeredOptions};

    fn node(id: &str) -> ElkInputNode {
        ElkInputNode {
            id: id.to_string(),
            width: 80.0,
            height: 40.0,
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
            options: LayeredOptions {
                direction: ElkDirection::Right,
                ..LayeredOptions::default()
            },
            nodes,
            edges,
        })
        .unwrap()
    }

    #[test]
    fn in_layer_constraint_processor_moves_top_and_bottom_nodes() {
        let mut graph = graph(vec![node("A"), node("B"), node("C"), node("D")], vec![]);
        for index in 0..graph.layerless_nodes.len() {
            graph.set_node_layer(index, 0);
        }
        graph.layerless_nodes[2].in_layer_constraint = InLayerConstraint::Top;
        graph.layerless_nodes[1].in_layer_constraint = InLayerConstraint::Bottom;

        process_in_layer_constraints(&mut graph);

        let order = graph.layers[0]
            .nodes
            .iter()
            .map(|node| graph.layerless_nodes[*node].id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(order, vec!["C", "A", "D", "B"]);
    }

    #[test]
    fn label_and_node_size_processor_delegates_common_node_spacing() {
        let mut graph = graph(vec![node("A"), node("B")], vec![edge("A-B", "A", "B")]);
        let a = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "A")
            .unwrap();
        let b = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "B")
            .unwrap();
        graph.set_node_layer(a, 0);
        graph.set_node_layer(b, 1);
        graph.layerless_nodes[a].ports[0].set_side(PortSide::East);

        calculate_label_and_node_sizes(&mut graph);

        let port = &graph.layerless_nodes[a].ports[0];
        assert_eq!(port.position.x, graph.layerless_nodes[a].size.width);
        assert!(port.position.y > 0.0);
        assert!(port.position.y < graph.layerless_nodes[a].size.height);
    }
}
