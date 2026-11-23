//! Port spreading and four-way Brandes-Kopf coordinate assignment on the ordered ranks.

use super::{Component, Csr, Expanded, Kind, acyclic::Orientation, to_canonical_port, validate};
use crate::work::{LayoutResult, Work, filled, reserved};
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
    work: &mut Work<'_>,
) -> LayoutResult<Csr> {
    work.charge(ends.start.len())?;
    work.charge(vertex.len())?;
    let mut start = reserved(ends.start.len())?;
    start.extend_from_slice(&ends.start);
    let mut entries = filled(vertex.len(), 0)?;
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
    Ok(Csr { start, entries })
}

#[derive(Clone, Copy, PartialEq, Eq)]
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

struct Ports {
    source: Vec<f64>,
    target: Vec<f64>,
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
    c: &Component,
    o: &Orientation,
    graph: &Graph,
    spacing: &Spacing,
    lists: &Neighbours,
    work: &mut Work<'_>,
) -> LayoutResult<Ports> {
    let m = g.from.len();
    work.charge(m.checked_mul(4).ok_or(WorkError::ArithmeticOverflow)?)?;
    let mut ports = Ports {
        source: filled(m, 0.0)?,
        target: filled(m, 0.0)?,
    };
    let mut starts = filled(m, End::Held)?;
    let mut ends = filled(m, End::Held)?;
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
                &mut ports.source[first],
            ),
            (
                g.to[last],
                Side::Top,
                head,
                head_role,
                &mut ends[last],
                &mut ports.target[last],
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
        spread_face(
            lists.succ.at(v as u32),
            &starts,
            &mut ports.source,
            span,
            spacing,
            work,
        )?;
        spread_face(
            lists.pred.at(v as u32),
            &ends,
            &mut ports.target,
            span,
            spacing,
            work,
        )?;
    }
    Ok(ports)
}

struct Neighbours {
    pred: Csr,
    succ: Csr,
}

fn inner(g: &Expanded, v: u32) -> Option<u32> {
    if g.kind[v as usize] == Kind::Node {
        return None;
    }
    let &e = g.pred.at(v).first()?;
    (g.kind[g.from[e as usize] as usize] != Kind::Node).then_some(e)
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
    let mut left = filled(len, 0u32)?;
    let mut right = filled(len, 0u32)?;
    for &e in anchors.iter() {
        work.visit(4 * (len.ilog2() as usize + 1))?;
        let (north, south) = slots(e);
        let at = south as usize;
        let rev = row.len() - 1 - at;
        let key = north + 1;
        let reverse_key = u32::MAX - north;
        if prefix(&left, at) > key || prefix(&right, rev) > reverse_key {
            blocked[e as usize] = true;
        } else {
            insert(&mut left, at, key);
            insert(&mut right, rev, reverse_key);
        }
    }
    Ok(())
}

fn conflicts(g: &Expanded, pos: &[u32], work: &mut Work<'_>) -> LayoutResult<Vec<bool>> {
    work.charge(g.from.len())?;
    let mut blocked = filled(g.from.len(), false)?;
    let mut anchors = Vec::new();
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
                        crate::work::push(&mut anchors, e)?;
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
            guard_inner(g, pos, row, &mut anchors, &mut blocked, work)?;
        }
    }
    Ok(blocked)
}

struct Sweep<'a> {
    pos: &'a [u32],
    blocked: &'a [bool],
    lists: &'a Csr,
    peer: &'a [u32],
    own_offset: &'a [f64],
    peer_offset: &'a [f64],
}

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
        work: &mut Work<'_>,
    ) -> LayoutResult<Blocks> {
        let n = self.pos.len();
        work.charge(n.checked_mul(2).ok_or(WorkError::ArithmeticOverflow)?)?;
        let mut root = reserved(n)?;
        root.extend(0..n as u32);
        let mut shift = filled(n, 0.0)?;
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
        Ok(Blocks { root, shift })
    }
}

#[derive(Clone, Copy)]
struct Constraint {
    from: u32,
    to: u32,
    gap: f64,
}

fn compact(
    g: &Expanded,
    ext: &[(f64, f64)],
    blocks: &Blocks,
    right: bool,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<f64>> {
    let n = g.rank.len();
    work.charge(n.checked_mul(4).ok_or(WorkError::ArithmeticOverflow)?)?;
    let mut edges = reserved(n)?;
    let mut degree = filled(n, 0u32)?;
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
    let outgoing = Csr::new(n, edges.iter().map(|e| e.from), work)?;
    let mut topo = reserved(n)?;
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
    let mut xs = filled(n, 0.0_f64)?;
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
    let mut result = reserved(n)?;
    for (v, &root) in blocks.root.iter().enumerate() {
        work.visit(1)?;
        result.push(finite(sign * xs[root as usize] + blocks.shift[v])?);
    }
    Ok(result)
}

fn middle([a, b, c, d]: [f64; 4]) -> f64 {
    let (a, b) = (a.min(b), a.max(b));
    let (c, d) = (c.min(d), c.max(d));
    let (b, c) = (b.min(d), a.max(c));
    b.min(c).midpoint(b.max(c))
}

pub(super) fn assign(
    g: &mut Expanded,
    c: &Component,
    orientation: &Orientation,
    graph: &Graph,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<f64>> {
    work.check()?;
    let n = g.rank.len();
    if n == 0 {
        return Ok(Vec::new());
    }
    work.charge(n.checked_mul(2).ok_or(WorkError::ArithmeticOverflow)?)?;
    let mut pos = filled(n, 0u32)?;
    let mut ext = filled(n, (0.0, 0.0))?;
    for row in &g.layers {
        for (index, &v) in row.iter().enumerate() {
            work.visit(1)?;
            pos[v as usize] = index as u32;
            let (left, right) = extents(g, c, options, v);
            ext[v as usize] = (finite(left)?, finite(right)?);
        }
    }
    let lists = Neighbours {
        pred: by_peer(g, &g.pred, &g.succ, &g.to, work)?,
        succ: by_peer(g, &g.succ, &g.pred, &g.from, work)?,
    };
    let ports = spread(g, c, orientation, graph, &options.spacing, &lists, work)?;
    let blocked = conflicts(g, &pos, work)?;
    let mut layouts: [Vec<f64>; 4] = Default::default();
    let mut bounds = [(0.0, 0.0); 4];
    let mut narrow = 0;
    let mut best = f64::INFINITY;
    for (k, layout) in layouts.iter_mut().enumerate() {
        let top_down = k < 2;
        let right = k % 2 != 0;
        let (list, peer, own, other) = if top_down {
            (&lists.pred, &g.from, &ports.target, &ports.source)
        } else {
            (&lists.succ, &g.to, &ports.source, &ports.target)
        };
        let sweep = Sweep {
            pos: &pos,
            blocked: &blocked,
            lists: list,
            peer,
            own_offset: own,
            peer_offset: other,
        };
        let blocks = sweep.align(&g.layers, top_down, right, work)?;
        *layout = compact(g, &ext, &blocks, right, work)?;
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
    g.source_offset = ports.source;
    g.target_offset = ports.target;
    Ok(std::mem::take(&mut layouts[0]))
}

#[cfg(test)]
mod tests;
