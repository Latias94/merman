//! ELK layered phase 5 edge routing.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p5edges/OrthogonalEdgeRouter.java
//! - https://github.com/eclipse-elk/elk/tree/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p5edges/orthogonal

use crate::graph::{LGraph, LNodeKind};

pub mod orthogonal;

pub fn route_edges_orthogonal(graph: &mut LGraph) {
    let node_node_spacing = graph.options.spacing.node_node_between_layers;
    let edge_edge_spacing = graph.options.spacing.edge_edge_between_layers;
    let edge_node_spacing = graph.options.spacing.edge_node_between_layers;

    let mut xpos = 0.0;
    let mut left_layer_nodes: Option<Vec<usize>> = None;

    for layer_index in 0..=graph.layers.len() {
        let right_layer_nodes = graph
            .layers
            .get(layer_index)
            .map(|layer| layer.nodes.clone());

        if let Some(nodes) = left_layer_nodes.as_ref() {
            place_nodes_horizontally(graph, nodes, xpos);
            xpos += graph.layers[layer_index - 1].size.width;
        }

        let start_pos = if left_layer_nodes.is_none() {
            xpos
        } else {
            xpos + edge_node_spacing
        };
        let slots_count = orthogonal::route_edges_west_to_east(
            graph,
            left_layer_nodes.as_deref(),
            right_layer_nodes.as_deref(),
            start_pos,
            edge_edge_spacing,
        );

        let is_left_layer_external = left_layer_nodes
            .as_deref()
            .map(|nodes| {
                nodes
                    .iter()
                    .all(|node| is_external_west_or_east_port(graph, *node))
            })
            .unwrap_or(true);
        let is_right_layer_external = right_layer_nodes
            .as_deref()
            .map(|nodes| {
                nodes
                    .iter()
                    .all(|node| is_external_west_or_east_port(graph, *node))
            })
            .unwrap_or(true);

        if slots_count > 0 {
            let mut routing_width = (slots_count - 1) as f64 * edge_edge_spacing;
            if left_layer_nodes.is_some() {
                routing_width += edge_node_spacing;
            }
            if right_layer_nodes.is_some() {
                routing_width += edge_node_spacing;
            }
            if routing_width < node_node_spacing
                && !is_left_layer_external
                && !is_right_layer_external
            {
                routing_width = node_node_spacing;
            }
            xpos += routing_width;
        } else if !is_left_layer_external && !is_right_layer_external {
            xpos += node_node_spacing;
        }

        left_layer_nodes = right_layer_nodes;
    }

    graph.size.width = xpos;
}

fn place_nodes_horizontally(graph: &mut LGraph, nodes: &[usize], xoffset: f64) {
    for node in nodes {
        graph.layerless_nodes[*node].position.x =
            xoffset + graph.layerless_nodes[*node].margin.left;
    }
}

fn is_external_west_or_east_port(graph: &LGraph, node: usize) -> bool {
    if graph.layerless_nodes[node].kind != LNodeKind::ExternalPort {
        return false;
    }
    graph.layerless_nodes[node]
        .ports
        .first()
        .map(|port| {
            matches!(
                port.side,
                crate::graph::PortSide::West | crate::graph::PortSide::East
            )
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{LNode, LPoint, LayeredEdge, PortSide, PortType};
    use crate::options::LayeredOptions;

    #[test]
    fn orthogonal_router_places_layers_and_routes_between_them() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        let source = graph.layerless_nodes.len();
        graph
            .layerless_nodes
            .push(LNode::new("A", 40.0, 20.0, Some(0)));
        let target = graph.layerless_nodes.len();
        let mut target_node = LNode::new("B", 40.0, 20.0, Some(1));
        target_node.position.y = 40.0;
        graph.layerless_nodes.push(target_node);
        graph.set_node_layer(source, 0);
        graph.set_node_layer(target, 1);
        graph.layers[0].size.width = 40.0;
        graph.layers[1].size.width = 40.0;

        let source_port = graph
            .add_port(
                source,
                PortType::Output,
                PortSide::East,
                LPoint { x: 40.0, y: 10.0 },
            )
            .unwrap();
        let target_port = graph
            .add_port(
                target,
                PortType::Output,
                PortSide::West,
                LPoint { x: 0.0, y: 10.0 },
            )
            .unwrap();
        graph.add_edge(LayeredEdge {
            id: "A-B".to_string(),
            source: source_port,
            target: target_port,
            source_node_id: "A".to_string(),
            target_node_id: "B".to_string(),
            labels: Vec::new(),
            minlen: 1,
            reversed: false,
            bend_points: Vec::new(),
            model_order: Some(0),
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
            thickness: 0.0,
            original_opposite_port: None,
        });

        route_edges_orthogonal(&mut graph);

        assert_eq!(graph.layerless_nodes[source].position.x, 0.0);
        assert_eq!(graph.layerless_nodes[target].position.x, 60.0);
        assert_eq!(graph.size.width, 100.0);
        assert_eq!(
            graph.edges[0].bend_points,
            vec![LPoint { x: 50.0, y: 10.0 }, LPoint { x: 50.0, y: 50.0 }]
        );
    }
}
