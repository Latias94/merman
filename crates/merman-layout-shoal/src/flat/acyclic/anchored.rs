use super::{Csr, Scc};
use crate::work::{LayoutResult, Work};

const MAX_VERTICES: usize = 24;
const MAX_ARCS: usize = 64;

#[derive(Clone, Copy, Default)]
struct Arc {
    from: usize,
    to: usize,
    span: u32,
}

#[derive(Default, PartialEq, Eq, PartialOrd, Ord)]
struct Score {
    // Shorter shortcuts cannot compensate for turning more authored edges around.
    reversed: usize,
    interleaved: usize,
    reversed_span: u64,
    total_span: u64,
}

/// Try two entry-anchored orders for an SCC small enough to score directly.
pub(super) fn append_order(
    scc: &Scc<'_>,
    members: &[u32],
    incoming: &Csr,
    topo: &mut Vec<u32>,
    work: &mut Work<'_>,
) -> LayoutResult<bool> {
    let n = members.len();
    if n > MAX_VERTICES {
        return Ok(false);
    }

    let mut arcs = [Arc::default(); MAX_ARCS];
    let mut count = 0;
    for (from, &v) in members.iter().enumerate() {
        work.visit(1)?;
        for &id in scc.outgoing.at(v) {
            work.visit(1)?;
            let arc = scc.links[id as usize];
            if scc.contains(arc.to) {
                let Some(slot) = arcs.get_mut(count) else {
                    return Ok(false);
                };
                *slot = Arc {
                    from,
                    to: arc.to as usize,
                    span: arc.span.max(1),
                };
                count += 1;
            }
        }
    }

    // Number vertices by (declaration order, ID). BFS scans successors in this order,
    // and the two candidate scores depend only on arc endpoints and minimum spans.
    let mut order = [0; MAX_VERTICES];
    let vertices = &mut order[..n];
    for (i, slot) in vertices.iter_mut().enumerate() {
        *slot = i;
    }
    work.sort(vertices, |&a, &b| {
        let key = |i: usize| (scc.nodes[members[i] as usize].seq, members[i]);
        key(a).cmp(&key(b))
    })?;
    let mut positions = [0; MAX_VERTICES];
    let mut root = None;
    for (v, &slot) in vertices.iter().enumerate() {
        positions[slot] = v;
        if root.is_none() {
            let incoming = incoming.at(members[slot]);
            work.visit(incoming.len())?;
            if incoming
                .iter()
                .any(|&e| !scc.contains(scc.links[e as usize].from))
            {
                root = Some(v);
            }
        }
    }
    let root = root.unwrap_or(0);

    let arcs = &mut arcs[..count];
    let mut successors = [0u32; MAX_VERTICES];
    for arc in arcs.iter_mut() {
        work.visit(n + 1)?;
        arc.from = positions[arc.from];
        let target = members
            .iter()
            .position(|&v| v as usize == arc.to)
            .expect("intra-component target");
        arc.to = positions[target];
        successors[arc.from] |= 1 << arc.to;
    }

    let (bfs, depth) = breadth_first(&successors, root, work)?;
    let greedy = greedy(arcs, n, root, &depth, work)?;
    let bfs = &bfs[..n];
    let greedy = &greedy[..n];
    let bfs_score = score(bfs, arcs, work)?.expect("breadth-first order has one entry");
    let chosen = match score(greedy, arcs, work)? {
        Some(greedy_score) if greedy_score <= bfs_score => greedy,
        _ => bfs,
    };
    work.charge(n)?;
    topo.extend(chosen.iter().map(|&v| members[vertices[v]]));
    Ok(true)
}

fn breadth_first(
    successors: &[u32; MAX_VERTICES],
    root: usize,
    work: &mut Work<'_>,
) -> LayoutResult<([usize; MAX_VERTICES], [u8; MAX_VERTICES])> {
    let mut queue = [root; MAX_VERTICES];
    let mut depth = [0; MAX_VERTICES];
    let mut seen = 1 << root;
    let (mut read, mut end) = (0, 1);
    while read < end {
        work.visit(1)?;
        let v = queue[read];
        let mut unseen = successors[v] & !seen;
        seen |= unseen;
        while unseen != 0 {
            work.visit(1)?;
            let u = unseen.trailing_zeros() as usize;
            unseen &= unseen - 1;
            depth[u] = depth[v] + 1;
            queue[end] = u;
            end += 1;
        }
        read += 1;
    }
    Ok((queue, depth))
}

fn greedy(
    arcs: &[Arc],
    n: usize,
    root: usize,
    depth: &[u8; MAX_VERTICES],
    work: &mut Work<'_>,
) -> LayoutResult<[usize; MAX_VERTICES]> {
    let mut order = [root; MAX_VERTICES];
    let mut live = (1u32 << n) - 1 - (1 << root);
    let mut degree = [[0i8; 2]; MAX_VERTICES];
    work.visit(arcs.len())?;
    for arc in arcs {
        if arc.from != root && arc.to != root {
            degree[arc.from][1] += 1;
            degree[arc.to][0] += 1;
        }
    }

    let (mut left, mut right) = (1, n);
    while left < right {
        let mut best = None;
        let mut eligible = None;
        let mut active = live;
        work.visit(active.count_ones() as usize)?;
        while active != 0 {
            let v = active.trailing_zeros() as usize;
            active &= active - 1;
            let [incoming, outgoing] = degree[v];
            let priority = if outgoing == 0 {
                (0, 0)
            } else if incoming == 0 {
                (1, 0)
            } else {
                (2, incoming - outgoing)
            };
            let key = (priority, depth[v], v);
            if best.is_none_or(|previous| key < previous) {
                best = Some(key);
            }
        }
        let (priority, _, first) = best.expect("at least one SCC member remains");
        let v = if priority.0 == 2 {
            active = live;
            work.visit(active.count_ones() as usize)?;
            while active != 0 {
                let v = active.trailing_zeros() as usize;
                active &= active - 1;
                let [incoming, outgoing] = degree[v];
                if incoming - outgoing <= priority.1 + 1 {
                    let key = (depth[v], v);
                    if eligible.is_none_or(|previous| key < previous) {
                        eligible = Some(key);
                    }
                }
            }
            eligible.expect("the highest-delta member is eligible").1
        } else {
            first
        };
        if degree[v][1] == 0 {
            right -= 1;
            order[right] = v;
        } else {
            order[left] = v;
            left += 1;
        }
        live &= !(1 << v);
        work.visit(arcs.len())?;
        for arc in arcs {
            if arc.from == v && live & (1 << arc.to) != 0 {
                degree[arc.to][0] -= 1;
            }
            if arc.to == v && live & (1 << arc.from) != 0 {
                degree[arc.from][1] -= 1;
            }
        }
    }
    Ok(order)
}

fn score(order: &[usize], arcs: &[Arc], work: &mut Work<'_>) -> LayoutResult<Option<Score>> {
    let mut position = [0; MAX_VERTICES];
    for (i, &v) in order.iter().enumerate() {
        position[v] = i;
    }
    let mut sorted = [0usize; MAX_ARCS];
    let ids = &mut sorted[..arcs.len()];
    for (id, slot) in ids.iter_mut().enumerate() {
        *slot = id;
    }
    let ends = |arc: Arc| {
        let (a, b) = (position[arc.from], position[arc.to]);
        (a.min(b), a.max(b))
    };
    work.sort(ids, |&a, &b| ends(arcs[a]).cmp(&ends(arcs[b])))?;

    let mut ranks = [0u64; MAX_VERTICES];
    for &id in ids.iter() {
        work.visit(1)?;
        let arc = arcs[id];
        let (a, b) = ends(arc);
        ranks[b] = ranks[b].max(ranks[a] + u64::from(arc.span));
    }
    work.visit(order.len())?;
    if ranks[1..order.len()].contains(&0) {
        return Ok(None);
    }

    let mut score = Score::default();
    for (i, &id) in ids.iter().enumerate() {
        work.visit(1)?;
        let arc = arcs[id];
        let (a, b) = ends(arc);
        let span = ranks[b] - ranks[a];
        score.total_span += span;
        if position[arc.from] > position[arc.to] {
            score.reversed += 1;
            score.reversed_span += span;
        }
        // Interleaved intervals estimate crossings before any coordinates are assigned.
        for &later in &ids[i + 1..] {
            work.visit(1)?;
            let (c, d) = ends(arcs[later]);
            if c >= b {
                break;
            }
            score.interleaved += usize::from(a < c && b < d);
        }
    }
    Ok(Some(score))
}
