//! Rank in source occurrence order, including the provisional border-port ranks.

use std::cmp::Ordering;
use std::collections::HashSet;

use super::{
    Adjacency, CompoundError, LevelPlan, NodeKind, Result, Work, add, check_slots, inconsistent,
};

const MAX_DUMMIES: u64 = 20_000;

pub(super) struct RankPlan {
    pub ranks: Vec<usize>,
    pub edges: Vec<RankedEdge>,
    pub step: i32,
    pub max_rank: usize,
    pub dummy_count: usize,
    pub segment_count: usize,
}

#[derive(Clone, Copy)]
pub(super) struct RankedEdge {
    tail: usize,
    head: usize,
    len: i32,
    pub reversed: bool,
    pub span: usize,
    pub direct: bool,
}

impl RankedEdge {
    pub fn rank_endpoints(self) -> (usize, usize) {
        if self.reversed {
            (self.head, self.tail)
        } else {
            (self.tail, self.head)
        }
    }
}

pub(super) fn step<E>(plan: LevelPlan<'_>, work: &mut Work<'_, E>) -> Result<i32, E> {
    let mut seen = HashSet::new();
    for e in plan.edges {
        work.price(1)?;
        let a = e.tail.min(e.head);
        let b = e.tail.max(e.head);
        // The source key has two Number operations and can round above 2^53.
        let key = (a as f64 * plan.nodes.len() as f64 + b as f64).to_bits();
        if e.label.width > 0.0 || seen.contains(&key) {
            return Ok(2);
        }
        if seen.len() == seen.capacity() {
            work.price(seen.len())?;
            seen.try_reserve(1)
                .map_err(|_| CompoundError::AllocationFailed)?;
        }
        seen.insert(key);
    }
    Ok(1)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Visit {
    Unseen,
    Open,
    Finished,
}

fn orient<E>(real: usize, edges: &mut [RankedEdge], work: &mut Work<'_, E>) -> Result<(), E> {
    // Capture the source outgoing adjacency before DFS turns feedback edges around.
    let outgoing = Adjacency::build(
        real,
        edges.len(),
        |i| (!edges[i].reversed).then_some(edges[i].tail),
        work,
    )?;
    let mut state = work.filled(real, Visit::Unseen)?;
    let mut stack = Vec::new();
    work.price(real)?;
    for root in 0..real {
        work.poll(root)?;
        if state[root] != Visit::Unseen {
            continue;
        }
        state[root] = Visit::Open;
        work.push(&mut stack, (root, 0usize))?;
        while let Some((node, next)) = stack.last_mut() {
            work.price(1)?;
            if let Some(&i) = outgoing.at(*node).get(*next) {
                *next += 1;
                let edge = &mut edges[i];
                match state[edge.head] {
                    Visit::Open => edge.reversed = true,
                    Visit::Unseen => {
                        state[edge.head] = Visit::Open;
                        work.push(&mut stack, (edge.head, 0))?;
                    }
                    Visit::Finished => {}
                }
            } else {
                state[*node] = Visit::Finished;
                stack.pop();
            }
        }
    }
    Ok(())
}

pub(super) fn rank<E>(
    plan: LevelPlan<'_>,
    step: i32,
    work: &mut Work<'_, E>,
) -> Result<RankPlan, E> {
    let real = plan.nodes.len();
    let m = plan.edges.len();
    let mut edges = work.reserve(m)?;
    work.price(m)?;
    for (i, e) in plan.edges.iter().enumerate() {
        work.poll(i)?;
        edges.push(RankedEdge {
            tail: e.tail,
            head: e.head,
            len: i32::try_from(e.min_len.max(1))
                .ok()
                .and_then(|len| len.checked_mul(step))
                .ok_or(CompoundError::ArithmeticOverflow)?,
            reversed: plan.nodes[e.head].kind == NodeKind::StartPort
                || plan.nodes[e.tail].kind == NodeKind::EndPort,
            span: 0,
            direct: false,
        });
    }
    orient(real, &mut edges, work)?;
    let outgoing = Adjacency::build(real, m, |i| Some(edges[i].rank_endpoints().0), work)?;
    let incoming = Adjacency::build(real, m, |i| Some(edges[i].rank_endpoints().1), work)?;
    let mut intermediate = work.map(plan.nodes, |n| {
        if n.kind == NodeKind::StartPort {
            -step
        } else {
            0
        }
    })?;
    let mut indegree = work.reserve(real)?;
    let mut topo = work.reserve(real)?;
    work.price(real)?;
    for node in 0..real {
        work.poll(node)?;
        let degree = incoming.at(node).len();
        indegree.push(degree);
        if degree == 0 {
            topo.push(node);
        }
    }
    work.price(real)?;
    let mut cursor = 0;
    while let Some(&node) = topo.get(cursor) {
        work.poll(cursor)?;
        cursor += 1;
        let list = outgoing.at(node);
        work.price(list.len())?;
        for (i, &slot) in list.iter().enumerate() {
            work.poll(i)?;
            let edge = edges[slot];
            let (_, target) = edge.rank_endpoints();
            let next = intermediate[node]
                .checked_add(edge.len)
                .ok_or(CompoundError::ArithmeticOverflow)?;
            intermediate[target] = intermediate[target].max(next);
            indegree[target] -= 1;
            if indegree[target] == 0 {
                topo.push(target);
            }
        }
    }
    if topo.len() != real {
        return Err(inconsistent("local orientation retained a cycle"));
    }

    for _ in 0..8 {
        let mut changed = false;
        work.price(real)?;
        for (i, &node) in topo.iter().rev().enumerate() {
            work.poll(i)?;
            let ins = incoming.at(node);
            let outs = outgoing.at(node);
            let mut lo = i32::MIN;
            let mut hi = i32::MAX;
            work.price(ins.len())?;
            for (j, &slot) in ins.iter().enumerate() {
                work.poll(j)?;
                let edge = edges[slot];
                let (source, _) = edge.rank_endpoints();
                let bound = intermediate[source]
                    .checked_add(edge.len)
                    .ok_or(CompoundError::ArithmeticOverflow)?;
                lo = lo.max(bound);
            }
            work.price(outs.len())?;
            for (j, &slot) in outs.iter().enumerate() {
                work.poll(j)?;
                let edge = edges[slot];
                let (_, target) = edge.rank_endpoints();
                let bound = intermediate[target]
                    .checked_sub(edge.len)
                    .ok_or(CompoundError::ArithmeticOverflow)?;
                hi = hi.min(bound);
            }
            let next = match outs.len().cmp(&ins.len()) {
                Ordering::Greater => hi,
                Ordering::Less => lo,
                Ordering::Equal => intermediate[node],
            };
            if next != intermediate[node] {
                intermediate[node] = next;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let mut start_ports = false;
    let mut normal_range = None::<(i32, i32)>;
    work.price(real)?;
    for (i, node) in plan.nodes.iter().enumerate() {
        work.poll(i)?;
        match node.kind {
            NodeKind::StartPort => start_ports = true,
            NodeKind::EndPort => {}
            NodeKind::Node => {
                let r = intermediate[i];
                normal_range = Some(normal_range.map_or((r, r), |(a, b)| (a.min(r), b.max(r))));
            }
        }
    }
    let (min, max) = normal_range.unwrap_or_default();
    let base = min
        .checked_sub(if start_ports { step } else { 0 })
        .ok_or(CompoundError::ArithmeticOverflow)?;
    let mut max_rank = 0;
    let mut ranks = work.reserve(real)?;
    work.price(real)?;
    for (i, node) in plan.nodes.iter().enumerate() {
        work.poll(i)?;
        let r = match node.kind {
            NodeKind::StartPort => base,
            NodeKind::EndPort => max
                .checked_add(step)
                .ok_or(CompoundError::ArithmeticOverflow)?,
            NodeKind::Node => intermediate[i],
        };
        let rank = r
            .checked_sub(base)
            .and_then(|n| usize::try_from(n).ok())
            .ok_or(CompoundError::ArithmeticOverflow)?;
        max_rank = max_rank.max(rank);
        ranks.push(rank);
    }
    Ok(RankPlan {
        ranks,
        edges,
        step,
        max_rank,
        dummy_count: 0,
        segment_count: 0,
    })
}

impl RankPlan {
    pub fn plan_expansion<E>(&mut self, work: &mut Work<'_, E>) -> Result<(), E> {
        let mut dummies = 0u64;
        let mut long_count = 0;
        work.price(self.edges.len())?;
        for (i, edge) in self.edges.iter_mut().enumerate() {
            work.poll(i)?;
            let (a, b) = edge.rank_endpoints();
            edge.span = self.ranks[b]
                .checked_sub(self.ranks[a])
                .filter(|&s| s > 0)
                .ok_or(inconsistent("edge does not increase solved rank"))?;
            if edge.span > 1 {
                // Counts and spans are each at most signed 32-bit.
                dummies += (edge.span - 1) as u64;
                long_count += 1;
            }
        }
        if dummies > MAX_DUMMIES {
            let mut longest = work.reserve(long_count)?;
            work.price(self.edges.len())?;
            for (i, edge) in self.edges.iter().enumerate() {
                work.poll(i)?;
                if edge.span > 1 {
                    longest.push(i);
                }
            }
            work.sort(&mut longest, |a, b| {
                self.edges[b].span.cmp(&self.edges[a].span).then(a.cmp(&b))
            })?;
            for i in longest {
                if dummies <= MAX_DUMMIES {
                    break;
                }
                work.price(1)?;
                self.edges[i].direct = true;
                dummies -= (self.edges[i].span - 1) as u64;
            }
        }
        self.dummy_count = dummies as usize;
        check_slots(add(self.ranks.len(), self.dummy_count)?)?;
        work.price(self.edges.len())?;
        for (i, edge) in self.edges.iter().enumerate() {
            work.poll(i)?;
            if !edge.direct {
                self.segment_count = add(self.segment_count, edge.span)?;
            }
        }
        check_slots(self.segment_count)
    }
}
