//! Align group endpoints with the peer order of routes stitched through the same face.

use super::*;
use crate::compound::refine::count_crossings;
use std::cmp::Ordering;

#[derive(Clone, Copy)]
struct Port {
    group: GroupId,
    side: Side,
    edge: usize,
    point: usize,
    forward: Option<bool>,
    stitched: bool,
    movable: bool,
}

impl Port {
    fn face(self) -> (GroupId, u8) {
        (self.group, self.side as u8)
    }

    fn outside(self, len: usize) -> impl Iterator<Item = usize> {
        let step = if self.forward == Some(true) { 1 } else { -1 };
        std::iter::successors(self.forward.map(|_| self.point), move |&index| {
            index.checked_add_signed(step).filter(|&next| next < len)
        })
        .skip(1)
    }
}

fn ports(ctx: &Scene<'_>, output: &LayoutOutput, work: &mut Work<'_>) -> LayoutResult<Vec<Port>> {
    let mut ports = Vec::new();
    for (edge, (drawing, spec)) in output.edges.iter().zip(&ctx.graph.edges).enumerate() {
        work.visit(1)?;
        let groups = matches!(
            (spec.source, spec.target),
            (Endpoint::Group(_), Endpoint::Group(_))
        );
        for (end, hint, side, point, marker, label, forward) in [
            (
                spec.source,
                spec.source_port,
                drawing.ports.0,
                0,
                spec.start_marker,
                spec.start_label,
                true,
            ),
            (
                spec.target,
                spec.target_port,
                drawing.ports.1,
                drawing.points.len() - 1,
                spec.end_marker,
                spec.end_label,
                false,
            ),
        ] {
            if let Endpoint::Group(group) = end {
                push(
                    &mut ports,
                    Port {
                        group,
                        side,
                        edge,
                        point,
                        forward: Some(forward),
                        stitched: false,
                        movable: groups
                            && hint == PortSideHint::Auto
                            && label.is_none()
                            && marker.length == 0.0
                            && marker.width == 0.0,
                    },
                )?;
            }
        }
    }
    work.visit(ports.len())?;
    if !ports.iter().any(|port| port.movable) {
        return Ok(Vec::new());
    }
    for &seam in &output.border_stitches {
        work.visit(1)?;
        let edge = seam.edge.0 as usize;
        let points = &output.edges[edge].points;
        let travel = Travel::from_face(seam.side);
        let along = travel.along(points[seam.point]);
        let outside = |index: Option<usize>| {
            index
                .and_then(|index| points.get(index))
                .is_some_and(|&point| travel.along(point) > along)
        };
        let ahead = outside(seam.point.checked_add(1));
        let behind = outside(seam.point.checked_sub(1));
        push(
            &mut ports,
            Port {
                group: seam.group,
                side: seam.side,
                edge,
                point: seam.point,
                forward: (ahead != behind).then_some(ahead),
                stitched: true,
                movable: false,
            },
        )?;
    }
    work.charge_sort(ports.len())?;
    ports.sort_unstable_by_key(|port| (port.face(), port.edge, port.point));
    work.check()?;
    Ok(ports)
}

fn sweep_end(
    port: Port,
    points: &[RoutePoint],
    travel: Travel,
    work: &mut Work<'_>,
) -> LayoutResult<Option<usize>> {
    let first = points[port.point];
    let mut previous = first;
    let mut outside = port.outside(points.len());
    while let Some(index) = outside.next() {
        work.visit(1)?;
        let next = points[index];
        if next.kind != KnotKind::Free
            || next.axis != travel.axis
            || travel.along(next) <= travel.along(previous)
        {
            break;
        }
        if travel.across(next) != travel.across(first) {
            // Both sides of the offset sweep need a straight lead. Leave its far anchor in place.
            return Ok((first != previous
                && outside.next().is_some_and(|after| {
                    let after = points[after];
                    travel.across(after) == travel.across(next)
                        && travel.along(after) > travel.along(next)
                }))
            .then_some(index));
        }
        previous = next;
    }
    Ok(None)
}

fn peer_order(
    port: Port,
    points: &[RoutePoint],
    travel: Travel,
    plane: f64,
    lane: f64,
    work: &mut Work<'_>,
) -> LayoutResult<Option<Ordering>> {
    let mut previous = points[port.point];
    for index in port.outside(points.len()) {
        work.visit(1)?;
        let next = points[index];
        let along = travel.along(next);
        if next.axis != travel.axis || along <= travel.along(previous) {
            break;
        }
        if along >= plane {
            let after = lane.total_cmp(&travel.across(next));
            // Inside an offset sweep its two tangents bound the transverse coordinate.
            let before = if along == plane {
                after
            } else {
                lane.total_cmp(&travel.across(previous))
            };
            return Ok((after == before && after != Ordering::Equal).then_some(after));
        }
        previous = next;
    }
    Ok(None)
}

fn edits(
    ctx: &Scene<'_>,
    output: &LayoutOutput,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<(usize, Vec<RoutePoint>)>> {
    let ports = ports(ctx, output, work)?;
    if ports.is_empty() {
        return Ok(Vec::new());
    }
    let obstacles = Obstacles::new(ctx.graph, output, work)?;
    let mut edited = filled(output.edges.len(), false)?;
    let mut edits = Vec::new();
    let spacing = ctx.options.spacing;
    let stitches = &output.border_stitches;
    for face in ports.chunk_by(|a, b| a.face() == b.face()) {
        work.visit(face.len())?;
        if !face.iter().any(|port| port.stitched) {
            continue;
        }
        'port: for &port in face.iter().filter(|port| port.movable) {
            work.visit(1)?;
            let points = &output.edges[port.edge].points;
            let travel = Travel::from_face(port.side);
            if edited[port.edge] || points[port.point].kind != KnotKind::Free {
                continue;
            }
            let Some(anchor) = sweep_end(port, points, travel, work)? else {
                continue;
            };
            let plane = travel.along(points[anchor]);
            let range = port.point.min(anchor)..=port.point.max(anchor);
            let first_stitch = stitches.partition_point(|seam| {
                (seam.edge.0 as usize, seam.point) < (port.edge, *range.start())
            });
            work.visit(stitches.len().max(1).ilog2() as usize + 1)?;
            if stitches.get(first_stitch).is_some_and(|seam| {
                seam.edge.0 as usize == port.edge && range.contains(&seam.point)
            }) {
                continue;
            }
            let tails = if port.point == 0 {
                &points[anchor + 1..]
            } else {
                &points[..anchor]
            };
            work.visit(tails.len())?;
            if tails.iter().any(|&point| travel.along(point) <= plane) {
                continue;
            }
            let lane = travel.across(points[anchor]);
            let from = travel.across(points[port.point]);
            let rect = output.groups[port.group.0 as usize].rect;
            let (lo, span) = match travel.axis {
                Axis::Horizontal => (rect.y, rect.height),
                Axis::Vertical => (rect.x, rect.width),
            };
            let inset = spacing.corner_clearance.min(span / 2.0);
            if lane < lo + inset || lane > lo + span - inset {
                continue;
            }
            let mut inverted = false;
            for &other in face {
                work.visit(1)?;
                if other.edge == port.edge && other.point == port.point {
                    continue;
                }
                let peer = &output.edges[other.edge].points;
                let at = travel.across(peer[other.point]);
                let Some(order) = peer_order(other, peer, travel, plane, lane, work)? else {
                    continue 'port;
                };
                if edited[other.edge]
                    || order != lane.total_cmp(&at)
                    || (lane - at).abs() < spacing.min_port_pitch
                {
                    continue 'port;
                }
                inverted |= other.stitched && order == at.total_cmp(&from);
            }
            if !inverted {
                continue;
            }
            let mut footprint = Bounds::route(&[points[port.point], points[anchor]], work)?;
            let half_pitch = spacing.min_port_pitch / 2.0;
            let clearance = [lane - half_pitch, lane + half_pitch];
            if let Axis::Horizontal = travel.axis {
                footprint.y = clearance;
            } else {
                footprint.x = clearance;
            }
            if !obstacles.clear(
                footprint,
                port.edge,
                [port.group; 2],
                output,
                std::iter::empty(),
                work,
            )? {
                continue;
            }
            work.charge(points.len())?;
            let mut route = reserved(points.len())?;
            route.extend_from_slice(points);
            work.visit(range.clone().count())?;
            for point in &mut route[range] {
                *point = travel.on_lane(*point, lane);
            }
            edited[port.edge] = true;
            push(&mut edits, (port.edge, route))?;
        }
    }
    Ok(edits)
}

pub(super) fn relax(
    ctx: &Scene<'_>,
    output: &mut LayoutOutput,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let old = output.stats.routed_crossings;
    if old == 0 {
        return Ok(());
    }
    let trial = work.compare(|trial| {
        let mut edits = edits(ctx, output, trial)?;
        if edits.is_empty() {
            return Ok(None);
        }
        trial.charge(
            edits
                .len()
                .checked_mul(3)
                .ok_or(crate::WorkError::ArithmeticOverflow)?,
        )?;
        for (edge, route) in &mut edits {
            std::mem::swap(&mut output.edges[*edge].points, route);
        }
        let crossings = count_crossings(ctx.graph, output, ctx.options.routing, old, trial);
        for (edge, route) in &mut edits {
            std::mem::swap(&mut output.edges[*edge].points, route);
        }
        let crossings = crossings?;
        Ok((crossings < old).then_some((crossings, edits)))
    });
    match trial {
        Ok(Some(Some((crossings, edits)))) => {
            for (edge, route) in edits {
                output.edges[edge].points = route;
            }
            output.stats.routed_crossings = crossings;
        }
        Ok(_) | Err(LayoutError::AllocationLimit) => {}
        Err(error) => return Err(error),
    }
    Ok(())
}
