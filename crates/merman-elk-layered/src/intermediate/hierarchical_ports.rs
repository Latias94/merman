//! Hierarchical port placement and orthogonal edge completion.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/HierarchicalPortConstraintProcessor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/HierarchicalPortDummySizeProcessor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/HierarchicalPortPositionProcessor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/HierarchicalPortOrthogonalEdgeRouter.java

use crate::graph::{
    GraphPortRef, LGraph, LNode, LNodeKind, LPoint, LayeredEdge, PortRef, PortSide, PortType,
};
use crate::options::{Alignment, ELK_DEFAULT_EDGE_THICKNESS, PortConstraints};
use crate::p5edges::orthogonal::{RoutingDirection, route_edges};

use super::first_port_on_side;

pub fn process_hierarchical_port_dummy_sizes(graph: &mut LGraph) {
    let delta = graph.options.spacing.edge_edge_between_layers * 2.0;

    for layer_index in 0..graph.layers.len() {
        let (northern_dummies, southern_dummies) =
            collect_north_south_external_port_dummies(graph, layer_index);
        set_hierarchical_port_dummy_widths(graph, &northern_dummies, true, delta);
        set_hierarchical_port_dummy_widths(graph, &southern_dummies, false, delta);
    }
}

fn collect_north_south_external_port_dummies(
    graph: &LGraph,
    layer_index: usize,
) -> (Vec<usize>, Vec<usize>) {
    let mut northern_dummies = Vec::new();
    let mut southern_dummies = Vec::new();

    for node_index in &graph.layers[layer_index].nodes {
        let node = &graph.layerless_nodes[*node_index];
        if node.kind != LNodeKind::ExternalPort {
            continue;
        }

        match node.external_port_side {
            PortSide::North => northern_dummies.push(*node_index),
            PortSide::South => southern_dummies.push(*node_index),
            PortSide::Undefined | PortSide::East | PortSide::West => {}
        }
    }

    (northern_dummies, southern_dummies)
}

fn set_hierarchical_port_dummy_widths(
    graph: &mut LGraph,
    nodes: &[usize],
    top_down: bool,
    delta: f64,
) {
    let mut current_width = if top_down {
        0.0
    } else {
        delta * nodes.len().saturating_sub(1) as f64
    };
    let step = if top_down { delta } else { -delta };

    for node_index in nodes {
        let node = &mut graph.layerless_nodes[*node_index];
        node.node_alignment = Alignment::Center;
        node.size.width = current_width;

        for port in &mut node.ports {
            if port.side == PortSide::East {
                port.position.x = current_width;
            }
        }

        current_width += step;
    }
}

pub fn process_hierarchical_port_positions(graph: &mut LGraph) {
    if !(graph.options.port_constraints.is_ratio_fixed()
        || graph.options.port_constraints.is_pos_fixed())
        || graph.layers.is_empty()
    {
        return;
    }

    process_hierarchical_port_positions_for_layer(graph, 0);
    if graph.layers.len() > 1 {
        process_hierarchical_port_positions_for_layer(graph, graph.layers.len() - 1);
    }
}

fn process_hierarchical_port_positions_for_layer(graph: &mut LGraph, layer_index: usize) {
    let graph_height = actual_graph_height(graph);
    let nodes = graph.layers[layer_index].nodes.clone();

    for node in nodes {
        if graph.layerless_nodes[node].kind != LNodeKind::ExternalPort {
            continue;
        }
        if !matches!(
            graph.layerless_nodes[node].external_port_side,
            PortSide::East | PortSide::West
        ) {
            continue;
        }

        let mut final_y_coordinate = graph.layerless_nodes[node].port_ratio_or_position;
        if graph.options.port_constraints.is_ratio_fixed() {
            final_y_coordinate *= graph_height;
        }

        let port_anchor_y = first_port_anchor_y(graph, node);
        graph.layerless_nodes[node].position.y = final_y_coordinate - port_anchor_y;
        border_to_content_area_y(graph, node);
    }
}

fn actual_graph_height(graph: &LGraph) -> f64 {
    graph.size.height + graph.padding.top + graph.padding.bottom
}

fn first_port_anchor_y(graph: &LGraph, node: usize) -> f64 {
    graph.layerless_nodes[node]
        .ports
        .first()
        .map(|port| port.position.y)
        .unwrap_or(0.0)
}

fn border_to_content_area_y(graph: &mut LGraph, node: usize) {
    graph.layerless_nodes[node].position.y -= graph.padding.top + graph.offset.y;
}

pub fn process_hierarchical_port_orthogonal_edges(graph: &mut LGraph) {
    if graph.layers.is_empty() {
        return;
    }

    let north_south_dummies = restore_north_south_dummies(graph);
    set_north_south_dummy_coordinates(graph, &north_south_dummies);
    route_north_south_dummy_edges(graph, &north_south_dummies);
    remove_temporary_north_south_dummies(graph);
    fix_hierarchical_port_coordinates(graph);
    correct_hierarchical_port_slanted_edge_segments(graph);
}

fn restore_north_south_dummies(graph: &mut LGraph) -> Vec<usize> {
    let restored_dummies = graph.replaced_external_port_dummies.clone();
    if restored_dummies.is_empty() {
        return restored_dummies;
    }

    for dummy in &restored_dummies {
        restore_north_south_dummy(graph, *dummy);
    }

    let replacement_nodes = graph
        .layers
        .iter()
        .flat_map(|layer| layer.nodes.iter().copied())
        .filter(|node| {
            graph.layerless_nodes[*node].kind == LNodeKind::ExternalPort
                && graph.layerless_nodes[*node]
                    .replaced_external_port_dummy
                    .is_some()
        })
        .collect::<Vec<_>>();

    for replacement in replacement_nodes {
        if let Some(original) = graph.layerless_nodes[replacement].replaced_external_port_dummy {
            connect_replacement_dummy_to_original(graph, replacement, original);
        }
    }

    let last_layer = graph.layers.len() - 1;
    for dummy in &restored_dummies {
        graph.set_node_layer(*dummy, last_layer);
    }

    restored_dummies
}

fn restore_north_south_dummy(graph: &mut LGraph, dummy: usize) {
    let external_side = graph.layerless_nodes[dummy].external_port_side;
    let Some(port) = graph.layerless_nodes[dummy].ports.first_mut() else {
        return;
    };

    port.side = match external_side {
        PortSide::North => PortSide::South,
        PortSide::South => PortSide::North,
        side => side,
    };
    port.port_type = PortType::Output;
}

fn connect_replacement_dummy_to_original(graph: &mut LGraph, replacement: usize, original: usize) {
    if graph.layerless_nodes[replacement].ports.iter().any(|port| {
        port.side == graph.layerless_nodes[replacement].external_port_side
            && port.outgoing_edges.iter().any(|edge| {
                graph.edges[*edge].target.node == original && graph.edges[*edge].target.port == 0
            })
    }) {
        return;
    }

    let external_side = graph.layerless_nodes[replacement].external_port_side;
    let Some(origin_port) = graph.add_port(
        replacement,
        PortType::Output,
        external_side,
        Default::default(),
    ) else {
        return;
    };

    let edge = LayeredEdge {
        id: format!(
            "{}->{}:hierarchical-origin",
            graph.layerless_nodes[replacement].id, graph.layerless_nodes[original].id
        ),
        source: origin_port,
        target: PortRef {
            node: original,
            port: 0,
        },
        source_node_id: graph.layerless_nodes[replacement].id.clone(),
        target_node_id: graph.layerless_nodes[original].id.clone(),
        labels: Vec::new(),
        minlen: 1,
        reversed: false,
        bend_points: Vec::new(),
        model_order: None,
        priority_direction: 0,
        priority_shortness: 0,
        priority_straightness: 0,
        thickness: ELK_DEFAULT_EDGE_THICKNESS,
        original_opposite_port: None,
        compound_segment: None,
    };
    graph.add_edge(edge);
}

fn set_north_south_dummy_coordinates(graph: &mut LGraph, north_south_dummies: &[usize]) {
    if north_south_dummies.is_empty() {
        return;
    }

    let graph_width = graph.size.width + graph.padding.left + graph.padding.right;
    let north_y = -graph.padding.top - graph.offset.y;
    let south_y = graph.size.height + graph.padding.top + graph.padding.bottom - graph.offset.y;
    let mut northern_dummies = Vec::new();
    let mut southern_dummies = Vec::new();

    for dummy in north_south_dummies {
        match graph.options.port_constraints {
            PortConstraints::Free | PortConstraints::FixedSide | PortConstraints::FixedOrder => {
                calculate_north_south_dummy_position(graph, *dummy);
            }
            PortConstraints::FixedRatio => {
                graph.layerless_nodes[*dummy].position.x = graph_width
                    * graph.layerless_nodes[*dummy].port_ratio_or_position
                    - first_port_anchor_x(graph, *dummy);
                border_to_content_area_x(graph, *dummy);
            }
            PortConstraints::FixedPos => {
                graph.layerless_nodes[*dummy].position.x = graph.layerless_nodes[*dummy]
                    .port_ratio_or_position
                    - first_port_anchor_x(graph, *dummy);
                border_to_content_area_x(graph, *dummy);
                let required_width = graph.layerless_nodes[*dummy].position.x
                    + graph.layerless_nodes[*dummy].size.width / 2.0;
                graph.size.width = graph.size.width.max(required_width);
            }
            PortConstraints::Undefined => {
                calculate_north_south_dummy_position(graph, *dummy);
            }
        }

        match graph.layerless_nodes[*dummy].external_port_side {
            PortSide::North => {
                graph.layerless_nodes[*dummy].position.y = north_y;
                northern_dummies.push(*dummy);
            }
            PortSide::South => {
                graph.layerless_nodes[*dummy].position.y = south_y;
                southern_dummies.push(*dummy);
            }
            PortSide::Undefined | PortSide::East | PortSide::West => {}
        }
    }

    match graph.options.port_constraints {
        PortConstraints::Free | PortConstraints::FixedSide | PortConstraints::Undefined => {
            ensure_unique_north_south_positions(graph, &mut northern_dummies);
            ensure_unique_north_south_positions(graph, &mut southern_dummies);
        }
        PortConstraints::FixedOrder => {
            restore_north_south_dummy_order(graph, &mut northern_dummies);
            restore_north_south_dummy_order(graph, &mut southern_dummies);
        }
        PortConstraints::FixedRatio | PortConstraints::FixedPos => {}
    }
}

fn calculate_north_south_dummy_position(graph: &mut LGraph, dummy: usize) {
    let Some(dummy_port) = graph.layerless_nodes[dummy].ports.first() else {
        graph.layerless_nodes[dummy].position.x = 0.0;
        return;
    };

    let connected_ports = dummy_port
        .incoming_edges
        .iter()
        .filter_map(|edge| graph.edges.get(*edge).map(|edge| edge.source))
        .chain(
            dummy_port
                .outgoing_edges
                .iter()
                .filter_map(|edge| graph.edges.get(*edge).map(|edge| edge.target)),
        )
        .collect::<Vec<_>>();

    if connected_ports.is_empty() {
        graph.layerless_nodes[dummy].position.x = 0.0;
        return;
    }

    let pos_sum = connected_ports
        .iter()
        .map(|port_ref| absolute_port_anchor(graph, *port_ref).x)
        .sum::<f64>();
    graph.layerless_nodes[dummy].position.x =
        pos_sum / connected_ports.len() as f64 - first_port_anchor_x(graph, dummy);
}

fn first_port_anchor_x(graph: &LGraph, node: usize) -> f64 {
    graph.layerless_nodes[node]
        .ports
        .first()
        .map(|port| port.position.x)
        .unwrap_or(0.0)
}

fn border_to_content_area_x(graph: &mut LGraph, node: usize) {
    graph.layerless_nodes[node].position.x -= graph.padding.left + graph.offset.x;
}

fn ensure_unique_north_south_positions(graph: &mut LGraph, dummies: &mut [usize]) {
    dummies.sort_by(|left, right| {
        graph.layerless_nodes[*left]
            .position
            .x
            .total_cmp(&graph.layerless_nodes[*right].position.x)
    });
    assign_ascending_north_south_coordinates(graph, dummies);
}

fn restore_north_south_dummy_order(graph: &mut LGraph, dummies: &mut [usize]) {
    dummies.sort_by(|left, right| {
        graph.layerless_nodes[*left]
            .port_ratio_or_position
            .total_cmp(&graph.layerless_nodes[*right].port_ratio_or_position)
    });
    assign_ascending_north_south_coordinates(graph, dummies);
}

fn assign_ascending_north_south_coordinates(graph: &mut LGraph, dummies: &[usize]) {
    let Some(first) = dummies.first() else {
        return;
    };

    let spacing = graph.options.spacing.port_port;
    let first_node = &graph.layerless_nodes[*first];
    let mut next_valid_coordinate =
        first_node.position.x + first_node.size.width + first_node.margin.right + spacing;

    for dummy in dummies.iter().skip(1) {
        let current = &graph.layerless_nodes[*dummy];
        let delta = current.position.x - current.margin.left - next_valid_coordinate;
        if delta < 0.0 {
            graph.layerless_nodes[*dummy].position.x -= delta;
        }

        let current = &graph.layerless_nodes[*dummy];
        graph.size.width = graph
            .size
            .width
            .max(current.position.x + current.size.width);
        next_valid_coordinate =
            current.position.x + current.size.width + current.margin.right + spacing;
    }
}

fn route_north_south_dummy_edges(graph: &mut LGraph, north_south_dummies: &[usize]) {
    let mut northern_source_layer = Vec::new();
    let mut northern_target_layer = Vec::new();
    let mut southern_source_layer = Vec::new();
    let mut southern_target_layer = Vec::new();

    for dummy in north_south_dummies {
        match graph.layerless_nodes[*dummy].external_port_side {
            PortSide::North => {
                push_unique(&mut northern_target_layer, *dummy);
                for edge in graph.node_incoming_edges(*dummy) {
                    push_unique(&mut northern_source_layer, graph.edges[edge].source.node);
                }
            }
            PortSide::South => {
                push_unique(&mut southern_target_layer, *dummy);
                for edge in graph.node_incoming_edges(*dummy) {
                    push_unique(&mut southern_source_layer, graph.edges[edge].source.node);
                }
            }
            PortSide::Undefined | PortSide::East | PortSide::West => {}
        }
    }

    let node_spacing = graph.options.spacing.node_node;
    let edge_spacing = graph.options.spacing.edge_edge;

    if !northern_source_layer.is_empty() {
        let slots = route_edges(
            graph,
            RoutingDirection::SouthToNorth,
            Some(&northern_source_layer),
            Some(&northern_target_layer),
            -node_spacing - graph.offset.y,
            edge_spacing,
        );
        if slots > 0 {
            let routing_height = node_spacing + slots.saturating_sub(1) as f64 * edge_spacing;
            graph.offset.y += routing_height;
            graph.size.height += routing_height;
        }
    }

    if !southern_source_layer.is_empty() {
        let slots = route_edges(
            graph,
            RoutingDirection::NorthToSouth,
            Some(&southern_source_layer),
            Some(&southern_target_layer),
            graph.size.height + node_spacing - graph.offset.y,
            edge_spacing,
        );
        if slots > 0 {
            graph.size.height += node_spacing + slots.saturating_sub(1) as f64 * edge_spacing;
        }
    }
}

fn push_unique(nodes: &mut Vec<usize>, node: usize) {
    if !nodes.contains(&node) {
        nodes.push(node);
    }
}

fn remove_temporary_north_south_dummies(graph: &mut LGraph) {
    let temporary_nodes = graph
        .layers
        .iter()
        .flat_map(|layer| layer.nodes.iter().copied())
        .filter(|node| {
            graph.layerless_nodes[*node].kind == LNodeKind::ExternalPort
                && graph.layerless_nodes[*node]
                    .replaced_external_port_dummy
                    .is_some()
        })
        .collect::<Vec<_>>();

    for node in &temporary_nodes {
        remove_temporary_north_south_dummy(graph, *node);
    }

    for node in temporary_nodes {
        graph.remove_node_from_layer(node);
    }
}

fn remove_temporary_north_south_dummy(graph: &mut LGraph, node: usize) {
    let Some(node_in_port) = first_port_on_side(graph, node, PortSide::West) else {
        return;
    };
    let Some(node_out_port) = first_port_on_side(graph, node, PortSide::East) else {
        return;
    };
    let Some(node_origin_port) = graph.layerless_nodes[node]
        .ports
        .iter()
        .position(|port| !matches!(port.side, PortSide::West | PortSide::East))
    else {
        return;
    };
    let Some(node_to_origin_edge) = graph.layerless_nodes[node].ports[node_origin_port]
        .outgoing_edges
        .first()
        .copied()
    else {
        return;
    };
    let Some(replaced_dummy) = graph.layerless_nodes[node].replaced_external_port_dummy else {
        return;
    };

    let mut incoming_edge_bend_points = graph.edges[node_to_origin_edge].bend_points.clone();
    incoming_edge_bend_points.insert(
        0,
        node_relative_port_position(graph, node, node_origin_port),
    );

    let mut outgoing_edge_bend_points = graph.edges[node_to_origin_edge].bend_points.clone();
    outgoing_edge_bend_points.reverse();
    outgoing_edge_bend_points.push(node_relative_port_position(graph, node, node_origin_port));

    let replaced_dummy_port = PortRef {
        node: replaced_dummy,
        port: 0,
    };

    let incoming_edges = graph.layerless_nodes[node].ports[node_in_port]
        .incoming_edges
        .clone();
    for edge in incoming_edges {
        graph.set_edge_target(edge, replaced_dummy_port);
        graph.edges[edge]
            .bend_points
            .extend(incoming_edge_bend_points.iter().copied());
    }

    let outgoing_edges = graph.layerless_nodes[node].ports[node_out_port]
        .outgoing_edges
        .clone();
    for edge in outgoing_edges {
        graph.set_edge_source(edge, replaced_dummy_port);
        graph.edges[edge]
            .bend_points
            .splice(0..0, outgoing_edge_bend_points.iter().copied());
    }

    graph.detach_edge(node_to_origin_edge);
}

fn node_relative_port_position(graph: &LGraph, node: usize, port: usize) -> LPoint {
    LPoint {
        x: graph.layerless_nodes[node].position.x
            + graph.layerless_nodes[node].ports[port].position.x,
        y: graph.layerless_nodes[node].position.y
            + graph.layerless_nodes[node].ports[port].position.y,
    }
}

fn fix_hierarchical_port_coordinates(graph: &mut LGraph) {
    fix_hierarchical_port_coordinates_for_layer(graph, 0);
    fix_hierarchical_port_coordinates_for_layer(graph, graph.layers.len() - 1);
}

fn fix_hierarchical_port_coordinates_for_layer(graph: &mut LGraph, layer_index: usize) {
    let nodes = graph.layers[layer_index].nodes.clone();
    let graph_actual_height = actual_graph_height(graph);
    let mut new_actual_graph_height = graph_actual_height;

    for node in &nodes {
        if graph.layerless_nodes[*node].kind != LNodeKind::ExternalPort {
            continue;
        }

        match graph.layerless_nodes[*node].external_port_side {
            PortSide::East => {
                graph.layerless_nodes[*node].position.x =
                    graph.size.width + graph.padding.right - graph.offset.x;
            }
            PortSide::West => {
                graph.layerless_nodes[*node].position.x = -graph.offset.x - graph.padding.left;
            }
            PortSide::North | PortSide::South | PortSide::Undefined => {}
        }

        if matches!(
            graph.layerless_nodes[*node].external_port_side,
            PortSide::East | PortSide::West
        ) {
            let required_actual_graph_height =
                fix_hierarchical_port_east_west_y_coordinate(graph, *node, graph_actual_height);
            new_actual_graph_height = new_actual_graph_height.max(required_actual_graph_height);
        }
    }

    graph.size.height += new_actual_graph_height - graph_actual_height;

    for node in nodes {
        if graph.layerless_nodes[node].kind != LNodeKind::ExternalPort {
            continue;
        }

        match graph.layerless_nodes[node].external_port_side {
            PortSide::North => {
                graph.layerless_nodes[node].position.y = -graph.offset.y - graph.padding.top;
            }
            PortSide::South => {
                graph.layerless_nodes[node].position.y =
                    graph.size.height + graph.padding.bottom - graph.offset.y;
            }
            PortSide::East | PortSide::West | PortSide::Undefined => {}
        }
    }
}

fn fix_hierarchical_port_east_west_y_coordinate(
    graph: &mut LGraph,
    node: usize,
    graph_actual_height: f64,
) -> f64 {
    let mut required_actual_graph_height = 0.0;

    if graph.options.port_constraints.is_ratio_fixed() {
        graph.layerless_nodes[node].position.y = graph_actual_height
            * graph.layerless_nodes[node].port_ratio_or_position
            - first_port_anchor_y(graph, node);
        required_actual_graph_height = graph.layerless_nodes[node].position.y
            + graph.layerless_nodes[node].external_port_size.height;
        border_to_content_area_y(graph, node);
    } else if graph.options.port_constraints.is_pos_fixed() {
        graph.layerless_nodes[node].position.y =
            graph.layerless_nodes[node].port_ratio_or_position - first_port_anchor_y(graph, node);
        required_actual_graph_height = graph.layerless_nodes[node].position.y
            + graph.layerless_nodes[node].external_port_size.height;
        border_to_content_area_y(graph, node);
    }

    required_actual_graph_height
}

fn correct_hierarchical_port_slanted_edge_segments(graph: &mut LGraph) {
    correct_hierarchical_port_slanted_edge_segments_for_layer(graph, 0);
    correct_hierarchical_port_slanted_edge_segments_for_layer(graph, graph.layers.len() - 1);
}

fn correct_hierarchical_port_slanted_edge_segments_for_layer(
    graph: &mut LGraph,
    layer_index: usize,
) {
    let nodes = graph.layers[layer_index].nodes.clone();

    for node in nodes {
        if graph.layerless_nodes[node].kind != LNodeKind::ExternalPort
            || !matches!(
                graph.layerless_nodes[node].external_port_side,
                PortSide::East | PortSide::West
            )
        {
            continue;
        }

        let connected_edges = graph.node_connected_edges(node);
        for edge in connected_edges {
            if graph.edges[edge].bend_points.is_empty() {
                continue;
            }

            let source = graph.edges[edge].source;
            if source.node == node {
                let source_y = absolute_port_anchor(graph, source).y;
                if let Some(first) = graph.edges[edge].bend_points.first_mut() {
                    first.y = source_y;
                }
            }

            let target = graph.edges[edge].target;
            if target.node == node {
                let target_y = absolute_port_anchor(graph, target).y;
                if let Some(last) = graph.edges[edge].bend_points.last_mut() {
                    last.y = target_y;
                }
            }
        }
    }
}

fn absolute_port_anchor(graph: &LGraph, port_ref: PortRef) -> LPoint {
    let node = &graph.layerless_nodes[port_ref.node];
    let port = &node.ports[port_ref.port];
    LPoint {
        x: node.position.x + port.position.x + port.anchor.x,
        y: node.position.y + port.position.y + port.anchor.y,
    }
}

pub fn process_hierarchical_port_constraints(graph: &mut LGraph) {
    process_east_west_hierarchical_port_constraints(graph);
    process_north_south_hierarchical_port_constraints(graph);
}

fn process_east_west_hierarchical_port_constraints(graph: &mut LGraph) {
    if !graph.options.port_constraints.is_order_fixed() || graph.layers.is_empty() {
        return;
    }

    process_east_west_hierarchical_port_constraints_for_layer(graph, 0);
    if graph.layers.len() > 1 {
        process_east_west_hierarchical_port_constraints_for_layer(graph, graph.layers.len() - 1);
    }
}

fn process_east_west_hierarchical_port_constraints_for_layer(
    graph: &mut LGraph,
    layer_index: usize,
) {
    let mut nodes = graph.layers[layer_index].nodes.clone();
    nodes.sort_by(|left, right| {
        let left_node = &graph.layerless_nodes[*left];
        let right_node = &graph.layerless_nodes[*right];

        match (
            left_node.kind == LNodeKind::ExternalPort,
            right_node.kind == LNodeKind::ExternalPort,
        ) {
            (_, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            (true, true) => left_node
                .port_ratio_or_position
                .total_cmp(&right_node.port_ratio_or_position),
        }
    });

    let mut last_hierarchical_dummy: Option<usize> = None;
    for node in nodes {
        if graph.layerless_nodes[node].kind != LNodeKind::ExternalPort {
            break;
        }
        if !matches!(
            graph.layerless_nodes[node].external_port_side,
            PortSide::West | PortSide::East
        ) {
            continue;
        }

        if let Some(previous) = last_hierarchical_dummy {
            graph.layerless_nodes[previous]
                .in_layer_successor_constraints
                .push(node);
        }
        last_hierarchical_dummy = Some(node);
    }
}

fn process_north_south_hierarchical_port_constraints(graph: &mut LGraph) {
    if !graph.options.port_constraints.is_side_fixed() {
        return;
    }

    let original_layer_count = graph.layers.len();
    let mut new_dummy_nodes = vec![Vec::<usize>::new(); original_layer_count + 2];
    let mut replacement_maps =
        vec![Vec::<(NorthSouthReplacementKey, usize)>::new(); original_layer_count + 2];
    let mut original_external_port_dummies = Vec::new();

    for layer_index in 0..original_layer_count {
        let current_nodes = graph.layers[layer_index].nodes.clone();

        for current_node in current_nodes {
            if is_north_south_external_port_dummy(graph, current_node) {
                original_external_port_dummies.push(current_node);
                continue;
            }

            let incoming_edges = graph.node_incoming_edges(current_node);
            for edge in incoming_edges {
                let source_node = graph.edges[edge].source.node;
                if !is_north_south_external_port_dummy(graph, source_node) {
                    continue;
                }

                let replacement = replacement_dummy_for_original(
                    graph,
                    source_node,
                    layer_index,
                    &mut replacement_maps,
                    &mut new_dummy_nodes,
                );
                graph.set_edge_source(
                    edge,
                    PortRef {
                        node: replacement,
                        port: 1,
                    },
                );
            }

            let outgoing_edges = graph.node_outgoing_edges(current_node);
            for edge in outgoing_edges {
                let target_node = graph.edges[edge].target.node;
                if !is_north_south_external_port_dummy(graph, target_node) {
                    continue;
                }

                let replacement = replacement_dummy_for_original(
                    graph,
                    target_node,
                    layer_index + 2,
                    &mut replacement_maps,
                    &mut new_dummy_nodes,
                );
                graph.set_edge_target(
                    edge,
                    PortRef {
                        node: replacement,
                        port: 0,
                    },
                );
            }
        }
    }

    for (virtual_layer, nodes) in new_dummy_nodes.into_iter().enumerate() {
        if nodes.is_empty() {
            continue;
        }

        let layer_index = if virtual_layer == 0 {
            graph.insert_layer(0);
            0
        } else if virtual_layer == original_layer_count + 1 {
            graph.layers.push(Default::default());
            graph.layers.len() - 1
        } else {
            virtual_layer - 1
        };

        for node in nodes {
            graph.set_node_layer(node, layer_index);
        }
    }

    for original_dummy in original_external_port_dummies {
        graph.remove_node_from_layer(original_dummy);
        if !graph
            .replaced_external_port_dummies
            .contains(&original_dummy)
        {
            graph.replaced_external_port_dummies.push(original_dummy);
        }
    }
}

fn is_north_south_external_port_dummy(graph: &LGraph, node: usize) -> bool {
    let node = &graph.layerless_nodes[node];
    node.kind == LNodeKind::ExternalPort
        && matches!(node.external_port_side, PortSide::North | PortSide::South)
}

fn replacement_dummy_for_original(
    graph: &mut LGraph,
    original_dummy: usize,
    virtual_layer: usize,
    maps: &mut [Vec<(NorthSouthReplacementKey, usize)>],
    new_dummy_nodes: &mut [Vec<usize>],
) -> usize {
    let key = north_south_replacement_key(graph, original_dummy);
    if let Some((_, replacement)) = maps[virtual_layer]
        .iter()
        .find(|(candidate, _)| candidate.same_target(&key))
    {
        return *replacement;
    }

    let replacement = create_hierarchical_port_replacement_dummy(graph, original_dummy);
    maps[virtual_layer].push((key, replacement));
    new_dummy_nodes[virtual_layer].push(replacement);
    replacement
}

#[derive(Debug, Clone)]
enum NorthSouthReplacementKey {
    Origin(GraphPortRef),
    Dummy(usize),
}

impl NorthSouthReplacementKey {
    fn same_target(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Origin(first), Self::Origin(second)) => first.same_target(second),
            (Self::Dummy(first), Self::Dummy(second)) => first == second,
            _ => false,
        }
    }
}

fn north_south_replacement_key(graph: &LGraph, original_dummy: usize) -> NorthSouthReplacementKey {
    graph
        .layerless_nodes
        .get(original_dummy)
        .and_then(|node| node.origin_port.as_ref())
        .cloned()
        .map(NorthSouthReplacementKey::Origin)
        .unwrap_or(NorthSouthReplacementKey::Dummy(original_dummy))
}

fn create_hierarchical_port_replacement_dummy(graph: &mut LGraph, original_dummy: usize) -> usize {
    let original = graph.layerless_nodes[original_dummy].copy_preserving_reference_domains();
    let node_index = graph.layerless_nodes.len();
    let mut replacement = LNode::new(
        format!("{}:replacement:{node_index}", original.id),
        original.size.width,
        original.size.height,
        None,
    );
    replacement.kind = LNodeKind::ExternalPort;
    replacement.margin = original.margin;
    replacement.padding = original.padding;
    replacement.node_label_placement = original.node_label_placement;
    replacement.model_order = original.model_order;
    replacement.layer_constraint = original.layer_constraint;
    replacement.external_port_side = original.external_port_side;
    replacement.external_port_size = original.external_port_size;
    replacement.port_ratio_or_position = original.port_ratio_or_position;
    replacement.replaced_external_port_dummy = Some(original_dummy);
    replacement.in_layer_successor_constraints = original.in_layer_successor_constraints;
    replacement.origin_port = original.origin_port;
    replacement.origin_edge = original.origin_edge;
    replacement.label_side = original.label_side;
    replacement.in_layer_constraint = original.in_layer_constraint;
    replacement.layer_constraint_explicit = original.layer_constraint_explicit;
    replacement.compound = original.compound;
    replacement.port_constraints = PortConstraints::FixedPos;
    replacement.node_alignment = Alignment::Center;
    graph.layerless_nodes.push(replacement);

    graph
        .add_port(
            node_index,
            PortType::Input,
            PortSide::West,
            Default::default(),
        )
        .expect("replacement node was just inserted");
    graph
        .add_port(
            node_index,
            PortType::Output,
            PortSide::East,
            Default::default(),
        )
        .expect("replacement node was just inserted");
    node_index
}
