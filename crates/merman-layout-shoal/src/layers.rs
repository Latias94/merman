use crate::work::{Result, Work, filled, push, reserved};
use crate::{Edge, Graph, LayoutError, NodeId};
use std::ops::Range;

/// One entry per edge occurrence in each direction, including parallel edges.
pub(crate) struct Adjacency {
    offsets: Vec<usize>,
    edges: Vec<usize>,
}

impl Adjacency {
    fn new<E>(graph: &Graph, end: impl Fn(&Edge) -> NodeId, work: &mut Work<E>) -> Result<Self, E> {
        let count = graph
            .nodes
            .len()
            .checked_add(1)
            .ok_or(LayoutError::Overflow)?;
        work.charge(count)?;
        let mut offsets = filled(count, 0usize)?;
        for edge in &graph.edges {
            work.visit(1)?;
            offsets[end(edge).0] += 1;
        }
        let mut prefix = 0;
        for count in &mut offsets[..graph.nodes.len()] {
            work.visit(1)?;
            prefix += *count; // Every prefix counts a subset of the input edges.
            *count = prefix;
        }
        offsets[graph.nodes.len()] = prefix;
        work.charge(graph.edges.len())?;
        let mut edges = filled(graph.edges.len(), 0)?;
        // Fill from the exclusive ends. The cursors finish at the slice starts, and the
        // reverse walk preserves input order within each node's incidence list.
        for (id, edge) in graph.edges.iter().enumerate().rev() {
            work.visit(1)?;
            let cursor = &mut offsets[end(edge).0];
            *cursor -= 1;
            edges[*cursor] = id;
        }
        Ok(Self { offsets, edges })
    }

    pub(crate) fn at(&self, node: usize) -> &[usize] {
        &self.edges[self.offsets[node]..self.offsets[node + 1]]
    }

    pub(crate) fn at_mut(&mut self, node: usize) -> &mut [usize] {
        &mut self.edges[self.offsets[node]..self.offsets[node + 1]]
    }
}

pub(crate) struct Layer {
    pub(crate) rank: usize,
    pub(crate) nodes: Range<usize>,
}

pub(crate) struct Layers {
    pub(crate) outgoing: Adjacency,
    pub(crate) incoming: Adjacency,
    pub(crate) order: Vec<usize>,
    pub(crate) rows: Vec<Layer>,
    pub(crate) row_of: Vec<usize>,
    pub(crate) components: Vec<Range<usize>>,
}

impl Layers {
    pub(crate) fn build<E>(graph: &Graph, work: &mut Work<E>) -> Result<Self, E> {
        let outgoing = Adjacency::new(graph, |edge| edge.source, work)?;
        let incoming = Adjacency::new(graph, |edge| edge.target, work)?;
        let ranks = longest_paths(graph, &outgoing, &incoming, work)?;
        let owners = components(graph, &outgoing, &incoming, work)?;

        let n = graph.nodes.len();
        work.charge(n)?;
        let mut order = reserved(n)?;
        order.extend(0..n);
        work.sort(&mut order, |&a, &b| {
            (owners[a], ranks[a], a).cmp(&(owners[b], ranks[b], b))
        })?;

        let mut rows = Vec::new();
        let mut components = Vec::new();
        let mut row_of = filled(n, 0)?;
        let mut row_start = 0;
        let mut component_start = 0;
        for (i, &node) in order.iter().enumerate() {
            work.visit(1)?;
            row_of[node] = rows.len();
            let end = i + 1;
            let next = order.get(end);
            let ends_component = next.is_none_or(|&next| owners[node] != owners[next]);
            if ends_component || next.is_some_and(|&next| ranks[node] != ranks[next]) {
                push(
                    &mut rows,
                    Layer {
                        rank: ranks[node],
                        nodes: row_start..end,
                    },
                )?;
                row_start = end;
            }
            if ends_component {
                push(&mut components, component_start..rows.len())?;
                component_start = rows.len();
            }
        }
        let mut layers = Self {
            outgoing,
            incoming,
            order,
            rows,
            row_of,
            components,
        };
        layers.order_neighbors(graph, work)?;
        Ok(layers)
    }

    /// Compare positions as fractions of the neighboring rank, so a wide fan does not
    /// outweigh a narrow rank solely because its last node has a larger ordinal.
    fn order_neighbors<E>(&mut self, graph: &Graph, work: &mut Work<E>) -> Result<(), E> {
        if self.rows.len() == graph.nodes.len() {
            return Ok(());
        }
        let mut position = filled(graph.nodes.len(), 0.0_f64)?;
        let mut mean = filled(graph.nodes.len(), 0.0_f64)?;
        for row in &self.rows {
            set_positions(&self.order[row.nodes.clone()], &mut position, work)?;
        }
        for forward in [true, false] {
            for i in 0..self.rows.len() {
                let row = if forward { i } else { self.rows.len() - 1 - i };
                let nodes = &mut self.order[self.rows[row].nodes.clone()];
                work.visit(1)?;
                if nodes.len() < 2 {
                    continue;
                }
                let mut constrained = false;
                for &node in nodes.iter() {
                    work.visit(1)?;
                    let edges = if forward {
                        self.incoming.at(node)
                    } else {
                        self.outgoing.at(node)
                    };
                    if edges.is_empty() {
                        mean[node] = position[node];
                        continue;
                    }
                    constrained = true;
                    let mut sum = 0.0;
                    for &id in edges {
                        work.visit(1)?;
                        let edge = &graph.edges[id];
                        let peer = if forward { edge.source } else { edge.target };
                        sum += position[peer.0];
                    }
                    mean[node] = sum / edges.len() as f64;
                }
                if constrained {
                    work.sort(nodes, |&a, &b| {
                        mean[a]
                            .total_cmp(&mean[b])
                            .then_with(|| position[a].total_cmp(&position[b]))
                    })?;
                    set_positions(nodes, &mut position, work)?;
                }
            }
        }
        Ok(())
    }
}

fn set_positions<E>(nodes: &[usize], positions: &mut [f64], work: &mut Work<E>) -> Result<(), E> {
    for (i, &node) in nodes.iter().enumerate() {
        work.visit(1)?;
        positions[node] = (i as f64 + 0.5) / nodes.len() as f64;
    }
    Ok(())
}

fn longest_paths<E>(
    graph: &Graph,
    outgoing: &Adjacency,
    incoming: &Adjacency,
    work: &mut Work<E>,
) -> Result<Vec<usize>, E> {
    let n = graph.nodes.len();
    work.charge(n)?;
    let mut rank = filled(n, 0usize)?;
    let mut remaining = reserved(n)?;
    let mut ready = reserved(n)?;
    for node in 0..n {
        work.visit(1)?;
        let degree = incoming.at(node).len();
        remaining.push(degree);
        if degree == 0 {
            ready.push(node);
        }
    }
    let mut head = 0;
    while head < ready.len() {
        work.visit(1)?;
        let node = ready[head];
        head += 1;
        for &id in outgoing.at(node) {
            work.visit(1)?;
            let edge = &graph.edges[id];
            let target = edge.target.0;
            let lower = rank[node]
                .checked_add(edge.min_rank_span.max(1))
                .ok_or(LayoutError::Overflow)?;
            rank[target] = rank[target].max(lower);
            remaining[target] -= 1;
            if remaining[target] == 0 {
                ready.push(target);
            }
        }
    }
    if head != n {
        return Err(LayoutError::Unsupported("directed cycles"));
    }
    Ok(rank)
}

fn components<E>(
    graph: &Graph,
    outgoing: &Adjacency,
    incoming: &Adjacency,
    work: &mut Work<E>,
) -> Result<Vec<usize>, E> {
    let n = graph.nodes.len();
    work.charge(n)?;
    let mut owner = filled(n, usize::MAX)?;
    let mut stack = Vec::new();
    for root in 0..n {
        work.visit(1)?;
        if owner[root] != usize::MAX {
            continue;
        }
        owner[root] = root;
        push(&mut stack, root)?;
        while let Some(node) = stack.pop() {
            work.visit(1)?;
            let successors = outgoing.at(node).iter().map(|&id| graph.edges[id].target);
            let predecessors = incoming.at(node).iter().map(|&id| graph.edges[id].source);
            for NodeId(peer) in successors.chain(predecessors) {
                work.visit(1)?;
                if owner[peer] == usize::MAX {
                    owner[peer] = root;
                    push(&mut stack, peer)?;
                }
            }
        }
    }
    Ok(owner)
}
