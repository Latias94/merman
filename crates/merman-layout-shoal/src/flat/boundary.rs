use super::{BoundaryRank, Link, acyclic};
use crate::work::{LayoutResult, Work, reserved};
use crate::{LayoutError, NodeSpec, Options, WorkError};

#[cfg(test)]
mod tests;

pub(super) fn orient(
    nodes: &[NodeSpec],
    links: &[Link],
    boundaries: bool,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<acyclic::Orientation> {
    if !boundaries {
        return acyclic::orient(nodes, links, options.cycle_policy, work);
    }
    work.charge(links.len())?;
    let mut directed = reserved(links.len())?;
    for &link in links {
        work.visit(1)?;
        let from = nodes[link.from as usize].boundary;
        let to = nodes[link.to as usize].boundary;
        if from.is_some() && from == to {
            return Err(LayoutError::InvalidInput(
                "edge joins ports on the same boundary rank",
            ));
        }
        let flip = from == Some(BoundaryRank::End) || to == Some(BoundaryRank::Start);
        directed.push(if flip { link.flipped() } else { link });
    }
    let mut orientation = acyclic::orient(nodes, &directed, options.cycle_policy, work)?;
    for ((back, original), arc) in orientation.reversed.iter_mut().zip(links).zip(directed) {
        work.visit(1)?;
        *back ^= original.from != arc.from;
    }
    Ok(orientation)
}

pub(super) fn normalize(
    nodes: &[NodeSpec],
    arcs: &[Link],
    rank: &mut [u64],
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let (mut low, mut high) = (u64::MAX, 0);
    let (mut before, mut after) = (0, 0);
    for (node, &value) in nodes.iter().zip(rank.iter()) {
        work.visit(1)?;
        match node.boundary {
            Some(BoundaryRank::Start) => before = 1,
            Some(BoundaryRank::End) => after = 1,
            None => {
                low = low.min(value);
                high = high.max(value);
            }
        }
    }
    if low == u64::MAX {
        low = 0;
        after = 0;
    }
    let mut through = 0;
    for arc in arcs {
        work.visit(1)?;
        let (from, to) = (arc.from as usize, arc.to as usize);
        let span = u64::from(arc.span.max(1));
        match (nodes[from].boundary, nodes[to].boundary) {
            (Some(BoundaryRank::Start), None) => {
                before = before.max(span.saturating_sub(rank[to] - low))
            }
            (None, Some(BoundaryRank::End)) => {
                after = after.max(span.saturating_sub(high - rank[from]))
            }
            (Some(BoundaryRank::Start), Some(BoundaryRank::End)) => through = through.max(span),
            _ => {}
        }
    }
    let end = (high - low)
        .checked_add(before)
        .and_then(|v| v.checked_add(after))
        .ok_or(WorkError::ArithmeticOverflow)?
        .max(through);
    for (node, value) in nodes.iter().zip(rank) {
        work.visit(1)?;
        *value = match node.boundary {
            Some(BoundaryRank::Start) => 0,
            Some(BoundaryRank::End) => end,
            None => (*value - low) + before,
        };
    }
    Ok(())
}
