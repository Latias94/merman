//! Layer sweep crossing minimization.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p3order/LayerSweepCrossingMinimizer.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p3order/BarycenterHeuristic.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p3order/AbstractBarycenterPortDistributor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p3order/NodeRelativePortDistributor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p3order/LayerTotalPortDistributor.java

use std::collections::HashMap;

use crate::graph::{LGraph, LNodeKind, PortRef, PortType};
use crate::options::OrderingStrategy;
use crate::p3order::counting::CrossingsCounter;
use crate::random::JavaRandom;

use super::GraphInfoHolder;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossMinType {
    Barycenter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PortDistributorKind {
    NodeRelative,
    LayerTotal,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct BarycenterState {
    summed_weight: f64,
    degree: usize,
    barycenter: Option<f64>,
    visited: bool,
}

#[derive(Debug, Clone)]
struct BarycenterPortDistributor {
    kind: PortDistributorKind,
    port_ranks: HashMap<PortRef, f64>,
}

impl BarycenterPortDistributor {
    fn new(kind: PortDistributorKind) -> Self {
        Self {
            kind,
            port_ranks: HashMap::new(),
        }
    }

    fn calculate_port_ranks(&mut self, graph: &LGraph, layer: &[usize], port_type: PortType) {
        self.port_ranks.clear();
        let mut consumed_rank = 0.0;
        for node in layer {
            consumed_rank += match self.kind {
                PortDistributorKind::NodeRelative => {
                    self.calculate_node_relative_port_ranks(graph, *node, consumed_rank, port_type)
                }
                PortDistributorKind::LayerTotal => {
                    self.calculate_layer_total_port_ranks(graph, *node, consumed_rank, port_type)
                }
            };
        }
    }

    fn rank_of(&self, port: PortRef) -> f64 {
        self.port_ranks.get(&port).copied().unwrap_or(0.0)
    }

    fn calculate_node_relative_port_ranks(
        &mut self,
        graph: &LGraph,
        node: usize,
        rank_sum: f64,
        port_type: PortType,
    ) -> f64 {
        match port_type {
            PortType::Input => {
                let input_count = actual_ports_of_type(graph, node, PortType::Input).len();
                let north_input_count = actual_ports_of_type(graph, node, PortType::Input)
                    .into_iter()
                    .filter(|port| {
                        graph.layerless_nodes[port.node].ports[port.port]
                            .side
                            .is_north()
                    })
                    .count();

                let increment = 1.0 / (input_count + 1) as f64;
                let mut north_position = rank_sum + north_input_count as f64 * increment;
                let mut rest_position = rank_sum + 1.0 - increment;
                for port in actual_ports_of_type(graph, node, PortType::Input) {
                    if graph.layerless_nodes[port.node].ports[port.port]
                        .side
                        .is_north()
                    {
                        self.port_ranks.insert(port, north_position);
                        north_position -= increment;
                    } else {
                        self.port_ranks.insert(port, rest_position);
                        rest_position -= increment;
                    }
                }
                1.0
            }
            PortType::Output => {
                let output_count = actual_ports_of_type(graph, node, PortType::Output).len();
                let increment = 1.0 / (output_count + 1) as f64;
                let mut position = rank_sum + increment;
                for port in actual_ports_of_type(graph, node, PortType::Output) {
                    self.port_ranks.insert(port, position);
                    position += increment;
                }
                1.0
            }
        }
    }

    fn calculate_layer_total_port_ranks(
        &mut self,
        graph: &LGraph,
        node: usize,
        rank_sum: f64,
        port_type: PortType,
    ) -> f64 {
        match port_type {
            PortType::Input => {
                let input_ports = actual_ports_of_type(graph, node, PortType::Input);
                let input_count = input_ports.len();
                let north_input_count = input_ports
                    .iter()
                    .filter(|port| {
                        graph.layerless_nodes[port.node].ports[port.port]
                            .side
                            .is_north()
                    })
                    .count();

                let mut north_position = rank_sum + north_input_count as f64;
                let mut rest_position = rank_sum + input_count as f64;
                for port in input_ports {
                    if graph.layerless_nodes[port.node].ports[port.port]
                        .side
                        .is_north()
                    {
                        self.port_ranks.insert(port, north_position);
                        north_position -= 1.0;
                    } else {
                        self.port_ranks.insert(port, rest_position);
                        rest_position -= 1.0;
                    }
                }
                input_count as f64
            }
            PortType::Output => {
                let mut position = 0.0;
                for port in actual_ports_of_type(graph, node, PortType::Output) {
                    position += 1.0;
                    self.port_ranks.insert(port, rank_sum + position);
                }
                position
            }
        }
    }
}

#[derive(Debug, Clone)]
struct BarycenterHeuristic {
    states: HashMap<usize, BarycenterState>,
    random: JavaRandom,
    port_distributor: BarycenterPortDistributor,
}

impl BarycenterHeuristic {
    const RANDOM_AMOUNT: f64 = 0.07;

    fn new(
        graph: &LGraph,
        random: JavaRandom,
        port_distributor: BarycenterPortDistributor,
    ) -> Self {
        let states = graph
            .layers
            .iter()
            .flat_map(|layer| layer.nodes.iter().copied())
            .map(|node| (node, BarycenterState::default()))
            .collect();
        Self {
            states,
            random,
            port_distributor,
        }
    }

    fn set_first_layer_order(&mut self, graph: &LGraph, order: &mut [Vec<usize>], forward: bool) {
        let Some(layer) = order.get_mut(first_index(forward, order.len())) else {
            return;
        };
        self.minimize_layer(graph, layer, false, true, forward);
    }

    fn minimize_crossings(
        &mut self,
        graph: &LGraph,
        order: &mut [Vec<usize>],
        free_layer_index: usize,
        forward: bool,
        is_first_sweep: bool,
    ) {
        if !is_first_layer(order, free_layer_index, forward) {
            let fixed_layer_index = if forward {
                free_layer_index - 1
            } else {
                free_layer_index + 1
            };
            let port_type = if forward {
                PortType::Output
            } else {
                PortType::Input
            };
            self.port_distributor
                .calculate_port_ranks(graph, &order[fixed_layer_index], port_type);
        }

        let pre_ordered = !is_first_sweep
            || order[free_layer_index]
                .first()
                .is_some_and(|node| graph.layerless_nodes[*node].kind == LNodeKind::ExternalPort);
        let mut nodes = order[free_layer_index].clone();
        self.minimize_layer(graph, &mut nodes, pre_ordered, false, forward);
        order[free_layer_index] = nodes;
    }

    fn minimize_layer(
        &mut self,
        graph: &LGraph,
        layer: &mut [usize],
        pre_ordered: bool,
        randomize: bool,
        forward: bool,
    ) {
        if randomize {
            self.randomize_barycenters(layer);
        } else {
            self.calculate_barycenters(graph, layer, forward);
            self.fill_in_unknown_barycenters(layer, pre_ordered);
        }

        if layer.len() > 1 {
            layer.sort_by(|left, right| {
                let left = self.states.get(left).and_then(|state| state.barycenter);
                let right = self.states.get(right).and_then(|state| state.barycenter);
                match (left, right) {
                    (Some(left), Some(right)) => left.total_cmp(&right),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal,
                }
            });
        }
    }

    fn randomize_barycenters(&mut self, nodes: &[usize]) {
        for node in nodes {
            let value = self.random.next_double();
            let state = self.state_mut(*node);
            state.barycenter = Some(value);
            state.summed_weight = value;
            state.degree = 1;
            state.visited = false;
        }
    }

    fn calculate_barycenters(&mut self, graph: &LGraph, nodes: &[usize], forward: bool) {
        for node in nodes {
            self.state_mut(*node).visited = false;
        }

        for node in nodes {
            self.calculate_barycenter(graph, *node, forward);
        }
    }

    fn calculate_barycenter(&mut self, graph: &LGraph, node: usize, forward: bool) {
        if self.state_mut(node).visited {
            return;
        }
        {
            let state = self.state_mut(node);
            state.visited = true;
            state.degree = 0;
            state.summed_weight = 0.0;
            state.barycenter = None;
        }

        for port_index in 0..graph.layerless_nodes[node].ports.len() {
            let free_port = PortRef {
                node,
                port: port_index,
            };
            for fixed_port in connected_ports_for_sweep(graph, free_port, forward) {
                if graph.layerless_nodes[fixed_port.node].layer_index
                    == graph.layerless_nodes[node].layer_index
                {
                    if fixed_port.node != node {
                        self.calculate_barycenter(graph, fixed_port.node, forward);
                        let fixed_state = self.state(fixed_port.node).clone();
                        let state = self.state_mut(node);
                        state.degree += fixed_state.degree;
                        state.summed_weight += fixed_state.summed_weight;
                    }
                } else {
                    let rank = self.port_distributor.rank_of(fixed_port);
                    let state = self.state_mut(node);
                    state.summed_weight += rank;
                    state.degree += 1;
                }
            }
        }

        let degree = self.state(node).degree;
        if degree > 0 {
            let perturbation =
                self.random.next_float() as f64 * Self::RANDOM_AMOUNT - Self::RANDOM_AMOUNT / 2.0;
            let state = self.state_mut(node);
            state.summed_weight += perturbation;
            state.barycenter = Some(state.summed_weight / degree as f64);
        }
    }

    fn fill_in_unknown_barycenters(&mut self, nodes: &[usize], pre_ordered: bool) {
        if pre_ordered {
            let mut last_value = -1.0;
            for (index, node) in nodes.iter().copied().enumerate() {
                let mut value = self.state(node).barycenter;
                if value.is_none() {
                    let next_value = nodes
                        .iter()
                        .skip(index + 1)
                        .find_map(|next| self.state(*next).barycenter)
                        .unwrap_or(last_value + 1.0);
                    value = Some((last_value + next_value) / 2.0);
                    let state = self.state_mut(node);
                    state.barycenter = value;
                    state.summed_weight = value.unwrap_or(0.0);
                    state.degree = 1;
                }
                last_value = value.unwrap_or(last_value);
            }
        } else {
            let max_barycenter = nodes
                .iter()
                .filter_map(|node| self.state(*node).barycenter)
                .fold(0.0, f64::max)
                + 2.0;
            for node in nodes {
                if self.state(*node).barycenter.is_none() {
                    let value = self.random.next_float() as f64 * max_barycenter - 1.0;
                    let state = self.state_mut(*node);
                    state.barycenter = Some(value);
                    state.summed_weight = value;
                    state.degree = 1;
                }
            }
        }
    }

    fn state(&self, node: usize) -> &BarycenterState {
        self.states
            .get(&node)
            .expect("barycenter state initialized for every layered node")
    }

    fn state_mut(&mut self, node: usize) -> &mut BarycenterState {
        self.states.entry(node).or_default()
    }
}

pub fn minimize_crossings_layer_sweep(graph: &mut LGraph) -> bool {
    minimize_crossings_layer_sweep_with_type(graph, CrossMinType::Barycenter)
}

pub fn minimize_crossings_layer_sweep_with_type(
    graph: &mut LGraph,
    cross_min_type: CrossMinType,
) -> bool {
    if graph.layers.is_empty() || graph.layers.iter().all(|layer| layer.nodes.is_empty()) {
        return false;
    }
    if graph.layers.len() == 1 && graph.layers[0].nodes.len() <= 1 {
        return false;
    }

    match cross_min_type {
        CrossMinType::Barycenter => minimize_barycenter(graph),
    }
}

fn minimize_barycenter(graph: &mut LGraph) -> bool {
    let mut graph_info = GraphInfoHolder::new(graph);
    let initial_order = graph_info.current_node_order.clone();

    let mut random = graph.random.clone();
    let random_seed = random.next_long();
    let distributor_kind = if random.next_bool() {
        PortDistributorKind::NodeRelative
    } else {
        PortDistributorKind::LayerTotal
    };
    random.set_seed(random_seed);

    let mut best_crossings = usize::MAX;
    let thoroughness = graph.options.thoroughness.max(1);
    let first_try_with_initial_order =
        graph.options.consider_model_order_strategy != OrderingStrategy::None;

    for run_index in 0..thoroughness {
        if first_try_with_initial_order && run_index <= 1 {
            graph_info.current_node_order = initial_order.clone();
        }

        let port_distributor = BarycenterPortDistributor::new(distributor_kind);
        let mut heuristic = BarycenterHeuristic::new(graph, random.clone(), port_distributor);
        let crossings = minimize_crossings_with_counter(
            graph,
            &mut graph_info,
            &mut heuristic,
            first_try_with_initial_order && run_index == 0,
            first_try_with_initial_order && run_index == 1,
        );
        random = heuristic.random;

        if crossings < best_crossings {
            best_crossings = crossings;
            if let Some(copy) = graph_info.currently_best_node_and_port_order.clone() {
                graph_info.set_best_node_and_port_order(copy);
            }
            if best_crossings == 0 {
                break;
            }
        }
    }

    graph.random = random;
    let Some(best_sweep) = graph_info.get_best_sweep().cloned() else {
        return false;
    };
    best_sweep.transfer_node_and_port_orders_to_graph(graph, true)
}

fn minimize_crossings_with_counter(
    graph: &LGraph,
    graph_info: &mut GraphInfoHolder,
    heuristic: &mut BarycenterHeuristic,
    first_try_with_initial_order: bool,
    second_try_with_initial_order: bool,
) -> usize {
    let mut is_forward_sweep = heuristic.random.next_bool();

    let initial_crossings =
        CrossingsCounter::new().count_all_crossings_in_order(graph, &graph_info.current_node_order);
    if initial_crossings == 0 && first_try_with_initial_order {
        graph_info.set_currently_best_node_and_port_order(graph);
        return 0;
    }

    if (!first_try_with_initial_order && !second_try_with_initial_order)
        || graph.options.consider_model_order_strategy == OrderingStrategy::None
    {
        heuristic.set_first_layer_order(
            graph,
            &mut graph_info.current_node_order,
            is_forward_sweep,
        );
    } else {
        is_forward_sweep = first_try_with_initial_order;
    }

    sweep_reducing_crossings(
        graph,
        graph_info,
        heuristic,
        is_forward_sweep,
        !first_try_with_initial_order && !second_try_with_initial_order,
    );

    let mut crossings_in_graph =
        CrossingsCounter::new().count_all_crossings_in_order(graph, &graph_info.current_node_order);
    loop {
        graph_info.set_currently_best_node_and_port_order(graph);
        if crossings_in_graph == 0 {
            return 0;
        }

        is_forward_sweep = !is_forward_sweep;
        let old_number_of_crossings = crossings_in_graph;
        sweep_reducing_crossings(graph, graph_info, heuristic, is_forward_sweep, false);
        crossings_in_graph = CrossingsCounter::new()
            .count_all_crossings_in_order(graph, &graph_info.current_node_order);
        if old_number_of_crossings <= crossings_in_graph {
            return old_number_of_crossings;
        }
    }
}

fn sweep_reducing_crossings(
    graph: &LGraph,
    graph_info: &mut GraphInfoHolder,
    heuristic: &mut BarycenterHeuristic,
    forward: bool,
    first_sweep: bool,
) {
    let length = graph_info.current_node_order.len();
    if length == 0 {
        return;
    }

    let mut index = first_free(forward, length);
    while is_not_end(length, index, forward) {
        let free_layer_index = index as usize;
        heuristic.minimize_crossings(
            graph,
            &mut graph_info.current_node_order,
            free_layer_index,
            forward,
            first_sweep,
        );
        index = next_index(index, forward);
    }
}

fn actual_ports_of_type(graph: &LGraph, node: usize, port_type: PortType) -> Vec<PortRef> {
    graph.layerless_nodes[node]
        .ports
        .iter()
        .enumerate()
        .filter_map(|(port, port_data)| {
            let matches = match port_type {
                PortType::Input => !port_data.incoming_edges.is_empty(),
                PortType::Output => !port_data.outgoing_edges.is_empty(),
            };
            matches.then_some(PortRef { node, port })
        })
        .collect()
}

fn connected_ports_for_sweep(graph: &LGraph, port: PortRef, forward: bool) -> Vec<PortRef> {
    let edge_indices = if forward {
        graph.layerless_nodes[port.node].ports[port.port]
            .incoming_edges
            .as_slice()
    } else {
        graph.layerless_nodes[port.node].ports[port.port]
            .outgoing_edges
            .as_slice()
    };

    edge_indices
        .iter()
        .filter_map(|edge| {
            if forward {
                graph.edges.get(*edge).map(|edge| edge.source)
            } else {
                graph.edges.get(*edge).map(|edge| edge.target)
            }
        })
        .collect()
}

fn first_index(forward: bool, length: usize) -> usize {
    if forward { 0 } else { length - 1 }
}

fn first_free(forward: bool, length: usize) -> isize {
    if forward { 1 } else { length as isize - 2 }
}

fn next_index(index: isize, forward: bool) -> isize {
    if forward { index + 1 } else { index - 1 }
}

fn is_not_end(length: usize, free_layer_index: isize, forward: bool) -> bool {
    if forward {
        free_layer_index < length as isize
    } else {
        free_layer_index >= 0
    }
}

fn is_first_layer(order: &[Vec<usize>], current_index: usize, forward: bool) -> bool {
    current_index == first_index(forward, order.len())
}

trait PortSideExt {
    fn is_north(self) -> bool;
}

impl PortSideExt for crate::graph::PortSide {
    fn is_north(self) -> bool {
        self == crate::graph::PortSide::North
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::importer::{ElkInputEdge, ElkInputGraph, ElkInputNode, import_graph};
    use crate::intermediate::split_long_edges;
    use crate::options::{ElkDirection, LayeredOptions};
    use crate::p2layers::layer_network_simplex;
    use crate::p3order::{process_port_sides, sort_port_lists};

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

    fn prepared_graph(nodes: Vec<ElkInputNode>, edges: Vec<ElkInputEdge>) -> LGraph {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
            nodes,
            edges,
        })
        .unwrap();
        layer_network_simplex(&mut graph);
        split_long_edges(&mut graph);
        process_port_sides(&mut graph);
        sort_port_lists(&mut graph);
        graph
    }

    #[test]
    fn barycenter_orders_free_layer_by_fixed_layer_port_ranks() {
        let mut graph = prepared_graph(
            vec![node("Top"), node("Bottom"), node("Left"), node("Right")],
            vec![
                edge("Top-Right", "Top", "Right"),
                edge("Bottom-Left", "Bottom", "Left"),
            ],
        );
        let top = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "Top")
            .unwrap();
        let bottom = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "Bottom")
            .unwrap();
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
        graph.layers[0].nodes = vec![top, bottom];
        graph.layers[1].nodes = vec![left, right];

        let mut order = graph
            .layers
            .iter()
            .map(|layer| layer.nodes.clone())
            .collect::<Vec<_>>();
        let port_distributor = BarycenterPortDistributor::new(PortDistributorKind::NodeRelative);
        let mut heuristic = BarycenterHeuristic::new(&graph, JavaRandom::new(1), port_distributor);

        heuristic.minimize_crossings(&graph, &mut order, 1, true, false);

        assert_eq!(order[1], vec![right, left]);
    }

    #[test]
    fn layer_sweep_keeps_best_crossing_order() {
        let mut graph = prepared_graph(
            vec![node("Top"), node("Bottom"), node("Left"), node("Right")],
            vec![
                edge("Top-Right", "Top", "Right"),
                edge("Bottom-Left", "Bottom", "Left"),
            ],
        );
        let top = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "Top")
            .unwrap();
        let bottom = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "Bottom")
            .unwrap();
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
        graph.layers[0].nodes = vec![top, bottom];
        graph.layers[1].nodes = vec![left, right];

        let before = CrossingsCounter::new().count_all_crossings(&graph);
        assert!(minimize_crossings_layer_sweep(&mut graph));
        let after = CrossingsCounter::new().count_all_crossings(&graph);

        assert_eq!(before, 1);
        assert_eq!(after, 0);
    }
}
