//! Orient a loop-free component for ranking. All edge indices here are local occurrences.

use super::{Csr, Link};
use crate::work::{LayoutResult, Work, filled, reserved};
use crate::{CyclePolicy, LayoutError, NodeSpec, WorkError};

#[derive(Debug)]
pub(super) struct Orientation {
    /// `reversed[i]` describes `links[i]`, independently of its semantic edge handle.
    pub(super) reversed: Vec<bool>,
    pub(super) topo: Vec<u32>,
    pub(super) cyclic_sccs: usize,
}

const NONE: u32 = u32::MAX;

fn admit(count: usize, arrays: usize, work: &mut Work<'_>) -> LayoutResult<()> {
    work.charge(
        count
            .checked_mul(arrays)
            .ok_or(WorkError::ArithmeticOverflow)?,
    )
}

pub(super) fn orient(
    nodes: &[NodeSpec],
    links: &[Link],
    policy: CyclePolicy,
    work: &mut Work<'_>,
) -> LayoutResult<Orientation> {
    let n = nodes.len();
    let count = u32::try_from(n).map_err(|_| WorkError::ArithmeticOverflow)?;
    u32::try_from(links.len()).map_err(|_| WorkError::ArithmeticOverflow)?;
    work.charge(1)?;
    admit(n, 2, work)?;
    let mut indegree = filled(n, 0u32)?;
    for link in links {
        work.visit(1)?;
        if link.from >= count || link.to >= count || link.from == link.to {
            return Err(LayoutError::InvalidInput(
                "orientation requires loop-free local endpoints",
            ));
        }
        indegree[link.to as usize] += 1; // At most links.len(), checked above.
    }
    let outgoing = Csr::new(n, links.iter().map(|e| e.from), work)?;
    let mut topo = reserved(n)?;
    for (v, &degree) in indegree.iter().enumerate() {
        work.visit(1)?;
        if degree == 0 {
            topo.push(v as u32);
        }
    }
    let mut head = 0;
    while head < topo.len() {
        work.visit(1)?;
        for &e in outgoing.at(topo[head]) {
            work.visit(1)?;
            let to = links[e as usize].to;
            let degree = &mut indegree[to as usize];
            *degree -= 1;
            if *degree == 0 {
                topo.push(to);
            }
        }
        head += 1;
    }
    work.charge(links.len())?;
    let mut reversed = filled(links.len(), false)?;
    if head == n {
        return Ok(Orientation {
            reversed,
            topo,
            cyclic_sccs: 0,
        });
    }
    drop(indegree);
    topo.clear();
    let components = tarjan(n, links, &outgoing, work)?;
    work.charge(n)?;
    let mut strategy = match policy {
        CyclePolicy::SccGreedyFas => {
            let incoming = Csr::new(n, links.iter().map(|e| e.to), work)?;
            Strategy::Greedy {
                incoming,
                degrees: filled(n, Degrees::default())?,
            }
        }
        CyclePolicy::SccDfs => Strategy::Dfs {
            seen: filled(n, false)?,
            frames: Vec::new(),
        },
    };
    for scc in components.members_in_topo_order() {
        work.visit(1)?;
        if scc.len() == 1 {
            topo.push(scc[0]);
            continue;
        }
        let graph = Scc {
            id: components.owner[scc[0] as usize],
            nodes,
            links,
            owner: &components.owner,
            outgoing: &outgoing,
        };
        match &mut strategy {
            Strategy::Greedy { incoming, degrees } => {
                graph.greedy(scc, incoming, degrees, &mut topo, work)?
            }
            Strategy::Dfs { seen, frames } => graph.dfs(scc, seen, frames, &mut topo, work)?,
        }
    }
    // The partition is complete; reuse its vertex-to-component table for positions.
    let mut position = components.owner;
    for (i, &v) in topo.iter().enumerate() {
        work.visit(1)?;
        position[v as usize] = i as u32;
    }
    for (e, link) in links.iter().enumerate() {
        work.visit(1)?;
        reversed[e] = position[link.from as usize] > position[link.to as usize];
    }
    Ok(Orientation {
        reversed,
        topo,
        cyclic_sccs: components.cyclic,
    })
}

enum Strategy {
    Greedy {
        incoming: Csr,
        degrees: Vec<Degrees>,
    },
    Dfs {
        seen: Vec<bool>,
        frames: Vec<Frame>,
    },
}

struct Components {
    owner: Vec<u32>,
    members: Vec<u32>,
    ends: Vec<usize>,
    cyclic: usize,
}

impl Components {
    // Tarjan pops a component only after all its successors. Reversing the component list
    // therefore preserves every bridge without a second sort or an oriented adjacency copy.
    fn members_in_topo_order(&self) -> impl Iterator<Item = &[u32]> {
        self.ends
            .windows(2)
            .rev()
            .map(|ends| &self.members[ends[0]..ends[1]])
    }
}

#[derive(Clone, Copy)]
struct Frame {
    node: u32,
    next: usize,
}

fn tarjan(
    n: usize,
    links: &[Link],
    outgoing: &Csr,
    work: &mut Work<'_>,
) -> LayoutResult<Components> {
    admit(n, 6, work)?;
    let mut index = filled(n, NONE)?;
    let mut low = filled(n, 0u32)?;
    let mut owner = filled(n, NONE)?;
    let mut active = reserved(n)?;
    let mut frames = reserved(n)?;
    let mut members = reserved(n)?;
    let bound = n.checked_add(1).ok_or(WorkError::ArithmeticOverflow)?;
    work.charge(bound)?;
    let mut ends = reserved(bound)?;
    ends.push(0);
    let mut next = 0u32;
    let mut cyclic = 0;
    for root in 0..n {
        work.visit(1)?;
        if index[root] != NONE {
            continue;
        }
        let mut discover = Some(root as u32);
        loop {
            work.visit(1)?;
            if let Some(v) = discover.take() {
                index[v as usize] = next;
                low[v as usize] = next;
                next += 1; // Each vertex is discovered once and n fits u32.
                active.push(v);
                frames.push(Frame { node: v, next: 0 });
            }
            let Some(frame) = frames.last_mut() else {
                break;
            };
            let v = frame.node as usize;
            if let Some(&e) = outgoing.at(frame.node).get(frame.next) {
                frame.next += 1;
                let to = links[e as usize].to as usize;
                if index[to] == NONE {
                    discover = Some(to as u32);
                } else if owner[to] == NONE {
                    low[v] = low[v].min(index[to]);
                }
                continue;
            }
            frames.pop();
            if let Some(parent) = frames.last() {
                let p = parent.node as usize;
                low[p] = low[p].min(low[v]);
            }
            if low[v] == index[v] {
                let id = (ends.len() - 1) as u32;
                let start = members.len();
                loop {
                    work.visit(1)?;
                    let member = active.pop().expect("an SCC root is on the Tarjan stack");
                    owner[member as usize] = id;
                    members.push(member);
                    if member as usize == v {
                        break;
                    }
                }
                cyclic += usize::from(members.len() - start > 1);
                ends.push(members.len());
            }
        }
    }
    Ok(Components {
        owner,
        members,
        ends,
        cyclic,
    })
}

struct Scc<'a> {
    id: u32,
    nodes: &'a [NodeSpec],
    links: &'a [Link],
    owner: &'a [u32],
    outgoing: &'a Csr,
}

impl Scc<'_> {
    fn contains(&self, v: u32) -> bool {
        self.owner[v as usize] == self.id
    }

    /// Reverse finish order preserves every tree/forward/cross edge. Only back edges flip.
    /// The control follows edge declaration order from the first-declared SCC member.
    fn dfs(
        &self,
        members: &[u32],
        seen: &mut [bool],
        frames: &mut Vec<Frame>,
        topo: &mut Vec<u32>,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        let start = topo.len();
        work.charge(members.len())?;
        frames
            .try_reserve(members.len())
            .map_err(|_| LayoutError::AllocationLimit)?;
        let mut root = members[0];
        for &v in members {
            work.visit(1)?;
            if (self.nodes[v as usize].seq, v) < (self.nodes[root as usize].seq, root) {
                root = v;
            }
        }
        // Every member is reachable from this root through intra-SCC edges.
        seen[root as usize] = true;
        frames.push(Frame {
            node: root,
            next: 0,
        });
        while let Some(frame) = frames.last_mut() {
            work.visit(1)?;
            if let Some(&e) = self.outgoing.at(frame.node).get(frame.next) {
                frame.next += 1;
                let to = self.links[e as usize].to;
                if self.contains(to) && !seen[to as usize] {
                    seen[to as usize] = true;
                    frames.push(Frame { node: to, next: 0 });
                }
            } else {
                topo.push(frame.node);
                frames.pop();
            }
        }
        work.charge(topo.len() - start)?;
        topo[start..].reverse();
        Ok(())
    }
}

#[derive(Clone, Copy, Default)]
struct Degrees {
    incoming: u32,
    outgoing: u32,
    slot: u32,
    bucket: usize,
}

// Missing hints sort after supplied ranks, including a supplied i32::MAX. The final local
// vertex id makes the key unique even if two nodes have the same declaration sequence.
fn tie_key(nodes: &[NodeSpec], v: u32) -> (bool, i32, u32, u32) {
    let node = &nodes[v as usize];
    (
        node.order_hint.is_none(),
        node.order_hint.map_or(0, |h| h.rank),
        node.seq,
        v,
    )
}

/// One indexed min-heap per degree-difference bucket. Separating sinks and sources gives
/// them precedence over every delta. The heap positions allow removal on each degree change
/// without stale entries. Per SCC the worst case is O((V + E) log(V + 1)) time, O(V + E)
/// storage, including retained heap capacity across moves. The outer Kahn/Tarjan scans are linear.
struct Buckets {
    heaps: Vec<Vec<u32>>,
    zero: usize,
    highest: usize,
}

impl Buckets {
    fn new(max_in: u32, max_out: u32, work: &mut Work<'_>) -> LayoutResult<Self> {
        // Delta -max_in maps to 0. Two more buckets are reserved for sources and sinks.
        let count = (max_in as usize)
            .checked_add(max_out as usize)
            .and_then(|n| n.checked_add(3))
            .ok_or(WorkError::ArithmeticOverflow)?;
        work.charge(count)?;
        Ok(Self {
            heaps: filled(count, Vec::new())?,
            zero: max_in as usize,
            highest: 0,
        })
    }

    fn source(&self) -> usize {
        self.heaps.len() - 2
    }
    fn sink(&self) -> usize {
        self.heaps.len() - 1
    }

    fn bucket(&self, degree: Degrees) -> usize {
        if degree.outgoing == 0 {
            self.sink()
        } else if degree.incoming == 0 {
            self.source()
        } else {
            self.zero - degree.incoming as usize + degree.outgoing as usize
        }
    }

    fn swap(heap: &mut [u32], degrees: &mut [Degrees], a: usize, b: usize) {
        heap.swap(a, b);
        degrees[heap[a] as usize].slot = a as u32;
        degrees[heap[b] as usize].slot = b as u32;
    }

    fn up(heap: &mut [u32], mut i: usize, nodes: &[NodeSpec], degrees: &mut [Degrees]) {
        let key = tie_key(nodes, heap[i]);
        while i > 0 {
            let parent = (i - 1) / 2;
            if tie_key(nodes, heap[parent]) <= key {
                break;
            }
            Self::swap(heap, degrees, i, parent);
            i = parent;
        }
    }

    fn down(heap: &mut [u32], mut i: usize, nodes: &[NodeSpec], degrees: &mut [Degrees]) {
        let key = tie_key(nodes, heap[i]);
        while i < heap.len() / 2 {
            let mut child = i * 2 + 1;
            if child + 1 < heap.len()
                && tie_key(nodes, heap[child + 1]) < tie_key(nodes, heap[child])
            {
                child += 1;
            }
            if key <= tie_key(nodes, heap[child]) {
                break;
            }
            Self::swap(heap, degrees, i, child);
            i = child;
        }
    }

    fn insert(
        &mut self,
        v: u32,
        nodes: &[NodeSpec],
        degrees: &mut [Degrees],
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        let bucket = self.bucket(degrees[v as usize]);
        if bucket < self.source() {
            self.highest = self.highest.max(bucket);
        }
        let heap = &mut self.heaps[bucket];
        let i = heap.len();
        // Charge the heap walk by its height; no comparison can trigger unmetered rescans.
        work.visit((i + 1).ilog2() as usize + 1)?;
        crate::work::push(heap, v)?;
        degrees[v as usize].slot = i as u32;
        degrees[v as usize].bucket = bucket;
        Self::up(heap, i, nodes, degrees);
        Ok(())
    }

    fn remove(
        &mut self,
        v: u32,
        nodes: &[NodeSpec],
        degrees: &mut [Degrees],
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        let degree = degrees[v as usize];
        let heap = &mut self.heaps[degree.bucket];
        let i = degree.slot as usize;
        work.visit(heap.len().ilog2() as usize + 1)?;
        heap.swap_remove(i);
        degrees[v as usize].slot = NONE;
        if i < heap.len() {
            let replacement = heap[i];
            degrees[replacement as usize].slot = i as u32;
            if i > 0 && tie_key(nodes, replacement) < tie_key(nodes, heap[(i - 1) / 2]) {
                Self::up(heap, i, nodes, degrees);
            } else {
                Self::down(heap, i, nodes, degrees);
            }
        }
        Ok(())
    }

    // Internal deltas change by one per removed edge. Upward jumps of the maximum after
    // a scan are bounded by those moves, so all descending searches together cost O(V + E).
    fn next(&mut self, work: &mut Work<'_>) -> LayoutResult<u32> {
        for special in [self.sink(), self.source()] {
            if let Some(&v) = self.heaps[special].first() {
                return Ok(v);
            }
        }
        while self.heaps[self.highest].is_empty() {
            work.visit(1)?;
            self.highest -= 1; // Called with at least one active vertex in an ordinary bucket.
        }
        Ok(self.heaps[self.highest][0])
    }
}

impl Scc<'_> {
    fn greedy(
        &self,
        members: &[u32],
        incoming: &Csr,
        degrees: &mut [Degrees],
        topo: &mut Vec<u32>,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        let (mut max_in, mut max_out) = (0, 0);
        for &v in members {
            work.visit(1)?;
            for &e in self.outgoing.at(v) {
                work.visit(1)?;
                let to = self.links[e as usize].to;
                if self.contains(to) {
                    degrees[v as usize].outgoing += 1;
                    degrees[to as usize].incoming += 1;
                    max_in = max_in.max(degrees[to as usize].incoming);
                    max_out = max_out.max(degrees[v as usize].outgoing);
                }
            }
        }
        let mut buckets = Buckets::new(max_in, max_out, work)?;
        for &v in members {
            buckets.insert(v, self.nodes, degrees, work)?;
        }
        let mut left = topo.len();
        let mut right = left + members.len(); // Disjoint SCCs partition the reserved n slots.
        work.charge(members.len())?;
        topo.resize(right, 0);
        while left < right {
            work.visit(1)?;
            let v = buckets.next(work)?;
            if degrees[v as usize].outgoing == 0 {
                right -= 1;
                topo[right] = v;
            } else {
                topo[left] = v;
                left += 1;
            }
            buckets.remove(v, self.nodes, degrees, work)?;
            for (outbound, edges) in [(true, self.outgoing.at(v)), (false, incoming.at(v))] {
                for &e in edges {
                    work.visit(1)?;
                    let edge = &self.links[e as usize];
                    let peer = if outbound { edge.to } else { edge.from };
                    if !self.contains(peer) || degrees[peer as usize].slot == NONE {
                        continue;
                    }
                    buckets.remove(peer, self.nodes, degrees, work)?;
                    let degree = &mut degrees[peer as usize];
                    if outbound {
                        degree.incoming -= 1;
                    } else {
                        degree.outgoing -= 1;
                    }
                    buckets.insert(peer, self.nodes, degrees, work)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NoopWorkControl, OrderHint, Size, WorkControl};

    fn nodes(n: usize) -> Vec<NodeSpec> {
        (0..n)
            .map(|v| NodeSpec::new(Size::default(), v as u32))
            .collect()
    }

    fn links(edges: &[(u32, u32)]) -> Vec<Link> {
        edges
            .iter()
            .enumerate()
            .map(|(i, &(from, to))| Link {
                from,
                to,
                edge: (1000 + i) as u32,
                span: 1,
                weight: 1.0,
            })
            .collect()
    }

    fn run(nodes: &[NodeSpec], links: &[Link], policy: CyclePolicy) -> Orientation {
        let result = orient(nodes, links, policy, &mut Work::new(&mut NoopWorkControl)).unwrap();
        assert_eq!(result.reversed.len(), links.len());
        let mut positions = vec![None; nodes.len()];
        for (i, &v) in result.topo.iter().enumerate() {
            assert!(positions[v as usize].replace(i).is_none());
        }
        assert!(positions.iter().all(Option::is_some));
        for (link, &reversed) in links.iter().zip(&result.reversed) {
            let from = positions[link.from as usize].unwrap();
            let to = positions[link.to as usize].unwrap();
            assert_eq!(reversed, from > to);
        }
        result
    }

    fn feedback(links: &[Link], orientation: &Orientation) -> Vec<u32> {
        let mut ids: Vec<_> = links
            .iter()
            .zip(&orientation.reversed)
            .filter_map(|(e, &flipped)| flipped.then_some(e.edge))
            .collect();
        ids.sort_unstable();
        ids
    }

    const POLICIES: [CyclePolicy; 2] = [CyclePolicy::SccGreedyFas, CyclePolicy::SccDfs];

    #[test]
    fn dag_and_empty_fast_paths_return_an_unflipped_permutation() {
        for policy in POLICIES {
            let empty = run(&[], &[], policy);
            assert_eq!(empty.cyclic_sccs, 0);
            let dag = run(
                &nodes(7),
                &links(&[(5, 1), (5, 1), (1, 3), (5, 3), (4, 1)]),
                policy,
            );
            assert_eq!(dag.cyclic_sccs, 0);
            assert!(dag.reversed.iter().all(|&r| !r));
        }
    }

    #[test]
    fn chains_of_sccs_keep_all_bridges_and_greedy_feedback_across_edge_permutations() {
        for n in [25, 50, 75, 100] {
            let nodes = nodes(n);
            let mut pairs = Vec::new();
            for first in (0..n as u32).step_by(5) {
                for offset in 0..5 {
                    pairs.push((first + offset, first + (offset + 1) % 5));
                }
                // A second occurrence counts toward the degree difference.
                pairs.push((first + 3, first + 4));
                if first > 0 {
                    pairs.extend([(first - 3, first + 3), (first - 2, first + 1)]);
                }
            }
            let mut links = links(&pairs);
            let reference = run(&nodes, &links, CyclePolicy::SccGreedyFas);
            let expected = feedback(&links, &reference);
            assert_eq!(expected.len(), n / 5);
            links.reverse();
            for policy in POLICIES {
                let oriented = run(&nodes, &links, policy);
                assert_eq!(oriented.cyclic_sccs, n / 5);
                for (e, &reversed) in links.iter().zip(&oriented.reversed) {
                    if e.from / 5 != e.to / 5 {
                        assert!(!reversed);
                    }
                }
                if policy == CyclePolicy::SccGreedyFas {
                    assert_eq!(feedback(&links, &oriented), expected);
                    assert_eq!(oriented.topo, reference.topo);
                }
            }
        }
    }

    #[test]
    fn equal_delta_uses_hint_then_declaration_then_local_vertex() {
        let mut nodes = nodes(4);
        for node in &mut nodes {
            node.seq = 0;
        }
        let links = links(&[(0, 1), (1, 2), (2, 3), (3, 0)]);
        let greedy = |nodes: &[NodeSpec]| run(nodes, &links, CyclePolicy::SccGreedyFas).topo[0];
        assert_eq!(greedy(&nodes), 0);
        nodes[1].order_hint = Some(OrderHint {
            rank: i32::MAX,
            cross_key: 5.0,
        });
        assert_eq!(greedy(&nodes), 1);
        let hint = Some(OrderHint {
            rank: -2,
            cross_key: 100.0,
        });
        nodes[2].order_hint = hint;
        nodes[3].order_hint = hint;
        nodes[2].seq = 1;
        assert_eq!(greedy(&nodes), 3);
        nodes[2].seq = 0;
        assert_eq!(greedy(&nodes), 2);
    }

    // Small independent Eades-Lin-Smyth oracle: count all live occurrences on every removal.
    fn slow_greedy(nodes: &[NodeSpec], links: &[Link]) -> Vec<u32> {
        let mut active = vec![true; nodes.len()];
        let (mut left, mut right) = (Vec::new(), Vec::new());
        for _ in nodes {
            let mut degrees = vec![(0i64, 0i64); nodes.len()];
            for e in links {
                if active[e.from as usize] && active[e.to as usize] {
                    degrees[e.from as usize].1 += 1;
                    degrees[e.to as usize].0 += 1;
                }
            }
            let picked = (0..nodes.len())
                .filter(|&v| active[v])
                .min_by_key(|&v| {
                    let (incoming, outgoing) = degrees[v];
                    let choice = if outgoing == 0 {
                        (0, 0)
                    } else if incoming == 0 {
                        (1, 0)
                    } else {
                        (2, incoming - outgoing)
                    };
                    (choice, tie_key(nodes, v as u32))
                })
                .unwrap();
            active[picked] = false;
            if degrees[picked].1 == 0 {
                right.push(picked as u32);
            } else {
                left.push(picked as u32);
            }
        }
        left.extend(right.into_iter().rev());
        left
    }

    #[test]
    fn buckets_match_recounting_greedy_for_parallel_and_antiparallel_occurrences() {
        let mut seed = 11u32;
        let mut next = || {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            seed
        };
        for n in 2..18 {
            for _ in 0..8 {
                let mut nodes = nodes(n as usize);
                for node in &mut nodes {
                    node.seq = next() % 4;
                    node.order_hint = (next() % 3 == 1).then(|| OrderHint {
                        rank: (next() % 3) as i32 - 1,
                        cross_key: 0.0,
                    });
                }
                let mut pairs: Vec<_> = (0..n).map(|v| (v, (v + 1) % n)).collect();
                for _ in 0..n * 3 {
                    let (from, to) = (next() % n, next() % n);
                    if from != to {
                        pairs.extend([(from, to), (from, to), (to, from)]);
                    }
                }
                let mut links = links(&pairs);
                let expected = slow_greedy(&nodes, &links);
                for _ in 0..2 {
                    let result = run(&nodes, &links, CyclePolicy::SccGreedyFas);
                    assert_eq!(result.cyclic_sccs, 1);
                    assert_eq!(result.topo, expected);
                    links.reverse();
                }
            }
        }
    }

    #[test]
    fn dfs_control_follows_declaration_order_inside_each_scc() {
        let nodes = nodes(4);
        let mut links = links(&[(0, 1), (0, 2), (1, 3), (2, 3), (3, 0), (3, 1), (3, 2)]);
        let a = run(&nodes, &links, CyclePolicy::SccDfs);
        let first = feedback(&links, &a);
        links.reverse();
        let b = run(&nodes, &links, CyclePolicy::SccDfs);
        assert_eq!((a.cyclic_sccs, b.cyclic_sccs), (1, 1));
        assert_ne!(first, feedback(&links, &b));
    }

    #[test]
    fn all_four_vertex_digraphs_match_mutual_reachability() {
        let nodes = nodes(4);
        let arcs: Vec<_> = (0..4u32)
            .flat_map(|v| (0..4).filter(move |&w| v != w).map(move |w| (v, w)))
            .collect();
        for mask in 0..1u32 << arcs.len() {
            let pairs: Vec<_> = arcs
                .iter()
                .enumerate()
                .filter_map(|(i, &e)| (mask & (1 << i) != 0).then_some(e))
                .collect();
            let mut links = links(&pairs);
            let mut reachable = [1u8, 2, 4, 8];
            for e in &links {
                reachable[e.from as usize] |= 1 << e.to;
            }
            for k in 0..4 {
                let through = reachable[k];
                for row in &mut reachable {
                    if *row & (1 << k) != 0 {
                        *row |= through;
                    }
                }
            }
            let same =
                |a: usize, b: usize| reachable[a] & (1 << b) != 0 && reachable[b] & (1 << a) != 0;
            let cyclic = (0..4)
                .filter(|&v| !(0..v).any(|w| same(v, w)) && (v + 1..4).any(|w| same(v, w)))
                .count();
            for policy in POLICIES {
                let oriented = run(&nodes, &links, policy);
                assert_eq!(oriented.cyclic_sccs, cyclic, "mask {mask}");
                for (e, &flip) in links.iter().zip(&oriented.reversed) {
                    assert!(!flip || same(e.from as usize, e.to as usize), "mask {mask}");
                }
                if policy == CyclePolicy::SccGreedyFas {
                    let expected = feedback(&links, &oriented);
                    links.reverse();
                    let permuted = run(&nodes, &links, policy);
                    assert_eq!(feedback(&links, &permuted), expected, "mask {mask}");
                }
            }
        }
    }

    #[test]
    fn deep_dfs_and_tarjan_use_explicit_stacks() {
        const N: u32 = 16_384;
        let nodes = nodes(N as usize);
        let links = links(&(0..N).map(|v| (v, (v + 1) % N)).collect::<Vec<_>>());
        for policy in POLICIES {
            let result = run(&nodes, &links, policy);
            assert_eq!(result.cyclic_sccs, 1);
            assert_eq!(
                feedback(&links, &result),
                vec![links[(N - 1) as usize].edge]
            );
        }
    }

    #[test]
    fn invalid_local_inputs_and_work_errors_are_typed() {
        let nodes = nodes(4);
        for bad in [links(&[(0, 0)]), links(&[(0, 4)]), links(&[(u32::MAX, 0)])] {
            assert!(matches!(
                orient(
                    &nodes,
                    &bad,
                    CyclePolicy::SccGreedyFas,
                    &mut Work::new(&mut NoopWorkControl)
                ),
                Err(LayoutError::InvalidInput(_))
            ));
        }
        let links = links(&[(0, 1), (1, 2), (2, 0), (0, 3), (3, 1)]);
        struct Meter {
            used: usize,
            budget: usize,
        }
        impl WorkControl for Meter {
            fn check(&mut self) -> Result<(), WorkError> {
                Ok(())
            }
            fn charge(&mut self, n: usize) -> Result<(), WorkError> {
                let next = self
                    .used
                    .checked_add(n)
                    .ok_or(WorkError::ArithmeticOverflow)?;
                if next > self.budget {
                    return Err(WorkError::Interrupted);
                }
                self.used = next;
                Ok(())
            }
        }
        for policy in POLICIES {
            let solve = |limit| {
                let mut meter = Meter {
                    used: 0,
                    budget: limit,
                };
                let mut work = Work::new(&mut meter);
                let output = orient(&nodes, &links, policy, &mut work)?;
                work.finish()?;
                Ok::<_, LayoutError>((output, meter.used))
            };
            let (reference, total) = solve(usize::MAX).unwrap();
            for limit in [0, total / 2, total - 1] {
                assert!(matches!(
                    solve(limit),
                    Err(LayoutError::Work(WorkError::Interrupted))
                ));
            }
            let (exact, used) = solve(total).unwrap();
            assert_eq!(used, total);
            assert_eq!(exact.reversed, reference.reversed);
            assert_eq!(exact.topo, reference.topo);
        }
        assert_eq!(
            admit(usize::MAX, 2, &mut Work::new(&mut NoopWorkControl)),
            Err(LayoutError::Work(WorkError::ArithmeticOverflow))
        );
    }
}
