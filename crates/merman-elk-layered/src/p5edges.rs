//! ELK layered phase 5 edge routing.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p5edges/OrthogonalEdgeRouter.java
//! - https://github.com/eclipse-elk/elk/tree/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p5edges/orthogonal

use crate::graph::{LGraph, LNodeKind};
use crate::options::Alignment;

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
    let max_left_margin = nodes
        .iter()
        .map(|node| graph.layerless_nodes[*node].margin.left)
        .fold(0.0, f64::max);
    let max_right_margin = nodes
        .iter()
        .map(|node| graph.layerless_nodes[*node].margin.right)
        .fold(0.0, f64::max);
    let layer_width = nodes
        .iter()
        .filter_map(|node| graph.layerless_nodes[*node].layer_index)
        .next()
        .and_then(|layer| graph.layers.get(layer))
        .map(|layer| layer.size.width)
        .unwrap_or(0.0);

    for node in nodes {
        let lnode = &graph.layerless_nodes[*node];
        let ratio = horizontal_alignment_ratio(graph, *node);

        let node_width = lnode.size.width;
        let mut xpos = (layer_width - node_width) * ratio;
        if ratio > 0.5 {
            xpos -= max_right_margin * 2.0 * (ratio - 0.5);
        } else if ratio < 0.5 {
            xpos += max_left_margin * 2.0 * (0.5 - ratio);
        }

        xpos = xpos.max(lnode.margin.left);
        xpos = xpos.min(layer_width - lnode.margin.right - node_width);
        graph.layerless_nodes[*node].position.x = xoffset + xpos;
    }
}

fn horizontal_alignment_ratio(graph: &LGraph, node: usize) -> f64 {
    let lnode = &graph.layerless_nodes[node];
    match lnode.node_alignment {
        Alignment::Left => 0.0,
        Alignment::Right => 1.0,
        Alignment::Center => 0.5,
        Alignment::Automatic | Alignment::Top | Alignment::Bottom => {
            let inports = lnode
                .ports
                .iter()
                .filter(|port| !port.incoming_edges.is_empty())
                .count();
            let outports = lnode
                .ports
                .iter()
                .filter(|port| !port.outgoing_edges.is_empty())
                .count();
            if inports + outports == 0 {
                0.5
            } else {
                outports as f64 / (inports + outports) as f64
            }
        }
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
            compound_segment: None,
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

    #[test]
    fn orthogonal_router_centers_unconnected_nodes_in_wide_layers() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        let wide = graph.layerless_nodes.len();
        graph
            .layerless_nodes
            .push(LNode::new("wide", 80.0, 20.0, Some(0)));
        let narrow = graph.layerless_nodes.len();
        graph
            .layerless_nodes
            .push(LNode::new("narrow", 20.0, 20.0, Some(1)));
        graph.set_node_layer(wide, 0);
        graph.set_node_layer(narrow, 1);
        graph.layers[0].size.width = 80.0;
        graph.layers[1].size.width = 80.0;

        route_edges_orthogonal(&mut graph);

        let expected_narrow_x =
            80.0 + graph.options.spacing.node_node_between_layers + (80.0 - 20.0) / 2.0;
        assert_eq!(graph.layerless_nodes[wide].position.x, 0.0);
        assert_eq!(graph.layerless_nodes[narrow].position.x, expected_narrow_x);
    }
}
