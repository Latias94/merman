//! Adjacency indexes and caches used by [`Graph`](super::Graph).
//!
//! Dagre algorithms query edges and adjacent nodes repeatedly. Edge CSR is rebuilt lazily after
//! mutation, while directed node adjacency is maintained incrementally to preserve Graphlib's
//! counted insertion-order semantics.

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
    successor_prev: Option<usize>,
    successor_next: Option<usize>,
    predecessor_prev: Option<usize>,
    predecessor_next: Option<usize>,
}

#[derive(Debug, Clone, Copy, Default)]
struct OrderedLinkList {
    head: Option<usize>,
    tail: Option<usize>,
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

/// Graphlib keeps `_sucs` and `_preds` as insertion-ordered endpoint keys with parallel-edge
/// counts. This state mirrors that lifecycle so removing one named edge cannot reorder a neighbor.
#[derive(Debug, Clone, Default)]
pub(in crate::graph) struct DirectedNodeAdjacency {
    link_index: DirectedLinkIndex,
    links: Vec<Option<DirectedLink>>,
    free_links: Vec<usize>,
    successors: Vec<OrderedLinkList>,
    predecessors: Vec<OrderedLinkList>,
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
        self.successors.push(OrderedLinkList::default());
        self.predecessors.push(OrderedLinkList::default());
    }

    pub(in crate::graph) fn truncate_nodes(&mut self, len: usize) {
        self.successors.truncate(len);
        self.predecessors.truncate(len);
    }

    pub(in crate::graph) fn clear(&mut self) {
        self.link_index.clear();
        self.links.clear();
        self.free_links.clear();
        self.successors.clear();
        self.predecessors.clear();
    }

    pub(in crate::graph) fn add_edge(&mut self, v_ix: usize, w_ix: usize) {
        if let Some(&link_ix) = self.link_index.get(&(v_ix, w_ix)) {
            let link = self.links[link_ix]
                .as_mut()
                .expect("directed link index referenced a removed link");
            link.count = link.count.saturating_add(1);
            return;
        }

        let link_ix = self.free_links.pop().unwrap_or(self.links.len());
        let link = DirectedLink {
            v_ix,
            w_ix,
            count: 1,
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
        self.append_successor(v_ix, link_ix);
        self.append_predecessor(w_ix, link_ix);
    }

    pub(in crate::graph) fn remove_edge(&mut self, v_ix: usize, w_ix: usize) {
        let Some(&link_ix) = self.link_index.get(&(v_ix, w_ix)) else {
            debug_assert!(false, "directed adjacency did not contain the removed edge");
            return;
        };
        let link = self.links[link_ix]
            .as_mut()
            .expect("directed link index referenced a removed link");
        if link.count > 1 {
            link.count -= 1;
            return;
        }

        self.unlink_successor(v_ix, link_ix);
        self.unlink_predecessor(w_ix, link_ix);
        self.link_index.remove(&(v_ix, w_ix));
        self.links[link_ix] = None;
        self.free_links.push(link_ix);
    }

    pub(in crate::graph) fn remap(&mut self, node_remap: &[Option<usize>], new_node_slots: usize) {
        let old = std::mem::take(self);
        self.reserve_edges(old.link_index.len());
        self.successors = vec![OrderedLinkList::default(); new_node_slots];
        self.predecessors = vec![OrderedLinkList::default(); new_node_slots];

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
                successor_prev: None,
                successor_next: None,
                predecessor_prev: None,
                predecessor_next: None,
            }));
            self.link_index.insert((v_ix, w_ix), new_link_ix);
            link_remap[old_link_ix] = Some(new_link_ix);
        }

        for old_list in &old.successors {
            let mut next = old_list.head;
            while let Some(old_link_ix) = next {
                let old_link = old.links[old_link_ix]
                    .as_ref()
                    .expect("successor list referenced a removed link");
                next = old_link.successor_next;
                if let Some(new_link_ix) = link_remap[old_link_ix] {
                    let v_ix = self.links[new_link_ix]
                        .as_ref()
                        .expect("remapped successor link was missing")
                        .v_ix;
                    self.append_successor(v_ix, new_link_ix);
                }
            }
        }
        for old_list in &old.predecessors {
            let mut next = old_list.head;
            while let Some(old_link_ix) = next {
                let old_link = old.links[old_link_ix]
                    .as_ref()
                    .expect("predecessor list referenced a removed link");
                next = old_link.predecessor_next;
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

    pub(in crate::graph) fn first_successor(&self, v_ix: usize) -> Option<usize> {
        let link_ix = self.successors[v_ix].head?;
        self.links[link_ix].as_ref().map(|link| link.w_ix)
    }

    pub(in crate::graph) fn first_predecessor(&self, v_ix: usize) -> Option<usize> {
        let link_ix = self.predecessors[v_ix].head?;
        self.links[link_ix].as_ref().map(|link| link.v_ix)
    }

    pub(in crate::graph) fn successors(&self, v_ix: usize) -> impl Iterator<Item = usize> + '_ {
        OrderedLinkIter {
            links: &self.links,
            next: self.successors[v_ix].head,
            direction: LinkDirection::Successor,
        }
    }

    pub(in crate::graph) fn predecessors(&self, v_ix: usize) -> impl Iterator<Item = usize> + '_ {
        OrderedLinkIter {
            links: &self.links,
            next: self.predecessors[v_ix].head,
            direction: LinkDirection::Predecessor,
        }
    }

    fn append_successor(&mut self, v_ix: usize, link_ix: usize) {
        let tail = self.successors[v_ix].tail;
        if let Some(tail_ix) = tail {
            self.links[tail_ix]
                .as_mut()
                .expect("successor tail referenced a removed link")
                .successor_next = Some(link_ix);
        } else {
            self.successors[v_ix].head = Some(link_ix);
        }
        let link = self.links[link_ix]
            .as_mut()
            .expect("appended successor link was missing");
        link.successor_prev = tail;
        link.successor_next = None;
        self.successors[v_ix].tail = Some(link_ix);
        self.successors[v_ix].len += 1;
    }

    fn append_predecessor(&mut self, w_ix: usize, link_ix: usize) {
        let tail = self.predecessors[w_ix].tail;
        if let Some(tail_ix) = tail {
            self.links[tail_ix]
                .as_mut()
                .expect("predecessor tail referenced a removed link")
                .predecessor_next = Some(link_ix);
        } else {
            self.predecessors[w_ix].head = Some(link_ix);
        }
        let link = self.links[link_ix]
            .as_mut()
            .expect("appended predecessor link was missing");
        link.predecessor_prev = tail;
        link.predecessor_next = None;
        self.predecessors[w_ix].tail = Some(link_ix);
        self.predecessors[w_ix].len += 1;
    }

    fn unlink_successor(&mut self, v_ix: usize, link_ix: usize) {
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
            self.successors[v_ix].head = next;
        }
        if let Some(next_ix) = next {
            self.links[next_ix]
                .as_mut()
                .expect("successor next referenced a removed link")
                .successor_prev = prev;
        } else {
            self.successors[v_ix].tail = prev;
        }
        self.successors[v_ix].len = self.successors[v_ix].len.saturating_sub(1);
    }

    fn unlink_predecessor(&mut self, w_ix: usize, link_ix: usize) {
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
            self.predecessors[w_ix].head = next;
        }
        if let Some(next_ix) = next {
            self.links[next_ix]
                .as_mut()
                .expect("predecessor next referenced a removed link")
                .predecessor_prev = prev;
        } else {
            self.predecessors[w_ix].tail = prev;
        }
        self.predecessors[w_ix].len = self.predecessors[w_ix].len.saturating_sub(1);
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
                adjacency.add_edge(v_ix, w_ix);
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
