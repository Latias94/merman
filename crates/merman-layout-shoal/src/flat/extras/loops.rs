use super::*;
use crate::Spacing;

pub(super) fn draw(
    edge: &mut EdgeGeometry,
    ports: [Port; 2],
    spec: &EdgeSpec,
    depth: f64,
    spacing: &Spacing,
    walls: &RouteWalls,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let [a, b] = ports;
    if a == b {
        let marker = MarkerMetrics {
            length: spec.start_marker.length.max(spec.end_marker.length),
            width: 0.0,
        };
        let path = coincident_path(a, marker, depth, spacing, walls, work)?;
        return write_route(edge, ports, &path, work);
    }

    let mut via = [advance(a.at, a.side, depth)?, advance(b.at, b.side, depth)?];
    for p in &mut via {
        let mut blocked = false;
        let mut far = projection(*p, a.side);
        for &wall in &walls.0 {
            work.visit(1)?;
            blocked |= wall.x[0] < p.x && p.x < wall.x[1] && wall.y[0] < p.y && p.y < wall.y[1];
            far = far.max(wall.along(a.side).1);
        }
        if blocked {
            *p = on_normal_line(*p, a.side, finite(far + spacing.edge)?);
        }
    }
    redraw(edge, ports, spec, &via, spacing.edge, walls, work)
}

fn projection(p: Point, side: Side) -> f64 {
    Bounds::between(p, p).along(side).1
}

fn on_normal_line(p: Point, side: Side, line: f64) -> Point {
    match side {
        Side::Right => Point { x: line, ..p },
        Side::Left => Point { x: -line, ..p },
        Side::Bottom => Point { y: line, ..p },
        Side::Top => Point { y: -line, ..p },
    }
}

fn ring(fork: Point, outer: Point, across: Side, radius: f64) -> LayoutResult<[Point; 6]> {
    Ok([
        fork,
        advance(fork, across, -radius)?,
        advance(outer, across, -radius)?,
        advance(outer, across, radius)?,
        advance(fork, across, radius)?,
        fork,
    ])
}

fn coincident_path(
    port: Port,
    marker: MarkerMetrics,
    depth: f64,
    spacing: &Spacing,
    walls: &RouteWalls,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<Point>> {
    let side = port.side;
    let across = match side.axis() {
        Axis::Horizontal => Side::Bottom,
        Axis::Vertical => Side::Right,
    };
    let radius = spacing
        .loop_gap
        .max(spacing.edge)
        .max(spacing.port_pitch / 2.0)
        .max(depth / 4.0);
    let gap = if marker.length == 0.0 && spacing.edge == 0.0 {
        radius
    } else {
        spacing.edge
    };
    let lead = port.stub(marker, gap, walls, work)?;
    let outer = advance(port.at, side, depth)?;
    let outer = if projection(outer, side) > projection(lead, side) {
        outer
    } else {
        advance(lead, side, radius)?
    };
    let local = ring(lead, outer, across, radius)?;
    let mut clear = true;
    for leg in local.windows(2) {
        if !walls.clear_leg(leg[0], leg[1], work)? {
            clear = false;
            break;
        }
    }
    if clear {
        work.charge(local.len())?;
        let mut path = reserved(local.len())?;
        path.extend(local);
        return Ok(path);
    }

    // Route one shared lead to open space, then mirror it on the return. Keep the
    // connector off the required shaft and the loop beyond the connector's first exit.
    let count = walls
        .0
        .len()
        .checked_add(1)
        .ok_or(WorkError::ArithmeticOverflow)?;
    work.charge(count)?;
    let mut blockers = reserved(count)?;
    let mut far = projection(lead, side);
    for &wall in &walls.0 {
        work.visit(1)?;
        far = far.max(wall.along(side).1);
        blockers.push(wall);
    }
    let mut shaft = Bounds::between(port.at, lead);
    match side.axis() {
        Axis::Horizontal => {
            shaft.y = [finite(port.at.y.next_down())?, finite(port.at.y.next_up())?];
        }
        Axis::Vertical => {
            shaft.x = [finite(port.at.x.next_down())?, finite(port.at.x.next_up())?];
        }
    }
    match side {
        Side::Right => shaft.x[0] = finite(shaft.x[0].next_down())?,
        Side::Left => shaft.x[1] = finite(shaft.x[1].next_up())?,
        Side::Top => shaft.y[1] = finite(shaft.y[1].next_up())?,
        Side::Bottom => shaft.y[0] = finite(shaft.y[0].next_down())?,
    }
    blockers.push(shaft);
    let exit = finite((far + spacing.edge).max(far.next_up()))?;
    let gate = on_normal_line(lead, side, exit);
    let mut path = corridor::path(lead, gate, &[], &RouteWalls(blockers), spacing.edge, work)?;
    let mut used = 0;
    for p in &mut path {
        work.visit(1)?;
        used += 1;
        if projection(*p, side) >= exit {
            *p = on_normal_line(*p, side, exit);
            break;
        }
    }
    path.truncate(used);
    let fork = path.last().copied().ok_or(corridor::NO_CORRIDOR)?;
    let closed = ring(fork, advance(fork, side, radius)?, across, radius)?;
    let extra = used
        .checked_add(closed.len() - 2)
        .ok_or(WorkError::ArithmeticOverflow)?;
    work.charge(extra)?;
    path.try_reserve_exact(extra)
        .map_err(|_| LayoutError::AllocationLimit)?;
    path.extend_from_slice(&closed[1..]);
    for i in (0..used - 1).rev() {
        path.push(path[i]);
    }
    Ok(path)
}

pub(super) fn place_label(
    id: u32,
    edge: &EdgeGeometry,
    [a, b]: [Port; 2],
    size: Size,
    gap: f64,
    walls: &Obstacles,
    work: &mut Work<'_>,
) -> LayoutResult<Point> {
    let side = a.side;
    let at = Point {
        x: a.at.x.midpoint(b.at.x),
        y: a.at.y.midpoint(b.at.y),
    };
    let mut front = projection(at, side);
    for &knot in &edge.points {
        work.visit(1)?;
        front = front.max(projection(point(knot), side));
    }
    let half = match side.axis() {
        Axis::Horizontal => size.width / 2.0,
        Axis::Vertical => size.height / 2.0,
    };
    let preferred = on_normal_line(at, side, finite(front + gap + half)?);
    loop_label(id, preferred, size, side, walls, work)
}
