use super::LEAD;
use crate::flat::{Component, Expanded, acyclic::Orientation, position};
use crate::work::{LayoutResult, Work, filled, reserved};
use crate::{Graph, Options, PortSideHint, Rect, Side};

#[derive(Clone, Copy)]
pub(super) struct SidePort {
    pub(super) side: Side,
    pub(super) rail: f64,
}

impl SidePort {
    pub(super) fn contact(self, node: Rect) -> f64 {
        match self.side {
            Side::Left => node.x,
            Side::Right => node.x + node.width,
            _ => unreachable!("exterior side ports are horizontal"),
        }
    }
}

pub(in crate::flat) fn side_docking_allowed(
    g: &Expanded,
    c: &Component,
    graph: &Graph,
    work: &mut Work<'_>,
) -> LayoutResult<bool> {
    work.visit(g.direct.len() + c.links.len())?;
    Ok(c.loops.is_empty()
        && !g.direct.contains(&true)
        && c.links.iter().all(|link| {
            let edge = &graph.edges[link.edge as usize];
            edge.source_port == PortSideHint::Auto
                && edge.target_port == PortSideHint::Auto
                && edge.start_label.is_none()
                && edge.end_label.is_none()
        }))
}

/// A terminal can turn inside its own rank when the horizontal leg has an unobstructed
/// run to the rail. Keep the other shafts pinned through that rank to preserve the clearance.
pub(super) fn attach_exterior_ports(
    g: &mut Expanded,
    c: &Component,
    orientation: &Orientation,
    graph: &Graph,
    options: &Options,
    x: &[f64],
    work: &mut Work<'_>,
) -> LayoutResult<Vec<[Option<SidePort>; 2]>> {
    if g.exterior_lanes.is_empty() {
        return Ok(Vec::new());
    }
    let mut side_ports = Vec::new();
    let mut used: Vec<(u32, f64, f64)> = Vec::new();
    let allow_side = side_docking_allowed(g, c, graph, work)?;
    if allow_side {
        work.charge(c.links.len() + 2 * g.exterior_lanes.len())?;
        side_ports = filled(c.links.len(), [None; 2])?;
        used = reserved(2 * g.exterior_lanes.len())?;
    }
    let half_separation = options.spacing.edge * 0.5;
    for lane in &g.exterior_lanes {
        let edge = lane.edge;
        let chain = &g.chains[edge];
        let last = chain.len() - 1;
        let rail = x[chain[1] as usize];
        let spec = &graph.edges[c.links[edge].edge as usize];
        let mut markers = [spec.start_marker, spec.end_marker];
        let mut hints = [spec.source_port, spec.target_port];
        if orientation.reversed[edge] {
            markers.swap(0, 1);
            hints.swap(0, 1);
        }
        let side = if lane.right { Side::Right } else { Side::Left };
        let sign = if lane.right { 1.0 } else { -1.0 };
        for end in 0..2 {
            work.visit(1)?;
            let v = chain[if end == 0 { 0 } else { last }];
            let i = v as usize;
            let node = &c.nodes[i];
            if hints[end] != PortSideHint::Auto || node.anchor_y.is_some() {
                continue;
            }
            let want = rail - x[i];
            let span = node
                .port_span
                .unwrap_or((g.width[i] - 2.0 * options.spacing.corner_clearance).max(0.0));
            let (segment, siblings, offsets) = if end == 0 {
                (g.pred.at(chain[1])[0], g.succ.at(v), &mut g.source_offset)
            } else {
                (
                    g.succ.at(chain[last - 1])[0],
                    g.pred.at(v),
                    &mut g.target_offset,
                )
            };
            let offset = offsets[segment as usize];
            if want.abs() <= span.min(g.width[i]) * 0.5 {
                work.visit(siblings.len())?;
                if siblings.iter().all(|&sibling| {
                    if sibling == segment {
                        return true;
                    }
                    let old = offsets[sibling as usize] - offset;
                    let new = offsets[sibling as usize] - want;
                    old != 0.0 && new.signum() == old.signum() && new.abs() >= old.abs()
                }) {
                    offsets[segment as usize] = want;
                }
                continue;
            }

            let marker = markers[end];
            let side_span = node
                .perpendicular_port_span
                .unwrap_or((g.height[i] - 2.0 * options.spacing.corner_clearance).max(0.0));
            if !allow_side
                || side_span.min(g.height[i]) < marker.width
                || g.height[i] == 0.0
                || sign * want - g.width[i] * 0.5
                    <= marker.length.max(LEAD) + half_separation.max(LEAD)
            {
                continue;
            }
            work.visit(siblings.len())?;
            if siblings.iter().any(|&sibling| {
                sibling != segment && sign * (offset - offsets[sibling as usize]) <= 0.0
            }) {
                continue;
            }
            let rank = g.rank[i];
            let row = &g.layers[rank as usize];
            let contact = x[i] + sign * g.width[i] * 0.5;
            let (lo, hi) = (contact.min(rail), contact.max(rail));
            work.visit(row.len() + used.len())?;
            if row.iter().any(|&peer| {
                if peer == v {
                    return false;
                }
                let (left, right) = position::extents(g, c, options, peer);
                let p = x[peer as usize];
                lo - half_separation <= p + right && p - left <= hi + half_separation
            }) || used.iter().any(|&(r, left, right)| {
                r == rank && lo - half_separation <= right && left <= hi + half_separation
            }) {
                continue;
            }
            work.visit(row.len())?;
            for &peer in row {
                g.keep[peer as usize] = true;
            }
            used.push((rank, lo, hi));
            side_ports[edge][end] = Some(SidePort { side, rail });
        }
    }
    Ok(side_ports)
}
