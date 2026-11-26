//! Improve completed routes within their layout scope while retaining contacts and marker leads.

mod channels;

use crate::ortho::{OrthoRequest, OrthoRouter, OrthoSegment};
use crate::work::{LayoutResult, Work, filled, push, reserved};
use crate::{
    Axis, EdgeGeometry, EdgeSpec, Endpoint, Graph, LayoutError, LayoutOutput, MIN_MARKER_LEAD,
    MarkerMetrics, Options, Point, Rect, RoutePoint, RoutingStyle, Side, Size,
};

const MAX_ROUTES: usize = 256;
const MAX_BODIES: usize = 512;

pub(crate) fn refine(
    graph: &Graph,
    drawing: &mut LayoutOutput,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    if options.routing != RoutingStyle::Orthogonal
        || !options.global_orthogonal
        || options.caps.max_ortho_grid_nodes == 0
        || graph.edges.len() > MAX_ROUTES
        || graph.nodes.len().saturating_add(graph.groups.len()) > MAX_BODIES
    {
        return Ok(());
    }
    let trial = work.compare(|trial| measured_refinement(graph, drawing, options, trial));
    let Some(Some((changes, crossings))) = (match trial {
        Ok(result) => result,
        // The original drawing already passed validation, even if optional clearance arithmetic
        // or an allocation for the visibility graph was unavailable.
        Err(LayoutError::InvalidInput(_) | LayoutError::AllocationLimit) => None,
        Err(error) => return Err(error),
    }) else {
        return Ok(());
    };
    for (geometry, change) in drawing.edges.iter_mut().zip(changes) {
        if let Some(points) = change {
            geometry.points = points;
            geometry.corridor_clearance = 0.0;
        }
    }
    drawing.stats.routed_crossings = crossings;
    Ok(())
}

type Changes = Vec<Option<Vec<RoutePoint>>>;

fn measured_refinement(
    graph: &Graph,
    drawing: &mut LayoutOutput,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<Option<(Changes, u64)>> {
    let (mut changes, removed) = route(graph, drawing, options, work)?;
    if !changes.iter().any(Option::is_some) {
        return Ok(None);
    }
    let before = drawing.stats.routed_crossings;
    if graph.groups.is_empty() {
        return Ok(Some((changes, before.saturating_sub(removed))));
    }
    // Compound crossings have their own shared-terminal and interpolation semantics.
    let swap = |drawing: &mut LayoutOutput, changes: &mut Changes| {
        for (edge, points) in drawing.edges.iter_mut().zip(changes) {
            if let Some(points) = points {
                std::mem::swap(&mut edge.points, points);
            }
        }
    };
    work.charge(changes.len())?;
    swap(drawing, &mut changes);
    let after = crate::compound::crossings(graph, drawing, options.routing, work);
    swap(drawing, &mut changes);
    let after = after?;
    Ok((after <= before).then_some((changes, after)))
}

fn route(
    graph: &Graph,
    drawing: &LayoutOutput,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<(Changes, u64)> {
    work.charge(graph.edges.len() + graph.groups.len() + 1)?;
    let mut scopes = reserved(graph.groups.len() + 1)?;
    scopes.resize_with(graph.groups.len() + 1, Vec::new);
    let root = graph.groups.len();
    let mut fixed = filled(graph.edges.len(), false)?;
    for stitch in &drawing.border_stitches {
        work.visit(1)?;
        fixed[stitch.edge.0 as usize] = true;
    }
    for truncated in &drawing.truncated_routes {
        work.visit(1)?;
        fixed[truncated.edge.0 as usize] = true;
    }
    for (edge, spec) in graph.edges.iter().enumerate() {
        work.visit(1)?;
        let (Endpoint::Node(source), Endpoint::Node(target)) = (spec.source, spec.target) else {
            continue;
        };
        let source_scope = graph.nodes[source.0 as usize].parent;
        let target_scope = graph.nodes[target.0 as usize].parent;
        if !fixed[edge]
            && !drawing.edges[edge].shared_bus
            && source != target
            && source_scope == target_scope
            && unlabelled(spec)
        {
            push(
                &mut scopes[source_scope.map_or(root, |group| group.0 as usize)],
                edge,
            )?;
        }
    }

    let mut changes = filled(graph.edges.len(), None)?;
    let mut removed_crossings = 0;
    let walls = obstacles(graph, drawing, work)?;
    let root_bounds = root_bounds(drawing);
    for (scope, edges) in scopes
        .iter()
        .enumerate()
        .filter(|(_, edges)| !edges.is_empty())
    {
        work.charge(walls.len() + drawing.groups.len())?;
        let mut local = reserved(walls.len() + drawing.groups.len())?;
        local.extend_from_slice(&walls);
        let parent = (scope != root).then_some(crate::GroupId(scope as u32));
        local.extend(
            drawing
                .groups
                .iter()
                .filter(|g| g.parent == parent)
                .map(|g| g.rect),
        );
        let bounds = drawing.groups.get(scope).map_or(root_bounds, |g| g.rect);
        let mut router = OrthoRouter::new(
            &local,
            options.spacing.edge,
            options.caps.max_ortho_grid_nodes,
            work,
        )?;
        for &edge in edges {
            let geometry = &drawing.edges[edge];
            let old = &geometry.points;
            if let Some(candidate) = propose(
                &mut router,
                &graph.edges[edge],
                geometry,
                Some(bounds),
                options.spacing.edge,
                work,
            )? && extent_preserved(old, &candidate, root_bounds, work)?
            {
                let before = quality(edge, old, drawing, &changes, work)?;
                let after = quality(edge, &candidate, drawing, &changes, work)?;
                if after.overlaps <= before.overlaps && after.better_than(before) {
                    removed_crossings += before.crossings - after.crossings;
                    changes[edge] = Some(candidate);
                }
            }
            router.record_route(changes[edge].as_deref().unwrap_or(old), work)?;
        }
        removed_crossings += channels::Scene {
            router: &router,
            graph,
            drawing,
            bounds,
            spacing: options.spacing.edge,
        }
        .nudge(edges, &mut changes, work)?;
    }
    Ok((changes, removed_crossings))
}

fn root_bounds(drawing: &LayoutOutput) -> Rect {
    Rect {
        x: 0.0,
        y: 0.0,
        width: drawing.bounds.width,
        height: drawing.bounds.height,
    }
}

fn extent_preserved(
    old: &[RoutePoint],
    new: &[RoutePoint],
    bounds: Rect,
    work: &mut Work<'_>,
) -> LayoutResult<bool> {
    let touches = |points: &[RoutePoint]| {
        points.iter().fold(0, |flags, p| {
            flags
                | u8::from(p.x == bounds.x)
                | (u8::from(p.y == bounds.y) << 1)
                | (u8::from(p.x == bounds.x + bounds.width) << 2)
                | (u8::from(p.y == bounds.y + bounds.height) << 3)
        })
    };
    work.visit(old.len() + new.len())?;
    Ok(touches(old) & !touches(new) == 0)
}

fn unlabelled(spec: &EdgeSpec) -> bool {
    spec.label.is_none() && spec.start_label.is_none() && spec.end_label.is_none()
}

fn obstacles(
    graph: &Graph,
    drawing: &LayoutOutput,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<Rect>> {
    work.charge(drawing.nodes.len() + drawing.groups.len() + drawing.edges.len())?;
    let mut walls = reserved(drawing.nodes.len())?;
    walls.extend_from_slice(&drawing.nodes);
    for (spec, group) in graph.groups.iter().zip(&drawing.groups) {
        if let (Some(size), Some(center)) = (spec.title, group.title_center) {
            push(&mut walls, label(center, size))?;
        }
    }
    for (spec, geometry) in graph.edges.iter().zip(&drawing.edges) {
        for (size, center) in [
            (spec.label, geometry.label_center),
            (spec.start_label, geometry.start_label_center),
            (spec.end_label, geometry.end_label_center),
        ] {
            if let (Some(size), Some(center)) = (size, center) {
                push(&mut walls, label(center, size))?;
            }
        }
        for (end, (marker, side, contact)) in [
            (spec.start_marker, geometry.ports.0, geometry.points.first()),
            (spec.end_marker, geometry.ports.1, geometry.points.last()),
        ]
        .into_iter()
        .enumerate()
        {
            if marker.width > 0.0
                && marker.length > 0.0
                && let Some(&contact) = contact
            {
                let mut distance = marker.length * 0.5;
                if !graph.lane_order.is_empty() {
                    // A lane endpoint connected to a member can use the inward normal. The
                    // public side still identifies the physical face for contour clipping.
                    let next = if end == 0 {
                        geometry.points.get(1)
                    } else {
                        geometry.points.iter().rev().nth(1)
                    };
                    if next.is_some_and(|p| match side {
                        Side::Top => p.y > contact.y,
                        Side::Right => p.x < contact.x,
                        Side::Bottom => p.y < contact.y,
                        Side::Left => p.x > contact.x,
                    }) {
                        distance = -distance;
                    }
                }
                let at = offset(contact, side, distance);
                let (width, height) = match side.axis() {
                    Axis::Horizontal => (marker.length, marker.width),
                    Axis::Vertical => (marker.width, marker.length),
                };
                push(&mut walls, label(point(at), Size { width, height }))?;
            }
        }
    }
    Ok(walls)
}

fn label(center: Point, size: Size) -> Rect {
    Rect {
        x: center.x - size.width * 0.5,
        y: center.y - size.height * 0.5,
        width: size.width,
        height: size.height,
    }
}

fn point(knot: RoutePoint) -> Point {
    Point {
        x: knot.x,
        y: knot.y,
    }
}

fn offset(knot: RoutePoint, side: Side, distance: f64) -> RoutePoint {
    let (dx, dy) = match side {
        Side::Left => (-distance, 0.0),
        Side::Right => (distance, 0.0),
        Side::Top => (0.0, -distance),
        Side::Bottom => (0.0, distance),
    };
    RoutePoint::new(knot.x + dx, knot.y + dy, side.axis())
}

fn distance(from: RoutePoint, to: RoutePoint, side: Side) -> Option<f64> {
    match side {
        Side::Left if from.y == to.y => Some(from.x - to.x),
        Side::Right if from.y == to.y => Some(to.x - from.x),
        Side::Top if from.x == to.x => Some(from.y - to.y),
        Side::Bottom if from.x == to.x => Some(to.y - from.y),
        _ => None,
    }
}

fn lead(
    mut knots: impl Iterator<Item = RoutePoint>,
    side: Side,
    marker: MarkerMetrics,
    margin: f64,
    work: &mut Work<'_>,
) -> LayoutResult<Option<RoutePoint>> {
    let Some(contact) = knots.next() else {
        return Ok(None);
    };
    let required = marker.length.max(MIN_MARKER_LEAD) + margin;
    let mut last = 0.0;
    for knot in knots {
        work.visit(1)?;
        let Some(at) = distance(contact, knot, side) else {
            break;
        };
        if at < last {
            break;
        }
        if at >= required {
            return Ok(Some(offset(contact, side, required)));
        }
        last = at;
    }
    Ok(None)
}

fn propose(
    router: &mut OrthoRouter,
    spec: &EdgeSpec,
    geometry: &EdgeGeometry,
    bounds: Option<Rect>,
    spacing: f64,
    work: &mut Work<'_>,
) -> LayoutResult<Option<Vec<RoutePoint>>> {
    let [first, .., last] = geometry.points.as_slice() else {
        return Ok(None);
    };
    let knots = geometry.points.iter().copied();
    let (start_side, end_side) = geometry.ports;
    let Some(start) = lead(
        knots.clone(),
        start_side,
        spec.start_marker,
        spacing * 0.5,
        work,
    )?
    else {
        return Ok(None);
    };
    let Some(end) = lead(knots.rev(), end_side, spec.end_marker, spacing * 0.5, work)? else {
        return Ok(None);
    };
    if point(start) == point(end) {
        return Ok(None);
    }
    let request = OrthoRequest {
        start,
        end,
        start_side: Some(start_side),
        end_side: Some(end_side),
        bounds,
    };
    let Some(route) = router.route(request, work)? else {
        return Ok(None);
    };
    work.charge(route.len() + 2)?;
    let mut output = reserved(route.len() + 2)?;
    output.push(*first);
    output.extend(route);
    output.push(*last);
    Ok(Some(output))
}

#[derive(Clone, Copy, Default)]
struct Quality {
    crossings: u64,
    overlaps: usize,
    bends: usize,
    length: f64,
}

impl Quality {
    fn better_than(&self, other: Self) -> bool {
        (self.crossings, self.overlaps, self.bends, self.length)
            < (other.crossings, other.overlaps, other.bends, other.length)
    }
}

fn quality(
    edge: usize,
    points: &[RoutePoint],
    drawing: &LayoutOutput,
    changes: &[Option<Vec<RoutePoint>>],
    work: &mut Work<'_>,
) -> LayoutResult<Quality> {
    let mut quality = Quality::default();
    let mut previous = None;
    for leg in points.windows(2) {
        work.visit(1)?;
        let a = leg[0];
        let b = leg[1];
        if point(a) == point(b) {
            continue;
        }
        let segment = OrthoSegment::between(a, b);
        if let Some(segment) = segment {
            let direction = (segment.axis, a.x < b.x || a.y < b.y);
            quality.bends += usize::from(previous.is_some_and(|prior| prior != direction));
            previous = Some(direction);
            quality.length += segment.max - segment.min;
        } else {
            quality.bends += 2;
            quality.length += (a.x - b.x).abs() + (a.y - b.y).abs();
            previous = None;
        }
    }
    for (other, geometry) in drawing.edges.iter().enumerate().filter(|(i, _)| *i != edge) {
        let peers = changes[other].as_deref().unwrap_or(&geometry.points);
        let (crossings, overlaps) = intersections(points, peers, work)?;
        quality.crossings += crossings;
        quality.overlaps += overlaps;
    }
    Ok(quality)
}

fn intersections(
    points: &[RoutePoint],
    peers: &[RoutePoint],
    work: &mut Work<'_>,
) -> LayoutResult<(u64, usize)> {
    let mut crossed = false;
    let mut overlaps = 0;
    for a in points.windows(2) {
        let main = OrthoSegment::between(a[0], a[1]);
        for b in peers.windows(2) {
            work.visit(1)?;
            crossed |= crosses(a, b);
            if let (Some(a), Some(b)) = (main, OrthoSegment::between(b[0], b[1]))
                && a.axis == b.axis
                && a.fixed == b.fixed
                && a.min.max(b.min) < a.max.min(b.max)
            {
                overlaps += 1;
            }
        }
    }
    Ok((u64::from(crossed), overlaps))
}

fn crosses(a: &[RoutePoint], b: &[RoutePoint]) -> bool {
    fn straddles(leg: &[RoutePoint], pair: &[RoutePoint]) -> bool {
        let (a, b) = (leg[0], leg[1]);
        let cross = |p: RoutePoint| (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
        let [left, right] = [cross(pair[0]), cross(pair[1])];
        (left < 0.0 && right > 0.0) || (left > 0.0 && right < 0.0)
    }
    straddles(a, b) && straddles(b, a)
}

#[cfg(test)]
mod tests;
