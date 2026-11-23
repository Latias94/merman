//! Specialized flat routes and labels, after placement in canonical Down coordinates.
//!
//! Ordinary links take a linear eligibility pass. For each exceptional route, elbow and
//! exterior-corridor checks scan the B obstacle boxes. Only blocked corridors build the
//! coordinate grid; its checked product is charged before allocation. A halo that seals a
//! passage can retry once with exact walls. A deferred label in an unreachable pocket can
//! retry once outside the drawing. Both retries consume the same work budget.
//!
//! Label placement checks boxes and previously routed segment envelopes. With K such
//! obstacles, one outward slide sorts at most K intervals in O(K log K) time and O(K)
//! temporary storage. No search advances one drawing unit at a time.

mod corridor;
mod obstacles;
#[cfg(test)]
mod tests;

use super::{Component, Drawing, Expanded, canonical_size, to_canonical_port};
use crate::work::{LayoutResult, Work, filled, push, reserved};
use crate::{
    Axis, Direction, EdgeGeometry, EdgeSpec, Graph, LayoutError, MarkerMetrics, NodeSpec, Options,
    Point, PortSideHint, Rect, RoutePoint, Side, Size, WorkError,
};
use obstacles::{Bounds, Obstacles, Owner, RouteWalls};

const PRECISION: LayoutError =
    LayoutError::InvalidInput("extra route exceeds coordinate precision");
fn finite(v: f64) -> LayoutResult<f64> {
    if v.is_finite() {
        Ok(v)
    } else {
        Err(WorkError::ArithmeticOverflow.into())
    }
}
fn advance(p: Point, side: Side, d: f64) -> LayoutResult<Point> {
    let q = match side {
        Side::Left => Point {
            x: finite(p.x - d)?,
            ..p
        },
        Side::Right => Point {
            x: finite(p.x + d)?,
            ..p
        },
        Side::Top => Point {
            y: finite(p.y - d)?,
            ..p
        },
        Side::Bottom => Point {
            y: finite(p.y + d)?,
            ..p
        },
    };
    if d != 0.0 && p == q {
        return Err(PRECISION);
    }
    Ok(q)
}
fn point(p: RoutePoint) -> Point {
    Point { x: p.x, y: p.y }
}
fn route_point(p: Point, axis: Axis) -> RoutePoint {
    RoutePoint {
        x: p.x,
        y: p.y,
        axis,
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Port {
    at: Point,
    side: Side,
}
impl Port {
    fn marker(self, m: MarkerMetrics) -> LayoutResult<Bounds> {
        let tip = advance(self.at, self.side, m.length)?;
        let half = m.width / 2.0;
        let bounds = match self.side.axis() {
            Axis::Horizontal => Bounds {
                x0: self.at.x.min(tip.x),
                x1: self.at.x.max(tip.x),
                y0: finite(self.at.y - half)?,
                y1: finite(self.at.y + half)?,
            },
            Axis::Vertical => Bounds {
                x0: finite(self.at.x - half)?,
                x1: finite(self.at.x + half)?,
                y0: self.at.y.min(tip.y),
                y1: self.at.y.max(tip.y),
            },
        };
        if m.width > 0.0 && m.length > 0.0 && (bounds.x0 >= bounds.x1 || bounds.y0 >= bounds.y1) {
            return Err(PRECISION);
        }
        Ok(bounds)
    }
    fn stub(
        self,
        marker: MarkerMetrics,
        gap: f64,
        walls: &RouteWalls,
        work: &mut Work<'_>,
    ) -> LayoutResult<Point> {
        let desired = finite(marker.length + gap)?;
        let mut tip = advance(self.at, self.side, desired)?;
        for &b in &walls.0 {
            work.visit(1)?;
            if b.blocks(self.at, tip) {
                match self.side {
                    Side::Right => tip.x = b.x0.max(self.at.x),
                    Side::Left => tip.x = b.x1.min(self.at.x),
                    Side::Bottom => tip.y = b.y0.max(self.at.y),
                    Side::Top => tip.y = b.y1.min(self.at.y),
                }
            }
        }
        let room = (tip.x - self.at.x).abs() + (tip.y - self.at.y).abs();
        if room < marker.length || (desired > 0.0 && room == 0.0) {
            return Err(LayoutError::InvalidInput(
                "requested port has no room for its marker shaft",
            ));
        }
        Ok(tip)
    }
}
fn old_ports(edge: &EdgeGeometry) -> LayoutResult<[Port; 2]> {
    let first = edge
        .points
        .first()
        .ok_or(LayoutError::InvalidInput("missing source contact"))?;
    let last = edge
        .points
        .last()
        .ok_or(LayoutError::InvalidInput("missing target contact"))?;
    Ok([
        Port {
            at: point(*first),
            side: edge.ports.0,
        },
        Port {
            at: point(*last),
            side: edge.ports.1,
        },
    ])
}
fn named(hint: PortSideHint) -> Option<Side> {
    match hint {
        PortSideHint::Auto => None,
        PortSideHint::Side(s) | PortSideHint::FixedOffset { side: s, .. } => Some(s),
    }
}

fn resolve(
    old: Port,
    hint: PortSideHint,
    node: &NodeSpec,
    rect: Rect,
    dir: Direction,
) -> LayoutResult<Port> {
    let hint = to_canonical_port(dir, hint);
    let side = named(hint).unwrap_or(old.side);
    // anchor_y is measured from the physical top, even when rank coordinates are reflected.
    let mut offset = if let PortSideHint::FixedOffset { offset, .. } = hint {
        Some(offset)
    } else {
        None
    };
    if offset.is_none()
        && let Some(anchor) = node.anchor_y
    {
        for s in [Side::Left, Side::Right] {
            if let PortSideHint::FixedOffset {
                side: face,
                offset: off,
            } = to_canonical_port(
                dir,
                PortSideHint::FixedOffset {
                    side: s,
                    offset: anchor - node.size.height / 2.0,
                },
            ) && face == side
            {
                offset = Some(off);
                break;
            }
        }
    }
    if side == old.side && offset.is_none() {
        return Ok(old);
    }
    let center = rect.center();
    let off = offset.unwrap_or(0.0);
    let at = match side {
        Side::Left => Point {
            x: rect.x,
            y: finite(center.y + off)?,
        },
        Side::Right => Point {
            x: finite(rect.x + rect.width)?,
            y: finite(center.y + off)?,
        },
        Side::Top => Point {
            x: finite(center.x + off)?,
            y: rect.y,
        },
        Side::Bottom => Point {
            x: finite(center.x + off)?,
            y: finite(rect.y + rect.height)?,
        },
    };
    Ok(Port { at, side })
}

fn loop_geometry(
    rect: Rect,
    l: &super::Loop,
    nth: usize,
    spec: &EdgeSpec,
    dir: Direction,
    options: &Options,
) -> LayoutResult<EdgeGeometry> {
    let side = if l.right { Side::Right } else { Side::Left };
    let face = if l.right {
        finite(rect.x + rect.width)?
    } else {
        rect.x
    };
    let center = rect.center();
    let f = 0.5 - 0.25 / (nth + 1) as f64;
    let a = Point {
        x: face,
        y: finite(center.y - rect.height * f)?,
    };
    let b = Point {
        x: face,
        y: finite(center.y + rect.height * f)?,
    };
    let label = spec.label.map(|s| canonical_size(dir, s));
    let width = label.unwrap_or_default().width;
    // validate::components reserves depth, measured width and loop_gap cumulatively per side.
    let lane = advance(
        a,
        side,
        (l.reach - width - options.spacing.loop_gap).max(0.0),
    )?;
    let label_center = if label.is_some() {
        Some(advance(
            Point {
                x: face,
                y: center.y,
            },
            side,
            l.reach - width / 2.0,
        )?)
    } else {
        None
    };
    let mut points = reserved(4)?;
    points.extend([
        route_point(a, Axis::Horizontal),
        route_point(lane, Axis::Horizontal),
        route_point(Point { x: lane.x, y: b.y }, Axis::Vertical),
        route_point(b, Axis::Horizontal),
    ]);
    Ok(EdgeGeometry {
        points,
        ports: (side, side),
        label_center,
        start_label_center: None,
        end_label_center: None,
        reversed_for_ranking: false,
    })
}

fn shortest_slide(
    at: Point,
    size: Size,
    sides: [Side; 2],
    id: u32,
    walls: &Obstacles,
    work: &mut Work<'_>,
) -> LayoutResult<Point> {
    let (a, da) = walls.slide(at, size, sides[0], Some(id), work)?;
    if da == 0.0 {
        return Ok(a);
    }
    let (b, db) = walls.slide(at, size, sides[1], Some(id), work)?;
    Ok(if db < da { b } else { a })
}

fn loop_label(
    id: u32,
    at: Point,
    size: Size,
    side: Side,
    walls: &Obstacles,
    work: &mut Work<'_>,
) -> LayoutResult<Point> {
    if walls.fits(Bounds::centered(at, size)?, Some(id), work)? {
        return Ok(at);
    }
    let directions = match side.axis() {
        Axis::Horizontal => [Side::Top, Side::Bottom],
        Axis::Vertical => [Side::Left, Side::Right],
    };
    shortest_slide(at, size, directions, id, walls, work)
}

fn choose_direct_label(
    id: u32,
    a: Point,
    b: Point,
    size: Size,
    gap: f64,
    walls: &Obstacles,
    work: &mut Work<'_>,
) -> LayoutResult<Point> {
    let mid = Point {
        x: a.x.midpoint(b.x),
        y: a.y.midpoint(b.y),
    };
    if walls.fits(Bounds::centered(mid, size)?, Some(id), work)? {
        return Ok(mid);
    }
    shortest_slide(mid, size, [Side::Left, Side::Right], id, walls, work).and_then(|p| {
        // Leave an aisle along the chosen side of a blocking node when space permits.
        let side = if p.x < mid.x { Side::Left } else { Side::Right };
        let padded = advance(p, side, gap)?;
        Ok(
            if walls.fits(Bounds::centered(padded, size)?, Some(id), work)? {
                padded
            } else {
                p
            },
        )
    })
}

fn redraw(
    edge: &mut EdgeGeometry,
    ports: [Port; 2],
    spec: &EdgeSpec,
    via: &[Point],
    gap: f64,
    walls: &RouteWalls,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let [a, b] = ports;
    let start = a.stub(spec.start_marker, gap, walls, work)?;
    let end = b.stub(spec.end_marker, gap, walls, work)?;
    let path = corridor::path(start, end, via, walls, gap, work)?;
    // Remove strictly collinear intermediate grid points. Keep the terminals and both stubs
    // explicit so marker trimming cannot move the first or last bend onto a box face.
    let count = path
        .len()
        .checked_add(2)
        .ok_or(WorkError::ArithmeticOverflow)?;
    work.charge(count)?;
    let mut points = reserved(count)?;
    points.push(route_point(a.at, a.side.axis()));
    let mut prev = start;
    for (i, &p) in path.iter().enumerate() {
        work.visit(1)?;
        if i > 0 && i + 1 < path.len() {
            let n = path[i + 1];
            if (prev.x == p.x && p.x == n.x && p.y >= prev.y.min(n.y) && p.y <= prev.y.max(n.y))
                || (prev.y == p.y && p.y == n.y && p.x >= prev.x.min(n.x) && p.x <= prev.x.max(n.x))
            {
                continue;
            }
        }
        let axis = if i == 0 {
            a.side.axis()
        } else if prev.y == p.y {
            Axis::Horizontal
        } else {
            Axis::Vertical
        };
        points.push(route_point(p, axis));
        prev = p;
    }
    points.push(route_point(b.at, b.side.axis()));
    edge.points = points;
    edge.ports = (a.side, b.side);
    Ok(())
}

fn loop_waypoints(
    ports: [Port; 2],
    depth: f64,
    spacing: &crate::Spacing,
    walls: &RouteWalls,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<Point>> {
    let [a, b] = ports;
    if a.side != b.side {
        return Ok(Vec::new());
    }
    let outside = advance(a.at, a.side, depth)?;
    let mut via = reserved(4)?;
    if a.at != b.at {
        via.extend([outside, advance(b.at, b.side, depth)?]);
    } else {
        // Coincident fixed anchors (or a point node) still get a loop with area.
        // The two shafts can share a contact; the upper and lower branches differ.
        let across = if a.side.axis() == Axis::Horizontal {
            Side::Bottom
        } else {
            Side::Right
        };
        let radius = spacing
            .loop_gap
            .max(spacing.edge)
            .max(spacing.port_pitch / 2.0)
            .max(depth / 4.0);
        let inner = advance(a.at, a.side, depth / 2.0)?;
        via.extend([
            advance(outside, across, -radius)?,
            advance(inner, across, -radius)?,
            advance(inner, across, radius)?,
            advance(outside, across, radius)?,
        ]);
    }
    for p in &mut via {
        let mut blocked = false;
        let mut far = match a.side {
            Side::Right => p.x,
            Side::Left => -p.x,
            Side::Bottom => p.y,
            Side::Top => -p.y,
        };
        for &r in &walls.0 {
            work.visit(1)?;
            blocked |= r.x0 < p.x && p.x < r.x1 && r.y0 < p.y && p.y < r.y1;
            far = far.max(r.along(a.side).1);
        }
        if blocked {
            let line = finite(far + spacing.edge)?;
            match a.side {
                Side::Right => p.x = line,
                Side::Left => p.x = -line,
                Side::Bottom => p.y = line,
                Side::Top => p.y = -line,
            }
        }
    }
    Ok(via)
}

fn end_label(
    edge: &EdgeGeometry,
    end: bool,
    size: Size,
    marker: MarkerMetrics,
    options: &Options,
    walls: &Obstacles,
    work: &mut Work<'_>,
) -> LayoutResult<Point> {
    let port = old_ports(edge)?[usize::from(end)];
    let vertical = port.side.axis() == Axis::Vertical;
    let (depth, width) = if vertical {
        (size.height, size.width)
    } else {
        (size.width, size.height)
    };
    let start = advance(
        port.at,
        port.side,
        finite(marker.length + options.spacing.end_label_along + depth / 2.0)?,
    )?;
    let cross = finite(options.spacing.end_label_across + width / 2.0)?;
    let mut positive = true;
    for j in 1..edge.points.len() {
        work.visit(1)?;
        let p = edge.points[if end { edge.points.len() - 1 - j } else { j }];
        let (a, b) = if vertical {
            (p.x, port.at.x)
        } else {
            (p.y, port.at.y)
        };
        if a != b {
            positive = a < b;
            break;
        }
    }
    let sides = match (vertical, positive) {
        (true, true) => [Side::Right, Side::Left],
        (true, false) => [Side::Left, Side::Right],
        (false, true) => [Side::Bottom, Side::Top],
        (false, false) => [Side::Top, Side::Bottom],
    };
    let mut best = None;
    for side in sides {
        let at = advance(start, side, cross)?;
        if walls.fits(Bounds::centered(at, size)?, None, work)? {
            return Ok(at);
        }
        let (p, d) = walls.slide(at, size, port.side, None, work)?;
        if best.is_none_or(|(_, old)| d < old) {
            best = Some((p, d));
        }
    }
    Ok(best.expect("two label sides").0)
}

fn endpoints(index: usize, c: &Component) -> [usize; 2] {
    if let Some(e) = c.links.get(index) {
        [e.from as usize, e.to as usize]
    } else {
        [c.loops[index - c.links.len()].node as usize; 2]
    }
}
fn ports_for(
    edge: &EdgeGeometry,
    spec: &EdgeSpec,
    index: usize,
    c: &Component,
    nodes: &[Rect],
    dir: Direction,
) -> LayoutResult<[Port; 2]> {
    let old = old_ports(edge)?;
    let [a, b] = endpoints(index, c);
    Ok([
        resolve(old[0], spec.source_port, &c.nodes[a], nodes[a], dir)?,
        resolve(old[1], spec.target_port, &c.nodes[b], nodes[b], dir)?,
    ])
}

pub(super) fn finish(
    drawing: &mut Drawing,
    g: &Expanded,
    c: &Component,
    graph: &Graph,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    work.check()?;
    let mut extra = !c.loops.is_empty();
    let mut has_end_labels = false;
    for (i, l) in c.links.iter().enumerate() {
        work.visit(1)?;
        let spec = &graph.edges[l.edge as usize];
        has_end_labels |= spec.start_label.is_some() || spec.end_label.is_some();
        extra |= g.direct[i]
            || spec.source_port != PortSideHint::Auto
            || spec.target_port != PortSideHint::Auto
            || c.nodes[l.from as usize].anchor_y.is_some()
            || c.nodes[l.to as usize].anchor_y.is_some();
    }
    if !extra && !has_end_labels {
        return Ok(());
    }
    let mut walls = Obstacles::new();
    for &rect in &drawing.nodes {
        walls.add(Owner::Node, Bounds::of(rect)?, work)?;
    }
    for (i, (id, edge)) in drawing.edges.iter().enumerate() {
        work.visit(1)?;
        let spec = &graph.edges[*id as usize];
        if !g.direct[i]
            && let (Some(size), Some(p)) = (spec.label, edge.label_center)
        {
            walls.add(
                Owner::Middle(*id),
                Bounds::centered(p, canonical_size(graph.direction, size))?,
                work,
            )?;
        }
    }
    if !c.loops.is_empty() {
        work.charge(
            c.nodes
                .len()
                .checked_add(c.loops.len())
                .ok_or(WorkError::ArithmeticOverflow)?,
        )?;
        drawing
            .edges
            .try_reserve(c.loops.len())
            .map_err(|_| LayoutError::AllocationLimit)?;
        let mut counts = filled(c.nodes.len(), [0usize; 2])?;
        for l in &c.loops {
            work.visit(1)?;
            let n = l.node as usize;
            let count = &mut counts[n][usize::from(l.right)];
            let spec = &graph.edges[l.edge as usize];
            let edge = loop_geometry(drawing.nodes[n], l, *count, spec, graph.direction, options)?;
            *count += 1;
            drawing.edges.push((l.edge, edge));
            has_end_labels |= spec.start_label.is_some() || spec.end_label.is_some();
        }
    }
    // Resolve all contacts before collecting marker walls, including markers on later edges.
    work.charge(drawing.edges.len())?;
    let mut ports = reserved(drawing.edges.len())?;
    let mut placing_labels = has_end_labels;
    for (i, (id, edge)) in drawing.edges.iter().enumerate() {
        work.visit(1)?;
        let spec = &graph.edges[*id as usize];
        let pair = ports_for(edge, spec, i, c, &drawing.nodes, graph.direction)?;
        for (port, m) in pair.into_iter().zip([spec.start_marker, spec.end_marker]) {
            if m.length > 0.0 && m.width > 0.0 {
                walls.add(Owner::Marker(*id), port.marker(m)?, work)?;
            }
        }
        let direct = g.direct.get(i).copied().unwrap_or(false);
        placing_labels |= spec.label.is_some() && (direct || i >= c.links.len());
        ports.push((pair, direct || old_ports(edge)? != pair));
    }
    // A deferred box must also clear ordinary dummy-chain strokes, which are already final.
    // Segment envelopes conservatively include the monotone curve between two route knots.
    if placing_labels {
        for ((id, edge), (_, redraw)) in drawing.edges.iter().zip(&ports) {
            work.visit(1)?;
            if !redraw {
                walls.add_stroke(*id, edge, work)?;
            }
        }
    }
    for ((id, edge), &(pair, changed)) in drawing.edges[c.links.len()..]
        .iter_mut()
        .zip(&ports[c.links.len()..])
    {
        work.visit(1)?;
        let spec = &graph.edges[*id as usize];
        if let (Some(at), Some(size)) = (edge.label_center, spec.label) {
            let size = canonical_size(graph.direction, size);
            let mut side = edge.ports.0;
            let at = if changed && pair[0].side == pair[1].side {
                side = pair[0].side;
                let center = Point {
                    x: pair[0].at.x.midpoint(pair[1].at.x),
                    y: pair[0].at.y.midpoint(pair[1].at.y),
                };
                let depth = (edge.points[1].x - edge.points[0].x).abs();
                let half = if side.axis() == Axis::Horizontal {
                    size.width / 2.0
                } else {
                    size.height / 2.0
                };
                advance(
                    center,
                    side,
                    finite(depth + options.spacing.loop_gap + half)?,
                )?
            } else {
                at
            };
            let p = loop_label(*id, at, size, side, &walls, work)?;
            edge.label_center = Some(p);
            walls.add(Owner::Middle(*id), Bounds::centered(p, size)?, work)?;
        }
    }
    let gap = options.spacing.edge;
    for (i, ((id, edge), (pair, changed))) in drawing.edges.iter_mut().zip(ports).enumerate() {
        work.visit(1)?;
        let direct = g.direct.get(i).copied().unwrap_or(false);
        let is_loop = i >= c.links.len();
        if !changed && !is_loop {
            continue;
        }
        let spec = &graph.edges[*id as usize];
        if direct && let Some(size) = spec.label {
            let size = canonical_size(graph.direction, size);
            let p = choose_direct_label(*id, pair[0].at, pair[1].at, size, gap, &walls, work)?;
            edge.label_center = Some(p);
        }
        let blockers = walls.for_route(*id, pair, work)?;
        let loop_via;
        let label = edge.label_center;
        let via = if is_loop {
            if !changed && pair[0].at != pair[1].at {
                let mut clear = true;
                for leg in edge.points.windows(2) {
                    if !blockers.clear_leg(point(leg[0]), point(leg[1]), work)? {
                        clear = false;
                        break;
                    }
                }
                if clear {
                    continue;
                }
            }
            if pair[0].side != pair[1].side {
                label.as_slice()
            } else {
                let depth = (edge.points[1].x - edge.points[0].x).abs();
                loop_via = loop_waypoints(pair, depth, &options.spacing, &blockers, work)?;
                &loop_via[..]
            }
        } else {
            label.as_slice()
        };
        let result = redraw(edge, pair, spec, via, gap, &blockers, work);
        let deferred = spec
            .label
            .filter(|_| direct)
            .map(|size| canonical_size(graph.direction, size));
        if let (Err(corridor::NO_CORRIDOR), Some(size), Some(mid)) = (result, deferred, label) {
            // A free label box may sit inside a sealed pocket. An exterior anchor is
            // reachable from the unbounded corridor and reserves the full measured width.
            let outside = walls.exterior(mid, size, gap, work)?;
            redraw(
                edge,
                pair,
                spec,
                std::slice::from_ref(&outside),
                gap,
                &blockers,
                work,
            )?;
            edge.label_center = Some(outside);
        } else {
            result?;
        }
        if let (Some(size), Some(p)) = (deferred, edge.label_center) {
            walls.add(Owner::Middle(*id), Bounds::centered(p, size)?, work)?;
        }
        if placing_labels {
            walls.add_stroke(*id, edge, work)?;
        }
    }
    if has_end_labels {
        for (id, edge) in &mut drawing.edges {
            work.visit(1)?;
            let spec = &graph.edges[*id as usize];
            for (end, label, m) in [
                (false, spec.start_label, spec.start_marker),
                (true, spec.end_label, spec.end_marker),
            ] {
                if let Some(size) = label {
                    let size = canonical_size(graph.direction, size);
                    let p = end_label(edge, end, size, m, options, &walls, work)?;
                    if end {
                        edge.end_label_center = Some(p)
                    } else {
                        edge.start_label_center = Some(p)
                    }
                    walls.add(Owner::End, Bounds::centered(p, size)?, work)?;
                }
            }
        }
    }
    Ok(())
}
