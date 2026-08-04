//! Adjacency indexes and caches used by [`Graph`](super::Graph).
//!
//! Dagre algorithms query edges and adjacent nodes repeatedly. Edge CSR is rebuilt lazily after
//! mutation, while directed node adjacency is maintained incrementally to preserve Graphlib's
//! counted JavaScript object-key semantics.

use std::collections::{BTreeMap, btree_map};

use rustc_hash::FxBuildHasher;

type DirectedLinkIndex = hashbrown::HashMap<(usize, usize), usize, FxBuildHasher>;

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
struct DirectedLink {
    v_ix: usize,
    w_ix: usize,
    count: usize,
    v_array_index: Option<u32>,
    w_array_index: Option<u32>,
    successor_prev: Option<usize>,
    successor_next: Option<usize>,
    predecessor_prev: Option<usize>,
    predecessor_next: Option<usize>,
}

#[derive(Debug, Clone, Default)]
struct DirectedNeighborBucket {
    #[allow(clippy::box_collection)]
    numeric: Option<Box<BTreeMap<u32, usize>>>,
    ordinary_head: Option<usize>,
    ordinary_tail: Option<usize>,
    len: usize,
}

#[derive(Debug, Clone, Copy)]
enum LinkDirection {
    Successor,
    Predecessor,
}

struct OrderedLinkIter<'a> {
    links: &'a [Option<DirectedLink>],
    next: Option<usize>,
    direction: LinkDirection,
}

struct NumericLinkIter<'a> {
    links: &'a [Option<DirectedLink>],
    entries: Option<btree_map::Values<'a, u32, usize>>,
    direction: LinkDirection,
}

impl Iterator for NumericLinkIter<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        let link_ix = *self.entries.as_mut()?.next()?;
        let link = self.links[link_ix]
            .as_ref()
            .expect("numeric adjacency referenced a removed link");
        Some(match self.direction {
            LinkDirection::Successor => link.w_ix,
            LinkDirection::Predecessor => link.v_ix,
        })
    }
}

impl Iterator for OrderedLinkIter<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        let link_ix = self.next?;
        let link = self.links[link_ix]
            .as_ref()
            .expect("ordered adjacency referenced a removed link");
        match self.direction {
            LinkDirection::Successor => {
                self.next = link.successor_next;
                Some(link.w_ix)
            }
            LinkDirection::Predecessor => {
                self.next = link.predecessor_next;
                Some(link.v_ix)
            }
        }
    }
}

/// Graphlib keeps `_sucs` and `_preds` as ordinary JavaScript objects with parallel-edge counts.
/// Array-index endpoint keys enumerate numerically before ordinary keys, while ordinary keys retain
/// property-creation order until the last parallel edge removes and recreates the property.
#[derive(Debug, Clone, Default)]
pub(in crate::graph) struct DirectedNodeAdjacency {
    link_index: DirectedLinkIndex,
    links: Vec<Option<DirectedLink>>,
    free_links: Vec<usize>,
    numeric_ordered_entry_count: usize,
    successors: Vec<DirectedNeighborBucket>,
    predecessors: Vec<DirectedNeighborBucket>,
}

impl DirectedNodeAdjacency {
    pub(in crate::graph) fn reserve_nodes(&mut self, additional: usize) {
        self.successors.reserve(additional);
        self.predecessors.reserve(additional);
    }

    pub(in crate::graph) fn reserve_edges(&mut self, additional: usize) {
        self.link_index.reserve(additional);
        self.links.reserve(additional);
    }

    pub(in crate::graph) fn add_node(&mut self) {
        self.successors.push(DirectedNeighborBucket::default());
        self.predecessors.push(DirectedNeighborBucket::default());
    }

    pub(in crate::graph) fn truncate_nodes(&mut self, len: usize) {
        self.successors.truncate(len);
        self.predecessors.truncate(len);
    }

    pub(in crate::graph) fn clear(&mut self) {
        self.link_index.clear();
        self.links.clear();
        self.free_links.clear();
        self.numeric_ordered_entry_count = 0;
        self.successors.clear();
        self.predecessors.clear();
    }

    pub(in crate::graph) fn add_edge(
        &mut self,
        v_ix: usize,
        w_ix: usize,
        v_array_index: Option<u32>,
        w_array_index: Option<u32>,
    ) {
        if let Some(&link_ix) = self.link_index.get(&(v_ix, w_ix)) {
            let link = self.links[link_ix]
                .as_mut()
                .expect("directed link index referenced a removed link");
            debug_assert_eq!(link.v_array_index, v_array_index);
            debug_assert_eq!(link.w_array_index, w_array_index);
            link.count = link.count.saturating_add(1);
            return;
        }

        let link_ix = self.free_links.pop().unwrap_or(self.links.len());
        let link = DirectedLink {
            v_ix,
            w_ix,
            count: 1,
            v_array_index,
            w_array_index,
            successor_prev: None,
            successor_next: None,
            predecessor_prev: None,
            predecessor_next: None,
        };
        if link_ix == self.links.len() {
            self.links.push(Some(link));
        } else {
            self.links[link_ix] = Some(link);
        }
        self.link_index.insert((v_ix, w_ix), link_ix);
        self.insert_successor(v_ix, link_ix, w_array_index);
        self.insert_predecessor(w_ix, link_ix, v_array_index);
    }

    pub(in crate::graph) fn remove_edge(&mut self, v_ix: usize, w_ix: usize) {
        let Some(&link_ix) = self.link_index.get(&(v_ix, w_ix)) else {
            debug_assert!(false, "directed adjacency did not contain the removed edge");
            return;
        };
        let (v_array_index, w_array_index) = {
            let link = self.links[link_ix]
                .as_mut()
                .expect("directed link index referenced a removed link");
            if link.count > 1 {
                link.count -= 1;
                return;
            }
            (link.v_array_index, link.w_array_index)
        };

        self.unlink_successor(v_ix, link_ix, w_array_index);
        self.unlink_predecessor(w_ix, link_ix, v_array_index);
        self.link_index.remove(&(v_ix, w_ix));
        self.links[link_ix] = None;
        self.free_links.push(link_ix);
    }

    /// Removes every endpoint pair incident to a marked node, regardless of parallel-edge count.
    /// Numeric maps and ordinary linked lists are each retained once per neighbor bucket.
    pub(in crate::graph) fn remove_incident_edges(&mut self, removed_nodes: &[bool]) {
        debug_assert_eq!(removed_nodes.len(), self.successors.len());
        debug_assert_eq!(removed_nodes.len(), self.predecessors.len());

        let mut numeric_ordered_entry_count = 0usize;
        for bucket in &mut self.successors {
            let numeric_len = Self::retain_bucket(
                bucket,
                &mut self.links,
                removed_nodes,
                LinkDirection::Successor,
            );
            numeric_ordered_entry_count = numeric_ordered_entry_count
                .checked_add(numeric_len)
                .expect("directed numeric adjacency entry count overflowed");
        }
        for bucket in &mut self.predecessors {
            let numeric_len = Self::retain_bucket(
                bucket,
                &mut self.links,
                removed_nodes,
                LinkDirection::Predecessor,
            );
            numeric_ordered_entry_count = numeric_ordered_entry_count
                .checked_add(numeric_len)
                .expect("directed numeric adjacency entry count overflowed");
        }
        self.numeric_ordered_entry_count = numeric_ordered_entry_count;

        let links = &mut self.links;
        let free_links = &mut self.free_links;
        self.link_index.retain(|&(v_ix, w_ix), link_ix| {
            if !removed_nodes[v_ix] && !removed_nodes[w_ix] {
                return true;
            }

            let link_ix = *link_ix;
            let link = links[link_ix]
                .take()
                .expect("directed link index referenced a removed link");
            debug_assert_eq!((link.v_ix, link.w_ix), (v_ix, w_ix));
            free_links.push(link_ix);
            false
        });
    }

    pub(in crate::graph) fn remap(&mut self, node_remap: &[Option<usize>], new_node_slots: usize) {
        let old = std::mem::take(self);
        self.reserve_edges(old.link_index.len());
        self.successors = vec![DirectedNeighborBucket::default(); new_node_slots];
        self.predecessors = vec![DirectedNeighborBucket::default(); new_node_slots];

        let mut link_remap = vec![None; old.links.len()];
        for (old_link_ix, old_link) in old.links.iter().enumerate() {
            let Some(old_link) = old_link else {
                continue;
            };
            let Some(v_ix) = node_remap.get(old_link.v_ix).copied().flatten() else {
                continue;
            };
            let Some(w_ix) = node_remap.get(old_link.w_ix).copied().flatten() else {
                continue;
            };
            let new_link_ix = self.links.len();
            self.links.push(Some(DirectedLink {
                v_ix,
                w_ix,
                count: old_link.count,
                v_array_index: old_link.v_array_index,
                w_array_index: old_link.w_array_index,
                successor_prev: None,
                successor_next: None,
                predecessor_prev: None,
                predecessor_next: None,
            }));
            self.link_index.insert((v_ix, w_ix), new_link_ix);
            if let Some(array_index) = old_link.w_array_index {
                self.insert_numeric_successor(v_ix, array_index, new_link_ix);
            }
            if let Some(array_index) = old_link.v_array_index {
                self.insert_numeric_predecessor(w_ix, array_index, new_link_ix);
            }
            link_remap[old_link_ix] = Some(new_link_ix);
        }

        for old_bucket in &old.successors {
            let mut next = old_bucket.ordinary_head;
            while let Some(old_link_ix) = next {
                let old_link = old.links[old_link_ix]
                    .as_ref()
                    .expect("successor list referenced a removed link");
                next = old_link.successor_next;
                debug_assert!(old_link.w_array_index.is_none());
                if let Some(new_link_ix) = link_remap[old_link_ix] {
                    let v_ix = self.links[new_link_ix]
                        .as_ref()
                        .expect("remapped successor link was missing")
                        .v_ix;
                    self.append_successor(v_ix, new_link_ix);
                }
            }
        }
        for old_bucket in &old.predecessors {
            let mut next = old_bucket.ordinary_head;
            while let Some(old_link_ix) = next {
                let old_link = old.links[old_link_ix]
                    .as_ref()
                    .expect("predecessor list referenced a removed link");
                next = old_link.predecessor_next;
                debug_assert!(old_link.v_array_index.is_none());
                if let Some(new_link_ix) = link_remap[old_link_ix] {
                    let w_ix = self.links[new_link_ix]
                        .as_ref()
                        .expect("remapped predecessor link was missing")
                        .w_ix;
                    self.append_predecessor(w_ix, new_link_ix);
                }
            }
        }
    }

    pub(in crate::graph) fn successor_count(&self, v_ix: usize) -> usize {
        self.successors[v_ix].len
    }

    pub(in crate::graph) fn predecessor_count(&self, v_ix: usize) -> usize {
        self.predecessors[v_ix].len
    }

    pub(in crate::graph) fn numeric_ordered_entry_count(&self) -> usize {
        self.numeric_ordered_entry_count
    }

    pub(in crate::graph) fn first_successor(&self, v_ix: usize) -> Option<usize> {
        if let Some(link_ix) = self.successors[v_ix]
            .numeric
            .as_deref()
            .and_then(|numeric| numeric.values().next().copied())
        {
            return self.links[link_ix].as_ref().map(|link| link.w_ix);
        }
        let link_ix = self.successors[v_ix].ordinary_head?;
        self.links[link_ix].as_ref().map(|link| link.w_ix)
    }

    pub(in crate::graph) fn first_predecessor(&self, v_ix: usize) -> Option<usize> {
        if let Some(link_ix) = self.predecessors[v_ix]
            .numeric
            .as_deref()
            .and_then(|numeric| numeric.values().next().copied())
        {
            return self.links[link_ix].as_ref().map(|link| link.v_ix);
        }
        let link_ix = self.predecessors[v_ix].ordinary_head?;
        self.links[link_ix].as_ref().map(|link| link.v_ix)
    }

    pub(in crate::graph) fn successors(&self, v_ix: usize) -> impl Iterator<Item = usize> + '_ {
        NumericLinkIter {
            links: &self.links,
            entries: self.successors[v_ix]
                .numeric
                .as_deref()
                .map(BTreeMap::values),
            direction: LinkDirection::Successor,
        }
        .chain(OrderedLinkIter {
            links: &self.links,
            next: self.successors[v_ix].ordinary_head,
            direction: LinkDirection::Successor,
        })
    }

    pub(in crate::graph) fn predecessors(&self, v_ix: usize) -> impl Iterator<Item = usize> + '_ {
        NumericLinkIter {
            links: &self.links,
            entries: self.predecessors[v_ix]
                .numeric
                .as_deref()
                .map(BTreeMap::values),
            direction: LinkDirection::Predecessor,
        }
        .chain(OrderedLinkIter {
            links: &self.links,
            next: self.predecessors[v_ix].ordinary_head,
            direction: LinkDirection::Predecessor,
        })
    }

    fn insert_successor(&mut self, v_ix: usize, link_ix: usize, array_index: Option<u32>) {
        if let Some(array_index) = array_index {
            self.insert_numeric_successor(v_ix, array_index, link_ix);
        } else {
            self.append_successor(v_ix, link_ix);
        }
    }

    fn insert_predecessor(&mut self, w_ix: usize, link_ix: usize, array_index: Option<u32>) {
        if let Some(array_index) = array_index {
            self.insert_numeric_predecessor(w_ix, array_index, link_ix);
        } else {
            self.append_predecessor(w_ix, link_ix);
        }
    }

    fn insert_numeric_successor(&mut self, v_ix: usize, array_index: u32, link_ix: usize) {
        let previous = self.successors[v_ix]
            .numeric
            .get_or_insert_with(|| Box::new(BTreeMap::new()))
            .insert(array_index, link_ix);
        debug_assert!(previous.is_none(), "node IDs are unique graph keys");
        self.successors[v_ix].len += 1;
        self.numeric_ordered_entry_count = self
            .numeric_ordered_entry_count
            .checked_add(1)
            .expect("directed numeric adjacency entry count overflowed");
    }

    fn insert_numeric_predecessor(&mut self, w_ix: usize, array_index: u32, link_ix: usize) {
        let previous = self.predecessors[w_ix]
            .numeric
            .get_or_insert_with(|| Box::new(BTreeMap::new()))
            .insert(array_index, link_ix);
        debug_assert!(previous.is_none(), "node IDs are unique graph keys");
        self.predecessors[w_ix].len += 1;
        self.numeric_ordered_entry_count = self
            .numeric_ordered_entry_count
            .checked_add(1)
            .expect("directed numeric adjacency entry count overflowed");
    }

    fn append_successor(&mut self, v_ix: usize, link_ix: usize) {
        let tail = self.successors[v_ix].ordinary_tail;
        if let Some(tail_ix) = tail {
            self.links[tail_ix]
                .as_mut()
                .expect("successor tail referenced a removed link")
                .successor_next = Some(link_ix);
        } else {
            self.successors[v_ix].ordinary_head = Some(link_ix);
        }
        let link = self.links[link_ix]
            .as_mut()
            .expect("appended successor link was missing");
        link.successor_prev = tail;
        link.successor_next = None;
        self.successors[v_ix].ordinary_tail = Some(link_ix);
        self.successors[v_ix].len += 1;
    }

    fn append_predecessor(&mut self, w_ix: usize, link_ix: usize) {
        let tail = self.predecessors[w_ix].ordinary_tail;
        if let Some(tail_ix) = tail {
            self.links[tail_ix]
                .as_mut()
                .expect("predecessor tail referenced a removed link")
                .predecessor_next = Some(link_ix);
        } else {
            self.predecessors[w_ix].ordinary_head = Some(link_ix);
        }
        let link = self.links[link_ix]
            .as_mut()
            .expect("appended predecessor link was missing");
        link.predecessor_prev = tail;
        link.predecessor_next = None;
        self.predecessors[w_ix].ordinary_tail = Some(link_ix);
        self.predecessors[w_ix].len += 1;
    }

    fn retain_bucket(
        bucket: &mut DirectedNeighborBucket,
        links: &mut [Option<DirectedLink>],
        removed_nodes: &[bool],
        direction: LinkDirection,
    ) -> usize {
        let numeric_len = if let Some(numeric) = bucket.numeric.as_deref_mut() {
            numeric.retain(|_, link_ix| {
                let link = links[*link_ix]
                    .as_ref()
                    .expect("numeric adjacency referenced a removed link");
                !removed_nodes[link.v_ix] && !removed_nodes[link.w_ix]
            });
            numeric.len()
        } else {
            0
        };
        if bucket
            .numeric
            .as_deref()
            .is_some_and(|numeric| numeric.is_empty())
        {
            bucket.numeric = None;
        }

        let mut next = bucket.ordinary_head;
        let mut retained_head = None;
        let mut retained_tail: Option<usize> = None;
        let mut ordinary_len = 0usize;
        while let Some(link_ix) = next {
            let link = links[link_ix]
                .as_ref()
                .expect("ordered adjacency referenced a removed link");
            next = match direction {
                LinkDirection::Successor => link.successor_next,
                LinkDirection::Predecessor => link.predecessor_next,
            };
            if removed_nodes[link.v_ix] || removed_nodes[link.w_ix] {
                continue;
            }

            if let Some(previous_ix) = retained_tail {
                let previous = links[previous_ix]
                    .as_mut()
                    .expect("retained adjacency tail referenced a removed link");
                match direction {
                    LinkDirection::Successor => previous.successor_next = Some(link_ix),
                    LinkDirection::Predecessor => previous.predecessor_next = Some(link_ix),
                }
            } else {
                retained_head = Some(link_ix);
            }

            let link = links[link_ix]
                .as_mut()
                .expect("retained adjacency link was missing");
            match direction {
                LinkDirection::Successor => {
                    link.successor_prev = retained_tail;
                    link.successor_next = None;
                }
                LinkDirection::Predecessor => {
                    link.predecessor_prev = retained_tail;
                    link.predecessor_next = None;
                }
            }
            retained_tail = Some(link_ix);
            ordinary_len = ordinary_len
                .checked_add(1)
                .expect("directed ordinary adjacency count overflowed");
        }

        bucket.ordinary_head = retained_head;
        bucket.ordinary_tail = retained_tail;
        bucket.len = numeric_len
            .checked_add(ordinary_len)
            .expect("directed adjacency bucket count overflowed");
        numeric_len
    }

    fn unlink_successor(&mut self, v_ix: usize, link_ix: usize, array_index: Option<u32>) {
        if let Some(array_index) = array_index {
            let numeric = self.successors[v_ix]
                .numeric
                .as_deref_mut()
                .expect("numeric successor map was missing");
            let removed_link_ix = numeric
                .remove(&array_index)
                .expect("numeric successor entry was missing");
            let numeric_is_empty = numeric.is_empty();
            assert_eq!(
                removed_link_ix, link_ix,
                "numeric successor entry referenced the wrong link"
            );
            if numeric_is_empty {
                self.successors[v_ix].numeric = None;
            }
            self.successors[v_ix].len = self.successors[v_ix]
                .len
                .checked_sub(1)
                .expect("numeric successor count underflowed");
            self.numeric_ordered_entry_count = self
                .numeric_ordered_entry_count
                .checked_sub(1)
                .expect("directed numeric adjacency entry count underflowed");
            return;
        }

        let link = self.links[link_ix]
            .as_ref()
            .expect("removed successor link was missing");
        let (prev, next) = (link.successor_prev, link.successor_next);
        if let Some(prev_ix) = prev {
            self.links[prev_ix]
                .as_mut()
                .expect("successor predecessor referenced a removed link")
                .successor_next = next;
        } else {
            self.successors[v_ix].ordinary_head = next;
        }
        if let Some(next_ix) = next {
            self.links[next_ix]
                .as_mut()
                .expect("successor next referenced a removed link")
                .successor_prev = prev;
        } else {
            self.successors[v_ix].ordinary_tail = prev;
        }
        self.successors[v_ix].len = self.successors[v_ix]
            .len
            .checked_sub(1)
            .expect("ordinary successor count underflowed");
    }

    fn unlink_predecessor(&mut self, w_ix: usize, link_ix: usize, array_index: Option<u32>) {
        if let Some(array_index) = array_index {
            let numeric = self.predecessors[w_ix]
                .numeric
                .as_deref_mut()
                .expect("numeric predecessor map was missing");
            let removed_link_ix = numeric
                .remove(&array_index)
                .expect("numeric predecessor entry was missing");
            let numeric_is_empty = numeric.is_empty();
            assert_eq!(
                removed_link_ix, link_ix,
                "numeric predecessor entry referenced the wrong link"
            );
            if numeric_is_empty {
                self.predecessors[w_ix].numeric = None;
            }
            self.predecessors[w_ix].len = self.predecessors[w_ix]
                .len
                .checked_sub(1)
                .expect("numeric predecessor count underflowed");
            self.numeric_ordered_entry_count = self
                .numeric_ordered_entry_count
                .checked_sub(1)
                .expect("directed numeric adjacency entry count underflowed");
            return;
        }

        let link = self.links[link_ix]
            .as_ref()
            .expect("removed predecessor link was missing");
        let (prev, next) = (link.predecessor_prev, link.predecessor_next);
        if let Some(prev_ix) = prev {
            self.links[prev_ix]
                .as_mut()
                .expect("predecessor predecessor referenced a removed link")
                .predecessor_next = next;
        } else {
            self.predecessors[w_ix].ordinary_head = next;
        }
        if let Some(next_ix) = next {
            self.links[next_ix]
                .as_mut()
                .expect("predecessor next referenced a removed link")
                .predecessor_prev = prev;
        } else {
            self.predecessors[w_ix].ordinary_tail = prev;
        }
        self.predecessors[w_ix].len = self.predecessors[w_ix]
            .len
            .checked_sub(1)
            .expect("ordinary predecessor count underflowed");
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

#[cfg(test)]
mod tests {
    use super::DirectedNodeAdjacency;

    type OrderedCounts = Vec<Vec<(usize, usize)>>;

    fn add_count(rows: &mut OrderedCounts, owner: usize, neighbor: usize) {
        if let Some((_, count)) = rows[owner]
            .iter_mut()
            .find(|(node_ix, _)| *node_ix == neighbor)
        {
            *count += 1;
        } else {
            rows[owner].push((neighbor, 1));
        }
    }

    fn remove_count(rows: &mut OrderedCounts, owner: usize, neighbor: usize) -> bool {
        let Some(position) = rows[owner]
            .iter()
            .position(|(node_ix, _)| *node_ix == neighbor)
        else {
            return false;
        };
        if rows[owner][position].1 > 1 {
            rows[owner][position].1 -= 1;
        } else {
            rows[owner].remove(position);
        }
        true
    }

    fn assert_matches(
        adjacency: &DirectedNodeAdjacency,
        successors: &OrderedCounts,
        predecessors: &OrderedCounts,
    ) {
        assert_eq!(adjacency.numeric_ordered_entry_count(), 0);
        for node_ix in 0..successors.len() {
            let expected_successors = successors[node_ix]
                .iter()
                .map(|(neighbor, _)| *neighbor)
                .collect::<Vec<_>>();
            let expected_predecessors = predecessors[node_ix]
                .iter()
                .map(|(neighbor, _)| *neighbor)
                .collect::<Vec<_>>();
            assert_eq!(
                adjacency.successors(node_ix).collect::<Vec<_>>(),
                expected_successors
            );
            assert_eq!(
                adjacency.predecessors(node_ix).collect::<Vec<_>>(),
                expected_predecessors
            );
            assert_eq!(
                adjacency.successor_count(node_ix),
                expected_successors.len()
            );
            assert_eq!(
                adjacency.predecessor_count(node_ix),
                expected_predecessors.len()
            );
            assert_eq!(
                adjacency.first_successor(node_ix),
                expected_successors.first().copied()
            );
            assert_eq!(
                adjacency.first_predecessor(node_ix),
                expected_predecessors.first().copied()
            );
        }
    }

    #[test]
    fn directed_numeric_entries_preserve_order_count_remap_and_clear() {
        let mut adjacency = DirectedNodeAdjacency::default();
        for _ in 0..8 {
            adjacency.add_node();
        }

        adjacency.add_edge(0, 1, None, None);
        adjacency.add_edge(0, 2, None, Some(10));
        adjacency.add_edge(0, 3, None, None);
        adjacency.add_edge(0, 4, None, Some(2));
        adjacency.add_edge(0, 2, None, Some(10));

        adjacency.add_edge(1, 5, None, None);
        adjacency.add_edge(6, 5, Some(10), None);
        adjacency.add_edge(3, 5, None, None);
        adjacency.add_edge(7, 5, Some(2), None);
        adjacency.add_edge(6, 5, Some(10), None);

        assert_eq!(
            adjacency.successors(0).collect::<Vec<_>>(),
            vec![4, 2, 1, 3]
        );
        assert_eq!(
            adjacency.predecessors(5).collect::<Vec<_>>(),
            vec![7, 6, 1, 3]
        );
        assert_eq!(adjacency.numeric_ordered_entry_count(), 4);

        adjacency.remove_edge(0, 2);
        adjacency.remove_edge(6, 5);
        assert_eq!(
            adjacency.successors(0).collect::<Vec<_>>(),
            vec![4, 2, 1, 3]
        );
        assert_eq!(
            adjacency.predecessors(5).collect::<Vec<_>>(),
            vec![7, 6, 1, 3]
        );
        assert_eq!(adjacency.numeric_ordered_entry_count(), 4);

        adjacency.remove_edge(0, 2);
        adjacency.remove_edge(6, 5);
        assert_eq!(adjacency.successors(0).collect::<Vec<_>>(), vec![4, 1, 3]);
        assert_eq!(adjacency.predecessors(5).collect::<Vec<_>>(), vec![7, 1, 3]);
        assert_eq!(adjacency.numeric_ordered_entry_count(), 2);

        adjacency.add_edge(0, 2, None, Some(10));
        adjacency.add_edge(6, 5, Some(10), None);
        assert_eq!(
            adjacency.successors(0).collect::<Vec<_>>(),
            vec![4, 2, 1, 3]
        );
        assert_eq!(
            adjacency.predecessors(5).collect::<Vec<_>>(),
            vec![7, 6, 1, 3]
        );
        assert_eq!(adjacency.numeric_ordered_entry_count(), 4);

        let node_remap = [
            Some(3),
            Some(0),
            Some(6),
            Some(1),
            Some(5),
            Some(2),
            Some(4),
            Some(7),
        ];
        adjacency.remap(&node_remap, node_remap.len());
        assert_eq!(
            adjacency.successors(3).collect::<Vec<_>>(),
            vec![5, 6, 0, 1]
        );
        assert_eq!(
            adjacency.predecessors(2).collect::<Vec<_>>(),
            vec![7, 4, 0, 1]
        );
        assert_eq!(adjacency.numeric_ordered_entry_count(), 4);

        adjacency.clear();
        assert_eq!(adjacency.numeric_ordered_entry_count(), 0);
    }

    #[test]
    fn directed_bulk_removal_preserves_surviving_link_state() {
        let mut adjacency = DirectedNodeAdjacency::default();
        for _ in 0..7 {
            adjacency.add_node();
        }

        adjacency.add_edge(0, 1, None, Some(2));
        adjacency.add_edge(0, 2, None, Some(10));
        adjacency.add_edge(0, 3, None, None);
        adjacency.add_edge(0, 3, None, None);
        adjacency.add_edge(0, 4, None, None);
        adjacency.add_edge(0, 4, None, None);
        adjacency.add_edge(1, 5, Some(2), None);
        adjacency.add_edge(2, 5, Some(10), None);
        adjacency.add_edge(3, 5, None, None);
        adjacency.add_edge(4, 5, None, None);
        adjacency.add_edge(3, 1, None, Some(2));
        adjacency.add_edge(1, 3, Some(2), None);
        adjacency.add_edge(4, 3, None, None);
        adjacency.add_edge(3, 4, None, None);
        adjacency.add_edge(6, 6, None, None);

        let link_slots = adjacency.links.len();
        adjacency.remove_incident_edges(&[false, false, true, false, true, false, false]);

        assert_eq!(adjacency.successors(0).collect::<Vec<_>>(), vec![1, 3]);
        assert_eq!(adjacency.successors(1).collect::<Vec<_>>(), vec![5, 3]);
        assert_eq!(adjacency.successors(3).collect::<Vec<_>>(), vec![1, 5]);
        assert_eq!(adjacency.successors(6).collect::<Vec<_>>(), vec![6]);
        assert_eq!(adjacency.predecessors(1).collect::<Vec<_>>(), vec![0, 3]);
        assert_eq!(adjacency.predecessors(3).collect::<Vec<_>>(), vec![1, 0]);
        assert_eq!(adjacency.predecessors(5).collect::<Vec<_>>(), vec![1, 3]);
        assert_eq!(adjacency.numeric_ordered_entry_count(), 4);
        assert_eq!(adjacency.link_index.len(), 7);
        assert_eq!(adjacency.links.iter().flatten().count(), 7);
        assert_eq!(adjacency.free_links.len(), link_slots - 7);
        assert!(
            adjacency
                .free_links
                .iter()
                .all(|&link_ix| adjacency.links[link_ix].is_none())
        );

        adjacency.remove_edge(0, 3);
        assert_eq!(adjacency.successors(0).collect::<Vec<_>>(), vec![1, 3]);
        adjacency.remove_edge(0, 3);
        assert_eq!(adjacency.successors(0).collect::<Vec<_>>(), vec![1]);
        assert_eq!(adjacency.predecessors(3).collect::<Vec<_>>(), vec![1]);

        adjacency.add_edge(0, 6, None, None);
        assert_eq!(adjacency.links.len(), link_slots);
        assert_eq!(adjacency.successors(0).collect::<Vec<_>>(), vec![1, 6]);
        assert_eq!(adjacency.predecessors(6).collect::<Vec<_>>(), vec![6, 0]);
    }

    #[test]
    fn directed_link_state_matches_ordered_count_oracle_through_churn_and_remap() {
        const NODE_COUNT: usize = 16;
        let mut adjacency = DirectedNodeAdjacency::default();
        adjacency.reserve_nodes(NODE_COUNT);
        adjacency.reserve_edges(64);
        for _ in 0..NODE_COUNT {
            adjacency.add_node();
        }

        let mut successors = vec![Vec::new(); NODE_COUNT];
        let mut predecessors = vec![Vec::new(); NODE_COUNT];
        let mut seed = 0x4d59_5df4_d0f3_3173_u64;
        for step in 0..4_096 {
            seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
            let v_ix = ((seed >> 17) as usize) % NODE_COUNT;
            let w_ix = ((seed >> 37) as usize) % NODE_COUNT;
            let pair_exists = successors[v_ix]
                .iter()
                .any(|(neighbor, _)| *neighbor == w_ix);
            let remove = pair_exists && (seed & 3 == 0 || step % 17 == 0);

            if remove {
                adjacency.remove_edge(v_ix, w_ix);
                assert!(remove_count(&mut successors, v_ix, w_ix));
                assert!(remove_count(&mut predecessors, w_ix, v_ix));
            } else {
                adjacency.add_edge(v_ix, w_ix, None, None);
                add_count(&mut successors, v_ix, w_ix);
                add_count(&mut predecessors, w_ix, v_ix);
            }

            if step % 31 == 0 {
                assert_matches(&adjacency, &successors, &predecessors);
            }
        }
        assert_matches(&adjacency, &successors, &predecessors);

        let mut node_remap = vec![None; NODE_COUNT];
        let mut new_node_count = 0;
        for (old_ix, remap) in node_remap.iter_mut().enumerate() {
            if !matches!(old_ix, 3 | 7 | 12) {
                *remap = Some(new_node_count);
                new_node_count += 1;
            }
        }
        adjacency.remap(&node_remap, new_node_count);

        let mut remapped_successors = vec![Vec::new(); new_node_count];
        let mut remapped_predecessors = vec![Vec::new(); new_node_count];
        for (old_owner, row) in successors.iter().enumerate() {
            let Some(new_owner) = node_remap[old_owner] else {
                continue;
            };
            for (old_neighbor, count) in row {
                if let Some(new_neighbor) = node_remap[*old_neighbor] {
                    remapped_successors[new_owner].push((new_neighbor, *count));
                }
            }
        }
        for (old_owner, row) in predecessors.iter().enumerate() {
            let Some(new_owner) = node_remap[old_owner] else {
                continue;
            };
            for (old_neighbor, count) in row {
                if let Some(new_neighbor) = node_remap[*old_neighbor] {
                    remapped_predecessors[new_owner].push((new_neighbor, *count));
                }
            }
        }
        assert_matches(&adjacency, &remapped_successors, &remapped_predecessors);
    }
}
