//! Phase 3 crossing counters.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p3order/counting/BinaryIndexedTree.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p3order/counting/CrossingsCounter.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/p3order/counting/AllCrossingsCounter.java

use std::collections::HashMap;

use crate::graph::{LGraph, PortRef, PortSide};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinaryIndexedTree {
    binary_sums: Vec<usize>,
    nums_per_index: Vec<usize>,
    size: usize,
}

impl BinaryIndexedTree {
    pub fn new(max_num: usize) -> Self {
        Self {
            binary_sums: vec![0; max_num + 1],
            nums_per_index: vec![0; max_num],
            size: 0,
        }
    }

    pub fn add(&mut self, index: usize) -> bool {
        if index >= self.nums_per_index.len() {
            return false;
        }

        self.size += 1;
        self.nums_per_index[index] += 1;
        let mut i = index + 1;
        while i < self.binary_sums.len() {
            self.binary_sums[i] += 1;
            i += i & i.wrapping_neg();
        }
        true
    }

    pub fn rank(&self, index: usize) -> usize {
        let mut i = index.min(self.binary_sums.len().saturating_sub(1));
        let mut sum = 0usize;
        while i > 0 {
            sum += self.binary_sums[i];
            i -= i & i.wrapping_neg();
        }
        sum
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn remove_all(&mut self, index: usize) -> bool {
        let Some(num_entries) = self.nums_per_index.get_mut(index) else {
            return false;
        };
        let num_entries = *num_entries;
        if num_entries == 0 {
            return true;
        }

        self.nums_per_index[index] = 0;
        self.size -= num_entries;
        let mut i = index + 1;
        while i < self.binary_sums.len() {
            self.binary_sums[i] -= num_entries;
            i += i & i.wrapping_neg();
        }
        true
    }

    pub fn clear(&mut self) {
        self.binary_sums.fill(0);
        self.nums_per_index.fill(0);
        self.size = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CrossingsCounter {
    port_positions: HashMap<PortRef, usize>,
}

impl CrossingsCounter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn count_crossings_between_layers(
        &mut self,
        graph: &LGraph,
        left_layer_nodes: &[usize],
        right_layer_nodes: &[usize],
    ) -> usize {
        let ports =
            self.init_port_positions_counter_clockwise(graph, left_layer_nodes, right_layer_nodes);
        let mut index_tree = BinaryIndexedTree::new(ports.len());
        self.count_crossings_on_ports(graph, &ports, &mut index_tree)
    }

    pub fn count_all_crossings(&mut self, graph: &LGraph) -> usize {
        let current_order = graph
            .layers
            .iter()
            .map(|layer| layer.nodes.clone())
            .collect::<Vec<_>>();
        self.count_all_crossings_in_order(graph, &current_order)
    }

    pub fn count_all_crossings_in_order(
        &mut self,
        graph: &LGraph,
        current_order: &[Vec<usize>],
    ) -> usize {
        if current_order.is_empty() {
            return 0;
        }

        let mut crossings =
            self.count_in_layer_crossings_on_side(graph, &current_order[0], PortSide::West);
        if current_order.len() > 1 {
            crossings += self.count_in_layer_crossings_on_side(
                graph,
                &current_order[current_order.len() - 1],
                PortSide::East,
            );
        }
        for layer_index in 0..current_order.len().saturating_sub(1) {
            crossings += self.count_crossings_between_layers(
                graph,
                &current_order[layer_index],
                &current_order[layer_index + 1],
            );
        }
        crossings
    }

    pub fn count_in_layer_crossings_on_side(
        &mut self,
        graph: &LGraph,
        nodes: &[usize],
        side: PortSide,
    ) -> usize {
        let ports = self.init_port_positions_for_in_layer_crossings(graph, nodes, side);
        let mut index_tree = BinaryIndexedTree::new(ports.len());
        self.count_in_layer_crossings_on_ports(graph, &ports, &mut index_tree)
    }

    fn init_port_positions_counter_clockwise(
        &mut self,
        graph: &LGraph,
        left_layer_nodes: &[usize],
        right_layer_nodes: &[usize],
    ) -> Vec<PortRef> {
        let mut ports = Vec::new();
        self.port_positions.clear();
        self.init_positions(graph, left_layer_nodes, &mut ports, PortSide::East, true);
        self.init_positions(graph, right_layer_nodes, &mut ports, PortSide::West, false);
        ports
    }

    fn init_positions(
        &mut self,
        graph: &LGraph,
        nodes: &[usize],
        ports: &mut Vec<PortRef>,
        side: PortSide,
        top_down: bool,
    ) {
        let node_iter: Box<dyn Iterator<Item = usize> + '_> = if top_down {
            Box::new(nodes.iter().copied())
        } else {
            Box::new(nodes.iter().rev().copied())
        };

        for node in node_iter {
            for port in ports_for_side(graph, node, side, top_down) {
                let position = ports.len();
                self.port_positions.insert(port, position);
                ports.push(port);
            }
        }
    }

    fn count_crossings_on_ports(
        &self,
        graph: &LGraph,
        ports: &[PortRef],
        index_tree: &mut BinaryIndexedTree,
    ) -> usize {
        let mut crossings = 0usize;
        let mut ends = Vec::new();

        for port in ports {
            let Some(port_position) = self.position_of(*port) else {
                continue;
            };
            index_tree.remove_all(port_position);

            for edge in connected_edges(graph, *port) {
                let Some(other_end) = other_end_of(graph, edge, *port) else {
                    continue;
                };
                let Some(end_position) = self.position_of(other_end) else {
                    continue;
                };
                if end_position > port_position {
                    crossings += index_tree.rank(end_position);
                    ends.push(end_position);
                }
            }

            while let Some(end) = ends.pop() {
                index_tree.add(end);
            }
        }

        crossings
    }

    fn init_port_positions_for_in_layer_crossings(
        &mut self,
        graph: &LGraph,
        nodes: &[usize],
        side: PortSide,
    ) -> Vec<PortRef> {
        let mut ports = Vec::new();
        self.port_positions.clear();
        for node in nodes {
            for port in ports_in_north_south_east_west_order(graph, *node, side) {
                let position = ports.len();
                self.port_positions.insert(port, position);
                ports.push(port);
            }
        }
        ports
    }

    fn count_in_layer_crossings_on_ports(
        &self,
        graph: &LGraph,
        ports: &[PortRef],
        index_tree: &mut BinaryIndexedTree,
    ) -> usize {
        let mut crossings = 0usize;
        let mut ends = Vec::new();

        for port in ports {
            let Some(port_position) = self.position_of(*port) else {
                continue;
            };
            index_tree.remove_all(port_position);
            let mut num_between_layer_edges = 0usize;

            for edge in connected_edges(graph, *port) {
                if is_in_layer_edge(graph, edge) {
                    let Some(other_end) = other_end_of(graph, edge, *port) else {
                        continue;
                    };
                    let Some(end_position) = self.position_of(other_end) else {
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

    fn position_of(&self, port: PortRef) -> Option<usize> {
        self.port_positions.get(&port).copied()
    }
}

pub fn ports_in_north_south_east_west_order(
    graph: &LGraph,
    node: usize,
    side: PortSide,
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
    if matches!(side, PortSide::South | PortSide::West) {
        ports.reverse();
    }
    ports
}

fn ports_for_side(graph: &LGraph, node: usize, side: PortSide, top_down: bool) -> Vec<PortRef> {
    let ports = graph.layerless_nodes[node]
        .ports
        .iter()
        .enumerate()
        .filter_map(|(port_index, port)| {
            (port.side == side).then_some(PortRef {
                node,
                port: port_index,
            })
        });

    let mut ports = ports.collect::<Vec<_>>();
    let preserve_order =
        (side == PortSide::East && top_down) || (side == PortSide::West && !top_down);
    if !preserve_order {
        ports.reverse();
    }
    ports
}

fn is_in_layer_edge(graph: &LGraph, edge: usize) -> bool {
    let Some(edge) = graph.edges.get(edge) else {
        return false;
    };
    graph.layerless_nodes[edge.source.node].layer_index
        == graph.layerless_nodes[edge.target.node].layer_index
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

fn other_end_of(graph: &LGraph, edge: usize, port: PortRef) -> Option<PortRef> {
    let edge = graph.edges.get(edge)?;
    if edge.source == port {
        Some(edge.target)
    } else if edge.target == port {
        Some(edge.source)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::importer::{ElkInputEdge, ElkInputGraph, ElkInputNode, import_graph};
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

    fn graph(nodes: Vec<ElkInputNode>, edges: Vec<ElkInputEdge>) -> LGraph {
        let mut graph = import_graph(&ElkInputGraph {
            id: "root".to_string(),
            options: LayeredOptions::mermaid_flowchart_defaults(ElkDirection::Down),
            nodes,
            edges,
        })
        .unwrap();
        layer_network_simplex(&mut graph);
        process_port_sides(&mut graph);
        sort_port_lists(&mut graph);
        graph
    }

    #[test]
    fn binary_indexed_tree_counts_entries_before_index() {
        let mut tree = BinaryIndexedTree::new(5);
        tree.add(3);
        tree.add(1);
        tree.add(3);

        assert_eq!(tree.rank(3), 1);
        assert_eq!(tree.rank(4), 3);
        assert_eq!(tree.size(), 3);

        tree.remove_all(3);

        assert_eq!(tree.rank(4), 1);
        assert_eq!(tree.size(), 1);
    }

    #[test]
    fn crossing_counter_counts_between_layer_inversions() {
        let mut graph = graph(
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

        let crossings = CrossingsCounter::new().count_all_crossings(&graph);

        assert_eq!(crossings, 1);
    }

    #[test]
    fn crossing_counter_reports_zero_for_non_crossing_between_layer_edges() {
        let mut graph = graph(
            vec![node("Top"), node("Bottom"), node("Left"), node("Right")],
            vec![
                edge("Top-Left", "Top", "Left"),
                edge("Bottom-Right", "Bottom", "Right"),
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

        let crossings = CrossingsCounter::new().count_all_crossings(&graph);

        assert_eq!(crossings, 0);
    }

    #[test]
    fn crossing_counter_detects_target_layer_order_improvement() {
        let mut graph = graph(
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

        let crossing_order = CrossingsCounter::new().count_all_crossings(&graph);
        graph.layers[1].nodes = vec![right, left];
        let improved_order = CrossingsCounter::new().count_all_crossings(&graph);

        assert_eq!(crossing_order, 1);
        assert_eq!(improved_order, 0);
    }
}
