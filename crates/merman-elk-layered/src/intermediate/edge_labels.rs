//! Center-label dummy processing and end-label placement.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/LabelDummyInserter.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/LabelDummySwitcher.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/LabelSideSelector.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/LabelDummyRemover.java

use std::collections::VecDeque;

use super::{first_port_on_side, join_long_edge_at, split_edge};
use crate::graph::{
    EdgeLabelPlacement, HorizontalLabelAlignment, LGraph, LLabel, LNode, LNodeKind, LPoint, LSize,
    LabelCellLayout, LabelSide, PortRef, PortSide, VerticalLabelAlignment,
};
use crate::options::{EdgeLabelSideSelection, ElkDirection, PortConstraints};

pub fn insert_label_dummies(graph: &mut LGraph) {
    let edge_indices = graph
        .edges
        .iter()
        .enumerate()
        .filter(|(_, edge)| edge.source.node != edge.target.node)
        .filter(|(_, edge)| {
            edge.labels
                .iter()
                .any(|label| label.placement == EdgeLabelPlacement::Center)
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();

    for edge_index in edge_indices {
        if !graph.edge_source_attached(edge_index) || !graph.edge_target_attached(edge_index) {
            continue;
        }

        let thickness = graph.edges[edge_index].thickness.max(0.0);
        graph.edges[edge_index].thickness = thickness;
        let original_source = graph.edges[edge_index].source;
        let original_target = graph.edges[edge_index].target;
        let dummy = create_label_dummy_node(graph, edge_index, thickness);
        if split_edge(graph, edge_index, dummy).is_none() {
            continue;
        }

        graph.layerless_nodes[dummy].long_edge_source = Some(original_source);
        graph.layerless_nodes[dummy].long_edge_target = Some(original_target);
        move_center_labels_to_dummy(graph, edge_index, dummy);
        size_label_dummy(graph, dummy, thickness);
    }
}

fn create_label_dummy_node(graph: &mut LGraph, edge_index: usize, thickness: f64) -> usize {
    let dummy_index = graph.layerless_nodes.len();
    let mut dummy = LNode::new(format!("label:{edge_index}:{dummy_index}"), 0.0, 0.0, None);
    dummy.kind = LNodeKind::Label;
    dummy.origin_edge = Some(edge_index);
    dummy.port_constraints = PortConstraints::FixedPos;
    graph.layerless_nodes.push(dummy);
    graph.layerless_nodes[dummy_index].size.height = thickness.max(0.0);
    dummy_index
}

fn move_center_labels_to_dummy(graph: &mut LGraph, edge_index: usize, dummy: usize) {
    let mut retained = Vec::new();
    let mut moved = Vec::new();

    for label in std::mem::take(&mut graph.edges[edge_index].labels) {
        if label.placement == EdgeLabelPlacement::Center {
            moved.push(label);
        } else {
            retained.push(label);
        }
    }

    graph.edges[edge_index].labels = retained;
    graph.layerless_nodes[dummy].represented_labels = moved;
}

fn size_label_dummy(graph: &mut LGraph, dummy: usize, thickness: f64) {
    let edge_label_spacing = graph.options.spacing.edge_label;
    let label_label_spacing = graph.options.spacing.label_label;
    let mut width = 0.0;
    // LongEdgeSplitter initializes the dummy's height to the edge thickness before
    // LabelDummyInserter adds the represented labels and its own thickness allowance.
    let mut height = thickness;

    if graph.options.direction.is_vertical() {
        for label in &graph.layerless_nodes[dummy].represented_labels {
            width += label.size.width + label_label_spacing;
            height = f64::max(height, label.size.height);
        }
        if !graph.layerless_nodes[dummy].represented_labels.is_empty() {
            width -= label_label_spacing;
        }
        height += edge_label_spacing + thickness;
    } else {
        for label in &graph.layerless_nodes[dummy].represented_labels {
            width = f64::max(width, label.size.width);
            height += label.size.height + label_label_spacing;
        }
        if !graph.layerless_nodes[dummy].represented_labels.is_empty() {
            height -= label_label_spacing;
        }
        height += edge_label_spacing + thickness;
    }

    graph.layerless_nodes[dummy].size.width = width.max(0.0);
    graph.layerless_nodes[dummy].size.height = height.max(thickness);
}

pub fn switch_label_dummies(graph: &mut LGraph) {
    let label_dummies = graph
        .layers
        .iter()
        .flat_map(|layer| layer.nodes.iter().copied())
        .filter(|node| graph.layerless_nodes[*node].kind == LNodeKind::Label)
        .collect::<Vec<_>>();

    for label_dummy in label_dummies {
        if let Some(info) = LabelDummyInfo::new(graph, label_dummy) {
            let target_layer = find_median_layer_target_id(&info);
            assign_label_dummy_layer(graph, &info, target_layer);
            update_long_edge_before_label_dummy_info(graph, info.label_dummy);
        }
    }
}

pub fn select_label_sides(graph: &mut LGraph) {
    match graph.options.edge_label_side_selection {
        EdgeLabelSideSelection::AlwaysUp => same_side(graph, LabelSide::Above),
        EdgeLabelSideSelection::AlwaysDown => same_side(graph, LabelSide::Below),
        EdgeLabelSideSelection::DirectionUp => based_on_direction(graph, LabelSide::Above),
        EdgeLabelSideSelection::DirectionDown => based_on_direction(graph, LabelSide::Below),
        EdgeLabelSideSelection::SmartUp => smart(graph, LabelSide::Above),
        EdgeLabelSideSelection::SmartDown => smart(graph, LabelSide::Below),
    }
}

pub fn preprocess_end_labels(graph: &mut LGraph) {
    let edge_label_spacing = graph.options.spacing.edge_label;
    let label_label_spacing = graph.options.spacing.label_label;
    let vertical_layout = graph.options.direction.is_vertical();
    let nodes = graph
        .layers
        .iter()
        .flat_map(|layer| layer.nodes.iter().copied())
        .collect::<Vec<_>>();

    for node in nodes {
        if !matches!(
            graph.layerless_nodes[node].kind,
            LNodeKind::Normal | LNodeKind::ExternalPort
        ) {
            continue;
        }
        gather_end_labels_for_node(graph, node);
        place_end_label_cells_for_node(
            graph,
            node,
            label_label_spacing,
            edge_label_spacing,
            vertical_layout,
        );
    }
}

fn same_side(graph: &mut LGraph, label_side: LabelSide) {
    for layer_index in 0..graph.layers.len() {
        let nodes = graph.layers[layer_index].nodes.clone();
        for node in nodes {
            if graph.layerless_nodes[node].kind == LNodeKind::Label {
                apply_label_side(graph, node, label_side);
            }

            let outgoing_edges = graph.node_outgoing_edges(node);
            for edge in outgoing_edges {
                apply_label_side_to_edge(graph, edge, label_side);
            }
        }
    }
}

fn based_on_direction(graph: &mut LGraph, side_for_rightward_edges: LabelSide) {
    for layer_index in 0..graph.layers.len() {
        let nodes = graph.layers[layer_index].nodes.clone();
        for node in nodes {
            if graph.layerless_nodes[node].kind == LNodeKind::Label {
                let side = if does_label_dummy_point_right(graph, node) {
                    side_for_rightward_edges
                } else {
                    side_for_rightward_edges.opposite()
                };
                apply_label_side(graph, node, side);
            }

            let outgoing_edges = graph.node_outgoing_edges(node);
            for edge in outgoing_edges {
                let side = if !graph.edges[edge].reversed {
                    side_for_rightward_edges
                } else {
                    side_for_rightward_edges.opposite()
                };
                apply_label_side_to_edge(graph, edge, side);
            }
        }
    }
}

fn smart(graph: &mut LGraph, default_side: LabelSide) {
    let mut dummy_node_queue = VecDeque::new();

    for layer_index in 0..graph.layers.len() {
        let nodes = graph.layers[layer_index].nodes.clone();
        let mut top_group = true;
        let mut label_dummies_in_queue = 0usize;

        for node in nodes {
            match graph.layerless_nodes[node].kind {
                LNodeKind::Label => {
                    label_dummies_in_queue += 1;
                    dummy_node_queue.push_back(node);
                }
                LNodeKind::LongEdge => {
                    dummy_node_queue.push_back(node);
                }
                LNodeKind::Normal => {
                    smart_for_regular_node(graph, node, default_side);
                    if !dummy_node_queue.is_empty() {
                        smart_for_consecutive_dummy_node_run(
                            graph,
                            &mut dummy_node_queue,
                            label_dummies_in_queue,
                            top_group,
                            false,
                            default_side,
                        );
                    }
                    top_group = false;
                    label_dummies_in_queue = 0;
                }
                _ => {
                    if !dummy_node_queue.is_empty() {
                        smart_for_consecutive_dummy_node_run(
                            graph,
                            &mut dummy_node_queue,
                            label_dummies_in_queue,
                            top_group,
                            false,
                            default_side,
                        );
                    }
                    top_group = false;
                    label_dummies_in_queue = 0;
                }
            }
        }

        if !dummy_node_queue.is_empty() {
            smart_for_consecutive_dummy_node_run(
                graph,
                &mut dummy_node_queue,
                label_dummies_in_queue,
                top_group,
                true,
                default_side,
            );
        }
    }
}

fn smart_for_consecutive_dummy_node_run(
    graph: &mut LGraph,
    dummy_nodes: &mut VecDeque<usize>,
    label_dummy_count: usize,
    top_group: bool,
    bottom_group: bool,
    default_side: LabelSide,
) {
    debug_assert!(!dummy_nodes.is_empty());

    if top_group
        && (!bottom_group || dummy_nodes.len() > 1)
        && label_dummy_count == 1
        && dummy_nodes
            .front()
            .is_some_and(|node| graph.layerless_nodes[*node].kind == LNodeKind::Label)
    {
        if let Some(node) = dummy_nodes.front().copied() {
            apply_label_side(graph, node, LabelSide::Above);
        }
    } else if bottom_group
        && (!top_group || dummy_nodes.len() > 1)
        && label_dummy_count == 1
        && dummy_nodes
            .back()
            .is_some_and(|node| graph.layerless_nodes[*node].kind == LNodeKind::Label)
    {
        if let Some(node) = dummy_nodes.back().copied() {
            apply_label_side(graph, node, LabelSide::Below);
        }
    } else if dummy_nodes.len() == 2 {
        if let Some(node) = dummy_nodes.pop_front() {
            apply_label_side(graph, node, LabelSide::Above);
        }
        if let Some(node) = dummy_nodes.pop_front() {
            apply_label_side(graph, node, LabelSide::Below);
        }
    } else {
        apply_for_dummy_node_run_with_simple_loops(graph, dummy_nodes, default_side);
    }

    dummy_nodes.clear();
}

fn apply_for_dummy_node_run_with_simple_loops(
    graph: &mut LGraph,
    dummy_nodes: &VecDeque<usize>,
    default_side: LabelSide,
) {
    let mut label_dummy_run = Vec::with_capacity(dummy_nodes.len());
    let mut prev_long_edge_source = None;
    let mut prev_long_edge_target = None;

    for current_dummy in dummy_nodes.iter().copied() {
        let curr_long_edge_source = get_long_edge_end_node(graph, current_dummy, true);
        let curr_long_edge_target = get_long_edge_end_node(graph, current_dummy, false);

        if prev_long_edge_source != curr_long_edge_source
            || prev_long_edge_target != curr_long_edge_target
        {
            apply_label_sides_to_label_dummy_run(graph, &mut label_dummy_run, default_side);
            prev_long_edge_source = curr_long_edge_source;
            prev_long_edge_target = curr_long_edge_target;
        }

        label_dummy_run.push(current_dummy);
    }

    apply_label_sides_to_label_dummy_run(graph, &mut label_dummy_run, default_side);
}

fn get_long_edge_end_node(graph: &LGraph, label_dummy: usize, source: bool) -> Option<usize> {
    let port = if source {
        graph.layerless_nodes[label_dummy].long_edge_source
    } else {
        graph.layerless_nodes[label_dummy].long_edge_target
    }?;
    Some(port.node)
}

fn apply_label_sides_to_label_dummy_run(
    graph: &mut LGraph,
    label_dummy_run: &mut Vec<usize>,
    default_side: LabelSide,
) {
    if label_dummy_run.is_empty() {
        return;
    }

    if label_dummy_run.len() == 2 {
        if let Some(node) = label_dummy_run.first().copied() {
            apply_label_side(graph, node, LabelSide::Above);
        }
        if let Some(node) = label_dummy_run.get(1).copied() {
            apply_label_side(graph, node, LabelSide::Below);
        }
    } else {
        for dummy_node in label_dummy_run.iter().copied() {
            apply_label_side(graph, dummy_node, default_side);
        }
    }

    label_dummy_run.clear();
}

fn smart_for_regular_node(graph: &mut LGraph, node: usize, default_side: LabelSide) {
    let mut end_label_queue = VecDeque::new();
    let mut current_port_side = None;
    let port_count = graph.layerless_nodes[node].ports.len();

    for port_index in 0..port_count {
        let port_side = graph.layerless_nodes[node].ports[port_index].side;
        if Some(port_side) != current_port_side {
            if !end_label_queue.is_empty() {
                smart_for_regular_node_port_end_labels(
                    graph,
                    node,
                    &mut end_label_queue,
                    current_port_side,
                    default_side,
                );
            }

            end_label_queue.clear();
            current_port_side = Some(port_side);
        }

        if port_has_incident_end_labels(graph, node, port_index) {
            end_label_queue.push_back(port_index);
        }
    }

    if !end_label_queue.is_empty() {
        smart_for_regular_node_port_end_labels(
            graph,
            node,
            &mut end_label_queue,
            current_port_side,
            default_side,
        );
    }
}

fn smart_for_regular_node_port_end_labels(
    graph: &mut LGraph,
    node: usize,
    end_label_queue: &mut VecDeque<usize>,
    port_side: Option<PortSide>,
    default_side: LabelSide,
) {
    debug_assert!(!end_label_queue.is_empty());
    let Some(port_side) = port_side else {
        return;
    };

    if end_label_queue.len() == 2 {
        if matches!(port_side, PortSide::North | PortSide::East) {
            if let Some(port_index) = end_label_queue.pop_front() {
                apply_label_side_to_port_labels(graph, node, port_index, LabelSide::Above);
            }
            if let Some(port_index) = end_label_queue.pop_front() {
                apply_label_side_to_port_labels(graph, node, port_index, LabelSide::Below);
            }
        } else {
            if let Some(port_index) = end_label_queue.pop_front() {
                apply_label_side_to_port_labels(graph, node, port_index, LabelSide::Below);
            }
            if let Some(port_index) = end_label_queue.pop_front() {
                apply_label_side_to_port_labels(graph, node, port_index, LabelSide::Above);
            }
        }
    } else {
        for port_index in end_label_queue.iter().copied() {
            apply_label_side_to_port_labels(graph, node, port_index, default_side);
        }
    }

    end_label_queue.clear();
}

fn apply_label_side_to_port_labels(
    graph: &mut LGraph,
    node: usize,
    port_index: usize,
    side: LabelSide,
) {
    for edge in incident_end_label_edges_for_port(graph, node, port_index) {
        let placement = end_label_placement_for_port(graph, edge, node, port_index);
        for label in &mut graph.edges[edge].labels {
            if Some(label.placement) == placement {
                label.label_side = Some(side);
            }
        }
    }
    for label in &mut graph.layerless_nodes[node].ports[port_index].labels {
        label.label_side = Some(side);
    }
}

fn apply_label_side_to_edge(graph: &mut LGraph, edge: usize, side: LabelSide) {
    for label in &mut graph.edges[edge].labels {
        label.label_side = Some(side);
    }
}

fn port_has_incident_end_labels(graph: &LGraph, node: usize, port_index: usize) -> bool {
    incident_end_label_edges_for_port(graph, node, port_index)
        .into_iter()
        .any(|edge| {
            let placement = end_label_placement_for_port(graph, edge, node, port_index);
            graph.edges[edge]
                .labels
                .iter()
                .any(|label| Some(label.placement) == placement)
        })
}

fn incident_end_label_edges_for_port(graph: &LGraph, node: usize, port_index: usize) -> Vec<usize> {
    let port = &graph.layerless_nodes[node].ports[port_index];
    port.incoming_edges
        .iter()
        .chain(port.outgoing_edges.iter())
        .copied()
        .collect()
}

fn end_label_placement_for_port(
    graph: &LGraph,
    edge: usize,
    node: usize,
    port_index: usize,
) -> Option<EdgeLabelPlacement> {
    if graph.edges[edge].source.node == node && graph.edges[edge].source.port == port_index {
        Some(EdgeLabelPlacement::Tail)
    } else if graph.edges[edge].target.node == node && graph.edges[edge].target.port == port_index {
        Some(EdgeLabelPlacement::Head)
    } else {
        None
    }
}

fn gather_end_labels_for_node(graph: &mut LGraph, node: usize) {
    let port_count = graph.layerless_nodes[node].ports.len();
    for port_index in 0..port_count {
        graph.layerless_nodes[node].ports[port_index].labels.clear();
        graph.layerless_nodes[node].ports[port_index].end_label_cell = None;
        let incident_edges = incident_end_label_edges_for_port(graph, node, port_index);
        for edge in incident_edges {
            let Some(placement) = end_label_placement_for_port(graph, edge, node, port_index)
            else {
                continue;
            };

            let labels = std::mem::take(&mut graph.edges[edge].labels);
            let mut retained = Vec::with_capacity(labels.len());
            for mut label in labels {
                if label.placement == placement {
                    if label.end_label_edge.is_none() {
                        label.end_label_edge = Some(edge);
                    }
                    graph.layerless_nodes[node].ports[port_index]
                        .labels
                        .push(label);
                } else {
                    retained.push(label);
                }
            }
            graph.edges[edge].labels = retained;
        }
    }
}

fn place_end_label_cells_for_node(
    graph: &mut LGraph,
    node: usize,
    label_label_spacing: f64,
    edge_label_spacing: f64,
    vertical_layout: bool,
) {
    let port_count = graph.layerless_nodes[node].ports.len();
    for port_index in 0..port_count {
        if graph.layerless_nodes[node].ports[port_index]
            .labels
            .is_empty()
        {
            continue;
        }
        let cell = create_end_label_cell(
            &graph.layerless_nodes[node].ports[port_index].labels,
            label_label_spacing,
            vertical_layout,
        );
        graph.layerless_nodes[node].ports[port_index].end_label_cell = Some(cell);
        place_end_label_cell_for_port(graph, node, port_index, edge_label_spacing);
    }

    if vertical_layout {
        remove_end_label_overlaps(graph, node, PortSide::East, 2.0 * label_label_spacing);
        remove_end_label_overlaps(graph, node, PortSide::West, 2.0 * label_label_spacing);
    } else {
        remove_end_label_overlaps(graph, node, PortSide::North, 2.0 * label_label_spacing);
        remove_end_label_overlaps(graph, node, PortSide::South, 2.0 * label_label_spacing);
    }

    update_end_label_node_margin(graph, node);
}

fn create_end_label_cell(
    labels: &[LLabel],
    label_label_spacing: f64,
    vertical_layout: bool,
) -> LabelCellLayout {
    let horizontal_layout = !vertical_layout;
    let size = label_cell_size(labels, label_label_spacing, horizontal_layout);
    LabelCellLayout {
        position: LPoint::default(),
        size,
        horizontal_layout,
        horizontal_alignment: HorizontalLabelAlignment::Center,
        vertical_alignment: VerticalLabelAlignment::Center,
    }
}

fn label_cell_size(labels: &[LLabel], label_label_spacing: f64, horizontal_layout: bool) -> LSize {
    let mut width: f64 = 0.0;
    let mut height: f64 = 0.0;

    if horizontal_layout {
        for label in labels {
            width += label.size.width + label_label_spacing;
            height = height.max(label.size.height);
        }
        if !labels.is_empty() {
            width -= label_label_spacing;
        }
    } else {
        for label in labels {
            width = width.max(label.size.width);
            height += label.size.height + label_label_spacing;
        }
        if !labels.is_empty() {
            height -= label_label_spacing;
        }
    }

    LSize { width, height }
}

fn place_end_label_cell_for_port(
    graph: &mut LGraph,
    node: usize,
    port_index: usize,
    edge_label_spacing: f64,
) {
    let node_size = graph.layerless_nodes[node].size;
    let node_margin = graph.layerless_nodes[node].margin;
    let port = graph.layerless_nodes[node].ports[port_index].clone();
    let Some(mut cell) = port.end_label_cell else {
        return;
    };
    let port_anchor = LPoint {
        x: port.position.x + port.anchor.x,
        y: port.position.y + port.anchor.y,
    };
    let max_edge_thickness = max_incident_edge_thickness(graph, &port);
    let label_side = port
        .labels
        .first()
        .and_then(|label| label.label_side)
        .unwrap_or(LabelSide::Below);

    match port.side {
        PortSide::North => {
            cell.vertical_alignment = VerticalLabelAlignment::Bottom;
            cell.position.y = -node_margin.top - edge_label_spacing - cell.size.height;
            if label_side == LabelSide::Above {
                cell.horizontal_alignment = HorizontalLabelAlignment::Right;
                cell.position.x =
                    port_anchor.x - max_edge_thickness - edge_label_spacing - cell.size.width;
            } else {
                cell.horizontal_alignment = HorizontalLabelAlignment::Left;
                cell.position.x = port_anchor.x + max_edge_thickness + edge_label_spacing;
            }
        }
        PortSide::East => {
            cell.horizontal_alignment = HorizontalLabelAlignment::Left;
            cell.position.x = node_size.width + node_margin.right + edge_label_spacing;
            if label_side == LabelSide::Above {
                cell.vertical_alignment = VerticalLabelAlignment::Bottom;
                cell.position.y =
                    port_anchor.y - max_edge_thickness - edge_label_spacing - cell.size.height;
            } else {
                cell.vertical_alignment = VerticalLabelAlignment::Top;
                cell.position.y = port_anchor.y + max_edge_thickness + edge_label_spacing;
            }
        }
        PortSide::South => {
            cell.vertical_alignment = VerticalLabelAlignment::Top;
            cell.position.y = node_size.height + node_margin.bottom + edge_label_spacing;
            if label_side == LabelSide::Above {
                cell.horizontal_alignment = HorizontalLabelAlignment::Right;
                cell.position.x =
                    port_anchor.x - max_edge_thickness - edge_label_spacing - cell.size.width;
            } else {
                cell.horizontal_alignment = HorizontalLabelAlignment::Left;
                cell.position.x = port_anchor.x + max_edge_thickness + edge_label_spacing;
            }
        }
        PortSide::West => {
            cell.horizontal_alignment = HorizontalLabelAlignment::Right;
            cell.position.x = -node_margin.left - edge_label_spacing - cell.size.width;
            if label_side == LabelSide::Above {
                cell.vertical_alignment = VerticalLabelAlignment::Bottom;
                cell.position.y =
                    port_anchor.y - max_edge_thickness - edge_label_spacing - cell.size.height;
            } else {
                cell.vertical_alignment = VerticalLabelAlignment::Top;
                cell.position.y = port_anchor.y + max_edge_thickness + edge_label_spacing;
            }
        }
        PortSide::Undefined => {}
    }

    graph.layerless_nodes[node].ports[port_index].end_label_cell = Some(cell);
}

fn max_incident_edge_thickness(graph: &LGraph, port: &crate::graph::LPort) -> f64 {
    port.incoming_edges
        .iter()
        .chain(port.outgoing_edges.iter())
        .filter_map(|edge| graph.edges.get(*edge).map(|edge| edge.thickness.max(0.0)))
        .fold(0.0, f64::max)
}

fn remove_end_label_overlaps(graph: &mut LGraph, node: usize, port_side: PortSide, gap: f64) {
    let mut ports = graph.layerless_nodes[node]
        .ports
        .iter()
        .enumerate()
        .filter_map(|(port, port_data)| {
            (port_data.side == port_side && port_data.end_label_cell.is_some()).then_some(port)
        })
        .collect::<Vec<_>>();

    if ports.len() <= 1 {
        return;
    }

    match port_side {
        PortSide::North => {
            ports.sort_by(|a, b| {
                graph.layerless_nodes[node].ports[*b]
                    .end_label_cell
                    .unwrap()
                    .position
                    .y
                    .total_cmp(
                        &graph.layerless_nodes[node].ports[*a]
                            .end_label_cell
                            .unwrap()
                            .position
                            .y,
                    )
            });
            let mut limit = f64::INFINITY;
            for port in ports {
                let mut cell = graph.layerless_nodes[node].ports[port]
                    .end_label_cell
                    .unwrap();
                let max_y = if limit.is_finite() {
                    limit - gap - cell.size.height
                } else {
                    cell.position.y
                };
                cell.position.y = cell.position.y.min(max_y);
                limit = cell.position.y;
                graph.layerless_nodes[node].ports[port].end_label_cell = Some(cell);
            }
        }
        PortSide::South => {
            ports.sort_by(|a, b| {
                graph.layerless_nodes[node].ports[*a]
                    .end_label_cell
                    .unwrap()
                    .position
                    .y
                    .total_cmp(
                        &graph.layerless_nodes[node].ports[*b]
                            .end_label_cell
                            .unwrap()
                            .position
                            .y,
                    )
            });
            let mut limit = f64::NEG_INFINITY;
            for port in ports {
                let mut cell = graph.layerless_nodes[node].ports[port]
                    .end_label_cell
                    .unwrap();
                let min_y = if limit.is_finite() {
                    limit + gap
                } else {
                    cell.position.y
                };
                cell.position.y = cell.position.y.max(min_y);
                limit = cell.position.y + cell.size.height;
                graph.layerless_nodes[node].ports[port].end_label_cell = Some(cell);
            }
        }
        PortSide::East => {
            ports.sort_by(|a, b| {
                graph.layerless_nodes[node].ports[*a]
                    .end_label_cell
                    .unwrap()
                    .position
                    .x
                    .total_cmp(
                        &graph.layerless_nodes[node].ports[*b]
                            .end_label_cell
                            .unwrap()
                            .position
                            .x,
                    )
            });
            let mut limit = f64::NEG_INFINITY;
            for port in ports {
                let mut cell = graph.layerless_nodes[node].ports[port]
                    .end_label_cell
                    .unwrap();
                let min_x = if limit.is_finite() {
                    limit + gap
                } else {
                    cell.position.x
                };
                cell.position.x = cell.position.x.max(min_x);
                limit = cell.position.x + cell.size.width;
                graph.layerless_nodes[node].ports[port].end_label_cell = Some(cell);
            }
        }
        PortSide::West => {
            ports.sort_by(|a, b| {
                graph.layerless_nodes[node].ports[*b]
                    .end_label_cell
                    .unwrap()
                    .position
                    .x
                    .total_cmp(
                        &graph.layerless_nodes[node].ports[*a]
                            .end_label_cell
                            .unwrap()
                            .position
                            .x,
                    )
            });
            let mut limit = f64::INFINITY;
            for port in ports {
                let mut cell = graph.layerless_nodes[node].ports[port]
                    .end_label_cell
                    .unwrap();
                let max_x = if limit.is_finite() {
                    limit - gap - cell.size.width
                } else {
                    cell.position.x
                };
                cell.position.x = cell.position.x.min(max_x);
                limit = cell.position.x;
                graph.layerless_nodes[node].ports[port].end_label_cell = Some(cell);
            }
        }
        PortSide::Undefined => {}
    }
}

fn update_end_label_node_margin(graph: &mut LGraph, node: usize) {
    let node_size = graph.layerless_nodes[node].size;
    let node_margin = graph.layerless_nodes[node].margin;
    let mut min_x = -node_margin.left;
    let mut min_y = -node_margin.top;
    let mut max_x = node_size.width + node_margin.right;
    let mut max_y = node_size.height + node_margin.bottom;

    for port in &graph.layerless_nodes[node].ports {
        if let Some(cell) = port.end_label_cell {
            min_x = min_x.min(cell.position.x);
            min_y = min_y.min(cell.position.y);
            max_x = max_x.max(cell.position.x + cell.size.width);
            max_y = max_y.max(cell.position.y + cell.size.height);
        }
    }

    graph.layerless_nodes[node].margin.left = (-min_x).max(0.0);
    graph.layerless_nodes[node].margin.top = (-min_y).max(0.0);
    graph.layerless_nodes[node].margin.right =
        (max_x - graph.layerless_nodes[node].margin.left - node_size.width).max(0.0);
    graph.layerless_nodes[node].margin.bottom =
        (max_y - graph.layerless_nodes[node].margin.top - node_size.height).max(0.0);
}

pub fn sort_end_labels(graph: &mut LGraph) {
    let edge_sort_keys = (0..graph.edges.len())
        .map(|edge| end_label_sort_key(graph, edge))
        .collect::<Vec<_>>();

    for node in &mut graph.layerless_nodes {
        for port in &mut node.ports {
            if port.labels.len() < 2 {
                continue;
            }
            port.labels.sort_by(|a, b| {
                let a_key = a
                    .end_label_edge
                    .and_then(|edge| edge_sort_keys.get(edge).copied())
                    .unwrap_or((usize::MAX, usize::MAX, usize::MAX));
                let b_key = b
                    .end_label_edge
                    .and_then(|edge| edge_sort_keys.get(edge).copied())
                    .unwrap_or((usize::MAX, usize::MAX, usize::MAX));
                a_key.cmp(&b_key)
            });
        }
    }
}

fn end_label_sort_key(graph: &LGraph, edge: usize) -> (usize, usize, usize) {
    let edge = &graph.edges[edge];
    let source_port = edge.source.port;
    let target_node = edge.target.node;
    let target_port = usize::MAX.saturating_sub(edge.target.port);
    (source_port, target_node, target_port)
}

pub fn postprocess_end_labels(graph: &mut LGraph) {
    for node in 0..graph.layerless_nodes.len() {
        if !matches!(
            graph.layerless_nodes[node].kind,
            LNodeKind::Normal | LNodeKind::ExternalPort
        ) {
            continue;
        }
        let node_position = graph.layerless_nodes[node].position;
        let port_count = graph.layerless_nodes[node].ports.len();
        for port_index in 0..port_count {
            let Some(cell) = graph.layerless_nodes[node].ports[port_index].end_label_cell else {
                continue;
            };
            apply_end_label_cell_layout(graph, node, port_index, node_position, cell);
            graph.layerless_nodes[node].ports[port_index].end_label_cell = None;
        }
    }
}

fn apply_end_label_cell_layout(
    graph: &mut LGraph,
    node: usize,
    port_index: usize,
    node_position: LPoint,
    cell: LabelCellLayout,
) {
    let label_label_spacing = graph.options.spacing.label_label;
    let mut labels = std::mem::take(&mut graph.layerless_nodes[node].ports[port_index].labels);
    let mut cursor = cell.position;
    cursor.x += node_position.x;
    cursor.y += node_position.y;

    for label in &mut labels {
        if cell.horizontal_layout {
            label.position.x = cursor.x;
            label.position.y = aligned_y(cursor.y, cell.size.height, label.size.height, cell);
            cursor.x += label.size.width + label_label_spacing;
        } else {
            label.position.x = aligned_x(cursor.x, cell.size.width, label.size.width, cell);
            label.position.y = cursor.y;
            cursor.y += label.size.height + label_label_spacing;
        }
    }

    for label in labels {
        let target_edge = label.end_label_edge.unwrap_or_else(|| {
            let port = &graph.layerless_nodes[node].ports[port_index];
            port.incoming_edges
                .first()
                .or_else(|| port.outgoing_edges.first())
                .copied()
                .unwrap_or(usize::MAX)
        });
        if let Some(edge) = graph.edges.get_mut(target_edge) {
            edge.labels.push(label);
        }
    }
}

fn aligned_x(x: f64, cell_width: f64, label_width: f64, cell: LabelCellLayout) -> f64 {
    match cell.horizontal_alignment {
        HorizontalLabelAlignment::Left => x,
        HorizontalLabelAlignment::Center => x + (cell_width - label_width) / 2.0,
        HorizontalLabelAlignment::Right => x + cell_width - label_width,
    }
}

fn aligned_y(y: f64, cell_height: f64, label_height: f64, cell: LabelCellLayout) -> f64 {
    match cell.vertical_alignment {
        VerticalLabelAlignment::Top => y,
        VerticalLabelAlignment::Center => y + (cell_height - label_height) / 2.0,
        VerticalLabelAlignment::Bottom => y + cell_height - label_height,
    }
}

pub fn remove_label_dummies(graph: &mut LGraph) {
    for layer_index in 0..graph.layers.len() {
        let mut node_position = 0usize;

        while node_position < graph.layers[layer_index].nodes.len() {
            let node = graph.layers[layer_index].nodes[node_position];
            if graph.layerless_nodes[node].kind == LNodeKind::Label {
                place_label_dummy_labels(graph, node);
                let add_unnecessary_bendpoints =
                    graph.options.edge_routing == crate::options::EdgeRouting::Polyline;
                join_long_edge_at(graph, node, add_unnecessary_bendpoints);
                graph.layers[layer_index].nodes.remove(node_position);
            } else {
                node_position += 1;
            }
        }
    }
}

#[derive(Debug, Clone)]
struct LabelDummyInfo {
    label_dummy: usize,
    left_long_edge_dummies: Vec<usize>,
    right_long_edge_dummies: Vec<usize>,
    leftmost_layer_id: usize,
    rightmost_layer_id: usize,
}

impl LabelDummyInfo {
    fn new(graph: &LGraph, label_dummy: usize) -> Option<Self> {
        let left_long_edge_dummies = gather_left_long_edge_dummies(graph, label_dummy)?;
        let right_long_edge_dummies = gather_right_long_edge_dummies(graph, label_dummy)?;
        let leftmost_layer_id = left_long_edge_dummies
            .first()
            .copied()
            .unwrap_or(label_dummy);
        let leftmost_layer_id = graph.layerless_nodes[leftmost_layer_id].layer_index?;
        let rightmost_layer_id = right_long_edge_dummies
            .last()
            .copied()
            .unwrap_or(label_dummy);
        let rightmost_layer_id = graph.layerless_nodes[rightmost_layer_id].layer_index?;

        Some(Self {
            label_dummy,
            left_long_edge_dummies,
            right_long_edge_dummies,
            leftmost_layer_id,
            rightmost_layer_id,
        })
    }

    fn dummy_at_offset(&self, offset: usize) -> Option<usize> {
        if offset < self.left_long_edge_dummies.len() {
            self.left_long_edge_dummies.get(offset).copied()
        } else if offset == self.left_long_edge_dummies.len() {
            Some(self.label_dummy)
        } else {
            self.right_long_edge_dummies
                .get(offset - self.left_long_edge_dummies.len() - 1)
                .copied()
        }
    }
}

fn gather_left_long_edge_dummies(graph: &LGraph, label_dummy: usize) -> Option<Vec<usize>> {
    let mut out = Vec::new();
    let mut current = label_dummy;

    loop {
        let incoming = graph.node_incoming_edges(current).into_iter().next()?;
        let source = graph.edges[incoming].source.node;
        if graph.layerless_nodes[source].kind == LNodeKind::LongEdge {
            out.push(source);
            current = source;
        } else {
            break;
        }
    }

    out.reverse();
    Some(out)
}

fn gather_right_long_edge_dummies(graph: &LGraph, label_dummy: usize) -> Option<Vec<usize>> {
    let mut out = Vec::new();
    let mut current = label_dummy;

    loop {
        let outgoing = graph.node_outgoing_edges(current).into_iter().next()?;
        let target = graph.edges[outgoing].target.node;
        if graph.layerless_nodes[target].kind == LNodeKind::LongEdge {
            out.push(target);
            current = target;
        } else {
            break;
        }
    }

    Some(out)
}

fn find_median_layer_target_id(info: &LabelDummyInfo) -> usize {
    // Layered.melk defaults center-edge labels to MEDIAN_LAYER. Java's LabelDummySwitcher uses
    // the lower median when an edge spans an even number of eligible dummy layers.
    let total = info.rightmost_layer_id - info.leftmost_layer_id + 1;
    info.leftmost_layer_id + (total - 1) / 2
}

fn assign_label_dummy_layer(graph: &mut LGraph, info: &LabelDummyInfo, target_layer: usize) {
    let current_layer = graph.layerless_nodes[info.label_dummy]
        .layer_index
        .unwrap_or(target_layer);
    if current_layer == target_layer {
        return;
    }

    let Some(target_dummy) = info.dummy_at_offset(target_layer - info.leftmost_layer_id) else {
        return;
    };
    if graph.layerless_nodes[target_dummy].kind != LNodeKind::LongEdge {
        return;
    }
    swap_label_and_long_edge_dummies(graph, info.label_dummy, target_dummy);
}

fn swap_label_and_long_edge_dummies(
    graph: &mut LGraph,
    label_dummy: usize,
    long_edge_dummy: usize,
) {
    let Some(label_layer) = graph.layerless_nodes[label_dummy].layer_index else {
        return;
    };
    let Some(long_edge_layer) = graph.layerless_nodes[long_edge_dummy].layer_index else {
        return;
    };
    let Some(label_position) = graph.layers[label_layer]
        .nodes
        .iter()
        .position(|node| *node == label_dummy)
    else {
        return;
    };
    let Some(long_edge_position) = graph.layers[long_edge_layer]
        .nodes
        .iter()
        .position(|node| *node == long_edge_dummy)
    else {
        return;
    };

    let Some(label_input) = first_port_on_side(graph, label_dummy, PortSide::West) else {
        return;
    };
    let Some(label_output) = first_port_on_side(graph, label_dummy, PortSide::East) else {
        return;
    };
    let Some(long_input) = first_port_on_side(graph, long_edge_dummy, PortSide::West) else {
        return;
    };
    let Some(long_output) = first_port_on_side(graph, long_edge_dummy, PortSide::East) else {
        return;
    };

    let label_input_ref = PortRef {
        node: label_dummy,
        port: label_input,
    };
    let label_output_ref = PortRef {
        node: label_dummy,
        port: label_output,
    };
    let long_input_ref = PortRef {
        node: long_edge_dummy,
        port: long_input,
    };
    let long_output_ref = PortRef {
        node: long_edge_dummy,
        port: long_output,
    };
    let label_incoming = graph.layerless_nodes[label_dummy].ports[label_input]
        .incoming_edges
        .clone();
    let label_outgoing = graph.layerless_nodes[label_dummy].ports[label_output]
        .outgoing_edges
        .clone();
    let long_incoming = graph.layerless_nodes[long_edge_dummy].ports[long_input]
        .incoming_edges
        .clone();
    let long_outgoing = graph.layerless_nodes[long_edge_dummy].ports[long_output]
        .outgoing_edges
        .clone();

    graph.layers[long_edge_layer].nodes[long_edge_position] = label_dummy;
    graph.layerless_nodes[label_dummy].layer_index = Some(long_edge_layer);
    for edge in long_incoming {
        graph.set_edge_target(edge, label_input_ref);
    }
    for edge in long_outgoing {
        graph.set_edge_source(edge, label_output_ref);
    }

    graph.layers[label_layer].nodes[label_position] = long_edge_dummy;
    graph.layerless_nodes[long_edge_dummy].layer_index = Some(label_layer);
    for edge in label_incoming {
        graph.set_edge_target(edge, long_input_ref);
    }
    for edge in label_outgoing {
        graph.set_edge_source(edge, long_output_ref);
    }
}

fn update_long_edge_before_label_dummy_info(graph: &mut LGraph, label_dummy: usize) {
    let mut current = label_dummy;
    while let Some(incoming) = graph.node_incoming_edges(current).into_iter().next() {
        let source = graph.edges[incoming].source.node;
        if graph.layerless_nodes[source].kind != LNodeKind::LongEdge {
            break;
        }
        graph.layerless_nodes[source].long_edge_has_label_dummies = true;
        current = source;
    }
}

fn apply_label_side(graph: &mut LGraph, label_dummy: usize, side: LabelSide) {
    if graph.layerless_nodes[label_dummy].kind != LNodeKind::Label {
        return;
    }

    let effective_side = if graph.layerless_nodes[label_dummy]
        .represented_labels
        .iter()
        .all(|label| label.inline)
    {
        LabelSide::Inline
    } else {
        side
    };

    graph.layerless_nodes[label_dummy].label_side = effective_side;
    for label in &mut graph.layerless_nodes[label_dummy].represented_labels {
        label.label_side = Some(effective_side);
    }

    if effective_side == LabelSide::Below {
        return;
    }

    let origin_edge = graph.layerless_nodes[label_dummy]
        .origin_edge
        .and_then(|edge| graph.edges.get(edge))
        .map(|edge| edge.thickness.max(0.0))
        .unwrap_or(0.0);
    let port_y = match effective_side {
        LabelSide::Above => {
            graph.layerless_nodes[label_dummy].size.height - (origin_edge / 2.0).ceil()
        }
        LabelSide::Inline => {
            let edge_label_spacing = graph.options.spacing.edge_label;
            let port_y =
                (graph.layerless_nodes[label_dummy].size.height - edge_label_spacing - origin_edge)
                    .ceil()
                    / 2.0;
            graph.layerless_nodes[label_dummy].size.height -= edge_label_spacing;
            graph.layerless_nodes[label_dummy].size.height -= origin_edge;
            port_y
        }
        LabelSide::Below => 0.0,
    };

    for port in &mut graph.layerless_nodes[label_dummy].ports {
        port.position.y = port_y;
    }
}

fn does_label_dummy_point_right(graph: &LGraph, label_dummy: usize) -> bool {
    let incoming = graph.node_incoming_edges(label_dummy).into_iter().next();
    let outgoing = graph.node_outgoing_edges(label_dummy).into_iter().next();

    incoming
        .map(|edge| !graph.edges[edge].reversed)
        .unwrap_or(false)
        || outgoing
            .map(|edge| !graph.edges[edge].reversed)
            .unwrap_or(false)
}

fn place_label_dummy_labels(graph: &mut LGraph, label_dummy: usize) {
    let Some(origin_edge) = graph.layerless_nodes[label_dummy].origin_edge else {
        return;
    };
    let thickness = graph.edges[origin_edge].thickness.max(0.0);
    let edge_label_spacing = graph.options.spacing.edge_label;
    let label_label_spacing = graph.options.spacing.label_label;
    let labels_below_edge = graph.layerless_nodes[label_dummy].label_side == LabelSide::Below;
    let inline = graph.layerless_nodes[label_dummy]
        .represented_labels
        .iter()
        .all(|label| label.inline);
    let mut label_position = graph.layerless_nodes[label_dummy].position;

    if labels_below_edge {
        label_position.y += thickness + edge_label_spacing;
    }

    let label_space = LPoint {
        x: graph.layerless_nodes[label_dummy].size.width,
        y: graph.layerless_nodes[label_dummy].size.height
            + if inline {
                0.0
            } else {
                -thickness - edge_label_spacing
            },
    };
    let mut labels = std::mem::take(&mut graph.layerless_nodes[label_dummy].represented_labels);

    if graph.options.direction.is_vertical() {
        place_labels_for_vertical_layout(
            &mut labels,
            label_position,
            label_label_spacing,
            label_space,
            labels_below_edge,
            graph.options.direction,
        );
    } else {
        place_labels_for_horizontal_layout(
            &mut labels,
            label_position,
            label_label_spacing,
            label_space,
        );
    }

    graph.edges[origin_edge].labels.extend(labels);
}

fn place_labels_for_horizontal_layout(
    labels: &mut [LLabel],
    mut label_position: LPoint,
    label_spacing: f64,
    label_space: LPoint,
) {
    for label in labels {
        label.position.x = label_position.x + (label_space.x - label.size.width) / 2.0;
        label.position.y = label_position.y;
        label_position.y += label.size.height + label_spacing;
    }
}

fn place_labels_for_vertical_layout(
    labels: &mut [LLabel],
    mut label_position: LPoint,
    label_spacing: f64,
    label_space: LPoint,
    left_aligned: bool,
    direction: ElkDirection,
) {
    let inline = labels.iter().all(|label| label.inline);

    if direction == ElkDirection::Up {
        labels.reverse();
    }

    for label in &mut *labels {
        label.position.x = label_position.x;
        if inline {
            label.position.y = label_position.y + (label_space.y - label.size.height) / 2.0;
        } else if left_aligned {
            label.position.y = label_position.y;
        } else {
            label.position.y = label_position.y + label_space.y - label.size.height;
        }
        label_position.x += label.size.width + label_spacing;
    }

    if direction == ElkDirection::Up {
        labels.reverse();
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{edge, graph, node};
    use super::*;
    use crate::importer::ElkInputLabel;

    #[test]
    fn horizontal_inline_label_dummy_retains_split_edge_thickness() {
        let mut center = ElkInputLabel::center("choice", 30.0, 12.0);
        center.placement = EdgeLabelPlacement::Center;
        let mut labelled = edge("A-B", "A", "B");
        labelled.labels = vec![center];
        let mut graph = graph(vec![node("A"), node("B")], vec![labelled]);
        graph.options.direction = ElkDirection::Right;

        insert_label_dummies(&mut graph);

        let dummy = graph
            .layerless_nodes
            .iter()
            .position(|node| node.kind == LNodeKind::Label)
            .unwrap();
        assert_eq!(graph.layerless_nodes[dummy].size.height, 18.0);

        apply_label_side(&mut graph, dummy, LabelSide::Below);

        assert_eq!(graph.layerless_nodes[dummy].size.height, 13.0);
        assert!(
            graph.layerless_nodes[dummy]
                .ports
                .iter()
                .all(|port| port.position.y == 6.5)
        );
    }

    #[test]
    fn default_median_label_dummy_strategy_uses_lower_median_layer() {
        let info = |leftmost_layer_id, rightmost_layer_id| LabelDummyInfo {
            label_dummy: 0,
            left_long_edge_dummies: Vec::new(),
            right_long_edge_dummies: Vec::new(),
            leftmost_layer_id,
            rightmost_layer_id,
        };

        assert_eq!(find_median_layer_target_id(&info(3, 4)), 3);
        assert_eq!(find_median_layer_target_id(&info(3, 5)), 4);
    }

    #[test]
    fn end_label_gather_preserves_large_mixed_label_order_and_identity() {
        let mut input = edge("A-B", "A", "B");
        input.labels = (0..4096)
            .map(|index| {
                let mut label = ElkInputLabel::center(index.to_string(), 12.0, 10.0);
                label.placement = if index % 3 == 0 {
                    EdgeLabelPlacement::Center
                } else {
                    EdgeLabelPlacement::Head
                };
                label.source_index = Some(index);
                label
            })
            .collect();
        let mut graph = graph(vec![node("A"), node("B")], vec![input]);
        let target = graph.edges[0].target;
        gather_end_labels_for_node(&mut graph, target.node);
        let port = &graph.layerless_nodes[target.node].ports[target.port];
        assert_eq!(
            port.labels
                .iter()
                .map(|label| label.source_index.unwrap())
                .collect::<Vec<_>>(),
            (0..4096).filter(|index| index % 3 != 0).collect::<Vec<_>>()
        );
        assert!(
            port.labels
                .iter()
                .all(|label| label.end_label_edge == Some(0))
        );
        assert_eq!(
            graph.edges[0]
                .labels
                .iter()
                .map(|label| label.source_index.unwrap())
                .collect::<Vec<_>>(),
            (0..4096).filter(|index| index % 3 == 0).collect::<Vec<_>>()
        );
    }
}
