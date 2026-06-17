//! Self-loop lifecycle processors.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/SelfLoopPreProcessor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/SelfLoopPortRestorer.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/SelfLoopRouter.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/SelfLoopPostProcessor.java
//! - https://github.com/eclipse-elk/elk/tree/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/loops

use std::collections::VecDeque;

use crate::graph::{
    LGraph, LMargin, LNodeKind, LPoint, PortRef, PortSide, SelfHyperLoop, SelfLoopEdge,
    SelfLoopHolder, SelfLoopPort, SelfLoopType,
};
use crate::options::SelfLoopDistributionStrategy;

const UNCONNECTED_PORT_PENALTY: usize = 1;
const CONNECTED_PORT_PENALTY: usize = 3;

pub fn preprocess_self_loops(graph: &mut LGraph) {
    graph.self_loop_holders.clear();

    let node_count = graph.layerless_nodes.len();
    for node in 0..node_count {
        if graph.layerless_nodes[node].kind != LNodeKind::Normal || !node_has_self_loop(graph, node)
        {
            continue;
        }

        let mut holder = install_self_loop_holder(graph, node);
        hide_self_loop_edges(graph, &holder);
        hide_ports(graph, &mut holder);
        graph.self_loop_holders.push(holder);
    }
}

pub fn restore_self_loop_ports(graph: &mut LGraph) {
    let mut holders = std::mem::take(&mut graph.self_loop_holders);
    for holder in &mut holders {
        if holder.ports_hidden && !holder.original_port_constraints.is_side_fixed() {
            assign_hidden_port_sides(graph, holder);
        }
        compute_self_loop_types(graph, holder);
        determine_loop_routes(graph, holder);
        assign_routing_slots(holder);
    }
    graph.self_loop_holders = holders;
}

pub fn route_self_loops(graph: &mut LGraph) {
    let holders = graph.self_loop_holders.clone();
    for holder in &holders {
        route_self_loop_holder(graph, holder);
    }
}

pub fn postprocess_self_loops(graph: &mut LGraph) {
    let holders = std::mem::take(&mut graph.self_loop_holders);
    for holder in &holders {
        let node_position = graph.layerless_nodes[holder.node].position;
        for hyper_loop in &holder.hyper_loops {
            for sl_edge in &hyper_loop.edges {
                reattach_self_loop_edge(graph, holder.node, sl_edge);
                offset_edge_bend_points(graph, sl_edge.edge, node_position);
            }
        }
    }
}

fn node_has_self_loop(graph: &LGraph, node: usize) -> bool {
    graph.layerless_nodes[node].ports.iter().any(|port| {
        port.outgoing_edges
            .iter()
            .any(|edge| graph.edges[*edge].source.node == graph.edges[*edge].target.node)
    })
}

fn install_self_loop_holder(graph: &LGraph, node: usize) -> SelfLoopHolder {
    let self_loop_edges = graph.layerless_nodes[node]
        .ports
        .iter()
        .flat_map(|port| port.outgoing_edges.iter().copied())
        .filter(|edge| graph.edges[*edge].source.node == graph.edges[*edge].target.node)
        .collect::<Vec<_>>();

    let mut ports = Vec::<SelfLoopPort>::new();
    let mut edges = Vec::<SelfLoopEdge>::new();

    for edge in self_loop_edges {
        let source_port = graph.edges[edge].source.port;
        let target_port = graph.edges[edge].target.port;
        ensure_self_loop_port(graph, node, source_port, &mut ports);
        ensure_self_loop_port(graph, node, target_port, &mut ports);
        edges.push(SelfLoopEdge {
            edge,
            source_port,
            target_port,
        });
    }

    let hyper_loops = initialize_hyper_loops(ports, edges);

    SelfLoopHolder {
        node,
        hyper_loops,
        ports_hidden: false,
        original_port_constraints: graph.layerless_nodes[node].port_constraints,
    }
}

fn ensure_self_loop_port(graph: &LGraph, node: usize, port: usize, ports: &mut Vec<SelfLoopPort>) {
    if ports.iter().any(|candidate| candidate.port == port) {
        return;
    }

    ports.push(SelfLoopPort {
        port,
        had_only_self_loops: port_had_only_self_loops(graph, node, port),
        hidden: false,
    });
}

fn port_had_only_self_loops(graph: &LGraph, node: usize, port: usize) -> bool {
    let port_data = &graph.layerless_nodes[node].ports[port];
    port_data
        .incoming_edges
        .iter()
        .chain(port_data.outgoing_edges.iter())
        .all(|edge| graph.edges[*edge].source.node == graph.edges[*edge].target.node)
}

fn initialize_hyper_loops(
    ports: Vec<SelfLoopPort>,
    edges: Vec<SelfLoopEdge>,
) -> Vec<SelfHyperLoop> {
    let mut visited = vec![false; ports.len()];
    let mut hyper_loops = Vec::new();

    for start in 0..ports.len() {
        if visited[start] {
            continue;
        }

        let mut queue = VecDeque::from([start]);
        let mut hyper_ports = Vec::new();
        let mut hyper_edges = Vec::new();

        while let Some(port_index) = queue.pop_front() {
            if visited[port_index] {
                continue;
            }
            visited[port_index] = true;

            let port = ports[port_index].port;
            hyper_ports.push(ports[port_index].clone());

            for edge in &edges {
                if edge.source_port != port && edge.target_port != port {
                    continue;
                }

                if !hyper_edges
                    .iter()
                    .any(|candidate: &SelfLoopEdge| candidate.edge == edge.edge)
                {
                    hyper_edges.push(edge.clone());
                }

                let opposite_port = if edge.source_port == port {
                    edge.target_port
                } else {
                    edge.source_port
                };
                if let Some(opposite_index) = ports
                    .iter()
                    .position(|candidate| candidate.port == opposite_port)
                    && !visited[opposite_index]
                {
                    queue.push_back(opposite_index);
                }
            }
        }

        hyper_loops.push(SelfHyperLoop {
            ports: hyper_ports,
            edges: hyper_edges,
            self_loop_type: None,
            leftmost_port: None,
            rightmost_port: None,
            occupied_sides: Vec::new(),
            routing_slots: [0; 5],
        });
    }

    hyper_loops
}

fn hide_self_loop_edges(graph: &mut LGraph, holder: &SelfLoopHolder) {
    for hyper_loop in &holder.hyper_loops {
        for edge in &hyper_loop.edges {
            graph.detach_edge(edge.edge);
        }
    }
}

fn hide_ports(graph: &LGraph, holder: &mut SelfLoopHolder) {
    let order_fixed = holder.original_port_constraints.is_order_fixed();
    let hierarchy_mode = graph.layerless_nodes[holder.node]
        .nested_graph
        .as_deref()
        .is_some_and(|nested| nested.graph_properties.external_ports);

    if order_fixed || hierarchy_mode {
        return;
    }

    for hyper_loop in &mut holder.hyper_loops {
        for port in &mut hyper_loop.ports {
            if port.had_only_self_loops {
                port.hidden = true;
                holder.ports_hidden = true;
            }
        }
    }
}

fn assign_hidden_port_sides(graph: &mut LGraph, holder: &SelfLoopHolder) {
    match graph.options.self_loop_distribution {
        SelfLoopDistributionStrategy::North => assign_hidden_ports_to_north(graph, holder),
        SelfLoopDistributionStrategy::NorthSouth => {
            assign_hidden_ports_to_north_or_south(graph, holder)
        }
        SelfLoopDistributionStrategy::Equally => assign_hidden_ports_equally(graph, holder),
    }
}

fn assign_hidden_ports_to_north(graph: &mut LGraph, holder: &SelfLoopHolder) {
    for hyper_loop in &holder.hyper_loops {
        for port in hyper_loop.ports.iter().filter(|port| port.hidden) {
            graph.layerless_nodes[holder.node].ports[port.port].set_side(PortSide::North);
        }
    }
}

fn assign_hidden_ports_to_north_or_south(graph: &mut LGraph, holder: &SelfLoopHolder) {
    let mut north_ports = 0usize;
    let mut south_ports = 0usize;

    for hyper_loop in &holder.hyper_loops {
        let hidden_ports = hyper_loop
            .ports
            .iter()
            .filter(|port| port.hidden)
            .collect::<Vec<_>>();
        let side = if north_ports <= south_ports {
            north_ports += hidden_ports.len();
            PortSide::North
        } else {
            south_ports += hidden_ports.len();
            PortSide::South
        };

        for port in hidden_ports {
            graph.layerless_nodes[holder.node].ports[port.port].set_side(side);
        }
    }
}

fn assign_hidden_ports_equally(graph: &mut LGraph, holder: &SelfLoopHolder) {
    let mut sorted_loop_indices = (0..holder.hyper_loops.len()).collect::<Vec<_>>();
    sorted_loop_indices.sort_by_key(|loop_index| {
        (
            std::cmp::Reverse(holder.hyper_loops[*loop_index].ports.len()),
            *loop_index,
        )
    });

    for (assignment_index, loop_index) in sorted_loop_indices.into_iter().enumerate() {
        let hyper_loop = &holder.hyper_loops[loop_index];
        let target = EqualDistributionTarget::VALUES
            [assignment_index % EqualDistributionTarget::VALUES.len()];
        let mut hidden_ports = hyper_loop
            .ports
            .iter()
            .filter(|port| port.hidden)
            .collect::<Vec<_>>();

        if hidden_ports.is_empty() {
            continue;
        }

        if target.is_corner() {
            hidden_ports.sort_by_key(|port| self_loop_port_net_flow(hyper_loop, port.port));
        }

        let second_half_start = hidden_ports.len() / 2;
        for (index, port) in hidden_ports.into_iter().enumerate() {
            let side = if index < second_half_start {
                target.first_side
            } else {
                target.second_side
            };
            graph.layerless_nodes[holder.node].ports[port.port].set_side(side);
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct EqualDistributionTarget {
    first_side: PortSide,
    second_side: PortSide,
}

impl EqualDistributionTarget {
    const VALUES: [Self; 8] = [
        Self::new(PortSide::North, PortSide::North),
        Self::new(PortSide::South, PortSide::South),
        Self::new(PortSide::East, PortSide::East),
        Self::new(PortSide::West, PortSide::West),
        Self::new(PortSide::West, PortSide::North),
        Self::new(PortSide::North, PortSide::East),
        Self::new(PortSide::South, PortSide::West),
        Self::new(PortSide::East, PortSide::South),
    ];

    const fn new(first_side: PortSide, second_side: PortSide) -> Self {
        Self {
            first_side,
            second_side,
        }
    }

    fn is_corner(self) -> bool {
        self.first_side != self.second_side
    }
}

fn compute_self_loop_types(graph: &LGraph, holder: &mut SelfLoopHolder) {
    for hyper_loop in &mut holder.hyper_loops {
        let mut sides = hyper_loop
            .ports
            .iter()
            .map(|port| graph.layerless_nodes[holder.node].ports[port.port].side)
            .filter(|side| *side != PortSide::Undefined)
            .collect::<Vec<_>>();
        sides.sort_by_key(|side| side.ordinal());
        sides.dedup();

        hyper_loop.self_loop_type = self_loop_type_from_sides(&sides);
    }
}

fn self_loop_port_net_flow(hyper_loop: &SelfHyperLoop, port: usize) -> isize {
    let incoming = hyper_loop
        .edges
        .iter()
        .filter(|edge| edge.target_port == port)
        .count() as isize;
    let outgoing = hyper_loop
        .edges
        .iter()
        .filter(|edge| edge.source_port == port)
        .count() as isize;
    incoming - outgoing
}

fn self_loop_type_from_sides(sides: &[PortSide]) -> Option<SelfLoopType> {
    match sides.len() {
        1 => Some(SelfLoopType::OneSide),
        2 => {
            let opposing = (sides.contains(&PortSide::East) && sides.contains(&PortSide::West))
                || (sides.contains(&PortSide::North) && sides.contains(&PortSide::South));
            if opposing {
                Some(SelfLoopType::TwoSidesOpposing)
            } else {
                Some(SelfLoopType::TwoSidesCorner)
            }
        }
        3 => Some(SelfLoopType::ThreeSides),
        4 => Some(SelfLoopType::FourSides),
        _ => None,
    }
}

fn determine_loop_routes(graph: &LGraph, holder: &mut SelfLoopHolder) {
    let port_penalties = compute_port_penalties(graph, holder.node);

    for hyper_loop in &mut holder.hyper_loops {
        hyper_loop.ports.sort_by_key(|port| port.port);

        let Some(loop_type) = hyper_loop.self_loop_type else {
            continue;
        };

        match loop_type {
            SelfLoopType::OneSide => {
                let side = graph.layerless_nodes[holder.node].ports[hyper_loop.ports[0].port].side;
                assign_leftmost_rightmost_ports(graph, holder.node, hyper_loop, side, side);
            }
            SelfLoopType::TwoSidesCorner => {
                if let Some((left, right)) =
                    sorted_two_side_loop_port_sides(graph, holder.node, hyper_loop)
                {
                    assign_leftmost_rightmost_ports(graph, holder.node, hyper_loop, left, right);
                }
            }
            SelfLoopType::TwoSidesOpposing => {
                determine_two_side_opposing_loop_route(
                    graph,
                    holder.node,
                    hyper_loop,
                    &port_penalties,
                );
            }
            SelfLoopType::ThreeSides => {
                if let Some((left, right)) = three_side_route_sides(graph, holder.node, hyper_loop)
                {
                    assign_leftmost_rightmost_ports(graph, holder.node, hyper_loop, left, right);
                }
            }
            SelfLoopType::FourSides => {
                determine_four_side_loop_route(graph, holder.node, hyper_loop, &port_penalties);
            }
        }

        compute_occupied_sides(graph, holder.node, hyper_loop);
    }
}

fn determine_two_side_opposing_loop_route(
    graph: &LGraph,
    node: usize,
    hyper_loop: &mut SelfHyperLoop,
    port_penalties: &[usize],
) {
    let sides = loop_sides(graph, node, hyper_loop);
    if sides.len() != 2 {
        return;
    }

    let Some(option1_leftmost_port) = lowest_port_on_side(graph, node, hyper_loop, sides[0]) else {
        return;
    };
    let Some(option1_rightmost_port) = highest_port_on_side(graph, node, hyper_loop, sides[1])
    else {
        return;
    };
    let option1_penalty = compute_edge_penalty(
        graph,
        node,
        option1_leftmost_port,
        option1_rightmost_port,
        port_penalties,
    );

    let Some(option2_leftmost_port) = lowest_port_on_side(graph, node, hyper_loop, sides[1]) else {
        return;
    };
    let Some(option2_rightmost_port) = highest_port_on_side(graph, node, hyper_loop, sides[0])
    else {
        return;
    };
    let option2_penalty = compute_edge_penalty(
        graph,
        node,
        option2_leftmost_port,
        option2_rightmost_port,
        port_penalties,
    );

    if option1_penalty <= option2_penalty {
        hyper_loop.leftmost_port = Some(option1_leftmost_port);
        hyper_loop.rightmost_port = Some(option1_rightmost_port);
    } else {
        hyper_loop.leftmost_port = Some(option2_leftmost_port);
        hyper_loop.rightmost_port = Some(option2_rightmost_port);
    }
}

fn determine_four_side_loop_route(
    graph: &LGraph,
    node: usize,
    hyper_loop: &mut SelfHyperLoop,
    port_penalties: &[usize],
) {
    let Some(mut worst_left_port) = hyper_loop.ports.last().map(|port| port.port) else {
        return;
    };
    let Some(mut worst_right_port) = hyper_loop.ports.first().map(|port| port.port) else {
        return;
    };
    let mut worst_penalty = compute_edge_penalty(
        graph,
        node,
        worst_left_port,
        worst_right_port,
        port_penalties,
    );

    for ports in hyper_loop.ports.windows(2) {
        let current_left_port = ports[0].port;
        let current_right_port = ports[1].port;
        let current_penalty = compute_edge_penalty(
            graph,
            node,
            current_left_port,
            current_right_port,
            port_penalties,
        );

        if current_penalty > worst_penalty {
            worst_left_port = current_left_port;
            worst_right_port = current_right_port;
            worst_penalty = current_penalty;
        }
    }

    hyper_loop.leftmost_port = Some(worst_right_port);
    hyper_loop.rightmost_port = Some(worst_left_port);
}

fn sorted_two_side_loop_port_sides(
    graph: &LGraph,
    node: usize,
    hyper_loop: &SelfHyperLoop,
) -> Option<(PortSide, PortSide)> {
    let mut sides = loop_sides(graph, node, hyper_loop);
    if sides.len() != 2 {
        return None;
    }
    sides.sort_by_key(|side| side.ordinal());
    if sides[0] == PortSide::North && sides[1] == PortSide::West {
        Some((PortSide::West, PortSide::North))
    } else {
        Some((sides[0], sides[1]))
    }
}

fn three_side_route_sides(
    graph: &LGraph,
    node: usize,
    hyper_loop: &SelfHyperLoop,
) -> Option<(PortSide, PortSide)> {
    let sides = loop_sides(graph, node, hyper_loop);
    if sides.len() != 3 {
        return None;
    }

    if !sides.contains(&PortSide::North) {
        Some((PortSide::East, PortSide::West))
    } else if !sides.contains(&PortSide::East) {
        Some((PortSide::South, PortSide::North))
    } else if !sides.contains(&PortSide::South) {
        Some((PortSide::West, PortSide::East))
    } else if !sides.contains(&PortSide::West) {
        Some((PortSide::North, PortSide::South))
    } else {
        None
    }
}

fn loop_sides(graph: &LGraph, node: usize, hyper_loop: &SelfHyperLoop) -> Vec<PortSide> {
    let mut sides = hyper_loop
        .ports
        .iter()
        .map(|port| graph.layerless_nodes[node].ports[port.port].side)
        .filter(|side| *side != PortSide::Undefined)
        .collect::<Vec<_>>();
    sides.sort_by_key(|side| side.ordinal());
    sides.dedup();
    sides
}

fn assign_leftmost_rightmost_ports(
    graph: &LGraph,
    node: usize,
    hyper_loop: &mut SelfHyperLoop,
    leftmost_side: PortSide,
    rightmost_side: PortSide,
) {
    hyper_loop.leftmost_port = hyper_loop
        .ports
        .iter()
        .filter(|port| graph.layerless_nodes[node].ports[port.port].side == leftmost_side)
        .map(|port| port.port)
        .min();
    hyper_loop.rightmost_port = hyper_loop
        .ports
        .iter()
        .filter(|port| graph.layerless_nodes[node].ports[port.port].side == rightmost_side)
        .map(|port| port.port)
        .max();
}

fn lowest_port_on_side(
    graph: &LGraph,
    node: usize,
    hyper_loop: &SelfHyperLoop,
    side: PortSide,
) -> Option<usize> {
    hyper_loop
        .ports
        .iter()
        .filter(|port| graph.layerless_nodes[node].ports[port.port].side == side)
        .map(|port| port.port)
        .min()
}

fn highest_port_on_side(
    graph: &LGraph,
    node: usize,
    hyper_loop: &SelfHyperLoop,
    side: PortSide,
) -> Option<usize> {
    hyper_loop
        .ports
        .iter()
        .filter(|port| graph.layerless_nodes[node].ports[port.port].side == side)
        .map(|port| port.port)
        .max()
}

fn compute_port_penalties(graph: &LGraph, node: usize) -> Vec<usize> {
    let mut penalty_sum = 0usize;
    graph.layerless_nodes[node]
        .ports
        .iter()
        .map(|port| {
            if port.incoming_edges.is_empty() && port.outgoing_edges.is_empty() {
                penalty_sum += UNCONNECTED_PORT_PENALTY;
            } else {
                penalty_sum += CONNECTED_PORT_PENALTY;
            }
            penalty_sum
        })
        .collect()
}

fn compute_edge_penalty(
    graph: &LGraph,
    node: usize,
    leftmost_port: usize,
    rightmost_port: usize,
    port_penalties: &[usize],
) -> usize {
    let port_count = graph.layerless_nodes[node].ports.len();
    if port_count == 0
        || port_penalties.len() != port_count
        || leftmost_port >= port_count
        || rightmost_port >= port_count
    {
        return 0;
    }

    let left_of_rightmost_port = if rightmost_port == 0 {
        port_count - 1
    } else {
        rightmost_port - 1
    };

    if leftmost_port <= left_of_rightmost_port {
        port_penalties[left_of_rightmost_port] - port_penalties[leftmost_port]
    } else {
        port_penalties[port_count - 1] - port_penalties[leftmost_port]
            + port_penalties[left_of_rightmost_port]
    }
}

fn compute_occupied_sides(graph: &LGraph, node: usize, hyper_loop: &mut SelfHyperLoop) {
    hyper_loop.occupied_sides.clear();

    let (Some(leftmost_port), Some(rightmost_port)) =
        (hyper_loop.leftmost_port, hyper_loop.rightmost_port)
    else {
        return;
    };

    let mut current = graph.layerless_nodes[node].ports[leftmost_port].side;
    let target = graph.layerless_nodes[node].ports[rightmost_port].side;
    if current == PortSide::Undefined || target == PortSide::Undefined {
        return;
    }

    loop {
        if !hyper_loop.occupied_sides.contains(&current) {
            hyper_loop.occupied_sides.push(current);
        }
        if current == target {
            break;
        }
        current = current.right();
    }
}

fn assign_routing_slots(holder: &mut SelfLoopHolder) {
    let mut next_slot = [0usize; 5];
    for hyper_loop in &mut holder.hyper_loops {
        for side in hyper_loop.occupied_sides.clone() {
            let ordinal = side.ordinal();
            hyper_loop.routing_slots[ordinal] = next_slot[ordinal];
            next_slot[ordinal] += 1;
        }
    }
}

fn route_self_loop_holder(graph: &mut LGraph, holder: &SelfLoopHolder) {
    let mut new_margins = graph.layerless_nodes[holder.node].margin;
    let routing_slot_positions = compute_routing_slot_positions(graph, holder);

    for hyper_loop in &holder.hyper_loops {
        for sl_edge in &hyper_loop.edges {
            let direction = compute_edge_routing_direction(graph, holder.node, hyper_loop, sl_edge);
            let bend_points = compute_orthogonal_bend_points(
                graph,
                holder.node,
                hyper_loop,
                sl_edge,
                direction,
                &routing_slot_positions,
            );
            graph.edges[sl_edge.edge].bend_points = bend_points;
            for point in graph.edges[sl_edge.edge].bend_points.iter().copied() {
                update_node_margins_for_bend_point(graph, holder.node, &mut new_margins, point);
            }
        }
    }

    graph.layerless_nodes[holder.node].margin = new_margins;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EdgeRoutingDirection {
    Clockwise,
    CounterClockwise,
}

fn compute_edge_routing_direction(
    graph: &LGraph,
    node: usize,
    hyper_loop: &SelfHyperLoop,
    sl_edge: &SelfLoopEdge,
) -> EdgeRoutingDirection {
    let source_side = graph.layerless_nodes[node].ports[sl_edge.source_port].side;
    let target_side = graph.layerless_nodes[node].ports[sl_edge.target_port].side;

    if source_side == target_side {
        if sl_edge.source_port < sl_edge.target_port {
            EdgeRoutingDirection::Clockwise
        } else {
            EdgeRoutingDirection::CounterClockwise
        }
    } else if source_side.right() == target_side {
        EdgeRoutingDirection::Clockwise
    } else if source_side.left() == target_side {
        EdgeRoutingDirection::CounterClockwise
    } else if hyper_loop.occupied_sides.contains(&source_side.right()) {
        EdgeRoutingDirection::Clockwise
    } else {
        EdgeRoutingDirection::CounterClockwise
    }
}

fn compute_routing_slot_positions(graph: &LGraph, holder: &SelfLoopHolder) -> [Vec<f64>; 5] {
    let mut slot_count = [0usize; 5];
    for hyper_loop in &holder.hyper_loops {
        for side in &hyper_loop.occupied_sides {
            let ordinal = side.ordinal();
            slot_count[ordinal] = slot_count[ordinal].max(hyper_loop.routing_slots[ordinal] + 1);
        }
    }

    std::array::from_fn(|side_index| {
        let Some(side) = side_from_ordinal(side_index) else {
            return Vec::new();
        };

        let mut positions = Vec::with_capacity(slot_count[side_index]);
        let mut current = compute_baseline_position(graph, holder.node, side);
        let factor = if matches!(side, PortSide::North | PortSide::West) {
            -1.0
        } else {
            1.0
        };

        for _ in 0..slot_count[side_index] {
            positions.push(current);
            current += factor * graph.options.spacing.edge_edge;
        }

        positions
    })
}

fn side_from_ordinal(ordinal: usize) -> Option<PortSide> {
    match ordinal {
        1 => Some(PortSide::North),
        2 => Some(PortSide::East),
        3 => Some(PortSide::South),
        4 => Some(PortSide::West),
        _ => None,
    }
}

fn compute_baseline_position(graph: &LGraph, node: usize, side: PortSide) -> f64 {
    let lnode = &graph.layerless_nodes[node];
    let node_self_loop_distance = graph.options.spacing.edge_node;
    match side {
        PortSide::North => -lnode.margin.top - node_self_loop_distance,
        PortSide::East => lnode.size.width + lnode.margin.right + node_self_loop_distance,
        PortSide::South => lnode.size.height + lnode.margin.bottom + node_self_loop_distance,
        PortSide::West => -lnode.margin.left - node_self_loop_distance,
        PortSide::Undefined => 0.0,
    }
}

fn compute_orthogonal_bend_points(
    graph: &LGraph,
    node: usize,
    hyper_loop: &SelfHyperLoop,
    sl_edge: &SelfLoopEdge,
    routing_direction: EdgeRoutingDirection,
    routing_slot_positions: &[Vec<f64>; 5],
) -> Vec<LPoint> {
    let mut bend_points = Vec::new();

    add_outer_bend_point(
        graph,
        node,
        hyper_loop,
        sl_edge.source_port,
        routing_slot_positions,
        &mut bend_points,
    );
    add_corner_bend_points(
        graph,
        node,
        hyper_loop,
        sl_edge,
        routing_direction,
        routing_slot_positions,
        &mut bend_points,
    );
    add_outer_bend_point(
        graph,
        node,
        hyper_loop,
        sl_edge.target_port,
        routing_slot_positions,
        &mut bend_points,
    );

    bend_points
}

fn add_outer_bend_point(
    graph: &LGraph,
    node: usize,
    hyper_loop: &SelfHyperLoop,
    port: usize,
    routing_slot_positions: &[Vec<f64>; 5],
    bend_points: &mut Vec<LPoint>,
) {
    let port_data = &graph.layerless_nodes[node].ports[port];
    let side = port_data.side;
    let slot = hyper_loop.routing_slots[side.ordinal()];
    let mut result = base_vector(side, slot, routing_slot_positions);
    let anchor = LPoint {
        x: port_data.position.x + port_data.anchor.x,
        y: port_data.position.y + port_data.anchor.y,
    };

    match side {
        PortSide::North | PortSide::South => {
            result.x += anchor.x;
        }
        PortSide::East | PortSide::West => {
            result.y += anchor.y;
        }
        PortSide::Undefined => {}
    }

    bend_points.push(result);
}

fn add_corner_bend_points(
    graph: &LGraph,
    node: usize,
    hyper_loop: &SelfHyperLoop,
    sl_edge: &SelfLoopEdge,
    routing_direction: EdgeRoutingDirection,
    routing_slot_positions: &[Vec<f64>; 5],
    bend_points: &mut Vec<LPoint>,
) {
    let source_side = graph.layerless_nodes[node].ports[sl_edge.source_port].side;
    let target_side = graph.layerless_nodes[node].ports[sl_edge.target_port].side;

    if source_side == target_side {
        return;
    }

    let mut current_side = source_side;
    while current_side != target_side {
        let next_side = match routing_direction {
            EdgeRoutingDirection::Clockwise => current_side.right(),
            EdgeRoutingDirection::CounterClockwise => current_side.left(),
        };
        let current_component = base_vector(
            current_side,
            hyper_loop.routing_slots[current_side.ordinal()],
            routing_slot_positions,
        );
        let next_component = base_vector(
            next_side,
            hyper_loop.routing_slots[next_side.ordinal()],
            routing_slot_positions,
        );
        bend_points.push(LPoint {
            x: current_component.x + next_component.x,
            y: current_component.y + next_component.y,
        });
        current_side = next_side;
    }
}

fn base_vector(side: PortSide, slot: usize, routing_slot_positions: &[Vec<f64>; 5]) -> LPoint {
    let position = routing_slot_positions[side.ordinal()]
        .get(slot)
        .copied()
        .unwrap_or(0.0);
    match side {
        PortSide::North | PortSide::South => LPoint {
            x: 0.0,
            y: position,
        },
        PortSide::East | PortSide::West => LPoint {
            x: position,
            y: 0.0,
        },
        PortSide::Undefined => LPoint::default(),
    }
}

fn update_node_margins_for_bend_point(
    graph: &LGraph,
    node: usize,
    margins: &mut LMargin,
    point: LPoint,
) {
    let size = graph.layerless_nodes[node].size;
    margins.left = margins.left.max(-point.x);
    margins.right = margins.right.max(point.x - size.width);
    margins.top = margins.top.max(-point.y);
    margins.bottom = margins.bottom.max(point.y - size.height);
}

fn reattach_self_loop_edge(graph: &mut LGraph, node: usize, sl_edge: &SelfLoopEdge) {
    let source = PortRef {
        node,
        port: sl_edge.source_port,
    };
    let target = PortRef {
        node,
        port: sl_edge.target_port,
    };

    if !graph.edge_source_attached(sl_edge.edge) {
        graph.layerless_nodes[node].ports[sl_edge.source_port]
            .outgoing_edges
            .push(sl_edge.edge);
    }
    if !graph.edge_target_attached(sl_edge.edge) {
        graph.layerless_nodes[node].ports[sl_edge.target_port]
            .incoming_edges
            .push(sl_edge.edge);
    }

    graph.edges[sl_edge.edge].source = source;
    graph.edges[sl_edge.edge].target = target;
}

fn offset_edge_bend_points(graph: &mut LGraph, edge: usize, offset: LPoint) {
    for point in &mut graph.edges[edge].bend_points {
        point.x += offset.x;
        point.y += offset.y;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{LNode, LayeredEdge, PortType};
    use crate::importer::{ElkInputEdge, ElkInputGraph, ElkInputNode, import_graph};
    use crate::options::{ElkDirection, LayeredOptions, PortConstraints};

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
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
        }
    }

    fn self_loop_edge(id: &str, node_id: &str, source: PortRef, target: PortRef) -> LayeredEdge {
        layered_edge(id, node_id, node_id, source, target)
    }

    fn layered_edge(
        id: &str,
        source_node_id: &str,
        target_node_id: &str,
        source: PortRef,
        target: PortRef,
    ) -> LayeredEdge {
        LayeredEdge {
            id: id.to_string(),
            source,
            target,
            source_node_id: source_node_id.to_string(),
            target_node_id: target_node_id.to_string(),
            labels: Vec::new(),
            minlen: 1,
            reversed: false,
            bend_points: Vec::new(),
            model_order: None,
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
            thickness: 0.0,
            original_opposite_port: None,
            compound_segment: None,
        }
    }

    fn empty_self_loop_holder(node: usize, hyper_loop: SelfHyperLoop) -> SelfLoopHolder {
        SelfLoopHolder {
            node,
            hyper_loops: vec![hyper_loop],
            ports_hidden: false,
            original_port_constraints: PortConstraints::FixedSide,
        }
    }

    fn self_loop_port(port: PortRef) -> SelfLoopPort {
        SelfLoopPort {
            port: port.port,
            had_only_self_loops: false,
            hidden: false,
        }
    }

    fn hyper_loop_with_ports(ports: &[PortRef], self_loop_type: SelfLoopType) -> SelfHyperLoop {
        SelfHyperLoop {
            ports: ports.iter().copied().map(self_loop_port).collect(),
            edges: Vec::new(),
            self_loop_type: Some(self_loop_type),
            leftmost_port: None,
            rightmost_port: None,
            occupied_sides: Vec::new(),
            routing_slots: [0; 5],
        }
    }

    fn add_node(graph: &mut LGraph, id: &str) -> usize {
        let node = graph.layerless_nodes.len();
        graph.layerless_nodes.push(LNode::new(id, 80.0, 40.0, None));
        node
    }

    fn add_port(graph: &mut LGraph, node: usize, side: PortSide) -> PortRef {
        graph
            .add_port(node, PortType::Output, side, LPoint::default())
            .unwrap()
    }

    fn connect_to_sink(graph: &mut LGraph, source: PortRef, sink: PortRef, id: &str) {
        graph.add_edge(layered_edge(id, "A", "B", source, sink));
    }

    #[test]
    fn preprocess_detaches_self_loop_edges_from_ports() {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
            nodes: vec![node("A")],
            edges: vec![edge("A-A", "A", "A")],
        })
        .unwrap();

        preprocess_self_loops(&mut graph);

        assert_eq!(graph.self_loop_holders.len(), 1);
        assert!(!graph.edge_source_attached(0));
        assert!(!graph.edge_target_attached(0));
        assert!(graph.self_loop_holders[0].ports_hidden);
    }

    #[test]
    fn restorer_assigns_first_equal_distribution_loop_to_north_side() {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
            nodes: vec![node("A")],
            edges: vec![edge("A-A", "A", "A")],
        })
        .unwrap();

        preprocess_self_loops(&mut graph);
        restore_self_loop_ports(&mut graph);

        assert!(
            graph.layerless_nodes[0]
                .ports
                .iter()
                .all(|port| port.side == PortSide::North)
        );
        assert_eq!(
            graph.self_loop_holders[0].hyper_loops[0].self_loop_type,
            Some(SelfLoopType::OneSide)
        );
    }

    #[test]
    fn restorer_assigns_equal_distribution_targets_by_descending_loop_port_count() {
        let mut graph = LGraph::new(
            "root",
            LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        );
        let node = add_node(&mut graph, "A");
        let single_port = graph
            .add_port(
                node,
                PortType::Output,
                PortSide::Undefined,
                LPoint::default(),
            )
            .unwrap();
        let double_source = graph
            .add_port(
                node,
                PortType::Output,
                PortSide::Undefined,
                LPoint::default(),
            )
            .unwrap();
        let double_target = graph
            .add_port(
                node,
                PortType::Input,
                PortSide::Undefined,
                LPoint::default(),
            )
            .unwrap();

        graph.add_edge(self_loop_edge("single", "A", single_port, single_port));
        graph.add_edge(self_loop_edge("double", "A", double_source, double_target));

        preprocess_self_loops(&mut graph);
        assert_eq!(graph.self_loop_holders[0].hyper_loops[0].ports.len(), 1);
        assert_eq!(graph.self_loop_holders[0].hyper_loops[1].ports.len(), 2);

        restore_self_loop_ports(&mut graph);

        assert_eq!(
            graph.layerless_nodes[node].ports[single_port.port].side,
            PortSide::South
        );
        assert_eq!(
            graph.layerless_nodes[node].ports[double_source.port].side,
            PortSide::North
        );
        assert_eq!(
            graph.layerless_nodes[node].ports[double_target.port].side,
            PortSide::North
        );
    }

    #[test]
    fn routing_director_uses_penalty_for_two_side_opposing_loops() {
        let mut graph = LGraph::new(
            "root",
            LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        );
        let node = add_node(&mut graph, "A");
        let sink = add_node(&mut graph, "B");

        let north = add_port(&mut graph, node, PortSide::North);
        let connected_1 = add_port(&mut graph, node, PortSide::East);
        let connected_2 = add_port(&mut graph, node, PortSide::East);
        let south = add_port(&mut graph, node, PortSide::South);
        let west_1 = add_port(&mut graph, node, PortSide::West);
        let west_2 = add_port(&mut graph, node, PortSide::West);
        let sink_1 = add_port(&mut graph, sink, PortSide::West);
        let sink_2 = add_port(&mut graph, sink, PortSide::West);
        connect_to_sink(&mut graph, connected_1, sink_1, "connected-1");
        connect_to_sink(&mut graph, connected_2, sink_2, "connected-2");

        let hyper_loop = hyper_loop_with_ports(&[north, south], SelfLoopType::TwoSidesOpposing);
        let mut holder = empty_self_loop_holder(node, hyper_loop);

        determine_loop_routes(&graph, &mut holder);

        let hyper_loop = &holder.hyper_loops[0];
        assert_eq!(hyper_loop.leftmost_port, Some(south.port));
        assert_eq!(hyper_loop.rightmost_port, Some(north.port));
        assert_eq!(
            hyper_loop.occupied_sides,
            vec![PortSide::South, PortSide::West, PortSide::North]
        );
        assert_eq!(west_1.port, 4);
        assert_eq!(west_2.port, 5);
    }

    #[test]
    fn routing_director_splits_four_side_loops_at_highest_penalty_gap() {
        let mut graph = LGraph::new(
            "root",
            LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        );
        let node = add_node(&mut graph, "A");
        let sink = add_node(&mut graph, "B");

        let north = add_port(&mut graph, node, PortSide::North);
        let connected_1 = add_port(&mut graph, node, PortSide::North);
        let connected_2 = add_port(&mut graph, node, PortSide::North);
        let east = add_port(&mut graph, node, PortSide::East);
        let south = add_port(&mut graph, node, PortSide::South);
        let west = add_port(&mut graph, node, PortSide::West);
        let sink_1 = add_port(&mut graph, sink, PortSide::West);
        let sink_2 = add_port(&mut graph, sink, PortSide::West);
        connect_to_sink(&mut graph, connected_1, sink_1, "connected-1");
        connect_to_sink(&mut graph, connected_2, sink_2, "connected-2");

        let hyper_loop =
            hyper_loop_with_ports(&[north, east, south, west], SelfLoopType::FourSides);
        let mut holder = empty_self_loop_holder(node, hyper_loop);

        determine_loop_routes(&graph, &mut holder);

        let hyper_loop = &holder.hyper_loops[0];
        assert_eq!(hyper_loop.leftmost_port, Some(east.port));
        assert_eq!(hyper_loop.rightmost_port, Some(north.port));
        assert_eq!(
            hyper_loop.occupied_sides,
            vec![
                PortSide::East,
                PortSide::South,
                PortSide::West,
                PortSide::North
            ]
        );
    }
}
