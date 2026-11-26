//! Build the proper layered graph over occupied ranks. Numeric gaps stay in `logical_rank`.

mod proxy;

use super::acyclic::Orientation;
use super::{BoundaryRank, Component, Expanded, Kind, Link, canonical_size, label_place, rank};
use crate::work::{LayoutResult, Work, filled, push, reserved};
use crate::{Graph, LayoutError, Options, PortSideHint, Size, WorkError};
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

pub(super) fn oriented<'a>(
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
        arcs.push(if back { link.flipped() } else { link });
    }
    Ok(Cow::Owned(arcs))
}

/// Insert one shared obstacle tier only in a gap between consecutive occupied node ranks.
/// An existing empty logical slot can host it without changing any distance downstream.
fn sublayers(
    raw: &[u64],
    arcs: &[Link],
    graph: &Graph,
    proxies: Option<proxy::ProxyPorts<'_>>,
    inline: Option<label_place::GapPolicy<'_>>,
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
    g.separated_proxies = match proxies {
        Some(proxies) => proxies.separated(arcs, raw, graph, work)?,
        None => Vec::new(),
    };
    work.charge(add(occupied.len(), arcs.len())?)?;
    let mut split = filled(occupied.len(), false)?;
    let mut label_counts = Vec::new();
    if let Some(policy) = &inline
        && !policy.restored.is_empty()
    {
        work.charge(occupied.len())?;
        label_counts = filled(occupied.len(), 0_u8)?;
        for &rank in policy.restored {
            work.visit(1)?;
            // A long edge's fallback can use an existing rank. Only a short edge
            // needs to insert a tier when inline placement in this gap fails.
            label_counts[rank as usize] = label_place::MAX_LABELS_IN_GAP;
        }
    }
    g.inline_labels.clear();
    g.has_parallel_links = false;
    let mut pairs = HashSet::new();
    pairs
        .try_reserve(arcs.len())
        .map_err(|_| LayoutError::AllocationLimit)?;
    for (index, edge) in arcs.iter().enumerate() {
        work.visit(1)?;
        let spec = graph
            .edges
            .get(edge.edge as usize)
            .ok_or(LayoutError::InvalidInput("invalid global edge occurrence"))?;
        let (a, b) = (g.rank[edge.from as usize], g.rank[edge.to as usize]);
        if b <= a {
            return Err(LayoutError::InvalidInput("edge opposes its ranking order"));
        }
        let adjacent = b - a == 1;
        let gap = (a + (b - a - 1) / 2) as usize;
        let ends = (edge.from, edge.to);
        let repeated = !pairs.insert(ends);
        g.has_parallel_links |= repeated;
        if let Some(size) = spec.label {
            if split[gap]
                || !inline.as_ref().is_some_and(|policy| {
                    policy.admits(size, spec, graph.direction, index, edge.from)
                })
                || occupied[gap + 1] - occupied[gap] != 1
                || g.inline_labels.len() == label_place::MAX_LABELS
            {
                split[gap] |= adjacent;
                continue;
            }
            if label_counts.is_empty() {
                work.charge(occupied.len())?;
                label_counts = filled(occupied.len(), 0_u8)?;
            }
            if label_counts[gap] == label_place::MAX_LABELS_IN_GAP {
                split[gap] |= adjacent;
                continue;
            }
            label_counts[gap] += 1;
            work.charge(1)?;
            push(
                &mut g.inline_labels,
                label_place::InlineLabel {
                    edge: index,
                    gap: gap as u32,
                },
            )?;
        }
        if adjacent && !split[gap] {
            split[gap] = repeated && !g.separate_ports(ends, work)?;
        }
    }
    if !g.inline_labels.is_empty() {
        work.charge(g.inline_labels.len())?;
        g.inline_labels.retain(|label| !split[label.gap as usize]);
    }
    let splits = split.iter().filter(|&&yes| yes).count();
    let levels = add(occupied.len(), splits)?;
    count(levels)?;
    work.charge(add(add(levels, occupied.len())?, splits)?)?;
    clear_reserve(&mut g.logical_rank, levels)?;
    clear_reserve(&mut g.inserted_ranks, splits)?;
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
                g.inserted_ranks.push(count(g.logical_rank.len())?);
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
    let mut dense = filled(len, u32::MAX)?;
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
    work.visit(g.inserted_ranks.len())?;
    g.inserted_ranks.retain_mut(|rank| {
        *rank = dense[*rank as usize];
        *rank != u32::MAX
    });
    debug_assert!(g.inserted_ranks.windows(2).all(|pair| pair[0] < pair[1]));
    rows(&mut g.layers, used)?;
    for (layer, size) in g.layers.iter_mut().zip(members) {
        clear_reserve(layer, size)?;
    }
    for r in g.rank.iter_mut().chain(real) {
        work.visit(1)?;
        *r = dense[*r as usize];
        debug_assert_ne!(*r, u32::MAX);
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
    component: &Component,
    orientation: &Orientation,
    graph: &Graph,
    options: &Options,
    restored_labels: &[u32],
    work: &mut Work<'_>,
    g: &mut Expanded,
) -> LayoutResult<()> {
    let nodes = component.nodes.as_slice();
    let links = component.links.as_slice();
    let n = nodes.len();
    let m = links.len();
    count(n)?;
    count(m)?;
    if orientation.topo.len() != n || orientation.reversed.len() != m {
        return Err(LayoutError::InvalidInput("incomplete edge orientation"));
    }
    let arcs = oriented(links, &orientation.reversed, work)?;
    let raw = rank::assign(
        n,
        &arcs,
        &orientation.topo,
        options.rank_policy,
        component.boundaries.then_some(nodes),
        work,
    )?;
    let proxies = component.group_proxies.then_some(proxy::ProxyPorts {
        nodes,
        reversed: &orientation.reversed,
        clearance: options.spacing.edge,
    });
    let inline = options.inline_labels
        && graph.groups.is_empty()
        && !component.boundaries
        && !component.pinned
        && !component.group_proxies;
    let mut real = sublayers(
        &raw,
        &arcs,
        graph,
        proxies,
        inline.then_some(label_place::GapPolicy {
            spacing: options.spacing,
            restored: restored_labels,
            nodes,
            reversed: &orientation.reversed,
            routing: options.routing,
        }),
        g,
        work,
    )?;
    let (dummies, segments) = dummy_budget(&arcs, g, options.caps.max_dummies, work)?;
    work.visit(g.inline_labels.len())?;
    g.inline_labels.retain(|label| !g.direct[label.edge]);
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
        g.kind.push(match node.boundary {
            None => Kind::Node,
            Some(BoundaryRank::Start) => Kind::StartPort,
            Some(BoundaryRank::End) => Kind::EndPort,
        });
        g.layers[g.rank[v] as usize].push(v as u32);
    }
    let mut inline = g.inline_labels.iter().map(|label| label.edge).peekable();
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
        let label = if let Some(size) = spec.label
            && inline.next_if_eq(&i).is_none()
        {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Direction, EdgeSpec, Endpoint, NodeId, NodeSpec, NoopWorkControl, Side, WorkControl,
    };

    fn build(
        nodes: &[NodeSpec],
        links: &[Link],
        orientation: &Orientation,
        graph: &Graph,
        options: &Options,
        work: &mut Work<'_>,
        g: &mut Expanded,
    ) -> LayoutResult<()> {
        let c = Component {
            nodes: nodes.to_vec(),
            links: links.to_vec(),
            ..Default::default()
        };
        super::build(&c, orientation, graph, options, &[], work, g)
    }

    fn shape(w: f64, h: f64) -> Size {
        Size {
            width: w,
            height: h,
        }
    }
    fn input(n: u32, pairs: &[(u32, u32, u32)]) -> (Graph, Vec<Link>, Orientation) {
        let mut graph = Graph {
            nodes: (0..n)
                .map(|i| NodeSpec::new(shape(12.0, 23.0), i))
                .collect(),
            ..Default::default()
        };
        let mut links = Vec::new();
        for (id, &(from, to, span)) in pairs.iter().enumerate() {
            let mut e = EdgeSpec::new(Endpoint::Node(NodeId(from)), Endpoint::Node(NodeId(to)));
            e.min_rank_span = span;
            links.push(Link {
                from,
                to,
                span,
                weight: e.weight,
                edge: id as u32,
            });
            graph.edges.push(e);
        }
        let order = Orientation {
            topo: (0..n).collect(),
            reversed: links.iter().map(|e| e.from > e.to).collect(),
            cyclic_sccs: 0,
        };
        (graph, links, order)
    }

    #[test]
    fn huge_minimum_spans_keep_their_u64_distances_and_a_single_label_obstacle() {
        let (mut graph, links, order) = input(3, &[(0, 1, u32::MAX), (1, 2, u32::MAX)]);
        graph.edges[0].label = Some(shape(5.0, 12.0));
        let mut g = Expanded::default();
        let mut options = Options::default();
        options.caps.max_dummies = usize::MAX;
        build(
            &graph.nodes,
            &links,
            &order,
            &graph,
            &options,
            &mut Work::new(&mut NoopWorkControl),
            &mut g,
        )
        .unwrap();
        let m = u64::from(u32::MAX);
        assert_eq!(g.logical_rank, [0, m / 2, m, 2 * m]);
        assert!(g.inserted_ranks.is_empty());
        assert_eq!(g.rank, [0, 2, 3, 1]);
        assert_eq!(g.kind, [Kind::Node, Kind::Node, Kind::Node, Kind::Label]);
        assert_eq!(g.chains, [vec![0, 3, 1], vec![1, 2]]);
    }

    #[test]
    fn antiparallel_occurrences_share_a_tier_and_keep_global_measurements_and_fixed_ports() {
        let (mut graph, mut links, order) = input(2, &[(1, 0, 1), (0, 1, 1)]);
        graph.direction = Direction::Left;
        let spec = &mut graph.edges[0];
        spec.source_port = PortSideHint::FixedOffset {
            side: Side::Left,
            offset: 3.5,
        };
        spec.target_port = PortSideHint::FixedOffset {
            side: Side::Right,
            offset: -2.0,
        };
        spec.label = Some(shape(31.0, 7.0));
        graph
            .edges
            .insert(0, EdgeSpec::new(NodeId(1).into(), NodeId(1).into()));
        for e in &mut links {
            e.edge += 1;
        }
        let mut g = Expanded::default();
        build(
            &graph.nodes,
            &links,
            &order,
            &graph,
            &Options::default(),
            &mut Work::new(&mut NoopWorkControl),
            &mut g,
        )
        .unwrap();
        assert_eq!(g.logical_rank, [0, 1, 2]);
        assert_eq!(g.inserted_ranks, [1]);
        assert_eq!(g.label_vertex, [Some(2), None]);
        assert_eq!(g.kind, [Kind::Node, Kind::Node, Kind::Label, Kind::Dummy]);
        assert_eq!((g.width[0], g.height[0]), (23.0, 12.0));
        assert_eq!((g.width[2], g.height[2]), (7.0, 31.0));
        assert_eq!(g.chains, [vec![0, 2, 1], vec![0, 3, 1]]);
        assert_eq!(g.source_offset, [-2.0, 0.0, 0.0, 0.0]);
        assert_eq!(g.target_offset, [0.0, 3.5, 0.0, 0.0]);
        assert_eq!(g.pred.at(1), [1, 3]);
        assert_eq!(g.succ.at(0), [0, 2]);
    }

    #[test]
    fn asymmetric_markers_select_existing_middle_ranks_in_the_ranked_direction() {
        let (mut graph, links, order) = input(
            6,
            &[
                (0, 1, 1),
                (1, 2, 1),
                (2, 3, 1),
                (3, 4, 1),
                (4, 5, 1),
                (0, 5, 1),
                (5, 0, 1),
                (5, 0, 1),
            ],
        );
        for spec in &mut graph.edges[5..] {
            spec.label = Some(shape(8.0, 4.0));
        }
        graph.edges[5].end_marker.length = 18.0;
        graph.edges[6].start_marker.length = 18.0;
        graph.edges[7].end_marker.length = 18.0;
        let mut g = Expanded::default();
        let options = Options {
            inline_labels: false,
            ..Options::default()
        };
        build(
            &graph.nodes,
            &links,
            &order,
            &graph,
            &options,
            &mut Work::new(&mut NoopWorkControl),
            &mut g,
        )
        .unwrap();
        assert_eq!(g.logical_rank, [0, 1, 2, 3, 4, 5]);
        for (edge, expected) in [(5, 1), (6, 1), (7, 3)] {
            let label = g.label_vertex[edge].unwrap();
            assert_eq!(g.rank[label as usize], expected);
            assert_eq!(g.chains[edge][expected as usize], label);
        }
    }

    #[test]
    fn labeled_long_links_set_and_reset_parallel_detection() {
        let (mut graph, links, mut order) =
            input(4, &[(0, 1, 1), (1, 2, 1), (2, 3, 1), (0, 3, 1), (3, 0, 1)]);
        for spec in &mut graph.edges[3..] {
            spec.label = Some(shape(8.0, 4.0));
        }
        let options = Options {
            inline_labels: false,
            ..Options::default()
        };
        let mut g = Expanded::default();
        for (links, expected) in [(links.as_slice(), true), (&links[..4], false)] {
            order.reversed.truncate(links.len());
            build(
                &graph.nodes,
                links,
                &order,
                &graph,
                &options,
                &mut Work::new(&mut NoopWorkControl),
                &mut g,
            )
            .unwrap();
            assert_eq!(g.has_parallel_links, expected);
            assert_eq!(g.logical_rank, [0, 1, 2, 3]);
        }
    }

    #[test]
    fn cap_removes_longest_chains_first_and_retains_their_semantic_label_for_routing() {
        let (mut graph, links, order) = input(
            6,
            &[
                (0, 1, 1),
                (1, 2, 1),
                (2, 3, 1),
                (3, 4, 1),
                (4, 5, 1),
                (0, 5, 1),
                (1, 4, 1),
            ],
        );
        graph.edges[5].label = Some(shape(9.0, 4.0));
        graph.edges[6].label = Some(shape(6.0, 3.0));
        let mut options = Options {
            inline_labels: false,
            ..Options::default()
        };
        options.caps.max_dummies = 2;
        let mut g = Expanded::default();
        build(
            &graph.nodes,
            &links,
            &order,
            &graph,
            &options,
            &mut Work::new(&mut NoopWorkControl),
            &mut g,
        )
        .unwrap();
        assert_eq!(g.direct, [false, false, false, false, false, true, false]);
        assert_eq!(g.chains[5], [0, 5]);
        assert!(g.label_vertex[5].is_none() && graph.edges[links[5].edge as usize].label.is_some());
        assert_eq!(g.rank.len(), 8);
        let label = g.label_vertex[6].unwrap() as usize;
        assert!(
            g.layers[g.rank[label] as usize]
                .iter()
                .any(|&v| g.kind[v as usize] == Kind::Node)
        );
        assert!(!g.succ.at(0).iter().any(|&e| g.to[e as usize] == 5));
    }

    struct Meter {
        used: usize,
        limit: usize,
    }
    impl WorkControl for Meter {
        fn check(&mut self) -> Result<(), WorkError> {
            Ok(())
        }
        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            if units > self.limit - self.used {
                return Err(WorkError::Interrupted);
            }
            self.used += units;
            Ok(())
        }
    }

    #[test]
    fn reused_workspace_recovers_from_interrupted_expansion_and_drops_empty_tiers() {
        let (graph, links, order) = input(3, &[(0, 1, 1), (0, 1, 1), (1, 2, 1)]);
        let mut g = Expanded::default();
        let run = |g: &mut Expanded, cap: usize, limit: usize| {
            let mut opts = Options::default();
            opts.caps.max_dummies = cap;
            let mut ctl = Meter { used: 0, limit };
            let mut work = Work::new(&mut ctl);
            build(&graph.nodes, &links, &order, &graph, &opts, &mut work, g)?;
            work.finish()?;
            Ok::<_, LayoutError>(ctl.used)
        };
        let exact = run(&mut g, 2, usize::MAX).unwrap();
        let old = g.width.as_ptr();
        for limit in [0, exact / 2, exact - 1] {
            assert_eq!(
                run(&mut g, 2, limit),
                Err(LayoutError::Work(WorkError::Interrupted))
            );
            assert_eq!(run(&mut g, 2, exact).unwrap(), exact);
            assert_eq!(g.width.as_ptr(), old);
            assert_eq!(g.kind.len(), 5);
        }
        run(&mut g, 0, usize::MAX).unwrap();
        assert_eq!(g.logical_rank, [0, 2, 3]);
        assert!(g.inserted_ranks.is_empty());
        assert!(g.layers.iter().all(|r| !r.is_empty()));
        assert_eq!(g.rank, [0, 1, 2]);
        assert_eq!(g.direct, [true, true, false]);
        assert_eq!(g.chains, [vec![0, 1], vec![0, 1], vec![1, 2]]);
        assert_eq!(g.from, [1]);
        assert_eq!(g.to, [2]);
    }

    #[test]
    fn random_multigraph_chains_match_their_rank_constraints_and_segment_indices() {
        let mut state = 7531_u32;
        let mut next = || {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            state
        };
        let mut g = Expanded::default();
        for _ in 0..120 {
            let n = 3 + next() % 12;
            let mut pairs = Vec::new();
            for _ in 0..n * 2 {
                let a = next() % n;
                let b = (a + 1 + next() % (n - 1)) % n;
                pairs.push((a, b, next() % 5));
            }
            let (mut graph, arcs, order) = input(n, &pairs);
            for spec in &mut graph.edges {
                if next() % 3 == 0 {
                    spec.label = Some(shape(0.0, 4.0));
                }
            }
            let mut o = Options::default();
            o.caps.max_dummies = (next() % 24) as usize;
            build(
                &graph.nodes,
                &arcs,
                &order,
                &graph,
                &o,
                &mut Work::new(&mut NoopWorkControl),
                &mut g,
            )
            .unwrap();
            assert!(g.kind.len() - n as usize <= o.caps.max_dummies);
            assert!(g.logical_rank.windows(2).all(|r| r[0] < r[1]));
            assert_eq!(g.layers.iter().map(Vec::len).sum::<usize>(), g.rank.len());
            let mut segment = 0;
            for (i, e) in arcs.iter().enumerate() {
                let a = e.from.min(e.to);
                let b = e.from.max(e.to);
                assert_eq!(
                    (g.chains[i].first(), g.chains[i].last()),
                    (Some(&a), Some(&b))
                );
                let dist = g.logical_rank[g.rank[b as usize] as usize]
                    - g.logical_rank[g.rank[a as usize] as usize];
                assert!(dist >= u64::from(e.span.max(1)));
                if g.direct[i] {
                    assert_eq!(g.chains[i].len(), 2);
                    continue;
                }
                assert_eq!(g.label_vertex[i].is_some(), graph.edges[i].label.is_some());
                for p in g.chains[i].windows(2) {
                    assert_eq!((g.from[segment], g.to[segment]), (p[0], p[1]));
                    assert_eq!(g.rank[p[1] as usize], g.rank[p[0] as usize] + 1);
                    assert!(
                        g.pred.at(p[1]).contains(&(segment as u32))
                            && g.succ.at(p[0]).contains(&(segment as u32))
                    );
                    segment += 1;
                }
            }
            assert_eq!(segment, g.from.len());
        }
    }
}
