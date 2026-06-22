//! Simple node placement.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p4nodes/SimpleNodePlacer.java

use super::vertical_spacing;
use crate::graph::LGraph;

pub fn place_nodes_simple(graph: &mut LGraph) {
    if graph.layers.is_empty() {
        return;
    }

    let mut max_layer_height: f64 = 0.0;

    for layer_index in 0..graph.layers.len() {
        let layer_height = layer_height(graph, layer_index);
        graph.layers[layer_index].size.height = layer_height;
        graph.layers[layer_index].size.width = layer_width(graph, layer_index);
        max_layer_height = max_layer_height.max(layer_height);
    }

    for layer_index in 0..graph.layers.len() {
        let mut y = (max_layer_height - graph.layers[layer_index].size.height) / 2.0;
        let mut previous = None;
        let nodes = graph.layers[layer_index].nodes.clone();

        for node_index in nodes {
            if let Some(previous_index) = previous {
                y += vertical_spacing(graph, previous_index, node_index);
            }

            let node = &graph.layerless_nodes[node_index];
            y += node.margin.top;
            graph.layerless_nodes[node_index].position.y = y;
            y += graph.layerless_nodes[node_index].size.height
                + graph.layerless_nodes[node_index].margin.bottom;
            previous = Some(node_index);
        }
    }
}

fn layer_height(graph: &LGraph, layer_index: usize) -> f64 {
    let mut height = 0.0;
    let mut previous = None;

    for node_index in graph.layers[layer_index].nodes.iter().copied() {
        if let Some(previous_index) = previous {
            height += vertical_spacing(graph, previous_index, node_index);
        }

        let node = &graph.layerless_nodes[node_index];
        height += node.margin.top + node.size.height + node.margin.bottom;
        previous = Some(node_index);
    }

    height
}

fn layer_width(graph: &LGraph, layer_index: usize) -> f64 {
    graph.layers[layer_index]
        .nodes
        .iter()
        .map(|node_index| {
            let node = &graph.layerless_nodes[*node_index];
            node.margin.left + node.size.width + node.margin.right
        })
        .fold(0.0, f64::max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{LGraph, LMargin};
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
    fn simple_placer_centers_shorter_layers() {
        let mut graph = graph(
            vec![
                node("A", 80.0, 20.0),
                node("B", 80.0, 20.0),
                node("C", 80.0, 100.0),
            ],
            vec![edge("A-C", "A", "C"), edge("B-C", "B", "C")],
        );
        graph.set_node_layer(0, 0);
        graph.set_node_layer(1, 0);
        graph.set_node_layer(2, 1);

        place_nodes_simple(&mut graph);

        assert_eq!(graph.layers[0].size.height, 80.0);
        assert_eq!(graph.layers[1].size.height, 100.0);
        assert_eq!(graph.layerless_nodes[0].position.y, 10.0);
        assert_eq!(graph.layerless_nodes[1].position.y, 70.0);
        assert_eq!(graph.layerless_nodes[2].position.y, 0.0);
    }

    #[test]
    fn simple_placer_respects_node_margins() {
        let mut graph = graph(vec![node("A", 80.0, 20.0), node("B", 80.0, 20.0)], vec![]);
        graph.set_node_layer(0, 0);
        graph.set_node_layer(1, 0);
        graph.layerless_nodes[0].margin = LMargin {
            top: 3.0,
            bottom: 7.0,
            ..LMargin::default()
        };
        graph.layerless_nodes[1].margin = LMargin {
            top: 5.0,
            bottom: 11.0,
            ..LMargin::default()
        };

        place_nodes_simple(&mut graph);

        assert_eq!(graph.layers[0].size.height, 106.0);
        assert_eq!(graph.layerless_nodes[0].position.y, 3.0);
        assert_eq!(graph.layerless_nodes[1].position.y, 75.0);
    }
}
