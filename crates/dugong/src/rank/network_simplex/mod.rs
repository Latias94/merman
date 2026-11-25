//! Network simplex ranker (Dagre-compatible).

use super::{RankError, feasible_tree, tree, util};
use crate::graphlib::{EdgeKey, Graph, alg};
use crate::work::{checked_add, checked_mul, checked_n_log_n, checked_ordered_key_updates};
use crate::{EdgeLabel, GraphLabel, NodeLabel};
use crate::{NoopWorkControl, WorkControl, WorkError};

mod edges;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DfsFrame {
    v_ix: usize,
    parent_ix: Option<usize>,
    low: i32,
    next_neighbor: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OrderedTreeEdge {
    position: usize,
    v_ix: usize,
    w_ix: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DenseTreeEdge {
    v_t_ix: usize,
    w_t_ix: usize,
    tail_t_ix: usize,
    head_t_ix: usize,
    minlen: usize,
    weight: f64,
}

fn capture_dense_tree_edge(
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    t_ix_by_g_ix: &[Option<usize>],
    v_g_ix: usize,
    w_g_ix: usize,
) -> Option<DenseTreeEdge> {
    if v_g_ix == w_g_ix {
        return None;
    }
    let mut v_t_ix = t_ix_by_g_ix.get(v_g_ix).copied().flatten()?;
    let mut w_t_ix = t_ix_by_g_ix.get(w_g_ix).copied().flatten()?;
    let mut v = graph.node_id_by_ix(v_g_ix)?;
    let mut w = graph.node_id_by_ix(w_g_ix)?;
    // The old undirected Graph canonicalized endpoints by ID, independently of discovery order.
    if v > w {
        std::mem::swap(&mut v, &mut w);
        std::mem::swap(&mut v_t_ix, &mut w_t_ix);
    }
    let (tail_t_ix, head_t_ix, label) = graph
        .edge(v, w, None)
        .map(|label| (v_t_ix, w_t_ix, label))
        .or_else(|| graph.edge(w, v, None).map(|label| (w_t_ix, v_t_ix, label)))?;
    Some(DenseTreeEdge {
        v_t_ix,
        w_t_ix,
        tail_t_ix,
        head_t_ix,
        minlen: label.minlen,
        weight: label.weight,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct NeighborBuildState {
    predecessor_total: usize,
    predecessor_ordinary_cursor: usize,
    successor_ordinary_cursor: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NumericNeighborEntry {
    owner_ix: usize,
    successor: bool,
    array_index: u32,
    neighbor_ix: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct PinnedUndirectedNeighbors {
    offsets: Vec<usize>,
    build_state: Vec<NeighborBuildState>,
    entries: Vec<usize>,
    numeric_entries: Vec<NumericNeighborEntry>,
}

impl PinnedUndirectedNeighbors {
    fn rebuild(
        &mut self,
        tree_edges_in_order: &[DenseTreeEdge],
        array_index_by_node_ix: &[Option<u32>],
        array_index_adjacency_entries: usize,
    ) -> Result<(), RankError> {
        let node_slots = array_index_by_node_ix.len();
        let offsets_len = checked_add(node_slots, 1)?;
        let mut planned_entry_count = 0usize;
        let mut planned_numeric_entry_count = 0usize;
        for edge in tree_edges_in_order {
            let (v_ix, w_ix) = (edge.v_t_ix, edge.w_t_ix);
            if v_ix >= node_slots || w_ix >= node_slots || v_ix == w_ix {
                return Err(RankError::InvalidNetworkSimplexTree);
            }
            planned_entry_count = checked_add(planned_entry_count, 2)?;
            planned_numeric_entry_count = checked_add(
                planned_numeric_entry_count,
                usize::from(array_index_by_node_ix[v_ix].is_some()),
            )?;
            planned_numeric_entry_count = checked_add(
                planned_numeric_entry_count,
                usize::from(array_index_by_node_ix[w_ix].is_some()),
            )?;
        }
        if planned_numeric_entry_count != array_index_adjacency_entries {
            return Err(RankError::InvalidNetworkSimplexTree);
        }

        self.offsets.resize(offsets_len, 0);
        self.offsets.fill(0);
        self.build_state
            .resize(node_slots, NeighborBuildState::default());
        self.build_state.fill(NeighborBuildState::default());

        for edge in tree_edges_in_order {
            let (v_ix, w_ix) = (edge.v_t_ix, edge.w_t_ix);
            let w_offset_ix = checked_add(w_ix, 1)?;
            self.offsets[w_offset_ix] = checked_add(self.offsets[w_offset_ix], 1)?;
            let predecessor = &mut self.build_state[w_ix];
            predecessor.predecessor_total = checked_add(predecessor.predecessor_total, 1)?;
            predecessor.predecessor_ordinary_cursor = checked_add(
                predecessor.predecessor_ordinary_cursor,
                usize::from(array_index_by_node_ix[v_ix].is_some()),
            )?;

            let v_offset_ix = checked_add(v_ix, 1)?;
            self.offsets[v_offset_ix] = checked_add(self.offsets[v_offset_ix], 1)?;
            let successor = &mut self.build_state[v_ix];
            successor.successor_ordinary_cursor = checked_add(
                successor.successor_ordinary_cursor,
                usize::from(array_index_by_node_ix[w_ix].is_some()),
            )?;
        }

        for node_ix in 0..node_slots {
            let next_node_ix = checked_add(node_ix, 1)?;
            self.offsets[next_node_ix] =
                checked_add(self.offsets[next_node_ix], self.offsets[node_ix])?;
            let state = &mut self.build_state[node_ix];
            let predecessor_numeric = state.predecessor_ordinary_cursor;
            let successor_numeric = state.successor_ordinary_cursor;
            state.predecessor_ordinary_cursor =
                checked_add(self.offsets[node_ix], predecessor_numeric)?;
            state.successor_ordinary_cursor = checked_add(
                checked_add(self.offsets[node_ix], state.predecessor_total)?,
                successor_numeric,
            )?;
        }
        if self.offsets.last().copied().unwrap_or(0) != planned_entry_count {
            return Err(RankError::InvalidNetworkSimplexTree);
        }
        self.entries.resize(planned_entry_count, 0);

        self.numeric_entries.clear();
        self.numeric_entries.reserve(array_index_adjacency_entries);
        for edge in tree_edges_in_order {
            let (v_ix, w_ix) = (edge.v_t_ix, edge.w_t_ix);
            if let Some(array_index) = array_index_by_node_ix[v_ix] {
                self.numeric_entries.push(NumericNeighborEntry {
                    owner_ix: w_ix,
                    successor: false,
                    array_index,
                    neighbor_ix: v_ix,
                });
            } else {
                let state = &mut self.build_state[w_ix];
                let Some(entry) = self.entries.get_mut(state.predecessor_ordinary_cursor) else {
                    return Err(RankError::InvalidNetworkSimplexTree);
                };
                *entry = v_ix;
                state.predecessor_ordinary_cursor =
                    checked_add(state.predecessor_ordinary_cursor, 1)?;
            }

            if let Some(array_index) = array_index_by_node_ix[w_ix] {
                self.numeric_entries.push(NumericNeighborEntry {
                    owner_ix: v_ix,
                    successor: true,
                    array_index,
                    neighbor_ix: w_ix,
                });
            } else {
                let state = &mut self.build_state[v_ix];
                let Some(entry) = self.entries.get_mut(state.successor_ordinary_cursor) else {
                    return Err(RankError::InvalidNetworkSimplexTree);
                };
                *entry = w_ix;
                state.successor_ordinary_cursor = checked_add(state.successor_ordinary_cursor, 1)?;
            }
        }

        self.numeric_entries
            .sort_unstable_by_key(|entry| (entry.owner_ix, entry.successor, entry.array_index));
        let mut current_bucket = None;
        let mut numeric_cursor = 0usize;
        for entry in &self.numeric_entries {
            let bucket = (entry.owner_ix, entry.successor);
            if current_bucket != Some(bucket) {
                current_bucket = Some(bucket);
                numeric_cursor = checked_add(
                    self.offsets[entry.owner_ix],
                    if entry.successor {
                        self.build_state[entry.owner_ix].predecessor_total
                    } else {
                        0
                    },
                )?;
            }
            let Some(slot) = self.entries.get_mut(numeric_cursor) else {
                return Err(RankError::InvalidNetworkSimplexTree);
            };
            *slot = entry.neighbor_ix;
            numeric_cursor = checked_add(numeric_cursor, 1)?;
        }
        Ok(())
    }

    fn get(&self, node_ix: usize, position: usize) -> Option<usize> {
        let start = *self.offsets.get(node_ix)?;
        let end = *self.offsets.get(node_ix.checked_add(1)?)?;
        if position >= end.saturating_sub(start) {
            return None;
        }
        self.entries.get(start.checked_add(position)?).copied()
    }

    #[cfg(test)]
    fn slice(&self, node_ix: usize) -> &[usize] {
        let Some(&start) = self.offsets.get(node_ix) else {
            return &[];
        };
        let Some(&end) = node_ix
            .checked_add(1)
            .and_then(|next_node_ix| self.offsets.get(next_node_ix))
        else {
            return &[];
        };
        self.entries.get(start..end).unwrap_or(&[])
    }
}

#[derive(Debug, Clone, PartialEq)]
struct TreeState {
    /// Tree node index -> graph node index.
    g_ix_by_t_ix: Vec<Option<usize>>,
    /// Graph node index -> tree node index.
    t_ix_by_g_ix: Vec<Option<usize>>,

    parent_t_ix: Vec<Option<usize>>,
    low: Vec<i32>,
    lim: Vec<i32>,

    /// Cut value for the tree edge between this node and its parent (roots have 0.0).
    cut_to_parent: Vec<f64>,

    roots: Vec<usize>,

    // Reused scratch buffers to avoid repeated allocations in the simplex loop.
    node_ixs: Vec<usize>,
    roots_to_visit: Vec<usize>,
    visited: Vec<bool>,
    array_index_by_t_ix: Vec<Option<u32>>,
    has_array_index_nodes: bool,
    array_index_adjacency_entry_count: usize,
    neighbors: PinnedUndirectedNeighbors,
    dfs_stack: Vec<DfsFrame>,

    in_tree_by_g_ix: Vec<bool>,
    low_by_g_ix: Vec<i32>,
    lim_by_g_ix: Vec<i32>,
    parent_g_ix_by_g_ix: Vec<Option<usize>>,
    cut_to_parent_by_g_ix: Vec<f64>,
    parent_edge_position_by_t_ix: Vec<Option<usize>>,
    g_ix_by_lim: Vec<Option<usize>>,
    tail_g_ixs: Vec<usize>,

    children: Vec<Vec<usize>>,
    postorder: Vec<usize>,
    post_stack: Vec<(usize, usize)>,
    rank_stack: Vec<usize>,

    tree_edges_in_order: Vec<DenseTreeEdge>,
    leave_edge_in_order: Option<OrderedTreeEdge>,
}

impl TreeState {
    #[cfg(test)]
    fn new(
        t: &Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ) -> Result<Self, RankError> {
        if t.is_directed() || t.is_multigraph() {
            return Err(RankError::InvalidNetworkSimplexTree);
        }
        let mut state = Self::from_slots(
            g,
            t.node_slot_count(),
            t.node_count(),
            t.edge_count(),
            t.nodes().filter_map(|id| t.node_ix(id).zip(g.node_ix(id))),
            t.edges()
                .map(|edge| g.node_ix(&edge.v).zip(g.node_ix(&edge.w))),
        )?;
        state.has_array_index_nodes = t.array_index_node_count() != 0;
        Ok(state)
    }

    fn from_slots(
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        t_len: usize,
        live_nodes: usize,
        live_edges: usize,
        ordered_nodes: impl Iterator<Item = (usize, usize)>,
        ordered_edges: impl Iterator<Item = Option<(usize, usize)>>,
    ) -> Result<Self, RankError> {
        let g_len = g.node_slot_count();
        let mut g_ix_by_t_ix: Vec<Option<usize>> = vec![None; t_len];
        let mut t_ix_by_g_ix: Vec<Option<usize>> = vec![None; g_len];
        let mut node_ixs = Vec::with_capacity(live_nodes);
        let mut array_index_by_t_ix = vec![None; t_len];
        let mut has_array_index_nodes = false;
        for (t_ix, g_ix) in ordered_nodes {
            g_ix_by_t_ix[t_ix] = Some(g_ix);
            t_ix_by_g_ix[g_ix] = Some(t_ix);
            node_ixs.push(t_ix);
            let array_index = g.javascript_array_index_by_node_ix(g_ix);
            array_index_by_t_ix[t_ix] = array_index;
            has_array_index_nodes |= array_index.is_some();
        }

        let mut tree_edges_in_order = Vec::with_capacity(live_edges);
        let mut array_index_adjacency_entry_count = 0usize;
        for ends in ordered_edges {
            let (v_g_ix, w_g_ix) = ends.ok_or(RankError::InvalidNetworkSimplexTree)?;
            let dense_edge = capture_dense_tree_edge(g, &t_ix_by_g_ix, v_g_ix, w_g_ix)
                .ok_or(RankError::InvalidNetworkSimplexTree)?;
            let v_array_index = array_index_by_t_ix[dense_edge.v_t_ix];
            let w_array_index = array_index_by_t_ix[dense_edge.w_t_ix];
            array_index_adjacency_entry_count = checked_add(
                array_index_adjacency_entry_count,
                usize::from(v_array_index.is_some()) + usize::from(w_array_index.is_some()),
            )?;
            tree_edges_in_order.push(dense_edge);
        }
        let lim_slots = checked_add(t_len, 1)?;

        Ok(Self {
            g_ix_by_t_ix,
            t_ix_by_g_ix,
            parent_t_ix: vec![None; t_len],
            low: vec![0; t_len],
            lim: vec![0; t_len],
            cut_to_parent: vec![0.0; t_len],
            roots: Vec::new(),
            node_ixs,
            roots_to_visit: Vec::new(),
            visited: vec![false; t_len],
            has_array_index_nodes,
            array_index_by_t_ix,
            array_index_adjacency_entry_count,
            neighbors: PinnedUndirectedNeighbors::default(),
            dfs_stack: Vec::new(),
            in_tree_by_g_ix: vec![false; g_len],
            low_by_g_ix: vec![0; g_len],
            lim_by_g_ix: vec![0; g_len],
            parent_g_ix_by_g_ix: vec![None; g_len],
            cut_to_parent_by_g_ix: vec![0.0; g_len],
            parent_edge_position_by_t_ix: vec![None; t_len],
            g_ix_by_lim: vec![None; lim_slots],
            tail_g_ixs: Vec::new(),
            children: vec![Vec::new(); t_len],
            postorder: Vec::new(),
            post_stack: Vec::new(),
            rank_stack: Vec::new(),
            tree_edges_in_order,
            leave_edge_in_order: None,
        })
    }

    #[cfg(test)]
    fn node_count(&self) -> usize {
        self.node_ixs.len()
    }

    fn node_slot_count(&self) -> usize {
        self.g_ix_by_t_ix.len()
    }

    fn edge_count(&self) -> usize {
        self.tree_edges_in_order.len()
    }

    fn node_low_lim_by_gix(&self, g_ix: usize) -> Option<(i32, i32)> {
        if !self.in_tree_by_g_ix.get(g_ix).copied().unwrap_or(false) {
            return None;
        }
        Some((
            self.low_by_g_ix.get(g_ix).copied()?,
            self.lim_by_g_ix.get(g_ix).copied()?,
        ))
    }

    fn rebuild(
        &mut self,
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        root: Option<&str>,
    ) -> Result<(), RankError> {
        let t_len = self.g_ix_by_t_ix.len();
        self.roots.clear();
        self.parent_t_ix.fill(None);
        self.low.fill(0);
        self.lim.fill(0);
        self.cut_to_parent.fill(0.0);

        // Pinned dagre-d3-es 7.0.14 implements undirected `neighbors(v)` as the union of
        // predecessor Object.keys followed by successor Object.keys. Array-index IDs are numeric
        // within each bucket; ordinary IDs retain the current tree-edge insertion order.
        self.neighbors.rebuild(
            &self.tree_edges_in_order,
            &self.array_index_by_t_ix,
            self.array_index_adjacency_entry_count,
        )?;

        self.visited.fill(false);
        let mut next_lim: i32 = 1;

        let preferred_root_ix: Option<usize> = root
            .and_then(|id| g.node_ix(id))
            .and_then(|g_ix| self.t_ix_by_g_ix.get(g_ix).copied().flatten())
            .or_else(|| self.node_ixs.first().copied());

        self.roots_to_visit.clear();
        if let Some(ix) = preferred_root_ix {
            self.roots_to_visit.push(ix);
        }
        self.roots_to_visit.extend(self.node_ixs.iter().copied());

        for &start_ix in &self.roots_to_visit {
            if start_ix >= self.visited.len() || self.visited[start_ix] {
                continue;
            }
            if self.g_ix_by_t_ix.get(start_ix).copied().flatten().is_none() {
                continue;
            }
            self.roots.push(start_ix);
            self.visited[start_ix] = true;

            self.dfs_stack.clear();
            self.dfs_stack.push(DfsFrame {
                v_ix: start_ix,
                parent_ix: None,
                low: next_lim,
                next_neighbor: 0,
            });

            while !self.dfs_stack.is_empty() {
                let next_child = {
                    let Some(top) = self.dfs_stack.last_mut() else {
                        break;
                    };
                    let neighbor = self.neighbors.get(top.v_ix, top.next_neighbor);
                    if neighbor.is_some() {
                        top.next_neighbor = checked_add(top.next_neighbor, 1)?;
                    }
                    neighbor.map(|w_ix| (w_ix, top.v_ix, top.parent_ix))
                };

                if let Some((w_ix, parent_v_ix, parent_ix)) = next_child {
                    if parent_ix.is_some_and(|p| p == w_ix) {
                        continue;
                    }
                    if w_ix >= self.visited.len() || self.visited[w_ix] {
                        continue;
                    }
                    self.visited[w_ix] = true;
                    self.parent_t_ix[w_ix] = Some(parent_v_ix);
                    self.dfs_stack.push(DfsFrame {
                        v_ix: w_ix,
                        parent_ix: Some(parent_v_ix),
                        low: next_lim,
                        next_neighbor: 0,
                    });
                    continue;
                }

                let Some(frame) = self.dfs_stack.pop() else {
                    break;
                };
                let DfsFrame {
                    v_ix,
                    parent_ix: _,
                    low,
                    next_neighbor: _,
                } = frame;
                self.low[v_ix] = low;
                self.lim[v_ix] = next_lim;
                next_lim = next_lim
                    .checked_add(1)
                    .ok_or(WorkError::ArithmeticOverflow)?;
            }
        }

        self.in_tree_by_g_ix.resize(self.t_ix_by_g_ix.len(), false);
        self.low_by_g_ix.resize(self.t_ix_by_g_ix.len(), 0);
        self.lim_by_g_ix.resize(self.t_ix_by_g_ix.len(), 0);
        self.parent_g_ix_by_g_ix
            .resize(self.t_ix_by_g_ix.len(), None);
        self.cut_to_parent_by_g_ix
            .resize(self.t_ix_by_g_ix.len(), 0.0);
        self.in_tree_by_g_ix.fill(false);
        self.low_by_g_ix.fill(0);
        self.lim_by_g_ix.fill(0);
        self.parent_g_ix_by_g_ix.fill(None);
        // `cut_to_parent_by_g_ix` is written in postorder during `rebuild_cut_values`;
        // roots are never read through this mapping, so we don't need to clear the whole
        // buffer here.

        for &t_ix in &self.node_ixs {
            let Some(g_ix) = self.g_ix_by_t_ix.get(t_ix).copied().flatten() else {
                continue;
            };
            if g_ix >= self.in_tree_by_g_ix.len() {
                continue;
            }
            self.in_tree_by_g_ix[g_ix] = true;
            self.low_by_g_ix[g_ix] = self.low.get(t_ix).copied().unwrap_or(0);
            self.lim_by_g_ix[g_ix] = self.lim.get(t_ix).copied().unwrap_or(0);
        }

        self.parent_edge_position_by_t_ix.resize(t_len, None);
        self.parent_edge_position_by_t_ix.fill(None);

        for (position, edge) in self.tree_edges_in_order.iter().enumerate() {
            let (u_tix, v_tix) = (edge.v_t_ix, edge.w_t_ix);
            if (edge.tail_t_ix, edge.head_t_ix) != (u_tix, v_tix)
                && (edge.tail_t_ix, edge.head_t_ix) != (v_tix, u_tix)
            {
                continue;
            }
            let child_tix = if self.parent_t_ix.get(u_tix).copied().flatten() == Some(v_tix) {
                u_tix
            } else if self.parent_t_ix.get(v_tix).copied().flatten() == Some(u_tix) {
                v_tix
            } else {
                continue;
            };
            let parent_tix = if child_tix == u_tix { v_tix } else { u_tix };
            let Some(child_gix) = self.g_ix_by_t_ix.get(child_tix).copied().flatten() else {
                continue;
            };
            let Some(parent_gix) = self.g_ix_by_t_ix.get(parent_tix).copied().flatten() else {
                continue;
            };
            if child_gix < self.parent_g_ix_by_g_ix.len() {
                self.parent_g_ix_by_g_ix[child_gix] = Some(parent_gix);
            }
            self.parent_edge_position_by_t_ix[child_tix] = Some(position);
        }

        self.g_ix_by_lim.resize(checked_add(t_len, 1)?, None);
        self.g_ix_by_lim.fill(None);
        for &t_ix in &self.node_ixs {
            let Some(g_ix) = self.g_ix_by_t_ix.get(t_ix).copied().flatten() else {
                continue;
            };
            let lim = self.lim.get(t_ix).copied().unwrap_or(0);
            let Ok(lim) = usize::try_from(lim) else {
                continue;
            };
            if lim < self.g_ix_by_lim.len() {
                self.g_ix_by_lim[lim] = Some(g_ix);
            }
        }

        self.rebuild_cut_values(g)?;
        Ok(())
    }

    fn rebuild_cut_values(
        &mut self,
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ) -> Result<(), RankError> {
        let t_len = self.parent_t_ix.len();
        self.children.resize_with(t_len, Vec::new);
        self.children.truncate(t_len);
        for ch in &mut self.children {
            ch.clear();
        }
        for (child_ix, parent_ix) in self.parent_t_ix.iter().copied().enumerate() {
            let Some(parent_ix) = parent_ix else {
                continue;
            };
            if parent_ix < self.children.len() {
                self.children[parent_ix].push(child_ix);
            }
        }

        // Postorder traversal for each tree component.
        self.postorder.clear();
        for &root_ix in &self.roots {
            if root_ix >= t_len {
                continue;
            }
            if self.g_ix_by_t_ix.get(root_ix).copied().flatten().is_none() {
                continue;
            }

            self.post_stack.clear();
            self.post_stack.push((root_ix, 0));
            while let Some((v_ix, idx)) = self.post_stack.last_mut() {
                let next_child = self
                    .children
                    .get(*v_ix)
                    .and_then(|ch| ch.get(*idx))
                    .copied();
                if let Some(w_ix) = next_child {
                    *idx = checked_add(*idx, 1)?;
                    self.post_stack.push((w_ix, 0));
                    continue;
                }
                let Some((v_ix, _idx)) = self.post_stack.pop() else {
                    break;
                };
                self.postorder.push(v_ix);
            }
        }

        self.cut_to_parent_by_g_ix
            .resize(self.t_ix_by_g_ix.len(), 0.0);
        // `cut_to_parent_by_g_ix` is populated in postorder below; we intentionally avoid
        // clearing the whole buffer here to keep rebuild costs down.
        for &child_tix in &self.postorder {
            if self.parent_t_ix.get(child_tix).copied().flatten().is_none() {
                continue;
            }
            let cut = self.calc_cut_value_by_tix(g, child_tix);
            if child_tix < self.cut_to_parent.len() {
                self.cut_to_parent[child_tix] = cut;
            }
            if let Some(child_gix) = self.g_ix_by_t_ix.get(child_tix).copied().flatten()
                && child_gix < self.cut_to_parent_by_g_ix.len()
            {
                self.cut_to_parent_by_g_ix[child_gix] = cut;
            }
        }
        self.leave_edge_in_order = None;
        for (position, edge) in self.tree_edges_in_order.iter().enumerate() {
            let (u_ix, v_ix) = (edge.v_t_ix, edge.w_t_ix);
            let child_tix = if self.parent_t_ix.get(u_ix).copied().flatten() == Some(v_ix) {
                Some(u_ix)
            } else if self.parent_t_ix.get(v_ix).copied().flatten() == Some(u_ix) {
                Some(v_ix)
            } else {
                None
            };
            let Some(child_tix) = child_tix else {
                continue;
            };
            if self.cut_to_parent.get(child_tix).copied().unwrap_or(0.0) < 0.0 {
                self.leave_edge_in_order = Some(OrderedTreeEdge {
                    position,
                    v_ix: u_ix,
                    w_ix: v_ix,
                });
                break;
            }
        }
        Ok(())
    }

    fn calc_cut_value_by_tix(
        &self,
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
        child_tix: usize,
    ) -> f64 {
        let Some(parent_tix) = self.parent_t_ix.get(child_tix).copied().flatten() else {
            return 0.0;
        };
        let Some(child_gix) = self.g_ix_by_t_ix.get(child_tix).copied().flatten() else {
            return 0.0;
        };
        let Some(parent_gix) = self.g_ix_by_t_ix.get(parent_tix).copied().flatten() else {
            return 0.0;
        };

        let Some(parent_edge) = self
            .parent_edge_position_by_t_ix
            .get(child_tix)
            .copied()
            .flatten()
            .and_then(|position| self.tree_edges_in_order.get(position))
        else {
            return 0.0;
        };
        let child_is_tail = parent_edge.tail_t_ix == child_tix;
        let mut cut_value = parent_edge.weight;

        if g.is_directed() {
            let parent_g_ix_by_g_ix = &self.parent_g_ix_by_g_ix;
            let cut_to_parent_by_g_ix = &self.cut_to_parent_by_g_ix;
            let out_sign: f64 = if child_is_tail { 1.0 } else { -1.0 };
            let in_sign: f64 = -out_sign;

            // Pinned Graphlib nodeEdges(child) concatenates incoming edges before outgoing edges.
            // Preserve that accumulation order because f64 addition is not associative and the
            // sign of a cut value decides whether the edge leaves the tree.
            g.for_each_in_edge_ix(child_gix, None, |tail_ix, _head_ix, _ek, lbl| {
                if tail_ix == parent_gix {
                    return;
                }

                cut_value += in_sign * lbl.weight;

                let (Some(parent), Some(other_cut_value)) = (
                    parent_g_ix_by_g_ix.get(tail_ix),
                    cut_to_parent_by_g_ix.get(tail_ix),
                ) else {
                    return;
                };
                if *parent == Some(child_gix) {
                    cut_value += -in_sign * *other_cut_value;
                }
            });

            g.for_each_out_edge_ix(child_gix, None, |_tail_ix, head_ix, _ek, lbl| {
                if head_ix == parent_gix {
                    return;
                }

                cut_value += out_sign * lbl.weight;

                let (Some(parent), Some(other_cut_value)) = (
                    parent_g_ix_by_g_ix.get(head_ix),
                    cut_to_parent_by_g_ix.get(head_ix),
                ) else {
                    return;
                };
                if *parent == Some(child_gix) {
                    cut_value += -out_sign * *other_cut_value;
                }
            });
        }

        cut_value
    }

    fn exchange_edge(
        &mut self,
        leaving: OrderedTreeEdge,
        entering: &EdgeKey,
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ) -> Result<(), RankError> {
        let Some(leaving_edge) = self.tree_edges_in_order.get(leaving.position).copied() else {
            return Err(RankError::InvalidNetworkSimplexTree);
        };
        let leaving_ends = (leaving_edge.v_t_ix, leaving_edge.w_t_ix);
        if leaving_ends != (leaving.v_ix, leaving.w_ix)
            && leaving_ends != (leaving.w_ix, leaving.v_ix)
        {
            return Err(RankError::InvalidNetworkSimplexTree);
        }

        let Some(tail_tix) = g
            .node_ix(&entering.v)
            .and_then(|g_ix| self.t_ix_by_g_ix.get(g_ix).copied().flatten())
        else {
            return Err(RankError::InvalidNetworkSimplexTree);
        };
        let Some(head_tix) = g
            .node_ix(&entering.w)
            .and_then(|g_ix| self.t_ix_by_g_ix.get(g_ix).copied().flatten())
        else {
            return Err(RankError::InvalidNetworkSimplexTree);
        };
        let Some(entering_label) = g.edge_by_key(entering) else {
            return Err(RankError::InvalidNetworkSimplexTree);
        };
        if tail_tix == head_tix {
            return Err(RankError::InvalidNetworkSimplexTree);
        }

        let entering_ends = if entering.v <= entering.w {
            (tail_tix, head_tix)
        } else {
            (head_tix, tail_tix)
        };
        if entering_ends == leaving_ends
            || entering_ends == (leaving_ends.1, leaving_ends.0)
            || self
                .tree_edges_in_order
                .iter()
                .enumerate()
                .any(|(position, edge)| {
                    let ends = (edge.v_t_ix, edge.w_t_ix);
                    position != leaving.position
                        && (ends == entering_ends || ends == (entering_ends.1, entering_ends.0))
                })
        {
            return Err(RankError::InvalidNetworkSimplexTree);
        }

        let leaving_numeric_entries = checked_add(
            usize::from(self.array_index_by_t_ix[leaving_ends.0].is_some()),
            usize::from(self.array_index_by_t_ix[leaving_ends.1].is_some()),
        )?;
        let entering_numeric_entries = checked_add(
            usize::from(self.array_index_by_t_ix[entering_ends.0].is_some()),
            usize::from(self.array_index_by_t_ix[entering_ends.1].is_some()),
        )?;
        let Some(remaining_numeric_entries) = self
            .array_index_adjacency_entry_count
            .checked_sub(leaving_numeric_entries)
        else {
            return Err(RankError::InvalidNetworkSimplexTree);
        };
        let next_numeric_entries =
            checked_add(remaining_numeric_entries, entering_numeric_entries)?;

        self.tree_edges_in_order.remove(leaving.position);
        self.tree_edges_in_order.push(DenseTreeEdge {
            v_t_ix: entering_ends.0,
            w_t_ix: entering_ends.1,
            tail_t_ix: tail_tix,
            head_t_ix: head_tix,
            minlen: entering_label.minlen,
            weight: entering_label.weight,
        });
        self.array_index_adjacency_entry_count = next_numeric_entries;
        Ok(())
    }

    fn find_leave_edge_in_insertion_order(&self) -> Option<OrderedTreeEdge> {
        self.leave_edge_in_order
    }
}

pub fn network_simplex(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
) -> Result<(), crate::LayoutError> {
    let mut work_control = NoopWorkControl;
    network_simplex_controlled(g, &mut work_control).map_err(crate::LayoutError::from)
}

pub(crate) fn network_simplex_controlled(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut dyn WorkControl,
) -> Result<(), RankError> {
    // These owners are deliberately boxed. In debug builds their aggregate return
    // places otherwise remain live together across the whole network-simplex coordinator, which
    // can exhaust small worker stacks even though every topology traversal is iterative.
    let mut simplified = build_simplified_graph_boxed(g, work_control)?;
    util::longest_path_controlled(&mut simplified, work_control)?;
    let mut t_state = build_initial_tree_state_controlled(&mut simplified, work_control)?;

    work_control.charge(simplified.node_slot_count())?;
    let mut rank_by_ix = vec![0_i128; simplified.node_slot_count()];
    simplified.for_each_node_ix(|g_ix, _id, lbl| {
        rank_by_ix[g_ix] = i128::from(lbl.rank.unwrap_or(0));
    });

    while let Some(leaving) = t_state.find_leave_edge_in_insertion_order() {
        pivot_controlled(
            &mut t_state,
            &mut simplified,
            &mut rank_by_ix,
            leaving,
            work_control,
        )?;
    }

    work_control.charge(g.node_count())?;
    for v in g.node_ids() {
        if let Some(rank) = simplified.node(&v).and_then(|n| n.rank)
            && let Some(lbl) = g.node_mut(&v)
        {
            lbl.rank = Some(rank);
        }
    }
    Ok(())
}

fn simplify_work_units(g: &Graph<NodeLabel, EdgeLabel, GraphLabel>) -> Result<usize, WorkError> {
    let node_work = checked_mul(g.node_count(), 2)?;
    let numeric_order_work =
        checked_ordered_key_updates(g.node_count(), g.array_index_node_count())?;
    let numeric_adjacency_work = checked_ordered_key_updates(
        g.node_count(),
        g.directed_array_index_adjacency_entry_count(),
    )?;
    // Rank validation and endpoint aggregation each scan the slot-backed edge storage. The
    // ordered endpoint index gives the merge a deterministic logarithmic bound while the separate
    // first-occurrence vector avoids the second endpoint-string sort used by the legacy path.
    let slot_scan_work = checked_mul(g.edge_slot_count(), 2)?;
    let live_edge_work = checked_mul(g.edge_count(), 3)?;
    let ordered_merge_work = checked_n_log_n(g.edge_count())?;
    let edge_work = checked_add(
        checked_add(slot_scan_work, live_edge_work)?,
        ordered_merge_work,
    )?;
    checked_add(
        checked_add(
            checked_add(node_work, numeric_order_work)?,
            numeric_adjacency_work,
        )?,
        edge_work,
    )
}

#[cfg(test)]
fn build_simplified_graph(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut dyn WorkControl,
) -> Result<Graph<NodeLabel, EdgeLabel, GraphLabel>, WorkError> {
    Ok(*build_simplified_graph_boxed(g, work_control)?)
}

fn build_simplified_graph_boxed(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut dyn WorkControl,
) -> Result<Box<Graph<NodeLabel, EdgeLabel, GraphLabel>>, WorkError> {
    work_control.charge(simplify_work_units(g)?)?;
    crate::rank::validate_rank_arithmetic(g)?;
    Ok(Box::new(crate::util::simplify(g)))
}

fn build_initial_tree_state_controlled(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut dyn WorkControl,
) -> Result<Box<TreeState>, RankError> {
    let seed = feasible_tree::feasible_tree_seed_controlled(g, work_control)?;
    let nodes = seed.discovery_order.len();
    let edges = seed.edges.len();
    // The forest is append-only, so both old Graph node slot counts equal its live node count.
    work_control.charge(tree_state_construction_work_units(
        g.node_slot_count(),
        nodes,
        nodes,
        edges,
    )?)?;
    let mut state = Box::new(TreeState::from_slots(
        g,
        nodes,
        nodes,
        edges,
        seed.ordered_nodes(),
        seed.edges.iter().copied().map(Some),
    )?);
    drop(seed);
    rebuild_initial_state_controlled(&mut state, g, None, work_control)?;
    Ok(state)
}

#[cfg(test)]
fn tree_state_new_work_units(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    tree: &Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
) -> Result<usize, WorkError> {
    tree_state_construction_work_units(
        g.node_slot_count(),
        tree.node_slot_count(),
        tree.node_order_slot_count(),
        tree.edge_slot_count(),
    )
}

fn tree_state_construction_work_units(
    graph_slots: usize,
    tree_slots: usize,
    tree_order_slots: usize,
    tree_edge_slots: usize,
) -> Result<usize, WorkError> {
    checked_add(
        graph_slots,
        checked_add(
            checked_add(tree_slots, tree_order_slots)?,
            checked_mul(tree_edge_slots, 2)?,
        )?,
    )
}

#[cfg(test)]
fn build_tree_state_controlled(
    tree: &Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    root: Option<&str>,
    work_control: &mut dyn WorkControl,
) -> Result<TreeState, RankError> {
    work_control.charge(tree_state_new_work_units(g, tree)?)?;
    let mut state = TreeState::new(tree, g)?;
    rebuild_initial_state_controlled(&mut state, g, root, work_control)?;
    Ok(state)
}

fn rebuild_initial_state_controlled(
    state: &mut TreeState,
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    root: Option<&str>,
    work_control: &mut dyn WorkControl,
) -> Result<(), RankError> {
    work_control.charge(dense_tree_state_rebuild_work_units(
        g,
        state.node_slot_count(),
        state.edge_count(),
        state.array_index_adjacency_entry_count,
    )?)?;
    state.rebuild(g, root)
}

fn dense_tree_state_rebuild_work_units(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    tree_nodes: usize,
    tree_edges: usize,
    numeric_adjacency_entries: usize,
) -> Result<usize, WorkError> {
    let graph_nodes = checked_mul(g.node_slot_count(), 3)?;
    let graph_edges = checked_mul(g.edge_slot_count(), 3)?;
    let tree_node_work = checked_mul(tree_nodes, 8)?;
    // Neighbor reconstruction performs one validation/size-planning pass before the two build
    // passes so all allocation sizes and cursors are checked before mutation.
    let tree_edge_work = checked_mul(tree_edges, 6)?;
    let numeric_adjacency_work =
        checked_ordered_key_updates(tree_nodes, numeric_adjacency_entries)?;
    checked_add(
        checked_add(graph_nodes, graph_edges)?,
        checked_add(
            checked_add(tree_node_work, tree_edge_work)?,
            numeric_adjacency_work,
        )?,
    )
}

fn simplex_iteration_work_units(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    tree: &TreeState,
) -> Result<usize, WorkError> {
    let entering_and_rank_work = checked_add(checked_mul(g.node_count(), 2)?, g.edge_count())?;
    // Validate that the entering edge is not already live, then shift the dense suffix during the
    // stable removal. The cached leaving position avoids a third full tree-edge scan.
    let stable_exchange_work = checked_mul(tree.edge_count(), 2)?;
    // The entering edge can increase the number of numeric Object.keys adjacency entries after
    // this pre-mutation charge. Bound the rebuild by both endpoints of every live tree edge rather
    // than by the current topology's count.
    let numeric_adjacency_entries = if tree.has_array_index_nodes {
        std::cmp::min(
            checked_mul(tree.edge_count(), 2)?,
            checked_add(tree.array_index_adjacency_entry_count, 2)?,
        )
    } else {
        0
    };
    checked_add(
        checked_add(entering_and_rank_work, stable_exchange_work)?,
        dense_tree_state_rebuild_work_units(
            g,
            tree.node_slot_count(),
            tree.edge_count(),
            numeric_adjacency_entries,
        )?,
    )
}

fn pivot_controlled(
    tree: &mut TreeState,
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    rank_by_ix: &mut [i128],
    leaving: OrderedTreeEdge,
    work_control: &mut dyn WorkControl,
) -> Result<(), RankError> {
    // Charge the complete pivot before candidate discovery mutates scratch state or the stable
    // exchange changes tree order. A rejected tranche therefore leaves topology and ranks intact.
    work_control.charge(simplex_iteration_work_units(g, tree)?)?;
    let entering = enter_edge_fast(tree, g, rank_by_ix, leaving)?;
    tree.exchange_edge(leaving, &entering, g)?;
    tree.rebuild(g, None)?;
    update_ranks_fast(tree, g, rank_by_ix)
}

fn enter_edge_fast(
    t_state: &mut TreeState,
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    rank_by_ix: &[i128],
    leaving: OrderedTreeEdge,
) -> Result<EdgeKey, RankError> {
    let Some(leaving_edge) = t_state.tree_edges_in_order.get(leaving.position).copied() else {
        return Err(RankError::InvalidNetworkSimplexTree);
    };
    let leaving_ends = (leaving_edge.v_t_ix, leaving_edge.w_t_ix);
    if leaving_ends != (leaving.v_ix, leaving.w_ix) && leaving_ends != (leaving.w_ix, leaving.v_ix)
    {
        return Err(RankError::InvalidNetworkSimplexTree);
    }
    let Some(v_gix) = t_state
        .g_ix_by_t_ix
        .get(leaving_edge.tail_t_ix)
        .copied()
        .flatten()
    else {
        return Err(RankError::InvalidNetworkSimplexTree);
    };
    let Some(w_gix) = t_state
        .g_ix_by_t_ix
        .get(leaving_edge.head_t_ix)
        .copied()
        .flatten()
    else {
        return Err(RankError::InvalidNetworkSimplexTree);
    };
    let Some(leave_u_gix) = t_state.g_ix_by_t_ix.get(leaving.v_ix).copied().flatten() else {
        return Err(RankError::InvalidNetworkSimplexTree);
    };
    let Some(leave_v_gix) = t_state.g_ix_by_t_ix.get(leaving.w_ix).copied().flatten() else {
        return Err(RankError::InvalidNetworkSimplexTree);
    };

    let Some((v_low, v_lim)) = t_state.node_low_lim_by_gix(v_gix) else {
        return Err(RankError::InvalidNetworkSimplexTree);
    };
    let Some((w_low, w_lim)) = t_state.node_low_lim_by_gix(w_gix) else {
        return Err(RankError::InvalidNetworkSimplexTree);
    };

    let ((tail_low, tail_lim), flip) = if v_lim > w_lim {
        ((w_low, w_lim), true)
    } else {
        ((v_low, v_lim), false)
    };

    let is_in_tail = |t_state: &TreeState, g_ix: usize| -> bool {
        if !t_state.in_tree_by_g_ix.get(g_ix).copied().unwrap_or(false) {
            return false;
        }
        let lim = t_state.lim_by_g_ix.get(g_ix).copied().unwrap_or(0);
        tail_low <= lim && lim <= tail_lim
    };

    t_state.tail_g_ixs.clear();
    let Ok(tail_low) = usize::try_from(tail_low) else {
        return Err(RankError::InvalidNetworkSimplexTree);
    };
    let Ok(tail_lim) = usize::try_from(tail_lim) else {
        return Err(RankError::InvalidNetworkSimplexTree);
    };
    if tail_low == 0 || tail_low > tail_lim {
        return Err(RankError::InvalidNetworkSimplexTree);
    }
    let max_lim = t_state.g_ix_by_lim.len().saturating_sub(1);
    if tail_lim > max_lim {
        return Err(RankError::InvalidNetworkSimplexTree);
    }

    let tail_node_count = checked_add(tail_lim.saturating_sub(tail_low), 1)?;
    t_state.tail_g_ixs.reserve(tail_node_count);
    for lim in tail_low..=tail_lim {
        let Some(g_ix) = t_state.g_ix_by_lim.get(lim).copied().flatten() else {
            continue;
        };
        t_state.tail_g_ixs.push(g_ix);
    }

    let mut best: Option<(i128, usize)> = None;

    if g.is_directed() {
        if !flip {
            for &head_gix in &t_state.tail_g_ixs {
                g.for_each_in_edge_entry_ix(
                    head_gix,
                    None,
                    |edge_ix, tail_ix, head_ix, _key, lbl| {
                        debug_assert_eq!(head_ix, head_gix);
                        // Skip re-adding the leaving edge.
                        if (tail_ix == leave_u_gix && head_ix == leave_v_gix)
                            || (tail_ix == leave_v_gix && head_ix == leave_u_gix)
                        {
                            return;
                        }
                        if is_in_tail(&*t_state, tail_ix) {
                            return;
                        }
                        if !t_state
                            .in_tree_by_g_ix
                            .get(tail_ix)
                            .copied()
                            .unwrap_or(false)
                        {
                            return;
                        }

                        let v_rank = rank_by_ix.get(tail_ix).copied().unwrap_or(0);
                        let w_rank = rank_by_ix.get(head_ix).copied().unwrap_or(0);
                        let minlen = lbl.minlen as i128;
                        let slack = w_rank - v_rank - minlen;
                        match &best {
                            Some(best_key) if (slack, edge_ix) >= *best_key => {}
                            _ => best = Some((slack, edge_ix)),
                        }
                    },
                );
            }
        } else {
            for &tail_gix in &t_state.tail_g_ixs {
                g.for_each_out_edge_entry_ix(
                    tail_gix,
                    None,
                    |edge_ix, tail_ix, head_ix, _key, lbl| {
                        debug_assert_eq!(tail_ix, tail_gix);
                        // Skip re-adding the leaving edge.
                        if (tail_ix == leave_u_gix && head_ix == leave_v_gix)
                            || (tail_ix == leave_v_gix && head_ix == leave_u_gix)
                        {
                            return;
                        }
                        if is_in_tail(&*t_state, head_ix) {
                            return;
                        }
                        if !t_state
                            .in_tree_by_g_ix
                            .get(head_ix)
                            .copied()
                            .unwrap_or(false)
                        {
                            return;
                        }

                        let v_rank = rank_by_ix.get(tail_ix).copied().unwrap_or(0);
                        let w_rank = rank_by_ix.get(head_ix).copied().unwrap_or(0);
                        let minlen = lbl.minlen as i128;
                        let slack = w_rank - v_rank - minlen;
                        match &best {
                            Some(best_key) if (slack, edge_ix) >= *best_key => {}
                            _ => best = Some((slack, edge_ix)),
                        }
                    },
                );
            }
        }
    } else {
        g.for_each_edge_entry_ix(|edge_ix, g_v_ix, g_w_ix, _key, lbl| {
            let v_desc = is_in_tail(&*t_state, g_v_ix);
            let w_desc = is_in_tail(&*t_state, g_w_ix);
            if flip == v_desc && flip != w_desc {
                let v_rank = rank_by_ix.get(g_v_ix).copied().unwrap_or(0);
                let w_rank = rank_by_ix.get(g_w_ix).copied().unwrap_or(0);
                let minlen = lbl.minlen as i128;
                let slack = w_rank - v_rank - minlen;
                match &best {
                    Some(best_key) if (slack, edge_ix) >= *best_key => {}
                    _ => best = Some((slack, edge_ix)),
                }
            }
        });
    }

    let Some((_, edge_ix)) = best else {
        return Err(RankError::InvalidNetworkSimplexTree);
    };
    g.edge_key_by_ix(edge_ix)
        .cloned()
        .ok_or(RankError::InvalidNetworkSimplexTree)
}

fn update_ranks_fast(
    t_state: &mut TreeState,
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    rank_by_ix: &mut [i128],
) -> Result<(), RankError> {
    for &root_tix in &t_state.roots {
        if t_state
            .g_ix_by_t_ix
            .get(root_tix)
            .copied()
            .flatten()
            .is_none()
        {
            continue;
        }

        t_state.rank_stack.clear();
        t_state.rank_stack.push(root_tix);

        while let Some(parent_tix) = t_state.rank_stack.pop() {
            let Some(parent_gix) = t_state.g_ix_by_t_ix.get(parent_tix).copied().flatten() else {
                continue;
            };

            let parent_rank = rank_by_ix.get(parent_gix).copied().unwrap_or(0);
            let Some(ch) = t_state.children.get(parent_tix) else {
                continue;
            };
            for &child_tix in ch {
                let Some(child_gix) = t_state.g_ix_by_t_ix.get(child_tix).copied().flatten() else {
                    continue;
                };

                let Some(parent_edge) = t_state
                    .parent_edge_position_by_t_ix
                    .get(child_tix)
                    .copied()
                    .flatten()
                    .and_then(|position| t_state.tree_edges_in_order.get(position))
                else {
                    continue;
                };
                let minlen = parent_edge.minlen as i128;
                let child_is_tail = parent_edge.tail_t_ix == child_tix;

                let rank = if child_is_tail {
                    parent_rank - minlen
                } else {
                    parent_rank + minlen
                };
                let rank_i32 = i32::try_from(rank).map_err(|_| WorkError::ArithmeticOverflow)?;

                if let Some(node) = g.node_label_mut_by_ix(child_gix) {
                    node.rank = Some(rank_i32);
                }

                let Some(rank_slot) = rank_by_ix.get_mut(child_gix) else {
                    return Err(RankError::InvalidNetworkSimplexTree);
                };
                *rank_slot = rank;
                t_state.rank_stack.push(child_tix);
            }
        }
    }
    Ok(())
}

pub fn init_low_lim_values(
    tree: &mut Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
    root: Option<&str>,
) {
    let Some(root) = root
        .map(|s| s.to_string())
        .or_else(|| tree.nodes().next().map(|s| s.to_string()))
    else {
        return;
    };
    let Some(root_ix) = tree.node_ix(&root) else {
        return;
    };
    let node_slots = tree.node_slot_count();
    let mut neighbors_by_ix = vec![Vec::new(); node_slots];
    for (node_ix, neighbors) in neighbors_by_ix.iter_mut().enumerate() {
        tree.for_each_pinned_predecessor_ix(node_ix, |neighbor_ix| {
            neighbors.push(neighbor_ix);
        });
        tree.for_each_pinned_successor_ix(node_ix, |neighbor_ix| {
            // A self loop occupies both predecessor and successor objects; Graphlib's union keeps
            // only the predecessor-side occurrence. Other undirected endpoint pairs are canonical
            // and therefore occur in exactly one bucket.
            if !neighbors.contains(&neighbor_ix) {
                neighbors.push(neighbor_ix);
            }
        });
    }

    let mut visited = vec![false; node_slots];
    let mut stack = Vec::new();
    let mut next_lim: i32 = 1;
    visited[root_ix] = true;
    stack.push(DfsFrame {
        v_ix: root_ix,
        parent_ix: None,
        low: next_lim,
        next_neighbor: 0,
    });

    while !stack.is_empty() {
        let next_child = {
            let Some(top) = stack.last_mut() else {
                break;
            };
            neighbors_by_ix
                .get(top.v_ix)
                .and_then(|neighbors| neighbors.get(top.next_neighbor))
                .copied()
                .inspect(|_| top.next_neighbor += 1)
                .map(|w_ix| (w_ix, top.v_ix, top.parent_ix))
        };

        if let Some((w_ix, parent_v_ix, parent_ix)) = next_child {
            if parent_ix.is_some_and(|parent_ix| parent_ix == w_ix) || visited[w_ix] {
                continue;
            }
            visited[w_ix] = true;
            stack.push(DfsFrame {
                v_ix: w_ix,
                parent_ix: Some(parent_v_ix),
                low: next_lim,
                next_neighbor: 0,
            });
            continue;
        }

        let Some(frame) = stack.pop() else {
            break;
        };
        let DfsFrame {
            v_ix,
            parent_ix,
            low,
            next_neighbor: _,
        } = frame;

        let parent = parent_ix
            .and_then(|p| tree.node_id_by_ix(p))
            .map(|p| p.to_string());
        if let Some(label) = tree.node_label_mut_by_ix(v_ix) {
            label.low = low;
            label.lim = next_lim;
            label.parent = parent;
        }

        next_lim += 1;
    }
}

pub fn init_cut_values(
    t: &mut Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
) {
    let mut vs: Vec<String> = {
        let roots: Vec<&str> = t.nodes().collect();
        alg::postorder(t, &roots)
    };
    let _ = vs.pop();
    for v in vs {
        assign_cut_value(t, g, &v);
    }
}

fn assign_cut_value(
    t: &mut Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    child: &str,
) {
    let Some(parent) = t.node(child).and_then(|lbl| lbl.parent.clone()) else {
        return;
    };
    let cutvalue = calc_cut_value(t, g, child);
    if let Some(edge) = t.edge_mut(child, &parent, None) {
        edge.cutvalue = cutvalue;
    }
}

pub fn calc_cut_value(
    t: &Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    child: &str,
) -> f64 {
    let Some(parent) = t.node(child).and_then(|lbl| lbl.parent.as_deref()) else {
        return 0.0;
    };

    let mut child_is_tail = true;
    let graph_edge = if g.is_directed() {
        let Some(child_ix) = g.node_ix(child) else {
            return 0.0;
        };
        let Some(parent_ix) = g.node_ix(parent) else {
            return 0.0;
        };

        if let Some(e) = g.edge_by_endpoints_ix(child_ix, parent_ix) {
            e
        } else {
            child_is_tail = false;
            let Some(e) = g.edge_by_endpoints_ix(parent_ix, child_ix) else {
                return 0.0;
            };
            e
        }
    } else {
        let mut graph_edge = g.edge(child, parent, None);
        if graph_edge.is_none() {
            child_is_tail = false;
            graph_edge = g.edge(parent, child, None);
        }
        let Some(graph_edge) = graph_edge else {
            return 0.0;
        };
        graph_edge
    };

    let mut cut_value = graph_edge.weight;

    if g.is_directed() {
        let Some(child_ix) = g.node_ix(child) else {
            return cut_value;
        };
        let parent_ix = g.node_ix(parent);

        g.for_each_in_edge_ix(child_ix, None, |tail_ix, _head_ix, _ek, lbl| {
            if parent_ix.is_some_and(|p| tail_ix == p) {
                return;
            }
            let Some(other) = g.node_id_by_ix(tail_ix) else {
                return;
            };

            let points_to_head = !child_is_tail;
            cut_value += if points_to_head {
                lbl.weight
            } else {
                -lbl.weight
            };

            if let Some(other_edge) = t.edge(child, other, None) {
                let other_cut_value = other_edge.cutvalue;
                cut_value += if points_to_head {
                    -other_cut_value
                } else {
                    other_cut_value
                };
            }
        });

        g.for_each_out_edge_ix(child_ix, None, |_tail_ix, head_ix, _ek, lbl| {
            if parent_ix.is_some_and(|p| head_ix == p) {
                return;
            }
            let Some(other) = g.node_id_by_ix(head_ix) else {
                return;
            };

            let points_to_head = child_is_tail;
            cut_value += if points_to_head {
                lbl.weight
            } else {
                -lbl.weight
            };

            if let Some(other_edge) = t.edge(child, other, None) {
                let other_cut_value = other_edge.cutvalue;
                cut_value += if points_to_head {
                    -other_cut_value
                } else {
                    other_cut_value
                };
            }
        });
    } else {
        // Graphlib `nodeEdges(child)` concatenates incoming edges before outgoing edges for both
        // directed and undirected graphs. Dugong's undirected adjacency iterator exposes every
        // incident edge, so filter its canonical endpoints into the same two ordered buckets.
        g.for_each_out_edge(child, None, |ek, lbl| {
            if ek.w != child {
                return;
            }
            let other = ek.v.as_str();
            if other == parent {
                return;
            }

            let points_to_head = !child_is_tail;
            cut_value += if points_to_head {
                lbl.weight
            } else {
                -lbl.weight
            };

            if let Some(other_edge) = t.edge(child, other, None) {
                let other_cut_value = other_edge.cutvalue;
                cut_value += if points_to_head {
                    -other_cut_value
                } else {
                    other_cut_value
                };
            }
        });

        g.for_each_out_edge(child, None, |ek, lbl| {
            if ek.v != child {
                return;
            }
            let other = ek.w.as_str();
            if other == parent {
                return;
            }

            let points_to_head = child_is_tail;
            cut_value += if points_to_head {
                lbl.weight
            } else {
                -lbl.weight
            };

            if let Some(other_edge) = t.edge(child, other, None) {
                let other_cut_value = other_edge.cutvalue;
                cut_value += if points_to_head {
                    -other_cut_value
                } else {
                    other_cut_value
                };
            }
        });
    }

    cut_value
}

pub fn leave_edge(t: &Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>) -> Option<EdgeKey> {
    edges::leave_edge(t)
}

pub fn enter_edge(
    t: &Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    rank_by_ix: &[i32],
    edge: &EdgeKey,
) -> EdgeKey {
    edges::enter_edge(t, g, rank_by_ix, edge)
}

pub fn exchange_edges(
    t: &mut Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    rank_by_ix: &mut Vec<i32>,
    e: &EdgeKey,
    f: &EdgeKey,
) {
    edges::exchange_edges(t, g, rank_by_ix, e, f);
}

// NOTE: Dagre treats the feasible tree as an undirected structure. We consider an edge to be a
// tree edge if it exists in `t` (queried via `t.edge(u, v, None)` in the hot loops).

#[cfg(test)]
mod tests;
