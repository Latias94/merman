//! Adjacency caches used by [`Graph`](super::Graph).
//!
//! These caches exist purely as an optimization: many Dagre algorithms query successors /
//! predecessors repeatedly, and scanning all edges each time is O(E) per query.

use rustc_hash::FxBuildHasher;

type NeighborIndex = hashbrown::HashMap<usize, usize, FxBuildHasher>;

#[derive(Debug, Clone)]
pub(in crate::graph) struct DirectedEdgeAdjCache {
    pub(in crate::graph) generation: u64,
    pub(in crate::graph) out_offsets: Vec<usize>,
    pub(in crate::graph) out_edges: Vec<usize>,
    pub(in crate::graph) in_offsets: Vec<usize>,
    pub(in crate::graph) in_edges: Vec<usize>,
}

impl DirectedEdgeAdjCache {
    pub(in crate::graph) fn out_edges(&self, v_ix: usize) -> &[usize] {
        let start = self.out_offsets[v_ix];
        let end = self.out_offsets[v_ix + 1];
        &self.out_edges[start..end]
    }

    pub(in crate::graph) fn in_edges(&self, v_ix: usize) -> &[usize] {
        let start = self.in_offsets[v_ix];
        let end = self.in_offsets[v_ix + 1];
        &self.in_edges[start..end]
    }
}

#[derive(Debug, Clone, Copy)]
struct NeighborCount {
    node_ix: usize,
    count: usize,
}

#[derive(Debug, Clone, Default)]
struct OrderedNeighborCounts {
    entries: Vec<Option<NeighborCount>>,
    index: NeighborIndex,
}

impl OrderedNeighborCounts {
    fn insert_count(&mut self, node_ix: usize, count: usize) {
        debug_assert!(count > 0);
        if let Some(&entry_ix) = self.index.get(&node_ix) {
            let entry = self.entries[entry_ix]
                .as_mut()
                .expect("neighbor index referenced a removed entry");
            entry.count = entry.count.saturating_add(count);
            return;
        }

        let entry_ix = self.entries.len();
        self.entries.push(Some(NeighborCount { node_ix, count }));
        self.index.insert(node_ix, entry_ix);
    }

    fn remove_one(&mut self, node_ix: usize) -> bool {
        let Some(&entry_ix) = self.index.get(&node_ix) else {
            return false;
        };
        let entry = self.entries[entry_ix]
            .as_mut()
            .expect("neighbor index referenced a removed entry");
        if entry.count > 1 {
            entry.count -= 1;
            return true;
        }

        self.index.remove(&node_ix);
        self.entries[entry_ix] = None;
        while matches!(self.entries.last(), Some(None)) {
            self.entries.pop();
        }
        true
    }

    fn len(&self) -> usize {
        self.index.len()
    }

    fn first(&self) -> Option<usize> {
        self.iter().next()
    }

    fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.entries
            .iter()
            .filter_map(|entry| entry.as_ref().map(|entry| entry.node_ix))
    }

    fn into_entries(self) -> impl Iterator<Item = (usize, usize)> {
        self.entries
            .into_iter()
            .flatten()
            .map(|entry| (entry.node_ix, entry.count))
    }
}

/// Graphlib keeps `_sucs` and `_preds` as insertion-ordered endpoint keys with parallel-edge
/// counts. This state mirrors that lifecycle so removing one named edge cannot reorder a neighbor.
#[derive(Debug, Clone, Default)]
pub(in crate::graph) struct DirectedNodeAdjacency {
    successors: Vec<OrderedNeighborCounts>,
    predecessors: Vec<OrderedNeighborCounts>,
}

impl DirectedNodeAdjacency {
    pub(in crate::graph) fn reserve_nodes(&mut self, additional: usize) {
        self.successors.reserve(additional);
        self.predecessors.reserve(additional);
    }

    pub(in crate::graph) fn add_node(&mut self) {
        self.successors.push(OrderedNeighborCounts::default());
        self.predecessors.push(OrderedNeighborCounts::default());
    }

    pub(in crate::graph) fn truncate_nodes(&mut self, len: usize) {
        self.successors.truncate(len);
        self.predecessors.truncate(len);
    }

    pub(in crate::graph) fn clear(&mut self) {
        self.successors.clear();
        self.predecessors.clear();
    }

    pub(in crate::graph) fn add_edge(&mut self, v_ix: usize, w_ix: usize) {
        self.successors[v_ix].insert_count(w_ix, 1);
        self.predecessors[w_ix].insert_count(v_ix, 1);
    }

    pub(in crate::graph) fn remove_edge(&mut self, v_ix: usize, w_ix: usize) {
        let successor_removed = self.successors[v_ix].remove_one(w_ix);
        let predecessor_removed = self.predecessors[w_ix].remove_one(v_ix);
        debug_assert!(successor_removed);
        debug_assert_eq!(successor_removed, predecessor_removed);
    }

    pub(in crate::graph) fn remap(&mut self, node_remap: &[Option<usize>], new_node_slots: usize) {
        let old = std::mem::take(self);
        self.successors = std::iter::repeat_with(OrderedNeighborCounts::default)
            .take(new_node_slots)
            .collect();
        self.predecessors = std::iter::repeat_with(OrderedNeighborCounts::default)
            .take(new_node_slots)
            .collect();

        for (old_owner, neighbors) in old.successors.into_iter().enumerate() {
            let Some(new_owner) = node_remap.get(old_owner).copied().flatten() else {
                continue;
            };
            for (old_neighbor, count) in neighbors.into_entries() {
                let Some(new_neighbor) = node_remap.get(old_neighbor).copied().flatten() else {
                    continue;
                };
                self.successors[new_owner].insert_count(new_neighbor, count);
            }
        }
        for (old_owner, neighbors) in old.predecessors.into_iter().enumerate() {
            let Some(new_owner) = node_remap.get(old_owner).copied().flatten() else {
                continue;
            };
            for (old_neighbor, count) in neighbors.into_entries() {
                let Some(new_neighbor) = node_remap.get(old_neighbor).copied().flatten() else {
                    continue;
                };
                self.predecessors[new_owner].insert_count(new_neighbor, count);
            }
        }
    }

    pub(in crate::graph) fn successor_count(&self, v_ix: usize) -> usize {
        self.successors[v_ix].len()
    }

    pub(in crate::graph) fn predecessor_count(&self, v_ix: usize) -> usize {
        self.predecessors[v_ix].len()
    }

    pub(in crate::graph) fn first_successor(&self, v_ix: usize) -> Option<usize> {
        self.successors[v_ix].first()
    }

    pub(in crate::graph) fn first_predecessor(&self, v_ix: usize) -> Option<usize> {
        self.predecessors[v_ix].first()
    }

    pub(in crate::graph) fn successors(&self, v_ix: usize) -> impl Iterator<Item = usize> + '_ {
        self.successors[v_ix].iter()
    }

    pub(in crate::graph) fn predecessors(&self, v_ix: usize) -> impl Iterator<Item = usize> + '_ {
        self.predecessors[v_ix].iter()
    }
}

#[derive(Debug, Clone)]
pub(in crate::graph) struct UndirectedAdjCache {
    pub(in crate::graph) generation: u64,
    pub(in crate::graph) offsets: Vec<usize>,
    pub(in crate::graph) edges: Vec<usize>,
}

impl UndirectedAdjCache {
    pub(in crate::graph) fn edges(&self, v_ix: usize) -> &[usize] {
        let start = self.offsets[v_ix];
        let end = self.offsets[v_ix + 1];
        &self.edges[start..end]
    }
}
