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
    tree: &Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
    graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    v_t_ix: usize,
    w_t_ix: usize,
) -> Option<DenseTreeEdge> {
    if v_t_ix == w_t_ix {
        return None;
    }
    let v = tree.node_id_by_ix(v_t_ix)?;
    let w = tree.node_id_by_ix(w_t_ix)?;
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
    fn new(
        t: &Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ) -> Result<Self, RankError> {
        if t.is_directed() || t.is_multigraph() {
            return Err(RankError::InvalidNetworkSimplexTree);
        }
        let t_len = t.node_slot_count();
        let g_len = g.node_slot_count();

        let mut g_ix_by_t_ix: Vec<Option<usize>> = vec![None; t_len];
        let mut t_ix_by_g_ix: Vec<Option<usize>> = vec![None; g_len];
        let mut node_ixs = Vec::with_capacity(t.node_count());
        let mut array_index_by_t_ix = vec![None; t_len];
        t.for_each_node_ix(|t_ix, id, _lbl| {
            let Some(g_ix) = g.node_ix(id) else {
                return;
            };
            g_ix_by_t_ix[t_ix] = Some(g_ix);
            t_ix_by_g_ix[g_ix] = Some(t_ix);
            node_ixs.push(t_ix);
            array_index_by_t_ix[t_ix] = t.javascript_array_index_by_node_ix(t_ix);
        });

        let mut tree_edges_in_order = Vec::with_capacity(t.edge_count());
        let mut array_index_adjacency_entry_count = 0usize;
        let mut tree_error = None;
        t.for_each_edge_ix(|v_ix, w_ix, _key, _lbl| {
            if tree_error.is_some() {
                return;
            }
            let Some(dense_edge) = capture_dense_tree_edge(t, g, v_ix, w_ix) else {
                tree_error = Some(RankError::InvalidNetworkSimplexTree);
                return;
            };
            tree_edges_in_order.push(dense_edge);
            let Some(v_array_index) = array_index_by_t_ix.get(v_ix) else {
                tree_error = Some(RankError::InvalidNetworkSimplexTree);
                return;
            };
            let Some(w_array_index) = array_index_by_t_ix.get(w_ix) else {
                tree_error = Some(RankError::InvalidNetworkSimplexTree);
                return;
            };
            let Ok(next_count) = checked_add(
                array_index_adjacency_entry_count,
                usize::from(v_array_index.is_some()),
            ) else {
                tree_error = Some(RankError::Work(WorkError::ArithmeticOverflow));
                return;
            };
            let Ok(next_count) = checked_add(next_count, usize::from(w_array_index.is_some()))
            else {
                tree_error = Some(RankError::Work(WorkError::ArithmeticOverflow));
                return;
            };
            array_index_adjacency_entry_count = next_count;
        });

        if let Some(error) = tree_error {
            return Err(error);
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
            has_array_index_nodes: t.array_index_node_count() != 0,
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
    // These graph and tree owners are deliberately boxed. In debug builds their aggregate return
    // places otherwise remain live together across the whole network-simplex coordinator, which
    // can exhaust small worker stacks even though every topology traversal is iterative.
    let mut simplified = build_simplified_graph_boxed(g, work_control)?;
    util::longest_path_controlled(&mut simplified, work_control)?;
    let t = build_feasible_tree_boxed_controlled(&mut simplified, work_control)?;
    let mut t_state = build_tree_state_boxed_controlled(&t, &simplified, None, work_control)?;
    drop(t);

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

fn build_feasible_tree_boxed_controlled(
    g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    work_control: &mut dyn WorkControl,
) -> Result<Box<Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>>, RankError> {
    Ok(Box::new(feasible_tree::feasible_tree_controlled(
        g,
        work_control,
    )?))
}

fn tree_state_new_work_units(
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    tree: &Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
) -> Result<usize, WorkError> {
    checked_add(
        g.node_slot_count(),
        checked_add(
            checked_add(tree.node_slot_count(), tree.node_order_slot_count())?,
            checked_mul(tree.edge_slot_count(), 2)?,
        )?,
    )
}

fn build_tree_state_controlled(
    tree: &Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    root: Option<&str>,
    work_control: &mut dyn WorkControl,
) -> Result<TreeState, RankError> {
    work_control.charge(tree_state_new_work_units(g, tree)?)?;
    let mut state = TreeState::new(tree, g)?;
    work_control.charge(dense_tree_state_rebuild_work_units(
        g,
        state.node_slot_count(),
        state.edge_count(),
        state.array_index_adjacency_entry_count,
    )?)?;
    state.rebuild(g, root)?;
    Ok(state)
}

fn build_tree_state_boxed_controlled(
    tree: &Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
    g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    root: Option<&str>,
    work_control: &mut dyn WorkControl,
) -> Result<Box<TreeState>, RankError> {
    Ok(Box::new(build_tree_state_controlled(
        tree,
        g,
        root,
        work_control,
    )?))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphlib::GraphOptions;

    #[derive(Default)]
    struct RecordingWorkControl {
        charges: Vec<usize>,
        remaining: Option<usize>,
    }

    impl RecordingWorkControl {
        fn with_limit(limit: usize) -> Self {
            Self {
                remaining: Some(limit),
                ..Self::default()
            }
        }
    }

    impl WorkControl for RecordingWorkControl {
        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            self.charges.push(units);
            let Some(remaining) = self.remaining else {
                return Ok(());
            };
            let Some(next) = remaining.checked_sub(units) else {
                return Err(WorkError::Interrupted);
            };
            self.remaining = Some(next);
            Ok(())
        }
    }

    fn ranking_graph() -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut graph = Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        graph.set_default_node_label(NodeLabel::default);
        graph.set_default_edge_label(|| EdgeLabel {
            minlen: 1,
            weight: 1.0,
            ..EdgeLabel::default()
        });
        graph.set_path(&["a", "b", "c", "d"]);
        graph.set_edge("a", "c");
        graph.set_edge("b", "d");
        graph
    }

    fn tree(edges: &[(&str, &str)]) -> Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()> {
        let mut tree = Graph::new(GraphOptions {
            directed: false,
            ..GraphOptions::default()
        });
        tree.set_default_node_label(tree::TreeNodeLabel::default);
        tree.set_default_edge_label(tree::TreeEdgeLabel::default);
        for &(source, target) in edges {
            tree.set_edge(source, target);
        }
        tree
    }

    fn numeric_path_tree_fixture(
        width: usize,
        numeric_leaf: bool,
    ) -> (
        Graph<tree::TreeNodeLabel, tree::TreeEdgeLabel, ()>,
        Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ) {
        let mut spanning_tree = Graph::new(GraphOptions {
            directed: false,
            ..GraphOptions::default()
        });
        spanning_tree.set_default_node_label(tree::TreeNodeLabel::default);
        spanning_tree.set_default_edge_label(tree::TreeEdgeLabel::default);

        let leaf = if numeric_leaf { "0" } else { "leaf" };
        spanning_tree.set_edge(leaf, "node-0");
        for index in 1..width {
            spanning_tree.set_edge(format!("node-{}", index - 1), format!("node-{index}"));
        }

        let mut graph = Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        for edge in spanning_tree.edge_keys() {
            graph.set_edge_with_label(
                edge.v,
                edge.w,
                EdgeLabel {
                    minlen: 1,
                    weight: 1.0,
                    ..EdgeLabel::default()
                },
            );
        }
        (spanning_tree, graph)
    }

    fn tree_node_id<'a>(
        state: &TreeState,
        graph: &'a Graph<NodeLabel, EdgeLabel, GraphLabel>,
        t_ix: usize,
    ) -> &'a str {
        state
            .g_ix_by_t_ix
            .get(t_ix)
            .copied()
            .flatten()
            .and_then(|g_ix| graph.node_id_by_ix(g_ix))
            .expect("tree nodes map to simplified graph nodes")
    }

    fn ordered_tree_edges(
        state: &TreeState,
        graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ) -> Vec<(String, String)> {
        state
            .tree_edges_in_order
            .iter()
            .map(|edge| {
                (
                    tree_node_id(state, graph, edge.v_t_ix).to_string(),
                    tree_node_id(state, graph, edge.w_t_ix).to_string(),
                )
            })
            .collect()
    }

    fn multi_pivot_graph() -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut graph = Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        graph.set_default_node_label(NodeLabel::default);
        for (v, w, minlen, weight) in [
            ("n0", "n1", 2, 1.0),
            ("n0", "n4", 3, 1.0),
            ("n1", "n3", 3, 6.0),
            ("n1", "n4", 1, 1.0),
            ("n1", "n5", 1, 7.0),
            ("n2", "n3", 3, 7.0),
            ("n2", "n4", 1, 4.0),
            ("n3", "n8", 2, 7.0),
            ("n4", "n5", 3, 3.0),
            ("n4", "n7", 2, 5.0),
            ("n4", "n8", 3, 4.0),
            ("n5", "n9", 3, 6.0),
            ("n7", "n9", 1, 2.0),
            ("n8", "n9", 2, 1.0),
        ] {
            graph.set_edge_with_label(
                v,
                w,
                EdgeLabel {
                    minlen,
                    weight,
                    ..EdgeLabel::default()
                },
            );
        }
        graph
    }

    fn first_multi_pivot_state() -> (
        TreeState,
        Graph<NodeLabel, EdgeLabel, GraphLabel>,
        Vec<i128>,
        OrderedTreeEdge,
    ) {
        let source = multi_pivot_graph();
        let mut work_control = NoopWorkControl;
        let mut graph = build_simplified_graph(&source, &mut work_control)
            .expect("the fixture simplify step succeeds");
        util::longest_path_controlled(&mut graph, &mut work_control)
            .expect("the fixture ranks fit i32");
        let tree = feasible_tree::feasible_tree_controlled(&mut graph, &mut work_control)
            .expect("the fixture feasible tree succeeds");
        let mut state = TreeState::new(&tree, &graph).expect("the tree is a graph-edge subset");
        drop(tree);
        state
            .rebuild(&graph, None)
            .expect("the dense tree rebuild succeeds");

        let mut rank_by_ix = vec![0_i128; graph.node_slot_count()];
        graph.for_each_node_ix(|g_ix, _id, label| {
            rank_by_ix[g_ix] = i128::from(label.rank.unwrap_or(0));
        });
        let leaving = state
            .find_leave_edge_in_insertion_order()
            .expect("the pinned fixture begins with a negative tree edge");
        (state, graph, rank_by_ix, leaving)
    }

    fn rank_snapshot(
        graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ) -> Vec<(String, Option<i32>)> {
        graph
            .nodes()
            .map(|id| (id.to_string(), graph.node(id).and_then(|node| node.rank)))
            .collect()
    }

    fn mixed_simplify_graph(numeric: bool) -> Graph<NodeLabel, EdgeLabel, GraphLabel> {
        let mut graph = Graph::new(GraphOptions {
            multigraph: true,
            ..GraphOptions::default()
        });
        graph.set_graph(GraphLabel::default());
        let ids = if numeric {
            ["0", "node-a", "1", "node-b"]
        } else {
            ["node-0", "node-a", "node-1", "node-b"]
        };
        for id in ids {
            graph.set_node(id, NodeLabel::default());
        }
        for (v, w, name, weight) in [
            (ids[0], ids[1], "parallel-0", 1.0),
            (ids[0], ids[1], "parallel-1", 2.0),
            (ids[1], ids[2], "mixed", 3.0),
            (ids[0], ids[2], "numeric", 4.0),
            (ids[2], ids[2], "self", 5.0),
            (ids[1], ids[3], "ordinary", 6.0),
        ] {
            graph.set_edge_named(
                v,
                w,
                Some(name),
                Some(EdgeLabel {
                    weight,
                    minlen: 1,
                    ..EdgeLabel::default()
                }),
            );
        }
        graph
    }

    #[test]
    fn simplify_work_curve_includes_numeric_object_key_rebuilds() {
        for width in (0..=10).map(|shift| 1usize << shift) {
            let mut numeric = Graph::new(GraphOptions::default());
            let mut ordinary = Graph::new(GraphOptions::default());
            for index in (0..width).rev() {
                numeric.set_node(index.to_string(), NodeLabel::default());
                ordinary.set_node(format!("node-{index}"), NodeLabel::default());
            }
            for index in 1..width {
                numeric.set_edge((index - 1).to_string(), index.to_string());
                ordinary.set_edge(format!("node-{}", index - 1), format!("node-{index}"));
            }

            let numeric_work = simplify_work_units(&numeric).unwrap();
            let ordinary_work = simplify_work_units(&ordinary).unwrap();
            let numeric_updates = width + (width.saturating_sub(1) * 2);
            let ordered_work = checked_ordered_key_updates(width, numeric_updates).unwrap();
            assert_eq!(numeric_work, ordinary_work + ordered_work);
        }
    }

    #[test]
    fn simplify_work_uses_one_ordered_merge_and_charges_tombstone_slots() {
        let mut graph = mixed_simplify_graph(false);
        let expected_dense = checked_add(
            checked_mul(graph.node_count(), 2).unwrap(),
            checked_add(
                checked_add(
                    checked_mul(graph.edge_slot_count(), 2).unwrap(),
                    checked_mul(graph.edge_count(), 3).unwrap(),
                )
                .unwrap(),
                checked_n_log_n(graph.edge_count()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(simplify_work_units(&graph), Ok(expected_dense));

        assert!(graph.remove_edge("node-0", "node-a", Some("parallel-0")));
        assert_eq!(graph.edge_slot_count(), 6);
        assert_eq!(graph.edge_count(), 5);
        let expected_sparse = checked_add(
            checked_mul(graph.node_count(), 2).unwrap(),
            checked_add(
                checked_add(
                    checked_mul(graph.edge_slot_count(), 2).unwrap(),
                    checked_mul(graph.edge_count(), 3).unwrap(),
                )
                .unwrap(),
                checked_n_log_n(graph.edge_count()).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(simplify_work_units(&graph), Ok(expected_sparse));
    }

    #[test]
    fn simplify_precharges_mixed_numeric_directed_adjacency() {
        let numeric = mixed_simplify_graph(true);
        let ordinary = mixed_simplify_graph(false);
        let adjacency_entries = numeric.directed_array_index_adjacency_entry_count();
        assert_eq!(adjacency_entries, 6);
        assert_eq!(ordinary.directed_array_index_adjacency_entry_count(), 0);

        let node_order_work =
            checked_ordered_key_updates(numeric.node_count(), numeric.array_index_node_count())
                .unwrap();
        let adjacency_work =
            checked_ordered_key_updates(numeric.node_count(), adjacency_entries).unwrap();
        let numeric_exact = simplify_work_units(&numeric).unwrap();
        let ordinary_exact = simplify_work_units(&ordinary).unwrap();
        assert_eq!(
            numeric_exact,
            checked_add(
                ordinary_exact,
                checked_add(node_order_work, adjacency_work).unwrap(),
            )
            .unwrap()
        );

        let mut measured = RecordingWorkControl::default();
        let simplified = build_simplified_graph(&numeric, &mut measured)
            .expect("the unbounded control admits the numeric simplify copy");
        assert_eq!(measured.charges, [numeric_exact]);
        assert_eq!(simplified.edge_count(), 5);
        assert_eq!(
            simplified.directed_array_index_adjacency_entry_count(),
            adjacency_entries
        );

        let source_nodes = numeric.node_ids();
        let source_edges = numeric.edge_keys();
        let mut below = RecordingWorkControl::with_limit(numeric_exact - 1);
        assert!(matches!(
            build_simplified_graph(&numeric, &mut below),
            Err(WorkError::Interrupted)
        ));
        assert_eq!(below.charges, [numeric_exact]);
        assert_eq!(below.remaining, Some(numeric_exact - 1));
        assert_eq!(numeric.node_ids(), source_nodes);
        assert_eq!(numeric.edge_keys(), source_edges);

        for limit in [numeric_exact, numeric_exact + 1] {
            let mut admitted = RecordingWorkControl::with_limit(limit);
            let simplified = build_simplified_graph(&numeric, &mut admitted)
                .expect("equal and above numeric simplify-copy budgets succeed");
            assert_eq!(simplified.edge_count(), 5);
        }
    }

    #[test]
    fn dense_tree_exchange_preserves_stable_delete_then_append_order() {
        let graph = ranking_graph();
        let first_tree = tree(&[("a", "b"), ("b", "c"), ("c", "d")]);
        let mut state =
            TreeState::new(&first_tree, &graph).expect("the tree is a graph-edge subset");
        let edge_capacity = state.tree_edges_in_order.capacity();

        state
            .exchange_edge(
                OrderedTreeEdge {
                    position: 1,
                    v_ix: first_tree.node_ix("b").expect("b is present"),
                    w_ix: first_tree.node_ix("c").expect("c is present"),
                },
                &EdgeKey {
                    v: "a".to_string(),
                    w: "c".to_string(),
                    name: None,
                },
                &graph,
            )
            .expect("the tree exchange is valid");

        assert_eq!(
            ordered_tree_edges(&state, &graph),
            [("a", "b"), ("c", "d"), ("a", "c")].map(|(v, w)| (v.to_string(), w.to_string()))
        );
        assert_eq!(state.edge_count(), 3);
        assert_eq!(state.tree_edges_in_order.capacity(), edge_capacity);

        state
            .rebuild(&graph, Some("a"))
            .expect("the exchanged tree rebuild succeeds");
        let second_tree = tree(&[("a", "b"), ("c", "d"), ("a", "c")]);
        let mut fresh =
            TreeState::new(&second_tree, &graph).expect("the tree is a graph-edge subset");
        fresh
            .rebuild(&graph, Some("a"))
            .expect("the fresh tree rebuild succeeds");
        assert_eq!(state, fresh);
    }

    #[test]
    fn generic_graph_tree_slots_grow_across_the_pinned_pivot_trace() {
        let mut generic_tree = tree(&[
            ("n0", "n1"),
            ("n1", "n3"),
            ("n2", "n3"),
            ("n2", "n4"),
            ("n4", "n5"),
            ("n5", "n9"),
            ("n7", "n9"),
            ("n8", "n9"),
        ]);
        let mut slot_curve = vec![generic_tree.edge_slot_count()];
        for ((leave_v, leave_w), (enter_v, enter_w)) in [
            (("n2", "n3"), ("n0", "n4")),
            (("n0", "n1"), ("n1", "n4")),
            (("n7", "n9"), ("n4", "n7")),
            (("n8", "n9"), ("n3", "n8")),
            (("n1", "n4"), ("n0", "n1")),
            (("n0", "n4"), ("n2", "n3")),
        ] {
            assert!(generic_tree.remove_edge(leave_v, leave_w, None));
            generic_tree.set_edge(enter_v, enter_w);
            assert_eq!(generic_tree.edge_count(), 8);
            slot_curve.push(generic_tree.edge_slot_count());
        }
        assert_eq!(slot_curve, [8, 9, 10, 11, 12, 13, 14]);
    }

    #[test]
    fn dense_tree_storage_stays_fixed_across_a_long_exchange_history() {
        const PIVOTS: usize = 256;
        const TREE_EDGES: usize = 4;
        const NODE_IDS: [&str; 5] = ["a", "b", "c", "d", "e"];

        let mut graph = Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        graph.set_default_node_label(NodeLabel::default);
        let mut complete_edges = Vec::new();
        for (position, &v) in NODE_IDS.iter().enumerate() {
            for &w in &NODE_IDS[position + 1..] {
                complete_edges.push((v, w));
                graph.set_edge_with_label(
                    v,
                    w,
                    EdgeLabel {
                        minlen: 1,
                        weight: 1.0,
                        ..EdgeLabel::default()
                    },
                );
            }
        }

        let initial_edges = [("a", "b"), ("b", "c"), ("c", "d"), ("d", "e")];
        let source_tree = tree(&initial_edges);
        let mut generic_tree = tree(&initial_edges);
        let mut state =
            TreeState::new(&source_tree, &graph).expect("the tree is a graph-edge subset");
        let dense_capacity = state.tree_edges_in_order.capacity();
        let mut dense_scan_entries = 0usize;
        let mut generic_slot_scan_entries = 0usize;

        for _ in 0..PIVOTS {
            let leaving_edge = state.tree_edges_in_order[0];
            let leaving_ends = (leaving_edge.v_t_ix, leaving_edge.w_t_ix);
            let leave_v = tree_node_id(&state, &graph, leaving_ends.0).to_string();
            let leave_w = tree_node_id(&state, &graph, leaving_ends.1).to_string();

            let mut reachable = vec![false; state.g_ix_by_t_ix.len()];
            let mut stack = vec![leaving_ends.0];
            reachable[leaving_ends.0] = true;
            while let Some(node_ix) = stack.pop() {
                for edge in &state.tree_edges_in_order[1..] {
                    let (v_ix, w_ix) = (edge.v_t_ix, edge.w_t_ix);
                    let adjacent = if v_ix == node_ix {
                        Some(w_ix)
                    } else if w_ix == node_ix {
                        Some(v_ix)
                    } else {
                        None
                    };
                    let Some(adjacent) = adjacent else {
                        continue;
                    };
                    if !reachable[adjacent] {
                        reachable[adjacent] = true;
                        stack.push(adjacent);
                    }
                }
            }

            let &(enter_v, enter_w) = complete_edges
                .iter()
                .find(|&&(v, w)| {
                    let v_ix = source_tree.node_ix(v).unwrap();
                    let w_ix = source_tree.node_ix(w).unwrap();
                    let candidate = (v_ix, w_ix);
                    let same_edge = |ends: (usize, usize)| {
                        ends == candidate || ends == (candidate.1, candidate.0)
                    };
                    !same_edge(leaving_ends)
                        && !state
                            .tree_edges_in_order
                            .iter()
                            .map(|edge| (edge.v_t_ix, edge.w_t_ix))
                            .any(same_edge)
                        && reachable[v_ix] != reachable[w_ix]
                })
                .expect("the complete graph supplies another cross-component edge");
            state
                .exchange_edge(
                    OrderedTreeEdge {
                        position: 0,
                        v_ix: leaving_ends.0,
                        w_ix: leaving_ends.1,
                    },
                    &EdgeKey {
                        v: enter_v.to_string(),
                        w: enter_w.to_string(),
                        name: None,
                    },
                    &graph,
                )
                .expect("the selected cross-component exchange preserves a connected tree");

            assert!(generic_tree.remove_edge(&leave_v, &leave_w, None));
            generic_tree.set_edge(enter_v, enter_w);

            let generic_order = generic_tree
                .edge_keys()
                .into_iter()
                .map(|edge| (edge.v, edge.w))
                .collect::<Vec<_>>();
            assert_eq!(ordered_tree_edges(&state, &graph), generic_order);
            assert_eq!(state.edge_count(), TREE_EDGES);
            assert_eq!(state.tree_edges_in_order.capacity(), dense_capacity);

            dense_scan_entries += state.edge_count();
            generic_slot_scan_entries += generic_tree.edge_slot_count();
        }

        assert_eq!(generic_tree.edge_slot_count(), TREE_EDGES + PIVOTS);
        assert_eq!(dense_scan_entries, TREE_EDGES * PIVOTS);
        assert_eq!(
            generic_slot_scan_entries,
            TREE_EDGES * PIVOTS + PIVOTS * (PIVOTS + 1) / 2
        );
    }

    #[test]
    fn dense_pivot_precharges_post_exchange_numeric_adjacency_upper_bound() {
        let mut graph = Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        graph.set_default_node_label(NodeLabel::default);
        for (v, w) in [("2", "a"), ("a", "b"), ("b", "10"), ("2", "10")] {
            graph.set_edge_with_label(
                v,
                w,
                EdgeLabel {
                    minlen: 1,
                    weight: 1.0,
                    ..EdgeLabel::default()
                },
            );
        }
        let spanning_tree = tree(&[("2", "a"), ("a", "b"), ("b", "10")]);
        let mut state =
            TreeState::new(&spanning_tree, &graph).expect("the tree is a graph-edge subset");
        assert!(state.has_array_index_nodes);
        assert_eq!(state.array_index_adjacency_entry_count, 2);

        let entering_and_rank_work = checked_add(
            checked_mul(graph.node_count(), 2).unwrap(),
            graph.edge_count(),
        )
        .unwrap();
        let stable_exchange_work = checked_mul(state.edge_count(), 2).unwrap();
        let common_work = checked_add(entering_and_rank_work, stable_exchange_work).unwrap();
        let current_rebuild_work = dense_tree_state_rebuild_work_units(
            &graph,
            state.node_slot_count(),
            state.edge_count(),
            state.array_index_adjacency_entry_count,
        )
        .unwrap();
        let post_exchange_upper_bound = std::cmp::min(
            checked_mul(state.edge_count(), 2).unwrap(),
            checked_add(state.array_index_adjacency_entry_count, 2).unwrap(),
        );
        assert_eq!(post_exchange_upper_bound, 4);
        let charged_rebuild_work = dense_tree_state_rebuild_work_units(
            &graph,
            state.node_slot_count(),
            state.edge_count(),
            post_exchange_upper_bound,
        )
        .unwrap();
        let exact = simplex_iteration_work_units(&graph, &state).unwrap();
        assert_eq!(
            exact,
            checked_add(common_work, charged_rebuild_work).unwrap()
        );
        assert!(exact > checked_add(common_work, current_rebuild_work).unwrap());

        let leaving = OrderedTreeEdge {
            position: 1,
            v_ix: spanning_tree.node_ix("a").expect("a is present"),
            w_ix: spanning_tree.node_ix("b").expect("b is present"),
        };
        state
            .exchange_edge(
                leaving,
                &EdgeKey {
                    v: "2".to_string(),
                    w: "10".to_string(),
                    name: None,
                },
                &graph,
            )
            .expect("the numeric entering edge reconnects the two tree components");
        assert_eq!(state.array_index_adjacency_entry_count, 4);
        assert!(state.array_index_adjacency_entry_count <= post_exchange_upper_bound);
        state
            .rebuild(&graph, None)
            .expect("the post-exchange tree rebuild succeeds");
    }

    #[test]
    fn tree_state_uses_pinned_graphlib_neighbor_object_key_order() {
        let mut spanning_tree = tree(&[
            ("m", "z"),
            ("a", "m"),
            ("m", "y"),
            ("b", "m"),
            ("10", "m"),
            ("2", "m"),
        ]);
        let mut graph = Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        for id in spanning_tree.node_ids() {
            graph.set_node(id, NodeLabel::default());
        }
        for edge in spanning_tree.edge_keys() {
            graph.set_edge_with_label(
                edge.v,
                edge.w,
                EdgeLabel {
                    minlen: 1,
                    weight: 1.0,
                    ..EdgeLabel::default()
                },
            );
        }

        let mut state =
            TreeState::new(&spanning_tree, &graph).expect("the tree is a graph-edge subset");
        state
            .rebuild(&graph, Some("m"))
            .expect("the mixed-key tree rebuild succeeds");
        let m_tix = spanning_tree.node_ix("m").expect("m is present");
        let neighbors = state
            .neighbors
            .slice(m_tix)
            .iter()
            .map(|&t_ix| tree_node_id(&state, &graph, t_ix))
            .collect::<Vec<_>>();

        // dagre-d3-es 7.0.14 Graph.neighbors is union(predecessors, successors). Each bucket uses
        // Object.keys order, so numeric IDs precede ordinary IDs and are sorted numerically.
        assert_eq!(neighbors, ["2", "10", "a", "b", "z", "y"]);
        for (id, expected_lim) in [("2", 1), ("10", 2), ("a", 3), ("b", 4), ("z", 5), ("y", 6)] {
            let g_ix = graph.node_ix(id).expect("fixture node is present");
            assert_eq!(
                state.node_low_lim_by_gix(g_ix),
                Some((expected_lim, expected_lim))
            );
            let t_ix = spanning_tree
                .node_ix(id)
                .expect("fixture tree node is present");
            assert_eq!(state.parent_t_ix[t_ix], Some(m_tix));
        }
        let m_gix = graph.node_ix("m").expect("m is present");
        assert_eq!(state.node_low_lim_by_gix(m_gix), Some((1, 7)));
        assert_eq!(state.parent_t_ix[m_tix], None);

        init_low_lim_values(&mut spanning_tree, Some("m"));
        for (id, expected_lim) in [("2", 1), ("10", 2), ("a", 3), ("b", 4), ("z", 5), ("y", 6)] {
            let label = spanning_tree
                .node(id)
                .expect("fixture tree label is present");
            assert_eq!((label.low, label.lim), (expected_lim, expected_lim));
            assert_eq!(label.parent.as_deref(), Some("m"));
        }
        let root = spanning_tree.node("m").expect("root tree label is present");
        assert_eq!((root.low, root.lim), (1, 7));
        assert_eq!(root.parent, None);
    }

    #[test]
    fn numeric_neighbor_storage_scales_with_numeric_entries_not_node_slots() {
        for width in [64, 256, 1024] {
            let (numeric_tree, numeric_graph) = numeric_path_tree_fixture(width, true);
            let mut numeric_state = TreeState::new(&numeric_tree, &numeric_graph)
                .expect("the numeric tree is a graph-edge subset");
            numeric_state
                .rebuild(&numeric_graph, None)
                .expect("the numeric star rebuild succeeds");
            assert_eq!(numeric_state.array_index_adjacency_entry_count, 1);
            assert_eq!(numeric_state.neighbors.offsets.len(), width + 2);
            assert_eq!(numeric_state.neighbors.build_state.len(), width + 1);
            assert_eq!(numeric_state.neighbors.entries.len(), width * 2);
            assert_eq!(numeric_state.neighbors.numeric_entries.len(), 1);

            let (ordinary_tree, ordinary_graph) = numeric_path_tree_fixture(width, false);
            let mut ordinary_state = TreeState::new(&ordinary_tree, &ordinary_graph)
                .expect("the ordinary tree is a graph-edge subset");
            ordinary_state
                .rebuild(&ordinary_graph, None)
                .expect("the ordinary star rebuild succeeds");
            assert_eq!(ordinary_state.array_index_adjacency_entry_count, 0);
            assert_eq!(ordinary_state.neighbors.offsets.len(), width + 2);
            assert_eq!(ordinary_state.neighbors.build_state.len(), width + 1);
            assert_eq!(ordinary_state.neighbors.entries.len(), width * 2);
            assert!(ordinary_state.neighbors.numeric_entries.is_empty());

            let numeric_work = dense_tree_state_rebuild_work_units(
                &numeric_graph,
                numeric_state.node_slot_count(),
                numeric_state.edge_count(),
                numeric_state.array_index_adjacency_entry_count,
            )
            .unwrap();
            let ordinary_work = dense_tree_state_rebuild_work_units(
                &ordinary_graph,
                ordinary_state.node_slot_count(),
                ordinary_state.edge_count(),
                ordinary_state.array_index_adjacency_entry_count,
            )
            .unwrap();
            assert_eq!(
                numeric_work,
                checked_add(
                    ordinary_work,
                    checked_ordered_key_updates(width + 1, 1).unwrap(),
                )
                .unwrap()
            );
        }
    }

    #[test]
    fn initial_tree_state_work_scales_with_live_numeric_adjacency_entries() {
        for width in [64, 256, 1024] {
            let (numeric_tree, numeric_graph) = numeric_path_tree_fixture(width, true);
            let numeric_state = TreeState::new(&numeric_tree, &numeric_graph)
                .expect("the numeric tree is a graph-edge subset");
            let (ordinary_tree, ordinary_graph) = numeric_path_tree_fixture(width, false);
            let ordinary_state = TreeState::new(&ordinary_tree, &ordinary_graph)
                .expect("the ordinary tree is a graph-edge subset");
            let numeric_work = dense_tree_state_rebuild_work_units(
                &numeric_graph,
                numeric_state.node_slot_count(),
                numeric_state.edge_count(),
                numeric_state.array_index_adjacency_entry_count,
            )
            .unwrap();
            let ordinary_work = dense_tree_state_rebuild_work_units(
                &ordinary_graph,
                ordinary_state.node_slot_count(),
                ordinary_state.edge_count(),
                ordinary_state.array_index_adjacency_entry_count,
            )
            .unwrap();

            assert_eq!(
                numeric_work,
                checked_add(
                    ordinary_work,
                    checked_ordered_key_updates(width + 1, 1).unwrap(),
                )
                .unwrap()
            );
        }
    }

    #[test]
    fn initial_tree_state_budget_uses_the_exact_numeric_adjacency_count() {
        let mut spanning_tree = Graph::new(GraphOptions {
            directed: false,
            ..GraphOptions::default()
        });
        spanning_tree.set_default_node_label(tree::TreeNodeLabel::default);
        spanning_tree.set_default_edge_label(tree::TreeEdgeLabel::default);
        spanning_tree.set_edge("0", "node-0");
        for index in 1..128 {
            spanning_tree.set_edge(format!("node-{}", index - 1), format!("node-{index}"));
        }

        let mut graph = Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        for edge in spanning_tree.edge_keys() {
            graph.set_edge_with_label(
                edge.v,
                edge.w,
                EdgeLabel {
                    minlen: 1,
                    weight: 1.0,
                    ..EdgeLabel::default()
                },
            );
        }

        assert_eq!(spanning_tree.pinned_array_index_adjacency_entry_count(), 1);
        let expected_state =
            TreeState::new(&spanning_tree, &graph).expect("the tree is a graph-edge subset");
        let construction = tree_state_new_work_units(&graph, &spanning_tree).unwrap();
        let rebuild = dense_tree_state_rebuild_work_units(
            &graph,
            expected_state.node_slot_count(),
            expected_state.edge_count(),
            expected_state.array_index_adjacency_entry_count,
        )
        .unwrap();
        let exact = checked_add(construction, rebuild).unwrap();

        let mut below_construction = RecordingWorkControl::with_limit(construction - 1);
        assert!(matches!(
            build_tree_state_controlled(&spanning_tree, &graph, None, &mut below_construction),
            Err(RankError::Work(WorkError::Interrupted))
        ));
        assert_eq!(below_construction.charges, [construction]);
        assert_eq!(below_construction.remaining, Some(construction - 1));

        let mut below_rebuild = RecordingWorkControl::with_limit(exact - 1);
        assert!(matches!(
            build_tree_state_controlled(&spanning_tree, &graph, None, &mut below_rebuild),
            Err(RankError::Work(WorkError::Interrupted))
        ));
        assert_eq!(below_rebuild.charges, [construction, rebuild]);
        assert_eq!(below_rebuild.remaining, Some(rebuild - 1));

        for limit in [exact, exact + 1] {
            let mut admitted = RecordingWorkControl::with_limit(limit);
            let state = build_tree_state_controlled(&spanning_tree, &graph, None, &mut admitted)
                .expect("equal and above tree-state budgets succeed");
            assert_eq!(state.array_index_adjacency_entry_count, 1);
            assert_eq!(state.tree_edges_in_order.len(), spanning_tree.edge_count());
            assert_eq!(admitted.charges, [construction, rebuild]);
            assert_eq!(admitted.remaining, Some(limit - exact));
        }
    }

    #[test]
    fn initial_tree_state_precharges_sparse_slots_and_a_cold_graph_cache() {
        let mut spanning_tree = tree(&[("temporary", "removed"), ("a", "b"), ("b", "c")]);
        assert!(spanning_tree.remove_edge("temporary", "removed", None));
        assert!(spanning_tree.remove_node("temporary"));
        assert!(spanning_tree.remove_node("removed"));

        let mut graph = Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        for (v, w) in [("temporary", "removed"), ("a", "b"), ("b", "c")] {
            graph.set_edge_with_label(
                v,
                w,
                EdgeLabel {
                    minlen: 1,
                    weight: 1.0,
                    ..EdgeLabel::default()
                },
            );
        }
        assert!(graph.remove_edge("temporary", "removed", None));
        assert!(graph.remove_node("temporary"));
        assert!(graph.remove_node("removed"));
        assert!(!graph.is_adjacency_cache_current());
        assert!(spanning_tree.node_slot_count() > spanning_tree.node_count());
        assert!(spanning_tree.edge_slot_count() > spanning_tree.edge_count());
        assert!(graph.node_slot_count() > graph.node_count());
        assert!(graph.edge_slot_count() > graph.edge_count());

        let expected_state =
            TreeState::new(&spanning_tree, &graph).expect("the live tree remains valid");
        let construction = tree_state_new_work_units(&graph, &spanning_tree).unwrap();
        let rebuild = dense_tree_state_rebuild_work_units(
            &graph,
            expected_state.node_slot_count(),
            expected_state.edge_count(),
            expected_state.array_index_adjacency_entry_count,
        )
        .unwrap();
        let exact = checked_add(construction, rebuild).unwrap();
        let mut work_control = RecordingWorkControl::with_limit(exact);
        let state = build_tree_state_controlled(&spanning_tree, &graph, None, &mut work_control)
            .expect("slot-aware exact budget admits the sparse fixture");

        assert_eq!(work_control.charges, [construction, rebuild]);
        assert_eq!(work_control.remaining, Some(0));
        assert_eq!(state.node_count(), 3);
        assert!(graph.is_adjacency_cache_current());
    }

    #[test]
    fn dense_tree_edges_remove_high_fanout_endpoint_rescans() {
        const LEAVES: usize = 512;

        let mut spanning_tree = Graph::new(GraphOptions {
            directed: false,
            ..GraphOptions::default()
        });
        spanning_tree.set_default_node_label(tree::TreeNodeLabel::default);
        spanning_tree.set_default_edge_label(tree::TreeEdgeLabel::default);

        let mut graph = Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        graph.set_node("root", NodeLabel::default());
        for index in 0..LEAVES {
            let leaf = format!("leaf-{index}");
            let minlen = index + 1;
            let weight = index as f64 + 0.5;
            spanning_tree.set_edge("root", leaf.clone());
            graph.set_edge_with_label(
                "root",
                leaf,
                EdgeLabel {
                    minlen,
                    weight,
                    ..EdgeLabel::default()
                },
            );
        }

        let mut state =
            TreeState::new(&spanning_tree, &graph).expect("the tree is a graph-edge subset");
        assert_eq!(state.tree_edges_in_order.len(), LEAVES);

        // Rebuild and rank propagation own immutable snapshots. Mutating the source labels after
        // construction proves these O(V) passes no longer query the root's growing out-edge list.
        for index in 0..LEAVES {
            let leaf = format!("leaf-{index}");
            let edge = graph
                .edge_mut("root", &leaf, None)
                .expect("the star edge exists");
            edge.minlen = usize::MAX;
            edge.weight = -1.0;
        }

        state
            .rebuild(&graph, Some("root"))
            .expect("the wide star rebuild succeeds");
        let root_tix = spanning_tree.node_ix("root").expect("root is present");
        for index in 0..LEAVES {
            let leaf = format!("leaf-{index}");
            let leaf_tix = spanning_tree.node_ix(&leaf).expect("leaf is present");
            assert_eq!(state.parent_t_ix[leaf_tix], Some(root_tix));
            let parent_edge = state.parent_edge_position_by_t_ix[leaf_tix]
                .and_then(|position| state.tree_edges_in_order.get(position))
                .expect("the leaf retains one parent edge");
            assert_eq!(parent_edge.minlen, index + 1);
            assert_eq!(parent_edge.weight, index as f64 + 0.5);
        }

        let mut rank_by_ix = vec![0_i128; graph.node_slot_count()];
        update_ranks_fast(&mut state, &mut graph, &mut rank_by_ix)
            .expect("snapshot minlen values fit the rank domain");
        for index in 0..LEAVES {
            let leaf = format!("leaf-{index}");
            assert_eq!(
                graph.node(&leaf).and_then(|node| node.rank),
                Some((index + 1) as i32)
            );
        }
    }

    #[test]
    fn cut_value_accumulates_incoming_edges_before_outgoing_edges() {
        const TWO_TO_53: f64 = 9_007_199_254_740_992.0;

        let mut graph = Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        for (v, w, weight) in [
            ("c", "p", 1.0),
            ("x", "p", 0.0),
            ("y", "p", 0.0),
            ("z", "p", 0.0),
            ("x", "c", TWO_TO_53),
            ("y", "c", 1.0),
            ("c", "z", TWO_TO_53),
        ] {
            graph.set_edge_with_label(
                v,
                w,
                EdgeLabel {
                    minlen: 1,
                    weight,
                    ..EdgeLabel::default()
                },
            );
        }

        let mut spanning_tree = tree(&[("c", "p"), ("p", "x"), ("p", "y"), ("p", "z")]);
        init_low_lim_values(&mut spanning_tree, Some("p"));
        assert_eq!(calc_cut_value(&spanning_tree, &graph, "c"), 0.0);

        let mut state =
            TreeState::new(&spanning_tree, &graph).expect("the tree is a graph-edge subset");
        state
            .rebuild(&graph, Some("p"))
            .expect("the parallel-edge tree rebuild succeeds");
        let c_tix = spanning_tree.node_ix("c").expect("c is present");
        assert_eq!(state.cut_to_parent[c_tix], 0.0);

        let mut undirected = Graph::new(GraphOptions {
            directed: false,
            ..GraphOptions::default()
        });
        undirected.set_graph(GraphLabel::default());
        for (v, w, weight) in [
            ("m", "z", 1.0),
            ("a", "m", TWO_TO_53),
            ("b", "m", 1.0),
            ("m", "x", TWO_TO_53),
        ] {
            undirected.set_edge_with_label(
                v,
                w,
                EdgeLabel {
                    minlen: 1,
                    weight,
                    ..EdgeLabel::default()
                },
            );
        }
        let mut undirected_tree = tree(&[("m", "z"), ("z", "a"), ("z", "b"), ("z", "x")]);
        init_low_lim_values(&mut undirected_tree, Some("z"));
        assert_eq!(calc_cut_value(&undirected_tree, &undirected, "m"), 0.0);
    }

    #[test]
    fn tree_state_counts_numeric_endpoints_and_rejects_self_loops() {
        let numeric_tree = tree(&[("0", "1")]);
        let mut graph = Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        graph.set_edge_with_label(
            "0",
            "1",
            EdgeLabel {
                minlen: 1,
                weight: 1.0,
                ..EdgeLabel::default()
            },
        );
        let state = TreeState::new(&numeric_tree, &graph).expect("a simple numeric tree is valid");
        assert_eq!(state.array_index_adjacency_entry_count, 2);

        let self_loop_tree = tree(&[("0", "0")]);
        let mut self_loop_graph = Graph::new(GraphOptions::default());
        self_loop_graph.set_graph(GraphLabel::default());
        self_loop_graph.set_edge_with_label(
            "0",
            "0",
            EdgeLabel {
                minlen: 1,
                weight: 1.0,
                ..EdgeLabel::default()
            },
        );
        assert_eq!(
            TreeState::new(&self_loop_tree, &self_loop_graph),
            Err(RankError::InvalidNetworkSimplexTree)
        );
    }

    #[test]
    fn network_simplex_rejects_a_negative_tree_without_an_entering_edge() {
        let mut graph = Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        graph.set_edge_with_label(
            "a",
            "b",
            EdgeLabel {
                minlen: 1,
                weight: -1.0,
                ..EdgeLabel::default()
            },
        );
        let source_nodes = graph.node_ids();
        let source_edges = graph.edge_keys();

        let mut work_control = NoopWorkControl;
        assert_eq!(
            network_simplex_controlled(&mut graph, &mut work_control),
            Err(RankError::InvalidNetworkSimplexTree)
        );
        assert_eq!(
            network_simplex(&mut graph),
            Err(crate::LayoutError::InvalidNetworkSimplexTree)
        );
        assert_eq!(
            crate::rank::rank(&mut graph),
            Err(crate::LayoutError::InvalidNetworkSimplexTree)
        );
        assert_eq!(graph.node_ids(), source_nodes);
        assert_eq!(graph.edge_keys(), source_edges);
        assert!(
            graph
                .nodes()
                .all(|id| graph.node(id).is_some_and(|node| node.rank.is_none()))
        );
    }

    #[test]
    fn network_simplex_initial_longest_path_owns_cold_csr_once() {
        let source = ranking_graph();
        let simplify_units = simplify_work_units(&source).unwrap();
        let mut preparation_control = NoopWorkControl;
        let mut simplified = build_simplified_graph(&source, &mut preparation_control)
            .expect("the fixture simplify step succeeds");
        assert!(!simplified.is_adjacency_cache_current());
        let mut longest_control = RecordingWorkControl::default();
        util::longest_path_controlled(&mut simplified, &mut longest_control)
            .expect("the fixture longest-path step succeeds");
        assert_eq!(longest_control.charges.len(), 1);
        let longest_units = longest_control.charges[0];
        assert!(simplified.is_adjacency_cache_current());

        let exact_prefix = checked_add(simplify_units, longest_units).unwrap();
        let mut rejected_graph = ranking_graph();
        let source_nodes = rejected_graph.node_ids();
        let source_edges = rejected_graph.edge_keys();
        let mut rejected = RecordingWorkControl::with_limit(exact_prefix - 1);
        assert_eq!(
            network_simplex_controlled(&mut rejected_graph, &mut rejected),
            Err(RankError::Work(WorkError::Interrupted))
        );
        assert_eq!(rejected.charges, [simplify_units, longest_units]);
        assert_eq!(rejected.remaining, Some(longest_units - 1));
        assert_eq!(rejected_graph.node_ids(), source_nodes);
        assert_eq!(rejected_graph.edge_keys(), source_edges);
        assert!(rejected_graph.nodes().all(|id| {
            rejected_graph
                .node(id)
                .is_some_and(|node| node.rank.is_none())
        }));

        let mut admitted_graph = ranking_graph();
        let mut admitted = RecordingWorkControl::default();
        network_simplex_controlled(&mut admitted_graph, &mut admitted)
            .expect("the unbounded control admits network simplex");
        assert_eq!(admitted.charges[0], simplify_units);
        assert_eq!(admitted.charges[1], longest_units);
    }

    #[test]
    fn dense_tree_matches_the_pinned_six_pivot_trace_without_slot_growth() {
        let source = multi_pivot_graph();
        let mut work_control = NoopWorkControl;
        let mut graph = build_simplified_graph(&source, &mut work_control)
            .expect("the fixture simplify step succeeds");
        util::longest_path_controlled(&mut graph, &mut work_control)
            .expect("the fixture ranks fit i32");
        let spanning_tree = feasible_tree::feasible_tree_controlled(&mut graph, &mut work_control)
            .expect("the fixture feasible tree succeeds");
        let mut state =
            TreeState::new(&spanning_tree, &graph).expect("the tree is a graph-edge subset");
        let edge_capacity = state.tree_edges_in_order.capacity();
        assert_eq!(state.edge_count(), 8);
        assert_eq!(
            ordered_tree_edges(&state, &graph),
            [
                ("n0", "n1"),
                ("n1", "n3"),
                ("n2", "n3"),
                ("n2", "n4"),
                ("n4", "n5"),
                ("n5", "n9"),
                ("n7", "n9"),
                ("n8", "n9"),
            ]
            .map(|(v, w)| (v.to_string(), w.to_string()))
        );
        drop(spanning_tree);
        state
            .rebuild(&graph, None)
            .expect("the initial pinned tree rebuild succeeds");

        let mut rank_by_ix = vec![0_i128; graph.node_slot_count()];
        graph.for_each_node_ix(|g_ix, _id, label| {
            rank_by_ix[g_ix] = i128::from(label.rank.unwrap_or(0));
        });

        let mut pivots = Vec::new();
        while let Some(leaving) = state.find_leave_edge_in_insertion_order() {
            assert!(pivots.len() < 16, "the pinned fixture must converge");
            let entering = enter_edge_fast(&mut state, &graph, &rank_by_ix, leaving)
                .expect("the pinned pivot has an entering edge");
            pivots.push((
                (
                    tree_node_id(&state, &graph, leaving.v_ix).to_string(),
                    tree_node_id(&state, &graph, leaving.w_ix).to_string(),
                ),
                (entering.v.clone(), entering.w.clone()),
            ));
            state
                .exchange_edge(leaving, &entering, &graph)
                .expect("the pinned pivot exchanges one live tree edge");
            assert_eq!(state.edge_count(), 8);
            assert_eq!(state.tree_edges_in_order.capacity(), edge_capacity);
            state
                .rebuild(&graph, None)
                .expect("the pivoted pinned tree rebuild succeeds");
            update_ranks_fast(&mut state, &mut graph, &mut rank_by_ix)
                .expect("the pinned fixture ranks fit i32");
        }

        assert_eq!(
            pivots,
            [
                (("n2", "n3"), ("n0", "n4")),
                (("n0", "n1"), ("n1", "n4")),
                (("n7", "n9"), ("n4", "n7")),
                (("n8", "n9"), ("n3", "n8")),
                (("n1", "n4"), ("n0", "n1")),
                (("n0", "n4"), ("n2", "n3")),
            ]
            .map(|((leave_v, leave_w), (enter_v, enter_w))| {
                (
                    (leave_v.to_string(), leave_w.to_string()),
                    (enter_v.to_string(), enter_w.to_string()),
                )
            })
        );
        assert_eq!(
            ordered_tree_edges(&state, &graph),
            [
                ("n1", "n3"),
                ("n2", "n4"),
                ("n4", "n5"),
                ("n5", "n9"),
                ("n4", "n7"),
                ("n3", "n8"),
                ("n0", "n1"),
                ("n2", "n3"),
            ]
            .map(|(v, w)| (v.to_string(), w.to_string()))
        );
        for (id, expected_rank) in [
            ("n0", -9),
            ("n1", -7),
            ("n2", -7),
            ("n3", -4),
            ("n4", -6),
            ("n5", -3),
            ("n7", -4),
            ("n8", -2),
            ("n9", 0),
        ] {
            assert_eq!(
                graph.node(id).and_then(|node| node.rank),
                Some(expected_rank)
            );
        }
    }

    #[test]
    fn dense_pivot_precharges_exact_work_before_mutation() {
        let (mut expected_state, mut expected_graph, mut expected_ranks, expected_leaving) =
            first_multi_pivot_state();
        let exact = simplex_iteration_work_units(&expected_graph, &expected_state)
            .expect("the fixture work bound fits usize");
        assert_eq!(exact, 237);

        let mut measured = RecordingWorkControl::default();
        pivot_controlled(
            &mut expected_state,
            &mut expected_graph,
            &mut expected_ranks,
            expected_leaving,
            &mut measured,
        )
        .expect("the unbounded control admits the first pivot");
        assert_eq!(measured.charges, [exact]);
        let expected_rank_snapshot = rank_snapshot(&expected_graph);

        let (mut rejected_state, mut rejected_graph, mut rejected_ranks, rejected_leaving) =
            first_multi_pivot_state();
        let source_state = rejected_state.clone();
        let source_nodes = rejected_graph.node_ids();
        let source_edges = rejected_graph.edge_keys();
        let source_rank_snapshot = rank_snapshot(&rejected_graph);
        let source_ranks = rejected_ranks.clone();
        let mut rejected = RecordingWorkControl::with_limit(exact - 1);
        assert_eq!(
            pivot_controlled(
                &mut rejected_state,
                &mut rejected_graph,
                &mut rejected_ranks,
                rejected_leaving,
                &mut rejected,
            ),
            Err(RankError::Work(WorkError::Interrupted))
        );
        assert_eq!(rejected.charges, [exact]);
        assert_eq!(rejected.remaining, Some(exact - 1));
        assert_eq!(rejected_state, source_state);
        assert_eq!(rejected_graph.node_ids(), source_nodes);
        assert_eq!(rejected_graph.edge_keys(), source_edges);
        assert_eq!(rank_snapshot(&rejected_graph), source_rank_snapshot);
        assert_eq!(rejected_ranks, source_ranks);

        for limit in [exact, exact + 1] {
            let (mut state, mut graph, mut ranks, leaving) = first_multi_pivot_state();
            let mut work_control = RecordingWorkControl::with_limit(limit);
            pivot_controlled(
                &mut state,
                &mut graph,
                &mut ranks,
                leaving,
                &mut work_control,
            )
            .expect("equal and above dense-pivot budgets succeed");
            assert_eq!(work_control.charges, [exact]);
            assert_eq!(work_control.remaining, Some(limit - exact));
            assert_eq!(state, expected_state);
            assert_eq!(rank_snapshot(&graph), expected_rank_snapshot);
            assert_eq!(ranks, expected_ranks);
        }
    }

    #[test]
    fn enter_edge_fast_breaks_equal_slack_ties_by_global_edge_order() {
        let mut graph = Graph::new(GraphOptions::default());
        graph.set_graph(GraphLabel::default());
        graph.set_default_node_label(NodeLabel::default);
        graph.set_default_edge_label(|| EdgeLabel {
            minlen: 1,
            weight: 1.0,
            ..EdgeLabel::default()
        });
        for node in ["r", "a", "b", "c", "x", "y"] {
            graph.set_node(node, NodeLabel::default());
        }
        for (tail, head) in [
            ("a", "r"),
            ("a", "b"),
            ("a", "c"),
            ("r", "x"),
            ("r", "y"),
            ("y", "c"),
            ("x", "b"),
        ] {
            graph.set_edge(tail, head);
        }

        let mut candidate_edge_ixs = Vec::new();
        graph.for_each_edge_entry_ix(|edge_ix, _tail_ix, _head_ix, key, _label| {
            if (key.v == "y" && key.w == "c") || (key.v == "x" && key.w == "b") {
                candidate_edge_ixs.push((key.clone(), edge_ix));
            }
        });
        assert_eq!(
            candidate_edge_ixs,
            [
                (
                    EdgeKey {
                        v: "y".to_string(),
                        w: "c".to_string(),
                        name: None,
                    },
                    5,
                ),
                (
                    EdgeKey {
                        v: "x".to_string(),
                        w: "b".to_string(),
                        name: None,
                    },
                    6,
                ),
            ]
        );

        let mut spanning_tree = tree(&[("r", "a"), ("a", "b"), ("a", "c"), ("r", "x"), ("r", "y")]);
        init_low_lim_values(&mut spanning_tree, Some("r"));

        let mut rank_by_ix = vec![0_i128; graph.node_slot_count()];
        rank_by_ix[graph.node_ix("b").expect("b is present")] = 1;
        rank_by_ix[graph.node_ix("c").expect("c is present")] = 1;
        let rank_by_ix_i32 = rank_by_ix
            .iter()
            .map(|rank| i32::try_from(*rank).expect("test ranks fit i32"))
            .collect::<Vec<_>>();

        let leaving = EdgeKey {
            v: "a".to_string(),
            w: "r".to_string(),
            name: None,
        };
        let expected = EdgeKey {
            v: "y".to_string(),
            w: "c".to_string(),
            name: None,
        };
        let slow = enter_edge(&spanning_tree, &graph, &rank_by_ix_i32, &leaving);
        assert_eq!(slow, expected);

        let mut tree_state =
            TreeState::new(&spanning_tree, &graph).expect("the tree is a graph-edge subset");
        tree_state
            .rebuild(&graph, Some("r"))
            .expect("the equal-slack tree rebuild succeeds");
        let a_ix = spanning_tree.node_ix("a").expect("a is present");
        let r_ix = spanning_tree.node_ix("r").expect("r is present");
        let leaving = OrderedTreeEdge {
            position: tree_state
                .tree_edges_in_order
                .iter()
                .position(|edge| {
                    (edge.v_t_ix, edge.w_t_ix) == (a_ix, r_ix)
                        || (edge.v_t_ix, edge.w_t_ix) == (r_ix, a_ix)
                })
                .expect("the leaving edge is present"),
            v_ix: a_ix,
            w_ix: r_ix,
        };
        let fast = enter_edge_fast(&mut tree_state, &graph, &rank_by_ix, leaving)
            .expect("the fixture has an entering edge");

        assert_eq!(fast, slow);
    }
}

// NOTE: Dagre treats the feasible tree as an undirected structure. We consider an edge to be a
// tree edge if it exists in `t` (queried via `t.edge(u, v, None)` in the hot loops).
