//! Phase 3 ordering processors.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/PortSideProcessor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/PortListSorter.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/SortByInputModelProcessor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/preserveorder/ModelOrderNodeComparator.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/preserveorder/ModelOrderPortComparator.java
//!
//! Model-order sorting and comparators include the ELK 0.12.0 changes:
//! https://github.com/eclipse-elk/elk/tree/323312916048544e4e594eab3303cdb0d01e2929/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/preserveorder

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use crate::graph::{LGraph, LNodeKind, PortRef, PortSide};
use crate::options::{OrderingStrategy, PortConstraints, PortSortingStrategy};
use crate::work::{WorkControl, WorkError, checked_add, checked_mul, checked_sum};

pub mod counting;
#[cfg(test)]
mod feedback_work_tests;
#[cfg(test)]
mod model_order_work_tests;
pub mod sweep;

pub(crate) fn initialize_crossing_minimization_port_ids(graph: &mut LGraph) {
    clear_crossing_minimization_port_ids(graph);
    let order = graph
        .layers
        .iter()
        .map(|layer| layer.nodes.clone())
        .collect::<Vec<_>>();
    assign_crossing_minimization_port_ids(graph, &order);
}

pub(crate) fn initialize_crossing_minimization_port_ids_hierarchy(graph: &mut LGraph) {
    initialize_crossing_minimization_port_ids(graph);
    for node in &mut graph.layerless_nodes {
        if let Some(nested_graph) = node.nested_graph.as_deref_mut() {
            initialize_crossing_minimization_port_ids_hierarchy(nested_graph);
        }
    }
}

fn clear_crossing_minimization_port_ids(graph: &mut LGraph) {
    for node in &mut graph.layerless_nodes {
        for port in &mut node.ports {
            port.crossing_minimization_id = None;
        }
    }
}

fn assign_crossing_minimization_port_ids(graph: &mut LGraph, order: &[Vec<usize>]) {
    let mut next_port_id = 0usize;
    for layer in order {
        for node in layer {
            let Some(node_data) = graph.layerless_nodes.get_mut(*node) else {
                continue;
            };
            for port in &mut node_data.ports {
                port.crossing_minimization_id = Some(next_port_id);
                next_port_id += 1;
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SweepCopy {
    pub node_order: Vec<Vec<usize>>,
    pub port_orders: Vec<Vec<Vec<PortOrderKey>>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortOrderKey {
    crossing_minimization_id: Option<usize>,
    fallback_id: String,
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
                                .map(|port| PortOrderKey {
                                    crossing_minimization_id: port.crossing_minimization_id,
                                    fallback_id: port.id.clone(),
                                })
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
    port_ids: &[PortOrderKey],
) -> Option<Vec<usize>> {
    let ports = graph.layerless_nodes.get(node)?.ports.as_slice();
    if ports.len() != port_ids.len() {
        return None;
    }

    let use_crossing_ids = port_ids
        .iter()
        .any(|key| key.crossing_minimization_id.is_some());
    let use_fallback_ids = port_ids
        .iter()
        .any(|key| key.crossing_minimization_id.is_none());
    let mut by_crossing_id = HashMap::<usize, Vec<usize>>::new();
    let mut by_fallback_id = HashMap::<&str, Vec<usize>>::new();
    // Pop the smallest current index first, preserving the original first-unused match.
    // A port can appear in both namespaces; the shared bitmap makes mixed saved keys safe.
    for (index, port) in ports.iter().enumerate().rev() {
        if use_crossing_ids && let Some(id) = port.crossing_minimization_id {
            by_crossing_id.entry(id).or_default().push(index);
        }
        if use_fallback_ids {
            by_fallback_id
                .entry(port.id.as_str())
                .or_default()
                .push(index);
        }
    }
    let mut order = Vec::with_capacity(port_ids.len());
    let mut used = vec![false; ports.len()];
    for port_key in port_ids {
        let candidates = if let Some(id) = port_key.crossing_minimization_id {
            by_crossing_id.get_mut(&id)?
        } else {
            by_fallback_id.get_mut(port_key.fallback_id.as_str())?
        };
        let index = loop {
            let index = candidates.pop()?;
            if !used[index] {
                break index;
            }
        };
        used[index] = true;
        order.push(index);
    }
    Some(order)
}

fn port_order_lookup_work_units(
    graph: &LGraph,
    node: usize,
    keys: &[PortOrderKey],
) -> Result<usize, WorkError> {
    let ports = &graph.layerless_nodes[node].ports;
    // Two key-kind scans, index construction, saved-key lookup, and two namespaces'
    // enqueue/dequeue visits are all linear. Every queue entry is popped at most once.
    let mut work = checked_mul(ports.len(), 8)?;
    if keys
        .iter()
        .any(|key| key.crossing_minimization_id.is_none())
    {
        // Numeric crossing IDs need no string hashing. Fallback keys additionally read
        // the current IDs once during indexing and the saved IDs once during lookup.
        work = checked_sum([
            work,
            checked_sum(ports.iter().map(|port| port.id.len()))?,
            checked_sum(
                keys.iter()
                    .filter(|key| key.crossing_minimization_id.is_none())
                    .map(|key| key.fallback_id.len()),
            )?,
        ])?;
    }
    Ok(work)
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
    let port_data = &graph.layerless_nodes[port.node].ports[port.port];
    let side = if let Some(dummy) = port_data.port_dummy.as_ref() {
        external_port_dummy_side(graph, port.node, dummy.graph_id.as_str(), dummy.node)
    } else if port_data.net_flow() < 0 {
        PortSide::East
    } else {
        PortSide::West
    };
    if side != PortSide::Undefined {
        graph.layerless_nodes[port.node].ports[port.port].set_side(side);
    }
}

fn external_port_dummy_side(
    graph: &LGraph,
    port_node: usize,
    dummy_graph_id: &str,
    dummy_node: usize,
) -> PortSide {
    if dummy_graph_id == graph.id {
        return graph
            .layerless_nodes
            .get(dummy_node)
            .map(|node| node.external_port_side)
            .unwrap_or(PortSide::Undefined);
    }

    graph
        .layerless_nodes
        .get(port_node)
        .and_then(|node| node.nested_graph.as_deref())
        .filter(|nested| nested.id == dummy_graph_id)
        .and_then(|nested| nested.layerless_nodes.get(dummy_node))
        .map(|node| node.external_port_side)
        .unwrap_or(PortSide::Undefined)
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

pub fn sort_by_input_model(
    graph: &mut LGraph,
    work: &mut dyn WorkControl,
) -> Result<(), WorkError> {
    let mut layer_index = 0usize;
    while layer_index < graph.layers.len() {
        let previous_layer_index = if layer_index == 0 { 0 } else { layer_index - 1 };
        let previous_layer = graph.layers[previous_layer_index].nodes.clone();
        let layer_nodes = sorted_nodes_by_model_order(
            graph,
            &graph.layers[layer_index].nodes,
            &previous_layer,
            true,
            work,
        )?;
        graph.layers[layer_index].nodes = layer_nodes.clone();
        let previous_layer = if layer_index == 0 {
            layer_nodes.clone()
        } else {
            previous_layer
        };

        for node in layer_nodes {
            let constraints = graph.layerless_nodes[node].port_constraints;
            if constraints != PortConstraints::FixedOrder
                && constraints != PortConstraints::FixedPos
            {
                let target_orders = long_edge_target_node_preprocessing(graph, node, work)?;
                let port_order = sorted_ports_by_model_order(
                    graph,
                    node,
                    &previous_layer,
                    &target_orders,
                    work,
                )?;
                graph.reorder_node_ports(node, port_order);
            }
        }

        let node_order = sorted_nodes_by_model_order(
            graph,
            &graph.layers[layer_index].nodes,
            &previous_layer,
            false,
            work,
        )?;
        graph.layers[layer_index].nodes = node_order;
        layer_index += 1;
    }
    Ok(())
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
    if ports.is_empty() {
        return;
    }

    // Match PortListSorter.java:105-143 at ELK 62d5909f and elkjs 0.9.3.
    // The published provider caps high at len - 1 and rereads low while advancing
    // high. South can temporarily include west ports; the subsequent west pass
    // must inspect the reordered list. A conventional side range changes routes.
    let lower_bound = port_side_order(side);
    let upper_bound = lower_bound + 1;
    let mut current_side = port_side_order(ports[0].side);
    let mut low = 0;
    while low < ports.len() - 1 && current_side < lower_bound {
        low += 1;
        current_side = port_side_order(ports[low].side);
    }
    let mut high = low;
    while high < ports.len() - 1 && current_side < upper_bound {
        high += 1;
        current_side = port_side_order(ports[low].side);
    }
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

// elkjs 0.9.3's GWT Arrays sort uses insertion sort below seven elements, then
// alternating merge buffers. Model-order comparators cache relations, including a
// fallback that returns Less for an unseen pair in either direction. Preserve the
// exact comparison schedule for both node and port sorting; Rust's sort_by is not
// interchangeable here. Source: elk-worker.js insertionSort/mergeSort_0/merge_1.
fn sort_model_order(
    order: &mut [usize],
    mut compare: impl FnMut(usize, usize) -> Result<Ordering, WorkError>,
) -> Result<(), WorkError> {
    fn merge_sort(
        source: &mut [usize],
        destination: &mut [usize],
        compare: &mut impl FnMut(usize, usize) -> Result<Ordering, WorkError>,
    ) -> Result<(), WorkError> {
        if destination.len() < 7 {
            for index in 1..destination.len() {
                let mut current = index;
                while current > 0
                    && compare(destination[current - 1], destination[current])? == Ordering::Greater
                {
                    destination.swap(current - 1, current);
                    current -= 1;
                }
            }
            return Ok(());
        }

        let middle = destination.len() / 2;
        let (source_left, source_right) = source.split_at_mut(middle);
        let (destination_left, destination_right) = destination.split_at_mut(middle);
        merge_sort(destination_left, source_left, compare)?;
        merge_sort(destination_right, source_right, compare)?;
        if compare(source[middle - 1], source[middle])? != Ordering::Greater {
            destination.copy_from_slice(source);
            return Ok(());
        }

        let mut left = 0;
        let mut right = middle;
        for item in destination {
            if right >= source.len()
                || (left < middle && compare(source[left], source[right])? != Ordering::Greater)
            {
                *item = source[left];
                left += 1;
            } else {
                *item = source[right];
                right += 1;
            }
        }
        Ok(())
    }

    let mut source = order.to_vec();
    merge_sort(&mut source, order, &mut compare)
}

fn sorted_ports_by_model_order(
    graph: &LGraph,
    node: usize,
    previous_layer: &[usize],
    target_node_model_order: &HashMap<usize, usize>,
    work: &mut dyn WorkControl,
) -> Result<Vec<usize>, WorkError> {
    let mut order = (0..graph.layerless_nodes[node].ports.len()).collect::<Vec<_>>();
    // ELK's comparator records transitive relations while it compares, including equal-order
    // cases. Keep one comparator for this stable sort: key sorting, unstable sorting, or rebuilding
    // the comparator per comparison changes observable Mermaid ordering.
    let mut comparator = ModelOrderPortComparator::new(
        graph,
        node,
        previous_layer,
        graph.options.consider_model_order_strategy,
        target_node_model_order,
        graph.options.consider_model_order_port_model_order,
    );
    sort_model_order(&mut order, |left, right| {
        comparator.compare_with_work(left, right, work)
    })?;
    Ok(order)
}

fn sorted_nodes_by_model_order(
    graph: &LGraph,
    nodes: &[usize],
    previous_layer: &[usize],
    before_ports: bool,
    work: &mut dyn WorkControl,
) -> Result<Vec<usize>, WorkError> {
    let mut order = nodes.to_vec();
    // See sorted_ports_by_model_order: comparison order and shared comparator state are semantic.
    let mut comparator = ModelOrderNodeComparator::new(
        graph,
        previous_layer,
        graph.options.consider_model_order_strategy,
    );
    comparator.before_ports = before_ports;
    for index in 1..order.len() {
        let node = order[index];
        let mut cursor = index;
        while cursor > 0
            && comparator.compare_with_work(order[cursor - 1], node, work)? == Ordering::Greater
        {
            order[cursor] = order[cursor - 1];
            cursor -= 1;
        }
        order[cursor] = node;
    }
    Ok(order)
}

// The comparator updates a transitive relation only when a pair is not already cached. Bound
// both possible update directions using the sets that exist for this comparison, preserving
// the source stable-sort call order without a second sorting pass or an owner-wide square.
fn model_order_relation_work(
    first: usize,
    second: usize,
    bigger: &HashMap<usize, HashSet<usize>>,
    smaller: &HashMap<usize, HashSet<usize>>,
) -> Result<usize, WorkError> {
    if bigger.get(&first).is_some_and(|set| set.contains(&second))
        || bigger.get(&second).is_some_and(|set| set.contains(&first))
        || smaller.get(&first).is_some_and(|set| set.contains(&second))
        || smaller.get(&second).is_some_and(|set| set.contains(&first))
    {
        return Ok(1);
    }
    checked_add(
        1,
        model_order_relation_update_work(first, second, bigger, smaller)?.max(
            model_order_relation_update_work(second, first, bigger, smaller)?,
        ),
    )
}

fn model_order_relation_update_work(
    large: usize,
    small: usize,
    bigger: &HashMap<usize, HashSet<usize>>,
    smaller: &HashMap<usize, HashSet<usize>>,
) -> Result<usize, WorkError> {
    let smaller_relations = bigger.get(&small).map_or(0, HashSet::len);
    let larger_relations = smaller.get(&large).map_or(0, HashSet::len);
    if large == small
        || bigger
            .get(&small)
            .is_some_and(|relations| relations.contains(&large))
        || smaller
            .get(&large)
            .is_some_and(|relations| relations.contains(&small))
    {
        let relation_count = checked_add(smaller_relations, larger_relations)?;
        let set_bound = checked_add(relation_count, 1)?;
        let per_relation = checked_add(2, checked_mul(set_bound, 2)?)?;
        return checked_sum([
            relation_count,
            2,
            checked_mul(relation_count, per_relation)?,
        ]);
    }
    checked_sum([
        smaller_relations,
        larger_relations,
        2,
        checked_mul(
            smaller_relations,
            checked_add(2, checked_mul(larger_relations, 2)?)?,
        )?,
        checked_mul(
            larger_relations,
            checked_add(2, checked_mul(smaller_relations, 2)?)?,
        )?,
    ])
}

pub fn long_edge_target_node_preprocessing(
    graph: &mut LGraph,
    node: usize,
    work: &mut dyn WorkControl,
) -> Result<HashMap<usize, usize>, WorkError> {
    let mut target_node_model_order: HashMap<usize, usize> = HashMap::new();

    for port in &mut graph.layerless_nodes[node].ports {
        port.long_edge_target_node = None;
    }

    for port in 0..graph.layerless_nodes[node].ports.len() {
        if graph.layerless_nodes[node].ports[port]
            .outgoing_edges
            .is_empty()
        {
            continue;
        }

        let Some(target_node) = target_node(graph, PortRef { node, port }, work)? else {
            continue;
        };
        graph.layerless_nodes[node].ports[port].long_edge_target_node = Some(target_node);
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

    Ok(target_node_model_order)
}

struct ScoringPortTargets {
    nodes: Vec<Option<usize>>,
    model_order: HashMap<usize, usize>,
}

fn scoring_long_edge_target_node_orders(
    graph: &LGraph,
    node: usize,
    work: &mut dyn WorkControl,
) -> Result<ScoringPortTargets, WorkError> {
    let mut target_nodes = Vec::with_capacity(graph.layerless_nodes[node].ports.len());
    let mut target_node_model_order: HashMap<usize, usize> = HashMap::new();
    for (port_index, port) in graph.layerless_nodes[node].ports.iter().enumerate() {
        let Some(edge_index) = port.outgoing_edges.first().copied() else {
            target_nodes.push(None);
            continue;
        };
        let target = match port.long_edge_target_node {
            Some(target) => Some(target),
            None => target_node(
                graph,
                PortRef {
                    node,
                    port: port_index,
                },
                work,
            )?,
        };
        target_nodes.push(target);
        let Some(target_node) = target else {
            continue;
        };
        let edge = &graph.edges[edge_index];
        if !edge.reversed {
            let edge_order = edge.model_order.unwrap_or(0);
            target_node_model_order
                .entry(target_node)
                .and_modify(|order| *order = (*order).min(edge_order))
                .or_insert(edge_order);
        }
    }
    Ok(ScoringPortTargets {
        nodes: target_nodes,
        model_order: target_node_model_order,
    })
}

pub fn target_node(
    graph: &LGraph,
    port: PortRef,
    work: &mut dyn WorkControl,
) -> Result<Option<usize>, WorkError> {
    let Some(mut edge) = graph
        .layerless_nodes
        .get(port.node)
        .and_then(|node| node.ports.get(port.port))
        .and_then(|port| port.outgoing_edges.first())
        .copied()
    else {
        return Ok(None);
    };
    let mut remaining_hops = graph.edges.len().saturating_add(1);
    while remaining_hops > 0 {
        work.check(1)?;
        work.charge(1)?;
        remaining_hops -= 1;
        let Some(edge_data) = graph.edges.get(edge) else {
            return Ok(None);
        };
        let node = edge_data.target.node;
        if let Some(target) = graph.layerless_nodes[node]
            .long_edge_target
            .map(|port_ref| port_ref.node)
        {
            return Ok(Some(target));
        }
        if graph.layerless_nodes[node].kind == LNodeKind::Normal {
            return Ok(Some(node));
        }
        let mut next_edge = None;
        for port in &graph.layerless_nodes[node].ports {
            work.check(1)?;
            work.charge(1)?;
            if let Some(next) = port.outgoing_edges.first().copied() {
                next_edge = Some(next);
                break;
            }
        }
        match next_edge {
            Some(next) => edge = next,
            None => return Ok(None),
        }
    }
    Ok(None)
}

pub(super) fn count_model_order_node_changes(
    graph: &LGraph,
    layers: &[Vec<usize>],
    strategy: OrderingStrategy,
    work: &mut dyn WorkControl,
) -> Result<usize, WorkError> {
    let mut previous_layer_index = None;
    let mut wrong_model_order = 0usize;
    for layer in layers {
        work.check(0)?;
        let previous_layer = previous_layer_index
            .and_then(|index| layers.get(index))
            .unwrap_or(&layers[0]);
        let mut comparator = ModelOrderNodeComparator::new(graph, previous_layer, strategy);
        comparator.long_edge_strategy = crate::options::LongEdgeOrderingStrategy::Equal;
        let ordered_nodes = layer
            .iter()
            .copied()
            .filter(|&node| graph.layerless_nodes[node].model_order.is_some())
            .collect::<Vec<_>>();
        for i in 0..ordered_nodes.len() {
            for j in (i + 1)..ordered_nodes.len() {
                if comparator.compare_with_work(ordered_nodes[i], ordered_nodes[j], work)?
                    == Ordering::Greater
                {
                    wrong_model_order += 1;
                }
            }
        }
        previous_layer_index = Some(previous_layer_index.map_or(0, |index| index + 1));
    }
    Ok(wrong_model_order)
}

pub(super) fn count_model_order_port_changes(
    graph: &LGraph,
    layers: &[Vec<usize>],
    work: &mut dyn WorkControl,
) -> Result<usize, WorkError> {
    let mut previous_layer_index = None;
    let mut wrong_model_order = 0usize;
    for layer in layers {
        work.check(0)?;
        let previous_layer = previous_layer_index
            .and_then(|index| layers.get(index))
            .unwrap_or(&layers[0]);
        for node in layer {
            work.check(0)?;
            if graph.layerless_nodes[*node].ports.len() < 2 {
                continue;
            }
            let targets = scoring_long_edge_target_node_orders(graph, *node, work)?;
            let mut comparator = ModelOrderPortComparator::new(
                graph,
                *node,
                previous_layer,
                graph.options.consider_model_order_strategy,
                &targets.model_order,
                graph.options.consider_model_order_port_model_order,
            );
            comparator.target_nodes = Some(&targets.nodes);
            for i in 0..graph.layerless_nodes[*node].ports.len() {
                for j in (i + 1)..graph.layerless_nodes[*node].ports.len() {
                    if comparator.compare_with_work(i, j, work)? == Ordering::Greater {
                        wrong_model_order += 1;
                    }
                }
            }
        }
        previous_layer_index = Some(previous_layer_index.map_or(0, |index| index + 1));
    }
    Ok(wrong_model_order)
}

struct ModelOrderNodeComparator<'a> {
    graph: &'a LGraph,
    previous_layer: &'a [usize],
    ordering_strategy: OrderingStrategy,
    before_ports: bool,
    long_edge_strategy: crate::options::LongEdgeOrderingStrategy,
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
            before_ports: false,
            long_edge_strategy: graph.options.consider_model_order_long_edge_strategy,
            bigger_than: HashMap::new(),
            smaller_than: HashMap::new(),
        }
    }

    fn compare_with_work(
        &mut self,
        mut n1: usize,
        mut n2: usize,
        work: &mut dyn WorkControl,
    ) -> Result<Ordering, WorkError> {
        let mut pending = Vec::new();
        let result = loop {
            let units = checked_sum([
                self.previous_layer.len(),
                self.graph.layerless_nodes[n1].ports.len(),
                self.graph.layerless_nodes[n2].ports.len(),
                first_previous_layer_source_port(self.graph, n1)
                    .map_or(0, |port| self.graph.layerless_nodes[port.node].ports.len()),
                first_incoming_source_port(self.graph, n1)
                    .map_or(0, |port| self.graph.layerless_nodes[port.node].ports.len()),
                first_incoming_source_port(self.graph, n2)
                    .map_or(0, |port| self.graph.layerless_nodes[port.node].ports.len()),
                1,
            ])?;
            work.check(units)?;
            work.charge(units)?;
            if n1 == n2 {
                break Ordering::Equal;
            }
            match self.compare_step(n1, n2, work)? {
                NodeComparison::Order(order) => break order,
                NodeComparison::References(first, second) => {
                    pending.push((n1, n2));
                    n1 = first;
                    n2 = second;
                }
            }
        };
        for (first, second) in pending.into_iter().rev() {
            if result == Ordering::Greater {
                self.update_bigger_smaller(first, second, work)?;
            } else if result == Ordering::Less {
                self.update_bigger_smaller(second, first, work)?;
            }
        }
        Ok(result)
    }

    fn compare_step(
        &mut self,
        n1: usize,
        n2: usize,
        work: &mut dyn WorkControl,
    ) -> Result<NodeComparison, WorkError> {
        if let Some(ordering) = self.cached_ordering(n1, n2) {
            return Ok(NodeComparison::Order(ordering));
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
                            self.update_bigger_smaller(n2, n1, work)?;
                            return Ok(NodeComparison::Order(Ordering::Less));
                        }
                        if port_index == p2.port {
                            self.update_bigger_smaller(n1, n2, work)?;
                            return Ok(NodeComparison::Order(Ordering::Greater));
                        }
                    }
                }

                for previous_node in self.previous_layer {
                    if *previous_node == p1.node {
                        self.update_bigger_smaller(n2, n1, work)?;
                        return Ok(NodeComparison::Order(Ordering::Less));
                    }
                    if *previous_node == p2.node {
                        self.update_bigger_smaller(n1, n2, work)?;
                        return Ok(NodeComparison::Order(Ordering::Greater));
                    }
                }
            }

            if (p1_source_port.is_none() || p2_source_port.is_none())
                && let Some(result) = self.compare_helper_dummies(n1, n2, work)?
            {
                if let NodeComparison::Order(order) = result {
                    if order == Ordering::Greater {
                        self.update_bigger_smaller(n1, n2, work)?;
                    } else if order == Ordering::Less {
                        self.update_bigger_smaller(n2, n1, work)?;
                    }
                }
                return Ok(result);
            }
            if p1_source_port.is_some() != p2_source_port.is_some()
                && (self.graph.layerless_nodes[n1].model_order.is_none()
                    || self.graph.layerless_nodes[n2].model_order.is_none())
            {
                return Ok(NodeComparison::Order(self.compare_node_model_orders(
                    n1,
                    n2,
                    self.model_order_from_connected_edges(n1),
                    self.model_order_from_connected_edges(n2),
                    work,
                )?));
            }
        }

        let order = match (
            self.graph.layerless_nodes[n1].model_order,
            self.graph.layerless_nodes[n2].model_order,
        ) {
            (Some(first), Some(second)) => {
                self.compare_node_model_orders(n1, n2, first as i64, second as i64, work)?
            }
            _ => {
                self.update_bigger_smaller(n2, n1, work)?;
                Ordering::Less
            }
        };
        Ok(NodeComparison::Order(order))
    }

    fn compare_node_model_orders(
        &mut self,
        n1: usize,
        n2: usize,
        n1_order: i64,
        n2_order: i64,
        work: &mut dyn WorkControl,
    ) -> Result<Ordering, WorkError> {
        if n1_order > n2_order {
            self.update_bigger_smaller(n1, n2, work)?;
            Ok(Ordering::Greater)
        } else {
            self.update_bigger_smaller(n2, n1, work)?;
            Ok(Ordering::Less)
        }
    }

    fn model_order_from_connected_edges(&self, node: usize) -> i64 {
        for port in &self.graph.layerless_nodes[node].ports {
            if let Some(edge) = port.incoming_edges.first() {
                return self.graph.edges[*edge].model_order.unwrap_or(0) as i64;
            }
        }
        self.long_edge_strategy.return_value()
    }

    fn compare_helper_dummies(
        &mut self,
        first: usize,
        second: usize,
        work: &mut dyn WorkControl,
    ) -> Result<Option<NodeComparison>, WorkError> {
        let first_dummy = self.graph.layerless_nodes[first].kind == LNodeKind::LongEdge;
        let second_dummy = self.graph.layerless_nodes[second].kind == LNodeKind::LongEdge;
        let endpoints = |node| {
            Some((
                first_incoming_source_port(self.graph, node)?,
                first_outgoing_target_port(self.graph, node)?,
            ))
        };
        if first_dummy && self.graph.layerless_nodes[second].kind == LNodeKind::Normal {
            let Some((source, target)) = endpoints(first) else {
                return Ok(None);
            };
            let layer = self.graph.layerless_nodes[first].layer_index;
            if self.graph.layerless_nodes[source.node].layer_index != layer
                && self.graph.layerless_nodes[target.node].layer_index != layer
            {
                return Ok(None);
            }
            return Ok(Some(if source.node == second || target.node == second {
                self.update_bigger_smaller(first, second, work)?;
                NodeComparison::Order(Ordering::Greater)
            } else {
                NodeComparison::References(source.node, second)
            }));
        }
        if self.graph.layerless_nodes[first].kind == LNodeKind::Normal && second_dummy {
            let Some((source, target)) = endpoints(second) else {
                return Ok(None);
            };
            let layer = self.graph.layerless_nodes[first].layer_index;
            if self.graph.layerless_nodes[source.node].layer_index != layer
                && self.graph.layerless_nodes[target.node].layer_index != layer
            {
                return Ok(None);
            }
            return Ok(Some(if source.node == first || target.node == first {
                self.update_bigger_smaller(second, first, work)?;
                NodeComparison::Order(Ordering::Less)
            } else {
                NodeComparison::References(first, source.node)
            }));
        }
        if first_dummy && second_dummy {
            let Some((first_source, first_target)) = endpoints(first) else {
                return Ok(None);
            };
            let Some((second_source, second_target)) = endpoints(second) else {
                return Ok(None);
            };
            let reference = |node: usize, source: PortRef, target: PortRef| {
                let layer = self.graph.layerless_nodes[node].layer_index;
                if self.graph.layerless_nodes[source.node].layer_index == layer {
                    (source.node, true, false)
                } else if self.graph.layerless_nodes[target.node].layer_index == layer {
                    (target.node, false, true)
                } else {
                    (node, false, false)
                }
            };
            let (first_reference, first_source_feedback, first_target_feedback) =
                reference(first, first_source, first_target);
            let (second_reference, second_source_feedback, second_target_feedback) =
                reference(second, second_source, second_target);
            if first_reference == second_reference {
                if self.before_ports {
                    let result = if first_source_feedback && second_source_feedback {
                        let orders = HashMap::new();
                        let mut comparator = ModelOrderPortComparator::new(
                            self.graph,
                            first_source.node,
                            self.previous_layer,
                            self.ordering_strategy,
                            &orders,
                            second_target_feedback,
                        );
                        comparator.compare_with_work(first_source.port, second_source.port, work)?
                    } else if first_source_feedback && second_target_feedback {
                        Ordering::Greater
                    } else if first_target_feedback && second_source_feedback {
                        Ordering::Less
                    } else {
                        return Ok(None);
                    };
                    if result == Ordering::Greater {
                        self.update_bigger_smaller(second, first, work)?;
                    } else {
                        self.update_bigger_smaller(first, second, work)?;
                    }
                    return Ok(Some(NodeComparison::Order(result)));
                }
                if first_source.node == first_reference
                    && (second_source.node != first_reference
                        || first_source.port <= second_source.port)
                {
                    self.update_bigger_smaller(second, first, work)?;
                    return Ok(Some(NodeComparison::Order(Ordering::Less)));
                }
                if second_source.node == first_reference {
                    self.update_bigger_smaller(first, second, work)?;
                    return Ok(Some(NodeComparison::Order(Ordering::Greater)));
                }
            }
            return Ok(Some(NodeComparison::References(
                first_reference,
                second_reference,
            )));
        }
        Ok(None)
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

    fn update_bigger_smaller(
        &mut self,
        bigger: usize,
        smaller: usize,
        work: &mut dyn WorkControl,
    ) -> Result<(), WorkError> {
        let units = model_order_relation_update_work(
            bigger,
            smaller,
            &self.bigger_than,
            &self.smaller_than,
        )?;
        work.check(units)?;
        work.charge(units)?;
        update_bigger_and_smaller_associations(
            bigger,
            smaller,
            &mut self.bigger_than,
            &mut self.smaller_than,
        );
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum NodeComparison {
    Order(Ordering),
    References(usize, usize),
}

struct ModelOrderPortComparator<'a> {
    graph: &'a LGraph,
    node: usize,
    previous_layer: &'a [usize],
    strategy: OrderingStrategy,
    target_node_model_order: &'a HashMap<usize, usize>,
    target_nodes: Option<&'a [Option<usize>]>,
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
            target_nodes: None,
            port_model_order,
            bigger_than: HashMap::new(),
            smaller_than: HashMap::new(),
        }
    }

    fn compare_with_work(
        &mut self,
        original_p1: usize,
        original_p2: usize,
        work: &mut dyn WorkControl,
    ) -> Result<Ordering, WorkError> {
        let units = model_order_relation_work(
            original_p1,
            original_p2,
            &self.bigger_than,
            &self.smaller_than,
        )?;
        work.check(units)?;
        work.charge(units)?;
        let p1 = original_p1;
        let p2 = original_p2;
        let p1_side = self.graph.layerless_nodes[self.node].ports[p1].side;

        if let Some(ordering) = self.cached_ordering(p1, p2) {
            return Ok(ordering);
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
            return Ok(result);
        }

        let mut reverse = false;
        if !self.graph.layerless_nodes[self.node].ports[p1]
            .incoming_edges
            .is_empty()
            && !self.graph.layerless_nodes[self.node].ports[p2]
                .incoming_edges
                .is_empty()
        {
            reverse = matches!(p1_side, PortSide::West | PortSide::North | PortSide::South);
            let p1_node = self.incoming_source_node(p1);
            let p2_node = self.incoming_source_node(p2);
            if p1_node == p2_node {
                let source1 = self.incoming_source_port(p1).unwrap();
                let source2 = self.incoming_source_port(p2).unwrap();
                return Ok(self.order_ports(p1, p2, source1.port.cmp(&source2.port), reverse));
            }
            if let (Some(source1), Some(source2)) = (p1_node, p2_node) {
                let first = &self.graph.layerless_nodes[source1];
                let second = &self.graph.layerless_nodes[source2];
                if first.kind == LNodeKind::LongEdge
                    && second.kind == LNodeKind::LongEdge
                    && first.layer_index == second.layer_index
                    && first.layer_index == self.graph.layerless_nodes[self.node].layer_index
                    && let Some(layer) = first
                        .layer_index
                        .and_then(|layer| self.graph.layers.get(layer))
                    && let Some(order) =
                        reference_layer_order(&layer.nodes, source1, source2, work)?
                {
                    if p1_side == PortSide::East {
                        reverse = !reverse;
                    }
                    return Ok(self.order_ports(p1, p2, order, reverse));
                }
                if let Some(order) =
                    reference_layer_order(self.previous_layer, source1, source2, work)?
                {
                    return Ok(self.order_ports(p1, p2, order, reverse));
                }
            }
            if self.port_model_order {
                let order = self.check_port_model_order(p1, p2);
                if order != Ordering::Equal {
                    return Ok(self.order_ports(p1, p2, order, reverse));
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
            if matches!(p1_side, PortSide::West | PortSide::South) {
                reverse = !reverse;
            }
            let p1_target = self.target_node(p1);
            let p2_target = self.target_node(p2);

            if self.strategy == OrderingStrategy::PreferNodes
                && let (Some(p1_target), Some(p2_target)) = (p1_target, p2_target)
                && let (Some(p1_order), Some(p2_order)) = (
                    self.graph.layerless_nodes[p1_target].model_order,
                    self.graph.layerless_nodes[p2_target].model_order,
                )
            {
                return Ok(self.order_ports(p1, p2, p1_order.cmp(&p2_order), reverse));
            }

            if self.port_model_order {
                let result = self.check_port_model_order(p1, p2);
                if result != Ordering::Equal {
                    return Ok(self.order_ports(p1, p2, result, reverse));
                }
            }

            let mut p1_order = self.outgoing_edge_order(p1);
            let mut p2_order = self.outgoing_edge_order(p2);

            if p1_target.is_some() && p1_target == p2_target {
                return Ok(self.order_ports(p1, p2, p1_order.cmp(&p2_order), reverse));
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
            return Ok(self.order_ports(p1, p2, p1_order.cmp(&p2_order), reverse));
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

        Ok(if p1_incoming && p2_outgoing {
            let _ = self.order_ports(p1, p2, Ordering::Greater, reverse);
            Ordering::Greater
        } else if p1_outgoing && p2_incoming {
            let _ = self.order_ports(p1, p2, Ordering::Less, reverse);
            Ordering::Less
        } else if let (Some(p1_order), Some(p2_order)) = (
            self.graph.layerless_nodes[self.node].ports[p1].model_order,
            self.graph.layerless_nodes[self.node].ports[p2].model_order,
        ) {
            if matches!(p1_side, PortSide::West | PortSide::South) {
                reverse = !reverse;
            }
            self.order_ports(p1, p2, p1_order.cmp(&p2_order), reverse)
        } else {
            self.order_ports(p1, p2, Ordering::Less, reverse)
        })
    }

    fn target_node(&self, port: usize) -> Option<usize> {
        self.target_nodes.map_or_else(
            || self.graph.layerless_nodes[self.node].ports[port].long_edge_target_node,
            |targets| targets[port],
        )
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
        self.incoming_source_port(port).map(|port| port.node)
    }

    fn incoming_source_port(&self, port: usize) -> Option<PortRef> {
        self.graph.layerless_nodes[self.node].ports[port]
            .incoming_edges
            .first()
            .map(|edge| self.graph.edges[*edge].source)
    }

    fn outgoing_edge_order(&self, port: usize) -> usize {
        self.graph.layerless_nodes[self.node].ports[port]
            .outgoing_edges
            .first()
            .and_then(|edge| self.graph.edges[*edge].model_order)
            .unwrap_or(0)
    }

    fn order_ports(&mut self, p1: usize, p2: usize, order: Ordering, reverse: bool) -> Ordering {
        let order = if order == Ordering::Greater {
            order
        } else {
            Ordering::Less
        };
        let result = if reverse { order.reverse() } else { order };
        if result == Ordering::Less {
            self.update_bigger_smaller(p2, p1);
        } else if result == Ordering::Greater {
            self.update_bigger_smaller(p1, p2);
        }
        result
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
            if graph.layerless_nodes[source.node]
                .layer_index
                .and_then(|layer| layer.checked_add(1))
                == graph.layerless_nodes[node].layer_index
            {
                return Some(source);
            }
        }
    }
    None
}

fn first_incoming_source_port(graph: &LGraph, node: usize) -> Option<PortRef> {
    graph.layerless_nodes[node].ports.iter().find_map(|port| {
        port.incoming_edges
            .first()
            .map(|&edge| graph.edges[edge].source)
    })
}

fn reference_layer_order(
    layer: &[usize],
    first: usize,
    second: usize,
    work: &mut dyn WorkControl,
) -> Result<Option<Ordering>, WorkError> {
    let mut position = 0;
    while position < layer.len() {
        work.check(1)?;
        work.charge(1)?;
        let node = layer[position];
        if node == first {
            return Ok(Some(Ordering::Less));
        } else if node == second {
            return Ok(Some(Ordering::Greater));
        }
        position += 1;
    }
    Ok(None)
}

fn first_outgoing_target_port(graph: &LGraph, node: usize) -> Option<PortRef> {
    graph.layerless_nodes[node].ports.iter().find_map(|port| {
        port.outgoing_edges
            .first()
            .map(|&edge| graph.edges[edge].target)
    })
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
    use crate::graph::{GraphNodeRef, LNode, LNodeKind, LPoint, LayeredEdge, PortType};
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
            labels: Vec::new(),
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

    fn layered_edge(id: &str, source: PortRef, target: PortRef) -> LayeredEdge {
        LayeredEdge {
            id: id.to_string(),
            source,
            target,
            source_node_id: format!("node-{}", source.node),
            target_node_id: format!("node-{}", target.node),
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

    #[test]
    fn model_order_sort_matches_pinned_worker_comparison_schedule() {
        // Extracted insertionSort/mergeSort_0/merge_1 from elkjs 0.9.3, including
        // the seven-element merge boundary and the already-ordered fast path.
        type SortCase = (
            &'static [usize],
            &'static [usize],
            &'static [(usize, usize)],
        );
        let cases: &[SortCase] = &[
            (&[], &[], &[]),
            (&[0], &[0], &[]),
            (&[2, 1, 0], &[0, 1, 2], &[(2, 1), (2, 0), (1, 0)]),
            (
                &[6, 5, 4, 3, 2, 1, 0],
                &[0, 1, 2, 3, 4, 5, 6],
                &[
                    (6, 5),
                    (6, 4),
                    (5, 4),
                    (3, 2),
                    (3, 1),
                    (2, 1),
                    (3, 0),
                    (2, 0),
                    (1, 0),
                    (6, 0),
                    (4, 0),
                    (4, 1),
                    (4, 2),
                    (4, 3),
                ],
            ),
            (
                &[0, 1, 2, 3, 4, 5, 6],
                &[0, 1, 2, 3, 4, 5, 6],
                &[(0, 1), (1, 2), (3, 4), (4, 5), (5, 6), (2, 3)],
            ),
        ];
        for &(input, expected, expected_calls) in cases {
            let mut order = input.to_vec();
            let mut calls = Vec::new();
            sort_model_order(&mut order, |left, right| {
                calls.push((left, right));
                Ok(left.cmp(&right))
            })
            .unwrap();
            assert_eq!(order, expected, "input={input:?}");
            assert_eq!(calls, expected_calls, "input={input:?}");
        }
    }

    #[test]
    fn model_order_sort_preserves_equal_keys_across_recursive_merges() {
        let keys = [2, 1, 2, 0, 1, 0, 2, 1, 2, 0, 1, 0, 2, 1];
        let mut order = (0..keys.len()).collect::<Vec<_>>();
        sort_model_order(&mut order, |left, right| Ok(keys[left].cmp(&keys[right]))).unwrap();
        assert_eq!(order, [3, 5, 9, 11, 1, 4, 7, 10, 13, 0, 2, 6, 8, 12]);
    }

    #[test]
    fn model_order_sort_stops_at_comparator_work_error() {
        let mut order = [2, 1, 0];
        let mut calls = Vec::new();
        let result = sort_model_order(&mut order, |left, right| {
            calls.push((left, right));
            if calls.len() == 2 {
                Err(WorkError::ArithmeticOverflow)
            } else {
                Ok(left.cmp(&right))
            }
        });
        assert_eq!(result, Err(WorkError::ArithmeticOverflow));
        assert_eq!(calls, [(2, 1), (2, 0)]);
    }

    #[test]
    fn model_order_sort_preserves_source_stateful_port_fallback() {
        for length in [2, 6, 7, 14] {
            let mut graph = LGraph::new("root", LayeredOptions::default());
            graph
                .layerless_nodes
                .push(LNode::new("parent", 0.0, 0.0, None));
            for _ in 0..length {
                graph
                    .add_port(0, PortType::Input, PortSide::West, LPoint::default())
                    .unwrap();
            }
            let target_orders = HashMap::new();
            let mut comparator = ModelOrderPortComparator::new(
                &graph,
                0,
                &[],
                OrderingStrategy::NodesAndEdges,
                &target_orders,
                false,
            );
            let mut order = (0..length).collect::<Vec<_>>();
            sort_model_order(&mut order, |left, right| {
                comparator.compare_with_work(left, right, &mut crate::work::NoopWorkControl)
            })
            .unwrap();
            assert_eq!(order, (0..length).collect::<Vec<_>>());
            // The first source comparison records port 0 < port 1. Reversing
            // the first call (as Rust sort_by can do) records the opposite.
            assert!(comparator.smaller_than[&0].contains(&1));
            assert!(comparator.bigger_than[&1].contains(&0));
        }
    }

    #[test]
    fn compound_self_loop_keeps_input_ports_in_source_model_order() {
        let mut parent = node("Active");
        parent.width = 0.0;
        parent.height = 0.0;
        parent.hierarchy_handling = Some(crate::options::HierarchyHandling::IncludeChildren);
        let mut child = node("Idle");
        child.parent = Some("Active".to_string());
        let mut graph = graph(
            vec![parent, child, node("Inactive")],
            vec![
                edge("edge0", "Inactive", "Idle"),
                edge("edge1", "Active", "Active"),
            ],
        );
        crate::pipeline::execute_ported_compound_processors_until(
            &mut graph,
            crate::pipeline::LayeredPhase::P3NodeOrdering,
        )
        .unwrap();
        let loop_edge = graph.edges.iter().find(|edge| edge.id == "edge1").unwrap();
        let parent_index = loop_edge.target.node;
        let parent = &graph.layerless_nodes[parent_index];
        let cross_edge = graph
            .edges
            .iter()
            .find(|edge| edge.id == "edge0" && edge.target.node == parent_index)
            .unwrap();
        assert_eq!(parent.ports[loop_edge.source.port].side, PortSide::East);
        assert_eq!(parent.ports[loop_edge.target.port].side, PortSide::West);
        assert_eq!(parent.ports[cross_edge.target.port].side, PortSide::West);
        assert!(loop_edge.target.port < cross_edge.target.port);
        // Reordering must also keep each child external port's origin linked
        // to its corresponding parent port, including detached self-loop ports.
        let child = parent.nested_graph.as_ref().unwrap();
        for (index, port) in parent.ports.iter().enumerate() {
            let dummy = port.port_dummy.as_ref().unwrap();
            let origin = child.layerless_nodes[dummy.node]
                .origin_port
                .as_ref()
                .unwrap();
            assert_eq!(
                origin.port,
                PortRef {
                    node: parent_index,
                    port: index
                }
            );
        }
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
    fn port_side_processor_uses_external_port_dummy_side_before_net_flow() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        let parent = graph.layerless_nodes.len();
        graph
            .layerless_nodes
            .push(LNode::new("parent", 80.0, 40.0, None));
        let dummy = graph.layerless_nodes.len();
        let mut external = LNode::new("external:parent", 0.0, 0.0, None);
        external.kind = LNodeKind::ExternalPort;
        external.external_port_side = PortSide::North;
        graph.layerless_nodes.push(external);
        let port = graph
            .add_port(
                parent,
                PortType::Output,
                PortSide::Undefined,
                LPoint::default(),
            )
            .unwrap();
        graph.layerless_nodes[parent].ports[port.port].port_dummy = Some(GraphNodeRef {
            graph_id: "root".to_string(),
            node: dummy,
        });

        set_port_side(&mut graph, port);

        assert_eq!(
            graph.layerless_nodes[parent].ports[port.port].side,
            PortSide::North
        );
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
    fn port_list_sorter_matches_pinned_side_ranges_and_preserves_edge_endpoints() {
        use PortSide::{East, North, South, West};

        // Expected permutations come from the pinned elkjs 0.9.3 PortListSorter,
        // including its exclusive last-port bound and sequential south/west passes.
        let cases: &[(&[PortSide], &[usize])] = &[
            (&[], &[]),
            (&[West], &[0]),
            (&[West, West], &[0, 1]),
            (&[West, West, West], &[0, 1, 2]),
            (&[West, West, West, West], &[2, 1, 0, 3]),
            (&[West, West, West, West, West], &[3, 2, 1, 0, 4]),
            (&[South], &[0]),
            (&[South, South], &[0, 1]),
            (&[South, South, South], &[0, 1, 2]),
            (&[South, South, South, South], &[2, 1, 0, 3]),
            (&[South, South, South, South, South], &[3, 2, 1, 0, 4]),
            (&[North, North, East, East, East], &[0, 1, 2, 3, 4]),
            (
                &[North, East, South, South, South, West],
                &[0, 1, 4, 3, 2, 5],
            ),
            (
                &[North, East, South, South, South, West, West],
                &[0, 1, 2, 3, 4, 5, 6],
            ),
            (
                &[North, North, South, South, South, West, West, West, West],
                &[0, 1, 2, 3, 4, 5, 6, 7, 8],
            ),
        ];
        for &(sides, expected) in cases {
            let mut graph = graph(vec![node("A"), node("B")], vec![]);
            graph.layerless_nodes[0].port_constraints = PortConstraints::FixedSide;
            graph.set_node_layer(0, 0);
            for (index, side) in sides.iter().copied().enumerate() {
                let outgoing = index % 2 == 0;
                let local = graph
                    .add_port(
                        0,
                        if outgoing {
                            PortType::Output
                        } else {
                            PortType::Input
                        },
                        side,
                        LPoint { x: 0.0, y: 0.0 },
                    )
                    .unwrap();
                let remote = graph
                    .add_port(
                        1,
                        if outgoing {
                            PortType::Input
                        } else {
                            PortType::Output
                        },
                        side.opposed(),
                        LPoint { x: 0.0, y: 0.0 },
                    )
                    .unwrap();
                graph.layerless_nodes[0].ports[local.port].id = index.to_string();
                let (source, target) = if outgoing {
                    (local, remote)
                } else {
                    (remote, local)
                };
                graph
                    .add_edge(layered_edge(&format!("edge{index}"), source, target))
                    .unwrap();
            }

            sort_port_lists(&mut graph);

            let actual = graph.layerless_nodes[0]
                .ports
                .iter()
                .map(|port| port.id.parse::<usize>().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "source range behavior for {sides:?}");
            for (position, &original) in expected.iter().enumerate() {
                let edge = &graph.edges[original];
                let (local, remote) = if original % 2 == 0 {
                    (edge.source, edge.target)
                } else {
                    (edge.target, edge.source)
                };
                assert_eq!(
                    local,
                    PortRef {
                        node: 0,
                        port: position
                    }
                );
                assert_eq!(
                    remote,
                    PortRef {
                        node: 1,
                        port: original
                    }
                );
                let port = &graph.layerless_nodes[0].ports[position];
                if original % 2 == 0 {
                    assert_eq!(port.outgoing_edges, vec![original]);
                    assert!(port.incoming_edges.is_empty());
                } else {
                    assert_eq!(port.incoming_edges, vec![original]);
                    assert!(port.outgoing_edges.is_empty());
                }
                assert!(graph.edge_source_attached(original));
                assert!(graph.edge_target_attached(original));
            }
        }
    }

    #[test]
    fn west_range_is_recomputed_after_south_temporarily_crosses_sides() {
        let mut graph = graph(vec![node("A")], vec![]);
        for (index, side) in [
            PortSide::North,
            PortSide::East,
            PortSide::South,
            PortSide::South,
            PortSide::South,
            PortSide::West,
            PortSide::West,
        ]
        .into_iter()
        .enumerate()
        {
            let port = graph
                .add_port(0, PortType::Input, side, LPoint { x: 0.0, y: 0.0 })
                .unwrap();
            graph.layerless_nodes[0].ports[port.port].id = index.to_string();
        }
        reverse_side_range(&mut graph, 0, PortSide::South);
        let after_south = graph.layerless_nodes[0]
            .ports
            .iter()
            .map(|port| port.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(after_south, vec!["0", "1", "5", "4", "3", "2", "6"]);

        reverse_side_range(&mut graph, 0, PortSide::West);
        let after_west = graph.layerless_nodes[0]
            .ports
            .iter()
            .map(|port| port.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(after_west, vec!["0", "1", "2", "3", "4", "5", "6"]);
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
    fn model_order_relation_work_tracks_current_sets_and_cached_pairs() {
        let mut bigger = HashMap::new();
        let mut smaller = HashMap::new();
        assert_eq!(model_order_relation_work(0, 1, &bigger, &smaller), Ok(3));
        bigger.insert(0, HashSet::from([1]));
        smaller.insert(1, HashSet::from([0]));
        assert_eq!(model_order_relation_work(0, 1, &bigger, &smaller), Ok(1));
        assert_eq!(model_order_relation_work(1, 0, &bigger, &smaller), Ok(1));
        // Unrelated owners cannot inflate the cost of comparing this pair.
        bigger.insert(99, (100..200).collect());
        assert_eq!(model_order_relation_work(2, 3, &bigger, &smaller), Ok(3));
    }

    #[test]
    fn model_order_relation_admission_keeps_one_sided_updates_linear() {
        let mut bigger = HashMap::new();
        let mut smaller = HashMap::new();
        for owner in 0..500 {
            update_bigger_and_smaller_associations(owner, owner + 1, &mut bigger, &mut smaller);
        }
        let units = model_order_relation_update_work(501, 0, &bigger, &smaller).unwrap();
        assert!(
            units < 2_000,
            "one-sided update was charged as dense work: {units}"
        );
        update_bigger_and_smaller_associations(501, 0, &mut bigger, &mut smaller);
        assert_eq!(bigger[&501].len(), 501);
        assert_eq!(smaller[&0].len(), 1);
    }

    #[test]
    fn feedback_reference_comparisons_charge_each_unwound_relation() {
        struct RemainingWork(usize);

        impl WorkControl for RemainingWork {
            fn check(&mut self, units: usize) -> Result<(), WorkError> {
                if units > self.0 {
                    Err(WorkError::Interrupted)
                } else {
                    Ok(())
                }
            }

            fn charge(&mut self, units: usize) -> Result<(), WorkError> {
                self.check(units)?;
                self.0 -= units;
                Ok(())
            }
        }

        let mut graph = LGraph::new("root", LayeredOptions::default());
        for index in 0..3 {
            graph.layerless_nodes.push(LNode::new(
                format!("normal-{index}"),
                10.0,
                10.0,
                Some(index),
            ));
            graph.set_node_layer(index, usize::from(index == 2));
        }
        let mut source = graph
            .add_port(0, PortType::Output, PortSide::East, LPoint::default())
            .unwrap();
        let target = graph
            .add_port(2, PortType::Input, PortSide::West, LPoint::default())
            .unwrap();
        for index in 3..259 {
            let mut dummy = LNode::new(format!("dummy-{index}"), 0.0, 0.0, None);
            dummy.kind = LNodeKind::LongEdge;
            graph.layerless_nodes.push(dummy);
            graph.set_node_layer(index, 0);
            let incoming = graph
                .add_port(index, PortType::Input, PortSide::West, LPoint::default())
                .unwrap();
            let outgoing = graph
                .add_port(index, PortType::Output, PortSide::East, LPoint::default())
                .unwrap();
            graph
                .add_edge(layered_edge("incoming", source, incoming))
                .unwrap();
            source = outgoing;
        }
        graph
            .add_edge(layered_edge("outgoing", source, target))
            .unwrap();
        let before = graph.clone();
        let previous = &graph.layers[0].nodes;
        let mut comparator =
            ModelOrderNodeComparator::new(&graph, previous, OrderingStrategy::NodesAndEdges);
        let mut measured = RemainingWork(usize::MAX);
        assert_eq!(
            comparator.compare_with_work(258, 1, &mut measured),
            Ok(Ordering::Less)
        );
        assert!(comparator.bigger_than[&1].contains(&258));
        let required = usize::MAX - measured.0;

        let mut comparator =
            ModelOrderNodeComparator::new(&graph, previous, OrderingStrategy::NodesAndEdges);
        let mut insufficient = RemainingWork(required - 1);
        assert_eq!(
            comparator.compare_with_work(258, 1, &mut insufficient),
            Err(WorkError::Interrupted)
        );
        assert!(!comparator.bigger_than[&1].contains(&258));
        assert_eq!(graph, before);
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
        layer_network_simplex(&mut graph).unwrap();
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

        sort_by_input_model(&mut graph, &mut crate::work::NoopWorkControl).unwrap();

        assert_eq!(graph.layers[target_layer].nodes, vec![left, right]);
    }

    #[test]
    fn elk_0_12_model_order_node_source_descriptor_uses_first_previous_layer_port() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        for index in 0..3 {
            graph.layerless_nodes.push(LNode::new(
                format!("node-{index}"),
                10.0,
                10.0,
                Some(index),
            ));
        }
        graph.set_node_layer(0, 0);
        graph.set_node_layer(1, 0);
        graph.set_node_layer(2, 1);

        let first_source = graph
            .add_port(0, PortType::Output, PortSide::East, LPoint::default())
            .unwrap();
        let last_source = graph
            .add_port(1, PortType::Output, PortSide::East, LPoint::default())
            .unwrap();
        let first_target = graph
            .add_port(2, PortType::Input, PortSide::West, LPoint::default())
            .unwrap();
        let last_target = graph
            .add_port(2, PortType::Input, PortSide::West, LPoint::default())
            .unwrap();
        graph
            .add_edge(layered_edge("first", first_source, first_target))
            .unwrap();
        graph
            .add_edge(layered_edge("last", last_source, last_target))
            .unwrap();

        assert_eq!(
            first_previous_layer_source_port(&graph, 2),
            Some(first_source)
        );
    }

    #[test]
    fn elk_0_12_sort_by_input_model_reads_each_outgoing_edge_order() {
        let mut graph = LGraph::new(
            "root",
            LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
        );
        graph
            .layerless_nodes
            .push(LNode::new("A", 80.0, 40.0, Some(0)));
        graph
            .layerless_nodes
            .push(LNode::new("dummy-1", 0.0, 0.0, None));
        graph
            .layerless_nodes
            .push(LNode::new("dummy-2", 0.0, 0.0, None));
        graph.layerless_nodes[1].kind = LNodeKind::LongEdge;
        graph.layerless_nodes[2].kind = LNodeKind::LongEdge;
        graph.set_node_layer(0, 0);
        graph.set_node_layer(1, 1);
        graph.set_node_layer(2, 1);
        let first = graph
            .add_port(0, PortType::Output, PortSide::East, LPoint::default())
            .unwrap();
        let second = graph
            .add_port(0, PortType::Output, PortSide::East, LPoint::default())
            .unwrap();
        let first_target = graph
            .add_port(1, PortType::Input, PortSide::West, LPoint::default())
            .unwrap();
        let second_target = graph
            .add_port(2, PortType::Input, PortSide::West, LPoint::default())
            .unwrap();
        graph
            .add_edge(crate::graph::LayeredEdge {
                id: "A-dummy-1".to_string(),
                source: first,
                target: first_target,
                source_node_id: "A".to_string(),
                target_node_id: "dummy-1".to_string(),
                labels: Vec::new(),
                minlen: 1,
                reversed: false,
                bend_points: Vec::new(),
                model_order: Some(1),
                priority_direction: 0,
                priority_shortness: 0,
                priority_straightness: 0,
                thickness: 0.0,
                original_opposite_port: None,
                compound_segment: None,
            })
            .unwrap();
        graph
            .add_edge(crate::graph::LayeredEdge {
                id: "A-dummy-2".to_string(),
                source: second,
                target: second_target,
                source_node_id: "A".to_string(),
                target_node_id: "dummy-2".to_string(),
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
            })
            .unwrap();

        assert_eq!(
            graph.layerless_nodes[0]
                .ports
                .iter()
                .map(|port| port.id.as_str())
                .collect::<Vec<_>>(),
            vec!["A:0", "A:1"]
        );

        sort_by_input_model(&mut graph, &mut crate::work::NoopWorkControl).unwrap();

        assert_eq!(
            graph.layerless_nodes[0]
                .ports
                .iter()
                .map(|port| port.id.as_str())
                .collect::<Vec<_>>(),
            vec!["A:1", "A:0"]
        );
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
        layer_network_simplex(&mut graph).unwrap();
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
        let orders =
            long_edge_target_node_preprocessing(&mut graph, a, &mut crate::work::NoopWorkControl)
                .unwrap();

        assert_eq!(orders.get(&d), Some(&3));
        assert!(
            graph.layerless_nodes[a]
                .ports
                .iter()
                .any(|port| port.long_edge_target_node == Some(d))
        );
    }

    #[test]
    fn target_node_rejects_cyclic_long_edge_chain() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        for index in 0..3 {
            graph.layerless_nodes.push(LNode::new(
                format!("node-{index}"),
                10.0,
                10.0,
                Some(index),
            ));
        }
        graph.layerless_nodes[1].kind = LNodeKind::LongEdge;
        graph.layerless_nodes[2].kind = LNodeKind::LongEdge;

        let source = graph
            .add_port(0, PortType::Output, PortSide::East, LPoint::default())
            .unwrap();
        let first_input = graph
            .add_port(1, PortType::Input, PortSide::West, LPoint::default())
            .unwrap();
        let first_output = graph
            .add_port(1, PortType::Output, PortSide::East, LPoint::default())
            .unwrap();
        let cycle_input = graph
            .add_port(1, PortType::Input, PortSide::West, LPoint::default())
            .unwrap();
        let second_input = graph
            .add_port(2, PortType::Input, PortSide::West, LPoint::default())
            .unwrap();
        let second_output = graph
            .add_port(2, PortType::Output, PortSide::East, LPoint::default())
            .unwrap();

        graph
            .add_edge(layered_edge("source", source, first_input))
            .unwrap();
        graph
            .add_edge(layered_edge("forward", first_output, second_input))
            .unwrap();
        graph
            .add_edge(layered_edge("cycle", second_output, cycle_input))
            .unwrap();

        assert_eq!(
            target_node(&graph, source, &mut crate::work::NoopWorkControl),
            Ok(None)
        );
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

    #[test]
    fn sweep_copy_restores_ports_by_crossing_minimization_id_when_external_ids_repeat() {
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
        graph.layerless_nodes[a].ports[first.port].id = "shared".to_string();
        graph.layerless_nodes[a].ports[first.port].crossing_minimization_id = Some(7);
        graph.layerless_nodes[a].ports[second.port].id = "shared".to_string();
        graph.layerless_nodes[a].ports[second.port].crossing_minimization_id = Some(8);
        graph.set_node_layer(a, 0);

        let copy = SweepCopy::new(&graph, &[vec![a]]);
        graph.reorder_node_ports(a, [1, 0]);

        assert!(copy.transfer_node_and_port_orders_to_graph(&mut graph, false));
        assert_eq!(
            graph.layerless_nodes[a]
                .ports
                .iter()
                .map(|port| port.crossing_minimization_id)
                .collect::<Vec<_>>(),
            vec![Some(7), Some(8)]
        );
    }
    #[test]
    fn sweep_copy_port_lookup_preserves_duplicate_and_mixed_fallback_matches() {
        let mut graph = graph(vec![node("A")], vec![]);
        graph.layerless_nodes[0].ports.clear();
        for (crossing_id, id) in [
            (Some(7), "same"),
            (Some(8), "same"),
            (Some(7), "other"),
            (None, "same"),
        ] {
            let port = graph
                .add_port(0, PortType::Output, PortSide::East, LPoint::default())
                .unwrap();
            graph.layerless_nodes[0].ports[port.port].crossing_minimization_id = crossing_id;
            graph.layerless_nodes[0].ports[port.port].id = id.to_string();
        }
        let key = |crossing_minimization_id, fallback_id: &str| PortOrderKey {
            crossing_minimization_id,
            fallback_id: fallback_id.to_string(),
        };
        let mixed = [
            key(None, "same"),
            key(Some(7), "ignored"),
            key(None, "same"),
            key(None, "same"),
        ];
        assert_eq!(
            port_order_indices_by_id(&graph, 0, &mixed),
            Some(vec![0, 2, 1, 3])
        );
        let repeated = [
            key(Some(7), "ignored"),
            key(Some(7), "ignored"),
            key(None, "same"),
            key(None, "same"),
        ];
        assert_eq!(
            port_order_indices_by_id(&graph, 0, &repeated),
            Some(vec![0, 2, 1, 3])
        );
        let missing = [
            key(Some(99), "same"),
            key(None, "same"),
            key(None, "other"),
            key(None, "same"),
        ];
        assert_eq!(
            port_order_indices_by_id(&graph, 0, &missing),
            None,
            "a saved numeric ID must never fall back to the external ID"
        );
    }
}
