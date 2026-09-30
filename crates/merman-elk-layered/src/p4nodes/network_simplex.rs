//! Network-simplex node placement.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p4nodes/NetworkSimplexPlacer.java

use super::vertical_spacing;
use crate::common::networksimplex::{NGraph, NetworkSimplex, NetworkSimplexError, coordinate};
use crate::graph::{LGraph, LNodeKind, PortRef, PortSide};
use crate::options::{NodeFlexibility, NodeLabelPlacement};

#[derive(Clone, Copy)]
struct NodeRep {
    head: usize,
    tail: usize,
    flexible: bool,
}

#[derive(Clone, Copy)]
struct EdgeRep {
    left: usize,
    right: usize,
}

struct Auxiliary {
    graph: NGraph,
    nodes: Vec<Option<NodeRep>>,
    ports: Vec<Vec<Option<usize>>>,
    edges: Vec<Option<EdgeRep>>,
}

pub fn place_nodes_network_simplex(graph: &mut LGraph) -> Result<(), NetworkSimplexError> {
    let order: Vec<_> = graph
        .layers
        .iter()
        .flat_map(|layer| layer.nodes.iter().copied())
        .filter(|node| !graph.layerless_nodes[*node].hidden)
        .collect();
    if order.is_empty() {
        return Ok(());
    }
    // ELK rounds anchors independently for flexible nodes, and combined port/anchor
    // positions for every node, before constructing integral simplex constraints.
    let flexible: Vec<_> = (0..graph.layerless_nodes.len())
        .map(|node| is_flexible(graph, node))
        .collect();
    for &node in &order {
        for port in &mut graph.layerless_nodes[node].ports {
            if flexible[node] {
                port.anchor.y = java_round(port.anchor.y)?;
            }
            port.position.y = java_round(port.position.y + port.anchor.y)? - port.anchor.y;
        }
    }
    let mut aux = Auxiliary {
        graph: NGraph::new(),
        nodes: vec![None; graph.layerless_nodes.len()],
        ports: graph
            .layerless_nodes
            .iter()
            .map(|node| vec![None; node.ports.len()])
            .collect(),
        edges: vec![None; graph.edges.len()],
    };
    for layer in &graph.layers {
        let mut previous: Option<usize> = None;
        for &node in &layer.nodes {
            if graph.layerless_nodes[node].hidden {
                continue;
            }
            let head = aux.graph.add_node(Some(node));
            let rep = if flexible[node] {
                let tail = aux.graph.add_node(Some(node));
                aux.graph.add_edge(
                    None,
                    head,
                    tail,
                    10_000.0,
                    ceil_i32(graph.layerless_nodes[node].size.height)?,
                );
                let rep = NodeRep {
                    head,
                    tail,
                    flexible: true,
                };
                transform_ports(graph, &mut aux, node, rep, PortSide::West)?;
                transform_ports(graph, &mut aux, node, rep, PortSide::East)?;
                rep
            } else {
                for (port, data) in graph.layerless_nodes[node].ports.iter().enumerate() {
                    if matches!(data.side, PortSide::East | PortSide::West) {
                        aux.ports[node][port] = Some(head);
                    }
                }
                NodeRep {
                    head,
                    tail: head,
                    flexible: false,
                }
            };
            aux.nodes[node] = Some(rep);
            if let Some(previous) = previous {
                let previous_rep = aux.nodes[previous].unwrap();
                let previous_node = &graph.layerless_nodes[previous];
                let spacing = previous_node.margin.bottom
                    + vertical_spacing(graph, previous, node)
                    + graph.layerless_nodes[node].margin.top
                    + if previous_rep.flexible {
                        0.0
                    } else {
                        previous_node.size.height
                    };
                aux.graph
                    .add_edge(None, previous_rep.tail, head, 0.0, ceil_i32(spacing)?);
            }
            previous = Some(node);
        }
    }
    // Use layer/node/port incidence order, matching LNode.getOutgoingEdges().
    for &node in &order {
        for edge in graph.node_outgoing_edges(node) {
            if !handled_edge(graph, edge) {
                continue;
            }
            let data = &graph.edges[edge];
            let (Some(source), Some(target)) = (
                aux.ports[data.source.node][data.source.port],
                aux.ports[data.target.node][data.target.port],
            ) else {
                continue;
            };
            let offset = |endpoint: PortRef| {
                let port = &graph.layerless_nodes[endpoint.node].ports[endpoint.port];
                port.anchor.y
                    + if aux.nodes[endpoint.node].unwrap().flexible {
                        0.0
                    } else {
                        port.position.y
                    }
            };
            let source_offset = offset(data.source);
            let target_offset = offset(data.target);
            let dummy = aux.graph.add_node(None);
            let weight = data.priority_straightness.max(1) as f64
                * edge_type_weight(
                    graph.layerless_nodes[data.source.node].kind,
                    graph.layerless_nodes[data.target.node].kind,
                );
            let left = aux
                .graph
                .add_edge(
                    Some(edge),
                    dummy,
                    source,
                    weight,
                    ceil_i32((target_offset - source_offset).max(0.0))?,
                )
                .unwrap();
            let right = aux
                .graph
                .add_edge(
                    Some(edge),
                    dummy,
                    target,
                    weight,
                    ceil_i32((source_offset - target_offset).max(0.0))?,
                )
                .unwrap();
            aux.edges[edge] = Some(EdgeRep { left, right });
        }
    }
    insert_port_auxiliary_edges(graph, &mut aux, &order);
    let two_paths = if graph
        .options
        .node_placement_favor_straight_edges
        .unwrap_or(true)
    {
        prefer_straight_edges(graph, &mut aux, &order)
    } else {
        Vec::new()
    };
    make_connected(&mut aux.graph);
    let iteration_limit = graph
        .options
        .thoroughness
        .saturating_mul(aux.graph.nodes.len());
    NetworkSimplex::for_graph(&mut aux.graph)
        .with_iteration_limit(iteration_limit)
        .with_balancing(false)
        .execute()?;
    let mut retry = Vec::new();
    for path in two_paths {
        if improve_two_path(graph, &mut aux, path, true)? {
            retry.push(path);
        }
    }
    for path in retry.into_iter().rev() {
        improve_two_path(graph, &mut aux, path, false)?;
    }
    for node in order {
        let rep = aux.nodes[node].unwrap();
        let y = aux.graph.nodes[rep.head].layer as f64;
        let data = &mut graph.layerless_nodes[node];
        data.position.y = y;
        if rep.flexible {
            let size_delta = (aux.graph.nodes[rep.tail].layer as f64 - y) - data.size.height;
            for (port, position) in data.ports.iter_mut().zip(&aux.ports[node]) {
                if let Some(position) = position {
                    port.position.y = aux.graph.nodes[*position].layer as f64 - y;
                }
            }
            let label_shift = match data.node_label_placement {
                NodeLabelPlacement::InsideBottomLeft
                | NodeLabelPlacement::InsideBottomCenter
                | NodeLabelPlacement::InsideBottomRight
                | NodeLabelPlacement::OutsideBottomLeft
                | NodeLabelPlacement::OutsideBottomCenter
                | NodeLabelPlacement::OutsideBottomRight
                | NodeLabelPlacement::OutsideLeftBottom
                | NodeLabelPlacement::OutsideRightBottom => size_delta,
                NodeLabelPlacement::InsideCenterLeft
                | NodeLabelPlacement::InsideCenter
                | NodeLabelPlacement::InsideCenterRight
                | NodeLabelPlacement::OutsideLeftCenter
                | NodeLabelPlacement::OutsideRightCenter => size_delta / 2.0,
                _ => 0.0,
            };
            for label in &mut data.labels {
                label.position.y += label_shift;
            }
        }
    }
    Ok(())
}

fn is_flexible(graph: &LGraph, node: usize) -> bool {
    let node = &graph.layerless_nodes[node];
    if node.kind != LNodeKind::Normal
        || node.ports.len() <= 1
        || node.port_constraints.is_pos_fixed()
        || node.node_flexibility == NodeFlexibility::None
    {
        return false;
    }
    let spacing = graph.options.spacing.port_port;
    // Eligibility reads the node property, unlike transformPorts' inherited spacing.
    let margins = node
        .ports_surrounding
        .map(|m| m.top + m.bottom)
        .unwrap_or(2.0 * spacing);
    [PortSide::West, PortSide::East].into_iter().all(|side| {
        let count = node.ports.iter().filter(|port| port.side == side).count();
        margins + (count as f64 - 1.0) * spacing <= node.size.height
    })
}

fn transform_ports(
    graph: &LGraph,
    aux: &mut Auxiliary,
    node: usize,
    rep: NodeRep,
    side: PortSide,
) -> Result<(), NetworkSimplexError> {
    let data = &graph.layerless_nodes[node];
    let mut ports: Vec<_> = data
        .ports
        .iter()
        .enumerate()
        .filter_map(|(index, port)| (port.side == side).then_some(index))
        .collect();
    if side == PortSide::West {
        ports.reverse();
    }
    let surrounding = graph.options.spacing.ports_surrounding;
    let mut previous = rep.head;
    let mut last_height = None;
    for port in ports {
        let position = aux.graph.add_node(None);
        aux.ports[node][port] = Some(position);
        let spacing = last_height
            .map(|height| height + graph.options.spacing.port_port)
            .unwrap_or(surrounding.top);
        aux.graph
            .add_edge(None, previous, position, 0.0, ceil_i32(spacing)?);
        last_height = Some(data.ports[port].size.height);
        previous = position;
    }
    if let Some(height) = last_height {
        aux.graph.add_edge(
            None,
            previous,
            rep.tail,
            0.0,
            ceil_i32(surrounding.bottom + height)?,
        );
    }
    Ok(())
}

fn handled_edge(graph: &LGraph, edge: usize) -> bool {
    let data = &graph.edges[edge];
    data.source.node != data.target.node
        && graph.layerless_nodes[data.source.node].layer_index
            != graph.layerless_nodes[data.target.node].layer_index
}

fn insert_port_auxiliary_edges(graph: &LGraph, aux: &mut Auxiliary, order: &[usize]) {
    for &node in order {
        let rep = aux.nodes[node].unwrap();
        for port in &graph.layerless_nodes[node].ports {
            let Some(dummy) = &port.port_dummy else {
                continue;
            };
            if dummy.graph_id != graph.id {
                continue;
            }
            let Some(other) = aux.nodes[dummy.node] else {
                continue;
            };
            match port.side {
                PortSide::South => {
                    aux.graph.add_edge(None, rep.tail, other.head, 0.1, 0);
                }
                PortSide::North => {
                    aux.graph.add_edge(None, other.tail, rep.head, 0.1, 0);
                }
                _ => {}
            }
        }
    }
    let mut position = vec![0; graph.layerless_nodes.len()];
    for layer in &graph.layers {
        for (index, &node) in layer.nodes.iter().enumerate() {
            position[node] = index;
        }
    }
    for &node in order {
        if graph.layerless_nodes[node].kind != LNodeKind::Normal {
            continue;
        }
        for edge in graph.node_connected_edges(node) {
            let edge = &graph.edges[edge];
            if edge.source.node == edge.target.node
                || graph.layerless_nodes[edge.source.node].layer_index
                    != graph.layerless_nodes[edge.target.node].layer_index
            {
                continue;
            }
            let (port, dummy) = if graph.layerless_nodes[edge.source.node].kind != LNodeKind::Normal
            {
                (edge.target, edge.source.node)
            } else {
                (edge.source, edge.target.node)
            };
            let (Some(port_rep), Some(dummy_rep)) =
                (aux.ports[port.node][port.port], aux.nodes[dummy])
            else {
                continue;
            };
            let (source, target) = if position[port.node] < position[dummy] {
                (port_rep, dummy_rep.head)
            } else {
                (dummy_rep.head, port_rep)
            };
            aux.graph.add_edge(None, source, target, 4.0, 0);
        }
    }
}

fn make_connected(graph: &mut NGraph) {
    let mut visited = vec![false; graph.nodes.len()];
    let mut representatives = Vec::new();
    for node in 0..graph.nodes.len() {
        if visited[node] {
            continue;
        }
        representatives.push(node);
        let mut stack = vec![node];
        visited[node] = true;
        while let Some(node) = stack.pop() {
            for edge in graph.connected_edges(node) {
                let edge = &graph.edges[edge];
                let other = if edge.source == node {
                    edge.target
                } else {
                    edge.source
                };
                if !visited[other] {
                    visited[other] = true;
                    stack.push(other);
                }
            }
        }
    }
    if representatives.len() > 1 {
        let root = graph.add_node(None);
        for node in representatives {
            graph.add_edge(None, root, node, 0.0, 0);
        }
    }
}

fn prefer_straight_edges(graph: &LGraph, aux: &mut Auxiliary, order: &[usize]) -> Vec<[usize; 2]> {
    let crossing = mark_crossings(graph);
    // -1 visited, 0 path interior, 2 junction (including leaves).
    let mut state = vec![0i8; graph.layerless_nodes.len()];
    for &node in order {
        let incoming = graph
            .node_incoming_edges(node)
            .into_iter()
            .filter(|edge| graph.edges[*edge].source.node != graph.edges[*edge].target.node)
            .count();
        let outgoing = graph
            .node_outgoing_edges(node)
            .into_iter()
            .filter(|edge| graph.edges[*edge].source.node != graph.edges[*edge].target.node)
            .count();
        if incoming > 1 || outgoing > 1 || incoming + outgoing == 1 {
            state[node] = 2;
        }
    }
    let mut paths = Vec::new();
    for &junction in order {
        if state[junction] != 2 {
            continue;
        }
        for first in graph.node_connected_edges(junction) {
            if !handled_edge(graph, first) {
                continue;
            }
            let mut path = Vec::new();
            let mut current = junction;
            let mut edge = first;
            loop {
                path.push(edge);
                let data = &graph.edges[edge];
                let other = if data.source.node == current {
                    data.target.node
                } else {
                    data.source.node
                };
                if state[other] == -1 || state[other] == 2 || crossing[edge] {
                    break;
                }
                state[other] = -1;
                let Some(next) = graph
                    .node_connected_edges(other)
                    .into_iter()
                    .find(|next| *next != edge && handled_edge(graph, *next))
                else {
                    break;
                };
                current = other;
                edge = next;
            }
            if path.len() > 1 {
                paths.push(path);
            }
        }
    }
    let mut two_paths = Vec::new();
    for mut path in paths {
        if path.iter().any(|edge| aux.edges[*edge].is_none()) {
            continue;
        }
        if path.len() == 2 {
            if graph.edges[path[0]].target.node != graph.edges[path[1]].source.node {
                path.swap(0, 1);
            }
            if !aux.nodes[graph.edges[path[0]].target.node]
                .unwrap()
                .flexible
            {
                two_paths.push([path[0], path[1]]);
            }
        } else {
            let long_dummy = std::iter::once(graph.edges[path[0]].source.node)
                .chain(path.iter().map(|edge| graph.edges[*edge].target.node))
                .any(|node| graph.layerless_nodes[node].kind == LNodeKind::LongEdge);
            if long_dummy {
                continue;
            }
            for (index, edge) in path.iter().copied().enumerate() {
                let weight: f64 = if index == 0 || index + 1 == path.len() {
                    16.0
                } else {
                    64.0
                };
                let rep = aux.edges[edge].unwrap();
                for edge in [rep.left, rep.right] {
                    aux.graph.edges[edge].weight = aux.graph.edges[edge].weight.max(weight);
                }
            }
        }
    }
    two_paths
}

fn mark_crossings(graph: &LGraph) -> Vec<bool> {
    let mut crossing = vec![false; graph.edges.len()];
    for (layer_index, layers) in graph.layers.windows(2).enumerate() {
        let mut open = Vec::new();
        for &node in &layers[0].nodes {
            for port in graph.layerless_nodes[node]
                .ports
                .iter()
                .filter(|port| port.side == PortSide::East)
            {
                open.extend(port.outgoing_edges.iter().copied().filter(|edge| {
                    handled_edge(graph, *edge)
                        && graph.layerless_nodes[graph.edges[*edge].target.node].layer_index
                            == Some(layer_index + 1)
                }));
            }
        }
        for &node in layers[1].nodes.iter().rev() {
            for port in graph.layerless_nodes[node]
                .ports
                .iter()
                .filter(|port| port.side == PortSide::West)
            {
                for &edge in &port.incoming_edges {
                    if !handled_edge(graph, edge)
                        || graph.layerless_nodes[graph.edges[edge].source.node].layer_index
                            != Some(layer_index)
                        || open.is_empty()
                    {
                        continue;
                    }
                    let mut index = open.len() - 1;
                    while open[index] != edge && index > 0 {
                        crossing[open[index]] = true;
                        crossing[edge] = true;
                        index -= 1;
                    }
                    // Preserve the source ListIterator's first-entry retention.
                    if index > 0 {
                        open.remove(index);
                    }
                }
            }
        }
    }
    crossing
}

fn improve_two_path(
    graph: &LGraph,
    aux: &mut Auxiliary,
    path: [usize; 2],
    probe: bool,
) -> Result<bool, NetworkSimplexError> {
    let left = aux.edges[path[0]].unwrap();
    let right = aux.edges[path[1]].unwrap();
    let not_straight = |rep: EdgeRep| {
        let l = &aux.graph.edges[rep.left];
        let r = &aux.graph.edges[rep.right];
        (i64::from(aux.graph.nodes[l.target].layer) - i64::from(l.delta))
            - (i64::from(aux.graph.nodes[r.target].layer) - i64::from(r.delta))
    };
    let left_bend = not_straight(left);
    let right_bend = not_straight(right);
    if left_bend == 0 && right_bend == 0 {
        return Ok(false);
    }
    let center = graph.edges[path[0]].target.node;
    let rep = aux.nodes[center].unwrap();
    if rep.flexible {
        return Ok(false);
    }
    let data = &graph.layerless_nodes[center];
    let layer = &graph.layers[data.layer_index.unwrap()].nodes;
    let index = layer.iter().position(|node| *node == center).unwrap();
    let y = aux.graph.nodes[rep.head].layer as f64;
    let mut above = f64::INFINITY;
    let mut below = f64::INFINITY;
    if index > 0 {
        let other = layer[index - 1];
        let other_data = &graph.layerless_nodes[other];
        above = y
            - data.margin.top
            - (aux.graph.nodes[aux.nodes[other].unwrap().head].layer as f64
                + other_data.size.height
                + other_data.margin.bottom)
            - vertical_spacing(graph, other, center).ceil();
    }
    if index + 1 < layer.len() {
        let other = layer[index + 1];
        let other_data = &graph.layerless_nodes[other];
        below = aux.graph.nodes[aux.nodes[other].unwrap().head].layer as f64
            - other_data.margin.top
            - (y + data.size.height + data.margin.bottom)
            - vertical_spacing(graph, other, center).ceil();
    }
    if probe && (above == below || (above - below).abs() <= 0.00001) {
        return Ok(true);
    }
    let length = |edge: usize| {
        let edge = &aux.graph.edges[edge];
        (i64::from(aux.graph.nodes[edge.source].layer)
            - i64::from(aux.graph.nodes[edge.target].layer))
        .abs()
            - i64::from(edge.delta)
    };
    let a = length(left.left);
    let b = -length(left.right);
    let c = -length(right.left);
    let d = length(right.right);
    let case_d = left_bend > 0 && right_bend < 0;
    let case_c = left_bend < 0 && right_bend > 0;
    let source_height = i64::from(aux.graph.nodes[aux.graph.edges[left.left].target].layer)
        + i64::from(aux.graph.edges[left.right].delta);
    let target_height = i64::from(aux.graph.nodes[aux.graph.edges[right.right].target].layer)
        + i64::from(aux.graph.edges[right.left].delta);
    let mut movement = 0;
    if !case_c && !case_d {
        if source_height > target_height {
            if above + c as f64 > 0.0 {
                movement = c;
            } else if below - a as f64 > 0.0 {
                movement = a;
            }
        } else if source_height < target_height {
            if above + b as f64 > 0.0 {
                movement = b;
            } else if below - d as f64 > 0.0 {
                movement = d;
            }
        }
    }
    aux.graph.nodes[rep.head].layer =
        coordinate(i64::from(aux.graph.nodes[rep.head].layer) + movement)?;
    Ok(false)
}

fn edge_type_weight(source: LNodeKind, target: LNodeKind) -> f64 {
    match (source, target) {
        (LNodeKind::Normal, LNodeKind::Normal) => 4.0,
        (LNodeKind::Normal, _) | (_, LNodeKind::Normal) => 8.0,
        _ => 32.0,
    }
}

fn java_round(value: f64) -> Result<f64, NetworkSimplexError> {
    let rounded = (value + 0.5).floor();
    checked_integer(rounded).map(f64::from)
}

fn ceil_i32(value: f64) -> Result<i32, NetworkSimplexError> {
    checked_integer(value.ceil())
}

fn checked_integer(value: f64) -> Result<i32, NetworkSimplexError> {
    if !value.is_finite() || value < i32::MIN as f64 || value > i32::MAX as f64 {
        return Err(NetworkSimplexError::CoordinateOutOfRange);
    }
    Ok(value as i32)
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
            label: None,
            minlen: 1,
            inside_self_loops_yo: false,
            model_order: None,
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

        place_nodes_network_simplex(&mut graph).unwrap();

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

        place_nodes_network_simplex(&mut graph).unwrap();

        assert!(graph.layerless_nodes[0].position.y.is_finite());
    }

    #[test]
    fn fixed_or_crowded_ports_preserve_their_precomputed_positions() {
        let mut graph = graph(
            vec![
                node("A", 40.0, 20.0),
                node("B", 40.0, 20.0),
                node("X", 80.0, 160.0),
            ],
            vec![edge("A-X", "A", "X"), edge("B-X", "B", "X")],
        );
        graph.layerless_nodes[2].node_flexibility = NodeFlexibility::PortPosition;
        assert!(is_flexible(&graph, 2));
        graph.layerless_nodes[2].port_constraints = crate::PortConstraints::FixedPos;
        assert!(!is_flexible(&graph, 2));
        graph.layerless_nodes[2].port_constraints = crate::PortConstraints::Free;
        graph.layerless_nodes[2].size.height = 10.0;
        assert!(!is_flexible(&graph, 2));
    }

    #[test]
    fn flexible_large_coordinates_and_bottom_labels_remain_source_aligned() {
        use NodeLabelPlacement::*;
        for height in [160.5, 100_000_000.5] {
            for placement in [
                InsideBottomLeft,
                InsideBottomCenter,
                InsideBottomRight,
                OutsideBottomLeft,
                OutsideBottomCenter,
                OutsideBottomRight,
                OutsideLeftBottom,
                OutsideRightBottom,
            ] {
                let mut middle = node("X", 80.0, height);
                middle.node_flexibility = NodeFlexibility::PortPosition;
                middle.node_label_placement = placement;
                let mut graph = graph(
                    vec![node("A", 40.0, 20.0), middle, node("B", 40.0, 20.0)],
                    vec![edge("A-X", "A", "X"), edge("X-B", "X", "B")],
                );
                for node in 0..3 {
                    graph.set_node_layer(node, node);
                }
                graph.options.direction = ElkDirection::Right;
                crate::p3order::process_port_sides(&mut graph);
                let mut label = crate::graph::LLabel::new("bottom", 20.0, 10.0);
                label.position.y = height + 7.0;
                graph.layerless_nodes[1].labels.push(label);

                place_nodes_network_simplex(&mut graph).unwrap();

                let middle = &graph.layerless_nodes[1];
                assert_eq!(middle.size.height, height);
                assert_eq!(middle.labels[0].position.y, height + 7.5, "{placement:?}");
                assert!(
                    middle
                        .ports
                        .iter()
                        .all(|port| port.position.y >= 0.0 && port.position.y <= height.ceil())
                );
            }
        }
    }

    #[test]
    fn flexible_height_outside_integer_range_propagates_from_pipeline() {
        let mut middle = node("X", 80.0, i32::MAX as f64 + 1.0);
        middle.node_flexibility = NodeFlexibility::PortPosition;
        let mut graph = graph(
            vec![node("A", 40.0, 20.0), middle, node("B", 40.0, 20.0)],
            vec![edge("A-X", "A", "X"), edge("X-B", "X", "B")],
        );
        graph.options.direction = ElkDirection::Right;
        graph.options.node_placement_strategy = crate::NodePlacementStrategy::NetworkSimplex;

        assert_eq!(
            crate::execute_ported_processors(&mut graph),
            Err(crate::PipelineError::NetworkSimplex(
                NetworkSimplexError::CoordinateOutOfRange
            )),
        );
    }

    #[test]
    fn port_flexibility_matches_elkjs_0_9_3_geometry() {
        for flexibility in [NodeFlexibility::None, NodeFlexibility::PortPosition] {
            let mut middle = node("X", 80.0, 160.0);
            middle.node_flexibility = flexibility;
            middle.ports_surrounding = Some(crate::SpacingMargin {
                top: 12.0,
                right: 12.0,
                bottom: 12.0,
                left: 12.0,
            });
            let mut graph = import_graph(&ElkInputGraph {
                id: "root".into(),
                options: LayeredOptions {
                    direction: ElkDirection::Right,
                    node_placement_strategy: crate::NodePlacementStrategy::NetworkSimplex,
                    ..Default::default()
                },
                nodes: vec![
                    node("A", 40.0, 20.0),
                    node("B", 40.0, 50.0),
                    middle,
                    node("C", 40.0, 30.0),
                    node("D", 40.0, 70.0),
                ],
                edges: vec![
                    edge("A-X", "A", "X"),
                    edge("B-X", "B", "X"),
                    edge("X-C", "X", "C"),
                    edge("X-D", "X", "D"),
                ],
            })
            .unwrap();
            crate::execute_ported_processors(&mut graph).unwrap();
            // Published elkjs coordinates include root padding (12, 12).
            let expected_y = match flexibility {
                NodeFlexibility::None => [110.0, 40.0, 12.0, 50.0, 100.0],
                NodeFlexibility::PortPosition => [82.0, 12.0, 37.0, 22.0, 72.0],
            };
            for (node, y) in graph.layerless_nodes.iter().zip(expected_y) {
                assert_eq!(
                    node.position.y + graph.padding.top,
                    y,
                    "{flexibility:?}: {}",
                    node.id
                );
            }
            assert_eq!(graph.layerless_nodes[2].size.height, 160.0);
            let straight_edges = graph
                .edges
                .iter()
                .filter(|edge| {
                    let source = &graph.layerless_nodes[edge.source.node];
                    let target = &graph.layerless_nodes[edge.target.node];
                    source.position.y + source.ports[edge.source.port].position.y
                        == target.position.y + target.ports[edge.target.port].position.y
                })
                .count();
            assert_eq!(
                straight_edges,
                if flexibility == NodeFlexibility::PortPosition {
                    4
                } else {
                    2
                }
            );
        }
    }
}
