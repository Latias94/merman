//! Crossing reduction on adjacent expanded ranks. Each distinct offset on a vertex face is
//! a port. Incidences on the same port cannot cross each other at that end.

mod feedback;
mod groups;

use super::{Component, Csr, Expanded, Kind, validate};
use crate::scratch::{capacity_bytes, clear_reserve};
use crate::work::{LayoutResult, Work, filled, reserved};
use crate::{GroupId, LayoutError, NodeSpec, Options, OrderHint, WorkError};
use groups::Groups;
use std::cmp::Ordering;
use std::ops::Range;

pub(super) use feedback::{FeedbackLane, favor_feedback};

fn numeric_cmp(a: f64, b: f64) -> Ordering {
    if a == b {
        Ordering::Equal
    } else {
        a.total_cmp(&b)
    }
}

fn synthetic(node: &NodeSpec) -> bool {
    node.boundary.is_some() || node.group_proxy
}

fn stateless_hint(node: &NodeSpec) -> Option<OrderHint> {
    if node.editor_order_hint().is_some() {
        node.internal_order_hint
    } else {
        node.effective_order_hint()
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
) -> LayoutResult<(Order, Vec<usize>, bool)> {
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
        stateless_hint(node).map_or(f64::from(node.seq), |hint| hint.cross_key)
    };
    let compare = |a: usize, b: usize| {
        numeric_cmp(preference(a), preference(b)).then(nodes[a].seq.cmp(&nodes[b].seq))
    };
    let mut preference_slot = filled(nodes.len(), 0)?;
    let mut has_editor_hint = false;
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
            has_editor_hint |= nodes[v].editor_order_hint().is_some();
            if i > 0 && compare(roots[i - 1], v) != Ordering::Equal {
                slot += 1;
            }
            preference_slot[v] = slot;
        }
        if !roots.is_empty() && roots.iter().all(|&v| stateless_hint(&nodes[v]).is_some()) {
            let real_slots = order.ids[range]
                .iter_mut()
                .filter(|v| (**v as usize) < nodes.len());
            for (at, &v) in real_slots.zip(&roots) {
                *at = v as u32;
            }
        }
        order.reindex(r, ports, work)?;
    }
    Ok((order, preference_slot, has_editor_hint))
}

#[derive(Debug, Default)]
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

const NO_PREFERENCE: usize = usize::MAX;

fn inversions<const EDITOR: bool>(
    ids: &[u32],
    starts: &[usize],
    preference: &[usize],
    tree: &mut Fenwick,
    work: &mut Work<'_>,
) -> LayoutResult<u64> {
    let mut total = 0;
    for bounds in starts.windows(2) {
        let row = &ids[bounds[0]..bounds[1]];
        work.visit(1)?;
        if row.len() < 2 {
            continue;
        }
        tree.reset(row.len(), work)?;
        tree.charge_entries(row.len(), work)?;
        for &v in row {
            if let Some(&slot) = preference.get(v as usize)
                && (!EDITOR || slot != NO_PREFERENCE)
            {
                total += tree.greater(slot);
                tree.insert(slot);
            }
        }
    }
    Ok(total)
}

struct EditorSeed {
    ids: Vec<u32>,
    preference: Vec<usize>,
    anchored: bool,
}

fn editor_seed(
    stateless: &[u32],
    order: &Order,
    nodes: &[NodeSpec],
    rules: &Rules<'_>,
    work: &mut Work<'_>,
) -> LayoutResult<Option<EditorSeed>> {
    work.charge(nodes.len())?;
    let mut preference = filled(nodes.len(), NO_PREFERENCE)?;
    let anchored = !rules.pins.is_empty() || !rules.lanes.is_empty() || nodes.iter().any(synthetic);
    let mut ids = snapshot(stateless, work)?;
    let mut survivors = reserved(
        order
            .start
            .windows(2)
            .map(|row| row[1] - row[0])
            .max()
            .unwrap_or(0),
    )?;
    let hint = |vertex: usize| nodes.get(vertex).and_then(NodeSpec::editor_order_hint);
    let cross = |vertex: usize| hint(vertex).unwrap().cross_key;
    let adjacent = |&left: &u32, &right: &u32| {
        let (left, right) = (left as usize, right as usize);
        !anchored
            || !nodes.get(left).is_some_and(synthetic)
                && !nodes.get(right).is_some_and(synthetic)
                && rules.pin(left) == rules.pin(right)
                && rules.lane(left) == rules.lane(right)
                && rules
                    .groups
                    .as_ref()
                    .is_none_or(|groups| groups.owner[left] == groups.owner[right])
    };
    let mut comparable = false;
    for rank in 0..order.ranks() {
        let row = order.range(rank);
        work.visit(row.len() + 1)?;
        if row.len() < 2 || rules.fixed(rank) {
            continue;
        }
        let mut offset = 0;
        for run in ids[row].chunk_by_mut(adjacent) {
            let first = offset;
            offset += run.len();
            if run.len() < 2 {
                continue;
            }
            survivors.clear();
            survivors.extend(
                run.iter()
                    .map(|&vertex| vertex as usize)
                    .filter(|&vertex| hint(vertex).is_some()),
            );
            if survivors.len() < 2 {
                continue;
            }
            work.sort(&mut survivors, |&left, &right| {
                numeric_cmp(cross(left), cross(right))
                    .then(nodes[left].seq.cmp(&nodes[right].seq))
                    .then(left.cmp(&right))
            })?;
            let mut slot = first;
            for (index, &vertex) in survivors.iter().enumerate() {
                if index > 0 && cross(survivors[index - 1]) != cross(vertex) {
                    slot += 1;
                }
                preference[vertex] = slot;
            }
            if slot == first {
                continue;
            }
            comparable = true;
            let slots = run.iter_mut().filter(|at| hint(**at as usize).is_some());
            for (at, &vertex) in slots.zip(&survivors) {
                *at = vertex as u32;
            }
        }
    }
    Ok(comparable.then_some(EditorSeed {
        ids,
        preference,
        anchored,
    }))
}

#[derive(Debug, Default)]
struct Sweep {
    mean: Vec<f64>,
    moving: Vec<usize>,
}

#[derive(Default)]
struct Rules<'a> {
    pins: &'a [NodeSpec],
    lanes: &'a [usize],
    fixed: [Option<usize>; 2],
    groups: Option<Groups<'a>>,
}

impl Rules<'_> {
    fn pin(&self, v: usize) -> i8 {
        self.pins.get(v).map_or(0, |n| n.pin)
    }

    fn lane(&self, v: usize) -> usize {
        self.lanes.get(v).copied().unwrap_or_default()
    }

    fn fixed(&self, rank: usize) -> bool {
        self.fixed.contains(&Some(rank))
    }

    fn can_mirror<const LIMITED: bool>(&self) -> bool {
        !LIMITED || self.pins.is_empty() && self.lanes.is_empty() && self.fixed == [None; 2]
    }

    fn initial(
        &mut self,
        order: &mut Order,
        ports: &Ports<'_>,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        for r in 0..order.ranks() {
            let range = order.range(r);
            let row = &mut order.ids[range];
            if row.len() < 2 || self.fixed(r) {
                continue;
            }
            if !self.pins.is_empty() || !self.lanes.is_empty() {
                validate::charge_sort(row.len(), work)?;
                row.sort_unstable_by_key(|&v| {
                    let v = v as usize;
                    (self.lane(v), self.pin(v), order.place[v].pos)
                });
                order.reindex(r, ports, work)?;
            }
            if let Some(groups) = &mut self.groups {
                let range = order.range(r);
                groups.arrange(
                    &mut order.ids[range],
                    &order.place,
                    |v| order.place[v].pos as f64,
                    work,
                )?;
                order.reindex(r, ports, work)?;
            }
        }
        Ok(())
    }
}

impl Sweep {
    fn run<const LIMITED: bool>(
        &mut self,
        g: &Expanded,
        order: &mut Order,
        ports: &Ports<'_>,
        (down, right): (bool, bool),
        rules: &mut Rules<'_>,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        let face = if down { &ports.target } else { &ports.source };
        for step in 1..order.ranks() {
            let rank = if down { step } else { order.ranks() - 1 - step };
            let row = order.range(rank);
            work.visit(row.len() + 1)?;
            if row.len() < 2 || LIMITED && rules.fixed(rank) {
                continue;
            }
            self.moving.clear();
            for &v in &order.ids[row.clone()] {
                let edges = face.at(v);
                if edges.is_empty() {
                    if LIMITED && rules.groups.is_some() {
                        let across = order.range(if down { rank - 1 } else { rank + 1 }).len();
                        self.mean[v as usize] =
                            order.place[v as usize].pos as f64 * across as f64 / row.len() as f64;
                    }
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
            if LIMITED && let Some(groups) = &mut rules.groups {
                groups.arrange(&mut order.ids[row], &order.place, |v| self.mean[v], work)?;
                order.reindex(rank, ports, work)?;
                continue;
            }
            if self.moving.len() < 2 {
                continue;
            }
            work.sort(&mut self.moving, |&a, &b| {
                let constraint = if LIMITED {
                    (rules.lane(a), rules.pin(a)).cmp(&(rules.lane(b), rules.pin(b)))
                } else {
                    Ordering::Equal
                };
                constraint
                    .then_with(|| numeric_cmp(self.mean[a], self.mean[b]))
                    .then_with(|| {
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
#[derive(Debug, Default)]
struct NeighborKeys {
    keys: Vec<usize>,
    valid: Vec<bool>,
}

impl NeighborKeys {
    #[cfg(test)]
    fn new(segments: usize, ranks: usize) -> LayoutResult<Self> {
        let mut result = Self::default();
        result.reset(segments, ranks)?;
        Ok(result)
    }

    fn reset(&mut self, segments: usize, ranks: usize) -> LayoutResult<()> {
        clear_reserve(&mut self.keys, segments)?;
        self.keys.resize(segments, 0);
        clear_reserve(&mut self.valid, ranks)?;
        self.valid.resize(ranks, false);
        Ok(())
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

fn transpose_with_keys<const LIMITED: bool>(
    order: &mut Order,
    ports: &Ports<'_>,
    mut count: u64,
    rules: &Rules<'_>,
    [up, down]: &mut [NeighborKeys; 2],
    work: &mut Work<'_>,
) -> LayoutResult<u64> {
    work.charge(ports.source.segments.len())?;
    work.charge(ports.target.segments.len())?;
    up.reset(ports.target.segments.len(), order.ranks())?;
    down.reset(ports.source.segments.len(), order.ranks())?;
    for _ in 0..8 {
        let mut changed = false;
        for r in 0..order.ranks() {
            let row = order.range(r);
            work.visit(row.len() + 1)?;
            if row.len() < 2 || LIMITED && rules.fixed(r) {
                continue;
            }
            up.prepare(r, order, ports, true, work)?;
            down.prepare(r, order, ports, false, work)?;
            let mut moved = false;
            for i in row.start..row.end - 1 {
                let (a, b) = (order.ids[i], order.ids[i + 1]);
                if LIMITED
                    && (rules.pin(a as usize) != rules.pin(b as usize)
                        || rules.lane(a as usize) != rules.lane(b as usize)
                        || rules.groups.as_ref().is_some_and(|groups| {
                            groups.owner[a as usize] != groups.owner[b as usize]
                        }))
                {
                    continue;
                }
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

#[cfg(test)]
fn transpose<const LIMITED: bool>(
    order: &mut Order,
    ports: &Ports<'_>,
    count: u64,
    rules: &Rules<'_>,
    work: &mut Work<'_>,
) -> LayoutResult<u64> {
    transpose_with_keys::<LIMITED>(order, ports, count, rules, &mut Default::default(), work)
}

fn snapshot(ids: &[u32], work: &mut Work<'_>) -> LayoutResult<Vec<u32>> {
    let mut copy = Vec::new();
    save_order(ids, &mut copy, work)?;
    Ok(copy)
}

fn save_order(ids: &[u32], copy: &mut Vec<u32>, work: &mut Work<'_>) -> LayoutResult<()> {
    work.charge(ids.len())?;
    clear_reserve(copy, ids.len())?;
    copy.extend_from_slice(ids);
    Ok(())
}

type Score = (u64, u64);

fn improved_mirror<const EDITOR: bool>(
    order: &mut Order,
    ports: &Ports<'_>,
    ids: &[u32],
    score: Score,
    preference: &[usize],
    tree: &mut Fenwick,
    work: &mut Work<'_>,
) -> LayoutResult<Option<Score>> {
    if order.ids.len() == ids.len() {
        work.charge(ids.len())?;
        order.ids.copy_from_slice(ids);
    } else {
        order.ids = snapshot(ids, work)?;
    }
    for rank in 0..order.ranks() {
        let row = order.range(rank);
        order.ids[row].reverse();
        order.reindex(rank, ports, work)?;
    }
    let count = crossings(order, ports, tree, work)?;
    if count <= score.0 {
        let candidate = (
            count,
            inversions::<EDITOR>(&order.ids, &order.start, preference, tree, work)?,
        );
        if candidate < score {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}

#[derive(Debug, Default)]
pub(super) struct OrderScratch {
    fenwick: Fenwick,
    initial: Vec<u32>,
    best: Vec<u32>,
    sweep: Sweep,
    neighbors: [NeighborKeys; 2],
}

impl OrderScratch {
    pub(super) fn retained_bytes(&self) -> usize {
        capacity_bytes(&self.fenwick.tree)
            + capacity_bytes(&self.initial)
            + capacity_bytes(&self.best)
            + capacity_bytes(&self.sweep.mean)
            + capacity_bytes(&self.sweep.moving)
            + self
                .neighbors
                .iter()
                .map(|keys| capacity_bytes(&keys.keys) + capacity_bytes(&keys.valid))
                .sum::<usize>()
    }
}

fn search<const LIMITED: bool, const EDITOR: bool>(
    (g, options): (&Expanded, &Options),
    order: &mut Order,
    preference: &[usize],
    ports: &Ports<'_>,
    rules: &mut Rules<'_>,
    scratch: &mut OrderScratch,
    work: &mut Work<'_>,
) -> LayoutResult<(Vec<u32>, Score)> {
    let OrderScratch {
        fenwick: tree,
        initial,
        best,
        sweep,
        neighbors,
    } = scratch;
    let initial_count = crossings(order, ports, tree, work)?;
    if !EDITOR && initial_count == 0 {
        return Ok((std::mem::take(&mut order.ids), (0, 0)));
    }

    let initial_score = (
        initial_count,
        inversions::<EDITOR>(&order.ids, &order.start, preference, tree, work)?,
    );
    if initial_score == (0, 0) {
        return Ok((std::mem::take(&mut order.ids), initial_score));
    }
    save_order(&order.ids, initial, work)?;
    let mut chosen = snapshot(initial, work)?;
    let mut chosen_score = initial_score;
    save_order(initial, best, work)?;
    clear_reserve(&mut sweep.mean, order.ids.len())?;
    sweep.mean.resize(order.ids.len(), 0.);
    clear_reserve(
        &mut sweep.moving,
        g.layers.iter().map(Vec::len).max().unwrap_or(0),
    )?;
    let small = order.ids.len() <= options.caps.max_transpose_nodes;
    for attempt in 0..if small { 2 } else { 1 } {
        if attempt != 0 {
            order.restore(initial, ports, work)?;
            work.charge(initial.len())?;
            best.copy_from_slice(initial);
        }
        let mut best_score = initial_score;
        let mut stale = 0;
        for i in 0..options.caps.max_sweeps {
            let down = (i % 2) == attempt;
            sweep.run::<LIMITED>(g, order, ports, (down, i % 4 >= 2), rules, work)?;
            let c = crossings(order, ports, tree, work)?;
            stale = if c < best_score.0 { 0 } else { stale + 1 };
            if c <= best_score.0 {
                let score = (
                    c,
                    inversions::<EDITOR>(&order.ids, &order.start, preference, tree, work)?,
                );
                if score < best_score {
                    best_score = score;
                    work.charge(best.len())?;
                    best.copy_from_slice(&order.ids);
                }
            }
            if c == 0 && (!EDITOR || best_score.1 == 0) || stale == 4 {
                break;
            }
        }
        if small && best_score.0 > 0 {
            order.restore(best, ports, work)?;
            let c =
                transpose_with_keys::<LIMITED>(order, ports, best_score.0, rules, neighbors, work)?;
            if c < best_score.0 {
                best_score = (
                    c,
                    inversions::<EDITOR>(&order.ids, &order.start, preference, tree, work)?,
                );
                work.charge(best.len())?;
                best.copy_from_slice(&order.ids);
            }
        }
        if best_score < chosen_score {
            chosen_score = best_score;
            std::mem::swap(best, &mut chosen);
        }
        if chosen_score == (0, 0) {
            break;
        }
    }
    if chosen_score != (0, 0)
        && rules.can_mirror::<LIMITED>()
        && let Some(score) =
            improved_mirror::<EDITOR>(order, ports, &chosen, chosen_score, preference, tree, work)?
    {
        return Ok((std::mem::take(&mut order.ids), score));
    }
    Ok((chosen, chosen_score))
}

fn minimize<const LIMITED: bool>(
    g: &Expanded,
    nodes: &[NodeSpec],
    options: &Options,
    rules: &mut Rules<'_>,
    scratch: &mut OrderScratch,
    work: &mut Work<'_>,
) -> LayoutResult<(Vec<u32>, u64)> {
    let ports = Ports {
        source: Face::new(g, false, work)?,
        target: Face::new(g, true, work)?,
    };
    let (mut order, declaration, has_editor_hint) = initial_order(g, nodes, &ports, work)?;
    if LIMITED {
        rules.initial(&mut order, &ports, work)?;
    }
    let (stateless, (count, _)) = search::<LIMITED, false>(
        (g, options),
        &mut order,
        &declaration,
        &ports,
        rules,
        scratch,
        work,
    )?;
    if !has_editor_hint {
        return Ok((stateless, count));
    }
    let Some(seed) = editor_seed(&stateless, &order, nodes, rules, work)? else {
        return Ok((stateless, count));
    };

    let stateless_score = (
        count,
        inversions::<true>(
            &stateless,
            &order.start,
            &seed.preference,
            &mut scratch.fenwick,
            work,
        )?,
    );
    if stateless_score == (0, 0) {
        return Ok((stateless, 0));
    }

    order.ids = seed.ids;
    for rank in 0..order.ranks() {
        order.reindex(rank, &ports, work)?;
    }
    if seed.anchored {
        let candidate_count = crossings(&order, &ports, &mut scratch.fenwick, work)?;
        if candidate_count <= count
            && (
                candidate_count,
                inversions::<true>(
                    &order.ids,
                    &order.start,
                    &seed.preference,
                    &mut scratch.fenwick,
                    work,
                )?,
            ) < stateless_score
        {
            work.editor_choice = true;
            return Ok((order.ids, candidate_count));
        }
        return Ok((stateless, count));
    }
    if LIMITED {
        rules.initial(&mut order, &ports, work)?;
    }
    let (hinted, hint_score) = search::<LIMITED, true>(
        (g, options),
        &mut order,
        &seed.preference,
        &ports,
        rules,
        scratch,
        work,
    )?;
    let score = stateless_score.min(hint_score);
    if score.1 > 0
        && rules.can_mirror::<LIMITED>()
        && let Some(mirror) = improved_mirror::<true>(
            &mut order,
            &ports,
            &stateless,
            score,
            &seed.preference,
            &mut scratch.fenwick,
            work,
        )?
    {
        work.editor_choice = true;
        return Ok((std::mem::take(&mut order.ids), mirror.0));
    }
    if hint_score < stateless_score {
        work.editor_choice = true;
        Ok((hinted, hint_score.0))
    } else {
        Ok((stateless, stateless_score.0))
    }
}

pub(super) fn reduce(
    g: &mut Expanded,
    nodes: &[NodeSpec],
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<u64> {
    publish::<false>(g, nodes, options, &mut Rules::default(), work)
}

type Membership<'a> = (&'a [Option<GroupId>], &'a [Option<GroupId>]);

pub(super) fn constrained(
    g: &mut Expanded,
    c: &Component,
    membership: Option<Membership<'_>>,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<u64> {
    let mut rules = Rules {
        pins: if c.pinned { &c.nodes } else { &[] },
        groups: membership
            .map(|(of, parents)| Groups::new(g, of, parents, work))
            .transpose()?,
        ..Rules::default()
    };
    if c.boundaries && !g.layers.is_empty() {
        for (side, r, kind) in [
            (0, 0, Kind::StartPort),
            (1, g.layers.len() - 1, Kind::EndPort),
        ] {
            let row = &g.layers[r];
            work.charge(row.len())?;
            if row.iter().all(|&v| {
                g.kind[v as usize] == kind && c.nodes[v as usize].effective_order_hint().is_some()
            }) {
                rules.fixed[side] = Some(r);
            }
        }
    }
    publish::<true>(g, &c.nodes, options, &mut rules, work)
}

pub(super) fn constrained_lanes(
    g: &mut Expanded,
    c: &Component,
    lanes: &[usize],
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<u64> {
    let mut rules = Rules {
        pins: if c.pinned { &c.nodes } else { &[] },
        lanes,
        ..Rules::default()
    };
    publish::<true>(g, &c.nodes, options, &mut rules, work)
}

fn publish<const LIMITED: bool>(
    g: &mut Expanded,
    nodes: &[NodeSpec],
    options: &Options,
    rules: &mut Rules<'_>,
    work: &mut Work<'_>,
) -> LayoutResult<u64> {
    work.charge(1)?;
    let mut scratch = std::mem::take(&mut g.order_scratch);
    let result = minimize::<LIMITED>(g, nodes, options, rules, &mut scratch, work);
    // Return storage before propagating errors so the outer frame can apply its cap.
    g.order_scratch = scratch;
    let (ids, crossings) = result?;
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
