//! Layer sweep crossing minimization.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p3order/LayerSweepCrossingMinimizer.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p3order/BarycenterHeuristic.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p3order/AbstractBarycenterPortDistributor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p3order/NodeRelativePortDistributor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p3order/LayerTotalPortDistributor.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/greedyswitch/GreedySwitchHeuristic.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/greedyswitch/SwitchDecider.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/greedyswitch/CrossingMatrixFiller.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/greedyswitch/BetweenLayerEdgeTwoNodeCrossingsCounter.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/intermediate/greedyswitch/NorthSouthEdgeNeighbouringNodeCrossingsCounter.java

use std::collections::{HashMap, HashSet};

use crate::graph::{LGraph, LNode, LNodeKind, PortRef, PortSide, PortType};
use crate::options::{OrderingStrategy, PortConstraints};
use crate::p3order::counting::{
    BinaryIndexedTree, CrossingsCounter, ports_in_north_south_east_west_order,
};
use crate::random::JavaRandom;

use super::{
    GraphInfoHolder, SweepCopy, count_model_order_node_changes, count_model_order_port_changes,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GraphPath(Vec<usize>);

#[derive(Debug, Clone, PartialEq, Eq)]
struct HierarchyGraphInfo {
    path: GraphPath,
    current_node_order: Vec<Vec<usize>>,
    currently_best_node_and_port_order: Option<SweepCopy>,
    best_node_and_port_order: Option<SweepCopy>,
    port_distributor_kind: PortDistributorKind,
    random: JavaRandom,
    child_paths: Vec<GraphPath>,
    parent_path: Option<GraphPath>,
    parent_node_index: Option<usize>,
    has_external_ports: bool,
    use_bottom_up: bool,
}

impl HierarchyGraphInfo {
    fn new(graph: &LGraph, path: GraphPath, cross_min_type: CrossMinType) -> Self {
        let mut random = graph.random.clone();
        let port_distributor_kind = create_port_distributor_kind(cross_min_type, &mut random);
        Self {
            path,
            current_node_order: graph
                .layers
                .iter()
                .map(|layer| layer.nodes.clone())
                .collect(),
            currently_best_node_and_port_order: None,
            best_node_and_port_order: None,
            port_distributor_kind,
            random,
            child_paths: Vec::new(),
            parent_path: None,
            parent_node_index: None,
            has_external_ports: graph.graph_properties.external_ports,
            use_bottom_up: true,
        }
    }

    fn set_currently_best_node_and_port_order(&mut self, graph: &LGraph) {
        self.currently_best_node_and_port_order =
            Some(SweepCopy::new(graph, &self.current_node_order));
    }

    fn set_best_node_and_port_order(&mut self, copy: SweepCopy) {
        self.best_node_and_port_order = Some(copy);
    }

    fn get_best_sweep(&self, cross_min_type: CrossMinType) -> Option<&SweepCopy> {
        if cross_min_type.is_deterministic() {
            self.currently_best_node_and_port_order.as_ref()
        } else {
            self.best_node_and_port_order.as_ref()
        }
    }
}

struct HierarchySweep {
    infos: Vec<HierarchyGraphInfo>,
    index_by_path: HashMap<GraphPath, usize>,
    changed: HashSet<usize>,
    random: JavaRandom,
    random_seed: i64,
    cross_min_type: CrossMinType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossMinType {
    Barycenter,
    OneSidedGreedySwitch,
    TwoSidedGreedySwitch,
}

impl CrossMinType {
    fn is_deterministic(self) -> bool {
        self != Self::Barycenter
    }

    fn always_improves(self) -> bool {
        self == Self::TwoSidedGreedySwitch
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PortDistributorKind {
    NodeRelative,
    LayerTotal,
}

fn create_port_distributor_kind(
    cross_min_type: CrossMinType,
    random: &mut JavaRandom,
) -> PortDistributorKind {
    if cross_min_type == CrossMinType::TwoSidedGreedySwitch || random.next_bool() {
        PortDistributorKind::NodeRelative
    } else {
        PortDistributorKind::LayerTotal
    }
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
    port_ranks: HashMap<String, f64>,
    port_barycenters: HashMap<String, f64>,
    node_positions: HashMap<usize, usize>,
    min_barycenter: f64,
    max_barycenter: f64,
}

impl BarycenterPortDistributor {
    fn new(kind: PortDistributorKind) -> Self {
        Self {
            kind,
            port_ranks: HashMap::new(),
            port_barycenters: HashMap::new(),
            node_positions: HashMap::new(),
            min_barycenter: 0.0,
            max_barycenter: 0.0,
        }
    }

    fn distribute_ports_while_sweeping(
        &mut self,
        graph: &mut LGraph,
        order: &[Vec<usize>],
        current_index: usize,
        forward: bool,
    ) {
        if order.is_empty() || current_index >= order.len() {
            return;
        }

        self.update_node_positions(order);
        let free_layer = order[current_index].clone();
        let side = if forward {
            PortSide::West
        } else {
            PortSide::East
        };

        if !is_first_layer(order, current_index, forward) {
            let fixed_layer_index = if forward {
                current_index - 1
            } else {
                current_index + 1
            };
            let fixed_layer = order[fixed_layer_index].clone();

            self.calculate_port_ranks(graph, &fixed_layer, port_type_for(forward));
            for node in &free_layer {
                self.distribute_ports(graph, *node, side, order);
            }

            self.calculate_port_ranks(graph, &free_layer, port_type_for(!forward));
            for node in &fixed_layer {
                if graph.layerless_nodes[*node].nested_graph.is_none() {
                    self.distribute_ports(graph, *node, side.opposed(), order);
                }
            }
        } else {
            for node in &free_layer {
                self.distribute_ports(graph, *node, side, order);
            }
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

    fn rank_of(&self, graph: &LGraph, port: PortRef) -> f64 {
        port_id(graph, port)
            .and_then(|id| self.port_ranks.get(id).copied())
            .unwrap_or(0.0)
    }

    fn distribute_ports(
        &mut self,
        graph: &mut LGraph,
        node: usize,
        side: PortSide,
        order: &[Vec<usize>],
    ) {
        if graph.layerless_nodes[node]
            .port_constraints
            .is_order_fixed()
        {
            return;
        }

        for port_side in [side, PortSide::South, PortSide::North] {
            let ports = ports_on_side(graph, node, port_side);
            self.distribute_ports_on_side(graph, node, &ports, order);
        }
        self.sort_ports(graph, node);
    }

    fn distribute_ports_on_side(
        &mut self,
        graph: &LGraph,
        node: usize,
        ports: &[PortRef],
        order: &[Vec<usize>],
    ) {
        let mut in_layer_ports = Vec::new();
        self.min_barycenter = 0.0;
        self.max_barycenter = 0.0;
        let absurdly_large = 2.0 * layer_node_count(graph, node, order) as f64 + 1.0;

        'port_loop: for port in ports.iter().copied() {
            let Some(port_data) = graph
                .layerless_nodes
                .get(port.node)
                .and_then(|node| node.ports.get(port.port))
            else {
                continue;
            };

            let north_south_port = matches!(port_data.side, PortSide::North | PortSide::South);
            let mut sum = 0.0;
            if north_south_port {
                let Some(dummy_node) = port_data
                    .port_dummy
                    .as_ref()
                    .filter(|dummy| dummy.graph_id == graph.id)
                    .map(|dummy| dummy.node)
                else {
                    continue;
                };
                sum += north_south_port_barycenter_sum(graph, port, dummy_node, absurdly_large)
                    .unwrap_or(0.0);
            } else {
                for outgoing_edge in &port_data.outgoing_edges {
                    let connected_port = graph.edges[*outgoing_edge].target;
                    if same_layer(graph, connected_port.node, node) {
                        in_layer_ports.push(port);
                        continue 'port_loop;
                    }
                    sum += self.rank_of(graph, connected_port);
                }
                for incoming_edge in &port_data.incoming_edges {
                    let connected_port = graph.edges[*incoming_edge].source;
                    if same_layer(graph, connected_port.node, node) {
                        in_layer_ports.push(port);
                        continue 'port_loop;
                    }
                    sum -= self.rank_of(graph, connected_port);
                }
            }

            let degree = port_degree(graph, port);
            if degree > 0 {
                let barycenter = sum / degree as f64;
                self.set_port_barycenter(graph, port, barycenter);
                self.min_barycenter = self.min_barycenter.min(barycenter);
                self.max_barycenter = self.max_barycenter.max(barycenter);
            } else if north_south_port {
                self.set_port_barycenter(graph, port, sum);
            }
        }

        if !in_layer_ports.is_empty() {
            self.calculate_in_layer_ports_barycenter_values(graph, node, &in_layer_ports, order);
        }
    }

    fn calculate_in_layer_ports_barycenter_values(
        &mut self,
        graph: &LGraph,
        node: usize,
        in_layer_ports: &[PortRef],
        order: &[Vec<usize>],
    ) {
        let Some(node_position) = self.node_positions.get(&node).copied() else {
            return;
        };
        let node_index_in_layer = node_position + 1;
        let layer_size = layer_node_count(graph, node, order) + 1;

        for in_layer_port in in_layer_ports {
            let mut sum = 0usize;
            let mut in_layer_connections = 0usize;

            for connected_port in connected_ports(graph, *in_layer_port) {
                if same_layer(graph, connected_port.node, node)
                    && let Some(position) = self.node_positions.get(&connected_port.node)
                {
                    sum += position + 1;
                    in_layer_connections += 1;
                }
            }

            if in_layer_connections == 0 {
                continue;
            }

            let barycenter = sum as f64 / in_layer_connections as f64;
            let port_side =
                graph.layerless_nodes[in_layer_port.node].ports[in_layer_port.port].side;
            let port_barycenter = match port_side {
                PortSide::East if barycenter < node_index_in_layer as f64 => {
                    self.min_barycenter - barycenter
                }
                PortSide::East => self.max_barycenter + (layer_size as f64 - barycenter),
                PortSide::West if barycenter < node_index_in_layer as f64 => {
                    self.max_barycenter + barycenter
                }
                PortSide::West => self.min_barycenter - (layer_size as f64 - barycenter),
                _ => continue,
            };
            self.set_port_barycenter(graph, *in_layer_port, port_barycenter);
        }
    }

    fn sort_ports(&self, graph: &mut LGraph, node: usize) {
        let mut order = (0..graph.layerless_nodes[node].ports.len()).collect::<Vec<_>>();
        order.sort_by(|left, right| {
            let left_port = &graph.layerless_nodes[node].ports[*left];
            let right_port = &graph.layerless_nodes[node].ports[*right];

            let side_order = port_side_order(left_port.side).cmp(&port_side_order(right_port.side));
            if side_order != std::cmp::Ordering::Equal {
                return side_order;
            }

            let left_barycenter = self
                .port_barycenters
                .get(&left_port.id)
                .copied()
                .unwrap_or(0.0);
            let right_barycenter = self
                .port_barycenters
                .get(&right_port.id)
                .copied()
                .unwrap_or(0.0);

            if left_barycenter == 0.0 && right_barycenter == 0.0 {
                std::cmp::Ordering::Equal
            } else if left_barycenter == 0.0 {
                std::cmp::Ordering::Less
            } else if right_barycenter == 0.0 {
                std::cmp::Ordering::Greater
            } else {
                left_barycenter.total_cmp(&right_barycenter)
            }
        });
        graph.reorder_node_ports(node, order);
    }

    fn set_port_barycenter(&mut self, graph: &LGraph, port: PortRef, barycenter: f64) {
        if let Some(id) = port_id(graph, port) {
            self.port_barycenters.insert(id.to_string(), barycenter);
        }
    }

    fn update_node_positions(&mut self, order: &[Vec<usize>]) {
        self.node_positions.clear();
        for layer in order {
            for (position, node) in layer.iter().copied().enumerate() {
                self.node_positions.insert(node, position);
            }
        }
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
                        if let Some(id) = port_id(graph, port) {
                            self.port_ranks.insert(id.to_string(), north_position);
                        }
                        north_position -= increment;
                    } else {
                        if let Some(id) = port_id(graph, port) {
                            self.port_ranks.insert(id.to_string(), rest_position);
                        }
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
                    if let Some(id) = port_id(graph, port) {
                        self.port_ranks.insert(id.to_string(), position);
                    }
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
                        if let Some(id) = port_id(graph, port) {
                            self.port_ranks.insert(id.to_string(), north_position);
                        }
                        north_position -= 1.0;
                    } else {
                        if let Some(id) = port_id(graph, port) {
                            self.port_ranks.insert(id.to_string(), rest_position);
                        }
                        rest_position -= 1.0;
                    }
                }
                input_count as f64
            }
            PortType::Output => {
                let mut position = 0.0;
                for port in actual_ports_of_type(graph, node, PortType::Output) {
                    position += 1.0;
                    if let Some(id) = port_id(graph, port) {
                        self.port_ranks.insert(id.to_string(), rank_sum + position);
                    }
                }
                position
            }
        }
    }
}

impl HierarchySweep {
    fn new(root: &mut LGraph, cross_min_type: CrossMinType) -> Self {
        let mut random = root.random.clone();
        let random_seed = random.next_long();
        root.random = random.clone();

        let mut sweep = Self {
            infos: Vec::new(),
            index_by_path: HashMap::new(),
            changed: HashSet::new(),
            random,
            random_seed,
            cross_min_type,
        };
        sweep.initialize_graphs(root);
        sweep
    }

    fn initialize_graphs(&mut self, root: &LGraph) {
        let mut paths = vec![GraphPath(Vec::new())];
        let mut index = 0usize;
        while index < paths.len() {
            let path = paths[index].clone();
            let graph = graph_at_path(root, &path);
            let info_index = self.infos.len();
            self.index_by_path.insert(path.clone(), info_index);
            let mut info = HierarchyGraphInfo::new(graph, path.clone(), self.cross_min_type);
            if path.0.is_empty() {
                self.random = info.random.clone();
            }
            let child_paths = graph
                .layerless_nodes
                .iter()
                .enumerate()
                .filter_map(|(node_index, node)| {
                    node.nested_graph.as_ref().map(|_| {
                        let mut child_path = path.0.clone();
                        child_path.push(node_index);
                        GraphPath(child_path)
                    })
                })
                .collect::<Vec<_>>();
            info.child_paths = child_paths.clone();
            self.infos.push(info);
            paths.extend(child_paths);
            index += 1;
        }

        for index in 0..self.infos.len() {
            let path = self.infos[index].path.clone();
            if let Some((parent_node_index, parent_path)) = path.0.split_last() {
                let parent_path = GraphPath(parent_path.to_vec());
                self.infos[index].parent_path = Some(parent_path);
                self.infos[index].parent_node_index = Some(*parent_node_index);
            }
        }

        for index in 0..self.infos.len() {
            let use_bottom_up = self.decide_use_bottom_up(root, index);
            self.infos[index].use_bottom_up = use_bottom_up;
        }
    }

    fn decide_use_bottom_up(&self, root: &LGraph, info_index: usize) -> bool {
        let info = &self.infos[info_index];
        let Some(parent_path) = info.parent_path.as_ref() else {
            return true;
        };
        let parent_graph = graph_at_path(root, parent_path);
        let Some(parent_node_index) = info.parent_node_index else {
            return true;
        };
        let parent_node = &parent_graph.layerless_nodes[parent_node_index];
        let boundary = graph_at_path(root, &info.path)
            .options
            .hierarchical_sweepiness;

        if boundary < -1.0
            || parent_node.port_constraints.is_order_fixed()
            || fewer_than_two_in_out_parent_ports(parent_node)
        {
            return true;
        }

        if self.cross_min_type.is_deterministic() {
            return false;
        }

        let mut decider = LayerSweepTypeDecider::default();
        decider.use_bottom_up(graph_at_path(root, &info.path), boundary)
    }

    fn graphs_to_sweep_on(&self) -> Vec<usize> {
        self.infos
            .iter()
            .enumerate()
            .filter_map(|(index, info)| info.use_bottom_up.then_some(index))
            .rev()
            .collect()
    }

    fn minimize(&mut self, root: &mut LGraph) -> bool {
        let graphs_to_sweep_on = self.graphs_to_sweep_on();
        for info_index in graphs_to_sweep_on {
            if self.infos[info_index].current_node_order.is_empty() {
                continue;
            }

            if self.cross_min_type.always_improves() {
                self.minimize_no_counter(root, info_index);
            } else if self.cross_min_type.is_deterministic() {
                self.minimize_with_counter(root, info_index);
            } else {
                self.compare_different_randomized_layouts(root, info_index);
            }

            if self.infos[info_index].parent_path.is_some() {
                self.set_port_order_on_parent_graph(root, info_index);
            }
        }

        self.transfer_node_and_port_orders_to_graph(root)
    }

    fn minimize_no_counter(&mut self, root: &mut LGraph, info_index: usize) {
        let mut is_forward_sweep = self.random.next_bool();
        loop {
            let mut heuristic = GreedySwitchHeuristic::new(self.cross_min_type);
            let mut improved = {
                let graph = graph_at_path(root, &self.infos[info_index].path);
                heuristic.set_first_layer_order(
                    graph,
                    &mut self.infos[info_index].current_node_order,
                    is_forward_sweep,
                )
            };
            improved |= self.sweep_reducing_crossings_greedy(
                root,
                info_index,
                &mut heuristic,
                is_forward_sweep,
            );
            if !improved {
                break;
            }
            is_forward_sweep = !is_forward_sweep;
        }
        self.set_currently_best_node_orders(root);
    }

    fn minimize_with_counter(&mut self, root: &mut LGraph, info_index: usize) -> usize {
        let mut is_forward_sweep = self.random.next_bool();
        {
            let graph = graph_at_path(root, &self.infos[info_index].path);
            let mut heuristic = GreedySwitchHeuristic::new(self.cross_min_type);
            heuristic.set_first_layer_order(
                graph,
                &mut self.infos[info_index].current_node_order,
                is_forward_sweep,
            );
            self.sweep_reducing_crossings_greedy(
                root,
                info_index,
                &mut heuristic,
                is_forward_sweep,
            );
        }

        let mut crossings_in_graph = self.count_current_number_of_crossings(root, info_index);
        loop {
            self.set_currently_best_node_orders(root);
            if crossings_in_graph == 0 {
                return 0;
            }

            is_forward_sweep = !is_forward_sweep;
            let old_number_of_crossings = crossings_in_graph;
            let mut heuristic = GreedySwitchHeuristic::new(self.cross_min_type);
            self.sweep_reducing_crossings_greedy(
                root,
                info_index,
                &mut heuristic,
                is_forward_sweep,
            );
            crossings_in_graph = self.count_current_number_of_crossings(root, info_index);
            if old_number_of_crossings <= crossings_in_graph {
                return old_number_of_crossings;
            }
        }
    }

    fn compare_different_randomized_layouts(&mut self, root: &mut LGraph, info_index: usize) {
        self.random.set_seed(self.random_seed);
        if self.infos[info_index].path.0.is_empty() {
            self.infos[info_index].random = self.random.clone();
        }
        self.changed.clear();

        let initial_order = self.infos[info_index].current_node_order.clone();
        let first_try_with_initial_order = graph_at_path(root, &self.infos[info_index].path)
            .options
            .consider_model_order_strategy
            != OrderingStrategy::None;
        let thoroughness = graph_at_path(root, &self.infos[info_index].path)
            .options
            .thoroughness
            .max(1);
        let distributor_kind = self.infos[info_index].port_distributor_kind;

        let use_node_port_order_counter =
            self.use_node_port_order_crossing_counter(root, info_index);
        let mut best_crossings = f64::MAX;
        for run_index in 0..thoroughness {
            if first_try_with_initial_order && run_index <= 1 {
                self.infos[info_index].current_node_order = initial_order.clone();
            }

            let crossings = self.minimize_barycenter_with_counter(
                root,
                info_index,
                distributor_kind,
                first_try_with_initial_order && run_index == 0,
                first_try_with_initial_order && run_index == 1,
                use_node_port_order_counter,
            );
            if crossings < best_crossings {
                best_crossings = crossings;
                self.save_all_node_orders_of_changed_graphs(root);
                if best_crossings == 0.0 {
                    break;
                }
            }
        }
    }

    fn minimize_barycenter_with_counter(
        &mut self,
        root: &mut LGraph,
        info_index: usize,
        distributor_kind: PortDistributorKind,
        first_try_with_initial_order: bool,
        second_try_with_initial_order: bool,
        use_node_port_order_counter: bool,
    ) -> f64 {
        let mut is_forward_sweep = self.next_sweep_direction(info_index);
        let initial_crossings =
            self.count_current_crossing_score(root, info_index, use_node_port_order_counter);
        if initial_crossings == 0.0 && first_try_with_initial_order {
            self.set_currently_best_node_orders(root);
            return 0.0;
        }

        let graph = graph_at_path(root, &self.infos[info_index].path);
        let mut heuristic = BarycenterHeuristic::new(
            graph,
            self.barycenter_random(info_index),
            BarycenterPortDistributor::new(distributor_kind),
        );
        if (!first_try_with_initial_order && !second_try_with_initial_order)
            || graph.options.consider_model_order_strategy == OrderingStrategy::None
        {
            heuristic.set_first_layer_order(
                graph,
                &mut self.infos[info_index].current_node_order,
                is_forward_sweep,
            );
        } else {
            is_forward_sweep = first_try_with_initial_order;
        }
        self.set_barycenter_random(info_index, heuristic.random.clone());

        self.sweep_reducing_crossings_barycenter(
            root,
            info_index,
            distributor_kind,
            is_forward_sweep,
            true,
            first_try_with_initial_order || second_try_with_initial_order,
        );

        let mut crossings_in_graph =
            self.count_current_crossing_score(root, info_index, use_node_port_order_counter);
        loop {
            self.set_currently_best_node_orders(root);
            if crossings_in_graph == 0.0 {
                return 0.0;
            }

            is_forward_sweep = !is_forward_sweep;
            let old_number_of_crossings = crossings_in_graph;
            self.sweep_reducing_crossings_barycenter(
                root,
                info_index,
                distributor_kind,
                is_forward_sweep,
                false,
                false,
            );
            crossings_in_graph =
                self.count_current_crossing_score(root, info_index, use_node_port_order_counter);
            if old_number_of_crossings <= crossings_in_graph {
                return old_number_of_crossings;
            }
        }
    }

    fn sweep_reducing_crossings_barycenter(
        &mut self,
        root: &mut LGraph,
        info_index: usize,
        distributor_kind: PortDistributorKind,
        forward: bool,
        first_sweep: bool,
        try_with_initial_order: bool,
    ) -> bool {
        let length = self.infos[info_index].current_node_order.len();
        if length == 0 {
            return false;
        }

        {
            let graph = graph_mut_at_path(root, &self.infos[info_index].path);
            let mut heuristic = BarycenterHeuristic::new(
                graph,
                self.barycenter_random(info_index),
                BarycenterPortDistributor::new(distributor_kind),
            );
            heuristic.port_distributor.distribute_ports_while_sweeping(
                graph,
                &self.infos[info_index].current_node_order,
                first_index(forward, length),
                forward,
            );
            self.set_barycenter_random(info_index, heuristic.random.clone());
        }

        let first_layer =
            self.infos[info_index].current_node_order[first_index(forward, length)].clone();
        let mut improved =
            self.sweep_in_hierarchical_nodes(root, info_index, &first_layer, forward, first_sweep);
        let mut index = first_free(forward, length);
        while is_not_end(length, index, forward) {
            let free_layer_index = index as usize;
            {
                let graph = graph_mut_at_path(root, &self.infos[info_index].path);
                let mut heuristic = BarycenterHeuristic::new(
                    graph,
                    self.barycenter_random(info_index),
                    BarycenterPortDistributor::new(distributor_kind),
                );
                heuristic.minimize_crossings(
                    graph,
                    &mut self.infos[info_index].current_node_order,
                    free_layer_index,
                    forward,
                    first_sweep && !try_with_initial_order,
                );
                heuristic.port_distributor.distribute_ports_while_sweeping(
                    graph,
                    &self.infos[info_index].current_node_order,
                    free_layer_index,
                    forward,
                );
                self.set_barycenter_random(info_index, heuristic.random.clone());
            }
            let layer = self.infos[info_index].current_node_order[free_layer_index].clone();
            improved |=
                self.sweep_in_hierarchical_nodes(root, info_index, &layer, forward, first_sweep);
            index = next_index(index, forward);
        }

        self.changed.insert(info_index);
        improved
    }

    fn sweep_reducing_crossings_greedy(
        &mut self,
        root: &mut LGraph,
        info_index: usize,
        heuristic: &mut GreedySwitchHeuristic,
        forward: bool,
    ) -> bool {
        let length = self.infos[info_index].current_node_order.len();
        if length == 0 {
            return false;
        }

        let first_layer =
            self.infos[info_index].current_node_order[first_index(forward, length)].clone();
        let mut improved =
            self.sweep_in_hierarchical_nodes(root, info_index, &first_layer, forward, false);
        let mut index = first_free(forward, length);
        while is_not_end(length, index, forward) {
            let free_layer_index = index as usize;
            {
                let graph = graph_at_path(root, &self.infos[info_index].path);
                improved |= heuristic.minimize_crossings(
                    graph,
                    &mut self.infos[info_index].current_node_order,
                    free_layer_index,
                    forward,
                );
            }
            let layer = self.infos[info_index].current_node_order[free_layer_index].clone();
            improved |= self.sweep_in_hierarchical_nodes(root, info_index, &layer, forward, false);
            index = next_index(index, forward);
        }

        self.changed.insert(info_index);
        improved
    }

    fn sweep_in_hierarchical_nodes(
        &mut self,
        root: &mut LGraph,
        parent_info_index: usize,
        layer: &[usize],
        forward: bool,
        first_sweep: bool,
    ) -> bool {
        let mut improved = false;
        for node in layer {
            let Some(child_index) = self.child_info_for_parent_node(parent_info_index, *node)
            else {
                continue;
            };
            if self.infos[child_index].use_bottom_up {
                continue;
            }
            improved |=
                self.sweep_in_hierarchical_node(root, child_index, *node, forward, first_sweep);
        }
        improved
    }

    fn sweep_in_hierarchical_node(
        &mut self,
        root: &mut LGraph,
        child_info_index: usize,
        parent_node_index: usize,
        forward: bool,
        first_sweep: bool,
    ) -> bool {
        let child_len = self.infos[child_info_index].current_node_order.len();
        if child_len == 0 {
            return false;
        }

        let start_index = first_index(forward, child_len);
        let first_node = self.infos[child_info_index]
            .current_node_order
            .get(start_index)
            .and_then(|layer| layer.first())
            .copied();

        if let Some(first_node) = first_node {
            let child_graph = graph_at_path(root, &self.infos[child_info_index].path);
            if child_graph.layerless_nodes[first_node].kind == LNodeKind::ExternalPort {
                let parent_graph = graph_at_path(
                    root,
                    self.infos[child_info_index]
                        .parent_path
                        .as_ref()
                        .expect("child graph should have parent graph"),
                );
                let sorted = sort_port_dummies_by_port_positions(
                    parent_graph,
                    child_graph,
                    parent_node_index,
                    &self.infos[child_info_index].current_node_order[start_index],
                    if forward {
                        PortSide::West
                    } else {
                        PortSide::East
                    },
                );
                self.infos[child_info_index].current_node_order[start_index] = sorted;
            } else {
                let child_graph = graph_at_path(root, &self.infos[child_info_index].path);
                match self.cross_min_type {
                    CrossMinType::Barycenter => {
                        let child_distributor_kind =
                            self.infos[child_info_index].port_distributor_kind;
                        let mut heuristic = BarycenterHeuristic::new(
                            child_graph,
                            self.barycenter_random(child_info_index),
                            BarycenterPortDistributor::new(child_distributor_kind),
                        );
                        heuristic.set_first_layer_order(
                            child_graph,
                            &mut self.infos[child_info_index].current_node_order,
                            forward,
                        );
                        self.set_barycenter_random(child_info_index, heuristic.random.clone());
                    }
                    CrossMinType::OneSidedGreedySwitch | CrossMinType::TwoSidedGreedySwitch => {
                        let mut heuristic = GreedySwitchHeuristic::new(self.cross_min_type);
                        heuristic.set_first_layer_order(
                            child_graph,
                            &mut self.infos[child_info_index].current_node_order,
                            forward,
                        );
                    }
                }
            }
        }

        let improved = match self.cross_min_type {
            CrossMinType::Barycenter => self.sweep_reducing_crossings_barycenter(
                root,
                child_info_index,
                self.infos[child_info_index].port_distributor_kind,
                forward,
                first_sweep,
                false,
            ),
            CrossMinType::OneSidedGreedySwitch | CrossMinType::TwoSidedGreedySwitch => {
                let mut heuristic = GreedySwitchHeuristic::new(self.cross_min_type);
                self.sweep_reducing_crossings_greedy(
                    root,
                    child_info_index,
                    &mut heuristic,
                    forward,
                )
            }
        };
        self.sort_ports_by_dummy_positions_in_last_layer(root, child_info_index, forward);
        improved
    }

    fn sort_ports_by_dummy_positions_in_last_layer(
        &mut self,
        root: &mut LGraph,
        child_info_index: usize,
        on_right_most_layer: bool,
    ) {
        let child_len = self.infos[child_info_index].current_node_order.len();
        if child_len == 0 {
            return;
        }
        let end_index = first_index(!on_right_most_layer, child_len);
        let last_layer = self.infos[child_info_index].current_node_order[end_index].clone();
        if last_layer.is_empty() {
            return;
        }
        let first = if on_right_most_layer {
            0
        } else {
            last_layer.len() - 1
        };
        let child_graph = graph_at_path(root, &self.infos[child_info_index].path);
        if child_graph.layerless_nodes[last_layer[first]].kind != LNodeKind::ExternalPort {
            return;
        }

        let Some(parent_path) = self.infos[child_info_index].parent_path.clone() else {
            return;
        };
        let Some(parent_node_index) = self.infos[child_info_index].parent_node_index else {
            return;
        };
        let parent_graph_id = graph_at_path(root, &parent_path).id.clone();
        let parent_order = parent_port_order_by_dummy_layer(
            child_graph,
            &last_layer,
            parent_graph_id.as_str(),
            parent_node_index,
            on_right_most_layer,
        );
        let parent_graph = graph_mut_at_path(root, &parent_path);
        apply_parent_hierarchical_port_order_on_sweep_side(
            parent_graph,
            parent_node_index,
            on_right_most_layer,
            &parent_order,
        );
    }

    fn set_port_order_on_parent_graph(&mut self, root: &mut LGraph, info_index: usize) {
        if !self.infos[info_index].has_external_ports {
            return;
        }
        let Some(best_sweep) = self.infos[info_index]
            .get_best_sweep(self.cross_min_type)
            .cloned()
        else {
            return;
        };
        for on_right_most_layer in [true, false] {
            self.sort_ports_by_sweep_copy(root, info_index, &best_sweep, on_right_most_layer);
        }
        if let Some(parent_path) = self.infos[info_index].parent_path.clone()
            && let Some(parent_node_index) = self.infos[info_index].parent_node_index
        {
            let parent_graph = graph_mut_at_path(root, &parent_path);
            parent_graph.layerless_nodes[parent_node_index].port_constraints =
                PortConstraints::FixedOrder;
        }
    }

    fn sort_ports_by_sweep_copy(
        &mut self,
        root: &mut LGraph,
        info_index: usize,
        sweep: &SweepCopy,
        on_right_most_layer: bool,
    ) {
        if sweep.node_order.is_empty() {
            return;
        }
        let end_index = first_index(!on_right_most_layer, sweep.node_order.len());
        let last_layer = sweep.node_order[end_index].clone();
        if last_layer.is_empty() {
            return;
        }
        let child_graph = graph_at_path(root, &self.infos[info_index].path);
        let Some(parent_path) = self.infos[info_index].parent_path.clone() else {
            return;
        };
        let Some(parent_node_index) = self.infos[info_index].parent_node_index else {
            return;
        };
        let parent_graph_id = graph_at_path(root, &parent_path).id.clone();
        let parent_order = parent_port_order_by_dummy_layer(
            child_graph,
            &last_layer,
            parent_graph_id.as_str(),
            parent_node_index,
            on_right_most_layer,
        );
        let parent_graph = graph_mut_at_path(root, &parent_path);
        apply_parent_hierarchical_port_order_on_sweep_side(
            parent_graph,
            parent_node_index,
            on_right_most_layer,
            &parent_order,
        );
    }

    fn next_sweep_direction(&mut self, _info_index: usize) -> bool {
        self.random.next_bool()
    }

    fn barycenter_random(&self, info_index: usize) -> JavaRandom {
        if self.infos[info_index].path.0.is_empty() {
            self.random.clone()
        } else {
            self.infos[info_index].random.clone()
        }
    }

    fn set_barycenter_random(&mut self, info_index: usize, random: JavaRandom) {
        if self.infos[info_index].path.0.is_empty() {
            self.random = random.clone();
        }
        self.infos[info_index].random = random;
    }

    fn count_current_number_of_crossings(&self, root: &LGraph, info_index: usize) -> usize {
        let mut total = 0usize;
        let mut stack = vec![info_index];
        while let Some(current) = stack.pop() {
            let info = &self.infos[current];
            let graph = graph_at_path(root, &info.path);
            total += CrossingsCounter::new()
                .count_all_crossings_in_order(graph, &info.current_node_order);
            for child_path in &info.child_paths {
                if let Some(child_index) = self.index_by_path.get(child_path).copied()
                    && !self.infos[child_index].use_bottom_up
                {
                    stack.push(child_index);
                }
            }
        }
        total
    }

    fn use_node_port_order_crossing_counter(&self, root: &LGraph, info_index: usize) -> bool {
        let graph = graph_at_path(root, &self.infos[info_index].path);
        graph
            .options
            .consider_model_order_crossing_counter_node_influence
            != 0.0
            || graph
                .options
                .consider_model_order_crossing_counter_node_influence
                != 0.0
    }

    fn count_current_crossing_score(
        &self,
        root: &LGraph,
        info_index: usize,
        use_node_port_order_counter: bool,
    ) -> f64 {
        if use_node_port_order_counter {
            self.count_current_number_of_crossings_node_port_order(root, info_index)
        } else {
            self.count_current_number_of_crossings(root, info_index) as f64
        }
    }

    fn count_current_number_of_crossings_node_port_order(
        &self,
        root: &LGraph,
        info_index: usize,
    ) -> f64 {
        let info = &self.infos[info_index];
        let graph = graph_at_path(root, &info.path);
        let mut model_order_influence = 0.0;
        let model_order_strategy = graph.options.consider_model_order_strategy;
        if model_order_strategy != OrderingStrategy::None {
            model_order_influence += graph
                .options
                .consider_model_order_crossing_counter_node_influence
                * count_model_order_node_changes(
                    graph,
                    &info.current_node_order,
                    model_order_strategy,
                ) as f64;
            model_order_influence += graph
                .options
                .consider_model_order_crossing_counter_port_influence
                * count_model_order_port_changes(graph, &info.current_node_order) as f64;
        }

        let mut total = CrossingsCounter::new()
            .count_all_crossings_in_order(graph, &info.current_node_order)
            as f64
            + model_order_influence;

        for child_path in &info.child_paths {
            if let Some(child_index) = self.index_by_path.get(child_path).copied()
                && !self.infos[child_index].use_bottom_up
            {
                total += self.count_current_number_of_crossings(root, child_index) as f64;
            }
        }

        total
    }

    fn set_currently_best_node_orders(&mut self, root: &LGraph) {
        let changed = self.changed.iter().copied().collect::<Vec<_>>();
        for index in changed {
            let graph = graph_at_path(root, &self.infos[index].path);
            self.infos[index].set_currently_best_node_and_port_order(graph);
        }
    }

    fn save_all_node_orders_of_changed_graphs(&mut self, root: &LGraph) {
        let changed = self.changed.iter().copied().collect::<Vec<_>>();
        for index in changed {
            let graph = graph_at_path(root, &self.infos[index].path);
            let copy = SweepCopy::new(graph, &self.infos[index].current_node_order);
            self.infos[index].set_best_node_and_port_order(copy);
        }
    }

    fn transfer_node_and_port_orders_to_graph(&self, root: &mut LGraph) -> bool {
        let mut changed = false;
        for index in 0..self.infos.len() {
            let Some(best_sweep) = self.infos[index]
                .get_best_sweep(self.cross_min_type)
                .cloned()
            else {
                continue;
            };
            let graph = graph_mut_at_path(root, &self.infos[index].path);
            changed |= best_sweep.transfer_node_and_port_orders_to_graph(graph, true);
        }
        changed
    }

    fn child_info_for_parent_node(
        &self,
        parent_info_index: usize,
        parent_node_index: usize,
    ) -> Option<usize> {
        let mut child_path = self.infos[parent_info_index].path.0.clone();
        child_path.push(parent_node_index);
        self.index_by_path.get(&GraphPath(child_path)).copied()
    }
}

#[derive(Debug, Default)]
struct LayerSweepTypeDecider {
    node_info: HashMap<usize, NodeInfluence>,
}

#[derive(Debug, Clone, Copy, Default)]
struct NodeInfluence {
    connected_edges: usize,
    hierarchical_influence: usize,
    random_influence: usize,
}

impl NodeInfluence {
    fn transfer(&mut self, source: NodeInfluence) {
        self.hierarchical_influence += source.hierarchical_influence;
        self.random_influence += source.random_influence;
        self.connected_edges += source.connected_edges;
    }
}

impl LayerSweepTypeDecider {
    fn use_bottom_up(&mut self, graph: &LGraph, boundary: f64) -> bool {
        let mut paths_to_random = 0usize;
        let mut paths_to_hierarchical = 0usize;
        let mut north_south_dummies = Vec::new();

        for layer in &graph.layers {
            for node in &layer.nodes {
                if is_north_south_node(graph, *node) {
                    north_south_dummies.push(*node);
                    continue;
                }

                let mut current = self.node_info_for(*node);
                if graph.layerless_nodes[*node].kind == LNodeKind::ExternalPort {
                    current.hierarchical_influence = 1;
                    if graph.layerless_nodes[*node].external_port_side == PortSide::East {
                        paths_to_hierarchical += current.connected_edges;
                    }
                } else if !has_connected_ports_on_side(graph, *node, PortSide::West) {
                    current.random_influence = 1;
                } else if !has_connected_ports_on_side(graph, *node, PortSide::East) {
                    paths_to_random += current.connected_edges;
                }
                self.node_info.insert(*node, current);

                for edge in graph.node_outgoing_edges(*node) {
                    paths_to_random += current.random_influence;
                    paths_to_hierarchical += current.hierarchical_influence;
                    let target = graph.edges[edge].target.node;
                    self.transfer_info_to(current, target);
                }

                for port in ports_in_north_south_east_west_order(graph, *node, PortSide::North)
                    .into_iter()
                    .chain(ports_in_north_south_east_west_order(
                        graph,
                        *node,
                        PortSide::South,
                    ))
                {
                    if let Some(dummy) = graph.layerless_nodes[port.node].ports[port.port]
                        .port_dummy
                        .as_ref()
                        && dummy.graph_id == graph.id
                    {
                        paths_to_random += current.random_influence;
                        paths_to_hierarchical += current.hierarchical_influence;
                        self.transfer_info_to(current, dummy.node);
                    }
                }
            }

            for node in north_south_dummies.drain(..) {
                let current = self.node_info_for(node);
                for edge in graph.node_outgoing_edges(node) {
                    paths_to_random += current.random_influence;
                    paths_to_hierarchical += current.hierarchical_influence;
                    let target = graph.edges[edge].target.node;
                    self.transfer_info_to(current, target);
                }
            }
        }

        let all_paths = paths_to_random + paths_to_hierarchical;
        let normalized = if all_paths == 0 {
            f64::INFINITY
        } else {
            (paths_to_random as f64 - paths_to_hierarchical as f64) / all_paths as f64
        };
        normalized >= boundary
    }

    fn node_info_for(&mut self, node: usize) -> NodeInfluence {
        *self.node_info.entry(node).or_default()
    }

    fn transfer_info_to(&mut self, current: NodeInfluence, target: usize) {
        let target_info = self.node_info.entry(target).or_default();
        target_info.transfer(current);
        target_info.connected_edges += 1;
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
                    let rank = self.port_distributor.rank_of(graph, fixed_port);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CrossingCountSide {
    West,
    East,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Adjacency {
    position: usize,
    cardinality: usize,
    current_cardinality: usize,
}

impl Adjacency {
    fn new(position: usize) -> Self {
        Self {
            position,
            cardinality: 1,
            current_cardinality: 1,
        }
    }

    fn reset(&mut self) {
        self.current_cardinality = self.cardinality;
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct AdjacencyList {
    adjacency_list: Vec<Adjacency>,
    size: usize,
    current_size: usize,
    current_index: usize,
}

impl AdjacencyList {
    fn new(graph: &LGraph, current_node_order: &[Vec<usize>], node: usize, side: PortSide) -> Self {
        let mut adjacency_list = Vec::new();
        let mut size = 0usize;
        let port_positions =
            neighboring_layer_port_positions(graph, current_node_order, node, side);

        for port in ports_in_north_south_east_west_order(graph, node, side) {
            for edge in edges_connected_to_side(graph, port, side) {
                if !is_edge_self_loop(graph, edge) && !is_in_layer_edge(graph, edge) {
                    let adjacent_port = adjacent_port_of_side(graph, edge, side);
                    if let Some(adjacent_port_position) = port_positions.get(&adjacent_port) {
                        adjacency_list.push(Adjacency::new(*adjacent_port_position));
                        size += 1;
                    }
                }
            }
        }

        adjacency_list.sort_by_key(|adjacency| adjacency.position);
        let adjacency_list = merge_equal_adjacencies(adjacency_list);
        Self {
            adjacency_list,
            size,
            current_size: size,
            current_index: 0,
        }
    }

    fn reset(&mut self) {
        self.current_index = 0;
        self.current_size = self.size;
        if !self.is_empty() {
            self.current_adjacency_mut().reset();
        }
    }

    fn count_adjacencies_below_node_of_first_port(&self) -> usize {
        self.current_size - self.current_adjacency().current_cardinality
    }

    fn remove_first(&mut self) {
        if self.is_empty() {
            return;
        }

        if self.current_adjacency().current_cardinality == 1 {
            self.increment_current_index();
        } else {
            self.current_adjacency_mut().current_cardinality -= 1;
        }

        self.current_size -= 1;
    }

    fn increment_current_index(&mut self) {
        self.current_index += 1;
        if self.current_index < self.adjacency_list.len() {
            self.current_adjacency_mut().reset();
        }
    }

    fn is_empty(&self) -> bool {
        self.current_size == 0
    }

    fn first(&self) -> usize {
        self.current_adjacency().position
    }

    fn size(&self) -> usize {
        self.current_size
    }

    fn current_adjacency(&self) -> &Adjacency {
        &self.adjacency_list[self.current_index]
    }

    fn current_adjacency_mut(&mut self) -> &mut Adjacency {
        &mut self.adjacency_list[self.current_index]
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct BetweenLayerEdgeTwoNodeCrossingsCounter {
    upper_lower_crossings: usize,
    lower_upper_crossings: usize,
    eastern_adjacencies: HashMap<usize, AdjacencyList>,
    western_adjacencies: HashMap<usize, AdjacencyList>,
}

impl BetweenLayerEdgeTwoNodeCrossingsCounter {
    fn new() -> Self {
        Self::default()
    }

    fn count_eastern_edge_crossings(
        &mut self,
        graph: &LGraph,
        current_node_order: &[Vec<usize>],
        free_layer_index: usize,
        upper_node: usize,
        lower_node: usize,
    ) {
        self.reset_crossing_count();
        if upper_node == lower_node {
            return;
        }
        self.add_crossings(
            graph,
            current_node_order,
            free_layer_index,
            upper_node,
            lower_node,
            PortSide::East,
        );
    }

    fn count_western_edge_crossings(
        &mut self,
        graph: &LGraph,
        current_node_order: &[Vec<usize>],
        free_layer_index: usize,
        upper_node: usize,
        lower_node: usize,
    ) {
        self.reset_crossing_count();
        if upper_node == lower_node {
            return;
        }
        self.add_crossings(
            graph,
            current_node_order,
            free_layer_index,
            upper_node,
            lower_node,
            PortSide::West,
        );
    }

    fn count_both_side_crossings(
        &mut self,
        graph: &LGraph,
        current_node_order: &[Vec<usize>],
        free_layer_index: usize,
        upper_node: usize,
        lower_node: usize,
    ) {
        self.reset_crossing_count();
        if upper_node == lower_node {
            return;
        }
        self.add_crossings(
            graph,
            current_node_order,
            free_layer_index,
            upper_node,
            lower_node,
            PortSide::West,
        );
        self.add_crossings(
            graph,
            current_node_order,
            free_layer_index,
            upper_node,
            lower_node,
            PortSide::East,
        );
    }

    fn reset_crossing_count(&mut self) {
        self.upper_lower_crossings = 0;
        self.lower_upper_crossings = 0;
    }

    fn add_crossings(
        &mut self,
        graph: &LGraph,
        current_node_order: &[Vec<usize>],
        free_layer_index: usize,
        upper_node: usize,
        lower_node: usize,
        side: PortSide,
    ) {
        if !neighboring_layer_exists(current_node_order, free_layer_index, side) {
            return;
        }

        let (upper, lower) = match side {
            PortSide::East => {
                if self.eastern_adjacencies.is_empty() {
                    self.eastern_adjacencies =
                        build_adjacencies(graph, current_node_order, free_layer_index, side);
                }
                (
                    self.eastern_adjacencies.get(&upper_node).cloned(),
                    self.eastern_adjacencies.get(&lower_node).cloned(),
                )
            }
            PortSide::West => {
                if self.western_adjacencies.is_empty() {
                    self.western_adjacencies =
                        build_adjacencies(graph, current_node_order, free_layer_index, side);
                }
                (
                    self.western_adjacencies.get(&upper_node).cloned(),
                    self.western_adjacencies.get(&lower_node).cloned(),
                )
            }
            _ => (None, None),
        };

        let (Some(mut upper), Some(mut lower)) = (upper, lower) else {
            return;
        };
        upper.reset();
        lower.reset();
        if upper.size() == 0 || lower.size() == 0 {
            return;
        }
        self.count_crossings_by_merging_adjacency_lists(upper, lower);
    }

    fn count_crossings_by_merging_adjacency_lists(
        &mut self,
        mut upper_adjacencies: AdjacencyList,
        mut lower_adjacencies: AdjacencyList,
    ) {
        while !upper_adjacencies.is_empty() && !lower_adjacencies.is_empty() {
            if is_below(upper_adjacencies.first(), lower_adjacencies.first()) {
                self.upper_lower_crossings += upper_adjacencies.size();
                lower_adjacencies.remove_first();
            } else if is_below(lower_adjacencies.first(), upper_adjacencies.first()) {
                self.lower_upper_crossings += lower_adjacencies.size();
                upper_adjacencies.remove_first();
            } else {
                self.upper_lower_crossings +=
                    upper_adjacencies.count_adjacencies_below_node_of_first_port();
                self.lower_upper_crossings +=
                    lower_adjacencies.count_adjacencies_below_node_of_first_port();
                upper_adjacencies.remove_first();
                lower_adjacencies.remove_first();
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CrossingMatrixFiller {
    is_crossing_matrix_filled: Vec<Vec<bool>>,
    crossing_matrix: Vec<Vec<usize>>,
    in_between_layer_crossing_counter: BetweenLayerEdgeTwoNodeCrossingsCounter,
    direction: CrossingCountSide,
    one_sided: bool,
}

impl CrossingMatrixFiller {
    fn new(
        free_layer_len: usize,
        greedy_switch_type: CrossMinType,
        direction: CrossingCountSide,
    ) -> Self {
        Self {
            is_crossing_matrix_filled: vec![vec![false; free_layer_len]; free_layer_len],
            crossing_matrix: vec![vec![0; free_layer_len]; free_layer_len],
            in_between_layer_crossing_counter: BetweenLayerEdgeTwoNodeCrossingsCounter::new(),
            direction,
            one_sided: greedy_switch_type == CrossMinType::OneSidedGreedySwitch,
        }
    }

    fn get_crossing_matrix_entry(
        &mut self,
        graph: &LGraph,
        current_node_order: &[Vec<usize>],
        free_layer_index: usize,
        layer_positions: &HashMap<usize, usize>,
        upper_node: usize,
        lower_node: usize,
    ) -> usize {
        let Some(upper_position) = layer_positions.get(&upper_node).copied() else {
            return 0;
        };
        let Some(lower_position) = layer_positions.get(&lower_node).copied() else {
            return 0;
        };

        if !self.is_crossing_matrix_filled[upper_position][lower_position] {
            self.fill_crossing_matrix(
                graph,
                current_node_order,
                free_layer_index,
                upper_position,
                lower_position,
                upper_node,
                lower_node,
            );
            self.is_crossing_matrix_filled[upper_position][lower_position] = true;
            self.is_crossing_matrix_filled[lower_position][upper_position] = true;
        }
        self.crossing_matrix[upper_position][lower_position]
    }

    fn fill_crossing_matrix(
        &mut self,
        graph: &LGraph,
        current_node_order: &[Vec<usize>],
        free_layer_index: usize,
        upper_position: usize,
        lower_position: usize,
        upper_node: usize,
        lower_node: usize,
    ) {
        if self.one_sided {
            match self.direction {
                CrossingCountSide::East => self
                    .in_between_layer_crossing_counter
                    .count_eastern_edge_crossings(
                        graph,
                        current_node_order,
                        free_layer_index,
                        upper_node,
                        lower_node,
                    ),
                CrossingCountSide::West => self
                    .in_between_layer_crossing_counter
                    .count_western_edge_crossings(
                        graph,
                        current_node_order,
                        free_layer_index,
                        upper_node,
                        lower_node,
                    ),
            }
        } else {
            self.in_between_layer_crossing_counter
                .count_both_side_crossings(
                    graph,
                    current_node_order,
                    free_layer_index,
                    upper_node,
                    lower_node,
                );
        }

        self.crossing_matrix[upper_position][lower_position] =
            self.in_between_layer_crossing_counter.upper_lower_crossings;
        self.crossing_matrix[lower_position][upper_position] =
            self.in_between_layer_crossing_counter.lower_upper_crossings;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InLayerCrossingCounter {
    port_positions: HashMap<PortRef, usize>,
    node_cardinalities: HashMap<usize, usize>,
    side: PortSide,
}

impl InLayerCrossingCounter {
    fn new(graph: &LGraph, free_layer: &[usize], side: PortSide) -> Self {
        let mut port_positions = HashMap::new();
        let mut node_cardinalities = HashMap::new();
        let mut position = 0usize;
        for node in free_layer {
            let ports = ports_for_counter_side(graph, *node, side, true);
            node_cardinalities.insert(*node, ports.len());
            for port in ports {
                port_positions.insert(port, position);
                position += 1;
            }
        }
        Self {
            port_positions,
            node_cardinalities,
            side,
        }
    }

    fn count_in_layer_crossings_between_nodes_in_both_orders(
        &mut self,
        graph: &LGraph,
        upper_node: usize,
        lower_node: usize,
    ) -> (usize, usize) {
        let mut ports =
            self.connected_in_layer_ports_sorted_by_position(graph, upper_node, lower_node);
        let upper_lower_crossings = self.count_in_layer_crossings_on_ports(graph, &ports);
        self.switch_nodes(graph, upper_node, lower_node);
        ports.sort_by_key(|port| self.position_of(*port));
        let lower_upper_crossings = self.count_in_layer_crossings_on_ports(graph, &ports);
        self.switch_nodes(graph, lower_node, upper_node);
        (upper_lower_crossings, lower_upper_crossings)
    }

    fn switch_nodes(&mut self, graph: &LGraph, was_upper_node: usize, was_lower_node: usize) {
        let lower_cardinality = *self.node_cardinalities.get(&was_lower_node).unwrap_or(&0);
        for port in ports_in_north_south_east_west_order(graph, was_upper_node, self.side) {
            if let Some(position) = self.port_positions.get_mut(&port) {
                *position += lower_cardinality;
            }
        }

        let upper_cardinality = *self.node_cardinalities.get(&was_upper_node).unwrap_or(&0);
        for port in ports_in_north_south_east_west_order(graph, was_lower_node, self.side) {
            if let Some(position) = self.port_positions.get_mut(&port) {
                *position = position.saturating_sub(upper_cardinality);
            }
        }
    }

    fn connected_in_layer_ports_sorted_by_position(
        &self,
        graph: &LGraph,
        upper_node: usize,
        lower_node: usize,
    ) -> Vec<PortRef> {
        let mut ports = HashSet::new();
        for node in [upper_node, lower_node] {
            for port in ports_in_north_south_east_west_order(graph, node, self.side) {
                for edge in connected_edges(graph, port) {
                    if is_edge_self_loop(graph, edge) {
                        continue;
                    }
                    ports.insert(port);
                    if is_in_layer_edge(graph, edge)
                        && let Some(other) = other_end_of_edge(graph, edge, port)
                    {
                        ports.insert(other);
                    }
                }
            }
        }

        let mut ports = ports.into_iter().collect::<Vec<_>>();
        ports.sort_by_key(|port| self.position_of(*port));
        ports
    }

    fn count_in_layer_crossings_on_ports(&self, graph: &LGraph, ports: &[PortRef]) -> usize {
        let mut crossings = 0usize;
        let mut ends = Vec::new();
        let mut index_tree = BinaryIndexedTree::new(self.port_positions.len());

        for port in ports {
            let Some(port_position) = self.position_of_checked(*port) else {
                continue;
            };
            index_tree.remove_all(port_position);
            let mut num_between_layer_edges = 0usize;

            for edge in connected_edges(graph, *port) {
                if is_in_layer_edge(graph, edge) {
                    let Some(other_end) = other_end_of_edge(graph, edge, *port) else {
                        continue;
                    };
                    let Some(end_position) = self.position_of_checked(other_end) else {
                        continue;
                    };
                    if end_position > port_position {
                        crossings += index_tree.rank(end_position);
                        ends.push(end_position);
                    }
                } else {
                    num_between_layer_edges += 1;
                }
            }

            crossings += index_tree.size() * num_between_layer_edges;
            while let Some(end) = ends.pop() {
                index_tree.add(end);
            }
        }

        crossings
    }

    fn position_of(&self, port: PortRef) -> usize {
        self.position_of_checked(port).unwrap_or(usize::MAX)
    }

    fn position_of_checked(&self, port: PortRef) -> Option<usize> {
        self.port_positions.get(&port).copied()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct NorthSouthEdgeNeighbouringNodeCrossingsCounter {
    upper_lower_crossings: usize,
    lower_upper_crossings: usize,
}

impl NorthSouthEdgeNeighbouringNodeCrossingsCounter {
    fn new() -> Self {
        Self::default()
    }

    fn count_crossings(&mut self, graph: &LGraph, upper_node: usize, lower_node: usize) {
        self.upper_lower_crossings = 0;
        self.lower_upper_crossings = 0;
        self.process_if_north_south_long_edge_dummy_crossing(graph, upper_node, lower_node);
        self.process_if_normal_node_with_ns_ports_and_long_edge_dummy(
            graph, upper_node, lower_node,
        );
    }

    fn process_if_north_south_long_edge_dummy_crossing(
        &mut self,
        graph: &LGraph,
        upper_node: usize,
        lower_node: usize,
    ) {
        if is_north_south_node(graph, upper_node) && is_long_edge_dummy(graph, lower_node) {
            if north_south_dummy_is_north_of_normal_node(graph, upper_node) {
                self.upper_lower_crossings = 1;
            } else {
                self.lower_upper_crossings = 1;
            }
        } else if is_north_south_node(graph, lower_node) && is_long_edge_dummy(graph, upper_node) {
            if north_south_dummy_is_north_of_normal_node(graph, lower_node) {
                self.lower_upper_crossings = 1;
            } else {
                self.upper_lower_crossings = 1;
            }
        }
    }

    fn process_if_normal_node_with_ns_ports_and_long_edge_dummy(
        &mut self,
        graph: &LGraph,
        upper_node: usize,
        lower_node: usize,
    ) {
        if graph.layerless_nodes[upper_node].kind == LNodeKind::Normal
            && is_long_edge_dummy(graph, lower_node)
        {
            self.upper_lower_crossings =
                number_of_north_south_edges(graph, upper_node, PortSide::South);
            self.lower_upper_crossings =
                number_of_north_south_edges(graph, upper_node, PortSide::North);
        }
        if graph.layerless_nodes[lower_node].kind == LNodeKind::Normal
            && is_long_edge_dummy(graph, upper_node)
        {
            self.upper_lower_crossings =
                number_of_north_south_edges(graph, lower_node, PortSide::North);
            self.lower_upper_crossings =
                number_of_north_south_edges(graph, lower_node, PortSide::South);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SwitchDecider {
    free_layer_index: usize,
    one_sided: bool,
    crossing_matrix_filler: CrossingMatrixFiller,
    left_in_layer_counter: InLayerCrossingCounter,
    right_in_layer_counter: InLayerCrossingCounter,
    north_south_counter: NorthSouthEdgeNeighbouringNodeCrossingsCounter,
}

impl SwitchDecider {
    fn new(
        graph: &LGraph,
        current_node_order: &[Vec<usize>],
        free_layer_index: usize,
        crossing_matrix_filler: CrossingMatrixFiller,
        one_sided: bool,
    ) -> Self {
        let free_layer = &current_node_order[free_layer_index];
        Self {
            free_layer_index,
            one_sided,
            crossing_matrix_filler,
            left_in_layer_counter: InLayerCrossingCounter::new(graph, free_layer, PortSide::West),
            right_in_layer_counter: InLayerCrossingCounter::new(graph, free_layer, PortSide::East),
            north_south_counter: NorthSouthEdgeNeighbouringNodeCrossingsCounter::new(),
        }
    }

    fn notify_of_switch(
        &mut self,
        graph: &LGraph,
        current_node_order: &[Vec<usize>],
        upper_node: usize,
        lower_node: usize,
    ) {
        self.left_in_layer_counter
            .switch_nodes(graph, upper_node, lower_node);
        self.right_in_layer_counter
            .switch_nodes(graph, upper_node, lower_node);
        self.crossing_matrix_filler = CrossingMatrixFiller::new(
            current_node_order[self.free_layer_index].len(),
            if self.one_sided {
                CrossMinType::OneSidedGreedySwitch
            } else {
                CrossMinType::TwoSidedGreedySwitch
            },
            self.crossing_matrix_filler.direction,
        );
    }

    fn does_switch_reduce_crossings(
        &mut self,
        graph: &LGraph,
        current_node_order: &[Vec<usize>],
        upper_node_index: usize,
        lower_node_index: usize,
    ) -> bool {
        if self.constraints_prevent_switch(
            graph,
            &current_node_order[self.free_layer_index],
            upper_node_index,
            lower_node_index,
        ) {
            return false;
        }

        let free_layer = &current_node_order[self.free_layer_index];
        let upper_node = free_layer[upper_node_index];
        let lower_node = free_layer[lower_node_index];
        let layer_positions = layer_position_map(free_layer);

        let left_in_layer = self
            .left_in_layer_counter
            .count_in_layer_crossings_between_nodes_in_both_orders(graph, upper_node, lower_node);
        let right_in_layer = self
            .right_in_layer_counter
            .count_in_layer_crossings_between_nodes_in_both_orders(graph, upper_node, lower_node);
        self.north_south_counter
            .count_crossings(graph, upper_node, lower_node);

        let upper_lower_crossings = self.crossing_matrix_filler.get_crossing_matrix_entry(
            graph,
            current_node_order,
            self.free_layer_index,
            &layer_positions,
            upper_node,
            lower_node,
        ) + left_in_layer.0
            + right_in_layer.0
            + self.north_south_counter.upper_lower_crossings;
        let lower_upper_crossings = self.crossing_matrix_filler.get_crossing_matrix_entry(
            graph,
            current_node_order,
            self.free_layer_index,
            &layer_positions,
            lower_node,
            upper_node,
        ) + left_in_layer.1
            + right_in_layer.1
            + self.north_south_counter.lower_upper_crossings;

        upper_lower_crossings > lower_upper_crossings
    }

    fn constraints_prevent_switch(
        &self,
        graph: &LGraph,
        free_layer: &[usize],
        upper_node_index: usize,
        lower_node_index: usize,
    ) -> bool {
        let upper_node = free_layer[upper_node_index];
        let lower_node = free_layer[lower_node_index];
        are_normal_and_north_south_port_dummy(graph, upper_node, lower_node)
            || have_north_south_layout_unit_guard(graph, upper_node, lower_node)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GreedySwitchHeuristic {
    greedy_switch_type: CrossMinType,
}

impl GreedySwitchHeuristic {
    fn new(greedy_switch_type: CrossMinType) -> Self {
        Self { greedy_switch_type }
    }

    fn minimize_crossings(
        &mut self,
        graph: &LGraph,
        order: &mut [Vec<usize>],
        free_layer_index: usize,
        forward: bool,
    ) -> bool {
        self.set_up_and_switch(graph, order, free_layer_index, forward, true)
    }

    fn set_first_layer_order(
        &mut self,
        graph: &LGraph,
        order: &mut [Vec<usize>],
        forward: bool,
    ) -> bool {
        let start_index = first_index(forward, order.len());
        self.set_up_and_switch(graph, order, start_index, forward, false)
    }

    fn set_up_and_switch(
        &mut self,
        graph: &LGraph,
        order: &mut [Vec<usize>],
        free_layer_index: usize,
        forward: bool,
        repeat_until_stable: bool,
    ) -> bool {
        let side = if forward {
            CrossingCountSide::West
        } else {
            CrossingCountSide::East
        };
        let filler =
            CrossingMatrixFiller::new(order[free_layer_index].len(), self.greedy_switch_type, side);
        let mut switch_decider = SwitchDecider::new(
            graph,
            order,
            free_layer_index,
            filler,
            self.greedy_switch_type == CrossMinType::OneSidedGreedySwitch,
        );

        if repeat_until_stable {
            self.continue_switching_until_no_improvement_in_layer(
                graph,
                order,
                free_layer_index,
                &mut switch_decider,
            )
        } else {
            self.sweep_downward_in_layer(graph, order, free_layer_index, &mut switch_decider)
        }
    }

    fn continue_switching_until_no_improvement_in_layer(
        &mut self,
        graph: &LGraph,
        order: &mut [Vec<usize>],
        free_layer_index: usize,
        switch_decider: &mut SwitchDecider,
    ) -> bool {
        let mut improved = false;
        loop {
            let continue_switching =
                self.sweep_downward_in_layer(graph, order, free_layer_index, switch_decider);
            improved |= continue_switching;
            if !continue_switching {
                return improved;
            }
        }
    }

    fn sweep_downward_in_layer(
        &mut self,
        graph: &LGraph,
        order: &mut [Vec<usize>],
        layer_index: usize,
        switch_decider: &mut SwitchDecider,
    ) -> bool {
        let mut continue_switching = false;
        let length_of_free_layer = order[layer_index].len();
        for upper_node_index in 0..length_of_free_layer.saturating_sub(1) {
            let lower_node_index = upper_node_index + 1;
            if switch_decider.does_switch_reduce_crossings(
                graph,
                order,
                upper_node_index,
                lower_node_index,
            ) {
                let upper_node = order[layer_index][upper_node_index];
                let lower_node = order[layer_index][lower_node_index];
                switch_decider.notify_of_switch(graph, order, upper_node, lower_node);
                order[layer_index].swap(upper_node_index, lower_node_index);
                continue_switching = true;
            }
        }
        continue_switching
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
        CrossMinType::OneSidedGreedySwitch => minimize_one_sided_greedy_switch(graph),
        CrossMinType::TwoSidedGreedySwitch => minimize_two_sided_greedy_switch(graph),
    }
}

pub fn minimize_crossings_layer_sweep_hierarchical_with_type(
    graph: &mut LGraph,
    cross_min_type: CrossMinType,
) -> bool {
    if graph.layers.is_empty() || graph.layers.iter().all(|layer| layer.nodes.is_empty()) {
        let hierarchical_graphs = graph
            .layerless_nodes
            .iter()
            .filter_map(|node| node.nested_graph.as_deref())
            .any(|nested| {
                !nested.layers.is_empty()
                    && nested.layers.iter().any(|layer| !layer.nodes.is_empty())
            });
        if !hierarchical_graphs {
            return false;
        }
    }

    let mut sweep = HierarchySweep::new(graph, cross_min_type);
    sweep.minimize(graph)
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

fn minimize_two_sided_greedy_switch(graph: &mut LGraph) -> bool {
    let mut graph_info = GraphInfoHolder::new(graph);
    let mut heuristic = GreedySwitchHeuristic::new(CrossMinType::TwoSidedGreedySwitch);
    let mut is_forward_sweep = graph.random.next_bool();

    loop {
        let mut sweep_improved = heuristic.set_first_layer_order(
            graph,
            &mut graph_info.current_node_order,
            is_forward_sweep,
        );
        sweep_improved |= sweep_reducing_crossings_greedy(
            graph,
            &mut graph_info,
            &mut heuristic,
            is_forward_sweep,
        );
        if !sweep_improved {
            break;
        }
        is_forward_sweep = !is_forward_sweep;
    }

    graph_info.set_currently_best_node_and_port_order(graph);

    let Some(best_sweep) = graph_info.get_best_sweep().cloned() else {
        return false;
    };
    best_sweep.transfer_node_and_port_orders_to_graph(graph, true)
}

fn minimize_one_sided_greedy_switch(graph: &mut LGraph) -> bool {
    let mut graph_info = GraphInfoHolder::new(graph);
    let mut heuristic = GreedySwitchHeuristic::new(CrossMinType::OneSidedGreedySwitch);
    let mut is_forward_sweep = graph.random.next_bool();

    heuristic.set_first_layer_order(graph, &mut graph_info.current_node_order, is_forward_sweep);
    sweep_reducing_crossings_greedy(graph, &mut graph_info, &mut heuristic, is_forward_sweep);

    let mut crossings_in_graph =
        CrossingsCounter::new().count_all_crossings_in_order(graph, &graph_info.current_node_order);
    loop {
        graph_info.set_currently_best_node_and_port_order(graph);
        if crossings_in_graph == 0 {
            break;
        }

        is_forward_sweep = !is_forward_sweep;
        let old_number_of_crossings = crossings_in_graph;
        sweep_reducing_crossings_greedy(graph, &mut graph_info, &mut heuristic, is_forward_sweep);
        crossings_in_graph = CrossingsCounter::new()
            .count_all_crossings_in_order(graph, &graph_info.current_node_order);
        if old_number_of_crossings <= crossings_in_graph {
            break;
        }
    }

    let Some(best_sweep) = graph_info.get_best_sweep().cloned() else {
        return false;
    };
    best_sweep.transfer_node_and_port_orders_to_graph(graph, true)
}

fn minimize_crossings_with_counter(
    graph: &mut LGraph,
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
    graph: &mut LGraph,
    graph_info: &mut GraphInfoHolder,
    heuristic: &mut BarycenterHeuristic,
    forward: bool,
    first_sweep: bool,
) {
    let length = graph_info.current_node_order.len();
    if length == 0 {
        return;
    }

    heuristic.port_distributor.distribute_ports_while_sweeping(
        graph,
        &graph_info.current_node_order,
        first_index(forward, length),
        forward,
    );

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
        heuristic.port_distributor.distribute_ports_while_sweeping(
            graph,
            &graph_info.current_node_order,
            free_layer_index,
            forward,
        );
        index = next_index(index, forward);
    }
}

fn sweep_reducing_crossings_greedy(
    graph: &LGraph,
    graph_info: &mut GraphInfoHolder,
    heuristic: &mut GreedySwitchHeuristic,
    forward: bool,
) -> bool {
    let length = graph_info.current_node_order.len();
    if length == 0 {
        return false;
    }

    let mut improved = false;
    let mut index = first_free(forward, length);
    while is_not_end(length, index, forward) {
        let free_layer_index = index as usize;
        improved |= heuristic.minimize_crossings(
            graph,
            &mut graph_info.current_node_order,
            free_layer_index,
            forward,
        );
        index = next_index(index, forward);
    }
    improved
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

fn ports_on_side(graph: &LGraph, node: usize, side: PortSide) -> Vec<PortRef> {
    graph.layerless_nodes[node]
        .ports
        .iter()
        .enumerate()
        .filter_map(|(port, port_data)| (port_data.side == side).then_some(PortRef { node, port }))
        .collect()
}

fn port_id(graph: &LGraph, port: PortRef) -> Option<&str> {
    graph
        .layerless_nodes
        .get(port.node)?
        .ports
        .get(port.port)
        .map(|port| port.id.as_str())
}

fn port_degree(graph: &LGraph, port: PortRef) -> usize {
    let Some(port) = graph
        .layerless_nodes
        .get(port.node)
        .and_then(|node| node.ports.get(port.port))
    else {
        return 0;
    };
    port.incoming_edges.len() + port.outgoing_edges.len()
}

fn connected_ports(graph: &LGraph, port: PortRef) -> Vec<PortRef> {
    connected_edges(graph, port)
        .filter_map(|edge| other_end_of_edge(graph, edge, port))
        .collect()
}

fn same_layer(graph: &LGraph, left: usize, right: usize) -> bool {
    graph.layerless_nodes[left].layer_index == graph.layerless_nodes[right].layer_index
}

fn north_south_port_barycenter_sum(
    graph: &LGraph,
    port: PortRef,
    dummy_node: usize,
    absurdly_large: f64,
) -> Option<f64> {
    let dummy = graph.layerless_nodes.get(dummy_node)?;
    let mut input = false;
    let mut output = false;

    for dummy_port in &dummy.ports {
        if dummy_port.node != dummy_node {
            continue;
        }
        if !dummy_port.outgoing_edges.is_empty() {
            output = true;
        } else if !dummy_port.incoming_edges.is_empty() {
            input = true;
        }
    }

    let position = graph.layerless_nodes[dummy_node]
        .layer_index
        .and_then(|layer| {
            graph.layers.get(layer).and_then(|layer| {
                layer
                    .nodes
                    .iter()
                    .position(|candidate| *candidate == dummy_node)
            })
        })? as f64;
    let side = graph.layerless_nodes[port.node].ports[port.port].side;

    let sum = if input && !output {
        if side == PortSide::North {
            -position
        } else {
            absurdly_large - position
        }
    } else if output && !input {
        position + 1.0
    } else if input && output {
        if side == PortSide::North {
            0.0
        } else {
            absurdly_large / 2.0
        }
    } else {
        0.0
    };
    Some(sum)
}

fn layer_node_count(graph: &LGraph, node: usize, order: &[Vec<usize>]) -> usize {
    let Some(layer_index) = graph.layerless_nodes[node].layer_index else {
        return 0;
    };
    order.get(layer_index).map_or(0, Vec::len)
}

fn port_type_for(forward: bool) -> PortType {
    if forward {
        PortType::Output
    } else {
        PortType::Input
    }
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

fn build_adjacencies(
    graph: &LGraph,
    current_node_order: &[Vec<usize>],
    free_layer_index: usize,
    side: PortSide,
) -> HashMap<usize, AdjacencyList> {
    current_node_order
        .get(free_layer_index)
        .into_iter()
        .flatten()
        .copied()
        .map(|node| {
            (
                node,
                AdjacencyList::new(graph, current_node_order, node, side),
            )
        })
        .collect()
}

fn merge_equal_adjacencies(adjacencies: Vec<Adjacency>) -> Vec<Adjacency> {
    let mut merged: Vec<Adjacency> = Vec::new();
    for adjacency in adjacencies {
        if let Some(last) = merged.last_mut()
            && last.position == adjacency.position
        {
            last.cardinality += adjacency.cardinality;
            last.current_cardinality += adjacency.current_cardinality;
            continue;
        }
        merged.push(adjacency);
    }
    merged
}

fn neighboring_layer_port_positions(
    graph: &LGraph,
    current_node_order: &[Vec<usize>],
    node: usize,
    side: PortSide,
) -> HashMap<PortRef, usize> {
    let mut positions = HashMap::new();
    let Some(layer_index) = graph.layerless_nodes[node].layer_index else {
        return positions;
    };
    let neighbor = match side {
        PortSide::West if layer_index > 0 => Some((layer_index - 1, PortSide::East)),
        PortSide::East if layer_index + 1 < current_node_order.len() => {
            Some((layer_index + 1, PortSide::West))
        }
        _ => None,
    };
    let Some((neighbor_index, neighbor_side)) = neighbor else {
        return positions;
    };

    for (position, port) in current_node_order[neighbor_index]
        .iter()
        .copied()
        .flat_map(|node| ports_in_north_south_east_west_order(graph, node, neighbor_side))
        .enumerate()
    {
        positions.insert(port, position);
    }
    positions
}

fn neighboring_layer_exists(
    current_node_order: &[Vec<usize>],
    free_layer_index: usize,
    side: PortSide,
) -> bool {
    match side {
        PortSide::West => free_layer_index > 0,
        PortSide::East => free_layer_index + 1 < current_node_order.len(),
        _ => false,
    }
}

fn edges_connected_to_side(graph: &LGraph, port: PortRef, side: PortSide) -> Vec<usize> {
    match side {
        PortSide::West => graph.layerless_nodes[port.node].ports[port.port]
            .incoming_edges
            .clone(),
        PortSide::East => graph.layerless_nodes[port.node].ports[port.port]
            .outgoing_edges
            .clone(),
        _ => connected_edges(graph, port).collect(),
    }
}

fn adjacent_port_of_side(graph: &LGraph, edge: usize, side: PortSide) -> PortRef {
    if side == PortSide::West {
        graph.edges[edge].source
    } else {
        graph.edges[edge].target
    }
}

fn is_below(first_port: usize, second_port: usize) -> bool {
    first_port > second_port
}

fn layer_position_map(layer: &[usize]) -> HashMap<usize, usize> {
    layer
        .iter()
        .copied()
        .enumerate()
        .map(|(position, node)| (node, position))
        .collect()
}

fn ports_for_counter_side(
    graph: &LGraph,
    node: usize,
    side: PortSide,
    top_down: bool,
) -> Vec<PortRef> {
    let mut ports = graph.layerless_nodes[node]
        .ports
        .iter()
        .enumerate()
        .filter_map(|(port_index, port)| {
            (port.side == side).then_some(PortRef {
                node,
                port: port_index,
            })
        })
        .collect::<Vec<_>>();
    let preserve_order =
        (side == PortSide::East && top_down) || (side == PortSide::West && !top_down);
    if !preserve_order {
        ports.reverse();
    }
    ports
}

fn connected_edges(graph: &LGraph, port: PortRef) -> impl Iterator<Item = usize> + '_ {
    graph.layerless_nodes[port.node].ports[port.port]
        .incoming_edges
        .iter()
        .chain(
            graph.layerless_nodes[port.node].ports[port.port]
                .outgoing_edges
                .iter(),
        )
        .copied()
}

fn is_edge_self_loop(graph: &LGraph, edge: usize) -> bool {
    graph
        .edges
        .get(edge)
        .is_some_and(|edge| edge.source.node == edge.target.node)
}

fn is_in_layer_edge(graph: &LGraph, edge: usize) -> bool {
    let Some(edge) = graph.edges.get(edge) else {
        return false;
    };
    graph.layerless_nodes[edge.source.node].layer_index
        == graph.layerless_nodes[edge.target.node].layer_index
}

fn other_end_of_edge(graph: &LGraph, edge: usize, port: PortRef) -> Option<PortRef> {
    let edge = graph.edges.get(edge)?;
    if edge.source == port {
        Some(edge.target)
    } else if edge.target == port {
        Some(edge.source)
    } else {
        None
    }
}

fn are_normal_and_north_south_port_dummy(
    graph: &LGraph,
    upper_node: usize,
    lower_node: usize,
) -> bool {
    (is_north_south_node(graph, upper_node)
        && graph.layerless_nodes[lower_node].kind == LNodeKind::Normal)
        || (is_north_south_node(graph, lower_node)
            && graph.layerless_nodes[upper_node].kind == LNodeKind::Normal)
}

fn have_north_south_layout_unit_guard(
    graph: &LGraph,
    upper_node: usize,
    lower_node: usize,
) -> bool {
    if graph.layerless_nodes[upper_node].kind == LNodeKind::LongEdge
        || graph.layerless_nodes[lower_node].kind == LNodeKind::LongEdge
    {
        return false;
    }

    has_edges_on_side(graph, upper_node, PortSide::North)
        || has_edges_on_side(graph, lower_node, PortSide::South)
}

fn has_edges_on_side(graph: &LGraph, node: usize, side: PortSide) -> bool {
    ports_in_north_south_east_west_order(graph, node, side)
        .into_iter()
        .any(|port| connected_edges(graph, port).next().is_some())
}

fn is_north_south_node(graph: &LGraph, node: usize) -> bool {
    graph.layerless_nodes[node].kind == LNodeKind::NorthSouthPort
}

fn is_long_edge_dummy(graph: &LGraph, node: usize) -> bool {
    graph.layerless_nodes[node].kind == LNodeKind::LongEdge
}

fn north_south_dummy_is_north_of_normal_node(graph: &LGraph, node: usize) -> bool {
    graph.layerless_nodes[node]
        .ports
        .first()
        .is_some_and(|port| port.side == PortSide::North)
}

fn number_of_north_south_edges(graph: &LGraph, node: usize, side: PortSide) -> usize {
    ports_in_north_south_east_west_order(graph, node, side)
        .into_iter()
        .filter(|port| connected_edges(graph, *port).next().is_some())
        .count()
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

fn graph_at_path<'a>(mut graph: &'a LGraph, path: &GraphPath) -> &'a LGraph {
    for index in &path.0 {
        graph = graph.layerless_nodes[*index]
            .nested_graph
            .as_deref()
            .expect("graph path should only contain nested graph nodes");
    }
    graph
}

fn graph_mut_at_path<'a>(mut graph: &'a mut LGraph, path: &GraphPath) -> &'a mut LGraph {
    for index in &path.0 {
        graph = graph.layerless_nodes[*index]
            .nested_graph
            .as_deref_mut()
            .expect("graph path should only contain nested graph nodes");
    }
    graph
}

fn fewer_than_two_in_out_parent_ports(parent_node: &LNode) -> bool {
    let east = parent_node
        .ports
        .iter()
        .filter(|port| port.side == PortSide::East)
        .count();
    let west = parent_node
        .ports
        .iter()
        .filter(|port| port.side == PortSide::West)
        .count();
    east < 2 && west < 2
}

fn has_connected_ports_on_side(graph: &LGraph, node: usize, side: PortSide) -> bool {
    ports_in_north_south_east_west_order(graph, node, side)
        .into_iter()
        .any(|port| connected_edges(graph, port).next().is_some())
}

fn sort_port_dummies_by_port_positions(
    parent_graph: &LGraph,
    child_graph: &LGraph,
    parent_node_index: usize,
    layer_close_to_node_edge: &[usize],
    side: PortSide,
) -> Vec<usize> {
    let parent_node = &parent_graph.layerless_nodes[parent_node_index];
    let mut sorted = Vec::with_capacity(layer_close_to_node_edge.len());
    for port in ports_in_north_south_east_west_order(parent_graph, parent_node_index, side) {
        let parent_port = &parent_node.ports[port.port];
        if !parent_port.inside_connections {
            continue;
        }
        if let Some(dummy) = parent_port.port_dummy.as_ref()
            && dummy.graph_id == child_graph.id
        {
            sorted.push(dummy.node);
        }
    }

    if sorted.len() == layer_close_to_node_edge.len() {
        sorted
    } else {
        layer_close_to_node_edge.to_vec()
    }
}

fn parent_port_order_by_dummy_layer(
    child_graph: &LGraph,
    dummy_layer: &[usize],
    parent_graph_id: &str,
    parent_node_index: usize,
    on_right_most_layer: bool,
) -> Vec<usize> {
    let mut order = Vec::new();
    let iter: Box<dyn Iterator<Item = usize> + '_> = if on_right_most_layer {
        Box::new(dummy_layer.iter().copied())
    } else {
        Box::new(dummy_layer.iter().copied().rev())
    };

    for dummy_node in iter {
        let Some(origin) = child_graph.layerless_nodes[dummy_node].origin_port.as_ref() else {
            continue;
        };
        if origin.graph_id == parent_graph_id && origin.port.node == parent_node_index {
            order.push(origin.port.port);
        }
    }
    order
}

fn apply_parent_hierarchical_port_order_on_sweep_side(
    parent_graph: &mut LGraph,
    parent_node_index: usize,
    on_right_most_layer: bool,
    ordered_ports: &[usize],
) {
    if ordered_ports.is_empty() {
        return;
    }

    let node = &parent_graph.layerless_nodes[parent_node_index];
    let mut new_order = Vec::with_capacity(node.ports.len());
    let mut used = vec![false; node.ports.len()];
    let mut ordered_iter = ordered_ports.iter().copied();
    let sweep_side = if on_right_most_layer {
        PortSide::East
    } else {
        PortSide::West
    };

    for (index, port) in node.ports.iter().enumerate() {
        if port.side == sweep_side && port.inside_connections {
            if let Some(next) = ordered_iter.next()
                && next < node.ports.len()
                && !used[next]
            {
                new_order.push(next);
                used[next] = true;
                continue;
            }
        }
        if !used[index] {
            new_order.push(index);
            used[index] = true;
        }
    }

    for index in 0..node.ports.len() {
        if !used[index] {
            new_order.push(index);
        }
    }

    parent_graph.reorder_node_ports(parent_node_index, new_order);
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
    use crate::options::{ElkDirection, HierarchyHandling, LayeredOptions};
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

    fn nested_node(id: &str, parent: Option<&str>) -> ElkInputNode {
        ElkInputNode {
            id: id.to_string(),
            width: 54.0,
            height: 68.0,
            parent: parent.map(str::to_string),
            direction: None,
            hierarchy_handling: None,
            layer_constraint: None,
            port_constraints: None,
            node_label_placement: crate::options::NodeLabelPlacement::Fixed,
            nested_spacing_base: None,
            label: None,
        }
    }

    fn nested_group(id: &str, parent: Option<&str>) -> ElkInputNode {
        ElkInputNode {
            id: id.to_string(),
            width: 0.0,
            height: 0.0,
            parent: parent.map(str::to_string),
            direction: None,
            hierarchy_handling: None,
            layer_constraint: None,
            port_constraints: None,
            node_label_placement: crate::options::NodeLabelPlacement::InsideTopCenter,
            nested_spacing_base: Some(30.0),
            label: None,
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

    fn node_order_by_id(graph: &LGraph, layer: &[usize]) -> Vec<String> {
        layer
            .iter()
            .map(|node| graph.layerless_nodes[*node].id.clone())
            .collect()
    }

    #[test]
    fn hierarchical_sweep_matches_source_node_port_counter_gate() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        graph
            .layerless_nodes
            .push(crate::graph::LNode::new("A", 10.0, 10.0, None));
        graph.set_node_layer(0, 0);
        graph
            .options
            .consider_model_order_crossing_counter_node_influence = 0.0;
        graph
            .options
            .consider_model_order_crossing_counter_port_influence = 1.0;

        let sweep = HierarchySweep::new(&mut graph, CrossMinType::Barycenter);

        assert!(!sweep.use_node_port_order_crossing_counter(&graph, 0));

        graph
            .options
            .consider_model_order_crossing_counter_node_influence = 1.0;

        assert!(sweep.use_node_port_order_crossing_counter(&graph, 0));
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

    #[test]
    fn parent_hierarchical_port_order_only_rewrites_sweep_side() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        graph
            .layerless_nodes
            .push(crate::graph::LNode::new("parent", 80.0, 40.0, None));
        let west_a = graph
            .add_port(0, PortType::Input, PortSide::West, Default::default())
            .unwrap()
            .port;
        let east_a = graph
            .add_port(0, PortType::Output, PortSide::East, Default::default())
            .unwrap()
            .port;
        let west_b = graph
            .add_port(0, PortType::Input, PortSide::West, Default::default())
            .unwrap()
            .port;
        let east_b = graph
            .add_port(0, PortType::Output, PortSide::East, Default::default())
            .unwrap()
            .port;
        graph.layerless_nodes[0].ports[west_a].id = "west_a".to_string();
        graph.layerless_nodes[0].ports[east_a].id = "east_a".to_string();
        graph.layerless_nodes[0].ports[west_b].id = "west_b".to_string();
        graph.layerless_nodes[0].ports[east_b].id = "east_b".to_string();
        for port in &mut graph.layerless_nodes[0].ports {
            port.inside_connections = true;
        }

        apply_parent_hierarchical_port_order_on_sweep_side(&mut graph, 0, true, &[east_b, east_a]);

        let port_ids = graph.layerless_nodes[0]
            .ports
            .iter()
            .map(|port| port.id.as_str())
            .collect::<Vec<_>>();
        let port_sides = graph.layerless_nodes[0]
            .ports
            .iter()
            .map(|port| port.side)
            .collect::<Vec<_>>();
        assert_eq!(port_ids, vec!["west_a", "east_b", "west_b", "east_a"]);
        assert_eq!(
            port_sides,
            vec![
                PortSide::West,
                PortSide::East,
                PortSide::West,
                PortSide::East
            ]
        );
    }

    #[test]
    fn north_south_hierarchical_ports_use_source_barycenter_keys() {
        let mut graph = LGraph::new("root", LayeredOptions::default());
        graph
            .layerless_nodes
            .push(crate::graph::LNode::new("parent", 80.0, 40.0, None));
        graph
            .layerless_nodes
            .push(crate::graph::LNode::new("input_dummy", 0.0, 0.0, None));
        graph.layerless_nodes[1].kind = LNodeKind::ExternalPort;
        graph
            .layerless_nodes
            .push(crate::graph::LNode::new("output_dummy", 0.0, 0.0, None));
        graph.layerless_nodes[2].kind = LNodeKind::ExternalPort;
        graph
            .layerless_nodes
            .push(crate::graph::LNode::new("both_dummy", 0.0, 0.0, None));
        graph.layerless_nodes[3].kind = LNodeKind::ExternalPort;
        graph.set_node_layer(1, 0);
        graph.set_node_layer(2, 0);
        graph.set_node_layer(3, 0);
        graph.layers[0].nodes = vec![1, 2, 3];

        let north_input = graph
            .add_port(0, PortType::Input, PortSide::North, Default::default())
            .unwrap();
        let south_input = graph
            .add_port(0, PortType::Input, PortSide::South, Default::default())
            .unwrap();
        let north_output = graph
            .add_port(0, PortType::Output, PortSide::North, Default::default())
            .unwrap();
        let south_both = graph
            .add_port(0, PortType::Output, PortSide::South, Default::default())
            .unwrap();
        for (port, dummy) in [
            (north_input, 1usize),
            (south_input, 1usize),
            (north_output, 2usize),
            (south_both, 3usize),
        ] {
            graph.layerless_nodes[0].ports[port.port].port_dummy =
                Some(crate::graph::GraphNodeRef {
                    graph_id: "root".to_string(),
                    node: dummy,
                });
        }

        let in_dummy_port = graph
            .add_port(1, PortType::Input, PortSide::South, Default::default())
            .unwrap();
        let out_dummy_port = graph
            .add_port(2, PortType::Output, PortSide::South, Default::default())
            .unwrap();
        let both_in_dummy_port = graph
            .add_port(3, PortType::Input, PortSide::South, Default::default())
            .unwrap();
        let both_out_dummy_port = graph
            .add_port(3, PortType::Output, PortSide::South, Default::default())
            .unwrap();

        graph
            .add_edge(crate::graph::LayeredEdge {
                id: "north-in".to_string(),
                source: north_input,
                target: in_dummy_port,
                source_node_id: "parent".to_string(),
                target_node_id: "input_dummy".to_string(),
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
            })
            .unwrap();
        graph
            .add_edge(crate::graph::LayeredEdge {
                id: "out".to_string(),
                source: out_dummy_port,
                target: north_output,
                source_node_id: "output_dummy".to_string(),
                target_node_id: "parent".to_string(),
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
            })
            .unwrap();
        graph
            .add_edge(crate::graph::LayeredEdge {
                id: "both-in".to_string(),
                source: south_both,
                target: both_in_dummy_port,
                source_node_id: "parent".to_string(),
                target_node_id: "both_dummy".to_string(),
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
            })
            .unwrap();
        graph
            .add_edge(crate::graph::LayeredEdge {
                id: "both-out".to_string(),
                source: both_out_dummy_port,
                target: south_both,
                source_node_id: "both_dummy".to_string(),
                target_node_id: "parent".to_string(),
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
            })
            .unwrap();

        assert_eq!(
            north_south_port_barycenter_sum(&graph, north_input, 1, 7.0),
            Some(-0.0)
        );
        assert_eq!(
            north_south_port_barycenter_sum(&graph, south_input, 1, 7.0),
            Some(7.0)
        );
        assert_eq!(
            north_south_port_barycenter_sum(&graph, north_output, 2, 7.0),
            Some(2.0)
        );
        assert_eq!(
            north_south_port_barycenter_sum(&graph, south_both, 3, 7.0),
            Some(3.5)
        );
    }

    #[test]
    fn barycenter_sweep_distributes_free_and_fixed_layer_ports() {
        let mut graph = prepared_graph(
            vec![
                node("Top"),
                node("Bottom"),
                node("Left"),
                node("Right"),
                node("Free"),
                node("Fixed"),
            ],
            vec![
                edge("Top-Free", "Top", "Free"),
                edge("Bottom-Free", "Bottom", "Free"),
                edge("Free-Right", "Free", "Right"),
                edge("Free-Left", "Free", "Left"),
                edge("Fixed-Right", "Fixed", "Right"),
                edge("Fixed-Left", "Fixed", "Left"),
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
        let free = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "Free")
            .unwrap();
        let fixed = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "Fixed")
            .unwrap();

        graph.layers[0].nodes = vec![top, bottom, fixed];
        graph.layers[1].nodes = vec![free, right, left];
        for (layer_index, layer) in graph.layers.iter().enumerate() {
            for node in &layer.nodes {
                graph.layerless_nodes[*node].layer_index = Some(layer_index);
            }
        }

        let mut graph_info = GraphInfoHolder::new(&graph);
        let port_distributor = BarycenterPortDistributor::new(PortDistributorKind::NodeRelative);
        let mut heuristic = BarycenterHeuristic::new(&graph, JavaRandom::new(1), port_distributor);

        sweep_reducing_crossings(&mut graph, &mut graph_info, &mut heuristic, true, false);

        let free_ports = graph.layerless_nodes[free]
            .ports
            .iter()
            .map(|port| port.id.as_str())
            .collect::<Vec<_>>();
        let fixed_ports = graph.layerless_nodes[fixed]
            .ports
            .iter()
            .map(|port| port.id.as_str())
            .collect::<Vec<_>>();

        assert_eq!(free_ports, vec!["Free:2", "Free:3", "Free:1", "Free:0"]);
        assert_eq!(fixed_ports, vec!["Fixed:0", "Fixed:1"]);
    }

    #[test]
    fn hierarchical_sweep_orders_nested_sources_by_child_boundary_ports() {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
            nodes: vec![
                nested_group("O", None),
                nested_group("A", Some("O")),
                nested_group("B", Some("A")),
                nested_node("b", Some("A")),
                nested_node("a", Some("A")),
                nested_node("c", Some("B")),
            ],
            edges: vec![edge("L_b_B_0", "b", "B"), edge("L_a_c_0", "a", "c")],
        })
        .unwrap();
        graph.options.hierarchy_handling = HierarchyHandling::IncludeChildren;

        crate::compound::preprocess_source_ported_compound_graph(&mut graph);
        crate::configurator::configure_graph_properties(&mut graph);
        crate::pipeline::execute_ported_compound_processors_until(
            &mut graph,
            crate::pipeline::LayeredPhase::P3NodeOrdering,
        )
        .unwrap();

        let o = graph.layerless_nodes[0].nested_graph.as_ref().unwrap();
        let a = o.layerless_nodes[0].nested_graph.as_ref().unwrap();
        assert_eq!(node_order_by_id(a, &a.layers[0].nodes), vec!["a", "b"]);
    }

    #[test]
    fn two_sided_greedy_switch_orders_layer_by_both_neighbors() {
        let mut graph = prepared_graph(
            vec![
                node("LeftTop"),
                node("LeftBottom"),
                node("MiddleTop"),
                node("MiddleBottom"),
                node("RightTop"),
                node("RightBottom"),
            ],
            vec![
                edge("LeftTop-MiddleBottom", "LeftTop", "MiddleBottom"),
                edge("LeftBottom-MiddleTop", "LeftBottom", "MiddleTop"),
                edge("MiddleTop-RightBottom", "MiddleTop", "RightBottom"),
                edge("MiddleBottom-RightTop", "MiddleBottom", "RightTop"),
            ],
        );
        let left_top = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "LeftTop")
            .unwrap();
        let left_bottom = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "LeftBottom")
            .unwrap();
        let middle_top = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "MiddleTop")
            .unwrap();
        let middle_bottom = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "MiddleBottom")
            .unwrap();
        let right_top = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "RightTop")
            .unwrap();
        let right_bottom = graph
            .layerless_nodes
            .iter()
            .position(|node| node.id == "RightBottom")
            .unwrap();
        graph.layers[0].nodes = vec![left_top, left_bottom];
        graph.layers[1].nodes = vec![middle_top, middle_bottom];
        graph.layers[2].nodes = vec![right_top, right_bottom];
        for (layer_index, layer) in graph.layers.iter().enumerate() {
            for node in &layer.nodes {
                graph.layerless_nodes[*node].layer_index = Some(layer_index);
            }
        }

        let before = CrossingsCounter::new().count_all_crossings(&graph);
        let mut order = graph
            .layers
            .iter()
            .map(|layer| layer.nodes.clone())
            .collect::<Vec<_>>();
        let mut heuristic = GreedySwitchHeuristic::new(CrossMinType::TwoSidedGreedySwitch);

        assert!(heuristic.minimize_crossings(&graph, &mut order, 1, true));
        assert_eq!(order[1], vec![middle_bottom, middle_top]);

        graph.layers[1].nodes = order[1].clone();
        let after = CrossingsCounter::new().count_all_crossings(&graph);

        assert_eq!(before, 2);
        assert_eq!(after, 0);
    }

    #[test]
    fn one_sided_greedy_switch_reduces_between_layer_crossings() {
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
        for (layer_index, layer) in graph.layers.iter().enumerate() {
            for node in &layer.nodes {
                graph.layerless_nodes[*node].layer_index = Some(layer_index);
            }
        }

        let before = CrossingsCounter::new().count_all_crossings(&graph);
        minimize_crossings_layer_sweep_with_type(&mut graph, CrossMinType::OneSidedGreedySwitch);
        let after = CrossingsCounter::new().count_all_crossings(&graph);

        assert_eq!(before, 1);
        assert_eq!(after, 0);
    }
}
