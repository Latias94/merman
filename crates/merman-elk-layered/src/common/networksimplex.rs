//! Common network simplex model and algorithm.
//!
//! Source references:
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.common/src/org/eclipse/elk/alg/common/networksimplex/NGraph.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.common/src/org/eclipse/elk/alg/common/networksimplex/NNode.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.common/src/org/eclipse/elk/alg/common/networksimplex/NEdge.java
//! - https://github.com/eclipse-elk/elk/blob/62d5909f96fad541bc101ad52dabaece6b7eab7e/plugins/org.eclipse.elk.alg.common/src/org/eclipse/elk/alg/common/networksimplex/NetworkSimplex.java

use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NetworkSimplexError {
    #[error("network simplex coordinate is outside the supported i32 range")]
    CoordinateOutOfRange,
    #[error("network simplex cannot allocate layer counts")]
    LayerCountAllocation,
}

type SimplexResult<T> = Result<T, NetworkSimplexError>;

pub(crate) fn coordinate(value: i64) -> SimplexResult<i32> {
    i32::try_from(value).map_err(|_| NetworkSimplexError::CoordinateOutOfRange)
}

const REMOVE_SUBTREES_THRESH: usize = 40;
const FUZZY_ST_ZERO: f64 = -1e-10;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NGraph {
    pub nodes: Vec<NNode>,
    pub edges: Vec<NEdge>,
    node_order: Vec<usize>,
}

impl NGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, origin: Option<usize>) -> usize {
        let node_index = self.nodes.len();
        self.nodes.push(NNode {
            origin,
            ..NNode::default()
        });
        self.node_order.push(node_index);
        node_index
    }

    pub fn add_edge(
        &mut self,
        origin: Option<usize>,
        source: usize,
        target: usize,
        weight: f64,
        delta: i32,
    ) -> Option<usize> {
        if source == target || self.nodes.get(source).is_none() || self.nodes.get(target).is_none()
        {
            return None;
        }

        let edge_index = self.edges.len();
        self.edges.push(NEdge {
            origin,
            source,
            target,
            weight,
            delta,
            ..NEdge::default()
        });
        self.nodes[source].outgoing_edges.push(edge_index);
        self.nodes[target].incoming_edges.push(edge_index);
        Some(edge_index)
    }

    pub fn active_nodes(&self) -> &[usize] {
        &self.node_order
    }

    pub fn connected_edges(&self, node: usize) -> Vec<usize> {
        self.nodes[node]
            .incoming_edges
            .iter()
            .chain(self.nodes[node].outgoing_edges.iter())
            .copied()
            .collect()
    }

    fn remove_active_node(&mut self, node: usize) {
        if let Some(position) = self
            .node_order
            .iter()
            .position(|candidate| *candidate == node)
        {
            self.node_order.remove(position);
        }
    }

    fn add_active_node(&mut self, node: usize) {
        self.node_order.push(node);
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NNode {
    pub id: usize,
    pub internal_id: usize,
    pub origin: Option<usize>,
    pub layer: i32,
    pub incoming_edges: Vec<usize>,
    pub outgoing_edges: Vec<usize>,
    tree_node: bool,
    unknown_cutvalues: Vec<usize>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NEdge {
    pub id: usize,
    pub internal_id: usize,
    pub origin: Option<usize>,
    pub source: usize,
    pub target: usize,
    pub weight: f64,
    pub delta: i32,
    tree_edge: bool,
}

impl NEdge {
    fn other(&self, node: usize) -> Option<usize> {
        if node == self.source {
            Some(self.target)
        } else if node == self.target {
            Some(self.source)
        } else {
            None
        }
    }
}

pub struct NetworkSimplex<'a> {
    graph: &'a mut NGraph,
    previous_layering_node_counts: Option<Vec<usize>>,
    balance: bool,
    iteration_limit: usize,
    edges: Vec<usize>,
    tree_edges: Vec<usize>,
    sources: Vec<usize>,
    edge_visited: Vec<bool>,
    post_order: i32,
    po_id: Vec<i32>,
    lowest_po_id: Vec<i32>,
    cutvalue: Vec<f64>,
    subtree_nodes_stack: Vec<(usize, usize)>,
}

impl<'a> NetworkSimplex<'a> {
    pub fn for_graph(graph: &'a mut NGraph) -> Self {
        Self {
            graph,
            previous_layering_node_counts: None,
            balance: false,
            iteration_limit: usize::MAX,
            edges: Vec::new(),
            tree_edges: Vec::new(),
            sources: Vec::new(),
            edge_visited: Vec::new(),
            post_order: 1,
            po_id: Vec::new(),
            lowest_po_id: Vec::new(),
            cutvalue: Vec::new(),
            subtree_nodes_stack: Vec::new(),
        }
    }

    pub fn with_balancing(mut self, balance: bool) -> Self {
        self.balance = balance;
        self
    }

    pub fn with_previous_layering(mut self, counts: Option<Vec<usize>>) -> Self {
        self.previous_layering_node_counts = counts;
        self
    }

    pub fn with_iteration_limit(mut self, limit: usize) -> Self {
        self.iteration_limit = limit;
        self
    }

    pub fn execute(mut self) -> SimplexResult<()> {
        if self.graph.node_order.is_empty() {
            return Ok(());
        }

        for node in self.graph.node_order.iter().copied() {
            self.graph.nodes[node].layer = 0;
        }

        let remove_subtrees = self.graph.node_order.len() >= REMOVE_SUBTREES_THRESH;
        if remove_subtrees {
            self.remove_subtrees();
        }

        self.initialize();
        self.feasible_tree()?;

        let mut leave = self.leave_edge();
        let mut iteration = 0usize;
        while let Some(leave_edge) = leave {
            if iteration >= self.iteration_limit {
                break;
            }
            if let Some(enter_edge) = self.enter_edge(leave_edge) {
                self.exchange(leave_edge, enter_edge)?;
            } else {
                break;
            }
            leave = self.leave_edge();
            iteration += 1;
        }

        if remove_subtrees {
            self.reattach_subtrees()?;
        }

        self.normalize()?;
        if self.balance {
            let filling = self.layer_counts()?;
            self.balance(filling);
        }
        Ok(())
    }

    fn initialize(&mut self) {
        let active_nodes = self.graph.node_order.clone();
        for node in &active_nodes {
            self.graph.nodes[*node].tree_node = false;
        }

        self.po_id = vec![0; active_nodes.len()];
        self.lowest_po_id = vec![0; active_nodes.len()];
        self.sources.clear();

        let mut edges = Vec::new();
        for (index, node) in active_nodes.iter().copied().enumerate() {
            self.graph.nodes[node].internal_id = index;
            if self.graph.nodes[node].incoming_edges.is_empty() {
                self.sources.push(node);
            }
            edges.extend(self.graph.nodes[node].outgoing_edges.iter().copied());
        }

        for (internal_id, edge) in edges.iter().copied().enumerate() {
            self.graph.edges[edge].internal_id = internal_id;
            self.graph.edges[edge].tree_edge = false;
        }

        self.cutvalue = vec![0.0; edges.len()];
        self.edge_visited = vec![false; edges.len()];
        self.edges = edges;
        self.tree_edges.clear();
        self.post_order = 1;
    }

    fn remove_subtrees(&mut self) {
        self.subtree_nodes_stack.clear();

        let mut leafs = VecDeque::new();
        for node in self.graph.node_order.iter().copied() {
            if self.graph.connected_edges(node).len() == 1 {
                leafs.push_back(node);
            }
        }

        while let Some(node) = leafs.pop_front() {
            let connected_edges = self.graph.connected_edges(node);
            if connected_edges.is_empty() {
                continue;
            }

            let edge = connected_edges[0];
            let is_out_edge = !self.graph.nodes[node].outgoing_edges.is_empty();
            let Some(other) = self.graph.edges[edge].other(node) else {
                continue;
            };

            if is_out_edge {
                remove_item(&mut self.graph.nodes[other].incoming_edges, edge);
            } else {
                remove_item(&mut self.graph.nodes[other].outgoing_edges, edge);
            }

            if self.graph.connected_edges(other).len() == 1 {
                leafs.push_back(other);
            }

            self.subtree_nodes_stack.push((node, edge));
            self.graph.remove_active_node(node);
        }
    }

    fn reattach_subtrees(&mut self) -> SimplexResult<()> {
        while let Some((node, edge)) = self.subtree_nodes_stack.pop() {
            let Some(placed) = self.graph.edges[edge].other(node) else {
                continue;
            };

            if self.graph.edges[edge].target == node {
                self.graph.nodes[placed].outgoing_edges.push(edge);
                self.graph.nodes[node].layer = coordinate(
                    i64::from(self.graph.nodes[placed].layer)
                        + i64::from(self.graph.edges[edge].delta),
                )?;
            } else {
                self.graph.nodes[placed].incoming_edges.push(edge);
                self.graph.nodes[node].layer = coordinate(
                    i64::from(self.graph.nodes[placed].layer)
                        - i64::from(self.graph.edges[edge].delta),
                )?;
            }

            self.graph.add_active_node(node);
        }
        Ok(())
    }

    fn feasible_tree(&mut self) -> SimplexResult<()> {
        self.layering_topological_numbering()?;

        if self.edges.is_empty() {
            return Ok(());
        }

        self.edge_visited.fill(false);
        while self.tight_tree_dfs(self.graph.node_order[0]) < self.graph.node_order.len() {
            let Some(edge) = self.minimal_slack() else {
                break;
            };
            let mut slack = self.graph.edges[edge].target_layer(self.graph)
                - self.graph.edges[edge].source_layer(self.graph)
                - i64::from(self.graph.edges[edge].delta);
            if self.graph.nodes[self.graph.edges[edge].target].tree_node {
                slack = -slack;
            }

            for node in self.graph.node_order.iter().copied() {
                if self.graph.nodes[node].tree_node {
                    self.graph.nodes[node].layer =
                        coordinate(i64::from(self.graph.nodes[node].layer) + slack)?;
                }
            }
            self.edge_visited.fill(false);
        }

        self.edge_visited.fill(false);
        self.postorder_traversal(self.graph.node_order[0]);
        self.cutvalues();
        Ok(())
    }

    fn layering_topological_numbering(&mut self) -> SimplexResult<()> {
        let mut incident = vec![0usize; self.graph.node_order.len()];
        for node in self.graph.node_order.iter().copied() {
            incident[self.graph.nodes[node].internal_id] +=
                self.graph.nodes[node].incoming_edges.len();
        }

        let mut roots = VecDeque::from(self.sources.clone());
        while let Some(node) = roots.pop_front() {
            let outgoing_edges = self.graph.nodes[node].outgoing_edges.clone();
            for edge in outgoing_edges {
                let target = self.graph.edges[edge].target;
                self.graph.nodes[target].layer = self.graph.nodes[target].layer.max(coordinate(
                    i64::from(self.graph.nodes[node].layer)
                        + i64::from(self.graph.edges[edge].delta),
                )?);
                let target_id = self.graph.nodes[target].internal_id;
                incident[target_id] = incident[target_id].saturating_sub(1);
                if incident[target_id] == 0 {
                    roots.push_back(target);
                }
            }
        }
        Ok(())
    }

    fn tight_tree_dfs(&mut self, node: usize) -> usize {
        let mut node_count = 1;
        self.graph.nodes[node].tree_node = true;

        for edge in self.graph.connected_edges(node) {
            let edge_id = self.graph.edges[edge].internal_id;
            if self.edge_visited[edge_id] {
                continue;
            }
            self.edge_visited[edge_id] = true;
            let Some(opposite) = self.graph.edges[edge].other(node) else {
                continue;
            };

            if self.graph.edges[edge].tree_edge {
                node_count += self.tight_tree_dfs(opposite);
            } else if !self.graph.nodes[opposite].tree_node
                && i64::from(self.graph.edges[edge].delta)
                    == self.graph.edges[edge].target_layer(self.graph)
                        - self.graph.edges[edge].source_layer(self.graph)
            {
                self.graph.edges[edge].tree_edge = true;
                push_unique(&mut self.tree_edges, edge);
                node_count += self.tight_tree_dfs(opposite);
            }
        }

        node_count
    }

    fn minimal_slack(&self) -> Option<usize> {
        let mut min_slack = i64::MAX;
        let mut min_slack_edge = None;

        for edge in &self.edges {
            let source = self.graph.edges[*edge].source;
            let target = self.graph.edges[*edge].target;
            if self.graph.nodes[source].tree_node ^ self.graph.nodes[target].tree_node {
                let slack = i64::from(self.graph.nodes[target].layer)
                    - i64::from(self.graph.nodes[source].layer)
                    - i64::from(self.graph.edges[*edge].delta);
                if slack < min_slack {
                    min_slack = slack;
                    min_slack_edge = Some(*edge);
                }
            }
        }

        min_slack_edge
    }

    fn postorder_traversal(&mut self, node: usize) -> i32 {
        let mut lowest = i32::MAX;
        for edge in self.graph.connected_edges(node) {
            let edge_id = self.graph.edges[edge].internal_id;
            if self.graph.edges[edge].tree_edge && !self.edge_visited[edge_id] {
                self.edge_visited[edge_id] = true;
                let opposite = self.graph.edges[edge]
                    .other(node)
                    .expect("tree edge should connect to current node");
                lowest = lowest.min(self.postorder_traversal(opposite));
            }
        }

        let node_id = self.graph.nodes[node].internal_id;
        self.po_id[node_id] = self.post_order;
        self.lowest_po_id[node_id] = lowest.min(self.post_order);
        self.post_order += 1;
        self.lowest_po_id[node_id]
    }

    fn is_in_head(&self, node: usize, edge: usize) -> bool {
        let source = self.graph.edges[edge].source;
        let target = self.graph.edges[edge].target;
        let source_id = self.graph.nodes[source].internal_id;
        let target_id = self.graph.nodes[target].internal_id;
        let node_id = self.graph.nodes[node].internal_id;

        if self.lowest_po_id[source_id] <= self.po_id[node_id]
            && self.po_id[node_id] <= self.po_id[source_id]
            && self.lowest_po_id[target_id] <= self.po_id[node_id]
            && self.po_id[node_id] <= self.po_id[target_id]
        {
            return self.po_id[source_id] >= self.po_id[target_id];
        }

        self.po_id[source_id] < self.po_id[target_id]
    }

    fn cutvalues(&mut self) {
        let mut leafs = Vec::new();

        for node in self.graph.node_order.iter().copied() {
            let mut tree_edge_count = 0usize;
            self.graph.nodes[node].unknown_cutvalues.clear();
            for edge in self.graph.connected_edges(node) {
                if self.graph.edges[edge].tree_edge {
                    self.graph.nodes[node].unknown_cutvalues.push(edge);
                    tree_edge_count += 1;
                }
            }
            if tree_edge_count == 1 {
                leafs.push(node);
            }
        }

        for start in leafs {
            let mut node = start;
            while self.graph.nodes[node].unknown_cutvalues.len() == 1 {
                let to_determine = self.graph.nodes[node].unknown_cutvalues[0];
                let to_determine_id = self.graph.edges[to_determine].internal_id;
                self.cutvalue[to_determine_id] = self.graph.edges[to_determine].weight;
                let source = self.graph.edges[to_determine].source;
                let target = self.graph.edges[to_determine].target;

                for edge in self.graph.connected_edges(node) {
                    if edge == to_determine {
                        continue;
                    }

                    let edge_id = self.graph.edges[edge].internal_id;
                    if self.graph.edges[edge].tree_edge {
                        if source == self.graph.edges[edge].source
                            || target == self.graph.edges[edge].target
                        {
                            self.cutvalue[to_determine_id] -=
                                self.cutvalue[edge_id] - self.graph.edges[edge].weight;
                        } else {
                            self.cutvalue[to_determine_id] +=
                                self.cutvalue[edge_id] - self.graph.edges[edge].weight;
                        }
                    } else if node == source {
                        if self.graph.edges[edge].source == node {
                            self.cutvalue[to_determine_id] += self.graph.edges[edge].weight;
                        } else {
                            self.cutvalue[to_determine_id] -= self.graph.edges[edge].weight;
                        }
                    } else if self.graph.edges[edge].source == node {
                        self.cutvalue[to_determine_id] -= self.graph.edges[edge].weight;
                    } else {
                        self.cutvalue[to_determine_id] += self.graph.edges[edge].weight;
                    }
                }

                remove_item(
                    &mut self.graph.nodes[source].unknown_cutvalues,
                    to_determine,
                );
                remove_item(
                    &mut self.graph.nodes[target].unknown_cutvalues,
                    to_determine,
                );

                node = if source == node { target } else { source };
            }
        }
    }

    fn leave_edge(&self) -> Option<usize> {
        self.tree_edges.iter().copied().find(|edge| {
            self.graph.edges[*edge].tree_edge
                && self.cutvalue[self.graph.edges[*edge].internal_id] < FUZZY_ST_ZERO
        })
    }

    fn enter_edge(&self, leave: usize) -> Option<usize> {
        if !self.graph.edges[leave].tree_edge {
            return None;
        }

        let mut replacement = None;
        let mut replacement_slack = i64::MAX;

        for edge in &self.edges {
            let source = self.graph.edges[*edge].source;
            let target = self.graph.edges[*edge].target;
            if self.is_in_head(source, leave) && !self.is_in_head(target, leave) {
                let slack = i64::from(self.graph.nodes[target].layer)
                    - i64::from(self.graph.nodes[source].layer)
                    - i64::from(self.graph.edges[*edge].delta);
                if slack < replacement_slack {
                    replacement_slack = slack;
                    replacement = Some(*edge);
                }
            }
        }

        replacement
    }

    fn exchange(&mut self, leave: usize, enter: usize) -> SimplexResult<()> {
        if !self.graph.edges[leave].tree_edge || self.graph.edges[enter].tree_edge {
            return Ok(());
        }

        self.graph.edges[leave].tree_edge = false;
        remove_item(&mut self.tree_edges, leave);
        self.graph.edges[enter].tree_edge = true;
        push_unique(&mut self.tree_edges, enter);

        let mut delta = self.graph.edges[enter].target_layer(self.graph)
            - self.graph.edges[enter].source_layer(self.graph)
            - i64::from(self.graph.edges[enter].delta);
        if !self.is_in_head(self.graph.edges[enter].target, leave) {
            delta = -delta;
        }

        for node in self.graph.node_order.clone() {
            if !self.is_in_head(node, leave) {
                self.graph.nodes[node].layer =
                    coordinate(i64::from(self.graph.nodes[node].layer) + delta)?;
            }
        }

        self.post_order = 1;
        self.edge_visited.fill(false);
        self.postorder_traversal(self.graph.node_order[0]);
        self.cutvalues();
        Ok(())
    }

    // Node placement uses layers as coordinates, not ordinal layer indices. Its
    // memory use must depend on graph size, never on the coordinate span.
    fn normalize(&mut self) -> SimplexResult<()> {
        let lowest = self
            .graph
            .node_order
            .iter()
            .map(|&node| self.graph.nodes[node].layer)
            .min()
            .unwrap_or(0);
        for node in self.graph.node_order.iter().copied() {
            self.graph.nodes[node].layer =
                coordinate(i64::from(self.graph.nodes[node].layer) - i64::from(lowest))?;
        }
        Ok(())
    }

    fn layer_counts(&self) -> SimplexResult<Vec<usize>> {
        let highest = self
            .graph
            .node_order
            .iter()
            .map(|&node| self.graph.nodes[node].layer as usize)
            .max()
            .unwrap_or(0);
        let count = highest
            .checked_add(1)
            .ok_or(NetworkSimplexError::LayerCountAllocation)?;
        let mut filling = Vec::new();
        filling
            .try_reserve_exact(count)
            .map_err(|_| NetworkSimplexError::LayerCountAllocation)?;
        filling.resize(count, 0usize);
        for node in self.graph.node_order.iter().copied() {
            filling[self.graph.nodes[node].layer as usize] += 1;
        }
        if let Some(previous) = self.previous_layering_node_counts.as_ref() {
            for (count, previous) in filling.iter_mut().zip(previous) {
                *count = count
                    .checked_add(*previous)
                    .ok_or(NetworkSimplexError::LayerCountAllocation)?;
            }
        }
        Ok(filling)
    }

    fn balance(&mut self, mut filling: Vec<usize>) {
        for node in self.graph.node_order.clone() {
            if self.graph.nodes[node].incoming_edges.len()
                != self.graph.nodes[node].outgoing_edges.len()
            {
                continue;
            }

            let mut new_layer = self.graph.nodes[node].layer;
            let (min_span_in, min_span_out) = self.minimal_span(node);
            let start = i64::from(self.graph.nodes[node].layer) - min_span_in + 1;
            let end = i64::from(self.graph.nodes[node].layer) + min_span_out;
            for layer in start.max(0)..end.min(filling.len() as i64) {
                let layer = layer as usize;
                let current = new_layer as usize;
                if layer < filling.len() && filling[layer] < filling[current] {
                    new_layer = layer as i32;
                }
            }

            let old_layer = self.graph.nodes[node].layer as usize;
            let new_layer_index = new_layer as usize;
            if new_layer_index < filling.len() && filling[new_layer_index] < filling[old_layer] {
                filling[old_layer] = filling[old_layer].saturating_sub(1);
                filling[new_layer_index] += 1;
                self.graph.nodes[node].layer = new_layer;
            }
        }
    }

    fn minimal_span(&self, node: usize) -> (i64, i64) {
        let mut min_span_out = i64::MAX;
        let mut min_span_in = i64::MAX;

        for edge in self.graph.connected_edges(node) {
            let span = self.graph.edges[edge].target_layer(self.graph)
                - self.graph.edges[edge].source_layer(self.graph);
            if self.graph.edges[edge].target == node && span < min_span_in {
                min_span_in = span;
            } else if span < min_span_out {
                min_span_out = span;
            }
        }

        if min_span_in == i64::MAX {
            min_span_in = -1;
        }
        if min_span_out == i64::MAX {
            min_span_out = -1;
        }
        (min_span_in, min_span_out)
    }
}

impl NEdge {
    fn source_layer(&self, graph: &NGraph) -> i64 {
        i64::from(graph.nodes[self.source].layer)
    }

    fn target_layer(&self, graph: &NGraph) -> i64 {
        i64::from(graph.nodes[self.target].layer)
    }
}

fn remove_item<T: PartialEq>(items: &mut Vec<T>, item: T) {
    if let Some(position) = items.iter().position(|candidate| *candidate == item) {
        items.remove(position);
    }
}

fn push_unique(items: &mut Vec<usize>, item: usize) {
    if !items.contains(&item) {
        items.push(item);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_coordinates_do_not_allocate_a_dense_layer_count() {
        let mut graph = NGraph::new();
        let top = graph.add_node(None);
        let bottom = graph.add_node(None);
        graph
            .add_edge(None, top, bottom, 10_000.0, i32::MAX)
            .unwrap();

        NetworkSimplex::for_graph(&mut graph)
            .with_balancing(false)
            .execute()
            .unwrap();

        assert_eq!(graph.nodes[top].layer, 0);
        assert_eq!(graph.nodes[bottom].layer, i32::MAX);
    }

    #[test]
    fn coordinate_overflow_is_an_error_instead_of_wrapped_geometry() {
        let mut graph = NGraph::new();
        let a = graph.add_node(None);
        let b = graph.add_node(None);
        let c = graph.add_node(None);
        graph.add_edge(None, a, b, 1.0, i32::MAX).unwrap();
        graph.add_edge(None, b, c, 1.0, 1).unwrap();

        assert_eq!(
            NetworkSimplex::for_graph(&mut graph).execute(),
            Err(NetworkSimplexError::CoordinateOutOfRange),
        );
    }

    #[test]
    fn network_simplex_assigns_layers_to_dag() {
        let mut graph = NGraph::new();
        let a = graph.add_node(Some(0));
        let b = graph.add_node(Some(1));
        let c = graph.add_node(Some(2));
        graph.add_edge(Some(0), a, b, 1.0, 1).unwrap();
        graph.add_edge(Some(1), b, c, 1.0, 1).unwrap();

        NetworkSimplex::for_graph(&mut graph)
            .with_iteration_limit(28)
            .with_balancing(true)
            .execute()
            .unwrap();

        assert_eq!(graph.nodes[a].layer, 0);
        assert!(graph.nodes[b].layer > graph.nodes[a].layer);
        assert!(graph.nodes[c].layer > graph.nodes[b].layer);
    }

    #[test]
    fn network_simplex_balances_equal_degree_node() {
        let mut graph = NGraph::new();
        let a = graph.add_node(Some(0));
        let b = graph.add_node(Some(1));
        let c = graph.add_node(Some(2));
        graph.add_edge(Some(0), a, c, 1.0, 1).unwrap();
        graph.add_edge(Some(1), b, c, 1.0, 1).unwrap();

        NetworkSimplex::for_graph(&mut graph)
            .with_iteration_limit(28)
            .with_balancing(true)
            .execute()
            .unwrap();

        assert_eq!(graph.nodes[a].layer, 0);
        assert_eq!(graph.nodes[b].layer, 0);
        assert!(graph.nodes[c].layer > graph.nodes[a].layer);
    }

    #[test]
    fn network_simplex_reattaches_removed_subtrees() {
        let mut graph = NGraph::new();
        let mut nodes = Vec::new();
        for index in 0..41 {
            nodes.push(graph.add_node(Some(index)));
        }
        for index in 0..40 {
            graph
                .add_edge(Some(index), nodes[index], nodes[index + 1], 1.0, 1)
                .unwrap();
        }

        NetworkSimplex::for_graph(&mut graph)
            .with_iteration_limit(28)
            .with_balancing(true)
            .execute()
            .unwrap();

        assert_eq!(graph.active_nodes().len(), 41);
        for edge in &graph.edges {
            assert!(graph.nodes[edge.target].layer > graph.nodes[edge.source].layer);
        }
    }
}
