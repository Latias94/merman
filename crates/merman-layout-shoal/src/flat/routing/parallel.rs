use super::SidePort;
use crate::flat::{Component, Expanded, diamond_offset};
use crate::work::{LayoutResult, Work, filled, push, reserved};
use crate::{Graph, LayoutError, Options, PortSideHint};
use std::collections::{HashMap, hash_map::Entry};

struct Member {
    nodes: [u32; 2],
    edge: usize,
    segments: Option<[usize; 2]>,
    order: [(f64, f64); 2],
}

fn contiguous(
    cache: &mut HashMap<u32, Vec<f64>>,
    (indices, offsets): (&[u32], &[f64]),
    node: u32,
    [lo, hi]: [f64; 2],
    members: usize,
    work: &mut Work<'_>,
) -> LayoutResult<bool> {
    let contacts = if let Some(contacts) = cache.get(&node) {
        contacts
    } else {
        work.charge(indices.len())?;
        let mut contacts = reserved(indices.len())?;
        for &index in indices {
            work.visit(1)?;
            contacts.push(offsets[index as usize]);
        }
        work.charge_sort(contacts.len())?;
        contacts.sort_unstable_by(f64::total_cmp);
        cache
            .try_reserve(1)
            .map_err(|_| LayoutError::AllocationLimit)?;
        cache.entry(node).or_insert(contacts)
    };
    work.visit(2 * (contacts.len().ilog2() as usize + 1))?;
    let first = contacts.partition_point(|&x| x < lo);
    let last = contacts.partition_point(|&x| x <= hi);
    Ok(last - first == members)
}

/// Share one centered slot sequence between both ranked faces of a parallel pair, including
/// reverse occurrences and edges that bypass expansion when the dummy budget is exhausted.
pub(super) fn balance(
    g: &mut Expanded,
    c: &Component,
    graph: &Graph,
    options: &Options,
    x: &[f64],
    side_ports: &[[Option<SidePort>; 2]],
    work: &mut Work<'_>,
) -> LayoutResult<Vec<[f64; 2]>> {
    if !g.has_parallel_links || c.boundaries || c.group_proxies {
        return Ok(Vec::new());
    }
    work.visit(g.direct.len())?;
    let has_direct = g.direct.contains(&true);
    // Ordinary repeated links get a separating dummy tier unless they took the direct fallback.
    if g.rank.len() == c.nodes.len() && !has_direct {
        return Ok(Vec::new());
    }
    let mut direct_counts = Vec::new();
    if has_direct {
        work.charge(c.nodes.len())?;
        direct_counts = filled(c.nodes.len(), [0usize; 2])?;
        for (edge, chain) in g.chains.iter().enumerate() {
            work.visit(1)?;
            if g.direct[edge] {
                direct_counts[chain[0] as usize][0] += 1;
                direct_counts[*chain.last().unwrap() as usize][1] += 1;
            }
        }
    }
    let degree = |a: u32, b: u32| {
        [
            g.succ.at(a).len() + direct_counts.get(a as usize).map_or(0, |ends| ends[0]),
            g.pred.at(b).len() + direct_counts.get(b as usize).map_or(0, |ends| ends[1]),
        ]
    };
    let mut members = Vec::new();
    let mut singletons = HashMap::new();
    let mut next = 0;
    for (edge, chain) in g.chains.iter().enumerate() {
        work.visit(1)?;
        let last = chain.len() - 1;
        let [a, b] = [chain[0], chain[last]];
        let segments = if g.direct[edge] {
            None
        } else {
            let first = next;
            next += last;
            Some([first, next - 1])
        };
        if degree(a, b).into_iter().any(|count| count < 2) {
            continue;
        }
        if singletons.len() == singletons.capacity() {
            singletons
                .try_reserve(1)
                .map_err(|_| LayoutError::AllocationLimit)?;
        }
        let pair = (u64::from(a) << u32::BITS) | u64::from(b);
        let first = match singletons.entry(pair) {
            Entry::Vacant(entry) => {
                entry.insert(Some((edge, segments)));
                continue;
            }
            Entry::Occupied(mut entry) => entry.get_mut().take(),
        };
        for (edge, segments) in first.into_iter().chain([(edge, segments)]) {
            work.visit(1)?;
            let chain = &g.chains[edge];
            let order = segments.map_or([(0.0, x[a as usize]), (0.0, x[b as usize])], |[s, t]| {
                [
                    (g.source_offset[s], x[chain[1] as usize]),
                    (g.target_offset[t], x[chain[chain.len() - 2] as usize]),
                ]
            });
            push(
                &mut members,
                Member {
                    nodes: [a, b],
                    edge,
                    segments,
                    order,
                },
            )?;
        }
    }
    if members.is_empty() {
        return Ok(Vec::new());
    }
    let cmp = |(a, ax): (f64, f64), (b, bx): (f64, f64)| a.total_cmp(&b).then(ax.total_cmp(&bx));
    work.charge_sort(members.len())?;
    members.sort_unstable_by(|a, b| {
        a.nodes
            .cmp(&b.nodes)
            .then(cmp(a.order[0], b.order[0]))
            .then(a.edge.cmp(&b.edge))
    });

    let mut direct = Vec::new();
    let mut face_contacts = [HashMap::new(), HashMap::new()];
    'pair: for pair in members.chunk_by(|a, b| a.nodes == b.nodes) {
        work.visit(1)?;
        let [a, b] = pair[0].nodes;
        if pair.len() < 2 {
            continue;
        }
        let nodes = [a as usize, b as usize];
        if nodes.into_iter().any(|v| c.nodes[v].anchor_y.is_some()) {
            continue;
        }
        let mut existing = [[f64::INFINITY, f64::NEG_INFINITY]; 2];
        let mut previous = None::<[(f64, f64); 2]>;
        let mut widths = None::<[f64; 2]>;
        let mut required: f64 = 0.0;
        for member in pair {
            work.visit(1)?;
            let link = c.links[member.edge];
            let spec = &graph.edges[link.edge as usize];
            if spec.source_port != PortSideHint::Auto
                || spec.target_port != PortSideHint::Auto
                || side_ports
                    .get(member.edge)
                    .is_some_and(|ports| ports.iter().any(Option::is_some))
                || member.segments.is_some_and(|[s, t]| {
                    g.spine_ports.centered(nodes[0], 1, s) || g.spine_ports.centered(nodes[1], 0, t)
                })
            {
                continue 'pair;
            }
            if member.segments.is_some() {
                if previous.is_some_and(|old| cmp(old[1], member.order[1]).is_gt()) {
                    continue 'pair;
                }
                previous = Some(member.order);
                for (range, (offset, _)) in existing.iter_mut().zip(member.order) {
                    range[0] = range[0].min(offset);
                    range[1] = range[1].max(offset);
                }
            }
            let mut marker_width = [spec.start_marker.width, spec.end_marker.width];
            if link.from != a {
                marker_width.swap(0, 1);
            }
            if let Some(widths) = widths {
                for (before, after) in widths.into_iter().zip(marker_width) {
                    required = required.max(before * 0.5 + after * 0.5);
                }
            }
            widths = Some(marker_width);
        }
        let steps = (pair.len() - 1) as f64;
        let span = |node: u32| {
            let i = node as usize;
            let width = g.width[i];
            let spec = &c.nodes[i];
            let outline = diamond_offset(spec.contour, graph.direction)
                .map_or(width, |shift| width - 2.0 * shift.x.abs());
            spec.port_span
                .unwrap_or(width - 2.0 * options.spacing.corner_clearance)
                .min(outline)
        };
        let mut ranges = [a, b].map(|node| {
            let half = span(node) * 0.5;
            [-half, half]
        });
        let degrees = degree(a, b);
        // A fan can share one face with the pair. Reuse the pair's occupied interval only
        // when no foreign contact lies between its ends, and keep both runs in lane order.
        for (side, node, contacts) in [
            (0, a, (g.succ.at(a), g.source_offset.as_slice())),
            (1, b, (g.pred.at(b), g.target_offset.as_slice())),
        ] {
            if degrees[side] == pair.len() {
                continue;
            }
            let [lo, hi] = existing[side];
            if lo >= hi
                || direct_counts
                    .get(node as usize)
                    .is_some_and(|ends| ends[side] > 0)
                || !contiguous(
                    &mut face_contacts[side],
                    contacts,
                    node,
                    [lo, hi],
                    pair.len(),
                    work,
                )?
            {
                continue 'pair;
            }
            ranges[side][0] = ranges[side][0].max(lo);
            ranges[side][1] = ranges[side][1].min(hi);
        }
        let usable = ranges.map(|[lo, hi]| hi - lo);
        let center = ranges.map(|[lo, hi]| lo.midpoint(hi));
        let pitch = options
            .spacing
            .port_pitch
            .max(required)
            .min(usable[0].min(usable[1]) / steps);
        if pitch <= 0.0 || pitch < required {
            continue;
        }
        for (slot, member) in pair.iter().enumerate() {
            work.visit(1)?;
            let shift = (slot as f64 - steps * 0.5) * pitch;
            let [source, target] = center.map(|offset| offset + shift);
            if let Some([s, t]) = member.segments {
                g.source_offset[s] = source;
                g.target_offset[t] = target;
            } else {
                if direct.is_empty() {
                    work.charge(c.links.len())?;
                    direct = filled(c.links.len(), [0.0; 2])?;
                }
                direct[member.edge] = [source, target];
            }
        }
    }
    Ok(direct)
}
