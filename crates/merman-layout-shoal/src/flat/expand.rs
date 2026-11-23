//! Build the proper layered graph over occupied ranks. Numeric gaps stay in `logical_rank`.

use super::acyclic::Orientation;
use super::{Expanded, Kind, Link, canonical_size, rank};
use crate::work::{LayoutResult, Work, filled, reserved};
use crate::{Graph, LayoutError, NodeSpec, Options, PortSideHint, Size, WorkError};
use std::borrow::Cow;
use std::collections::HashSet;

fn add(a: usize, b: usize) -> LayoutResult<usize> {
    a.checked_add(b).ok_or(WorkError::ArithmeticOverflow.into())
}

fn count(value: usize) -> LayoutResult<u32> {
    u32::try_from(value).map_err(|_| WorkError::ArithmeticOverflow.into())
}

fn clear_reserve<T>(vec: &mut Vec<T>, len: usize) -> LayoutResult<()> {
    vec.clear();
    vec.try_reserve_exact(len)
        .map_err(|_| LayoutError::AllocationLimit)
}

fn rows(vec: &mut Vec<Vec<u32>>, len: usize) -> LayoutResult<()> {
    vec.truncate(len);
    vec.try_reserve_exact(len - vec.len())
        .map_err(|_| LayoutError::AllocationLimit)?;
    vec.resize_with(len, Vec::new);
    Ok(())
}

fn oriented<'a>(
    links: &'a [Link],
    rev: &[bool],
    work: &mut Work<'_>,
) -> LayoutResult<Cow<'a, [Link]>> {
    work.charge(links.len())?;
    if !rev.contains(&true) {
        return Ok(Cow::Borrowed(links));
    }
    let mut arcs = reserved(links.len())?;
    for (&link, &back) in links.iter().zip(rev) {
        work.visit(1)?;
        arcs.push(if back {
            Link {
                from: link.to,
                to: link.from,
                ..link
            }
        } else {
            link
        });
    }
    Ok(Cow::Owned(arcs))
}

/// Insert one shared obstacle tier only in a gap between consecutive occupied node ranks.
/// An existing empty logical slot can host it without changing any distance downstream.
fn sublayers(
    raw: &[u64],
    arcs: &[Link],
    graph: &Graph,
    g: &mut Expanded,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<u32>> {
    let n = raw.len();
    work.charge(n.checked_mul(3).ok_or(WorkError::ArithmeticOverflow)?)?;
    let mut order = reserved(n)?;
    order.extend(0..n);
    work.sort(&mut order, |&a, &b| raw[a].cmp(&raw[b]).then(a.cmp(&b)))?;
    let mut occupied = reserved(n)?;
    clear_reserve(&mut g.rank, n)?;
    g.rank.resize(n, 0);
    for v in order {
        work.visit(1)?;
        if occupied.last() != Some(&raw[v]) {
            occupied.push(raw[v]);
        }
        g.rank[v] = count(occupied.len() - 1)?;
    }
    work.charge(add(occupied.len(), arcs.len())?)?;
    let mut split = filled(occupied.len(), false)?;
    let mut pairs = HashSet::new();
    pairs
        .try_reserve(arcs.len())
        .map_err(|_| LayoutError::AllocationLimit)?;
    for edge in arcs {
        work.visit(1)?;
        let spec = graph
            .edges
            .get(edge.edge as usize)
            .ok_or(LayoutError::InvalidInput("invalid global edge occurrence"))?;
        let (a, b) = (g.rank[edge.from as usize], g.rank[edge.to as usize]);
        if b <= a {
            return Err(LayoutError::InvalidInput("edge opposes its ranking order"));
        }
        if b - a == 1 && !split[a as usize] {
            split[a as usize] = spec.label.is_some() || !pairs.insert((edge.from, edge.to));
        }
    }
    let levels = add(occupied.len(), split.iter().filter(|&&yes| yes).count())?;
    count(levels)?;
    work.charge(add(levels, occupied.len())?)?;
    clear_reserve(&mut g.logical_rank, levels)?;
    let mut real = reserved(occupied.len())?;
    let mut extra = 0_u64;
    for (i, &r) in occupied.iter().enumerate() {
        work.visit(1)?;
        let start = r.checked_add(extra).ok_or(WorkError::ArithmeticOverflow)?;
        real.push(count(g.logical_rank.len())?);
        g.logical_rank.push(start);
        if split[i] {
            let gap = occupied[i + 1] - r;
            if gap == 1 {
                extra += 1;
            }
            g.logical_rank.push(
                start
                    .checked_add((gap / 2).max(1))
                    .ok_or(WorkError::ArithmeticOverflow)?,
            );
        }
    }
    for r in &mut g.rank {
        work.visit(1)?;
        *r = real[*r as usize];
    }
    Ok(real)
}

fn dummy_budget(
    arcs: &[Link],
    g: &mut Expanded,
    cap: usize,
    work: &mut Work<'_>,
) -> LayoutResult<(usize, usize)> {
    let m = arcs.len();
    work.charge(m)?;
    clear_reserve(&mut g.direct, m)?;
    g.direct.resize(m, false);
    let interior = |e: &Link| g.rank[e.to as usize] - g.rank[e.from as usize] - 1;
    // Both the occurrence count and the dense rank count are bounded by u32 before this sum.
    let mut total = 0_u64;
    for e in arcs {
        work.visit(1)?;
        total += u64::from(interior(e));
    }
    let mut retained = m;
    if total > cap as u64 {
        work.charge(m)?;
        let mut longest = reserved(m)?;
        longest.extend((0..m).filter(|&i| interior(&arcs[i]) != 0));
        work.sort(&mut longest, |&a, &b| {
            interior(&arcs[b])
                .cmp(&interior(&arcs[a]))
                .then(arcs[a].edge.cmp(&arcs[b].edge))
        })?;
        for i in longest {
            work.visit(1)?;
            if total <= cap as u64 {
                break;
            }
            g.direct[i] = true;
            total -= u64::from(interior(&arcs[i]));
            retained -= 1;
        }
    }
    let dummies = usize::try_from(total).map_err(|_| WorkError::ArithmeticOverflow)?;
    Ok((dummies, add(retained, dummies)?))
}

/// Count layer occupancy by interval differences before allocating the member arrays. If every
/// edge that needed a sublayer was downgraded, remove that empty row from the dense indices.
fn prepare_layers(
    arcs: &[Link],
    real: &mut [u32],
    g: &mut Expanded,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let len = g.logical_rank.len();
    work.charge(len.checked_mul(3).ok_or(WorkError::ArithmeticOverflow)?)?;
    let mut delta = filled(len, 0_i64)?;
    let mut members = filled(len, 0_usize)?;
    let mut dense = filled(len, 0_u32)?;
    for &r in &g.rank {
        work.visit(1)?;
        members[r as usize] += 1;
    }
    for (i, e) in arcs.iter().enumerate() {
        work.visit(1)?;
        if !g.direct[i] {
            let (a, b) = (
                g.rank[e.from as usize] as usize,
                g.rank[e.to as usize] as usize,
            );
            delta[a + 1] += 1;
            delta[b] -= 1;
        }
    }
    let mut running = 0_i64;
    let mut used = 0;
    for i in 0..len {
        work.visit(1)?;
        running += delta[i];
        let size = add(members[i], running as usize)?;
        if size > 0 {
            dense[i] = count(used)?;
            members[used] = size;
            g.logical_rank[used] = g.logical_rank[i];
            used += 1;
        }
    }
    g.logical_rank.truncate(used);
    rows(&mut g.layers, used)?;
    for (layer, size) in g.layers.iter_mut().zip(members) {
        clear_reserve(layer, size)?;
    }
    for r in g.rank.iter_mut().chain(real) {
        work.visit(1)?;
        *r = dense[*r as usize];
    }
    Ok(())
}

fn label_layer(
    a: u32,
    b: u32,
    real: &[u32],
    logical: &[u64],
    reversed: bool,
    start: f64,
    end: f64,
) -> u32 {
    if b - a == 2 {
        return a + 1;
    }
    let candidates = &real[real.partition_point(|&r| r <= a)..real.partition_point(|&r| r < b)];
    let (lo, hi) = (logical[a as usize], logical[b as usize]);
    let middle = lo + (hi - lo) / 2;
    let middle = if end > start {
        middle.saturating_sub(1)
    } else if start > end {
        middle + 1
    } else {
        middle
    };
    let i = candidates.partition_point(|&r| logical[r as usize] < middle);
    match (
        i.checked_sub(1).map(|j| candidates[j]),
        candidates.get(i).copied(),
    ) {
        (Some(x), Some(y)) => {
            let left = logical[x as usize].abs_diff(middle);
            let right = logical[y as usize].abs_diff(middle);
            if left < right || left == right && !reversed {
                x
            } else {
                y
            }
        }
        (Some(r), None) | (None, Some(r)) => r,
        (None, None) => a + 1,
    }
}

fn offset(port: PortSideHint) -> f64 {
    match port {
        PortSideHint::FixedOffset { offset, .. } => offset,
        _ => 0.0,
    }
}

pub(super) fn build(
    nodes: &[NodeSpec],
    links: &[Link],
    orientation: &Orientation,
    graph: &Graph,
    options: &Options,
    work: &mut Work<'_>,
    g: &mut Expanded,
) -> LayoutResult<()> {
    let n = nodes.len();
    let m = links.len();
    count(n)?;
    count(m)?;
    if orientation.topo.len() != n || orientation.reversed.len() != m {
        return Err(LayoutError::InvalidInput("incomplete edge orientation"));
    }
    let arcs = oriented(links, &orientation.reversed, work)?;
    let raw = rank::assign(n, &arcs, &orientation.topo, options.rank_policy, work)?;
    let mut real = sublayers(&raw, &arcs, graph, g, work)?;
    let (dummies, segments) = dummy_budget(&arcs, g, options.caps.max_dummies, work)?;
    let vertices = add(n, dummies)?;
    count(vertices)?;
    count(segments)?;
    // Admit all SoA columns, the nested memberships and the complete chains before growing them.
    let vertex_units = vertices
        .checked_mul(5)
        .ok_or(WorkError::ArithmeticOverflow)?;
    let segment_units = segments
        .checked_mul(5)
        .ok_or(WorkError::ArithmeticOverflow)?;
    let edge_units = add(
        m.checked_mul(4).ok_or(WorkError::ArithmeticOverflow)?,
        dummies,
    )?;
    work.charge(add(add(vertex_units, segment_units)?, edge_units)?)?;
    prepare_layers(&arcs, &mut real, g, work)?;
    g.rank
        .try_reserve_exact(dummies)
        .map_err(|_| LayoutError::AllocationLimit)?;
    clear_reserve(&mut g.width, vertices)?;
    clear_reserve(&mut g.height, vertices)?;
    clear_reserve(&mut g.kind, vertices)?;
    clear_reserve(&mut g.from, segments)?;
    clear_reserve(&mut g.to, segments)?;
    clear_reserve(&mut g.weight, segments)?;
    clear_reserve(&mut g.source_offset, segments)?;
    clear_reserve(&mut g.target_offset, segments)?;
    clear_reserve(&mut g.label_vertex, m)?;
    rows(&mut g.chains, m)?;
    for (v, node) in nodes.iter().enumerate() {
        work.visit(1)?;
        let size = canonical_size(graph.direction, node.size);
        g.width.push(size.width);
        g.height.push(size.height);
        g.kind.push(Kind::Node);
        g.layers[g.rank[v] as usize].push(v as u32);
    }
    for (i, e) in arcs.iter().enumerate() {
        work.visit(1)?;
        let (a, b) = (g.rank[e.from as usize], g.rank[e.to as usize]);
        let chain = &mut g.chains[i];
        if g.direct[i] {
            clear_reserve(chain, 2)?;
            chain.extend([e.from, e.to]);
            g.label_vertex.push(None);
            continue;
        }
        clear_reserve(chain, add((b - a) as usize, 1)?)?;
        chain.push(e.from);
        let spec = &graph.edges[e.edge as usize];
        let reversed = orientation.reversed[i];
        let (mut source_port, mut target_port) = (spec.source_port, spec.target_port);
        let (mut start, mut end) = (spec.start_marker.length, spec.end_marker.length);
        if reversed {
            std::mem::swap(&mut source_port, &mut target_port);
            std::mem::swap(&mut start, &mut end);
        }
        let mut label_vertex = None;
        let label = if let Some(size) = spec.label {
            work.visit(3 * (real.len().ilog2() as usize + 1))?;
            Some((
                label_layer(a, b, &real, &g.logical_rank, reversed, start, end),
                canonical_size(graph.direction, size),
            ))
        } else {
            None
        };
        for r in a + 1..b {
            work.visit(1)?;
            let v = g.rank.len() as u32;
            let (kind, size) = match label {
                Some((chosen, size)) if chosen == r => {
                    label_vertex = Some(v);
                    (Kind::Label, size)
                }
                _ => (Kind::Dummy, Size::default()),
            };
            g.rank.push(r);
            g.width.push(size.width);
            g.height.push(size.height);
            g.kind.push(kind);
            g.layers[r as usize].push(v);
            chain.push(v);
        }
        chain.push(e.to);
        g.label_vertex.push(label_vertex);
        for pair in chain.windows(2) {
            work.visit(1)?;
            g.from.push(pair[0]);
            g.to.push(pair[1]);
            g.weight.push(e.weight);
            g.source_offset.push(if pair[0] == e.from {
                offset(source_port)
            } else {
                0.0
            });
            g.target_offset.push(if pair[1] == e.to {
                offset(target_port)
            } else {
                0.0
            });
        }
    }
    g.pred.rebuild(vertices, g.to.iter().copied(), work)?;
    g.succ.rebuild(vertices, g.from.iter().copied(), work)
}
