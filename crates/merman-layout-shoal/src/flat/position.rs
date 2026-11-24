//! Port spreading and four-way Brandes-Kopf coordinate assignment on the ordered ranks.

use super::{Component, Csr, Expanded, Kind, acyclic::Orientation, to_canonical_port, validate};
use crate::scratch::{capacity_bytes, clear_reserve};
use crate::work::{LayoutResult, Work, filled};
use crate::{
    Direction, Graph, LayoutError, NodeSpec, Options, PortSideHint, Side, Spacing, WorkError,
};

pub(super) fn extents(g: &Expanded, c: &Component, opts: &Options, v: u32) -> (f64, f64) {
    let v = v as usize;
    let half = g.width[v] * 0.5;
    if g.kind[v] == Kind::Node {
        let gap = opts.spacing.node * 0.5;
        (
            half + c.margin_left[v] + gap,
            half + c.margin_right[v] + gap,
        )
    } else {
        let extent = half + opts.spacing.edge * 0.5;
        (extent, extent)
    }
}

/// Use one straight column for each exterior lane that the order pass reserved.
pub(super) fn straighten_exterior_lanes(
    g: &mut Expanded,
    c: &Component,
    options: &Options,
    x: &mut [f64],
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    if g.exterior_lanes.is_empty() {
        return Ok(());
    }
    work.charge(x.len())?;
    let mut reserved = filled(x.len(), false)?;
    for lane in &g.exterior_lanes {
        let chain = &g.chains[lane.edge];
        work.visit(chain.len())?;
        for &v in &chain[1..chain.len() - 1] {
            reserved[v as usize] = true;
        }
    }

    // The order pass reserves outer lanes first. Place the innermost lane first so each
    // outer lane can read its neighbor's final position wherever their ranks overlap.
    let pitch = options.spacing.edge.max(12.0);
    // A label needs the full pitch past its box. Extents already include each half-gap.
    let label_clearance = |inner: u32, outer: u32| {
        let kind = g.kind[inner as usize];
        if kind != Kind::Label && g.kind[outer as usize] != Kind::Label {
            return 0.0;
        }
        let inner_gap = if kind == Kind::Node {
            options.spacing.node
        } else {
            options.spacing.edge
        };
        (pitch - inner_gap * 0.5 - options.spacing.edge * 0.5).max(0.0)
    };
    work.charge(g.layers.len())?;
    let mut outside = filled(g.layers.len(), [None; 2])?;
    for lane in g.exterior_lanes.iter().rev() {
        let chain = &g.chains[lane.edge];
        let bends = &chain[1..chain.len() - 1];
        let side = usize::from(lane.right);
        let sign = if lane.right { 1.0 } else { -1.0 };
        let mut axis = f64::NEG_INFINITY;
        for &v in bends {
            let rank = g.rank[v as usize] as usize;
            let row = &g.layers[rank];
            let own = if lane.right {
                extents(g, c, options, v).0
            } else {
                extents(g, c, options, v).1
            };
            work.visit(row.len() + 1)?;
            let next = |v: u32| !reserved[v as usize];
            let interior = if lane.right {
                row.iter().rev().copied().find(|&v| next(v))
            } else {
                row.iter().copied().find(|&v| next(v))
            };
            if let Some(other) = interior {
                let extent = extents(g, c, options, other);
                let edge = if lane.right { extent.1 } else { extent.0 };
                axis = axis.max(sign * x[other as usize] + edge + own + label_clearance(other, v));
            }
            if let Some(other) = outside[rank][side] {
                let extent = extents(g, c, options, other);
                let edge = if lane.right { extent.1 } else { extent.0 };
                let distance = pitch.max(edge + own + label_clearance(other, v));
                axis = axis.max(sign * x[other as usize] + distance);
            }
        }
        let center = finite(axis)?;
        for &v in bends {
            work.visit(1)?;
            let rank = g.rank[v as usize] as usize;
            outside[rank][side] = Some(v);
            x[v as usize] = sign * center;
            g.keep[v as usize] = true;
        }
    }
    Ok(())
}

fn finite(x: f64) -> LayoutResult<f64> {
    if x.is_finite() {
        Ok(x)
    } else {
        Err(LayoutError::InvalidInput(
            "horizontal coordinates exceed finite precision",
        ))
    }
}

/// Visiting opposite ranks in order is a counting sort for all incident lists at once.
/// Ties at the same peer follow its original CSR order (segment occurrence).
fn by_peer(
    g: &Expanded,
    ends: &Csr,
    peers: &Csr,
    vertex: &[u32],
    Csr { start, entries }: &mut Csr,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    work.charge(ends.start.len())?;
    work.charge(vertex.len())?;
    clear_reserve(start, ends.start.len())?;
    start.extend_from_slice(&ends.start);
    clear_reserve(entries, vertex.len())?;
    entries.resize(vertex.len(), 0);
    for &peer in g.layers.iter().flatten() {
        work.visit(1)?;
        for &e in peers.at(peer) {
            work.visit(1)?;
            let at = &mut start[vertex[e as usize] as usize];
            entries[*at as usize] = e;
            *at += 1;
        }
    }
    start.copy_within(..g.rank.len(), 1);
    start[0] = 0;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum End {
    Held,
    Leaves,
    Arrives,
}

/// A perpendicular attachment contributes the face's x to BK; its offset along y belongs
/// to the terminal stub router. An explicit zero is just as fixed as a nonzero offset.
fn held_offset(
    dir: Direction,
    node: &NodeSpec,
    width: f64,
    natural: Side,
    hint: PortSideHint,
) -> Option<f64> {
    let (face, fixed) = match to_canonical_port(dir, hint) {
        PortSideHint::Auto => (natural, None),
        PortSideHint::Side(s) => (s, None),
        PortSideHint::FixedOffset { side, offset } => (side, Some(offset)),
    };
    match face {
        Side::Left => Some(-width * 0.5),
        Side::Right => Some(width * 0.5),
        Side::Top | Side::Bottom => fixed.or_else(|| {
            if let Some(y) = node.anchor_y
                && matches!(dir, Direction::Left | Direction::Right)
            {
                Some(y - node.size.height * 0.5)
            } else {
                (face != natural).then_some(0.0)
            }
        }),
    }
}

#[derive(Debug, Default)]
struct Ports {
    source: Vec<f64>,
    target: Vec<f64>,
    starts: Vec<End>,
    ends: Vec<End>,
}

fn spread_face(
    ids: &[u32],
    role: &[End],
    offsets: &mut [f64],
    span: f64,
    spacing: &Spacing,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    if ids.len() < 2 || span <= 0.0 {
        return Ok(());
    }
    let free = || {
        ids.iter()
            .map(|&v| v as usize)
            .filter(|&i| role[i] != End::Held)
    };
    let mut last = None;
    let (mut gaps, mut twin_tails, mut mixed) = (0usize, 0usize, 0usize);
    work.visit(ids.len())?;
    for e in free() {
        if let Some(prev) = last {
            gaps += 1;
            twin_tails += usize::from(prev == End::Leaves && role[e] == End::Leaves);
            mixed += usize::from(prev != role[e]);
        }
        last = Some(role[e]);
    }
    if gaps == 0 {
        return Ok(());
    }
    let all = gaps as f64;
    let half_tails = all - 0.5 * twin_tails as f64;
    let (tail_gap, head_gap, units) = if span / all >= spacing.min_port_pitch {
        (1.0, 1.0, all)
    } else if span / half_tails >= spacing.min_port_pitch {
        (0.5, 1.0, half_tails)
    } else {
        (0.0, 0.0, mixed as f64)
    };
    if units == 0.0 {
        return Ok(());
    }
    // If transitions alone exceed the physical span, share the available width among them.
    let desired = if mixed == 0 {
        spacing.port_pitch
    } else {
        spacing.port_pitch.max(spacing.min_port_pitch)
    };
    let pitch = desired.min(span / units);
    let mut used = 0.0;
    last = None;
    work.visit(ids.len())?;
    for e in free() {
        if let Some(prev) = last {
            used += if prev != role[e] {
                1.0
            } else if prev == End::Leaves {
                tail_gap
            } else {
                head_gap
            };
        }
        offsets[e] = (used - units * 0.5) * pitch;
        last = Some(role[e]);
    }
    Ok(())
}

fn spread(
    g: &Expanded,
    (c, o): (&Component, &Orientation),
    graph: &Graph,
    spacing: &Spacing,
    lists: &Neighbours,
    Ports {
        source,
        target,
        starts,
        ends,
    }: &mut Ports,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let m = g.from.len();
    work.charge(m.checked_mul(4).ok_or(WorkError::ArithmeticOverflow)?)?;
    for offsets in [&mut *source, &mut *target] {
        clear_reserve(offsets, m)?;
        offsets.resize(m, 0.0);
    }
    for roles in [&mut *starts, &mut *ends] {
        clear_reserve(roles, m)?;
        roles.resize(m, End::Held);
    }
    let mut first = 0;
    for (i, chain) in g.chains.iter().enumerate() {
        work.visit(1)?;
        if g.direct[i] {
            continue;
        }
        let last = first + (chain.len() - 2);
        let spec = &graph.edges[c.links[i].edge as usize];
        let (tail, head, tail_role, head_role) = if o.reversed[i] {
            (
                spec.target_port,
                spec.source_port,
                End::Arrives,
                End::Leaves,
            )
        } else {
            (
                spec.source_port,
                spec.target_port,
                End::Leaves,
                End::Arrives,
            )
        };
        for (v, side, hint, role, slot, offset) in [
            (
                g.from[first],
                Side::Bottom,
                tail,
                tail_role,
                &mut starts[first],
                &mut source[first],
            ),
            (
                g.to[last],
                Side::Top,
                head,
                head_role,
                &mut ends[last],
                &mut target[last],
            ),
        ] {
            let v = v as usize;
            let node = &graph.nodes[c.ids[v] as usize];
            if let Some(dx) = held_offset(graph.direction, node, g.width[v], side, hint) {
                *offset = dx;
            } else {
                *slot = role;
            }
        }
        first = last + 1;
    }
    for (v, &id) in c.ids.iter().enumerate() {
        work.visit(1)?;
        let width = g.width[v];
        let span = graph.nodes[id as usize]
            .port_span
            .unwrap_or((width - 2.0 * spacing.corner_clearance).max(0.0))
            .min(width);
        spread_face(lists.succ.at(v as u32), starts, source, span, spacing, work)?;
        spread_face(lists.pred.at(v as u32), ends, target, span, spacing, work)?;
    }
    Ok(())
}

#[derive(Debug, Default)]
struct Neighbours {
    pred: Csr,
    succ: Csr,
}

impl Neighbours {
    fn prefer_spine(
        &mut self,
        g: &Expanded,
        orientation: &Orientation,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        if g.exterior_lanes.is_empty() {
            return Ok(());
        }
        // The full incidence lists have already spread the ports. For block alignment, a
        // short forward link takes precedence over a shortcut at the same node face.
        work.charge(g.layers.len() + g.from.len() * 2 + g.rank.len())?;
        let occupied = g
            .layers
            .iter()
            .map(|row| row.iter().any(|&v| g.kind[v as usize] == Kind::Node));
        let mut node_ranks = 0usize;
        let mut rank_count = crate::work::reserved(g.layers.len())?;
        for occupied in occupied {
            node_ranks += usize::from(occupied);
            rank_count.push(node_ranks);
        }
        let mut keep = filled(g.from.len(), true)?;
        let mut short = filled(g.from.len(), false)?;
        for lane in &g.exterior_lanes {
            let bends = &g.chains[lane.edge];
            keep[g.pred.at(bends[1])[0] as usize] = false;
            keep[g.succ.at(bends[bends.len() - 2])[0] as usize] = false;
        }
        let mut first = 0;
        for (edge, chain) in g.chains.iter().enumerate() {
            work.visit(1)?;
            if g.direct[edge] {
                continue;
            }
            let last = first + chain.len() - 2;
            let (from, to) = (chain[0], *chain.last().unwrap());
            if !orientation.reversed[edge]
                && rank_count[g.rank[to as usize] as usize]
                    == rank_count[g.rank[from as usize] as usize] + 1
            {
                short[first] = true;
                short[last] = true;
            }
            first = last + 1;
        }
        for (vertex, kind) in g.kind.iter().enumerate() {
            work.visit(1)?;
            if *kind != Kind::Node {
                continue;
            }
            for face in [self.pred.at(vertex as u32), self.succ.at(vertex as u32)] {
                work.visit(face.len() * 2)?;
                if face.iter().any(|&e| short[e as usize]) {
                    for &e in face {
                        keep[e as usize] &= short[e as usize];
                    }
                }
            }
        }
        for list in [&mut self.pred, &mut self.succ] {
            let mut write = 0;
            for v in 0..g.rank.len() {
                work.visit(1)?;
                let (start, end) = (list.start[v] as usize, list.start[v + 1] as usize);
                list.start[v] = write as u32;
                work.visit(end - start)?;
                for read in start..end {
                    let edge = list.entries[read];
                    if keep[edge as usize] {
                        list.entries[write] = edge;
                        write += 1;
                    }
                }
            }
            *list.start.last_mut().unwrap() = write as u32;
            list.entries.truncate(write);
        }
        Ok(())
    }
}

fn inner(g: &Expanded, v: u32) -> Option<u32> {
    if !g.kind[v as usize].interior() {
        return None;
    }
    let &e = g.pred.at(v).first()?;
    g.kind[g.from[e as usize] as usize].interior().then_some(e)
}

/// Prefix maxima on occupied slots; zero denotes an empty prefix.
fn prefix(tree: &[u32], mut end: usize) -> u32 {
    let mut high = 0;
    while end > 0 {
        high = high.max(tree[end]);
        end &= end - 1;
    }
    high
}

fn insert(tree: &mut [u32], slot: usize, value: u32) {
    let mut i = slot + 1;
    while i < tree.len() {
        tree[i] = tree[i].max(value);
        i = i.saturating_add(i & i.wrapping_neg());
    }
}

/// All inner segments span one rank gap. Keep the shorter lateral span, then the earlier
/// occurrence. In the usual monotone gap neither sorting nor these trees is needed.
fn guard_inner(
    g: &Expanded,
    pos: &[u32],
    row: &[u32],
    anchors: &mut [u32],
    blocked: &mut [bool],
    [left, right]: &mut [Vec<u32>; 2],
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let slots = |e: u32| {
        (
            pos[g.from[e as usize] as usize],
            pos[g.to[e as usize] as usize],
        )
    };
    validate::charge_sort(anchors.len(), work)?;
    anchors.sort_unstable_by_key(|&e| {
        let (a, b) = slots(e);
        (a.abs_diff(b), e)
    });
    let len = row
        .len()
        .checked_add(1)
        .ok_or(WorkError::ArithmeticOverflow)?;
    work.charge(len.checked_mul(2).ok_or(WorkError::ArithmeticOverflow)?)?;
    for tree in [&mut *left, &mut *right] {
        clear_reserve(tree, len)?;
        tree.resize(len, 0);
    }
    for &e in anchors.iter() {
        work.visit(4 * (len.ilog2() as usize + 1))?;
        let (north, south) = slots(e);
        let at = south as usize;
        let rev = row.len() - 1 - at;
        let key = north + 1;
        let reverse_key = u32::MAX - north;
        if prefix(left, at) > key || prefix(right, rev) > reverse_key {
            blocked[e as usize] = true;
        } else {
            insert(left, at, key);
            insert(right, rev, reverse_key);
        }
    }
    Ok(())
}

#[derive(Debug, Default)]
struct Conflicts {
    blocked: Vec<bool>,
    anchors: Vec<u32>,
    trees: [Vec<u32>; 2],
}

fn find_conflicts(
    g: &Expanded,
    pos: &[u32],
    Conflicts {
        blocked,
        anchors,
        trees,
    }: &mut Conflicts,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    work.charge(g.from.len())?;
    clear_reserve(blocked, g.from.len())?;
    blocked.resize(g.from.len(), false);
    for row in g.layers.iter().skip(1) {
        anchors.clear();
        let mut crossed = false;
        // A non-inner edge crosses an inner to its left if its upper endpoint lies to
        // the left of the largest upper endpoint seen. The reverse scan tests its right.
        for reverse in [false, true] {
            let mut boundary = None::<u32>;
            for i in 0..row.len() {
                work.visit(1)?;
                let v = row[if reverse { row.len() - 1 - i } else { i }];
                if let Some(e) = inner(g, v) {
                    let up = pos[g.from[e as usize] as usize];
                    if !reverse {
                        crossed |= boundary.is_some_and(|before| before > up);
                        crate::work::push(anchors, e)?;
                    }
                    boundary = Some(
                        boundary.map_or(up, |old| if reverse { old.min(up) } else { old.max(up) }),
                    );
                } else if let Some(limit) = boundary {
                    for &e in g.pred.at(v) {
                        work.visit(1)?;
                        let up = pos[g.from[e as usize] as usize];
                        if if reverse { up > limit } else { up < limit } {
                            blocked[e as usize] = true;
                        }
                    }
                }
            }
        }
        if crossed {
            guard_inner(g, pos, row, anchors, blocked, trees, work)?;
        }
    }
    Ok(())
}

#[cfg(test)]
fn conflicts(g: &Expanded, pos: &[u32], work: &mut Work<'_>) -> LayoutResult<Vec<bool>> {
    let mut scratch = Conflicts::default();
    find_conflicts(g, pos, &mut scratch, work)?;
    Ok(scratch.blocked)
}

struct Sweep<'a> {
    pos: &'a [u32],
    blocked: &'a [bool],
    lists: &'a Csr,
    peer: &'a [u32],
    own_offset: &'a [f64],
    peer_offset: &'a [f64],
}

#[derive(Debug, Default)]
struct Blocks {
    root: Vec<u32>,
    shift: Vec<f64>,
}

impl Sweep<'_> {
    fn align(
        &self,
        layers: &[Vec<u32>],
        top_down: bool,
        right: bool,
        Blocks { root, shift }: &mut Blocks,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        let n = self.pos.len();
        work.charge(n.checked_mul(2).ok_or(WorkError::ArithmeticOverflow)?)?;
        clear_reserve(root, n)?;
        root.extend(0..n as u32);
        clear_reserve(shift, n)?;
        shift.resize(n, 0.0);
        for i in 0..layers.len() {
            let row = &layers[if top_down { i } else { layers.len() - 1 - i }];
            let mut previous = None;
            for j in 0..row.len() {
                work.visit(1)?;
                let v = row[if right { row.len() - 1 - j } else { j }];
                let list = self.lists.at(v);
                if list.is_empty() {
                    continue;
                }
                let (lo, hi) = ((list.len() - 1) / 2, list.len() / 2);
                let medians = if right { [hi, lo] } else { [lo, hi] };
                for &median in &medians[..1 + usize::from(lo != hi)] {
                    let e = list[median] as usize;
                    let other = self.peer[e] as usize;
                    let place = self.pos[other];
                    if self.blocked[e]
                        || previous
                            .is_some_and(|prev| if right { place >= prev } else { place <= prev })
                    {
                        continue;
                    }
                    let v = v as usize;
                    root[v] = root[other];
                    shift[v] = finite(shift[other] + (self.peer_offset[e] - self.own_offset[e]))?;
                    previous = Some(place);
                    break;
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct Constraint {
    from: u32,
    to: u32,
    gap: f64,
}

#[derive(Debug, Default)]
struct Compaction {
    edges: Vec<Constraint>,
    degree: Vec<u32>,
    outgoing: Csr,
    topo: Vec<u32>,
    xs: Vec<f64>,
}

fn compact_into(
    g: &Expanded,
    ext: &[(f64, f64)],
    blocks: &Blocks,
    right: bool,
    Compaction {
        edges,
        degree,
        outgoing,
        topo,
        xs,
    }: &mut Compaction,
    result: &mut Vec<f64>,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let n = g.rank.len();
    work.charge(n.checked_mul(4).ok_or(WorkError::ArithmeticOverflow)?)?;
    clear_reserve(edges, n)?;
    clear_reserve(degree, n)?;
    degree.resize(n, 0);
    let sign = if right { -1.0 } else { 1.0 };
    for row in &g.layers {
        for pair in row.windows(2) {
            work.visit(1)?;
            let (l, r) = (pair[0] as usize, pair[1] as usize);
            let (a, b) = if right { (r, l) } else { (l, r) };
            let gap = finite(ext[l].1 + ext[r].0 + sign * (blocks.shift[a] - blocks.shift[b]))?;
            let to = blocks.root[b];
            edges.push(Constraint {
                from: blocks.root[a],
                to,
                gap,
            });
            degree[to as usize] += 1;
        }
    }
    outgoing.rebuild(n, edges.iter().map(|e| e.from), work)?;
    clear_reserve(topo, n)?;
    let mut roots = 0;
    for (i, &root) in blocks.root.iter().enumerate() {
        work.visit(1)?;
        if root == i as u32 {
            roots += 1;
            if degree[i] == 0 {
                topo.push(root);
            }
        }
    }
    clear_reserve(xs, n)?;
    xs.resize(n, 0.0);
    let mut next = 0;
    while next < topo.len() {
        let v = topo[next];
        work.visit(1)?;
        for &e in outgoing.at(v) {
            work.visit(1)?;
            let edge = edges[e as usize];
            let w = edge.to as usize;
            xs[w] = xs[w].max(finite(xs[v as usize] + edge.gap)?);
            degree[w] -= 1;
            if degree[w] == 0 {
                topo.push(edge.to);
            }
        }
        next += 1;
    }
    if roots != topo.len() {
        return Err(LayoutError::InvalidInput(
            "cyclic horizontal block constraints",
        ));
    }
    for &v in topo.iter().rev() {
        work.visit(1)?;
        let mut limit = f64::INFINITY;
        for &e in outgoing.at(v) {
            work.visit(1)?;
            let edge = edges[e as usize];
            limit = limit.min(finite(xs[edge.to as usize] - edge.gap)?);
        }
        if limit.is_finite() {
            xs[v as usize] = xs[v as usize].max(limit);
        }
    }
    clear_reserve(result, n)?;
    for (v, &root) in blocks.root.iter().enumerate() {
        work.visit(1)?;
        result.push(finite(sign * xs[root as usize] + blocks.shift[v])?);
    }
    Ok(())
}

#[cfg(test)]
fn compact(
    g: &Expanded,
    ext: &[(f64, f64)],
    blocks: &Blocks,
    right: bool,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<f64>> {
    let mut result = Vec::new();
    compact_into(
        g,
        ext,
        blocks,
        right,
        &mut Compaction::default(),
        &mut result,
        work,
    )?;
    Ok(result)
}

fn middle([a, b, c, d]: [f64; 4]) -> f64 {
    let (a, b) = (a.min(b), a.max(b));
    let (c, d) = (c.min(d), c.max(d));
    let (b, c) = (b.min(d), a.max(c));
    b.min(c).midpoint(b.max(c))
}

#[derive(Debug, Default)]
pub(super) struct PositionScratch {
    pos: Vec<u32>,
    ext: Vec<(f64, f64)>,
    lists: Neighbours,
    ports: Ports,
    conflicts: Conflicts,
    blocks: Blocks,
    compaction: Compaction,
    alternatives: [Vec<f64>; 3],
}

impl PositionScratch {
    pub(super) fn retained_bytes(&self) -> usize {
        let csr = |list: &Csr| capacity_bytes(&list.start) + capacity_bytes(&list.entries);
        let [left, right] = &self.conflicts.trees;
        capacity_bytes(&self.pos)
            + capacity_bytes(&self.ext)
            + csr(&self.lists.pred)
            + csr(&self.lists.succ)
            + capacity_bytes(&self.ports.source)
            + capacity_bytes(&self.ports.target)
            + capacity_bytes(&self.ports.starts)
            + capacity_bytes(&self.ports.ends)
            + capacity_bytes(&self.conflicts.blocked)
            + capacity_bytes(&self.conflicts.anchors)
            + capacity_bytes(left)
            + capacity_bytes(right)
            + capacity_bytes(&self.blocks.root)
            + capacity_bytes(&self.blocks.shift)
            + capacity_bytes(&self.compaction.edges)
            + capacity_bytes(&self.compaction.degree)
            + csr(&self.compaction.outgoing)
            + capacity_bytes(&self.compaction.topo)
            + capacity_bytes(&self.compaction.xs)
            + self.alternatives.iter().map(capacity_bytes).sum::<usize>()
    }
}

pub(super) fn assign(
    g: &mut Expanded,
    c: &Component,
    spread_ends: Option<&Orientation>,
    graph: &Graph,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<f64>> {
    work.check()?;
    if g.rank.is_empty() {
        return Ok(Vec::new());
    }
    let mut scratch = std::mem::take(&mut g.position_scratch);
    let result = assign_with_scratch(g, c, spread_ends, graph, options, &mut scratch, work);
    // Keep both allocations for the next solve. Offsets become visible only after balance succeeds.
    if result.is_ok() && spread_ends.is_some() {
        std::mem::swap(&mut g.source_offset, &mut scratch.ports.source);
        std::mem::swap(&mut g.target_offset, &mut scratch.ports.target);
    }
    g.position_scratch = scratch;
    result
}

fn assign_with_scratch(
    g: &Expanded,
    c: &Component,
    spread_ends: Option<&Orientation>,
    graph: &Graph,
    options: &Options,
    PositionScratch {
        pos,
        ext,
        lists,
        ports,
        conflicts,
        blocks,
        compaction,
        alternatives: [second, third, fourth],
    }: &mut PositionScratch,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<f64>> {
    let n = g.rank.len();
    work.charge(n.checked_mul(2).ok_or(WorkError::ArithmeticOverflow)?)?;
    clear_reserve(pos, n)?;
    pos.resize(n, 0);
    clear_reserve(ext, n)?;
    ext.resize(n, (0.0, 0.0));
    for row in &g.layers {
        for (index, &v) in row.iter().enumerate() {
            work.visit(1)?;
            pos[v as usize] = index as u32;
            let (left, right) = extents(g, c, options, v);
            ext[v as usize] = (finite(left)?, finite(right)?);
        }
    }
    by_peer(g, &g.pred, &g.succ, &g.to, &mut lists.pred, work)?;
    by_peer(g, &g.succ, &g.pred, &g.from, &mut lists.succ, work)?;
    let (source, target) = if let Some(o) = spread_ends {
        spread(g, (c, o), graph, &options.spacing, lists, ports, work)?;
        lists.prefer_spine(g, o, work)?;
        (&ports.source, &ports.target)
    } else {
        (&g.source_offset, &g.target_offset)
    };
    find_conflicts(g, pos, conflicts, work)?;
    let mut result = Vec::new();
    let mut layouts = [&mut result, second, third, fourth];
    let mut bounds = [(0.0, 0.0); 4];
    let mut narrow = 0;
    let mut best = f64::INFINITY;
    for (k, layout) in layouts.iter_mut().enumerate() {
        let top_down = k < 2;
        let right = k % 2 != 0;
        let (list, peer, own, other) = if top_down {
            (&lists.pred, &g.from, target, source)
        } else {
            (&lists.succ, &g.to, source, target)
        };
        let sweep = Sweep {
            pos,
            blocked: &conflicts.blocked,
            lists: list,
            peer,
            own_offset: own,
            peer_offset: other,
        };
        sweep.align(&g.layers, top_down, right, blocks, work)?;
        compact_into(g, ext, blocks, right, compaction, layout, work)?;
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        for (v, &x) in layout.iter().enumerate() {
            work.visit(1)?;
            let half = g.width[v] * 0.5;
            let (left, right) = if g.kind[v] == Kind::Node {
                (c.margin_left[v], c.margin_right[v])
            } else {
                (0.0, 0.0)
            };
            lo = lo.min(finite(x - (half + left))?);
            hi = hi.max(finite(x + (half + right))?);
        }
        bounds[k] = (lo, hi);
        let width = finite(hi - lo)?;
        if width < best {
            best = width;
            narrow = k;
        }
    }
    let delta: [f64; 4] = std::array::from_fn(|k| {
        if k % 2 == 0 {
            bounds[narrow].0 - bounds[k].0
        } else {
            bounds[narrow].1 - bounds[k].1
        }
    });
    for v in 0..n {
        work.visit(1)?;
        let candidates = std::array::from_fn(|k| layouts[k][v] + delta[k]);
        if !candidates.iter().all(|x| x.is_finite()) {
            return Err(LayoutError::InvalidInput(
                "horizontal balance exceeds finite precision",
            ));
        }
        layouts[0][v] = middle(candidates);
    }
    work.finish()?;
    Ok(result)
}

#[cfg(test)]
mod scratch_tests;
#[cfg(test)]
mod tests;
