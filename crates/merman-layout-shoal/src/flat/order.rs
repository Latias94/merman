//! Crossing reduction on adjacent expanded ranks. Each distinct offset on a vertex face is
//! a port. Incidences on the same port cannot cross each other at that end.

use super::{Csr, Expanded};
use crate::work::{LayoutResult, Work, filled, reserved};
use crate::{LayoutError, NodeSpec, Options, WorkError};
use std::cmp::Ordering;
use std::ops::Range;

fn numeric_cmp(a: f64, b: f64) -> Ordering {
    if a == b {
        Ordering::Equal
    } else {
        a.total_cmp(&b)
    }
}

/// The CSR order within a face and the local port number of a segment survive every sweep.
struct Face<'a> {
    start: Vec<usize>,
    segments: Vec<usize>,
    local: Vec<usize>,
    count: Vec<usize>,
    peer: &'a [u32],
    peer_offset: &'a [f64],
}

impl<'a> Face<'a> {
    fn new(g: &'a Expanded, incoming: bool, work: &mut Work<'_>) -> LayoutResult<Self> {
        let (csr, peer, offset, peer_offset): (&Csr, _, _, _) = if incoming {
            (&g.pred, &g.from, &g.target_offset, &g.source_offset)
        } else {
            (&g.succ, &g.to, &g.source_offset, &g.target_offset)
        };
        let n = g.rank.len();
        let m = g.from.len();
        work.charge(n)?;
        work.charge(m)?;
        let mut face = Self {
            start: reserved(n.checked_add(1).ok_or(WorkError::ArithmeticOverflow)?)?,
            segments: reserved(m)?,
            local: filled(m, 0)?,
            count: filled(n, 0)?,
            peer,
            peer_offset,
        };
        for v in 0..n {
            let first = face.segments.len();
            face.start.push(first);
            let incident = csr.at(v as u32);
            work.visit(incident.len() + 1)?;
            for &e in incident {
                let e = e as usize;
                // A capped, unexpanded long edge has no attachment on the intervening ranks.
                if g.rank[g.to[e] as usize].checked_sub(g.rank[g.from[e] as usize]) == Some(1) {
                    face.segments.push(e);
                }
            }
            let edges = &mut face.segments[first..];
            if let Some(&head) = edges.first() {
                let mut count = 1;
                if edges.iter().any(|&e| offset[e] != offset[head]) {
                    work.sort(edges, |&a, &b| numeric_cmp(offset[a], offset[b]))?;
                    let mut previous = offset[edges[0]];
                    for &e in edges.iter() {
                        if offset[e] != previous {
                            count += 1;
                        }
                        face.local[e] = count - 1;
                        previous = offset[e];
                    }
                }
                face.count[v] = count;
            }
        }
        face.start.push(face.segments.len());
        Ok(face)
    }

    fn range(&self, v: u32) -> Range<usize> {
        let v = v as usize;
        self.start[v]..self.start[v + 1]
    }

    fn at(&self, v: u32) -> &[usize] {
        &self.segments[self.range(v)]
    }
}

struct Ports<'a> {
    source: Face<'a>,
    target: Face<'a>,
}

#[derive(Clone, Copy, Default)]
struct Place {
    pos: usize,
    source: usize,
    target: usize,
}

/// Rank-contiguous snapshots let an attempt restore the entire order with one copy. The input
/// graph is published only after all work charges and cancellation checks succeed.
struct Order {
    start: Vec<usize>,
    ids: Vec<u32>,
    place: Vec<Place>,
}

impl Order {
    fn range(&self, rank: usize) -> Range<usize> {
        self.start[rank]..self.start[rank + 1]
    }

    fn ranks(&self) -> usize {
        self.start.len() - 1
    }

    fn reindex(&mut self, rank: usize, ports: &Ports<'_>, work: &mut Work<'_>) -> LayoutResult<()> {
        let row = self.range(rank);
        work.visit(row.len() + 1)?;
        let (mut source, mut target) = (0, 0);
        for (pos, &id) in self.ids[row].iter().enumerate() {
            let v = id as usize;
            self.place[v] = Place {
                pos,
                source,
                target,
            };
            source += ports.source.count[v];
            target += ports.target.count[v];
        }
        Ok(())
    }

    fn restore(&mut self, ids: &[u32], ports: &Ports<'_>, work: &mut Work<'_>) -> LayoutResult<()> {
        work.charge(ids.len())?;
        self.ids.copy_from_slice(ids);
        for r in 0..self.ranks() {
            self.reindex(r, ports, work)?;
        }
        Ok(())
    }

    fn swap(&mut self, i: usize, ports: &Ports<'_>) {
        let a = self.ids[i] as usize;
        let b = self.ids[i + 1] as usize;
        let left = self.place[a];
        self.place[b] = left;
        self.place[a] = Place {
            pos: left.pos + 1,
            source: left.source + ports.source.count[b],
            target: left.target + ports.target.count[b],
        };
        self.ids.swap(i, i + 1);
    }
}

fn initial_order(
    g: &Expanded,
    nodes: &[NodeSpec],
    ports: &Ports<'_>,
    work: &mut Work<'_>,
) -> LayoutResult<(Order, Vec<usize>)> {
    let n = g.rank.len();
    work.charge(n)?;
    let mut start = reserved(
        g.layers
            .len()
            .checked_add(1)
            .ok_or(WorkError::ArithmeticOverflow)?,
    )?;
    start.push(0);
    for row in &g.layers {
        start.push(start.last().copied().unwrap() + row.len());
    }
    let mut next = reserved(g.layers.len())?;
    next.extend_from_slice(&start[..g.layers.len()]);
    let mut order = Order {
        start,
        ids: filled(n, 0)?,
        place: filled(n, Place::default())?,
    };
    let mut seen = filled(n, false)?;
    let mut roots = reserved(g.layers.iter().map(Vec::len).max().unwrap_or(0))?;
    let mut stack = reserved(n)?;
    let declared = |&v: &usize| (nodes.get(v).map_or(u32::MAX, |node| node.seq), v);
    // Visit disconnected sources on later ranks too. Sorting each set of children by sequence
    // makes the walk independent of the segment insertion order.
    for row in &g.layers {
        roots.clear();
        roots.extend(row.iter().map(|&v| v as usize).filter(|&v| !seen[v]));
        work.sort(&mut roots, |a, b| declared(a).cmp(&declared(b)))?;
        for &root in &roots {
            stack.push(root);
            while let Some(v) = stack.pop() {
                work.visit(1)?;
                if std::mem::replace(&mut seen[v], true) {
                    continue;
                }
                let dest = &mut next[g.rank[v] as usize];
                order.ids[*dest] = v as u32;
                *dest += 1;
                let first = stack.len();
                let edges = ports.source.at(v as u32);
                work.visit(edges.len())?;
                stack
                    .try_reserve(edges.len())
                    .map_err(|_| LayoutError::AllocationLimit)?;
                stack.extend(
                    edges
                        .iter()
                        .map(|&e| g.to[e] as usize)
                        .filter(|&v| !seen[v]),
                );
                work.sort(&mut stack[first..], |a, b| declared(b).cmp(&declared(a)))?;
            }
        }
    }

    let preference = |v: usize| {
        let node = &nodes[v];
        node.order_hint
            .map_or(f64::from(node.seq), |hint| hint.cross_key)
    };
    let compare = |a: usize, b: usize| {
        numeric_cmp(preference(a), preference(b)).then(nodes[a].seq.cmp(&nodes[b].seq))
    };
    let mut preference_slot = filled(nodes.len(), 0)?;
    for r in 0..order.ranks() {
        let range = order.range(r);
        work.visit(range.len() + 1)?;
        roots.clear();
        roots.extend(
            order.ids[range.clone()]
                .iter()
                .map(|&v| v as usize)
                .filter(|&v| v < nodes.len()),
        );
        work.sort(&mut roots, |&a, &b| compare(a, b).then(a.cmp(&b)))?;
        let mut slot = 0;
        for (i, &v) in roots.iter().enumerate() {
            if i > 0 && compare(roots[i - 1], v) != Ordering::Equal {
                slot += 1;
            }
            preference_slot[v] = slot;
        }
        if !roots.is_empty() && roots.iter().all(|&v| nodes[v].order_hint.is_some()) {
            let real_slots = order.ids[range]
                .iter_mut()
                .filter(|v| (**v as usize) < nodes.len());
            for (at, &v) in real_slots.zip(&roots) {
                *at = v as u32;
            }
        }
        order.reindex(r, ports, work)?;
    }
    Ok((order, preference_slot))
}

#[derive(Default)]
struct Fenwick {
    tree: Vec<u32>,
    seen: u32,
}

impl Fenwick {
    fn reset(&mut self, slots: usize, work: &mut Work<'_>) -> LayoutResult<()> {
        let size = slots
            .checked_next_power_of_two()
            .ok_or(WorkError::ArithmeticOverflow)?;
        work.charge(size)?;
        self.tree.clear();
        self.tree
            .try_reserve(size)
            .map_err(|_| LayoutError::AllocationLimit)?;
        self.tree.resize(size, 0);
        self.seen = 0;
        Ok(())
    }

    fn greater(&self, slot: usize) -> u64 {
        let mut count = self.seen;
        let mut i = slot + 1;
        while i > 0 {
            count -= self.tree[i - 1];
            i &= i - 1;
        }
        u64::from(count)
    }

    fn insert(&mut self, slot: usize) {
        self.seen += 1;
        let mut i = slot;
        while i < self.tree.len() {
            self.tree[i] += 1;
            i |= i + 1;
        }
    }

    fn charge_entries(&self, count: usize, work: &mut Work<'_>) -> LayoutResult<()> {
        let levels = self.tree.len().ilog2() as usize + 1;
        work.visit(
            count
                .checked_mul(2 * levels)
                .ok_or(WorkError::ArithmeticOverflow)?,
        )
    }
}

fn crossings(
    order: &Order,
    ports: &Ports<'_>,
    tree: &mut Fenwick,
    work: &mut Work<'_>,
) -> LayoutResult<u64> {
    let mut count = 0;
    for r in 1..order.ranks() {
        work.visit(1)?;
        let north = &order.ids[order.range(r - 1)];
        let south = &order.ids[order.range(r)];
        let (Some(&n), Some(&s)) = (north.last(), south.last()) else {
            continue;
        };
        let slots = order.place[s as usize].target + ports.target.count[s as usize];
        if slots < 2 || order.place[n as usize].source + ports.source.count[n as usize] < 2 {
            continue;
        }
        tree.reset(slots, work)?;
        let target =
            |e: usize| order.place[ports.source.peer[e] as usize].target + ports.target.local[e];
        for &v in north {
            let edges = ports.source.at(v);
            tree.charge_entries(edges.len(), work)?;
            // Batch queries before insertion: segments sharing the same source port never
            // contribute an inversion amongst themselves, whatever their destination order.
            for group in edges.chunk_by(|&a, &b| ports.source.local[a] == ports.source.local[b]) {
                for &e in group {
                    count += tree.greater(target(e));
                }
                for &e in group {
                    tree.insert(target(e));
                }
            }
        }
    }
    Ok(count)
}

fn inversions(
    order: &Order,
    preference: &[usize],
    tree: &mut Fenwick,
    work: &mut Work<'_>,
) -> LayoutResult<u64> {
    let mut total = 0;
    for r in 0..order.ranks() {
        let row = &order.ids[order.range(r)];
        work.visit(1)?;
        if row.len() < 2 {
            continue;
        }
        tree.reset(row.len(), work)?;
        tree.charge_entries(row.len(), work)?;
        for &v in row {
            if let Some(&slot) = preference.get(v as usize) {
                total += tree.greater(slot);
                tree.insert(slot);
            }
        }
    }
    Ok(total)
}

struct Sweep {
    mean: Vec<f64>,
    moving: Vec<usize>,
}

impl Sweep {
    fn run(
        &mut self,
        g: &Expanded,
        order: &mut Order,
        ports: &Ports<'_>,
        down: bool,
        right: bool,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        let face = if down { &ports.target } else { &ports.source };
        for step in 1..order.ranks() {
            let rank = if down { step } else { order.ranks() - 1 - step };
            let row = order.range(rank);
            work.visit(row.len() + 1)?;
            if row.len() < 2 {
                continue;
            }
            self.moving.clear();
            for &v in &order.ids[row.clone()] {
                let edges = face.at(v);
                if edges.is_empty() {
                    continue;
                }
                work.visit(edges.len())?;
                let mut sum = 0.0;
                let mut weights = 0.0;
                for &e in edges {
                    let u = face.peer[e] as usize;
                    let mut x = order.place[u].pos as f64;
                    if g.width[u] > 0.0 {
                        x += face.peer_offset[e] / g.width[u];
                    }
                    let weight = f64::from(g.weight[e]);
                    sum += weight * x;
                    weights += weight;
                }
                let mean = sum / weights;
                if !mean.is_finite() {
                    return Err(WorkError::ArithmeticOverflow.into());
                }
                self.mean[v as usize] = mean;
                self.moving.push(v as usize);
            }
            if self.moving.len() < 2 {
                continue;
            }
            work.sort(&mut self.moving, |&a, &b| {
                numeric_cmp(self.mean[a], self.mean[b]).then_with(|| {
                    let old = order.place[a].pos.cmp(&order.place[b].pos);
                    if right { old.reverse() } else { old }
                })
            })?;
            let slots = order.ids[row]
                .iter_mut()
                .filter(|v| !face.at(**v).is_empty());
            for (at, &v) in slots.zip(&self.moving) {
                *at = v as u32;
            }
            order.reindex(rank, ports, work)?;
        }
        Ok(())
    }
}

/// Strict inversion counts for both orders of an adjacent pair. Consume each sorted run once;
/// equal keys at a shared neighbor port contribute to neither orientation.
fn between(a: &[usize], b: &[usize]) -> (u64, u64) {
    let (mut before, mut after) = (0, 0);
    let mut j = 0;
    for run in a.chunk_by(|x, y| x == y) {
        let key = run[0];
        while j < b.len() && b[j] < key {
            j += 1;
        }
        before += run.len() as u64 * j as u64;
        while j < b.len() && b[j] == key {
            j += 1;
        }
        after += run.len() as u64 * (b.len() - j) as u64;
    }
    (before, after)
}

/// One flat slice per incidence list. All keys for a rank depend only on the opposite rank's
/// order and fixed port offsets. Swaps inside the rank leave its own keys valid.
struct NeighborKeys {
    keys: Vec<usize>,
    valid: Vec<bool>,
}

impl NeighborKeys {
    fn new(segments: usize, ranks: usize) -> LayoutResult<Self> {
        Ok(Self {
            keys: filled(segments, 0)?,
            valid: filled(ranks, false)?,
        })
    }

    fn prepare(
        &mut self,
        rank: usize,
        order: &Order,
        ports: &Ports<'_>,
        incoming: bool,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        if self.valid[rank] {
            return Ok(());
        }
        let (own, opposite) = if incoming {
            (&ports.target, &ports.source)
        } else {
            (&ports.source, &ports.target)
        };
        for &v in &order.ids[order.range(rank)] {
            let edges = own.at(v);
            work.visit(1 + edges.len())?;
            let keys = &mut self.keys[own.range(v)];
            for (key, &e) in keys.iter_mut().zip(edges) {
                let place = order.place[own.peer[e] as usize];
                let first = if incoming { place.source } else { place.target };
                *key = first + opposite.local[e];
            }
            if !keys.is_sorted() {
                work.sort(keys, usize::cmp)?;
            }
        }
        self.valid[rank] = true;
        Ok(())
    }

    fn pair(
        &self,
        face: &Face<'_>,
        a: u32,
        b: u32,
        work: &mut Work<'_>,
    ) -> LayoutResult<(u64, u64)> {
        let a = &self.keys[face.range(a)];
        let b = &self.keys[face.range(b)];
        if a.is_empty() || b.is_empty() {
            return Ok((0, 0));
        }
        work.visit(a.len() + b.len())?;
        Ok(between(a, b))
    }
}

fn transpose(
    order: &mut Order,
    ports: &Ports<'_>,
    mut count: u64,
    work: &mut Work<'_>,
) -> LayoutResult<u64> {
    work.charge(ports.source.segments.len())?;
    work.charge(ports.target.segments.len())?;
    let mut up = NeighborKeys::new(ports.target.segments.len(), order.ranks())?;
    let mut down = NeighborKeys::new(ports.source.segments.len(), order.ranks())?;
    for _ in 0..8 {
        let mut changed = false;
        for r in 0..order.ranks() {
            let row = order.range(r);
            work.visit(row.len() + 1)?;
            if row.len() < 2 {
                continue;
            }
            up.prepare(r, order, ports, true, work)?;
            down.prepare(r, order, ports, false, work)?;
            let mut moved = false;
            for i in row.start..row.end - 1 {
                let (a, b) = (order.ids[i], order.ids[i + 1]);
                let (up_old, up_new) = up.pair(&ports.target, a, b, work)?;
                let (down_old, down_new) = down.pair(&ports.source, a, b, work)?;
                let (old, new) = (up_old + down_old, up_new + down_new);
                if old > new {
                    order.swap(i, ports);
                    count -= old - new;
                    if count == 0 {
                        return Ok(0);
                    }
                    moved = true;
                }
            }
            if moved {
                changed = true;
                if r > 0 {
                    down.valid[r - 1] = false;
                }
                if r + 1 < order.ranks() {
                    up.valid[r + 1] = false;
                }
            }
        }
        if !changed {
            break;
        }
    }
    Ok(count)
}

fn snapshot(ids: &[u32], work: &mut Work<'_>) -> LayoutResult<Vec<u32>> {
    work.charge(ids.len())?;
    let mut copy = reserved(ids.len())?;
    copy.extend_from_slice(ids);
    Ok(copy)
}

/// Minimize (crossings, real-node inversions) lexicographically. The second coordinate is
/// needed only for candidates that can still win on crossings.
fn minimize(
    g: &Expanded,
    nodes: &[NodeSpec],
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<(Vec<u32>, u64)> {
    let ports = Ports {
        source: Face::new(g, false, work)?,
        target: Face::new(g, true, work)?,
    };
    let (mut order, preference) = initial_order(g, nodes, &ports, work)?;
    let mut tree = Fenwick::default();
    let initial_count = crossings(&order, &ports, &mut tree, work)?;
    if initial_count == 0 {
        return Ok((order.ids, 0));
    }

    let initial_score = (
        initial_count,
        inversions(&order, &preference, &mut tree, work)?,
    );
    let initial = snapshot(&order.ids, work)?;
    let mut chosen = snapshot(&initial, work)?;
    let mut chosen_score = initial_score;
    let mut best = snapshot(&initial, work)?;
    let mut sweep = Sweep {
        mean: filled(order.ids.len(), 0.)?,
        moving: reserved(g.layers.iter().map(Vec::len).max().unwrap_or(0))?,
    };
    let small = order.ids.len() <= options.caps.max_transpose_nodes;
    for attempt in 0..if small { 2 } else { 1 } {
        if attempt != 0 {
            order.restore(&initial, &ports, work)?;
            work.charge(initial.len())?;
            best.copy_from_slice(&initial);
        }
        let mut best_score = initial_score;
        let mut stale = 0;
        for i in 0..options.caps.max_sweeps {
            let down = (i % 2) == attempt;
            sweep.run(g, &mut order, &ports, down, i % 4 >= 2, work)?;
            let c = crossings(&order, &ports, &mut tree, work)?;
            stale = if c < best_score.0 { 0 } else { stale + 1 };
            if c <= best_score.0 {
                let score = (c, inversions(&order, &preference, &mut tree, work)?);
                if score < best_score {
                    best_score = score;
                    work.charge(best.len())?;
                    best.copy_from_slice(&order.ids);
                }
            }
            if c == 0 || stale == 4 {
                break;
            }
        }
        if small && best_score.0 > 0 {
            order.restore(&best, &ports, work)?;
            let c = transpose(&mut order, &ports, best_score.0, work)?;
            if c < best_score.0 {
                best_score = (c, inversions(&order, &preference, &mut tree, work)?);
                work.charge(best.len())?;
                best.copy_from_slice(&order.ids);
            }
        }
        if best_score < chosen_score {
            chosen_score = best_score;
            std::mem::swap(&mut best, &mut chosen);
        }
        if chosen_score == (0, 0) {
            break;
        }
    }
    if chosen_score != (0, 0) {
        work.charge(chosen.len())?;
        order.ids.copy_from_slice(&chosen);
        for r in 0..order.ranks() {
            let row = order.range(r);
            order.ids[row].reverse();
            order.reindex(r, &ports, work)?;
        }
        // Port offsets retain their signs in a mirror, so crossings can change too.
        let c = crossings(&order, &ports, &mut tree, work)?;
        if c <= chosen_score.0
            && (c, inversions(&order, &preference, &mut tree, work)?) < chosen_score
        {
            return Ok((order.ids, c));
        }
    }
    Ok((chosen, chosen_score.0))
}

pub(super) fn reduce(
    g: &mut Expanded,
    nodes: &[NodeSpec],
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<u64> {
    work.charge(1)?;
    let (ids, crossings) = minimize(g, nodes, options, work)?;
    work.charge(ids.len())?;
    work.finish()?;
    let mut rest = ids.as_slice();
    for row in &mut g.layers {
        let (new, tail) = rest.split_at(row.len());
        row.copy_from_slice(new);
        rest = tail;
    }
    Ok(crossings)
}

#[cfg(test)]
mod tests;
