//! Cross-rank coordinate polish after compaction. All moves respect the original rank order,
//! asymmetric extents and component envelope. Port offsets are relative to their own vertex.

use super::{
    Component, Expanded, Kind, acyclic::Orientation, position::extents, to_canonical_port,
};
use crate::work::{LayoutResult, Work, filled, reserved};
use crate::{Direction, Graph, LayoutError, Options, PortSideHint, RoutingStyle, Side, WorkError};

mod mental_map;

const INLINE: f64 = 0.5;
const COLLINEAR: f64 = 0.01;
const NODE_SNAP: f64 = 12.;
const BEND_SNAP: f64 = 24.;
const MICRO_DRIFT: f64 = 2.;
const LEAN_CLEAR: f64 = 40.;
const MIN_BOUNDARY_PITCH: f64 = 1.;
const TOP: usize = 0;
const BOTTOM: usize = 1;

fn copy<T: Copy>(values: &[T]) -> LayoutResult<Vec<T>> {
    let mut out = reserved(values.len())?;
    out.extend_from_slice(values);
    Ok(out)
}

#[derive(Clone, Copy)]
struct Interval {
    lo: f64,
    hi: f64,
}

impl Interval {
    const ALL: Self = Self {
        lo: f64::NEG_INFINITY,
        hi: f64::INFINITY,
    };
    fn intersect(self, other: Self) -> Self {
        Self {
            lo: self.lo.max(other.lo),
            hi: self.hi.min(other.hi),
        }
    }
    fn closest(self, x: f64) -> Option<f64> {
        (self.lo <= self.hi).then(|| x.clamp(self.lo, self.hi))
    }
    fn contains(self, x: f64) -> bool {
        self.lo <= x && x <= self.hi
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Leaves,
    Arrives,
    Held,
}

fn incident(g: &Expanded, v: usize, side: usize) -> &[u32] {
    if side == TOP {
        g.pred.at(v as u32)
    } else {
        g.succ.at(v as u32)
    }
}

fn vertex(g: &Expanded, e: usize, side: usize) -> usize {
    if side == TOP {
        g.to[e] as usize
    } else {
        g.from[e] as usize
    }
}

struct Geometry<'a> {
    g: &'a Expanded,
    columns: Option<super::lanes::Columns<'a>>,
    x: Vec<f64>,
    dx: [Vec<f64>; 2],
    pos: Vec<usize>,
    extent: Vec<(f64, f64)>,
    envelope: Interval,
    bare: Vec<bool>,
    labels_only: Vec<bool>,
    span: Vec<f64>,
    free: Vec<[usize; 2]>,
    role: [Vec<Role>; 2],
    // For an incidence, the opposite end of its original chain and the first attachment
    // beyond consecutive bend-only ranks. Neither lookup depends on the route's length.
    terminal: [Vec<usize>; 2],
    follow: [Vec<usize>; 2],
    direct_end: Vec<[bool; 2]>,
    twin: Vec<usize>,
    protected: Vec<bool>,
    keep: Vec<bool>,
    has_position_hints: bool,
}

impl<'a> Geometry<'a> {
    fn new(
        g: &'a Expanded,
        c: &Component,
        orientation: &Orientation,
        graph: &Graph,
        options: &Options,
        x: &[f64],
        work: &mut Work<'_>,
    ) -> LayoutResult<Self> {
        let n = x.len();
        let m = g.from.len();
        work.charge(n.checked_mul(12).ok_or(WorkError::ArithmeticOverflow)?)?;
        work.charge(m.checked_mul(8).ok_or(WorkError::ArithmeticOverflow)?)?;
        let mut s = Self {
            g,
            columns: None,
            x: copy(x)?,
            dx: [copy(&g.target_offset)?, copy(&g.source_offset)?],
            pos: filled(n, 0)?,
            extent: reserved(n)?,
            envelope: Interval {
                lo: f64::INFINITY,
                hi: f64::NEG_INFINITY,
            },
            bare: reserved(g.layers.len())?,
            labels_only: reserved(g.layers.len())?,
            span: filled(n, 0.)?,
            free: filled(n, [0; 2])?,
            role: [filled(m, Role::Arrives)?, filled(m, Role::Leaves)?],
            terminal: [filled(m, 0)?, filled(m, 0)?],
            follow: [filled(m, 0)?, filled(m, 0)?],
            direct_end: filled(n, [false; 2])?,
            twin: filled(n, END)?,
            protected: filled(n, false)?,
            keep: filled(n, true)?,
            has_position_hints: false,
        };
        for (v, &center) in x.iter().enumerate() {
            let (left, right) = extents(g, c, options, v as u32);
            s.extent.push((left, right));
            s.envelope.lo = s.envelope.lo.min(center - left);
            s.envelope.hi = s.envelope.hi.max(center + right);
            if let Some(node) = c.nodes.get(v) {
                s.has_position_hints |= c.nodes.len() > 1
                    && !s.has_position_hints
                    && node.position_hint_in(graph.direction, g.rank[v]).is_some();
                s.span[v] = node
                    .port_span
                    .unwrap_or((g.width[v] - 2. * options.spacing.corner_clearance).max(0.))
                    .min(g.width[v]);
            }
        }
        for row in &g.layers {
            s.bare
                .push(row.iter().all(|&v| g.kind[v as usize] == Kind::Dummy));
            s.labels_only.push(
                row.iter().all(|&v| g.kind[v as usize].interior())
                    && row.iter().any(|&v| g.kind[v as usize] == Kind::Label),
            );
            for (i, &v) in row.iter().enumerate() {
                s.pos[v as usize] = i;
            }
        }
        let mut start = 0;
        let mut chain_ids = reserved(g.chains.len())?;
        for (i, chain) in g.chains.iter().enumerate() {
            work.visit(chain.len())?;
            if g.direct[i] {
                s.direct_end[chain[0] as usize][BOTTOM] = true;
                s.direct_end[*chain.last().unwrap() as usize][TOP] = true;
                continue;
            }
            let last = start + chain.len() - 2;
            let reversed = orientation.reversed[i];
            let spec = &graph.edges[c.links[i].edge as usize];
            let (src, dst) = if reversed {
                (spec.target_port, spec.source_port)
            } else {
                (spec.source_port, spec.target_port)
            };
            for e in start..=last {
                s.terminal[TOP][e] = start;
                s.terminal[BOTTOM][e] = last;
                s.follow[TOP][e] = if e > start && s.bare[g.rank[g.from[e] as usize] as usize] {
                    s.follow[TOP][e - 1]
                } else {
                    e
                };
                if reversed {
                    s.role[TOP][e] = Role::Leaves;
                    s.role[BOTTOM][e] = Role::Arrives;
                }
            }
            for e in (start..=last).rev() {
                s.follow[BOTTOM][e] = if e < last && s.bare[g.rank[g.to[e] as usize] as usize] {
                    s.follow[BOTTOM][e + 1]
                } else {
                    e
                };
            }
            for (side, e, hint) in [(BOTTOM, start, src), (TOP, last, dst)] {
                let v = vertex(g, e, side);
                let anchor = matches!(graph.direction, Direction::Left | Direction::Right)
                    .then_some(c.nodes[v].anchor_y)
                    .flatten()
                    .map(|p| p - g.width[v] * 0.5);
                let fixed = match to_canonical_port(graph.direction, hint) {
                    PortSideHint::Auto => anchor,
                    PortSideHint::FixedOffset {
                        side: Side::Top | Side::Bottom,
                        offset,
                    } => Some(offset),
                    // The transverse coordinate of a side stub is the face itself. Its offset
                    // along the flow remains the router's (possibly signed) side parameter.
                    PortSideHint::FixedOffset { .. } | PortSideHint::Side(_) => Some(s.dx[side][e]),
                };
                if let Some(offset) = fixed {
                    s.role[side][e] = Role::Held;
                    s.dx[side][e] = offset;
                } else {
                    s.free[v][side] += 1;
                }
            }
            if chain.len() > 2 {
                chain_ids.push(i);
            }
            start = last + 1;
        }
        let ends = |i: usize| {
            let chain = &g.chains[i];
            (chain[0], chain[chain.len() - 1])
        };
        work.sort(&mut chain_ids, |&a, &b| ends(a).cmp(&ends(b)))?;
        for twins in chain_ids
            .chunk_by(|&a, &b| ends(a) == ends(b))
            .filter(|ids| ids.len() > 1)
        {
            if g.separate_ports(ends(twins[0]), work)? {
                continue;
            }
            for &i in twins {
                let chain = &g.chains[i];
                for &v in &chain[1..chain.len() - 1] {
                    s.twin[v as usize] = twins[0];
                }
            }
        }
        Ok(s)
    }

    fn at(&self, e: usize, side: usize) -> f64 {
        self.x[vertex(self.g, e, side)] + self.dx[side][e]
    }

    fn same_lane(&self, a: usize, b: usize) -> bool {
        self.columns
            .is_none_or(|columns| columns.owners[a] == columns.owners[b])
    }

    fn room(&self, v: usize, same: impl Fn(usize) -> bool) -> Interval {
        self.room_with_slack(v, same, 0.0)
    }

    fn room_with_slack(&self, v: usize, same: impl Fn(usize) -> bool, node_slack: f64) -> Interval {
        let row = &self.g.layers[self.g.rank[v] as usize];
        let i = self.pos[v];
        let (l, r) = self.extent[v];
        let slack = |neighbor| {
            if self.g.kind[neighbor] == Kind::Node {
                node_slack
            } else {
                0.0
            }
        };
        let [left, right] = self
            .columns
            .map_or([self.envelope.lo, self.envelope.hi], |columns| {
                columns.content[columns.owners[v]]
            });
        let mut lo = left + l;
        let mut hi = right - r;
        if i > 0 {
            let p = row[i - 1] as usize;
            if !same(p) {
                lo = lo.max(self.x[p] + (self.extent[p].1 - slack(p)) + l);
            }
        }
        if i + 1 < row.len() {
            let q = row[i + 1] as usize;
            if !same(q) {
                hi = hi.min(self.x[q] - (self.extent[q].0 - slack(q)) - r);
            }
        }
        Interval { lo, hi }
    }

    fn movable(&self, v: usize) -> Interval {
        if self.protected[v] {
            Interval {
                lo: self.x[v],
                hi: self.x[v],
            }
        } else {
            self.room(v, |_| false)
        }
    }

    fn free_bend(&self, v: usize) -> bool {
        self.g.kind[v] == Kind::Dummy && !self.protected[v] && self.twin[v] == END
    }
}

// Weighted union by size with explicit member lists. Checking a candidate translation visits
// only that block and its immediate rank neighbors, including on ranks with multiple members.
struct Blocks {
    root: Vec<usize>,
    next: Vec<usize>,
    tail: Vec<usize>,
    size: Vec<usize>,
    solid: Vec<bool>,
}
const END: usize = usize::MAX;

impl Blocks {
    fn new(g: &Expanded) -> LayoutResult<Self> {
        let n = g.rank.len();
        let mut root = reserved(n)?;
        root.extend(0..n);
        let tail = copy(&root)?;
        let mut solid = reserved(n)?;
        solid.extend(g.kind.iter().map(|k| *k != Kind::Dummy));
        Ok(Self {
            root,
            next: filled(n, END)?,
            tail,
            size: filled(n, 1)?,
            solid,
        })
    }
    fn members(&self, b: usize) -> impl Iterator<Item = usize> + '_ {
        std::iter::successors(Some(b), |&v| (self.next[v] != END).then_some(self.next[v]))
    }
    fn join(&mut self, a: usize, b: usize, work: &mut Work<'_>) -> LayoutResult<()> {
        let (mut a, mut b) = (self.root[a], self.root[b]);
        if a == b {
            return Ok(());
        }
        if self.size[a] < self.size[b] {
            std::mem::swap(&mut a, &mut b);
        }
        work.visit(self.size[b])?;
        let mut v = b;
        while v != END {
            self.root[v] = a;
            v = self.next[v];
        }
        self.next[self.tail[a]] = b;
        self.tail[a] = self.tail[b];
        self.size[a] += self.size[b];
        self.solid[a] |= self.solid[b];
        Ok(())
    }
    fn allowed(&self, s: &Geometry<'_>, b: usize, work: &mut Work<'_>) -> LayoutResult<Interval> {
        let mut interval = Interval::ALL;
        work.visit(self.size[b])?;
        for v in self.members(b) {
            if s.protected[v] {
                return Ok(Interval { lo: 0., hi: 0. });
            }
            let room = s.room(v, |u| self.root[u] == b);
            interval = interval.intersect(Interval {
                lo: room.lo - s.x[v],
                hi: room.hi - s.x[v],
            });
        }
        Ok(interval)
    }
}

#[derive(Clone, Copy)]
struct Fan {
    left: usize,
    right: usize,
    private: bool,
}

fn fan(
    s: &Geometry<'_>,
    v: usize,
    side: usize,
    next_real: &[[Option<usize>; 2]],
    seen: &mut [usize],
    stamp: usize,
    work: &mut Work<'_>,
) -> LayoutResult<Option<Fan>> {
    let g = s.g;
    let other = 1 - side;
    let edges = incident(g, v, side);
    work.visit(edges.len())?;
    if edges.len() < 2 || s.direct_end[v][side] {
        return Ok(None);
    }
    let mut first = usize::MAX;
    let mut last = 0;
    let mut fan = Fan {
        left: 0,
        right: 0,
        private: true,
    };
    for &e in edges {
        let e = s.terminal[side][e as usize];
        let u = vertex(g, e, other);
        let i = s.pos[u];
        if Some(g.rank[u] as usize) != next_real[g.rank[v] as usize][side] || seen[u] == stamp {
            return Ok(None);
        }
        seen[u] = stamp;
        fan.private &= incident(g, u, other).len() == 1 && !s.direct_end[u][other];
        if i < first {
            first = i;
            fan.left = e;
        }
        if i >= last {
            last = i;
            fan.right = e;
        }
    }
    Ok((last - first + 1 == edges.len() && (fan.private || edges.len() == 2)).then_some(fan))
}

fn center(
    s: &mut Geometry<'_>,
    blocks: &Blocks,
    v: usize,
    side: usize,
    fan: Fan,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    if s.protected[v] {
        return Ok(());
    }
    let other = 1 - side;
    let a = vertex(s.g, fan.left, other);
    let b = vertex(s.g, fan.right, other);
    let target = s.at(fan.left, other).midpoint(s.at(fan.right, other));
    let block = blocks.root[v];
    if blocks.root[a] == block || blocks.root[b] == block {
        return Ok(());
    }
    let desired = target - s.x[v];
    let Some(shift) = blocks.allowed(s, block, work)?.closest(desired) else {
        return Ok(());
    };
    if shift != 0. {
        for member in blocks.members(block) {
            let opposite = incident(s.g, member, other);
            work.visit(opposite.len())?;
            for &segment in opposite {
                let segment = segment as usize;
                let peer = vertex(s.g, segment, side);
                if blocks.root[peer] != block {
                    let offset = s.at(segment, other) - s.at(segment, side);
                    if offset.abs() < INLINE && (offset + shift).abs() >= INLINE {
                        return Ok(());
                    }
                }
            }
        }
    }
    for member in blocks.members(block) {
        s.x[member] += shift;
        s.protected[member] = true;
    }
    for (branch, segment) in [(a, fan.left), (b, fan.right)] {
        s.protected[branch] = true;
        if shift == desired && s.free[branch][other] > 1 && s.role[other][segment] != Role::Held {
            s.role[other][segment] = Role::Held;
            s.free[branch][other] -= 1;
        }
    }
    Ok(())
}

fn center_forks(s: &mut Geometry<'_>, real: usize, work: &mut Work<'_>) -> LayoutResult<()> {
    let g = s.g;
    let mut nearest = filled(g.layers.len(), [None; 2])?;
    for side in [TOP, BOTTOM] {
        let mut prev = None;
        for i in 0..g.layers.len() {
            let r = if side == TOP {
                i
            } else {
                g.layers.len() - 1 - i
            };
            nearest[r][side] = prev;
            if g.layers[r].iter().any(|&v| (v as usize) < real) {
                prev = Some(r);
            }
        }
    }
    let mut blocks = Blocks::new(g)?;
    for &source in g.layers.iter().flatten() {
        work.visit(1)?;
        let [segment] = g.succ.at(source) else {
            continue;
        };
        let segment = *segment as usize;
        let target = g.to[segment];
        if g.pred.at(target).len() == 1
            && !s.direct_end[source as usize][BOTTOM]
            && !s.direct_end[target as usize][TOP]
            && (s.at(segment, TOP) - s.at(segment, BOTTOM)).abs() < INLINE
        {
            blocks.join(source as usize, target as usize, work)?;
        }
    }
    let mut fans = filled(real, [None; 2])?;
    let mut seen = filled(real, END)?;
    for (v, f) in fans.iter_mut().enumerate() {
        for side in [TOP, BOTTOM] {
            f[side] = fan(s, v, side, &nearest, &mut seen, v * 2 + side, work)?;
        }
    }
    // Resolve a complete private diamond before any overlapping one-sided fork can move or
    // lock its vertices. Both endpoints then use the very same outer branch centers.
    for (v, f) in fans.iter().enumerate() {
        let Some(fork) = f[BOTTOM].filter(|f| f.private) else {
            continue;
        };
        let mut join = None;
        let mut paired = true;
        for &e in g.succ.at(v as u32) {
            work.visit(1)?;
            let u = vertex(g, s.terminal[BOTTOM][e as usize], TOP);
            let [out] = g.succ.at(u as u32) else {
                paired = false;
                break;
            };
            let to = vertex(g, s.terminal[BOTTOM][*out as usize], TOP);
            if s.direct_end[u][BOTTOM] || join.is_some_and(|j| j != to) {
                paired = false;
                break;
            }
            join = Some(to);
        }
        if paired
            && let Some(j) = join
            && let Some(back) = fans[j][TOP].filter(|f| f.private)
            && incident(g, j, TOP).len() == incident(g, v, BOTTOM).len()
        {
            center(s, &blocks, v, BOTTOM, fork, work)?;
            center(s, &blocks, j, TOP, back, work)?;
        }
    }
    for (v, f) in fans.iter().enumerate() {
        // A vertex with competing fans keeps its existing spine. A paired diamond has already
        // had first choice; ordinary private forks/joins and binary decision branches follow.
        if let [Some(fan), None] = *f {
            center(s, &blocks, v, TOP, fan, work)?;
        }
        if let [None, Some(fan)] = *f {
            center(s, &blocks, v, BOTTOM, fan, work)?;
        }
    }
    Ok(())
}

fn choose(current: f64, a: (f64, usize), b: (f64, usize), room: Interval) -> Option<f64> {
    let (first, second) =
        if a.1 < b.1 || (a.1 == b.1 && (a.0 - current).abs() <= (b.0 - current).abs()) {
            (a.0, b.0)
        } else {
            (b.0, a.0)
        };
    if room.contains(first) {
        Some(first)
    } else if room.contains(second) {
        Some(second)
    } else {
        room.closest(first)
    }
}

fn align_labels(s: &mut Geometry<'_>, work: &mut Work<'_>) -> LayoutResult<()> {
    let g = s.g;
    let endpoint =
        |s: &Geometry<'_>, e: usize, side: usize| (s.at(e, side), s.free[vertex(g, e, side)][side]);
    for (i, chain) in g.chains.iter().enumerate() {
        if g.direct[i] || chain.len() < 3 {
            continue;
        }
        let inner = &chain[1..chain.len() - 1];
        let placed = |v: u32| !s.bare[g.rank[v as usize] as usize];
        let Some(&first) = inner.iter().find(|&&v| placed(v)) else {
            continue;
        };
        work.visit(chain.len())?;
        let a = endpoint(s, g.pred.at(inner[0])[0] as usize, BOTTOM);
        let b = endpoint(s, g.succ.at(*inner.last().unwrap())[0] as usize, TOP);
        let shared = inner
            .iter()
            .copied()
            .filter(|&v| placed(v))
            .fold(Interval::ALL, |room, v| {
                room.intersect(s.movable(v as usize))
            });
        let common = choose(s.x[first as usize], a, b, shared);
        for &v in inner {
            if !placed(v) {
                continue;
            }
            let id = v as usize;
            let target = common.or_else(|| {
                let up = s.follow[TOP][g.pred.at(v)[0] as usize];
                let down = s.follow[BOTTOM][g.succ.at(v)[0] as usize];
                choose(
                    s.x[id],
                    endpoint(s, up, BOTTOM),
                    endpoint(s, down, TOP),
                    s.movable(id),
                )
            });
            if let Some(target) = target {
                s.x[id] = target;
            }
        }
    }
    Ok(())
}

struct Pool {
    mean: f64,
    count: usize,
    weight: f64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ZeroGap {
    Equal,
    Ordered,
}

fn pava(
    values: &mut [f64],
    gap: &[f64],
    bounds: Interval,
    zero_gap: ZeroGap,
    pools: &mut Vec<Pool>,
) -> bool {
    weighted_pava(values, gap, bounds, zero_gap, pools, |_| 1.)
}

/// Subtract cumulative separations to turn ordered coordinates into an isotonic sequence.
/// The transformed bounds are common to all pools.
fn weighted_pava(
    values: &mut [f64],
    gap: &[f64],
    bounds: Interval,
    zero_gap: ZeroGap,
    pools: &mut Vec<Pool>,
    weight: impl Fn(usize) -> f64,
) -> bool {
    let total = gap.iter().skip(1).sum::<f64>();
    let hi = bounds.hi - total;
    if bounds.lo > hi {
        return false;
    }
    pools.clear();
    let mut before = 0.;
    for (i, &target) in values.iter().enumerate() {
        if i > 0 {
            before += gap[i];
        }
        pools.push(Pool {
            mean: target - before,
            count: 1,
            weight: weight(i),
        });
        let mut together = zero_gap == ZeroGap::Equal && i > 0 && gap[i] == 0.;
        while pools.len() > 1 {
            let last = pools.len() - 1;
            if !together && pools[last - 1].mean <= pools[last].mean {
                break;
            }
            let right = pools.pop().unwrap();
            let left = pools.last_mut().unwrap();
            left.count += right.count;
            left.weight += right.weight;
            left.mean += (right.mean - left.mean) * (right.weight / left.weight);
            together = false;
        }
    }
    before = 0.;
    let mut i = 0;
    for pool in pools.iter() {
        let level = pool.mean.clamp(bounds.lo, hi);
        for _ in 0..pool.count {
            if i > 0 {
                before += gap[i];
            }
            values[i] = level + before;
            i += 1;
        }
    }
    true
}

struct PortRun {
    v: usize,
    side: usize,
    edges: Vec<usize>,
    gap: Vec<f64>,
    bounds: Interval,
}

fn port_runs(
    s: &Geometry<'_>,
    real: usize,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<PortRun>> {
    let mut runs = reserved(real)?;
    let spacing = options.spacing;
    for v in 0..real {
        for side in [TOP, BOTTOM] {
            let ids = incident(s.g, v, side);
            let free = s.free[v][side];
            if ids.len() < 2 || free == 0 || s.span[v] <= 0. {
                continue;
            }
            work.visit(ids.len())?;
            let mut edges = reserved(ids.len())?;
            edges.extend(ids.iter().map(|&e| e as usize));
            work.sort(&mut edges, |&a, &b| {
                let key = |e| s.pos[vertex(s.g, e, 1 - side)];
                s.dx[side][a]
                    .total_cmp(&s.dx[side][b])
                    .then(key(a).cmp(&key(b)))
                    .then(a.cmp(&b))
            })?;
            let half = s.span[v] * 0.5;
            let mut start = 0;
            for end in 0..=edges.len() {
                if end < edges.len() && s.role[side][edges[end]] != Role::Held {
                    continue;
                }
                if start < end {
                    let free = &edges[start..end];
                    let left = start.checked_sub(1).map(|i| s.dx[side][edges[i]]);
                    let right = edges.get(end).map(|&e| s.dx[side][e]);
                    let mut gap = filled(free.len(), 0.)?;
                    let mixed = free
                        .windows(2)
                        .any(|pair| s.role[side][pair[0]] != s.role[side][pair[1]]);
                    let requested = if mixed || left.is_some() || right.is_some() {
                        spacing.port_pitch.max(spacing.min_port_pitch)
                    } else {
                        spacing.port_pitch
                    };
                    let mut pitch = requested;
                    for tier in 0..=2 {
                        let mut units = 0.;
                        for (pair, step) in free.windows(2).zip(&mut gap[1..]) {
                            let (a, b) = (s.role[side][pair[0]], s.role[side][pair[1]]);
                            *step = if tier == 2 && a == b {
                                0.
                            } else if tier == 1 && a == Role::Leaves && a == b {
                                0.5
                            } else {
                                1.
                            };
                            units += *step;
                        }
                        let mut limit = if units > 0. {
                            s.span[v] / units
                        } else {
                            f64::INFINITY
                        };
                        if let Some(x) = left {
                            limit = limit.min((half - x) / (units + 1.));
                        }
                        if let Some(x) = right {
                            limit = limit.min((x + half) / (units + 1.));
                        }
                        if let (Some(a), Some(b)) = (left, right) {
                            limit = limit.min((b - a) / (units + 2.));
                        }
                        pitch = requested.min(limit.max(0.));
                        if limit >= spacing.min_port_pitch {
                            break;
                        }
                    }
                    for step in &mut gap {
                        *step *= pitch;
                    }
                    let bounds = Interval {
                        lo: left.map_or(-half, |x| (x + pitch).max(-half)),
                        hi: right.map_or(half, |x| (x - pitch).min(half)),
                    };
                    crate::work::push(
                        &mut runs,
                        PortRun {
                            v,
                            side,
                            edges: copy(free)?,
                            gap,
                            bounds,
                        },
                    )?;
                }
                start = end + 1;
            }
        }
    }
    Ok(runs)
}

fn wanted(s: &Geometry<'_>, f: &PortRun, e: usize, again: bool) -> f64 {
    let v = f.v;
    let side = f.side;
    let other = 1 - side;
    let e = s.follow[side][e];
    let u = vertex(s.g, e, other);
    let span = s.span[v];
    let center = s.x[v];
    let point = s.at(e, other);
    if s.free[v][side] < 2 {
        return 0.;
    }
    if again && (point - center).abs() <= span * 0.5 + INLINE {
        return (point - center).clamp(-span * 0.5, span * 0.5);
    }
    let width = if s.role[other][e] != Role::Held && s.free[u][other] >= 2 {
        s.span[u]
    } else {
        0.
    };
    let middle = if width > 0. { s.x[u] } else { point };
    let lo = (center - span * 0.5).max(middle - width * 0.5);
    let hi = (center + span * 0.5).min(middle + width * 0.5);
    if lo > hi {
        let lean = (span * 0.5 - LEAN_CLEAR).max(0.);
        return (point - center).clamp(-lean, lean);
    }
    let location = center + (middle - center) * (span / (span + width));
    location.clamp(lo, hi) - center
}

fn settle(
    s: &mut Geometry<'_>,
    real: usize,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let runs = port_runs(s, real, options, work)?;
    let widest = runs.iter().map(|f| f.edges.len()).max().unwrap_or(0);
    let mut values = reserved(widest)?;
    let mut pools = reserved(widest)?;
    for pass in 0..6 {
        let mut moved = false;
        for f in &runs {
            work.visit(f.edges.len())?;
            values.clear();
            values.extend(f.edges.iter().map(|&e| wanted(s, f, e, pass > 0)));
            if pava(&mut values, &f.gap, f.bounds, ZeroGap::Equal, &mut pools) {
                for (&e, &new) in f.edges.iter().zip(&values) {
                    let dx = &mut s.dx[f.side][e];
                    moved |= (*dx - new).abs() > COLLINEAR;
                    *dx = new;
                }
            }
        }
        if !moved {
            break;
        }
    }
    Ok(())
}

fn snap(s: &mut Geometry<'_>, work: &mut Work<'_>) -> LayoutResult<()> {
    let g = s.g;
    let mut blocks = Blocks::new(g)?;
    let mut near = reserved(g.from.len())?;
    let delta = |s: &Geometry<'_>, e: usize| s.at(s.follow[BOTTOM][e], TOP) - s.at(e, BOTTOM);
    for e in 0..g.from.len() {
        work.visit(1)?;
        let a = vertex(g, e, BOTTOM);
        if g.kind[a] == Kind::Dummy && s.bare[g.rank[a] as usize] {
            continue;
        }
        let b = vertex(g, s.follow[BOTTOM][e], TOP);
        let d = delta(s, e).abs();
        if d < COLLINEAR {
            blocks.join(a, b, work)?;
        } else if d <= BEND_SNAP {
            near.push(e);
        }
    }
    work.sort(&mut near, |&a, &b| {
        delta(s, a)
            .abs()
            .total_cmp(&delta(s, b).abs())
            .then(a.cmp(&b))
    })?;
    for e in near {
        work.visit(1)?;
        let (a, b) = (vertex(g, e, BOTTOM), vertex(g, s.follow[BOTTOM][e], TOP));
        let (upper, lower) = (blocks.root[a], blocks.root[b]);
        let shift = delta(s, e);
        if upper == lower || shift.abs() > BEND_SNAP {
            continue;
        }
        if shift.abs() < COLLINEAR {
            blocks.join(a, b, work)?;
            continue;
        }
        let mut choices = [(upper, shift), (lower, -shift)];
        if blocks.size[upper] > blocks.size[lower] {
            choices.swap(0, 1);
        }
        for (block, delta) in choices {
            if blocks.solid[block] && delta.abs() > NODE_SNAP {
                continue;
            }
            if !blocks.allowed(s, block, work)?.contains(delta) {
                continue;
            }
            for v in blocks.members(block) {
                s.x[v] += delta;
            }
            blocks.join(a, b, work)?;
            break;
        }
    }
    Ok(())
}

fn chord(s: &Geometry<'_>, chain: &[u32], lo: usize, hi: usize) -> (f64, f64) {
    (
        s.at(s.g.pred.at(chain[lo + 1])[0] as usize, BOTTOM),
        s.at(s.g.succ.at(chain[hi - 1])[0] as usize, TOP),
    )
}

fn lerp(s: &Geometry<'_>, a: u32, b: u32, v: u32, from: f64, to: f64) -> f64 {
    let rank = |v: u32| s.g.logical_rank[s.g.rank[v as usize] as usize];
    let t = (rank(v) - rank(a)) as f64 / (rank(b) - rank(a)) as f64;
    from + (to - from) * t
}

fn align_ports(s: &mut Geometry<'_>, work: &mut Work<'_>) -> LayoutResult<()> {
    let g = s.g;
    for (row, kind, side) in [
        (g.layers.first(), Kind::StartPort, BOTTOM),
        (g.layers.last(), Kind::EndPort, TOP),
    ] {
        let Some(row) = row.filter(|row| row.first().is_some_and(|&v| g.kind[v as usize] == kind))
        else {
            continue;
        };
        let target = |v: usize| -> Option<f64> {
            let [edge] = incident(g, v, side) else {
                return None;
            };
            if g.kind[v] != kind || s.direct_end[v][side] {
                return None;
            }
            let edge = *edge as usize;
            let attached = s.follow[side][edge];
            Some(s.at(attached, 1 - side) - s.dx[side][edge])
        };
        work.charge(row.len())?;
        let mut targets = reserved(row.len())?;
        let mut last_right = f64::NEG_INFINITY;
        for &v in row {
            work.visit(1)?;
            let v = v as usize;
            let Some(at) = target(v) else { break };
            let half = g.width[v] * 0.5;
            if at - half < last_right + MIN_BOUNDARY_PITCH {
                break;
            }
            targets.push(at);
            last_right = at + half;
        }
        if targets.len() == row.len() {
            for (&v, at) in row.iter().zip(targets) {
                s.x[v as usize] = at;
            }
        }
    }
    Ok(())
}

fn de_jitter(s: &mut Geometry<'_>, options: &Options, work: &mut Work<'_>) -> LayoutResult<()> {
    let g = s.g;
    let node_slack = ((options.spacing.node - options.spacing.edge) * 0.5).clamp(0., MICRO_DRIFT);
    let mut coordinates = Vec::new();
    for chain in &g.chains {
        let mut first = 1;
        while first + 1 < chain.len() {
            work.visit(1)?;
            let vertex = chain[first] as usize;
            if !s.free_bend(vertex) {
                first += 1;
                continue;
            }
            let mut low = s.x[vertex];
            let mut high = low;
            let mut room = s.room_with_slack(vertex, |_| false, node_slack);
            let mut end = first + 1;
            while end + 1 < chain.len() && s.free_bend(chain[end] as usize) {
                work.visit(1)?;
                let next = chain[end] as usize;
                let (lo, hi) = (low.min(s.x[next]), high.max(s.x[next]));
                let common = room.intersect(s.room_with_slack(next, |_| false, node_slack));
                if hi - lo > MICRO_DRIFT
                    || common.lo > hi
                    || common.hi < lo
                    || common.lo > common.hi
                {
                    break;
                }
                (low, high, room) = (lo, hi, common);
                end += 1;
            }

            let run = &chain[first..end];
            let (before, after) = chord(s, chain, first - 1, end);
            let anchor = [(chain[first - 1], before), (chain[end], after)]
                .into_iter()
                .filter(|&(v, _)| !s.free_bend(v as usize))
                .map(|(_, x)| x)
                .find(|&x| room.contains(x) && high - x <= MICRO_DRIFT && x - low <= MICRO_DRIFT);
            let target = if let Some(anchor) = anchor {
                Some(anchor)
            } else if run.len() > 1 {
                work.charge(
                    run.len()
                        .checked_mul(3)
                        .ok_or(WorkError::ArithmeticOverflow)?,
                )?;
                coordinates.clear();
                coordinates
                    .try_reserve(run.len())
                    .map_err(|_| LayoutError::AllocationLimit)?;
                coordinates.extend(run.iter().map(|&v| s.x[v as usize]));
                let (_, middle, _) =
                    coordinates.select_nth_unstable_by(run.len() / 2, f64::total_cmp);
                room.closest(*middle)
            } else {
                None
            };
            if let Some(x) = target {
                work.visit(run.len())?;
                for &v in run {
                    s.x[v as usize] = x;
                }
            }
            first = end;
        }
    }
    Ok(())
}

fn straighten(s: &mut Geometry<'_>, options: &Options, work: &mut Work<'_>) -> LayoutResult<()> {
    let g = s.g;
    let sep = options.spacing.edge;
    let curved = options.routing == RoutingStyle::Curved;
    let mut want = copy(&s.x)?;
    for chain in &g.chains {
        let mut i = 1;
        while i + 1 < chain.len() {
            if !s.bare[g.rank[chain[i] as usize] as usize] {
                i += 1;
                continue;
            }
            let lo = i - 1;
            while i + 1 < chain.len() && s.bare[g.rank[chain[i] as usize] as usize] {
                i += 1;
            }
            let (a, b) = chord(s, chain, lo, i);
            work.visit(i - lo)?;
            for &v in &chain[lo + 1..i] {
                want[v as usize] = lerp(s, chain[lo], chain[i], v, a, b);
            }
        }
    }
    let widest = g.layers.iter().map(Vec::len).max().unwrap_or(0);
    let mut twins = reserved(widest)?;
    let mut values = reserved(widest)?;
    let mut gaps = reserved(widest)?;
    let mut pools = reserved(widest)?;
    for (r, row) in g.layers.iter().enumerate() {
        if !s.bare[r] {
            continue;
        }
        work.visit(row.len())?;
        twins.clear();
        twins.extend(
            row.iter()
                .map(|&v| v as usize)
                .filter(|&v| s.twin[v] != END),
        );
        work.sort(&mut twins, |&a, &b| {
            (s.twin[a], s.pos[a]).cmp(&(s.twin[b], s.pos[b]))
        })?;
        for group in twins.chunk_by(|&a, &b| s.twin[a] == s.twin[b]) {
            let center = want[group[0]].midpoint(want[*group.last().unwrap()]);
            for (k, &v) in group.iter().enumerate() {
                want[v] = center + (k as f64 - (group.len() - 1) as f64 * 0.5) * sep;
            }
        }
        let mut first = 0;
        while first < row.len() {
            if s.protected[row[first] as usize] {
                first += 1;
                continue;
            }
            let mut end = first + 1;
            while end < row.len()
                && !s.protected[row[end] as usize]
                && s.same_lane(row[first] as usize, row[end] as usize)
            {
                end += 1;
            }
            let ids = &row[first..end];
            let a = ids[0] as usize;
            let b = ids[ids.len() - 1] as usize;
            values.clear();
            values.extend(ids.iter().map(|&v| want[v as usize]));
            gaps.clear();
            gaps.push(0.);
            gaps.extend(
                ids.windows(2)
                    .map(|pair| s.extent[pair[0] as usize].1 + s.extent[pair[1] as usize].0),
            );
            let room = Interval {
                lo: s.room(a, |_| false).lo,
                hi: s.room(b, |_| false).hi,
            };
            if pava(&mut values, &gaps, room, ZeroGap::Ordered, &mut pools) {
                for (&v, &new) in ids.iter().zip(&values) {
                    s.x[v as usize] = new;
                }
            }
            first = end;
        }
    }

    let mut clear = Vec::new();
    if curved {
        de_jitter(s, options, work)?;
    } else {
        clear = filled(s.x.len(), Interval::ALL)?;
        for (r, row) in g.layers.iter().enumerate() {
            if !s.labels_only[r] {
                continue;
            }
            let mut left = f64::NEG_INFINITY;
            let mut right = f64::INFINITY;
            for (&a, &b) in row.iter().zip(row.iter().rev()) {
                let (a, b) = (a as usize, b as usize);
                clear[a].lo = left;
                clear[b].hi = right;
                if g.kind[a] == Kind::Label {
                    left = s.x[a] + g.width[a] * 0.5 + sep;
                }
                if g.kind[b] == Kind::Label {
                    right = s.x[b] - g.width[b] * 0.5 - sep;
                }
            }
        }
    }
    // A retained bend also anchors its neighboring runs. Recompute their chords before
    // omitting anything, since interpolation through the old endpoints is no longer valid.
    let mut ranges = Vec::new();
    for chain in &g.chains {
        let can_skip = |v: u32| {
            let v = v as usize;
            let r = g.rank[v] as usize;
            g.kind[v] == Kind::Dummy
                && (s.bare[r] || !curved && s.labels_only[r])
                && !s.protected[v]
                && s.twin[v] == END
        };
        let mut i = 1;
        while i + 1 < chain.len() {
            if !can_skip(chain[i]) {
                let v = chain[i] as usize;
                if curved && s.columns.is_none() && s.free_bend(v) && !s.bare[g.rank[v] as usize] {
                    s.keep[v] = false;
                }
                i += 1;
                continue;
            }
            let start = i - 1;
            while i + 1 < chain.len() && can_skip(chain[i]) {
                i += 1;
            }
            crate::work::push(&mut ranges, (start, i))?;
            while let Some((lo, hi)) = ranges.pop() {
                if hi <= lo + 1 {
                    continue;
                }
                work.visit(hi - lo)?;
                let (a, b) = chord(s, chain, lo, hi);
                let mid = lo + (hi - lo) / 2;
                let mut blocker = None;
                for k in lo + 1..hi {
                    let v = chain[k] as usize;
                    let desired = lerp(s, chain[lo], chain[hi], chain[k], a, b);
                    if (clear
                        .get(v)
                        .is_some_and(|room| !room.contains(a.min(b)) || !room.contains(a.max(b)))
                        || !s.movable(v).contains(desired))
                        && blocker.is_none_or(|old: usize| k.abs_diff(mid) < old.abs_diff(mid))
                    {
                        blocker = Some(k);
                    }
                }
                if let Some(k) = blocker {
                    crate::work::push(&mut ranges, (lo, k))?;
                    crate::work::push(&mut ranges, (k, hi))?;
                } else {
                    for &v in &chain[lo + 1..hi] {
                        s.x[v as usize] = lerp(s, chain[lo], chain[hi], v, a, b);
                        s.keep[v as usize] = false;
                    }
                }
            }
        }
    }
    Ok(())
}

pub(super) fn refine(
    g: &mut Expanded,
    c: &Component,
    orientation: &Orientation,
    graph: &Graph,
    options: &Options,
    x: &mut [f64],
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    refine_in_columns(g, c, orientation, graph, options, x, None, work)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn refine_in_columns(
    g: &mut Expanded,
    c: &Component,
    orientation: &Orientation,
    graph: &Graph,
    options: &Options,
    x: &mut [f64],
    columns: Option<super::lanes::Columns<'_>>,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    work.charge(1)?;
    let mut s = Geometry::new(g, c, orientation, graph, options, x, work)?;
    s.columns = columns;
    center_forks(&mut s, c.nodes.len(), work)?;
    align_labels(&mut s, work)?;
    settle(&mut s, c.nodes.len(), options, work)?;
    snap(&mut s, work)?;
    if s.has_position_hints {
        mental_map::stabilize(&mut s, c, graph.direction, options, work)?;
    }
    if c.boundaries {
        align_ports(&mut s, work)?;
    }
    straighten(&mut s, options, work)?;
    if s.x
        .iter()
        .chain(&s.dx[TOP])
        .chain(&s.dx[BOTTOM])
        .any(|v| !v.is_finite())
    {
        return Err(WorkError::ArithmeticOverflow.into());
    }
    work.charge(x.len())?;
    work.finish()?;
    let Geometry {
        x: coordinates,
        dx: [target, source],
        keep,
        ..
    } = s;
    x.copy_from_slice(&coordinates);
    g.target_offset = target;
    g.source_offset = source;
    g.keep = keep;
    Ok(())
}

#[cfg(test)]
mod tests;
