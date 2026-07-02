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
    LGraph, LMargin, LNodeKind, LPoint, LSize, PortRef, PortSide, SelfHyperLoop,
    SelfHyperLoopLabels, SelfLoopEdge, SelfLoopHolder, SelfLoopLabelAlignment, SelfLoopLabelRef,
    SelfLoopPort, SelfLoopType,
};
use crate::options::{SelfLoopDistributionStrategy, SelfLoopOrderingStrategy};
use crate::p5edges::orthogonal::{HyperEdgeGraph, HyperEdgeSegment, break_non_critical_cycles};

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
        if holder.ports_hidden {
            restore_hidden_ports(graph, holder);
        }
    }
    graph.self_loop_holders = holders;
}

pub fn route_self_loops(graph: &mut LGraph) {
    let mut holders = std::mem::take(&mut graph.self_loop_holders);
    for holder in &mut holders {
        determine_loop_routes(graph, holder);
        place_self_loop_labels(graph, holder);
        assign_routing_slots(graph, holder);
        route_self_loop_holder(graph, holder);
    }
    graph.self_loop_holders = holders;
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
            if let Some(labels) = &hyper_loop.labels {
                offset_self_loop_label_positions(graph, labels, node_position);
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

    let hyper_loops = initialize_hyper_loops(graph, ports, edges);

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
    graph: &LGraph,
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
            labels: None,
            self_loop_type: None,
            leftmost_port: None,
            rightmost_port: None,
            occupied_sides: Vec::new(),
            routing_slots: [0; 5],
        });
    }

    for hyper_loop in &mut hyper_loops {
        initialize_self_hyper_loop_labels(graph, hyper_loop);
    }

    hyper_loops
}

fn initialize_self_hyper_loop_labels(graph: &LGraph, hyper_loop: &mut SelfHyperLoop) {
    let mut label_refs = Vec::new();
    let mut size = LSize::default();
    let label_spacing = graph.options.spacing.label_label;
    let horizontal_layout = !graph.options.direction.is_vertical();

    for sl_edge in &hyper_loop.edges {
        for (label_index, label) in graph.edges[sl_edge.edge].labels.iter().enumerate() {
            label_refs.push(SelfLoopLabelRef {
                edge: sl_edge.edge,
                label: label_index,
            });
            update_self_hyper_loop_label_size(
                &mut size,
                label.size,
                label_refs.len(),
                label_spacing,
                horizontal_layout,
            );
        }
    }

    if !label_refs.is_empty() {
        hyper_loop.labels = Some(SelfHyperLoopLabels {
            id: None,
            label_refs,
            size,
            position: LPoint::default(),
            side: None,
            alignment: None,
            alignment_reference_port: None,
        });
    }
}

fn update_self_hyper_loop_label_size(
    size: &mut LSize,
    label_size: LSize,
    label_count: usize,
    label_spacing: f64,
    horizontal_layout: bool,
) {
    if horizontal_layout {
        size.width = size.width.max(label_size.width);
        size.height += label_size.height;
        if label_count > 1 {
            size.height += label_spacing;
        }
    } else {
        size.width += label_size.width;
        size.height = size.height.max(label_size.height);
        if label_count > 1 {
            size.width += label_spacing;
        }
    }
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

fn restore_hidden_ports(graph: &mut LGraph, holder: &mut SelfLoopHolder) {
    let mut target_areas = PortRestoreTargetAreas::new();
    collect_restore_target_areas(graph, holder, &mut target_areas);

    let old_order = restored_port_order(graph, holder.node, &target_areas);
    let Some(old_to_new) = graph.reorder_node_ports_with_map(holder.node, old_order) else {
        return;
    };

    remap_holder_ports(holder, &old_to_new);
    holder.ports_hidden = false;
    for hyper_loop in &mut holder.hyper_loops {
        for port in &mut hyper_loop.ports {
            port.hidden = false;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PortSideArea {
    Start,
    Middle,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AddMode {
    Prepend,
    Append,
}

#[derive(Debug, Clone)]
struct PortRestoreTargetAreas {
    areas: [[Vec<usize>; 3]; 5],
}

impl PortRestoreTargetAreas {
    fn new() -> Self {
        Self {
            areas: std::array::from_fn(|_| std::array::from_fn(|_| Vec::new())),
        }
    }

    fn add_ports(
        &mut self,
        graph: &LGraph,
        node: usize,
        hyper_loop: &SelfHyperLoop,
        side: PortSide,
        area: PortSideArea,
        mode: AddMode,
    ) {
        let ports = hyper_loop
            .ports
            .iter()
            .filter(|port| port.hidden && graph.layerless_nodes[node].ports[port.port].side == side)
            .map(|port| port.port)
            .collect::<Vec<_>>();
        self.add_port_list(ports, side, area, mode);
    }

    fn add_port_list(
        &mut self,
        mut ports: Vec<usize>,
        side: PortSide,
        area: PortSideArea,
        mode: AddMode,
    ) {
        ports.reverse();
        let target = &mut self.areas[side.ordinal()][area.ordinal()];
        match mode {
            AddMode::Prepend => {
                target.splice(0..0, ports);
            }
            AddMode::Append => target.extend(ports),
        }
    }

    fn get(&self, side: PortSide, area: PortSideArea) -> &[usize] {
        &self.areas[side.ordinal()][area.ordinal()]
    }

    fn ports(&self) -> Vec<usize> {
        self.areas
            .iter()
            .flat_map(|side_areas| side_areas.iter())
            .flat_map(|ports| ports.iter().copied())
            .collect()
    }
}

impl PortSideArea {
    fn ordinal(self) -> usize {
        match self {
            Self::Start => 0,
            Self::Middle => 1,
            Self::End => 2,
        }
    }
}

fn collect_restore_target_areas(
    graph: &LGraph,
    holder: &SelfLoopHolder,
    target_areas: &mut PortRestoreTargetAreas,
) {
    process_one_side_loops(graph, holder, target_areas);
    process_two_side_corner_loops(graph, holder, target_areas);
    process_three_side_loops(graph, holder, target_areas);
    process_four_side_loops(graph, holder, target_areas);
    process_two_side_opposing_loops(graph, holder, target_areas);
}

fn process_one_side_loops(
    graph: &LGraph,
    holder: &SelfLoopHolder,
    target_areas: &mut PortRestoreTargetAreas,
) {
    let mut one_side_loops = holder
        .hyper_loops
        .iter()
        .filter(|hyper_loop| hyper_loop.self_loop_type == Some(SelfLoopType::OneSide))
        .collect::<Vec<_>>();
    if graph.options.self_loop_ordering == SelfLoopOrderingStrategy::ReverseStacked {
        one_side_loops.reverse();
    }

    for hyper_loop in one_side_loops {
        let Some(side) = hyper_loop
            .ports
            .first()
            .map(|port| graph.layerless_nodes[holder.node].ports[port.port].side)
        else {
            continue;
        };

        let mut sorted_ports = hyper_loop.ports.clone();
        sorted_ports.sort_by_key(|port| self_loop_port_net_flow(hyper_loop, port.port));
        match graph.options.self_loop_ordering {
            SelfLoopOrderingStrategy::Sequenced => {
                target_areas.add_port_list(
                    sorted_ports
                        .iter()
                        .filter(|port| port.hidden)
                        .map(|port| port.port)
                        .collect(),
                    side,
                    PortSideArea::Middle,
                    AddMode::Append,
                );
            }
            SelfLoopOrderingStrategy::Stacked | SelfLoopOrderingStrategy::ReverseStacked => {
                let split_index = compute_port_list_split_index(hyper_loop, &sorted_ports);
                target_areas.add_port_list(
                    sorted_ports[..split_index]
                        .iter()
                        .filter(|port| port.hidden)
                        .map(|port| port.port)
                        .collect(),
                    side,
                    PortSideArea::Middle,
                    AddMode::Prepend,
                );
                target_areas.add_port_list(
                    sorted_ports[split_index..]
                        .iter()
                        .filter(|port| port.hidden)
                        .map(|port| port.port)
                        .collect(),
                    side,
                    PortSideArea::Middle,
                    AddMode::Append,
                );
            }
        }
    }
}

fn process_two_side_corner_loops(
    graph: &LGraph,
    holder: &SelfLoopHolder,
    target_areas: &mut PortRestoreTargetAreas,
) {
    for hyper_loop in holder
        .hyper_loops
        .iter()
        .filter(|hyper_loop| hyper_loop.self_loop_type == Some(SelfLoopType::TwoSidesCorner))
    {
        if let Some((start, end)) = sorted_two_side_loop_port_sides(graph, holder.node, hyper_loop)
        {
            target_areas.add_ports(
                graph,
                holder.node,
                hyper_loop,
                start,
                PortSideArea::End,
                AddMode::Prepend,
            );
            target_areas.add_ports(
                graph,
                holder.node,
                hyper_loop,
                end,
                PortSideArea::Start,
                AddMode::Append,
            );
        }
    }
}

fn process_two_side_opposing_loops(
    graph: &LGraph,
    holder: &SelfLoopHolder,
    target_areas: &mut PortRestoreTargetAreas,
) {
    for hyper_loop in holder
        .hyper_loops
        .iter()
        .filter(|hyper_loop| hyper_loop.self_loop_type == Some(SelfLoopType::TwoSidesOpposing))
    {
        if let Some((start, end)) = sorted_two_side_loop_port_sides(graph, holder.node, hyper_loop)
        {
            target_areas.add_ports(
                graph,
                holder.node,
                hyper_loop,
                start,
                PortSideArea::End,
                AddMode::Prepend,
            );
            target_areas.add_ports(
                graph,
                holder.node,
                hyper_loop,
                end,
                PortSideArea::Start,
                AddMode::Append,
            );
        }
    }
}

fn process_three_side_loops(
    graph: &LGraph,
    holder: &SelfLoopHolder,
    target_areas: &mut PortRestoreTargetAreas,
) {
    for hyper_loop in holder
        .hyper_loops
        .iter()
        .filter(|hyper_loop| hyper_loop.self_loop_type == Some(SelfLoopType::ThreeSides))
    {
        let Some((start, middle, end)) = three_side_restore_sides(graph, holder.node, hyper_loop)
        else {
            continue;
        };
        target_areas.add_ports(
            graph,
            holder.node,
            hyper_loop,
            start,
            PortSideArea::End,
            AddMode::Prepend,
        );
        target_areas.add_ports(
            graph,
            holder.node,
            hyper_loop,
            middle,
            PortSideArea::Middle,
            AddMode::Append,
        );
        target_areas.add_ports(
            graph,
            holder.node,
            hyper_loop,
            end,
            PortSideArea::Start,
            AddMode::Append,
        );
    }
}

fn process_four_side_loops(
    graph: &LGraph,
    holder: &SelfLoopHolder,
    target_areas: &mut PortRestoreTargetAreas,
) {
    for hyper_loop in holder
        .hyper_loops
        .iter()
        .filter(|hyper_loop| hyper_loop.self_loop_type == Some(SelfLoopType::FourSides))
    {
        for side in loop_sides(graph, holder.node, hyper_loop) {
            target_areas.add_ports(
                graph,
                holder.node,
                hyper_loop,
                side,
                PortSideArea::Middle,
                AddMode::Append,
            );
        }
    }
}

fn compute_port_list_split_index(
    hyper_loop: &SelfHyperLoop,
    sorted_ports: &[SelfLoopPort],
) -> usize {
    if sorted_ports.is_empty() {
        return 0;
    }

    let positive_net_flow_index = sorted_ports
        .iter()
        .position(|port| self_loop_port_net_flow(hyper_loop, port.port) > 0)
        .unwrap_or(sorted_ports.len());
    if positive_net_flow_index > 0 && positive_net_flow_index < sorted_ports.len() - 1 {
        return positive_net_flow_index;
    }

    let non_negative_net_flow_index = sorted_ports
        .iter()
        .position(|port| self_loop_port_net_flow(hyper_loop, port.port) > 0)
        .unwrap_or(sorted_ports.len());
    if non_negative_net_flow_index > 0 && non_negative_net_flow_index < sorted_ports.len() - 1 {
        return non_negative_net_flow_index;
    }

    sorted_ports.len() / 2
}

fn three_side_restore_sides(
    graph: &LGraph,
    node: usize,
    hyper_loop: &SelfHyperLoop,
) -> Option<(PortSide, PortSide, PortSide)> {
    let sides = loop_sides(graph, node, hyper_loop);
    if sides.len() != 3 {
        return None;
    }

    if !sides.contains(&PortSide::North) {
        Some((PortSide::East, PortSide::South, PortSide::West))
    } else if !sides.contains(&PortSide::East) {
        Some((PortSide::South, PortSide::West, PortSide::North))
    } else if !sides.contains(&PortSide::South) {
        Some((PortSide::West, PortSide::North, PortSide::East))
    } else if !sides.contains(&PortSide::West) {
        Some((PortSide::North, PortSide::East, PortSide::South))
    } else {
        None
    }
}

fn restored_port_order(
    graph: &LGraph,
    node: usize,
    target_areas: &PortRestoreTargetAreas,
) -> Vec<usize> {
    let hidden_ports = target_areas.ports();
    let old_ports = (0..graph.layerless_nodes[node].ports.len())
        .filter(|port| !hidden_ports.contains(port))
        .collect::<Vec<_>>();
    let mut next_old_port_index = 0usize;
    let mut new_order = Vec::with_capacity(old_ports.len());

    add_target_area(
        target_areas,
        PortSide::North,
        PortSideArea::Start,
        &mut new_order,
    );
    next_old_port_index = add_all_that(
        graph,
        node,
        &old_ports,
        next_old_port_index,
        &mut new_order,
        |port| {
            port.side == PortSide::North
                && is_north_south_port_with_west_or_west_east_connections(graph, port)
        },
    );
    add_target_area(
        target_areas,
        PortSide::North,
        PortSideArea::Middle,
        &mut new_order,
    );
    next_old_port_index = add_all_that(
        graph,
        node,
        &old_ports,
        next_old_port_index,
        &mut new_order,
        |port| port.side == PortSide::North,
    );
    add_target_area(
        target_areas,
        PortSide::North,
        PortSideArea::End,
        &mut new_order,
    );

    add_target_area(
        target_areas,
        PortSide::East,
        PortSideArea::Start,
        &mut new_order,
    );
    add_target_area(
        target_areas,
        PortSide::East,
        PortSideArea::Middle,
        &mut new_order,
    );
    next_old_port_index = add_all_that(
        graph,
        node,
        &old_ports,
        next_old_port_index,
        &mut new_order,
        |port| port.side == PortSide::East,
    );
    add_target_area(
        target_areas,
        PortSide::East,
        PortSideArea::End,
        &mut new_order,
    );

    add_target_area(
        target_areas,
        PortSide::South,
        PortSideArea::Start,
        &mut new_order,
    );
    next_old_port_index = add_all_that(
        graph,
        node,
        &old_ports,
        next_old_port_index,
        &mut new_order,
        |port| {
            port.side == PortSide::South && is_north_south_port_with_east_connections(graph, port)
        },
    );
    add_target_area(
        target_areas,
        PortSide::South,
        PortSideArea::Middle,
        &mut new_order,
    );
    next_old_port_index = add_all_that(
        graph,
        node,
        &old_ports,
        next_old_port_index,
        &mut new_order,
        |port| port.side == PortSide::South,
    );
    add_target_area(
        target_areas,
        PortSide::South,
        PortSideArea::End,
        &mut new_order,
    );

    add_target_area(
        target_areas,
        PortSide::West,
        PortSideArea::Start,
        &mut new_order,
    );
    let _ = add_all_that(
        graph,
        node,
        &old_ports,
        next_old_port_index,
        &mut new_order,
        |port| port.side == PortSide::West,
    );
    add_target_area(
        target_areas,
        PortSide::West,
        PortSideArea::Middle,
        &mut new_order,
    );
    add_target_area(
        target_areas,
        PortSide::West,
        PortSideArea::End,
        &mut new_order,
    );

    for old_port in old_ports {
        if !new_order.contains(&old_port) {
            new_order.push(old_port);
        }
    }

    new_order
}

fn add_target_area(
    target_areas: &PortRestoreTargetAreas,
    side: PortSide,
    area: PortSideArea,
    new_order: &mut Vec<usize>,
) {
    for port in target_areas.get(side, area) {
        if !new_order.contains(port) {
            new_order.push(*port);
        }
    }
}

fn add_all_that(
    graph: &LGraph,
    node: usize,
    old_ports: &[usize],
    from_index: usize,
    new_order: &mut Vec<usize>,
    condition: impl Fn(&crate::graph::LPort) -> bool,
) -> usize {
    for (index, old_port) in old_ports.iter().enumerate().skip(from_index) {
        let port = &graph.layerless_nodes[node].ports[*old_port];
        if !condition(port) {
            return index;
        }
        if !new_order.contains(old_port) {
            new_order.push(*old_port);
        }
    }

    old_ports.len()
}

fn is_north_south_port_with_west_or_west_east_connections(
    graph: &LGraph,
    port: &crate::graph::LPort,
) -> bool {
    let connections = north_south_port_connection_sides(graph, port);
    connections.contains(&PortSide::West)
}

fn is_north_south_port_with_east_connections(graph: &LGraph, port: &crate::graph::LPort) -> bool {
    north_south_port_connection_sides(graph, port).contains(&PortSide::East)
}

fn north_south_port_connection_sides(graph: &LGraph, port: &crate::graph::LPort) -> Vec<PortSide> {
    let Some(dummy_ref) = port.port_dummy.as_ref() else {
        return Vec::new();
    };
    let Some(dummy_graph) = graph_by_id(graph, dummy_ref.graph_id.as_str()) else {
        return Vec::new();
    };
    let Some(dummy_node) = dummy_graph.layerless_nodes.get(dummy_ref.node) else {
        return Vec::new();
    };

    let mut sides = dummy_node
        .ports
        .iter()
        .filter(|_| {
            dummy_node.origin_port.as_ref().is_some_and(|origin| {
                origin.port.node == port.node && origin.port.port == port_index(graph, port)
            })
        })
        .filter(|dummy_port| {
            !dummy_port.incoming_edges.is_empty() || !dummy_port.outgoing_edges.is_empty()
        })
        .map(|dummy_port| dummy_port.side)
        .collect::<Vec<_>>();
    sides.sort_by_key(|side| side.ordinal());
    sides.dedup();
    sides
}

fn port_index(graph: &LGraph, port: &crate::graph::LPort) -> usize {
    graph
        .layerless_nodes
        .get(port.node)
        .and_then(|node| {
            node.ports
                .iter()
                .position(|candidate| std::ptr::eq(candidate, port))
        })
        .unwrap_or(usize::MAX)
}

fn graph_by_id<'a>(graph: &'a LGraph, id: &str) -> Option<&'a LGraph> {
    if graph.id == id {
        return Some(graph);
    }

    graph
        .layerless_nodes
        .iter()
        .filter_map(|node| node.nested_graph.as_deref())
        .find_map(|nested| graph_by_id(nested, id))
}

fn remap_holder_ports(holder: &mut SelfLoopHolder, old_to_new: &[usize]) {
    for hyper_loop in &mut holder.hyper_loops {
        for port in &mut hyper_loop.ports {
            port.port = old_to_new[port.port];
        }
        for edge in &mut hyper_loop.edges {
            edge.source_port = old_to_new[edge.source_port];
            edge.target_port = old_to_new[edge.target_port];
        }
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

fn place_self_loop_labels(graph: &mut LGraph, holder: &mut SelfLoopHolder) {
    assign_label_side_and_alignment(graph, holder);
    for hyper_loop in &mut holder.hyper_loops {
        compute_label_coordinates(graph, holder.node, hyper_loop);
    }
}

fn assign_label_side_and_alignment(graph: &LGraph, holder: &mut SelfLoopHolder) {
    for hyper_loop in &mut holder.hyper_loops {
        if hyper_loop.labels.is_none() {
            continue;
        }

        let Some(loop_type) = hyper_loop.self_loop_type else {
            continue;
        };

        match loop_type {
            SelfLoopType::OneSide => {
                let Some(loop_side) = hyper_loop.occupied_sides.first().copied().or_else(|| {
                    hyper_loop
                        .ports
                        .first()
                        .map(|port| graph.layerless_nodes[holder.node].ports[port.port].side)
                }) else {
                    continue;
                };
                assign_one_sided_label_side_and_alignment(
                    graph,
                    holder.node,
                    hyper_loop,
                    loop_side,
                );
            }
            SelfLoopType::TwoSidesCorner => {
                assign_two_sides_corner_label_side_and_alignment(graph, holder.node, hyper_loop);
            }
            SelfLoopType::TwoSidesOpposing | SelfLoopType::ThreeSides => {
                assign_two_sides_opposing_and_three_sides_label_side_and_alignment(hyper_loop);
            }
            SelfLoopType::FourSides => {
                assign_four_sides_label_side_and_alignment(graph, holder.node, hyper_loop);
            }
        }
    }
}

fn assign_one_sided_label_side_and_alignment(
    graph: &LGraph,
    node: usize,
    hyper_loop: &mut SelfHyperLoop,
    loop_side: PortSide,
) {
    match loop_side {
        PortSide::East | PortSide::West => {
            let mut topmost_port = hyper_loop.leftmost_port;
            if let (Some(left), Some(right)) = (hyper_loop.leftmost_port, hyper_loop.rightmost_port)
                && graph.layerless_nodes[node].ports[right].position.y
                    < graph.layerless_nodes[node].ports[left].position.y
            {
                topmost_port = Some(right);
            }
            assign_label_side_and_alignment_to_loop(
                hyper_loop,
                loop_side,
                SelfLoopLabelAlignment::Top,
                topmost_port,
            );
        }
        PortSide::North | PortSide::South => {
            assign_label_side_and_alignment_to_loop(
                hyper_loop,
                loop_side,
                SelfLoopLabelAlignment::Center,
                None,
            );
        }
        PortSide::Undefined => {}
    }
}

fn assign_two_sides_corner_label_side_and_alignment(
    graph: &LGraph,
    node: usize,
    hyper_loop: &mut SelfHyperLoop,
) {
    let (Some(leftmost_port), Some(rightmost_port)) =
        (hyper_loop.leftmost_port, hyper_loop.rightmost_port)
    else {
        return;
    };
    let leftmost_side = graph.layerless_nodes[node].ports[leftmost_port].side;
    let rightmost_side = graph.layerless_nodes[node].ports[rightmost_port].side;

    if leftmost_side == PortSide::North {
        assign_label_side_and_alignment_to_loop(
            hyper_loop,
            PortSide::North,
            SelfLoopLabelAlignment::Left,
            Some(leftmost_port),
        );
    } else if rightmost_side == PortSide::North {
        assign_label_side_and_alignment_to_loop(
            hyper_loop,
            PortSide::North,
            SelfLoopLabelAlignment::Right,
            Some(rightmost_port),
        );
    } else if leftmost_side == PortSide::South {
        assign_label_side_and_alignment_to_loop(
            hyper_loop,
            PortSide::South,
            SelfLoopLabelAlignment::Right,
            Some(leftmost_port),
        );
    } else if rightmost_side == PortSide::South {
        assign_label_side_and_alignment_to_loop(
            hyper_loop,
            PortSide::South,
            SelfLoopLabelAlignment::Left,
            Some(rightmost_port),
        );
    }
}

fn assign_two_sides_opposing_and_three_sides_label_side_and_alignment(
    hyper_loop: &mut SelfHyperLoop,
) {
    if !hyper_loop.occupied_sides.contains(&PortSide::North) {
        assign_label_side_and_alignment_to_loop(
            hyper_loop,
            PortSide::South,
            SelfLoopLabelAlignment::Center,
            None,
        );
    } else if !hyper_loop.occupied_sides.contains(&PortSide::South) {
        assign_label_side_and_alignment_to_loop(
            hyper_loop,
            PortSide::North,
            SelfLoopLabelAlignment::Center,
            None,
        );
    } else if !hyper_loop.occupied_sides.contains(&PortSide::West) {
        assign_label_side_and_alignment_to_loop(
            hyper_loop,
            PortSide::North,
            SelfLoopLabelAlignment::Left,
            hyper_loop.leftmost_port,
        );
    } else if !hyper_loop.occupied_sides.contains(&PortSide::East) {
        assign_label_side_and_alignment_to_loop(
            hyper_loop,
            PortSide::North,
            SelfLoopLabelAlignment::Right,
            hyper_loop.rightmost_port,
        );
    }
}

fn assign_four_sides_label_side_and_alignment(
    graph: &LGraph,
    node: usize,
    hyper_loop: &mut SelfHyperLoop,
) {
    let (Some(leftmost_port), Some(rightmost_port)) =
        (hyper_loop.leftmost_port, hyper_loop.rightmost_port)
    else {
        return;
    };
    let leftmost_side = graph.layerless_nodes[node].ports[leftmost_port].side;
    let rightmost_side = graph.layerless_nodes[node].ports[rightmost_port].side;

    if leftmost_side == PortSide::North || rightmost_side == PortSide::North {
        assign_label_side_and_alignment_to_loop(
            hyper_loop,
            PortSide::South,
            SelfLoopLabelAlignment::Center,
            None,
        );
    } else {
        assign_label_side_and_alignment_to_loop(
            hyper_loop,
            PortSide::North,
            SelfLoopLabelAlignment::Center,
            None,
        );
    }
}

fn assign_label_side_and_alignment_to_loop(
    hyper_loop: &mut SelfHyperLoop,
    side: PortSide,
    alignment: SelfLoopLabelAlignment,
    alignment_reference_port: Option<usize>,
) {
    if let Some(labels) = &mut hyper_loop.labels {
        labels.side = Some(side);
        labels.alignment = Some(alignment);
        labels.alignment_reference_port = alignment_reference_port;
    }
}

fn compute_label_coordinates(graph: &LGraph, node: usize, hyper_loop: &mut SelfHyperLoop) {
    let Some(labels) = &mut hyper_loop.labels else {
        return;
    };
    let Some(alignment) = labels.alignment else {
        return;
    };

    match alignment {
        SelfLoopLabelAlignment::Center => {
            labels.position.x = (graph.layerless_nodes[node].size.width - labels.size.width) / 2.0;
        }
        SelfLoopLabelAlignment::Left => {
            if let Some(port) = labels.alignment_reference_port {
                let port_data = &graph.layerless_nodes[node].ports[port];
                labels.position.x = port_data.position.x + port_data.anchor.x;
            }
        }
        SelfLoopLabelAlignment::Right => {
            if let Some(port) = labels.alignment_reference_port {
                let port_data = &graph.layerless_nodes[node].ports[port];
                labels.position.x = port_data.position.x + port_data.anchor.x - labels.size.width;
            }
        }
        SelfLoopLabelAlignment::Top => {
            if let Some(port) = labels.alignment_reference_port {
                let port_data = &graph.layerless_nodes[node].ports[port];
                labels.position.y = port_data.position.y + port_data.anchor.y;
            }
        }
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

fn assign_routing_slots(graph: &mut LGraph, holder: &mut SelfLoopHolder) {
    let label_crossing_matrix = compute_self_loop_label_crossing_matrix(holder);
    let activity_over_ports = compute_loop_activity(graph, holder);
    let mut slot_graph =
        create_self_loop_crossing_graph(holder, &activity_over_ports, &label_crossing_matrix);

    break_non_critical_cycles(&mut slot_graph, &mut graph.random);
    assign_raw_routing_slots_to_segments(&mut slot_graph);
    assign_raw_routing_slots_to_loops(holder, &slot_graph);
    shift_slots_towards_node(graph, holder, &activity_over_ports, &label_crossing_matrix);
}

fn compute_self_loop_label_crossing_matrix(holder: &mut SelfLoopHolder) -> Vec<Vec<bool>> {
    let mut label_id = 0usize;
    for hyper_loop in &mut holder.hyper_loops {
        if let Some(labels) = &mut hyper_loop.labels {
            labels.id = Some(label_id);
            label_id += 1;
        }
    }

    let mut matrix = vec![vec![false; label_id]; label_id];
    for first in 0..holder.hyper_loops.len().saturating_sub(1) {
        for second in (first + 1)..holder.hyper_loops.len() {
            if self_loop_labels_overlap(&holder.hyper_loops[first], &holder.hyper_loops[second])
                && let (Some(first_id), Some(second_id)) = (
                    holder.hyper_loops[first]
                        .labels
                        .as_ref()
                        .and_then(|labels| labels.id),
                    holder.hyper_loops[second]
                        .labels
                        .as_ref()
                        .and_then(|labels| labels.id),
                )
            {
                matrix[first_id][second_id] = true;
                matrix[second_id][first_id] = true;
            }
        }
    }

    matrix
}

fn self_loop_labels_overlap(first: &SelfHyperLoop, second: &SelfHyperLoop) -> bool {
    let (Some(first_labels), Some(second_labels)) = (&first.labels, &second.labels) else {
        return false;
    };
    let (Some(first_side), Some(second_side)) = (first_labels.side, second_labels.side) else {
        return false;
    };
    if first_side != second_side || matches!(first_side, PortSide::East | PortSide::West) {
        return false;
    }

    let first_start = first_labels.position.x;
    let first_end = first_start + first_labels.size.width;
    let second_start = second_labels.position.x;
    let second_end = second_start + second_labels.size.width;

    first_start <= second_end && first_end >= second_start
}

fn self_loop_label_ids(
    holder: &SelfLoopHolder,
    first: usize,
    second: usize,
) -> Option<(usize, usize)> {
    Some((
        holder.hyper_loops.get(first)?.labels.as_ref()?.id?,
        holder.hyper_loops.get(second)?.labels.as_ref()?.id?,
    ))
}

fn self_loop_label_matrix_overlaps(
    holder: &SelfLoopHolder,
    label_crossing_matrix: &[Vec<bool>],
    first: usize,
    second: usize,
) -> bool {
    let Some((first_id, second_id)) = self_loop_label_ids(holder, first, second) else {
        return false;
    };
    label_crossing_matrix
        .get(first_id)
        .and_then(|row| row.get(second_id))
        .copied()
        .unwrap_or(false)
}

fn create_self_loop_crossing_graph(
    holder: &SelfLoopHolder,
    activity_over_ports: &[Vec<bool>],
    label_crossing_matrix: &[Vec<bool>],
) -> HyperEdgeGraph {
    let mut graph = HyperEdgeGraph::default();
    for _ in &holder.hyper_loops {
        graph.add_segment(HyperEdgeSegment::new());
    }

    for first in 0..holder.hyper_loops.len().saturating_sub(1) {
        for second in (first + 1)..holder.hyper_loops.len() {
            create_self_loop_slot_dependencies(
                holder,
                activity_over_ports,
                label_crossing_matrix,
                &mut graph,
                first,
                second,
            );
        }
    }

    graph
}

fn compute_loop_activity(graph: &LGraph, holder: &SelfLoopHolder) -> Vec<Vec<bool>> {
    let port_count = graph
        .layerless_nodes
        .get(holder.node)
        .map(|node| node.ports.len())
        .unwrap_or(0);
    compute_loop_activity_for_port_count(holder, port_count)
}

fn compute_loop_activity_for_port_count(
    holder: &SelfLoopHolder,
    port_count: usize,
) -> Vec<Vec<bool>> {
    holder
        .hyper_loops
        .iter()
        .map(|hyper_loop| {
            let mut activity = vec![false; port_count];
            let (Some(leftmost_port), Some(rightmost_port)) =
                (hyper_loop.leftmost_port, hyper_loop.rightmost_port)
            else {
                return activity;
            };
            if port_count == 0 {
                return activity;
            }

            let mut port = leftmost_port as isize - 1;
            while port != rightmost_port as isize {
                port = (port + 1).rem_euclid(port_count as isize);
                activity[port as usize] = true;
            }
            activity
        })
        .collect()
}

fn create_self_loop_slot_dependencies(
    holder: &SelfLoopHolder,
    activity_over_ports: &[Vec<bool>],
    label_crossing_matrix: &[Vec<bool>],
    graph: &mut HyperEdgeGraph,
    first: usize,
    second: usize,
) {
    let first_above_second =
        count_self_loop_slot_crossings(&holder.hyper_loops[first], &activity_over_ports[second]);
    let second_above_first =
        count_self_loop_slot_crossings(&holder.hyper_loops[second], &activity_over_ports[first]);

    if first_above_second < second_above_first {
        graph.add_regular_dependency(
            first,
            second,
            (second_above_first - first_above_second) as i32,
        );
    } else if second_above_first < first_above_second {
        graph.add_regular_dependency(
            second,
            first,
            (first_above_second - second_above_first) as i32,
        );
    } else if first_above_second != 0
        || self_loop_label_matrix_overlaps(holder, label_crossing_matrix, first, second)
    {
        graph.add_regular_dependency(first, second, 0);
        graph.add_regular_dependency(second, first, 0);
    }
}

fn count_self_loop_slot_crossings(
    upper_loop: &SelfHyperLoop,
    lower_loop_activity: &[bool],
) -> usize {
    upper_loop
        .ports
        .iter()
        .filter(|port| lower_loop_activity.get(port.port).copied().unwrap_or(false))
        .count()
}

fn assign_raw_routing_slots_to_segments(graph: &mut HyperEdgeGraph) {
    let mut sinks = VecDeque::new();

    for segment in 0..graph.segments.len() {
        graph.segments[segment].in_dep_weight = graph.segments[segment]
            .incoming_segment_dependencies
            .iter()
            .filter(|dependency| graph.dependencies[**dependency].source.is_some())
            .count() as i32;
        graph.segments[segment].out_dep_weight = graph.segments[segment]
            .outgoing_segment_dependencies
            .iter()
            .filter(|dependency| graph.dependencies[**dependency].target.is_some())
            .count() as i32;

        if graph.segments[segment].out_dep_weight == 0 {
            graph.segments[segment].routing_slot = 0;
            sinks.push_back(segment);
        }
    }

    while let Some(segment) = sinks.pop_front() {
        let next_routing_slot = graph.segments[segment].routing_slot + 1;
        for dependency in graph.segments[segment]
            .incoming_segment_dependencies
            .clone()
        {
            let Some(source) = graph.dependencies[dependency].source else {
                continue;
            };
            graph.segments[source].routing_slot =
                graph.segments[source].routing_slot.max(next_routing_slot);
            graph.segments[source].out_dep_weight -= 1;
            if graph.segments[source].out_dep_weight == 0 {
                sinks.push_back(source);
            }
        }
    }
}

fn assign_raw_routing_slots_to_loops(holder: &mut SelfLoopHolder, graph: &HyperEdgeGraph) {
    for (loop_index, hyper_loop) in holder.hyper_loops.iter_mut().enumerate() {
        let slot = graph.segments[loop_index].routing_slot.max(0) as usize;
        for side in hyper_loop.occupied_sides.clone() {
            hyper_loop.routing_slots[side.ordinal()] = slot;
        }
    }
}

fn shift_slots_towards_node(
    graph: &LGraph,
    holder: &mut SelfLoopHolder,
    activity_over_ports: &[Vec<bool>],
    label_crossing_matrix: &[Vec<bool>],
) {
    let port_count = activity_over_ports.iter().map(Vec::len).max().unwrap_or(0);
    let mut next_free_routing_slot_at_port = vec![0usize; port_count];

    for side in [
        PortSide::North,
        PortSide::East,
        PortSide::South,
        PortSide::West,
    ] {
        shift_slots_towards_node_on_side(
            graph,
            holder,
            activity_over_ports,
            side,
            &mut next_free_routing_slot_at_port,
            label_crossing_matrix,
        );
    }
}

fn shift_slots_towards_node_on_side(
    graph: &LGraph,
    holder: &mut SelfLoopHolder,
    activity_over_ports: &[Vec<bool>],
    side: PortSide,
    next_free_routing_slot_at_port: &mut [usize],
    label_crossing_matrix: &[Vec<bool>],
) {
    let mut loops = holder
        .hyper_loops
        .iter()
        .enumerate()
        .filter(|(_, hyper_loop)| hyper_loop.occupied_sides.contains(&side))
        .map(|(index, hyper_loop)| (index, hyper_loop.routing_slots[side.ordinal()]))
        .collect::<Vec<_>>();
    loops.sort_by_key(|(_, slot)| *slot);

    let Some((min_port, max_port)) = port_range_on_side(graph, holder.node, side) else {
        for (slot, (loop_index, _)) in loops.into_iter().enumerate() {
            holder.hyper_loops[loop_index].routing_slots[side.ordinal()] = slot;
        }
        return;
    };

    let mut slot_assigned_to_label = vec![None; label_crossing_matrix.len()];
    for (loop_index, _) in loops {
        let active_at_port = &activity_over_ports[loop_index];
        let mut lowest_available_slot = 0usize;
        for (port, next_free_slot) in next_free_routing_slot_at_port
            .iter()
            .enumerate()
            .take(max_port + 1)
            .skip(min_port)
        {
            if active_at_port.get(port).copied().unwrap_or(false) {
                lowest_available_slot = lowest_available_slot.max(*next_free_slot);
            }
        }

        if let Some(label_id) = holder.hyper_loops[loop_index]
            .labels
            .as_ref()
            .and_then(|labels| labels.id)
        {
            while label_crossing_matrix.get(label_id).is_some_and(|row| {
                row.iter().enumerate().any(|(other_label_id, overlaps)| {
                    *overlaps
                        && slot_assigned_to_label
                            .get(other_label_id)
                            .copied()
                            .flatten()
                            == Some(lowest_available_slot)
                })
            }) {
                lowest_available_slot += 1;
            }
        }

        holder.hyper_loops[loop_index].routing_slots[side.ordinal()] = lowest_available_slot;
        for (port, next_free_slot) in next_free_routing_slot_at_port
            .iter_mut()
            .enumerate()
            .take(max_port + 1)
            .skip(min_port)
        {
            if active_at_port.get(port).copied().unwrap_or(false) {
                *next_free_slot = lowest_available_slot + 1;
            }
        }
        if let Some(label_id) = holder.hyper_loops[loop_index]
            .labels
            .as_ref()
            .and_then(|labels| labels.id)
            && let Some(slot) = slot_assigned_to_label.get_mut(label_id)
        {
            *slot = Some(lowest_available_slot);
        }
    }
}

fn port_range_on_side(graph: &LGraph, node: usize, side: PortSide) -> Option<(usize, usize)> {
    graph
        .layerless_nodes
        .get(node)?
        .ports
        .iter()
        .enumerate()
        .filter_map(|(port, port_data)| (port_data.side == side).then_some(port))
        .fold(None, |range, port| match range {
            Some((min, max)) => Some((min.min(port), max.max(port))),
            None => Some((port, port)),
        })
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
        if let Some(mut labels) = hyper_loop.labels.clone() {
            place_labels_on_routing_slot(graph, hyper_loop, &mut labels, &routing_slot_positions);
            update_node_margins_for_label(graph, holder.node, &mut new_margins, &labels);
            write_self_loop_label_positions(graph, &labels);
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

    let mut positions: [Vec<f64>; 5] = std::array::from_fn(|side_index| {
        let Some(_side) = side_from_ordinal(side_index) else {
            return Vec::new();
        };

        Vec::with_capacity(slot_count[side_index])
    });

    initialize_routing_slot_positions_with_max_label_height(
        &mut positions,
        holder,
        PortSide::North,
    );
    initialize_routing_slot_positions_with_max_label_height(
        &mut positions,
        holder,
        PortSide::South,
    );

    for side in [
        PortSide::North,
        PortSide::East,
        PortSide::South,
        PortSide::West,
    ] {
        let side_index = side.ordinal();
        let mut side_positions = std::mem::take(&mut positions[side_index]);
        let mut current = compute_baseline_position(graph, holder.node, side);
        let factor = if matches!(side, PortSide::North | PortSide::West) {
            -1.0
        } else {
            1.0
        };

        for slot in 0..slot_count[side_index] {
            let mut largest_label_size = side_positions.get(slot).copied().unwrap_or(0.0);
            if largest_label_size > 0.0 {
                largest_label_size += graph.options.spacing.edge_label;
            }
            if slot < side_positions.len() {
                side_positions[slot] = current;
            } else {
                side_positions.push(current);
            }
            current += factor * (largest_label_size + graph.options.spacing.edge_edge);
        }

        positions[side_index] = side_positions;
    }

    positions
}

fn initialize_routing_slot_positions_with_max_label_height(
    positions: &mut [Vec<f64>; 5],
    holder: &SelfLoopHolder,
    side: PortSide,
) {
    for hyper_loop in &holder.hyper_loops {
        let Some(labels) = &hyper_loop.labels else {
            continue;
        };
        if labels.side != Some(side) {
            continue;
        }
        let slot = hyper_loop.routing_slots[side.ordinal()];
        let side_positions = &mut positions[side.ordinal()];
        while side_positions.len() <= slot {
            side_positions.push(0.0);
        }
        side_positions[slot] = side_positions[slot].max(labels.size.height);
    }
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
    let node_self_loop_distance = graph.options.spacing.node_self_loop;
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

fn place_labels_on_routing_slot(
    graph: &LGraph,
    hyper_loop: &SelfHyperLoop,
    labels: &mut SelfHyperLoopLabels,
    routing_slot_positions: &[Vec<f64>; 5],
) {
    let Some(side) = labels.side else {
        return;
    };
    let slot = hyper_loop.routing_slots[side.ordinal()];
    let Some(mut label_position) = routing_slot_positions[side.ordinal()].get(slot).copied() else {
        return;
    };

    match side {
        PortSide::North => {
            label_position -= graph.options.spacing.edge_label + labels.size.height;
            labels.position.y = label_position;
        }
        PortSide::South => {
            label_position += graph.options.spacing.edge_label;
            labels.position.y = label_position;
        }
        PortSide::West => {
            label_position -= graph.options.spacing.edge_label + labels.size.width;
            labels.position.x = label_position;
        }
        PortSide::East => {
            label_position += graph.options.spacing.edge_label;
            labels.position.x = label_position;
        }
        PortSide::Undefined => {}
    }
}

fn update_node_margins_for_label(
    graph: &LGraph,
    node: usize,
    margins: &mut LMargin,
    labels: &SelfHyperLoopLabels,
) {
    update_node_margins_for_bend_point(graph, node, margins, labels.position);
    update_node_margins_for_bend_point(
        graph,
        node,
        margins,
        LPoint {
            x: labels.position.x + labels.size.width,
            y: labels.position.y + labels.size.height,
        },
    );
}

fn write_self_loop_label_positions(graph: &mut LGraph, labels: &SelfHyperLoopLabels) {
    if graph.options.direction.is_vertical() {
        write_self_loop_label_positions_for_vertical_layout(graph, labels);
    } else {
        write_self_loop_label_positions_for_horizontal_layout(graph, labels);
    }
}

fn write_self_loop_label_positions_for_horizontal_layout(
    graph: &mut LGraph,
    labels: &SelfHyperLoopLabels,
) {
    let mut y = labels.position.y;
    for label_ref in &labels.label_refs {
        let Some(label) = graph
            .edges
            .get_mut(label_ref.edge)
            .and_then(|edge| edge.labels.get_mut(label_ref.label))
        else {
            continue;
        };

        label.position.x = if labels.alignment == Some(SelfLoopLabelAlignment::Left)
            || labels.side == Some(PortSide::East)
        {
            labels.position.x
        } else if labels.alignment == Some(SelfLoopLabelAlignment::Right)
            || labels.side == Some(PortSide::West)
        {
            labels.position.x + labels.size.width - label.size.width
        } else {
            labels.position.x + (labels.size.width - label.size.width) / 2.0
        };
        label.position.y = y;
        y += label.size.height + graph.options.spacing.label_label;
    }
}

fn write_self_loop_label_positions_for_vertical_layout(
    graph: &mut LGraph,
    labels: &SelfHyperLoopLabels,
) {
    let mut x = labels.position.x;
    for label_ref in &labels.label_refs {
        let Some(label) = graph
            .edges
            .get_mut(label_ref.edge)
            .and_then(|edge| edge.labels.get_mut(label_ref.label))
        else {
            continue;
        };

        label.position.x = x;
        label.position.y = if labels.side == Some(PortSide::North) {
            labels.position.y + labels.size.height - label.size.height
        } else {
            labels.position.y
        };
        x += label.size.width + graph.options.spacing.label_label;
    }
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

fn offset_self_loop_label_positions(
    graph: &mut LGraph,
    labels: &SelfHyperLoopLabels,
    offset: LPoint,
) {
    for label_ref in &labels.label_refs {
        if let Some(label) = graph
            .edges
            .get_mut(label_ref.edge)
            .and_then(|edge| edge.labels.get_mut(label_ref.label))
        {
            label.position.x += offset.x;
            label.position.y += offset.y;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{LLabel, LNode, LayeredEdge, PortType};
    use crate::importer::{ElkInputEdge, ElkInputGraph, ElkInputNode, import_graph};
    use crate::options::{
        ElkDirection, LayeredOptions, PortConstraints, SelfLoopDistributionStrategy,
        SelfLoopOrderingStrategy,
    };

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
            labels: None,
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
    fn restore_target_areas_match_java_prepend_and_append_ordering() {
        let mut areas = PortRestoreTargetAreas::new();

        areas.add_port_list(
            vec![1, 2],
            PortSide::North,
            PortSideArea::Middle,
            AddMode::Append,
        );
        areas.add_port_list(
            vec![3, 4],
            PortSide::North,
            PortSideArea::Middle,
            AddMode::Prepend,
        );

        assert_eq!(
            areas.get(PortSide::North, PortSideArea::Middle),
            &[4, 3, 2, 1]
        );
    }

    #[test]
    fn restore_hidden_one_side_ports_respects_sequenced_ordering() {
        let mut graph = LGraph::new(
            "root",
            LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        );
        graph.options.self_loop_distribution = SelfLoopDistributionStrategy::North;
        graph.options.self_loop_ordering = SelfLoopOrderingStrategy::Sequenced;

        let node = add_node(&mut graph, "A");
        let source = add_port(&mut graph, node, PortSide::North);
        let target = add_port(&mut graph, node, PortSide::North);
        graph.add_edge(self_loop_edge("loop", "A", source, target));

        preprocess_self_loops(&mut graph);
        restore_self_loop_ports(&mut graph);

        let order = graph.layerless_nodes[node]
            .ports
            .iter()
            .map(|port| port.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(order, vec!["A:1", "A:0"]);
    }

    #[test]
    fn restore_hidden_one_side_ports_respects_reverse_stacked_ordering() {
        let mut graph = LGraph::new(
            "root",
            LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        );
        graph.options.self_loop_distribution = SelfLoopDistributionStrategy::North;
        graph.options.self_loop_ordering = SelfLoopOrderingStrategy::ReverseStacked;

        let node = add_node(&mut graph, "A");
        let p0 = add_port(&mut graph, node, PortSide::North);
        let p1 = add_port(&mut graph, node, PortSide::North);
        let p2 = add_port(&mut graph, node, PortSide::North);
        let p3 = add_port(&mut graph, node, PortSide::North);
        graph.add_edge(self_loop_edge("loop-1", "A", p0, p1));
        graph.add_edge(self_loop_edge("loop-2", "A", p2, p3));

        preprocess_self_loops(&mut graph);
        restore_self_loop_ports(&mut graph);

        let order = graph.layerless_nodes[node]
            .ports
            .iter()
            .map(|port| port.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(order, vec!["A:0", "A:2", "A:3", "A:1"]);
    }

    #[test]
    fn loop_activity_includes_zero_leftmost_port() {
        let hyper_loop = hyper_loop_with_ports(
            &[PortRef { node: 0, port: 0 }, PortRef { node: 0, port: 2 }],
            SelfLoopType::OneSide,
        );
        let mut holder = empty_self_loop_holder(0, hyper_loop);
        holder.hyper_loops[0].leftmost_port = Some(0);
        holder.hyper_loops[0].rightmost_port = Some(2);

        let activity = compute_loop_activity_for_port_count(&holder, 4);

        assert_eq!(activity[0], vec![true, true, true, false]);
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
            graph.layerless_nodes[node].ports[graph.edges[0].source.port].side,
            PortSide::South
        );
        assert_eq!(
            graph.layerless_nodes[node].ports[graph.edges[1].source.port].side,
            PortSide::North
        );
        assert_eq!(
            graph.layerless_nodes[node].ports[graph.edges[1].target.port].side,
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

    #[test]
    fn routing_slot_assigner_uses_crossing_graph_dependencies() {
        let mut graph = LGraph::new(
            "root",
            LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        );
        let node = add_node(&mut graph, "A");
        let p0 = add_port(&mut graph, node, PortSide::North);
        let p1 = add_port(&mut graph, node, PortSide::North);
        let p2 = add_port(&mut graph, node, PortSide::North);
        let p3 = add_port(&mut graph, node, PortSide::North);

        let mut outer = hyper_loop_with_ports(&[p0, p3], SelfLoopType::OneSide);
        outer.leftmost_port = Some(p0.port);
        outer.rightmost_port = Some(p3.port);
        outer.occupied_sides = vec![PortSide::North];
        let mut inner = hyper_loop_with_ports(&[p1, p2], SelfLoopType::OneSide);
        inner.leftmost_port = Some(p1.port);
        inner.rightmost_port = Some(p2.port);
        inner.occupied_sides = vec![PortSide::North];
        let mut holder = SelfLoopHolder {
            node,
            hyper_loops: vec![outer, inner],
            ports_hidden: false,
            original_port_constraints: PortConstraints::FixedSide,
        };

        assign_routing_slots(&mut graph, &mut holder);

        assert_eq!(
            holder.hyper_loops[0].routing_slots[PortSide::North.ordinal()],
            1
        );
        assert_eq!(
            holder.hyper_loops[1].routing_slots[PortSide::North.ordinal()],
            0
        );
    }

    #[test]
    fn routing_slot_assigner_separates_overlapping_self_loop_labels() {
        let mut graph = LGraph::new(
            "root",
            LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        );
        let node = add_node(&mut graph, "A");
        let p0 = add_port(&mut graph, node, PortSide::North);
        let p1 = add_port(&mut graph, node, PortSide::North);
        let p2 = add_port(&mut graph, node, PortSide::North);
        let p3 = add_port(&mut graph, node, PortSide::North);

        let mut left = hyper_loop_with_ports(&[p0, p1], SelfLoopType::OneSide);
        left.leftmost_port = Some(p0.port);
        left.rightmost_port = Some(p1.port);
        left.occupied_sides = vec![PortSide::North];
        left.labels = Some(SelfHyperLoopLabels {
            id: None,
            label_refs: Vec::new(),
            size: LSize {
                width: 90.0,
                height: 12.0,
            },
            position: LPoint { x: 0.0, y: 0.0 },
            side: Some(PortSide::North),
            alignment: Some(SelfLoopLabelAlignment::Center),
            alignment_reference_port: None,
        });

        let mut right = hyper_loop_with_ports(&[p2, p3], SelfLoopType::OneSide);
        right.leftmost_port = Some(p2.port);
        right.rightmost_port = Some(p3.port);
        right.occupied_sides = vec![PortSide::North];
        right.labels = Some(SelfHyperLoopLabels {
            id: None,
            label_refs: Vec::new(),
            size: LSize {
                width: 90.0,
                height: 12.0,
            },
            position: LPoint { x: 10.0, y: 0.0 },
            side: Some(PortSide::North),
            alignment: Some(SelfLoopLabelAlignment::Center),
            alignment_reference_port: None,
        });

        let mut holder = SelfLoopHolder {
            node,
            hyper_loops: vec![left, right],
            ports_hidden: false,
            original_port_constraints: PortConstraints::FixedSide,
        };

        assign_routing_slots(&mut graph, &mut holder);

        assert_ne!(
            holder.hyper_loops[0].routing_slots[PortSide::North.ordinal()],
            holder.hyper_loops[1].routing_slots[PortSide::North.ordinal()]
        );
    }

    #[test]
    fn route_self_loops_writes_label_positions_and_expands_margins() {
        let mut graph = LGraph::new(
            "root",
            LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        );
        let node = add_node(&mut graph, "A");
        let source = add_port(&mut graph, node, PortSide::North);
        let target = add_port(&mut graph, node, PortSide::North);
        let mut edge = self_loop_edge("loop", "A", source, target);
        edge.labels.push(LLabel::new("loop label", 60.0, 12.0));
        graph.add_edge(edge);

        preprocess_self_loops(&mut graph);
        restore_self_loop_ports(&mut graph);
        route_self_loops(&mut graph);

        let label = &graph.edges[0].labels[0];
        assert_eq!(label.position.x, 10.0);
        assert_eq!(label.position.y, -36.0);
        assert!(graph.layerless_nodes[node].margin.top >= 36.0);
    }
}
