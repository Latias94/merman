//! Phase 3 ordering processors.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/PortSideProcessor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/PortListSorter.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/SortByInputModelProcessor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/preserveorder/ModelOrderNodeComparator.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/preserveorder/ModelOrderPortComparator.java

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use crate::graph::{LGraph, LNodeKind, PortRef, PortSide};
use crate::options::{OrderingStrategy, PortConstraints, PortSortingStrategy};

pub mod counting;
pub mod sweep;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SweepCopy {
    pub node_order: Vec<Vec<usize>>,
    pub port_orders: Vec<Vec<Vec<String>>>,
}

impl SweepCopy {
    pub fn new(graph: &LGraph, node_order: &[Vec<usize>]) -> Self {
        Self {
            node_order: node_order.to_vec(),
            port_orders: node_order
                .iter()
                .map(|layer| {
                    layer
                        .iter()
                        .map(|node| {
                            graph.layerless_nodes[*node]
                                .ports
                                .iter()
                                .map(|port| port.id.clone())
                                .collect()
                        })
                        .collect()
                })
                .collect(),
        }
    }

    pub fn transfer_node_and_port_orders_to_graph(
        &self,
        graph: &mut LGraph,
        set_port_constraints: bool,
    ) -> bool {
        if self.node_order.len() != graph.layers.len() {
            return false;
        }

        for (layer_index, layer_order) in self.node_order.iter().enumerate() {
            if layer_order.len() != graph.layers[layer_index].nodes.len() {
                return false;
            }
        }

        for (layer_index, layer_order) in self.node_order.iter().enumerate() {
            graph.layers[layer_index].nodes = layer_order.clone();
            for (position, node) in layer_order.iter().copied().enumerate() {
                graph.layerless_nodes[node].layer_index = Some(layer_index);
                let Some(port_order) = self
                    .port_orders
                    .get(layer_index)
                    .and_then(|layer| layer.get(position))
                else {
                    return false;
                };
                let Some(port_order) = port_order_indices_by_id(graph, node, port_order) else {
                    return false;
                };
                if !graph.reorder_node_ports(node, port_order) {
                    return false;
                }
                if set_port_constraints
                    && !graph.layerless_nodes[node]
                        .port_constraints
                        .is_order_fixed()
                {
                    graph.layerless_nodes[node].port_constraints = PortConstraints::FixedOrder;
                }
            }
        }

        true
    }
}

fn port_order_indices_by_id(
    graph: &LGraph,
    node: usize,
    port_ids: &[String],
) -> Option<Vec<usize>> {
    let ports = graph.layerless_nodes.get(node)?.ports.as_slice();
    if ports.len() != port_ids.len() {
        return None;
    }

    let mut order = Vec::with_capacity(port_ids.len());
    let mut used = vec![false; ports.len()];
    for port_id in port_ids {
        let index = ports
            .iter()
            .enumerate()
            .find_map(|(index, port)| (!used[index] && port.id == *port_id).then_some(index))?;
        used[index] = true;
        order.push(index);
    }
    Some(order)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphInfoHolder {
    pub current_node_order: Vec<Vec<usize>>,
    pub currently_best_node_and_port_order: Option<SweepCopy>,
    pub best_node_and_port_order: Option<SweepCopy>,
}

impl GraphInfoHolder {
    pub fn new(graph: &LGraph) -> Self {
        Self {
            current_node_order: graph
                .layers
                .iter()
                .map(|layer| layer.nodes.clone())
                .collect(),
            currently_best_node_and_port_order: None,
            best_node_and_port_order: None,
        }
    }

    pub fn set_currently_best_node_and_port_order(&mut self, graph: &LGraph) {
        self.currently_best_node_and_port_order =
            Some(SweepCopy::new(graph, &self.current_node_order));
    }

    pub fn set_best_node_and_port_order(&mut self, copy: SweepCopy) {
        self.best_node_and_port_order = Some(copy);
    }

    pub fn get_best_sweep(&self) -> Option<&SweepCopy> {
        self.best_node_and_port_order
            .as_ref()
            .or(self.currently_best_node_and_port_order.as_ref())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SortableF64(f64);

impl Eq for SortableF64 {}

impl Ord for SortableF64 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl PartialOrd for SortableF64 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub fn process_port_sides(graph: &mut LGraph) {
    let layerless_nodes = (0..graph.layerless_nodes.len()).collect::<Vec<_>>();
    for node in layerless_nodes {
        process_node_port_sides(graph, node);
    }

    let layered_nodes = graph
        .layers
        .iter()
        .flat_map(|layer| layer.nodes.iter().copied())
        .collect::<Vec<_>>();
    for node in layered_nodes {
        process_node_port_sides(graph, node);
    }
}

fn process_node_port_sides(graph: &mut LGraph, node: usize) {
    let side_fixed = graph.layerless_nodes[node].port_constraints.is_side_fixed();
    let port_count = graph.layerless_nodes[node].ports.len();

    for port in 0..port_count {
        if !side_fixed || graph.layerless_nodes[node].ports[port].side == PortSide::Undefined {
            set_port_side(graph, PortRef { node, port });
        }
    }

    if !side_fixed {
        graph.layerless_nodes[node].port_constraints = PortConstraints::FixedSide;
    }
}

pub fn set_port_side(graph: &mut LGraph, port: PortRef) {
    let side = if graph.layerless_nodes[port.node].ports[port.port].net_flow() < 0 {
        PortSide::East
    } else {
        PortSide::West
    };
    graph.layerless_nodes[port.node].ports[port.port].set_side(side);
}

pub fn sort_port_lists(graph: &mut LGraph) {
    let node_indices = graph
        .layers
        .iter()
        .flat_map(|layer| layer.nodes.iter().copied())
        .collect::<Vec<_>>();

    for node in node_indices {
        let constraints = graph.layerless_nodes[node].port_constraints;
        if constraints.is_order_fixed() {
            let mut order = (0..graph.layerless_nodes[node].ports.len()).collect::<Vec<_>>();
            order.sort_by(|left, right| {
                compare_combined_ports(graph, node, *left, *right, constraints)
            });
            graph.reorder_node_ports(node, order);
        } else if constraints.is_side_fixed() {
            let keys = (0..graph.layerless_nodes[node].ports.len())
                .map(|port| {
                    (
                        port_side_order(graph.layerless_nodes[node].ports[port].side),
                        port,
                    )
                })
                .collect::<Vec<_>>();
            reorder_by_keys(graph, node, keys);
            reverse_west_and_south_side(graph, node);

            if graph.options.port_sorting_strategy == PortSortingStrategy::PortDegree {
                let keys = (0..graph.layerless_nodes[node].ports.len())
                    .map(|port| (port_degree_east_west_key(graph, node, port), port))
                    .collect::<Vec<_>>();
                reorder_by_keys(graph, node, keys);
            }
        }
    }
}

pub fn sort_by_input_model(graph: &mut LGraph) {
    let mut layer_index = 0usize;
    while layer_index < graph.layers.len() {
        let previous_layer_index = if layer_index == 0 { 0 } else { layer_index - 1 };
        let previous_layer = graph.layers[previous_layer_index].nodes.clone();
        let layer_nodes = graph.layers[layer_index].nodes.clone();

        for node in layer_nodes {
            let constraints = graph.layerless_nodes[node].port_constraints;
            if constraints != PortConstraints::FixedOrder
                && constraints != PortConstraints::FixedPos
            {
                let target_orders = long_edge_target_node_preprocessing(graph, node);
                let port_order =
                    sorted_ports_by_model_order(graph, node, &previous_layer, &target_orders);
                graph.reorder_node_ports(node, port_order);
            }
        }

        let node_order =
            sorted_nodes_by_model_order(graph, &graph.layers[layer_index].nodes, &previous_layer);
        graph.layers[layer_index].nodes = node_order;
        layer_index += 1;
    }
}

fn reorder_by_keys<K: Ord>(graph: &mut LGraph, node: usize, mut keys: Vec<(K, usize)>) {
    keys.sort_by(|left, right| left.0.cmp(&right.0));
    graph.reorder_node_ports(node, keys.into_iter().map(|(_, port)| port));
}

fn compare_combined_ports(
    graph: &LGraph,
    node: usize,
    first: usize,
    second: usize,
    constraints: PortConstraints,
) -> Ordering {
    let first_port = &graph.layerless_nodes[node].ports[first];
    let second_port = &graph.layerless_nodes[node].ports[second];
    let side_order = port_side_order(first_port.side).cmp(&port_side_order(second_port.side));
    if side_order != Ordering::Equal || !constraints.is_order_fixed() {
        return side_order;
    }

    if constraints == PortConstraints::FixedOrder
        && let (Some(first_index), Some(second_index)) =
            (first_port.port_index, second_port.port_index)
    {
        let index_order = first_index.cmp(&second_index);
        if index_order != Ordering::Equal {
            return index_order;
        }
    }

    compare_fixed_pos(first_port.side, first_port.position, second_port.position)
}

fn compare_fixed_pos(
    side: PortSide,
    first: crate::graph::LPoint,
    second: crate::graph::LPoint,
) -> Ordering {
    match side {
        PortSide::North => SortableF64(first.x).cmp(&SortableF64(second.x)),
        PortSide::East => SortableF64(first.y).cmp(&SortableF64(second.y)),
        PortSide::South => SortableF64(second.x).cmp(&SortableF64(first.x)),
        PortSide::West => SortableF64(second.y).cmp(&SortableF64(first.y)),
        PortSide::Undefined => Ordering::Equal,
    }
}

fn reverse_west_and_south_side(graph: &mut LGraph, node: usize) {
    reverse_side_range(graph, node, PortSide::South);
    reverse_side_range(graph, node, PortSide::West);
}

fn reverse_side_range(graph: &mut LGraph, node: usize, side: PortSide) {
    let ports = &graph.layerless_nodes[node].ports;
    let Some(low) = ports.iter().position(|port| port.side == side) else {
        return;
    };
    let high = ports
        .iter()
        .enumerate()
        .skip(low)
        .find_map(|(index, port)| (port.side != side).then_some(index))
        .unwrap_or(ports.len());
    if high <= low + 2 {
        return;
    }

    let mut order = (0..ports.len()).collect::<Vec<_>>();
    order[low..high].reverse();
    graph.reorder_node_ports(node, order);
}

fn port_degree_east_west_key(graph: &LGraph, node: usize, port: usize) -> (u8, i64, usize) {
    let port_data = &graph.layerless_nodes[node].ports[port];
    let degree_key = match port_data.side {
        PortSide::East => -(real_out_degree(graph, node, port) as i64),
        PortSide::West => real_in_degree(graph, node, port) as i64,
        _ => 0,
    };
    (port_side_order(port_data.side), degree_key, port)
}

fn real_in_degree(graph: &LGraph, node: usize, port: usize) -> usize {
    graph.layerless_nodes[node].ports[port]
        .incoming_edges
        .iter()
        .filter(|edge| !graph.edges[**edge].reversed)
        .count()
}

fn real_out_degree(graph: &LGraph, node: usize, port: usize) -> usize {
    graph.layerless_nodes[node].ports[port]
        .outgoing_edges
        .iter()
        .filter(|edge| !graph.edges[**edge].reversed)
        .count()
}

fn port_side_order(side: PortSide) -> u8 {
    match side {
        PortSide::Undefined => 0,
        PortSide::North => 1,
        PortSide::East => 2,
        PortSide::South => 3,
        PortSide::West => 4,
    }
}

fn sorted_ports_by_model_order(
    graph: &LGraph,
    node: usize,
    previous_layer: &[usize],
    target_node_model_order: &HashMap<usize, usize>,
) -> Vec<usize> {
    let mut order = (0..graph.layerless_nodes[node].ports.len()).collect::<Vec<_>>();
    let mut comparator = ModelOrderPortComparator::new(
        graph,
        node,
        previous_layer,
        graph.options.consider_model_order_strategy,
        target_node_model_order,
        graph.options.consider_model_order_port_model_order,
    );
    order.sort_by(|left, right| comparator.compare(*left, *right));
    order
}

fn sorted_nodes_by_model_order(
    graph: &LGraph,
    nodes: &[usize],
    previous_layer: &[usize],
) -> Vec<usize> {
    let mut order = nodes.to_vec();
    let mut comparator = ModelOrderNodeComparator::new(
        graph,
        previous_layer,
        graph.options.consider_model_order_strategy,
    );
    order.sort_by(|left, right| comparator.compare(*left, *right));
    order
}

pub fn long_edge_target_node_preprocessing(graph: &LGraph, node: usize) -> HashMap<usize, usize> {
    let mut target_node_model_order: HashMap<usize, usize> = HashMap::new();

    for port in 0..graph.layerless_nodes[node].ports.len() {
        if graph.layerless_nodes[node].ports[port]
            .outgoing_edges
            .is_empty()
        {
            continue;
        }

        let Some(target_node) = target_node(graph, PortRef { node, port }) else {
            continue;
        };
        let edge_index = graph.layerless_nodes[node].ports[port].outgoing_edges[0];
        let edge = &graph.edges[edge_index];
        if !edge.reversed {
            let edge_order = edge.model_order.unwrap_or(0);
            target_node_model_order
                .entry(target_node)
                .and_modify(|order| *order = (*order).min(edge_order))
                .or_insert(edge_order);
        }
    }

    target_node_model_order
}

pub fn target_node(graph: &LGraph, port: PortRef) -> Option<usize> {
    let mut edge = graph
        .layerless_nodes
        .get(port.node)?
        .ports
        .get(port.port)?
        .outgoing_edges
        .first()
        .copied()?;

    loop {
        let node = graph.edges.get(edge)?.target.node;
        if let Some(target) = graph.layerless_nodes[node]
            .long_edge_target
            .map(|port_ref| port_ref.node)
        {
            return Some(target);
        }

        if graph.layerless_nodes[node].kind == LNodeKind::Normal {
            return Some(node);
        }

        let next_edge = graph.node_outgoing_edges(node).into_iter().next();
        match next_edge {
            Some(next_edge) => edge = next_edge,
            None => return None,
        }
    }
}

struct ModelOrderNodeComparator<'a> {
    graph: &'a LGraph,
    previous_layer: &'a [usize],
    ordering_strategy: OrderingStrategy,
    bigger_than: HashMap<usize, HashSet<usize>>,
    smaller_than: HashMap<usize, HashSet<usize>>,
}

impl<'a> ModelOrderNodeComparator<'a> {
    fn new(
        graph: &'a LGraph,
        previous_layer: &'a [usize],
        ordering_strategy: OrderingStrategy,
    ) -> Self {
        Self {
            graph,
            previous_layer,
            ordering_strategy,
            bigger_than: HashMap::new(),
            smaller_than: HashMap::new(),
        }
    }

    fn compare(&mut self, n1: usize, n2: usize) -> Ordering {
        if let Some(ordering) = self.cached_ordering(n1, n2) {
            return ordering;
        }

        if self.ordering_strategy == OrderingStrategy::PreferEdges
            || self.graph.layerless_nodes[n1].model_order.is_none()
            || self.graph.layerless_nodes[n2].model_order.is_none()
        {
            let p1_source_port = first_previous_layer_source_port(self.graph, n1);
            let p2_source_port = first_previous_layer_source_port(self.graph, n2);

            if let (Some(p1), Some(p2)) = (p1_source_port, p2_source_port) {
                if p1.node == p2.node {
                    for port_index in 0..self.graph.layerless_nodes[p1.node].ports.len() {
                        if port_index == p1.port {
                            self.update_bigger_smaller(n2, n1);
                            return Ordering::Less;
                        }
                        if port_index == p2.port {
                            self.update_bigger_smaller(n1, n2);
                            return Ordering::Greater;
                        }
                    }
                }

                for previous_node in self.previous_layer {
                    if *previous_node == p1.node {
                        self.update_bigger_smaller(n2, n1);
                        return Ordering::Less;
                    }
                    if *previous_node == p2.node {
                        self.update_bigger_smaller(n1, n2);
                        return Ordering::Greater;
                    }
                }
            }

            if self.graph.layerless_nodes[n1].model_order.is_none()
                || self.graph.layerless_nodes[n2].model_order.is_none()
            {
                return self.compare_node_model_orders(
                    n1,
                    n2,
                    self.model_order_from_connected_edges(n1),
                    self.model_order_from_connected_edges(n2),
                );
            }
        }

        self.compare_node_model_orders(
            n1,
            n2,
            self.graph.layerless_nodes[n1].model_order.unwrap_or(0) as i64,
            self.graph.layerless_nodes[n2].model_order.unwrap_or(0) as i64,
        )
    }

    fn compare_node_model_orders(
        &mut self,
        n1: usize,
        n2: usize,
        n1_order: i64,
        n2_order: i64,
    ) -> Ordering {
        if n1_order > n2_order {
            self.update_bigger_smaller(n1, n2);
        } else {
            self.update_bigger_smaller(n2, n1);
        }
        n1_order.cmp(&n2_order)
    }

    fn model_order_from_connected_edges(&self, node: usize) -> i64 {
        for port in &self.graph.layerless_nodes[node].ports {
            if let Some(edge) = port.incoming_edges.first() {
                return self.graph.edges[*edge].model_order.unwrap_or(0) as i64;
            }
        }
        self.graph
            .options
            .consider_model_order_long_edge_strategy
            .return_value()
    }

    fn cached_ordering(&mut self, first: usize, second: usize) -> Option<Ordering> {
        self.ensure_node(first);
        self.ensure_node(second);
        if self.bigger_than[&first].contains(&second) {
            return Some(Ordering::Greater);
        }
        if self.bigger_than[&second].contains(&first) {
            return Some(Ordering::Less);
        }
        if self.smaller_than[&first].contains(&second) {
            return Some(Ordering::Less);
        }
        if self.smaller_than[&second].contains(&first) {
            return Some(Ordering::Greater);
        }
        None
    }

    fn ensure_node(&mut self, node: usize) {
        self.bigger_than.entry(node).or_default();
        self.smaller_than.entry(node).or_default();
    }

    fn update_bigger_smaller(&mut self, bigger: usize, smaller: usize) {
        update_bigger_and_smaller_associations(
            bigger,
            smaller,
            &mut self.bigger_than,
            &mut self.smaller_than,
        );
    }
}

struct ModelOrderPortComparator<'a> {
    graph: &'a LGraph,
    node: usize,
    previous_layer: &'a [usize],
    strategy: OrderingStrategy,
    target_node_model_order: &'a HashMap<usize, usize>,
    port_model_order: bool,
    bigger_than: HashMap<usize, HashSet<usize>>,
    smaller_than: HashMap<usize, HashSet<usize>>,
}

impl<'a> ModelOrderPortComparator<'a> {
    fn new(
        graph: &'a LGraph,
        node: usize,
        previous_layer: &'a [usize],
        strategy: OrderingStrategy,
        target_node_model_order: &'a HashMap<usize, usize>,
        port_model_order: bool,
    ) -> Self {
        Self {
            graph,
            node,
            previous_layer,
            strategy,
            target_node_model_order,
            port_model_order,
            bigger_than: HashMap::new(),
            smaller_than: HashMap::new(),
        }
    }

    fn compare(&mut self, original_p1: usize, original_p2: usize) -> Ordering {
        let mut p1 = original_p1;
        let mut p2 = original_p2;
        let p1_side = self.graph.layerless_nodes[self.node].ports[p1].side;
        let p2_side = self.graph.layerless_nodes[self.node].ports[p2].side;

        if self.port_model_order && p1_side == PortSide::West && p2_side == PortSide::West {
            std::mem::swap(&mut p1, &mut p2);
        }

        if let Some(ordering) = self.cached_ordering(p1, p2) {
            return ordering;
        }

        if self.graph.layerless_nodes[self.node].ports[p1].side
            != self.graph.layerless_nodes[self.node].ports[p2].side
        {
            let result = port_side_order(self.graph.layerless_nodes[self.node].ports[p1].side).cmp(
                &port_side_order(self.graph.layerless_nodes[self.node].ports[p2].side),
            );
            if result == Ordering::Less {
                self.update_bigger_smaller(p2, p1);
            } else {
                self.update_bigger_smaller(p1, p2);
            }
            return result;
        }

        if !self.graph.layerless_nodes[self.node].ports[p1]
            .incoming_edges
            .is_empty()
            && !self.graph.layerless_nodes[self.node].ports[p2]
                .incoming_edges
                .is_empty()
        {
            if self.port_model_order {
                let result = self.check_port_model_order(p1, p2);
                if result != Ordering::Equal {
                    self.update_from_result(p1, p2, result);
                    return result;
                }
            }

            let p1_node = self.incoming_source_node(p1);
            let p2_node = self.incoming_source_node(p2);
            if p1_node == p2_node {
                let p1_order = self.incoming_edge_order(p1);
                let p2_order = self.incoming_edge_order(p2);
                return self.compare_port_orders(p1, p2, p1_order, p2_order);
            }

            for previous_node in self.previous_layer {
                if Some(*previous_node) == p1_node {
                    self.update_bigger_smaller(p1, p2);
                    return Ordering::Greater;
                }
                if Some(*previous_node) == p2_node {
                    self.update_bigger_smaller(p2, p1);
                    return Ordering::Less;
                }
            }
        }

        if !self.graph.layerless_nodes[self.node].ports[p1]
            .outgoing_edges
            .is_empty()
            && !self.graph.layerless_nodes[self.node].ports[p2]
                .outgoing_edges
                .is_empty()
        {
            let p1_target = target_node(
                self.graph,
                PortRef {
                    node: self.node,
                    port: p1,
                },
            );
            let p2_target = target_node(
                self.graph,
                PortRef {
                    node: self.node,
                    port: p2,
                },
            );

            if self.strategy == OrderingStrategy::PreferNodes
                && let (Some(p1_target), Some(p2_target)) = (p1_target, p2_target)
                && let (Some(p1_order), Some(p2_order)) = (
                    self.graph.layerless_nodes[p1_target].model_order,
                    self.graph.layerless_nodes[p2_target].model_order,
                )
            {
                return self.compare_port_orders(p1, p2, p1_order, p2_order);
            }

            if self.port_model_order {
                let result = self.check_port_model_order(p1, p2);
                if result != Ordering::Equal {
                    self.update_from_result(p1, p2, result);
                    return result;
                }
            }

            let mut p1_order = self.outgoing_edge_order(p1);
            let mut p2_order = self.outgoing_edge_order(p2);

            if p1_target.is_some() && p1_target == p2_target {
                let p1_reversed = self.outgoing_edge_reversed(p1);
                let p2_reversed = self.outgoing_edge_reversed(p2);
                if p1_reversed && !p2_reversed {
                    self.update_bigger_smaller(p1, p2);
                    return Ordering::Greater;
                }
                if !p1_reversed && p2_reversed {
                    self.update_bigger_smaller(p2, p1);
                    return Ordering::Less;
                }
                return self.compare_port_orders(p1, p2, p1_order, p2_order);
            }

            if let Some(target) = p1_target
                && let Some(order) = self.target_node_model_order.get(&target)
            {
                p1_order = *order;
            }
            if let Some(target) = p2_target
                && let Some(order) = self.target_node_model_order.get(&target)
            {
                p2_order = *order;
            }
            return self.compare_port_orders(p1, p2, p1_order, p2_order);
        }

        let p1_incoming = !self.graph.layerless_nodes[self.node].ports[p1]
            .incoming_edges
            .is_empty();
        let p1_outgoing = !self.graph.layerless_nodes[self.node].ports[p1]
            .outgoing_edges
            .is_empty();
        let p2_incoming = !self.graph.layerless_nodes[self.node].ports[p2]
            .incoming_edges
            .is_empty();
        let p2_outgoing = !self.graph.layerless_nodes[self.node].ports[p2]
            .outgoing_edges
            .is_empty();

        if p1_incoming && p2_outgoing {
            self.update_bigger_smaller(p1, p2);
            Ordering::Greater
        } else if p1_outgoing && p2_incoming {
            self.update_bigger_smaller(p2, p1);
            Ordering::Less
        } else if let (Some(p1_order), Some(p2_order)) = (
            self.graph.layerless_nodes[self.node].ports[p1].model_order,
            self.graph.layerless_nodes[self.node].ports[p2].model_order,
        ) {
            self.compare_port_orders(p1, p2, p1_order, p2_order)
        } else {
            self.update_bigger_smaller(p2, p1);
            Ordering::Less
        }
    }

    fn check_port_model_order(&self, p1: usize, p2: usize) -> Ordering {
        match (
            self.graph.layerless_nodes[self.node].ports[p1].model_order,
            self.graph.layerless_nodes[self.node].ports[p2].model_order,
        ) {
            (Some(p1_order), Some(p2_order)) => p1_order.cmp(&p2_order),
            _ => Ordering::Equal,
        }
    }

    fn incoming_source_node(&self, port: usize) -> Option<usize> {
        self.graph.layerless_nodes[self.node].ports[port]
            .incoming_edges
            .first()
            .map(|edge| self.graph.edges[*edge].source.node)
    }

    fn incoming_edge_order(&self, port: usize) -> usize {
        self.graph.layerless_nodes[self.node].ports[port]
            .incoming_edges
            .first()
            .and_then(|edge| self.graph.edges[*edge].model_order)
            .unwrap_or(0)
    }

    fn outgoing_edge_order(&self, port: usize) -> usize {
        self.graph.layerless_nodes[self.node].ports[port]
            .outgoing_edges
            .first()
            .and_then(|edge| self.graph.edges[*edge].model_order)
            .unwrap_or(0)
    }

    fn outgoing_edge_reversed(&self, port: usize) -> bool {
        self.graph.layerless_nodes[self.node].ports[port]
            .outgoing_edges
            .first()
            .map(|edge| self.graph.edges[*edge].reversed)
            .unwrap_or(false)
    }

    fn compare_port_orders(
        &mut self,
        p1: usize,
        p2: usize,
        p1_order: usize,
        p2_order: usize,
    ) -> Ordering {
        if p1_order > p2_order {
            self.update_bigger_smaller(p1, p2);
        } else {
            self.update_bigger_smaller(p2, p1);
        }
        p1_order.cmp(&p2_order)
    }

    fn update_from_result(&mut self, p1: usize, p2: usize, result: Ordering) {
        if result == Ordering::Less {
            self.update_bigger_smaller(p2, p1);
        } else if result == Ordering::Greater {
            self.update_bigger_smaller(p1, p2);
        }
    }

    fn cached_ordering(&mut self, first: usize, second: usize) -> Option<Ordering> {
        self.ensure_port(first);
        self.ensure_port(second);
        if self.bigger_than[&first].contains(&second) {
            return Some(Ordering::Greater);
        }
        if self.bigger_than[&second].contains(&first) {
            return Some(Ordering::Less);
        }
        if self.smaller_than[&first].contains(&second) {
            return Some(Ordering::Less);
        }
        if self.smaller_than[&second].contains(&first) {
            return Some(Ordering::Greater);
        }
        None
    }

    fn ensure_port(&mut self, port: usize) {
        self.bigger_than.entry(port).or_default();
        self.smaller_than.entry(port).or_default();
    }

    fn update_bigger_smaller(&mut self, bigger: usize, smaller: usize) {
        update_bigger_and_smaller_associations(
            bigger,
            smaller,
            &mut self.bigger_than,
            &mut self.smaller_than,
        );
    }
}

fn first_previous_layer_source_port(graph: &LGraph, node: usize) -> Option<PortRef> {
    for port in &graph.layerless_nodes[node].ports {
        if let Some(edge) = port.incoming_edges.first() {
            let source = graph.edges[*edge].source;
            if graph.layerless_nodes[source.node].layer_index
                != graph.layerless_nodes[node].layer_index
            {
                return Some(source);
            }
        }
    }
    None
}

fn update_bigger_and_smaller_associations<T>(
    bigger: T,
    smaller: T,
    bigger_than: &mut HashMap<T, HashSet<T>>,
    smaller_than: &mut HashMap<T, HashSet<T>>,
) where
    T: Copy + Eq + std::hash::Hash,
{
    bigger_than.entry(bigger).or_default();
    bigger_than.entry(smaller).or_default();
    smaller_than.entry(bigger).or_default();
    smaller_than.entry(smaller).or_default();

    let smaller_bigger_than = bigger_than[&smaller].clone();
    let bigger_smaller_than = smaller_than[&bigger].clone();

    bigger_than.get_mut(&bigger).unwrap().insert(smaller);
    smaller_than.get_mut(&smaller).unwrap().insert(bigger);

    for very_small in smaller_bigger_than {
        bigger_than.get_mut(&bigger).unwrap().insert(very_small);
        smaller_than.entry(very_small).or_default().insert(bigger);
        let bigger_smaller_than = smaller_than[&bigger].clone();
        smaller_than
            .get_mut(&very_small)
            .unwrap()
            .extend(bigger_smaller_than);
    }

    for very_big in bigger_smaller_than {
        smaller_than.get_mut(&smaller).unwrap().insert(very_big);
        bigger_than.entry(very_big).or_default().insert(smaller);
        let smaller_bigger_than = bigger_than[&smaller].clone();
        bigger_than
            .get_mut(&very_big)
            .unwrap()
            .extend(smaller_bigger_than);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{LPoint, PortType};
    use crate::importer::{ElkInputEdge, ElkInputGraph, ElkInputNode, import_graph};
    use crate::intermediate::split_long_edges;
    use crate::options::{ElkDirection, LayeredOptions};
    use crate::p2layers::layer_network_simplex;

    fn node(id: &str) -> ElkInputNode {
        ElkInputNode {
            id: id.to_string(),
            width: 80.0,
            height: 40.0,
            parent: None,
            direction: None,
            hierarchy_handling: None,
            layer_constraint: None,
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
    fn port_side_processor_assigns_by_net_flow_and_fixes_constraints() {
        let mut graph = graph(vec![node("A"), node("B")], vec![edge("A-B", "A", "B")]);

        process_port_sides(&mut graph);

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
        assert_eq!(
            graph.layerless_nodes[a].port_constraints,
            PortConstraints::FixedSide
        );
        assert_eq!(
            graph.layerless_nodes[b].port_constraints,
            PortConstraints::FixedSide
        );
        assert_eq!(graph.layerless_nodes[a].ports[0].side, PortSide::East);
        assert_eq!(graph.layerless_nodes[b].ports[0].side, PortSide::West);
    }

    #[test]
    fn port_list_sorter_uses_clockwise_order_and_rewrites_edge_refs() {
        let mut graph = graph(vec![node("A"), node("B")], vec![edge("A-B", "A", "B")]);
        let a = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "A")
            .unwrap();
        graph.layerless_nodes[a].port_constraints = PortConstraints::FixedPos;
        let north = graph
            .add_port(
                a,
                PortType::Output,
                PortSide::North,
                LPoint { x: 10.0, y: 0.0 },
            )
            .unwrap();
        let south = graph
            .add_port(
                a,
                PortType::Output,
                PortSide::South,
                LPoint { x: 1.0, y: 0.0 },
            )
            .unwrap();
        graph.layerless_nodes[a].ports[north.port].id = "north".to_string();
        graph.layerless_nodes[a].ports[south.port].id = "south".to_string();
        graph.layerless_nodes[a].ports[0].set_side(PortSide::West);
        graph.layerless_nodes[a].ports[0].position = LPoint { x: 0.0, y: 100.0 };
        graph.set_node_layer(a, 0);

        sort_port_lists(&mut graph);

        assert_eq!(
            graph.layerless_nodes[a]
                .ports
                .iter()
                .map(|port| port.id.as_str())
                .collect::<Vec<_>>(),
            vec!["north", "south", "A:0"]
        );
        assert_eq!(graph.edges[0].source.node, a);
        assert_eq!(
            graph.layerless_nodes[a].ports[graph.edges[0].source.port].id,
            "A:0"
        );
    }

    #[test]
    fn fixed_order_ports_fall_back_to_position_without_both_indices() {
        let mut graph = graph(vec![node("A")], vec![]);
        let a = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "A")
            .unwrap();
        graph.layerless_nodes[a].port_constraints = PortConstraints::FixedOrder;
        let left = graph
            .add_port(
                a,
                PortType::Output,
                PortSide::North,
                LPoint { x: 1.0, y: 0.0 },
            )
            .unwrap();
        let right = graph
            .add_port(
                a,
                PortType::Output,
                PortSide::North,
                LPoint { x: 9.0, y: 0.0 },
            )
            .unwrap();
        graph.layerless_nodes[a].ports[left.port].id = "left".to_string();
        graph.layerless_nodes[a].ports[right.port].id = "right".to_string();
        graph.layerless_nodes[a].ports[left.port].port_index = Some(99);
        graph.set_node_layer(a, 0);

        sort_port_lists(&mut graph);

        assert_eq!(
            graph.layerless_nodes[a]
                .ports
                .iter()
                .map(|port| port.id.as_str())
                .collect::<Vec<_>>(),
            vec!["left", "right"]
        );
    }

    #[test]
    fn sort_by_input_model_orders_layers_by_model_order() {
        let mut graph = graph(
            vec![node("Top"), node("Bottom"), node("Left"), node("Right")],
            vec![
                edge("Top-Right", "Top", "Right"),
                edge("Bottom-Left", "Bottom", "Left"),
            ],
        );
        layer_network_simplex(&mut graph);
        process_port_sides(&mut graph);
        sort_port_lists(&mut graph);

        let left = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "Left")
            .unwrap();
        let right = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "Right")
            .unwrap();
        let target_layer = graph.layerless_nodes[left].layer_index.unwrap();
        graph.layers[target_layer].nodes = vec![right, left];

        sort_by_input_model(&mut graph);

        assert_eq!(graph.layers[target_layer].nodes, vec![left, right]);
    }

    #[test]
    fn long_edge_target_preprocessing_uses_original_target_node() {
        let mut graph = graph(
            vec![node("A"), node("B"), node("C"), node("D")],
            vec![
                edge("A-B", "A", "B"),
                edge("B-C", "B", "C"),
                edge("C-D", "C", "D"),
                edge("A-D", "A", "D"),
            ],
        );
        layer_network_simplex(&mut graph);
        split_long_edges(&mut graph);

        let a = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "A")
            .unwrap();
        let d = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "D")
            .unwrap();
        let orders = long_edge_target_node_preprocessing(&graph, a);

        assert_eq!(orders.get(&d), Some(&3));
    }

    #[test]
    fn sweep_copy_restores_ports_by_stable_id_after_reorder() {
        let mut graph = graph(vec![node("A")], vec![]);
        let a = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "A")
            .unwrap();
        graph.layerless_nodes[a].ports.clear();
        let first = graph
            .add_port(
                a,
                PortType::Output,
                PortSide::East,
                LPoint { x: 0.0, y: 0.0 },
            )
            .unwrap();
        let second = graph
            .add_port(
                a,
                PortType::Output,
                PortSide::East,
                LPoint { x: 0.0, y: 0.0 },
            )
            .unwrap();
        graph.layerless_nodes[a].ports[first.port].id = "first".to_string();
        graph.layerless_nodes[a].ports[second.port].id = "second".to_string();
        graph.set_node_layer(a, 0);

        let copy = SweepCopy::new(&graph, &[vec![a]]);
        graph.reorder_node_ports(a, [1, 0]);

        assert!(copy.transfer_node_and_port_orders_to_graph(&mut graph, false));
        assert_eq!(
            graph.layerless_nodes[a]
                .ports
                .iter()
                .map(|port| port.id.as_str())
                .collect::<Vec<_>>(),
            vec!["first", "second"]
        );
    }
}
