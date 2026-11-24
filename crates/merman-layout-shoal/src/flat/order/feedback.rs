use super::{Face, Fenwick, Order, Place, Ports, crossings, snapshot};
use crate::flat::{Component, Expanded, Kind, acyclic::Orientation};
use crate::work::{LayoutResult, Work, filled, reserved};

// An exterior preference is useful for authored feedback diagrams. Keep the exact crossing
// rechecks out of the large SCC and heavily expanded paths.
const MAX_NODES: usize = 48;
const MAX_EDGES: usize = 96;
const MAX_VERTICES: usize = 256;

#[derive(Debug)]
pub(in crate::flat) struct FeedbackLane {
    pub(in crate::flat) edge: usize,
    pub(in crate::flat) right: bool,
}

pub(in crate::flat) fn favor_feedback(
    g: &mut Expanded,
    c: &Component,
    orientation: &Orientation,
    count: u64,
    work: &mut Work<'_>,
) -> LayoutResult<u64> {
    if c.nodes.len() > MAX_NODES || c.links.len() > MAX_EDGES || g.rank.len() > MAX_VERTICES {
        return Ok(count);
    }
    let Some((order, lanes, count)) = order_lanes(g, orientation, count, work)? else {
        return Ok(count);
    };
    work.charge(order.ids.len())?;
    for (rank, row) in g.layers.iter_mut().enumerate() {
        row.copy_from_slice(&order.ids[order.range(rank)]);
    }
    g.feedback_lanes = lanes;
    Ok(count)
}

fn order_lanes(
    g: &Expanded,
    orientation: &Orientation,
    mut count: u64,
    work: &mut Work<'_>,
) -> LayoutResult<Option<(Order, Vec<FeedbackLane>, u64)>> {
    let mut candidates = reserved(g.chains.len())?;
    for (edge, chain) in g.chains.iter().enumerate() {
        work.visit(1)?;
        if !orientation.reversed[edge] || g.direct[edge] || chain.len() < 3 {
            continue;
        }
        // Adjacent edges with a label or a lens already have a separate lane in that gap.
        let first = g.rank[chain[0] as usize] as usize;
        let last = g.rank[*chain.last().unwrap() as usize] as usize;
        work.visit(g.layers[first + 1..last].iter().map(Vec::len).sum())?;
        if g.layers[first + 1..last]
            .iter()
            .flatten()
            .any(|&v| g.kind[v as usize] == Kind::Node)
        {
            candidates.push(edge);
        }
    }
    if candidates.is_empty() {
        return Ok(None);
    }
    work.sort(&mut candidates, |&a, &b| {
        let ends = |edge: usize| {
            let chain = &g.chains[edge];
            (chain[0], *chain.last().unwrap())
        };
        g.chains[b]
            .len()
            .cmp(&g.chains[a].len())
            .then(ends(a).cmp(&ends(b)))
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
    let mut lanes = reserved(candidates.len())?;
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
        let right = end_bias + bend_bias >= 1.0;
        for side in [right, !right] {
            // Already accepted exterior lanes form contiguous prefixes and suffixes.
            // Insert the new lane on the inside of these reserved slots.
            place_lane(g, &mut order, &ports, bends, &lanes, side, work)?;
            let trial_count = crossings(&order, &ports, &mut tree, work)?;
            if trial_count <= count {
                count = trial_count;
                work.visit(n)?;
                accepted.copy_from_slice(&order.ids);
                lanes.push(FeedbackLane { edge, right: side });
                break;
            }
            order.restore(&accepted, &ports, work)?;
        }
    }
    Ok((!lanes.is_empty()).then_some((order, lanes, count)))
}

fn place_lane(
    g: &Expanded,
    order: &mut Order,
    ports: &Ports<'_>,
    bends: &[u32],
    lanes: &[FeedbackLane],
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
                let chain = &g.chains[lane.edge];
                (g.rank[chain[0] as usize] as usize) < rank
                    && rank < g.rank[*chain.last().unwrap() as usize] as usize
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
