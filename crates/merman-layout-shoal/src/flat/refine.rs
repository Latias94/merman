//! Cross-rank coordinate polish after compaction. All moves respect the original rank order,
//! asymmetric extents and component envelope. Port offsets are relative to their own vertex.

use super::{
    Component, Expanded, Kind, acyclic::Orientation, position::extents, to_canonical_port,
};
use crate::scratch::{capacity_bytes, clear_reserve};
use crate::work::{LayoutResult, Work};
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

fn copy_into<T: Copy>(out: &mut Vec<T>, values: &[T]) -> LayoutResult<()> {
    clear_reserve(out, values.len())?;
    out.extend_from_slice(values);
    Ok(())
}

fn fill<T: Copy>(out: &mut Vec<T>, len: usize, value: T) -> LayoutResult<()> {
    clear_reserve(out, len)?;
    out.resize(len, value);
    Ok(())
}

#[derive(Clone, Copy, Debug)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

#[derive(Debug, Default)]
struct GeometryScratch {
    x: Vec<f64>,
    dx: [Vec<f64>; 2],
    pos: Vec<usize>,
    extent: Vec<(f64, f64)>,
    bare: Vec<bool>,
    labels_only: Vec<bool>,
    span: Vec<f64>,
    free: Vec<[usize; 2]>,
    role: [Vec<Role>; 2],
    marker_width: [Vec<f64>; 2],
    // For an incidence, the opposite end of its original chain and the first attachment
    // beyond consecutive bend-only ranks. Neither lookup depends on the route's length.
    terminal: [Vec<usize>; 2],
    follow: [Vec<usize>; 2],
    direct_end: Vec<[bool; 2]>,
    twin: Vec<usize>,
    protected: Vec<bool>,
    keep: Vec<bool>,
}

impl GeometryScratch {
    fn retained_bytes(&self) -> usize {
        capacity_bytes(&self.x)
            + self.dx.iter().map(capacity_bytes).sum::<usize>()
            + capacity_bytes(&self.pos)
            + capacity_bytes(&self.extent)
            + capacity_bytes(&self.bare)
            + capacity_bytes(&self.labels_only)
            + capacity_bytes(&self.span)
            + capacity_bytes(&self.free)
            + self.role.iter().map(capacity_bytes).sum::<usize>()
            + self.marker_width.iter().map(capacity_bytes).sum::<usize>()
            + self.terminal.iter().map(capacity_bytes).sum::<usize>()
            + self.follow.iter().map(capacity_bytes).sum::<usize>()
            + capacity_bytes(&self.direct_end)
            + capacity_bytes(&self.twin)
            + capacity_bytes(&self.protected)
            + capacity_bytes(&self.keep)
    }
}

struct Geometry<'a> {
    g: &'a Expanded,
    columns: Option<super::lanes::Columns<'a>>,
    data: &'a mut GeometryScratch,
    envelope: Interval,
    boundary_pitch: f64,
    has_position_hints: bool,
}

impl<'a> Geometry<'a> {
    fn new(
        g: &'a Expanded,
        (c, orientation): (&Component, &Orientation),
        graph: &Graph,
        options: &Options,
        x: &[f64],
        (geometry, chain_ids): (&'a mut GeometryScratch, &mut Vec<usize>),
        work: &mut Work<'_>,
    ) -> LayoutResult<Self> {
        let n = x.len();
        let m = g.from.len();
        work.charge(n.checked_mul(12).ok_or(WorkError::ArithmeticOverflow)?)?;
        work.charge(m.checked_mul(10).ok_or(WorkError::ArithmeticOverflow)?)?;
        copy_into(&mut geometry.x, x)?;
        copy_into(&mut geometry.dx[TOP], &g.target_offset)?;
        copy_into(&mut geometry.dx[BOTTOM], &g.source_offset)?;
        fill(&mut geometry.pos, n, 0)?;
        clear_reserve(&mut geometry.extent, n)?;
        clear_reserve(&mut geometry.bare, g.layers.len())?;
        clear_reserve(&mut geometry.labels_only, g.layers.len())?;
        fill(&mut geometry.span, n, 0.)?;
        fill(&mut geometry.free, n, [0; 2])?;
        fill(&mut geometry.role[TOP], m, Role::Arrives)?;
        fill(&mut geometry.role[BOTTOM], m, Role::Leaves)?;
        fill(&mut geometry.marker_width[TOP], m, 0.0)?;
        fill(&mut geometry.marker_width[BOTTOM], m, 0.0)?;
        for side in [TOP, BOTTOM] {
            fill(&mut geometry.terminal[side], m, 0)?;
            fill(&mut geometry.follow[side], m, 0)?;
        }
        fill(&mut geometry.direct_end, n, [false; 2])?;
        fill(&mut geometry.twin, n, END)?;
        fill(&mut geometry.protected, n, false)?;
        fill(&mut geometry.keep, n, true)?;
        let mut s = Self {
            g,
            columns: None,
            data: geometry,
            boundary_pitch: options.spacing.edge.max(MIN_BOUNDARY_PITCH),
            envelope: Interval {
                lo: f64::INFINITY,
                hi: f64::NEG_INFINITY,
            },
            has_position_hints: false,
        };
        for (v, &center) in x.iter().enumerate() {
            let (left, right) = extents(g, c, options, v as u32);
            s.data.extent.push((left, right));
            s.envelope.lo = s.envelope.lo.min(center - left);
            s.envelope.hi = s.envelope.hi.max(center + right);
            if let Some(node) = c.nodes.get(v) {
                s.has_position_hints |= c.nodes.len() > 1
                    && !s.has_position_hints
                    && node.position_hint_in(graph.direction, g.rank[v]).is_some();
                s.data.span[v] = node
                    .port_span
                    .unwrap_or((g.width[v] - 2. * options.spacing.corner_clearance).max(0.))
                    .min(g.width[v]);
            }
        }
        for row in &g.layers {
            s.data
                .bare
                .push(row.iter().all(|&v| g.kind[v as usize] == Kind::Dummy));
            s.data.labels_only.push(
                row.iter().all(|&v| g.kind[v as usize].interior())
                    && row.iter().any(|&v| g.kind[v as usize] == Kind::Label),
            );
            for (i, &v) in row.iter().enumerate() {
                s.data.pos[v as usize] = i;
            }
        }
        let mut start = 0;
        clear_reserve(chain_ids, g.chains.len())?;
        for (i, chain) in g.chains.iter().enumerate() {
            work.visit(chain.len())?;
            if g.direct[i] {
                s.data.direct_end[chain[0] as usize][BOTTOM] = true;
                s.data.direct_end[*chain.last().unwrap() as usize][TOP] = true;
                continue;
            }
            let last = start + chain.len() - 2;
            let reversed = orientation.reversed[i];
            let spec = &graph.edges[c.links[i].edge as usize];
            let (src, dst, start_marker_w, end_marker_w) = if reversed {
                (
                    spec.target_port,
                    spec.source_port,
                    spec.end_marker.width,
                    spec.start_marker.width,
                )
            } else {
                (
                    spec.source_port,
                    spec.target_port,
                    spec.start_marker.width,
                    spec.end_marker.width,
                )
            };
            s.data.marker_width[BOTTOM][start] = start_marker_w;
            s.data.marker_width[TOP][last] = end_marker_w;
            for e in start..=last {
                s.data.terminal[TOP][e] = start;
                s.data.terminal[BOTTOM][e] = last;
                s.data.follow[TOP][e] =
                    if e > start && s.data.bare[g.rank[g.from[e] as usize] as usize] {
                        s.data.follow[TOP][e - 1]
                    } else {
                        e
                    };
                if reversed {
                    s.data.role[TOP][e] = Role::Leaves;
                    s.data.role[BOTTOM][e] = Role::Arrives;
                }
            }
            for e in (start..=last).rev() {
                s.data.follow[BOTTOM][e] =
                    if e < last && s.data.bare[g.rank[g.to[e] as usize] as usize] {
                        s.data.follow[BOTTOM][e + 1]
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
                    PortSideHint::Auto => {
                        anchor.or_else(|| g.spine_ports.centered(v, side, e).then_some(0.0))
                    }
                    PortSideHint::FixedOffset {
                        side: Side::Top | Side::Bottom,
                        offset,
                    } => Some(offset),
                    // The transverse coordinate of a side stub is the face itself. Its offset
                    // along the flow remains the router's (possibly signed) side parameter.
                    PortSideHint::FixedOffset { .. } | PortSideHint::Side(_) => {
                        Some(s.data.dx[side][e])
                    }
                };
                if let Some(offset) = fixed {
                    s.data.role[side][e] = Role::Held;
                    s.data.dx[side][e] = offset;
                } else {
                    s.data.free[v][side] += 1;
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
        work.sort(chain_ids, |&a, &b| ends(a).cmp(&ends(b)))?;
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
                    s.data.twin[v as usize] = twins[0];
                }
            }
        }
        Ok(s)
    }

    fn at(&self, e: usize, side: usize) -> f64 {
        self.data.x[vertex(self.g, e, side)] + self.data.dx[side][e]
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
        let i = self.data.pos[v];
        let (l, r) = self.data.extent[v];
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
                lo = lo.max(self.data.x[p] + (self.data.extent[p].1 - slack(p)) + l);
            }
        }
        if i + 1 < row.len() {
            let q = row[i + 1] as usize;
            if !same(q) {
                hi = hi.min(self.data.x[q] - (self.data.extent[q].0 - slack(q)) - r);
            }
        }
        Interval { lo, hi }
    }

    fn movable(&self, v: usize) -> Interval {
        if self.data.protected[v] {
            Interval {
                lo: self.data.x[v],
                hi: self.data.x[v],
            }
        } else {
            self.room(v, |_| false)
        }
    }

    fn free_bend(&self, v: usize) -> bool {
        self.g.kind[v] == Kind::Dummy && !self.data.protected[v] && self.data.twin[v] == END
    }
}

// Weighted union by size with explicit member lists. Checking a candidate translation visits
// only that block and its immediate rank neighbors, including on ranks with multiple members.
#[derive(Debug, Default)]
struct Blocks {
    root: Vec<usize>,
    next: Vec<usize>,
    tail: Vec<usize>,
    size: Vec<usize>,
    solid: Vec<bool>,
}
const END: usize = usize::MAX;

impl Blocks {
    fn rebuild(&mut self, g: &Expanded) -> LayoutResult<()> {
        let n = g.rank.len();
        clear_reserve(&mut self.root, n)?;
        self.root.extend(0..n);
        copy_into(&mut self.tail, &self.root)?;
        clear_reserve(&mut self.solid, n)?;
        self.solid.extend(g.kind.iter().map(|k| *k != Kind::Dummy));
        fill(&mut self.next, n, END)?;
        fill(&mut self.size, n, 1)
    }

    fn retained_bytes(&self) -> usize {
        capacity_bytes(&self.root)
            + capacity_bytes(&self.next)
            + capacity_bytes(&self.tail)
            + capacity_bytes(&self.size)
            + capacity_bytes(&self.solid)
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
            if s.data.protected[v] {
                return Ok(Interval { lo: 0., hi: 0. });
            }
            let room = s.room(v, |u| self.root[u] == b);
            interval = interval.intersect(Interval {
                lo: room.lo - s.data.x[v],
                hi: room.hi - s.data.x[v],
            });
        }
        Ok(interval)
    }
}

#[derive(Clone, Copy, Debug)]
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
    if edges.len() < 2 || s.data.direct_end[v][side] {
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
        let e = s.data.terminal[side][e as usize];
        let u = vertex(g, e, other);
        let i = s.data.pos[u];
        if Some(g.rank[u] as usize) != next_real[g.rank[v] as usize][side] || seen[u] == stamp {
            return Ok(None);
        }
        seen[u] = stamp;
        fan.private &= incident(g, u, other).len() == 1 && !s.data.direct_end[u][other];
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
    if s.data.protected[v] {
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
    let desired = target - s.data.x[v];
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
        s.data.x[member] += shift;
        s.data.protected[member] = true;
    }
    for (branch, segment) in [(a, fan.left), (b, fan.right)] {
        s.data.protected[branch] = true;
        if shift == desired
            && s.data.free[branch][other] > 1
            && s.data.role[other][segment] != Role::Held
        {
            s.data.role[other][segment] = Role::Held;
            s.data.free[branch][other] -= 1;
        }
    }
    Ok(())
}

fn center_forks(
    s: &mut Geometry<'_>,
    real: usize,
    scratch: &mut PassScratch,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let g = s.g;
    let PassScratch {
        nearest,
        blocks,
        fans,
        indices: seen,
        ..
    } = scratch;
    fill(nearest, g.layers.len(), [None; 2])?;
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
    blocks.rebuild(g)?;
    for &source in g.layers.iter().flatten() {
        work.visit(1)?;
        let [segment] = g.succ.at(source) else {
            continue;
        };
        let segment = *segment as usize;
        let target = g.to[segment];
        if g.pred.at(target).len() == 1
            && !s.data.direct_end[source as usize][BOTTOM]
            && !s.data.direct_end[target as usize][TOP]
            && (s.at(segment, TOP) - s.at(segment, BOTTOM)).abs() < INLINE
        {
            blocks.join(source as usize, target as usize, work)?;
        }
    }
    fill(fans, real, [None; 2])?;
    fill(seen, real, END)?;
    for (v, f) in fans.iter_mut().enumerate() {
        for side in [TOP, BOTTOM] {
            f[side] = fan(s, v, side, nearest, seen, v * 2 + side, work)?;
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
            let u = vertex(g, s.data.terminal[BOTTOM][e as usize], TOP);
            let [out] = g.succ.at(u as u32) else {
                paired = false;
                break;
            };
            let to = vertex(g, s.data.terminal[BOTTOM][*out as usize], TOP);
            if s.data.direct_end[u][BOTTOM] || join.is_some_and(|j| j != to) {
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
            center(s, blocks, v, BOTTOM, fork, work)?;
            center(s, blocks, j, TOP, back, work)?;
        }
    }
    for (v, f) in fans.iter().enumerate() {
        // A vertex with competing fans keeps its existing spine. A paired diamond has already
        // had first choice; ordinary private forks/joins and binary decision branches follow.
        if let [Some(fan), None] = *f {
            center(s, blocks, v, TOP, fan, work)?;
        }
        if let [None, Some(fan)] = *f {
            center(s, blocks, v, BOTTOM, fan, work)?;
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
    let endpoint = |s: &Geometry<'_>, e: usize, side: usize| {
        (s.at(e, side), s.data.free[vertex(g, e, side)][side])
    };
    for (i, chain) in g.chains.iter().enumerate() {
        if g.direct[i] || chain.len() < 3 {
            continue;
        }
        let inner = &chain[1..chain.len() - 1];
        let placed = |v: u32| !s.data.bare[g.rank[v as usize] as usize];
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
        let common = choose(s.data.x[first as usize], a, b, shared);
        for &v in inner {
            if !placed(v) {
                continue;
            }
            let id = v as usize;
            let target = common.or_else(|| {
                let up = s.data.follow[TOP][g.pred.at(v)[0] as usize];
                let down = s.data.follow[BOTTOM][g.succ.at(v)[0] as usize];
                choose(
                    s.data.x[id],
                    endpoint(s, up, BOTTOM),
                    endpoint(s, down, TOP),
                    s.movable(id),
                )
            });
            if let Some(target) = target {
                s.data.x[id] = target;
            }
        }
    }
    Ok(())
}

#[derive(Debug)]
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
    if values.is_empty() {
        return true;
    }
    if bounds.contains(values[0])
        && bounds.contains(values[values.len() - 1])
        && values
            .windows(2)
            .zip(gap.iter().skip(1))
            .all(|(pair, &gap)| {
                if zero_gap == ZeroGap::Equal && gap == 0.0 {
                    pair[0] == pair[1]
                } else {
                    pair[1] - pair[0] >= gap
                }
            })
    {
        return true;
    }
    let total = gap.iter().skip(1).sum::<f64>();
    let hi = bounds.hi - total;
    if bounds.lo.is_nan() || hi.is_nan() || bounds.lo > hi {
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

#[derive(Debug)]
struct PortRun {
    v: usize,
    side: usize,
    edges: std::ops::Range<usize>,
    bounds: Interval,
}

#[derive(Debug, Default)]
struct PortRuns {
    runs: Vec<PortRun>,
    edges: Vec<usize>,
    gaps: Vec<f64>,
}

impl PortRuns {
    fn retained_bytes(&self) -> usize {
        capacity_bytes(&self.runs) + capacity_bytes(&self.edges) + capacity_bytes(&self.gaps)
    }
}

fn port_runs(
    s: &Geometry<'_>,
    real: usize,
    options: &Options,
    PortRuns {
        runs,
        edges: run_edges,
        gaps,
    }: &mut PortRuns,
    edges: &mut Vec<usize>,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    runs.clear();
    run_edges.clear();
    gaps.clear();
    let spacing = options.spacing;
    for v in 0..real {
        for side in [TOP, BOTTOM] {
            let ids = incident(s.g, v, side);
            let free = s.data.free[v][side];
            if ids.len() < 2 || free == 0 || s.data.span[v] <= 0. {
                continue;
            }
            work.visit(ids.len())?;
            clear_reserve(edges, ids.len())?;
            edges.extend(ids.iter().map(|&e| e as usize));
            work.sort(edges, |&a, &b| {
                let key = |e| s.data.pos[vertex(s.g, e, 1 - side)];
                s.data.dx[side][a]
                    .total_cmp(&s.data.dx[side][b])
                    .then(key(a).cmp(&key(b)))
                    .then(a.cmp(&b))
            })?;
            let half = s.data.span[v] * 0.5;
            let mut start = 0;
            for end in 0..=edges.len() {
                if end < edges.len() && s.data.role[side][edges[end]] != Role::Held {
                    continue;
                }
                if start < end {
                    let free = &edges[start..end];
                    let left = start.checked_sub(1).map(|i| s.data.dx[side][edges[i]]);
                    let right = edges.get(end).map(|&e| s.data.dx[side][e]);
                    let offset = run_edges.len();
                    run_edges
                        .try_reserve(free.len())
                        .map_err(|_| LayoutError::AllocationLimit)?;
                    gaps.try_reserve(free.len())
                        .map_err(|_| LayoutError::AllocationLimit)?;
                    run_edges.extend_from_slice(free);
                    let end_offset = run_edges.len();
                    gaps.resize(end_offset, 0.);
                    let gap = &mut gaps[offset..end_offset];
                    let mixed = free
                        .windows(2)
                        .any(|pair| s.data.role[side][pair[0]] != s.data.role[side][pair[1]]);
                    let requested = if mixed || left.is_some() || right.is_some() {
                        spacing.port_pitch.max(spacing.min_port_pitch)
                    } else {
                        spacing.port_pitch
                    };
                    let mut pitch = requested;
                    for tier in 0..=2 {
                        let mut units = 0.;
                        for (pair, step) in free.windows(2).zip(&mut gap[1..]) {
                            let (a, b) = (s.data.role[side][pair[0]], s.data.role[side][pair[1]]);
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
                            s.data.span[v] / units
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
                    for step in gap.iter_mut() {
                        *step *= pitch;
                    }
                    let marker_clear = |a: usize, b: usize| {
                        let wa = s.data.marker_width[side][a];
                        let wb = s.data.marker_width[side][b];
                        if wa > 0.0 || wb > 0.0 {
                            0.5 * wa + 0.5 * wb + 4.0
                        } else {
                            0.0
                        }
                    };
                    let mut lo_gap = pitch;
                    let mut hi_gap = pitch;
                    if let Some(left_edge) = start.checked_sub(1).map(|i| edges[i]) {
                        let clear = marker_clear(left_edge, free[0]);
                        if clear > requested {
                            lo_gap = lo_gap.max(clear);
                        }
                    }
                    if let Some(&right_edge) = edges.get(end) {
                        let clear = marker_clear(free[free.len() - 1], right_edge);
                        if clear > requested {
                            hi_gap = hi_gap.max(clear);
                        }
                    }
                    // Keep held-port bounds feasible before applying expanded marker gaps.
                    let base_lo = left.map_or(-half, |x| (x + pitch).max(-half));
                    let base_hi = right.map_or(half, |x| (x - pitch).min(half));
                    let want_lo = left.map_or(-half, |x| (x + lo_gap).max(-half));
                    let want_hi = right.map_or(half, |x| (x - hi_gap).min(half));
                    let base_sum: f64 = gap[1..].iter().sum();
                    let bounds = if want_hi - want_lo >= base_sum {
                        Interval {
                            lo: want_lo,
                            hi: want_hi,
                        }
                    } else {
                        Interval {
                            lo: base_lo,
                            hi: base_hi,
                        }
                    };
                    let avail = (bounds.hi - bounds.lo).max(0.0);
                    if free.len() >= 2 && avail > 0.0 {
                        let pair_need = |a: usize, b: usize, base: f64| {
                            if base > 0.0 {
                                let clear = marker_clear(a, b);
                                if clear > requested {
                                    return base.max(clear.min(avail));
                                }
                            }
                            base
                        };
                        let mut target_sum = 0.0;
                        for (pair, &base) in free.windows(2).zip(&gap[1..]) {
                            target_sum += pair_need(pair[0], pair[1], base);
                        }
                        if target_sum > base_sum && target_sum.is_finite() {
                            let scale = if target_sum > avail {
                                ((avail - base_sum).max(0.0) / (target_sum - base_sum))
                                    .clamp(0.0, 1.0)
                            } else {
                                1.0
                            };
                            for (pair, step) in free.windows(2).zip(&mut gap[1..]) {
                                let need = pair_need(pair[0], pair[1], *step);
                                if need > *step {
                                    *step += scale * (need - *step);
                                }
                            }
                            // Ensure IEEE-754 rounding in the scaled gaps cannot exceed `avail`
                            // by a few ULPs and silently abort `weighted_pava`.
                            for _ in 0..8 {
                                let total: f64 = gap[1..].iter().sum();
                                if bounds.lo <= bounds.hi - total {
                                    break;
                                }
                                let excess = (bounds.lo - (bounds.hi - total)).max(0.0);
                                let mut count = free
                                    .windows(2)
                                    .zip(&gap[1..])
                                    .filter(|(pair, step)| {
                                        **step > 0.0 && marker_clear(pair[0], pair[1]) > requested
                                    })
                                    .count();
                                let only_expanded = count > 0;
                                if !only_expanded {
                                    count = gap[1..].iter().filter(|&&step| step > 0.0).count();
                                }
                                if count == 0 {
                                    break;
                                }
                                let trim = excess / count as f64;
                                for (pair, step) in free.windows(2).zip(&mut gap[1..]) {
                                    if *step > 0.0
                                        && (!only_expanded
                                            || marker_clear(pair[0], pair[1]) > requested)
                                    {
                                        let shrunk = (*step - trim).max(0.0);
                                        *step = if shrunk < *step {
                                            shrunk
                                        } else {
                                            step.next_down().max(0.0)
                                        };
                                    }
                                }
                            }
                        }
                    }
                    crate::work::push(
                        runs,
                        PortRun {
                            v,
                            side,
                            edges: offset..end_offset,
                            bounds,
                        },
                    )?;
                }
                start = end + 1;
            }
        }
    }
    Ok(())
}

fn wanted(s: &Geometry<'_>, f: &PortRun, e: usize, again: bool) -> f64 {
    let v = f.v;
    let side = f.side;
    let other = 1 - side;
    let e = s.data.follow[side][e];
    let u = vertex(s.g, e, other);
    let span = s.data.span[v];
    let center = s.data.x[v];
    let point = s.at(e, other);
    if s.data.free[v][side] < 2 {
        return 0.;
    }
    if again && (point - center).abs() <= span * 0.5 + INLINE {
        return (point - center).clamp(-span * 0.5, span * 0.5);
    }
    let width = if s.data.role[other][e] != Role::Held && s.data.free[u][other] >= 2 {
        s.data.span[u]
    } else {
        0.
    };
    let middle = if width > 0. { s.data.x[u] } else { point };
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
    scratch: &mut PassScratch,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let PassScratch {
        ports,
        indices,
        row: RowScratch { values, pools, .. },
        ..
    } = scratch;
    port_runs(s, real, options, ports, indices, work)?;
    let widest = ports.runs.iter().map(|f| f.edges.len()).max().unwrap_or(0);
    clear_reserve(values, widest)?;
    clear_reserve(pools, widest)?;
    for pass in 0..6 {
        let mut moved = false;
        for f in &ports.runs {
            let edges = &ports.edges[f.edges.clone()];
            let gaps = &ports.gaps[f.edges.clone()];
            work.visit(edges.len())?;
            values.clear();
            values.extend(edges.iter().map(|&e| wanted(s, f, e, pass > 0)));
            if pava(values, gaps, f.bounds, ZeroGap::Equal, pools) {
                for (&e, &new) in edges.iter().zip(values.iter()) {
                    let dx = &mut s.data.dx[f.side][e];
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

fn snap(s: &mut Geometry<'_>, scratch: &mut PassScratch, work: &mut Work<'_>) -> LayoutResult<()> {
    let g = s.g;
    let PassScratch {
        blocks,
        indices: near,
        ..
    } = scratch;
    blocks.rebuild(g)?;
    clear_reserve(near, g.from.len())?;
    let delta = |s: &Geometry<'_>, e: usize| s.at(s.data.follow[BOTTOM][e], TOP) - s.at(e, BOTTOM);
    for e in 0..g.from.len() {
        work.visit(1)?;
        let a = vertex(g, e, BOTTOM);
        if g.kind[a] == Kind::Dummy && s.data.bare[g.rank[a] as usize] {
            continue;
        }
        let b = vertex(g, s.data.follow[BOTTOM][e], TOP);
        let d = delta(s, e).abs();
        if d < COLLINEAR {
            blocks.join(a, b, work)?;
        } else if d <= BEND_SNAP {
            near.push(e);
        }
    }
    work.sort(near, |&a, &b| {
        delta(s, a)
            .abs()
            .total_cmp(&delta(s, b).abs())
            .then(a.cmp(&b))
    })?;
    for &e in near.iter() {
        work.visit(1)?;
        let (a, b) = (
            vertex(g, e, BOTTOM),
            vertex(g, s.data.follow[BOTTOM][e], TOP),
        );
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
                s.data.x[v] += delta;
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

fn align_ports(
    s: &mut Geometry<'_>,
    RowScratch {
        values,
        gaps,
        pools,
    }: &mut RowScratch,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let g = s.g;
    for (row, kind, side) in [
        (g.layers.first(), Kind::StartPort, BOTTOM),
        (g.layers.last(), Kind::EndPort, TOP),
    ] {
        let Some(row) = row.filter(|row| row.first().is_some_and(|&v| g.kind[v as usize] == kind))
        else {
            continue;
        };
        let target = |s: &Geometry<'_>, v: usize| -> Option<f64> {
            let [edge] = incident(g, v, side) else {
                return None;
            };
            if g.kind[v] != kind || s.data.direct_end[v][side] {
                return None;
            }
            let edge = *edge as usize;
            let attached = s.data.follow[side][edge];
            Some(s.at(attached, 1 - side) - s.data.dx[side][edge])
        };
        work.charge(row.len())?;
        clear_reserve(values, row.len())?;
        clear_reserve(gaps, row.len())?;
        clear_reserve(pools, row.len())?;
        let pitch = |a: u32, b: u32| {
            g.width[a as usize] * 0.5 + MIN_BOUNDARY_PITCH + g.width[b as usize] * 0.5
        };
        let mut first = 0;
        for end in 0..=row.len() {
            work.visit(1)?;
            if let Some(&v) = row.get(end)
                && let Some(at) = target(s, v as usize)
            {
                gaps.push(if let Some(&before) = values.last() {
                    let minimum = pitch(row[end - 1], v);
                    if at - before < minimum {
                        minimum + (s.boundary_pitch - MIN_BOUNDARY_PITCH)
                    } else {
                        minimum
                    }
                } else {
                    0.0
                });
                values.push(at);
                continue;
            }
            if first < end {
                let room = Interval {
                    lo: if first == 0 {
                        f64::NEG_INFINITY
                    } else {
                        s.data.x[row[first - 1] as usize] + pitch(row[first - 1], row[first])
                    },
                    hi: if end == row.len() {
                        f64::INFINITY
                    } else {
                        s.data.x[row[end] as usize] - pitch(row[end - 1], row[end])
                    },
                };
                work.visit(values.len())?;
                if pava(values, gaps, room, ZeroGap::Ordered, pools) {
                    for (&v, &at) in row[first..end].iter().zip(values.iter()) {
                        s.data.x[v as usize] = at;
                    }
                }
            }
            values.clear();
            gaps.clear();
            first = end + 1;
        }
    }
    Ok(())
}

fn port_accepts(
    s: &Geometry<'_>,
    ports: &PortRuns,
    v: usize,
    side: usize,
    edge: usize,
    x: f64,
) -> bool {
    let want = x - s.data.x[v];
    let first = ports.runs.partition_point(|r| (r.v, r.side) < (v, side));
    let matching = &ports.runs[first..];
    let matching_len = matching
        .iter()
        .take_while(|r| r.v == v && r.side == side)
        .count();
    if matching_len == 0 {
        return s.data.role[side][edge] != Role::Held
            && s.data.free[v][side] >= 2
            && want.abs() <= s.data.span[v] * 0.5;
    }
    let Some((f, i)) = matching[..matching_len].iter().find_map(|f| {
        ports.edges[f.edges.clone()]
            .iter()
            .position(|&e| e == edge)
            .map(|i| (f, i))
    }) else {
        return false;
    };
    let edges = &ports.edges[f.edges.clone()];
    let gaps = &ports.gaps[f.edges.clone()];
    let lo = if i == 0 {
        f.bounds.lo
    } else {
        s.data.dx[side][edges[i - 1]] + gaps[i]
    };
    let hi = if i + 1 == edges.len() {
        f.bounds.hi
    } else {
        s.data.dx[side][edges[i + 1]] - gaps[i + 1]
    };
    lo <= want && want <= hi
}

fn de_jitter(
    s: &mut Geometry<'_>,
    options: &Options,
    ports: &PortRuns,
    coordinates: &mut Vec<f64>,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let g = s.g;
    let node_slack = ((options.spacing.node - options.spacing.edge) * 0.5).clamp(0., MICRO_DRIFT);
    for chain in &g.chains {
        let mut first = 1;
        while first + 1 < chain.len() {
            work.visit(1)?;
            let vertex = chain[first] as usize;
            if !s.free_bend(vertex) {
                first += 1;
                continue;
            }
            let mut low = s.data.x[vertex];
            let mut high = low;
            let mut room = s.room_with_slack(vertex, |_| false, node_slack);
            let mut end = first + 1;
            while end + 1 < chain.len() && s.free_bend(chain[end] as usize) {
                work.visit(1)?;
                let next = chain[end] as usize;
                let (lo, hi) = (low.min(s.data.x[next]), high.max(s.data.x[next]));
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
            let before_edge = g.pred.at(chain[first])[0] as usize;
            let after_edge = g.succ.at(chain[end - 1])[0] as usize;
            let fits_other = |other: u32, side: usize, edge: usize, x: f64| {
                let v = other as usize;
                matches!(g.kind[v], Kind::StartPort | Kind::EndPort)
                    || port_accepts(s, ports, v, side, edge, x)
            };
            let candidates = [
                (chain[first - 1], before, chain[end], TOP, after_edge),
                (chain[end], after, chain[first - 1], BOTTOM, before_edge),
            ];
            let eligible = |(v, x, _, _, _): (u32, f64, u32, usize, usize)| {
                !s.free_bend(v as usize)
                    && room.contains(x)
                    && high - x <= MICRO_DRIFT
                    && x - low <= MICRO_DRIFT
            };
            // Prefer an anchor that the opposite terminal's port (or movable boundary port)
            // can genuinely reach without violating neighbor port pitch.
            let anchor = candidates
                .into_iter()
                .filter(|&c| eligible(c))
                .find(|&(_, x, other, side, edge)| fits_other(other, side, edge, x))
                .or_else(|| candidates.into_iter().find(|&c| eligible(c)))
                .map(|(_, x, _, _, _)| x);
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
                coordinates.extend(run.iter().map(|&v| s.data.x[v as usize]));
                let (_, middle, _) =
                    coordinates.select_nth_unstable_by(run.len() / 2, f64::total_cmp);
                room.closest(*middle)
            } else {
                None
            };
            if let Some(x) = target {
                work.visit(run.len())?;
                for &v in run {
                    s.data.x[v as usize] = x;
                }
            }
            first = end;
        }
    }
    Ok(())
}

fn snap_micro_ports(
    s: &mut Geometry<'_>,
    ports: &PortRuns,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    for f in &ports.runs {
        let edges = &ports.edges[f.edges.clone()];
        let gaps = &ports.gaps[f.edges.clone()];
        work.visit(edges.len())?;
        for (i, &e) in edges.iter().enumerate() {
            let follow = s.data.follow[f.side][e];
            let want = s.at(follow, 1 - f.side) - s.data.x[f.v];
            let cur = s.data.dx[f.side][e];
            let delta = (want - cur).abs();
            if delta <= COLLINEAR || delta > MICRO_DRIFT {
                continue;
            }
            let lo = if i == 0 {
                f.bounds.lo
            } else {
                s.data.dx[f.side][edges[i - 1]] + gaps[i]
            };
            let hi = if i + 1 == edges.len() {
                f.bounds.hi
            } else {
                s.data.dx[f.side][edges[i + 1]] - gaps[i + 1]
            };
            if lo <= want && want <= hi {
                s.data.dx[f.side][e] = want;
            }
        }
    }
    Ok(())
}

fn straighten(
    s: &mut Geometry<'_>,
    options: &Options,
    scratch: &mut PassScratch,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let g = s.g;
    let sep = options.spacing.edge;
    let curved = options.routing == RoutingStyle::Curved;
    let across = |s: &Geometry<'_>, lo, hi, v, a, b| {
        if options.routing == RoutingStyle::Orthogonal {
            a
        } else {
            lerp(s, lo, hi, v, a, b)
        }
    };
    let PassScratch {
        want,
        indices: twins,
        ports,
        row: RowScratch {
            values,
            gaps,
            pools,
        },
        clearance,
        ranges,
        ..
    } = scratch;
    copy_into(want, &s.data.x)?;
    for chain in &g.chains {
        let mut i = 1;
        while i + 1 < chain.len() {
            if !s.data.bare[g.rank[chain[i] as usize] as usize] {
                i += 1;
                continue;
            }
            let lo = i - 1;
            while i + 1 < chain.len() && s.data.bare[g.rank[chain[i] as usize] as usize] {
                i += 1;
            }
            let (a, b) = chord(s, chain, lo, i);
            work.visit(i - lo)?;
            for &v in &chain[lo + 1..i] {
                want[v as usize] = across(s, chain[lo], chain[i], v, a, b);
            }
        }
    }
    let widest = g
        .layers
        .iter()
        .zip(&s.data.bare)
        .filter_map(|(row, &bare)| bare.then_some(row.len()))
        .max()
        .unwrap_or(0);
    clear_reserve(twins, widest)?;
    clear_reserve(values, widest)?;
    clear_reserve(gaps, widest)?;
    clear_reserve(pools, widest)?;
    for (r, row) in g.layers.iter().enumerate() {
        if !s.data.bare[r] {
            continue;
        }
        work.visit(row.len())?;
        twins.clear();
        twins.extend(
            row.iter()
                .map(|&v| v as usize)
                .filter(|&v| s.data.twin[v] != END),
        );
        work.sort(twins, |&a, &b| {
            (s.data.twin[a], s.data.pos[a]).cmp(&(s.data.twin[b], s.data.pos[b]))
        })?;
        for group in twins.chunk_by(|&a, &b| s.data.twin[a] == s.data.twin[b]) {
            work.visit(group.len())?;
            if group
                .windows(2)
                .all(|pair| want[pair[1]] - want[pair[0]] >= sep)
            {
                continue;
            }
            values.clear();
            values.extend(group.iter().map(|&v| want[v]));
            gaps.clear();
            gaps.resize(group.len(), sep);
            if pava(values, gaps, Interval::ALL, ZeroGap::Ordered, pools) {
                for (&v, &value) in group.iter().zip(values.iter()) {
                    want[v] = value;
                }
            }
        }
        let mut first = 0;
        while first < row.len() {
            if s.data.protected[row[first] as usize] {
                first += 1;
                continue;
            }
            let mut end = first + 1;
            while end < row.len()
                && !s.data.protected[row[end] as usize]
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
                ids.windows(2).map(|pair| {
                    s.data.extent[pair[0] as usize].1 + s.data.extent[pair[1] as usize].0
                }),
            );
            let room = Interval {
                lo: s.room(a, |_| false).lo,
                hi: s.room(b, |_| false).hi,
            };
            if pava(values, gaps, room, ZeroGap::Ordered, pools) {
                for (&v, &new) in ids.iter().zip(values.iter()) {
                    s.data.x[v as usize] = new;
                }
            }
            first = end;
        }
    }

    clearance.clear();
    de_jitter(s, options, ports, values, work)?;
    snap_micro_ports(s, ports, work)?;
    if !curved {
        fill(clearance, s.data.x.len(), Interval::ALL)?;
        for (r, row) in g.layers.iter().enumerate() {
            if !s.data.labels_only[r] {
                continue;
            }
            let mut left = f64::NEG_INFINITY;
            let mut right = f64::INFINITY;
            for (&a, &b) in row.iter().zip(row.iter().rev()) {
                let (a, b) = (a as usize, b as usize);
                clearance[a].lo = left;
                clearance[b].hi = right;
                if g.kind[a] == Kind::Label {
                    left = s.data.x[a] + g.width[a] * 0.5 + sep;
                }
                if g.kind[b] == Kind::Label {
                    right = s.data.x[b] - g.width[b] * 0.5 - sep;
                }
            }
        }
    }
    // A retained bend also anchors its neighboring runs. Recompute their chords before
    // omitting anything, since interpolation through the old endpoints is no longer valid.
    ranges.clear();
    for chain in &g.chains {
        let can_skip = |v: u32| {
            let v = v as usize;
            let r = g.rank[v] as usize;
            g.kind[v] == Kind::Dummy
                && (s.data.bare[r] || !curved && s.data.labels_only[r])
                && !s.data.protected[v]
                && (s.data.twin[v] == END || (sep > 0.0 && s.data.bare[r]))
        };
        let mut i = 1;
        while i + 1 < chain.len() {
            if !can_skip(chain[i]) {
                let v = chain[i] as usize;
                if curved
                    && s.columns.is_none()
                    && s.free_bend(v)
                    && !s.data.bare[g.rank[v] as usize]
                {
                    s.data.keep[v] = false;
                }
                i += 1;
                continue;
            }
            let start = i - 1;
            while i + 1 < chain.len() && can_skip(chain[i]) {
                i += 1;
            }
            crate::work::push(ranges, (start, i))?;
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
                    let desired = across(s, chain[lo], chain[hi], chain[k], a, b);
                    // A twin can omit the waypoint only when its chord already has the
                    // projected separation. A tightened lane needs its own waypoint.
                    let displaced_twin =
                        s.data.twin[v] != END && (desired - s.data.x[v]).abs() >= COLLINEAR;
                    if (displaced_twin
                        || clearance.get(v).is_some_and(|room| {
                            !room.contains(a.min(b)) || !room.contains(a.max(b))
                        })
                        || !s.movable(v).contains(desired))
                        && blocker.is_none_or(|old: usize| k.abs_diff(mid) < old.abs_diff(mid))
                    {
                        blocker = Some(k);
                    }
                }
                if let Some(k) = blocker {
                    crate::work::push(ranges, (lo, k))?;
                    crate::work::push(ranges, (k, hi))?;
                } else {
                    for &v in &chain[lo + 1..hi] {
                        s.data.x[v as usize] = across(s, chain[lo], chain[hi], v, a, b);
                        s.data.keep[v as usize] = false;
                    }
                }
            }
        }
    }
    Ok(())
}

#[derive(Debug, Default)]
struct RowScratch {
    values: Vec<f64>,
    gaps: Vec<f64>,
    pools: Vec<Pool>,
}

impl RowScratch {
    fn retained_bytes(&self) -> usize {
        capacity_bytes(&self.values) + capacity_bytes(&self.gaps) + capacity_bytes(&self.pools)
    }
}

#[derive(Debug, Default)]
struct PassScratch {
    // Chain IDs, visit stamps and sorted indices live in separate passes.
    indices: Vec<usize>,
    blocks: Blocks,
    nearest: Vec<[Option<usize>; 2]>,
    fans: Vec<[Option<Fan>; 2]>,
    ports: PortRuns,
    row: RowScratch,
    want: Vec<f64>,
    clearance: Vec<Interval>,
    ranges: Vec<(usize, usize)>,
}

#[derive(Debug, Default)]
pub(super) struct RefineScratch {
    geometry: GeometryScratch,
    passes: PassScratch,
    mental_map: mental_map::ProjectionScratch,
}

impl RefineScratch {
    pub(super) fn retained_bytes(&self) -> usize {
        let PassScratch {
            indices,
            blocks,
            nearest,
            fans,
            ports,
            row,
            want,
            clearance,
            ranges,
        } = &self.passes;
        self.geometry.retained_bytes()
            + capacity_bytes(indices)
            + blocks.retained_bytes()
            + capacity_bytes(nearest)
            + capacity_bytes(fans)
            + ports.retained_bytes()
            + row.retained_bytes()
            + capacity_bytes(want)
            + capacity_bytes(clearance)
            + capacity_bytes(ranges)
            + self.mental_map.retained_bytes()
    }
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
    let mut scratch = std::mem::take(&mut g.refine_scratch);
    let result = refine_with_scratch(
        g,
        (c, orientation),
        graph,
        options,
        x,
        columns,
        &mut scratch,
        work,
    );
    let result = result.and_then(|()| {
        let GeometryScratch {
            x: coordinates,
            dx: [target, source],
            keep,
            ..
        } = &mut scratch.geometry;
        // The keep flags have no other producer to size the spare on the first solve.
        g.keep
            .try_reserve_exact(keep.len().saturating_sub(g.keep.len()))
            .map_err(|_| LayoutError::AllocationLimit)?;
        x.copy_from_slice(coordinates);
        std::mem::swap(&mut g.target_offset, target);
        std::mem::swap(&mut g.source_offset, source);
        std::mem::swap(&mut g.keep, keep);
        Ok(())
    });
    g.refine_scratch = scratch;
    result
}

#[allow(clippy::too_many_arguments)]
fn refine_with_scratch(
    g: &Expanded,
    (c, orientation): (&Component, &Orientation),
    graph: &Graph,
    options: &Options,
    x: &[f64],
    columns: Option<super::lanes::Columns<'_>>,
    scratch: &mut RefineScratch,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let RefineScratch {
        geometry,
        passes,
        mental_map: projection,
    } = scratch;
    let mut s = Geometry::new(
        g,
        (c, orientation),
        graph,
        options,
        x,
        (geometry, &mut passes.indices),
        work,
    )?;
    s.columns = columns;
    center_forks(&mut s, c.nodes.len(), passes, work)?;
    align_labels(&mut s, work)?;
    settle(&mut s, c.nodes.len(), options, passes, work)?;
    snap(&mut s, passes, work)?;
    if s.has_position_hints {
        mental_map::stabilize(
            &mut s,
            c,
            graph.direction,
            options,
            passes,
            projection,
            work,
        )?;
    }
    if c.boundaries {
        align_ports(&mut s, &mut passes.row, work)?;
    }
    straighten(&mut s, options, passes, work)?;
    if c.boundaries {
        align_ports(&mut s, &mut passes.row, work)?;
    }
    if s.data
        .x
        .iter()
        .chain(&s.data.dx[TOP])
        .chain(&s.data.dx[BOTTOM])
        .any(|v| !v.is_finite())
    {
        return Err(WorkError::ArithmeticOverflow.into());
    }
    work.charge(x.len())?;
    work.finish()
}

// Rejoin straight chains whose contacts moved after terminal docking. Row neighbors and
// protected waypoints retain the clearances established during refinement.
pub(super) fn follow_final_ports(
    g: &Expanded,
    options: &Options,
    x: &mut [f64],
    columns: Option<super::lanes::Columns<'_>>,
    old_offsets: &[[f64; 2]],
    docked: impl Fn(usize) -> bool,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let geometry = &g.refine_scratch.geometry;
    let mut segment = 0;
    for (edge, chain) in g.chains.iter().enumerate() {
        work.visit(1)?;
        if g.direct[edge] {
            continue;
        }
        let first = segment;
        segment += chain.len() - 1;
        let last = segment - 1;
        let [old_source, old_target] = old_offsets[edge];
        if chain.len() < 3
            || docked(edge)
            || (old_source == g.source_offset[first] && old_target == g.target_offset[last])
        {
            continue;
        }
        let a = chain[0] as usize;
        let b = *chain.last().unwrap() as usize;
        let contact = |v: usize, offset: f64| (x[v] - g.width[v] / 2.0) + g.width[v] / 2.0 + offset;
        let from = contact(a, g.source_offset[first]);
        let to = contact(b, g.target_offset[last]);
        let old_from = contact(a, old_source);
        let old_to = contact(b, old_target);
        let orthogonal = options.routing == RoutingStyle::Orthogonal;
        let on_chord = |v: usize, from: f64, to: f64| {
            let rank = |v: usize| g.logical_rank[g.rank[v] as usize];
            let t = (rank(v) - rank(a)) as f64 / (rank(b) - rank(a)) as f64;
            if orthogonal {
                from
            } else {
                from + (to - from) * t
            }
        };
        let interiors = &chain[1..chain.len() - 1];
        work.visit(interiors.len())?;
        for &v in interiors {
            let v = v as usize;
            let desired = on_chord(v, from, to);
            let previous = on_chord(v, old_from, old_to);
            let room = {
                let row = &g.layers[g.rank[v] as usize];
                let slot = geometry.pos[v];
                let (left, right) = geometry.extent[v];
                let lo = slot.checked_sub(1).map_or(f64::NEG_INFINITY, |slot| {
                    let neighbor = row[slot];
                    x[neighbor as usize] + geometry.extent[neighbor as usize].1 + left
                });
                let hi = row.get(slot + 1).map_or(f64::INFINITY, |&neighbor| {
                    x[neighbor as usize] - geometry.extent[neighbor as usize].0 - right
                });
                let [lane_lo, lane_hi] = columns
                    .map_or([f64::NEG_INFINITY, f64::INFINITY], |columns| {
                        columns.content[columns.owners[v]]
                    });
                Interval {
                    lo: lo.max(lane_lo + left),
                    hi: hi.min(lane_hi - right),
                }
            };
            if g.kind[v] == Kind::Dummy
                && !geometry.protected[v]
                && x[v] == geometry.x[v]
                && (x[v] - previous).abs() < COLLINEAR
                && room.contains(desired)
            {
                x[v] = desired;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
