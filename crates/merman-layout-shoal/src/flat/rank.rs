//! Feasible integer ranks for a component's already oriented edges.

use super::{Csr, Link};
use crate::work::{LayoutResult, Work, filled, reserved};
use crate::{RankPolicy, WorkError};

struct Dag<'a> {
    arcs: &'a [Link],
    pred: Csr,
    succ: Csr,
}

pub(super) fn assign(
    n: usize,
    arcs: &[Link],
    topo: &[u32],
    policy: RankPolicy,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<u64>> {
    let dag = Dag {
        arcs,
        pred: Csr::new(n, arcs.iter().map(|e| e.to), work)?,
        succ: Csr::new(n, arcs.iter().map(|e| e.from), work)?,
    };
    work.charge(n)?;
    let mut rank = filled(n, 0_u64)?;
    for &v in topo {
        work.visit(1)?;
        for &id in dag.succ.at(v) {
            work.visit(1)?;
            let edge = &arcs[id as usize];
            let lower = rank[v as usize]
                .checked_add(u64::from(edge.span.max(1)))
                .ok_or(WorkError::ArithmeticOverflow)?;
            rank[edge.to as usize] = rank[edge.to as usize].max(lower);
        }
    }
    if arcs.is_empty() {
        return Ok(rank);
    }
    for pass in 0..8 {
        let mut changed = false;
        for i in 0..n {
            let v = topo[if pass % 2 == 0 { n - 1 - i } else { i }];
            let next = dag.tightened(v, &rank, work)?;
            changed |= rank[v as usize] != next;
            rank[v as usize] = next;
        }
        if !changed {
            break;
        }
    }
    if let RankPolicy::BoundedNetworkSimplex { max_pivots } = policy
        && max_pivots > 0
        && n.saturating_add(arcs.len()) <= 1000
    {
        simplex(&dag, &mut rank, max_pivots, work)?;
    }
    Ok(rank)
}

impl Dag<'_> {
    fn tightened(&self, v: u32, rank: &[u64], work: &mut Work<'_>) -> LayoutResult<u64> {
        work.visit(1)?;
        let current = rank[v as usize];
        let (mut low, mut high) = (0, u64::MAX);
        let (mut incoming, mut outgoing) = (0.0, 0.0);
        let (mut before, mut after) = (0.0, 0.0);
        for &id in self.pred.at(v) {
            work.visit(1)?;
            let e = &self.arcs[id as usize];
            let other = rank[e.from as usize];
            low = low.max(other + u64::from(e.span.max(1)));
            incoming += f64::from(e.weight);
            before += (current - other) as f64 * f64::from(e.weight);
        }
        for &id in self.succ.at(v) {
            work.visit(1)?;
            let e = &self.arcs[id as usize];
            let other = rank[e.to as usize];
            high = high.min(other - u64::from(e.span.max(1)));
            outgoing += f64::from(e.weight);
            after += (other - current) as f64 * f64::from(e.weight);
        }
        Ok(if outgoing > incoming {
            high
        } else if incoming > outgoing {
            low
        } else if incoming > 0.0 {
            // On a flat objective, balance the mean spans. Round toward the current rank so an
            // odd integral gap cannot oscillate between two equally good positions.
            let shift = (after - before) / (2.0 * incoming);
            if shift >= 0.0 {
                current.saturating_add(shift as u64).min(high)
            } else {
                current.saturating_sub((-shift) as u64).max(low)
            }
        } else {
            current
        })
    }
}

const NONE: usize = usize::MAX;

fn slack(edge: &Link, rank: &[i64]) -> i64 {
    rank[edge.to as usize] - rank[edge.from as usize] - i64::from(edge.span.max(1))
}

// A tight spanning forest is a feasible network-simplex basis. Every pivot removes one tree
// constraint, shifts its subtree to the next tight constraint, and installs that entering edge.
struct Basis {
    tree: Vec<bool>,
    parent: Vec<usize>,
    via: Vec<usize>,
    preorder: Vec<usize>,
    stack: Vec<usize>,
    start: Vec<usize>,
    size: Vec<usize>,
    gradient: Vec<f64>,
}

impl Basis {
    fn new(n: usize, m: usize, work: &mut Work<'_>) -> LayoutResult<Self> {
        work.charge(n * 7 + m)?; // Simplex is admitted only for n + m <= 1000.
        Ok(Self {
            tree: filled(m, false)?,
            parent: filled(n, NONE)?,
            via: filled(n, NONE)?,
            preorder: reserved(n)?,
            stack: reserved(n)?,
            start: filled(n, 0)?,
            size: filled(n, 0)?,
            gradient: filled(n, 0.0)?,
        })
    }

    fn make_feasible(
        &mut self,
        arcs: &[Link],
        rank: &mut [i64],
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        for root in 0..rank.len() {
            if self.parent[root] != NONE {
                continue;
            }
            self.parent[root] = root;
            self.preorder.clear();
            self.preorder.push(root);
            loop {
                let mut best: Option<(i64, usize, usize, i64)> = None;
                for (id, edge) in arcs.iter().enumerate() {
                    work.visit(1)?;
                    let (a, b) = (edge.from as usize, edge.to as usize);
                    let a_in = self.parent[a] != NONE;
                    if a_in == (self.parent[b] != NONE) {
                        continue;
                    }
                    let gap = slack(edge, rank);
                    if best.is_none_or(|(old, ..)| gap < old) {
                        best = Some((
                            gap,
                            id,
                            if a_in { b } else { a },
                            if a_in { gap } else { -gap },
                        ));
                        if gap == 0 {
                            break;
                        }
                    }
                }
                let Some((_, id, added, shift)) = best else {
                    break;
                };
                if shift != 0 {
                    for &v in &self.preorder {
                        work.visit(1)?;
                        rank[v] += shift;
                    }
                }
                self.tree[id] = true;
                self.parent[added] = root;
                self.preorder.push(added);
            }
        }
        Ok(())
    }

    /// Subtree sums of the node gradients give all tree cut derivatives in one traversal.
    fn leaving(
        &mut self,
        dag: &Dag<'_>,
        slope: &[f64],
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<(usize, usize, i64)>> {
        work.charge(slope.len())?;
        self.parent.fill(NONE);
        self.size.fill(1);
        self.gradient.copy_from_slice(slope);
        self.preorder.clear();
        for root in 0..slope.len() {
            if self.parent[root] != NONE {
                continue;
            }
            self.parent[root] = root;
            self.stack.push(root);
            while let Some(v) = self.stack.pop() {
                work.visit(1)?;
                self.start[v] = self.preorder.len();
                self.preorder.push(v);
                for &id in dag.pred.at(v as u32).iter().chain(dag.succ.at(v as u32)) {
                    work.visit(1)?;
                    let id = id as usize;
                    if !self.tree[id] {
                        continue;
                    }
                    let e = &dag.arcs[id];
                    let other = if e.from as usize == v { e.to } else { e.from } as usize;
                    if self.parent[other] == NONE {
                        self.parent[other] = v;
                        self.via[other] = id;
                        self.stack.push(other);
                    }
                }
            }
        }
        let mut leaving: Option<(usize, usize, i64)> = None;
        for &v in self.preorder.iter().rev() {
            work.visit(1)?;
            let parent = self.parent[v];
            if parent == v {
                continue;
            }
            let id = self.via[v];
            let sign = if dag.arcs[id].to as usize == v { 1 } else { -1 };
            if self.gradient[v] * (sign as f64) < 0.0 && leaving.is_none_or(|(old, ..)| id < old) {
                leaving = Some((id, v, sign));
            }
            self.size[parent] += self.size[v];
            self.gradient[parent] += self.gradient[v];
        }
        Ok(leaving)
    }

    fn pivot(
        &mut self,
        dag: &Dag<'_>,
        rank: &mut [i64],
        slope: &[f64],
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        let Some((leave, child, sign)) = self.leaving(dag, slope, work)? else {
            return Ok(false);
        };
        let subtree = self.start[child]..self.start[child] + self.size[child];
        let mut enter = None;
        for (id, e) in dag.arcs.iter().enumerate() {
            work.visit(1)?;
            let tail_in = subtree.contains(&self.start[e.from as usize]);
            let head_in = subtree.contains(&self.start[e.to as usize]);
            if tail_in == head_in || tail_in != (sign > 0) {
                continue;
            }
            let gap = slack(e, rank);
            if enter.is_none_or(|(old, _)| gap < old) {
                enter = Some((gap, id));
            }
        }
        let Some((gap, enter)) = enter else {
            return Ok(false);
        };
        for &v in &self.preorder[subtree] {
            work.visit(1)?;
            rank[v] += sign * gap;
        }
        self.tree[leave] = false;
        self.tree[enter] = true;
        Ok(true)
    }
}

fn simplex(
    dag: &Dag<'_>,
    rank: &mut [u64],
    max_pivots: u32,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let n = rank.len();
    work.charge(2 * n)?;
    // This small-graph policy bounds any tight path by (n-1) * u32::MAX, well within i64.
    let mut trial = reserved(n)?;
    trial.extend(rank.iter().map(|&r| r as i64));
    let mut slope = filled(n, 0.0)?;
    let mut baseline = 0.0;
    for e in dag.arcs {
        work.visit(1)?;
        let weight = f64::from(e.weight);
        baseline += weight * slack(e, &trial) as f64;
        slope[e.to as usize] += weight;
        slope[e.from as usize] -= weight;
    }
    if baseline == 0.0 {
        return Ok(());
    }
    let mut basis = Basis::new(n, dag.arcs.len(), work)?;
    basis.make_feasible(dag.arcs, &mut trial, work)?;
    for _ in 0..max_pivots {
        if !basis.pivot(dag, &mut trial, &slope, work)? {
            break;
        }
    }
    let mut score = 0.0;
    for e in dag.arcs {
        work.visit(1)?;
        score += f64::from(e.weight) * slack(e, &trial) as f64;
    }
    if score < baseline {
        work.charge(n)?;
        let origin = trial.iter().copied().min().unwrap_or(0);
        for (out, value) in rank.iter_mut().zip(trial) {
            *out = (value - origin) as u64;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NoopWorkControl;

    fn edge(from: u32, to: u32, span: u32, weight: u32) -> Link {
        Link {
            from,
            to,
            edge: 0,
            span,
            weight: weight as f32,
        }
    }

    fn run(n: usize, arcs: &[Link], policy: RankPolicy) -> Vec<u64> {
        let topo = (0..n as u32).collect::<Vec<_>>();
        assign(n, arcs, &topo, policy, &mut Work::new(&mut NoopWorkControl)).unwrap()
    }

    fn objective(arcs: &[Link], ranks: &[u64]) -> u64 {
        arcs.iter()
            .map(|e| {
                let length = ranks[e.to as usize]
                    .checked_sub(ranks[e.from as usize])
                    .unwrap();
                assert!(length >= u64::from(e.span.max(1)), "{e:?} {ranks:?}");
                length * e.weight as u64
            })
            .sum()
    }

    #[test]
    fn simplex_can_move_a_tight_block_that_no_single_vertex_can_improve() {
        let arcs = [edge(0, 3, 4, 1), edge(1, 2, 1, 10), edge(2, 3, 1, 1)];
        let initial = run(4, &arcs, RankPolicy::TightenedLongestPath);
        assert_eq!(initial, [0, 0, 1, 4]);
        assert_eq!(
            initial,
            run(
                4,
                &arcs,
                RankPolicy::BoundedNetworkSimplex { max_pivots: 0 }
            )
        );
        let optimal = run(
            4,
            &arcs,
            RankPolicy::BoundedNetworkSimplex { max_pivots: 10 },
        );
        assert_eq!(optimal, [0, 2, 3, 4]);
        assert!(objective(&arcs, &optimal) < objective(&arcs, &initial));
    }

    #[test]
    fn negative_cut_exchanges_a_tight_tree_edge_for_the_minimum_slack_edge() {
        let arcs = [edge(0, 1, 1, 1), edge(0, 2, 3, 1), edge(1, 2, 1, 10)];
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        let dag = Dag {
            arcs: &arcs,
            pred: Csr::new(3, arcs.iter().map(|e| e.to), &mut work).unwrap(),
            succ: Csr::new(3, arcs.iter().map(|e| e.from), &mut work).unwrap(),
        };
        let mut basis = Basis::new(3, 3, &mut work).unwrap();
        basis.tree.copy_from_slice(&[true, true, false]);
        let mut ranks = [0, 1, 3];
        let slope = [-2.0, -9.0, 11.0];
        assert!(basis.pivot(&dag, &mut ranks, &slope, &mut work).unwrap());
        assert_eq!(ranks, [0, 2, 3]);
        assert_eq!(basis.tree, [false, true, true]);
        assert!(!basis.pivot(&dag, &mut ranks, &slope, &mut work).unwrap());
    }

    #[test]
    fn fractional_weights_move_a_tight_block_across_two_maximum_spans() {
        let mut arcs = [
            edge(0, 1, u32::MAX, 1),
            edge(1, 4, u32::MAX, 1),
            edge(2, 3, 1, 1),
            edge(3, 4, 1, 1),
            edge(5, 6, 7, 1),
        ];
        for (arc, weight) in arcs.iter_mut().zip([0.03125, 0.25, 0.75, 0.125, 0.5]) {
            arc.weight = weight;
        }
        let ranks = run(
            7,
            &arcs,
            RankPolicy::BoundedNetworkSimplex { max_pivots: 8 },
        );
        let span = u64::from(u32::MAX);
        assert_eq!(ranks[4] - ranks[0], 2 * span);
        for arc in arcs {
            assert_eq!(
                ranks[arc.to as usize] - ranks[arc.from as usize],
                u64::from(arc.span)
            );
        }
    }

    // A connected tight-tree optimum has span at most (n-1) times the largest minimum length.
    // Translation lets the same enumeration cover disconnected graphs and isolated vertices.
    fn exhaustive(arcs: &[Link], r: &mut [u64], v: usize, ceiling: u64) -> u64 {
        if v == r.len() {
            return objective(arcs, r);
        }
        let lo = arcs
            .iter()
            .filter(|e| e.to as usize == v)
            .map(|e| r[e.from as usize] + u64::from(e.span.max(1)))
            .max()
            .unwrap_or(0);
        (lo..=ceiling)
            .map(|x| {
                r[v] = x;
                exhaustive(arcs, r, v + 1, ceiling)
            })
            .min()
            .unwrap_or(u64::MAX)
    }

    #[test]
    fn bounded_simplex_matches_exhaustive_integer_rankings_on_small_random_dags() {
        let mut state = 20261123_u32;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state
        };
        for n in 2..=5 {
            for _ in 0..60 {
                let mut arcs = Vec::new();
                for a in 0..n as u32 {
                    for b in a + 1..n as u32 {
                        if next() % 3 != 0 {
                            arcs.push(edge(a, b, next() % 3, 1 + next() % 9));
                        }
                        if next() % 9 == 0 {
                            arcs.push(edge(a, b, 1 + next() % 2, 1 + next() % 9));
                        }
                    }
                }
                let best = exhaustive(&arcs, &mut vec![0; n], 0, 2 * (n as u64 - 1));
                let tight = objective(&arcs, &run(n, &arcs, RankPolicy::TightenedLongestPath));
                for pivots in [1, 64] {
                    let rank = run(
                        n,
                        &arcs,
                        RankPolicy::BoundedNetworkSimplex { max_pivots: pivots },
                    );
                    let cost = objective(&arcs, &rank);
                    assert!(cost <= tight && cost >= best, "{arcs:?} {rank:?}");
                    if pivots == 64 {
                        assert_eq!(cost, best, "{arcs:?} {rank:?}");
                    }
                }
            }
        }
    }
}
