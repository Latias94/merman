use super::{Face, Fenwick, Order, Place, Ports, crossings, snapshot};
use crate::flat::{Component, Expanded, Kind, acyclic::Orientation};
use crate::work::{LayoutResult, Work, filled, reserved};

// Exact crossing rechecks stay out of the large or heavily expanded paths.
const MAX_NODES: usize = 48;
const MAX_EDGES: usize = 96;
const MAX_VERTICES: usize = 256;

#[derive(Debug)]
pub(in crate::flat) struct ExteriorLane {
    pub(in crate::flat) edge: usize,
    pub(in crate::flat) right: bool,
}

pub(in crate::flat) fn reserve_exterior_lanes(
    g: &mut Expanded,
    c: &Component,
    orientation: &Orientation,
    count: u64,
    work: &mut Work<'_>,
) -> LayoutResult<u64> {
    if c.nodes.len() > MAX_NODES
        || c.links.len() > MAX_EDGES
        || g.rank.len() > MAX_VERTICES
        || c.links.len() < c.nodes.len()
    {
        return Ok(count);
    }
    let Some((order, lanes, count)) = order_lanes(g, c, orientation, count, work)? else {
        return Ok(count);
    };
    work.charge(order.ids.len())?;
    for (rank, row) in g.layers.iter_mut().enumerate() {
        row.copy_from_slice(&order.ids[order.range(rank)]);
    }
    g.exterior_lanes = lanes;
    Ok(count)
}

/// A forward edge is a bypass only if its target is reachable by a different forward path.
fn bypass_targets(
    c: &Component,
    orientation: &Orientation,
    work: &mut Work<'_>,
) -> LayoutResult<[u64; MAX_NODES]> {
    let mut successors = [0u64; MAX_NODES];
    work.visit(c.links.len())?;
    for (link, &reversed) in c.links.iter().zip(&orientation.reversed) {
        if !reversed {
            successors[link.from as usize] |= 1 << link.to;
        }
    }
    let mut reachable = successors;
    let mut bypass = [0; MAX_NODES];
    for &node in orientation.topo.iter().rev() {
        let node = node as usize;
        let mut peers = successors[node];
        work.visit(1 + peers.count_ones() as usize)?;
        while peers != 0 {
            let peer = peers.trailing_zeros() as usize;
            peers &= peers - 1;
            bypass[node] |= reachable[peer];
        }
        reachable[node] |= bypass[node];
    }
    Ok(bypass)
}

fn order_lanes(
    g: &Expanded,
    c: &Component,
    orientation: &Orientation,
    mut count: u64,
    work: &mut Work<'_>,
) -> LayoutResult<Option<(Order, Vec<ExteriorLane>, u64)>> {
    work.charge(g.chains.len() + g.layers.len() + 1)?;
    let mut occupied = reserved(g.layers.len() + 1)?;
    occupied.push(0);
    for row in &g.layers {
        work.visit(row.len() + 1)?;
        let node = row.iter().any(|&v| g.kind[v as usize] == Kind::Node);
        occupied.push(occupied.last().unwrap() + usize::from(node));
    }
    let mut candidates = reserved(g.chains.len())?;
    let mut bypass = None;
    for (edge, chain) in g.chains.iter().enumerate() {
        work.visit(1)?;
        if g.direct[edge] || chain.len() < 3 {
            continue;
        }
        // Adjacent edges with a label or a lens already have a separate lane in that gap.
        let (first, last) = ranks(g, edge);
        let spanned = occupied[last] - occupied[first + 1];
        let reversed = orientation.reversed[edge];
        if !reversed {
            if spanned < 2 {
                continue;
            }
            if bypass.is_none() {
                bypass = Some(bypass_targets(c, orientation, work)?);
            }
            let link = c.links[edge];
            if bypass.as_ref().unwrap()[link.from as usize] & (1 << link.to) == 0 {
                continue;
            }
        } else if spanned == 0 {
            continue;
        }
        candidates.push(edge);
    }
    if candidates.is_empty() {
        return Ok(None);
    }
    work.sort(&mut candidates, |&a, &b| {
        let ends = |edge: usize| {
            let chain = &g.chains[edge];
            (chain[0], *chain.last().unwrap())
        };
        orientation.reversed[b]
            .cmp(&orientation.reversed[a])
            .then(g.chains[b].len().cmp(&g.chains[a].len()))
            .then(ends(a).cmp(&ends(b)))
            .then(a.cmp(&b))
    })?;

    let n = g.rank.len();
    work.charge(n + g.layers.len())?;
    let ports = Ports {
        source: Face::new(g, false, work)?,
        target: Face::new(g, true, work)?,
    };
    let mut order = Order {
        start: reserved(g.layers.len() + 1)?,
        ids: reserved(n)?,
        place: filled(n, Place::default())?,
    };
    for row in &g.layers {
        order.start.push(order.ids.len());
        order.ids.extend_from_slice(row);
    }
    order.start.push(n);
    for rank in 0..order.ranks() {
        order.reindex(rank, &ports, work)?;
    }
    let mut accepted = snapshot(&order.ids, work)?;
    let mut trial = snapshot(&order.ids, work)?;
    work.charge(candidates.len())?;
    let mut lanes: Vec<ExteriorLane> = reserved(candidates.len())?;
    let mut tree = Fenwick::default();
    for edge in candidates {
        let chain = &g.chains[edge];
        let position = |id: u32| {
            let row = &g.layers[g.rank[id as usize] as usize];
            if row.len() == 1 {
                0.5
            } else {
                order.place[id as usize].pos as f64 / (row.len() - 1) as f64
            }
        };
        let end_bias = (position(chain[0]) + position(*chain.last().unwrap())) * 0.5;
        let bends = &chain[1..chain.len() - 1];
        work.visit(bends.len())?;
        let bend_bias = bends.iter().map(|&v| position(v)).sum::<f64>() / bends.len() as f64;
        let mut right = end_bias + bend_bias >= 1.0;
        if !orientation.reversed[edge] {
            let (first, last) = ranks(g, edge);
            let mut feedback = [0; 2];
            work.visit(lanes.len())?;
            for lane in &lanes {
                let (a, b) = ranks(g, lane.edge);
                if orientation.reversed[lane.edge] && first.max(a) < last.min(b) {
                    feedback[usize::from(lane.right)] += 1;
                }
            }
            if feedback[0] != feedback[1] {
                right = feedback[1] < feedback[0];
            }
        }
        let mut best = None;
        for side in [right, !right] {
            // Already accepted exterior lanes form contiguous prefixes and suffixes.
            // Insert the new lane on the inside of these reserved slots.
            place_lane(g, &mut order, &ports, bends, &lanes, side, work)?;
            let trial_count = crossings(&order, &ports, &mut tree, work)?;
            if trial_count <= count && best.is_none_or(|(score, _)| trial_count < score) {
                best = Some((trial_count, side));
                work.visit(n)?;
                trial.copy_from_slice(&order.ids);
            }
            if trial_count == 0 {
                break;
            }
            order.restore(&accepted, &ports, work)?;
        }
        if let Some((trial_count, right)) = best {
            count = trial_count;
            std::mem::swap(&mut accepted, &mut trial);
            lanes.push(ExteriorLane { edge, right });
            order.restore(&accepted, &ports, work)?;
        }
    }
    Ok((!lanes.is_empty()).then_some((order, lanes, count)))
}

fn ranks(g: &Expanded, edge: usize) -> (usize, usize) {
    let chain = &g.chains[edge];
    (
        g.rank[chain[0] as usize] as usize,
        g.rank[*chain.last().unwrap() as usize] as usize,
    )
}

fn place_lane(
    g: &Expanded,
    order: &mut Order,
    ports: &Ports<'_>,
    bends: &[u32],
    lanes: &[ExteriorLane],
    right: bool,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    for &v in bends {
        let rank = g.rank[v as usize] as usize;
        let range = order.range(rank);
        work.visit(lanes.len() + range.len())?;
        let reserved = lanes
            .iter()
            .filter(|lane| {
                if lane.right != right {
                    return false;
                }
                let (first, last) = ranks(g, lane.edge);
                first < rank && rank < last
            })
            .count();
        let at = order.place[v as usize].pos;
        let target = if right {
            range.len() - 1 - reserved
        } else {
            reserved
        };
        let row = &mut order.ids[range];
        if at < target {
            row[at..=target].rotate_left(1);
        } else {
            row[target..=at].rotate_right(1);
        }
        order.reindex(rank, ports, work)?;
    }
    Ok(())
}
