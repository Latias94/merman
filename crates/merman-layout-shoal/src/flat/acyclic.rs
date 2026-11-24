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
    removed: bool,
}

impl Degrees {
    /// Sinks take 0, sources 1, and ordinary deltas map in descending order into 2..2^33.
    fn bucket(self) -> u64 {
        if self.outgoing == 0 {
            0
        } else if self.incoming == 0 {
            1
        } else {
            (1u64 << 32) + u64::from(self.incoming) - u64::from(self.outgoing)
        }
    }
}

/// Words run from least to most significant. A missing rank follows even a supplied i32::MAX.
#[derive(Clone, Copy)]
struct Priority([u64; 3]);

impl Priority {
    const MIN: Self = Self([0; 3]);

    fn new(node: &NodeSpec, vertex: u32, bucket: u64) -> Self {
        let rank = node
            .effective_order_hint()
            .map_or(1u64 << 32, |hint| u64::from((hint.rank as u32) ^ (1 << 31)));
        Self([
            (u64::from(node.seq) << 32) | u64::from(vertex),
            rank,
            bucket,
        ])
    }

    fn split_bit(self, other: Self) -> u8 {
        for word in (0..self.0.len()).rev() {
            let difference = self.0[word] ^ other.0[word];
            if difference != 0 {
                return word as u8 * 64 + difference.ilog2() as u8;
            }
        }
        unreachable!("distinct local vertices have distinct priorities")
    }

    fn side(self, bit: u8) -> usize {
        ((self.0[usize::from(bit / 64)] >> (bit % 64)) & 1) as usize
    }
}

#[derive(Clone, Copy)]
enum Entry {
    Leaf(u32),
    Branch(u32),
}

struct Branch {
    bit: u8,
    children: [Entry; 2],
}

/// One Patricia trie indexes the occupied SCC-local buckets and the ties within them.
/// Branch bits strictly decrease: 33 bucket bits, 33 rank bits, 32 sequence bits, 32 ID bits.
/// Taking the leftmost leaf selects the minimum in at most 130 branches, including bucket
/// selection. Insertion takes at most two walks. Each vertex is inserted and peeled once,
/// and each live edge can move one peer once, giving O(V + E) total time.
/// Building V leaves uses V - 1 branches. Each move reuses the branch its removal freed.
struct Buckets<'a> {
    nodes: &'a [NodeSpec],
    root: Option<Entry>,
    branches: Vec<Branch>,
}

impl<'a> Buckets<'a> {
    fn from_members(
        nodes: &'a [NodeSpec],
        members: &[u32],
        degrees: &[Degrees],
        work: &mut Work<'_>,
    ) -> LayoutResult<Self> {
        work.charge(members.len())?;
        let mut buckets = Self {
            nodes,
            root: None,
            branches: reserved(members.len().saturating_sub(1))?,
        };
        for &vertex in members {
            buckets.insert(vertex, degrees, None, work)?;
        }
        Ok(buckets)
    }

    fn attach(&mut self, parent: Option<(u32, usize)>, child: Entry) {
        if let Some((branch, side)) = parent {
            self.branches[branch as usize].children[side] = child;
        } else {
            self.root = Some(child);
        }
    }

    fn insert(
        &mut self,
        vertex: u32,
        degrees: &[Degrees],
        vacant: Option<u32>,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        work.visit(1)?;
        let leaf = Entry::Leaf(vertex);
        let Some(root) = self.root else {
            self.root = Some(leaf);
            return Ok(());
        };
        let priority = |vertex: u32| {
            let index = vertex as usize;
            Priority::new(&self.nodes[index], vertex, degrees[index].bucket())
        };
        let key = priority(vertex);
        let mut cursor = root;
        let bit = loop {
            match cursor {
                Entry::Leaf(other) => break key.split_bit(priority(other)),
                Entry::Branch(index) => {
                    work.visit(1)?;
                    let branch = &self.branches[index as usize];
                    cursor = branch.children[key.side(branch.bit)];
                }
            }
        };
        let mut parent = None;
        cursor = root;
        while let Entry::Branch(index) = cursor {
            work.visit(1)?;
            let branch = &self.branches[index as usize];
            if branch.bit <= bit {
                break;
            }
            let side = key.side(branch.bit);
            parent = Some((index, side));
            cursor = branch.children[side];
        }
        let mut children = [cursor; 2];
        children[key.side(bit)] = leaf;
        let branch = Branch { bit, children };
        let index = if let Some(index) = vacant {
            self.branches[index as usize] = branch;
            index
        } else {
            let index =
                u32::try_from(self.branches.len()).map_err(|_| WorkError::ArithmeticOverflow)?;
            self.branches.push(branch);
            index
        };
        self.attach(parent, Entry::Branch(index));
        Ok(())
    }

    fn take(
        &mut self,
        probe: Priority,
        track_editor_tie: bool,
        work: &mut Work<'_>,
    ) -> LayoutResult<(u32, Option<u32>)> {
        let mut cursor = self.root.expect("only a populated queue is searched");
        let mut parent = None;
        let mut grandparent = None;
        let vertex = loop {
            work.visit(1)?;
            match cursor {
                Entry::Leaf(vertex) => break vertex,
                Entry::Branch(index) => {
                    let branch = &self.branches[index as usize];
                    if track_editor_tie && branch.bit / 64 == 1 {
                        work.editor_choice = true;
                    }
                    let side = probe.side(branch.bit);
                    grandparent = parent;
                    parent = Some((index, side));
                    cursor = branch.children[side];
                }
            }
        };
        if let Some((index, side)) = parent {
            let sibling = self.branches[index as usize].children[side ^ 1];
            self.attach(grandparent, sibling);
            Ok((vertex, Some(index)))
        } else {
            self.root = None;
            Ok((vertex, None))
        }
    }

    fn decrement(
        &mut self,
        vertex: u32,
        incoming: bool,
        degrees: &mut [Degrees],
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        let degree = &mut degrees[vertex as usize];
        let previous = degree.bucket();
        if incoming {
            degree.incoming -= 1;
        } else {
            degree.outgoing -= 1;
        }
        if previous != degree.bucket() {
            let old_key = Priority::new(&self.nodes[vertex as usize], vertex, previous);
            let (found, vacant) = self.take(old_key, false, work)?;
            debug_assert_eq!(found, vertex);
            self.insert(vertex, degrees, vacant, work)?;
        }
        Ok(())
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
        let mut has_editor_hint = false;
        for &v in members {
            work.visit(1)?;
            has_editor_hint |= self.nodes[v as usize].editor_order_hint().is_some();
            for &e in self.outgoing.at(v) {
                work.visit(1)?;
                let to = self.links[e as usize].to;
                if self.contains(to) {
                    degrees[v as usize].outgoing += 1;
                    degrees[to as usize].incoming += 1;
                }
            }
        }
        let mut buckets = Buckets::from_members(self.nodes, members, degrees, work)?;
        let mut left = topo.len();
        let mut right = left + members.len(); // Disjoint SCCs partition the reserved n slots.
        work.charge(members.len())?;
        topo.resize(right, 0);
        while left < right {
            work.visit(1)?;
            let (v, _) = buckets.take(Priority::MIN, has_editor_hint, work)?;
            degrees[v as usize].removed = true;
            if degrees[v as usize].outgoing == 0 {
                right -= 1;
                topo[right] = v;
            } else {
                topo[left] = v;
                left += 1;
            }
            for (outbound, edges) in [(true, self.outgoing.at(v)), (false, incoming.at(v))] {
                for &e in edges {
                    work.visit(1)?;
                    let edge = &self.links[e as usize];
                    let peer = if outbound { edge.to } else { edge.from };
                    if !self.contains(peer) || degrees[peer as usize].removed {
                        continue;
                    }
                    buckets.decrement(peer, outbound, degrees, work)?;
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

    fn with_budget<T>(
        budget: usize,
        run: impl FnOnce(&mut Work<'_>) -> LayoutResult<T>,
    ) -> LayoutResult<(T, usize)> {
        struct Meter {
            used: usize,
            budget: usize,
        }
        impl WorkControl for Meter {
            fn check(&mut self) -> Result<(), WorkError> {
                Ok(())
            }
            fn charge(&mut self, units: usize) -> Result<(), WorkError> {
                let next = self
                    .used
                    .checked_add(units)
                    .ok_or(WorkError::ArithmeticOverflow)?;
                if next > self.budget {
                    return Err(WorkError::Interrupted);
                }
                self.used = next;
                Ok(())
            }
        }
        let mut meter = Meter { used: 0, budget };
        let mut work = Work::new(&mut meter);
        let result = run(&mut work)?;
        work.finish()?;
        Ok((result, meter.used))
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
                    let node = &nodes[v];
                    (
                        choice,
                        node.order_hint.is_none(),
                        node.order_hint.map_or(0, |hint| hint.rank),
                        node.seq,
                        v,
                    )
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
    fn shuffled_multigraphs_with_full_width_ties_match_the_recounting_oracle() {
        let mut state = 0x6a09e667u32;
        let mut random = || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state
        };
        for count in [7u32, 16, 33, 64] {
            for trial in 0..12 {
                let mut nodes = nodes(count as usize);
                let shared_seq = random();
                let shared_rank = random() as i32;
                for node in &mut nodes {
                    node.seq = match random() % 5 {
                        0 => 0,
                        1 => u32::MAX,
                        2 | 3 => shared_seq,
                        _ => random(),
                    };
                    node.order_hint = match random() % 7 {
                        0 | 1 => None,
                        2 => Some(i32::MIN),
                        3 => Some(i32::MAX),
                        4 | 5 => Some(shared_rank),
                        _ => Some(random() as i32),
                    }
                    .map(|rank| OrderHint {
                        rank,
                        cross_key: f64::from(random()),
                    });
                }
                let mut pairs: Vec<_> = (0..count).map(|v| (v, (v + 1) % count)).collect();
                for _ in 0..3 * count {
                    let from = if random() % 4 == 0 {
                        0
                    } else {
                        random() % count
                    };
                    let to = (from + 1 + random() % (count - 1)) % count;
                    pairs.extend(std::iter::repeat_n((from, to), 1 + (random() % 5) as usize));
                }
                let mut links = links(&pairs);
                let expected = slow_greedy(&nodes, &links);
                let forward = run(&nodes, &links, CyclePolicy::SccGreedyFas);
                assert_eq!(forward.topo, expected, "size {count}, trial {trial}");
                let expected_feedback = feedback(&links, &forward);
                for last in (1..links.len()).rev() {
                    links.swap(last, random() as usize % (last + 1));
                }
                for _ in 0..2 {
                    let result = run(&nodes, &links, CyclePolicy::SccGreedyFas);
                    assert_eq!(result.topo, expected, "size {count}, trial {trial}");
                    assert_eq!(feedback(&links, &result), expected_feedback);
                    links.reverse();
                }
            }
        }
    }

    #[test]
    fn sparse_extreme_delta_buckets_reuse_branches_with_bounded_visits() {
        let mut nodes = nodes(8);
        nodes[5].order_hint = Some(OrderHint {
            rank: i32::MAX,
            cross_key: 0.0,
        });
        let max = u32::MAX;
        let mut degrees = [
            (max, 1),
            (1, max),
            (max, max),
            (0, max),
            (max, 0),
            (0, 0),
            (max - 1, 1),
            (1, max - 1),
        ]
        .map(|(incoming, outgoing)| Degrees {
            incoming,
            outgoing,
            removed: false,
        });
        let members = [7, 6, 5, 4, 3, 2, 1, 0];
        let (mut buckets, _) = with_budget(2 * members.len().pow(2), |work| {
            Buckets::from_members(&nodes, &members, &degrees, work)
        })
        .unwrap();
        for _ in 0..64 {
            for (vertex, incoming) in [(1, false), (0, true), (2, true)] {
                let (_, used) = with_budget(3 * members.len(), |work| {
                    buckets.decrement(vertex, incoming, &mut degrees, work)
                })
                .unwrap();
                assert!(used > 0);
            }
        }
        assert_eq!(buckets.branches.len(), members.len() - 1);
        for expected in [5, 4, 3, 7, 1, 2, 0, 6] {
            let ((next, _), _) = with_budget(members.len(), |work| {
                buckets.take(Priority::MIN, false, work)
            })
            .unwrap();
            assert_eq!(next, expected);
        }
        assert!(buckets.root.is_none());
    }

    #[test]
    fn leftmost_walk_charges_each_significant_bit_once() {
        const ID_BITS: usize = 8;
        const WALK_LIMIT: usize = 33 + 33 + 32 + 32 + 1;
        let hint = |rank| {
            Some(OrderHint {
                rank,
                cross_key: 0.0,
            })
        };
        let base = NodeSpec {
            order_hint: hint(i32::MIN),
            ..NodeSpec::new(Size::default(), 0)
        };
        let sink = Degrees {
            incoming: 1,
            ..Degrees::default()
        };
        let mut nodes = vec![base; 1 << ID_BITS];
        let mut degrees = vec![sink; nodes.len()];
        let mut add = |node, degree| {
            nodes.push(node);
            degrees.push(degree);
        };
        for bit in 0..32 {
            add(
                NodeSpec {
                    seq: 1 << bit,
                    ..base
                },
                sink,
            );
            add(
                NodeSpec {
                    order_hint: hint(i32::MIN ^ (1 << bit)),
                    ..base
                },
                sink,
            );
        }
        add(
            NodeSpec {
                order_hint: None,
                ..base
            },
            sink,
        );
        add(
            base,
            Degrees {
                incoming: 0,
                outgoing: 1,
                ..Degrees::default()
            },
        );
        for bit in 1..=32 {
            let outgoing = ((1u64 << 32) + 1 - (1u64 << bit)) as u32;
            add(base, Degrees { outgoing, ..sink });
        }
        let members: Vec<_> = (0..nodes.len() as u32).rev().collect();
        let (mut buckets, _) = with_budget(2 * WALK_LIMIT * members.len(), |work| {
            Buckets::from_members(&nodes, &members, &degrees, work)
        })
        .unwrap();
        let ((minimum, _), used) =
            with_budget(WALK_LIMIT, |work| buckets.take(Priority::MIN, false, work)).unwrap();
        assert_eq!(minimum, 0);
        assert_eq!(used, 33 + 33 + 32 + ID_BITS + 1);
    }

    #[test]
    fn orientation_work_has_a_linear_budget_for_ties_parallel_edges_and_small_sccs() {
        for count in [64u32, 512, 4096] {
            let nodes = vec![NodeSpec::new(Size::default(), 0); count as usize];
            let ring: Vec<_> = (0..count).map(|v| (v, (v + 1) % count)).collect();
            let mut paired = Vec::new();
            for first in (0..count).step_by(2) {
                paired.extend([(first, first + 1), (first + 1, first)]);
                if first > 0 {
                    paired.push((first - 1, first));
                }
            }
            let star = (1..count).flat_map(|v| [(0, v), (0, v), (v, 0)]).collect();
            for (family, pairs, components) in [
                ("ring", ring, 1),
                ("paired", paired, count / 2),
                ("star", star, 1),
            ] {
                let edges = links(&pairs);
                let budget = 64 * (nodes.len() + edges.len());
                let (result, used) = with_budget(budget, |work| {
                    orient(&nodes, &edges, CyclePolicy::SccGreedyFas, work)
                })
                .unwrap_or_else(|error| {
                    panic!(
                        "{family} V={count} E={} budget={budget}: {error}",
                        edges.len()
                    )
                });
                assert_eq!(result.cyclic_sccs, components as usize);
                assert_eq!(result.topo.len(), nodes.len());
                eprintln!(
                    "{family} V={count} E={} work={used} budget={budget}",
                    edges.len()
                );
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
        for policy in POLICIES {
            let solve = |limit| with_budget(limit, |work| orient(&nodes, &links, policy, work));
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
