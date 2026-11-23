use super::*;
use crate::work::push;
use crate::{MarkerMetrics, Options};
use fit::blocks;

fn opposite(side: Side) -> Side {
    match side {
        Side::Top => Side::Bottom,
        Side::Right => Side::Left,
        Side::Bottom => Side::Top,
        Side::Left => Side::Right,
    }
}

fn travel(wire: &Wire, end: usize) -> Contact {
    let contact = wire.contacts[end].expect("resolved contact");
    if matches!(wire.ends[end], End::Border { .. }) {
        Contact {
            side: opposite(contact.side),
            ..contact
        }
    } else {
        contact
    }
}

fn snap(edge: &mut EdgeGeometry, end: usize, contact: Contact) {
    let index = if end == 0 { 0 } else { edge.points.len() - 1 };
    let next = if end == 0 { 1 } else { index - 1 };
    let old = point(edge.points[index]);
    if edge.points.len() > 2 {
        let near = &mut edge.points[next];
        if contact.side.axis() == Axis::Vertical && near.x == old.x {
            near.x = contact.at.x;
        } else if contact.side.axis() == Axis::Horizontal && near.y == old.y {
            near.y = contact.at.y;
        }
    }
    edge.points[index] = RoutePoint {
        x: contact.at.x,
        y: contact.at.y,
        axis: contact.side.axis(),
    };
    if end == 0 {
        edge.ports.0 = contact.side;
    } else {
        edge.ports.1 = contact.side;
    }
}

fn position(
    wire: &mut Wire,
    nodes: &[Rect],
    ports: &[Point],
    offset: Point,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    if let Some(route) = wire.route.as_mut() {
        work.charge(route.points.len())?;
        for knot in &mut route.points {
            knot.x += offset.x;
            knot.y += offset.y;
        }
        for center in [
            &mut route.label_center,
            &mut route.start_label_center,
            &mut route.end_label_center,
        ]
        .into_iter()
        .flatten()
        {
            *center = translated(*center, offset);
        }
    }
    for end in 0..2 {
        let contact = match wire.ends[end] {
            End::Border { slot, side, .. } => Contact {
                at: ports[slot],
                side,
            },
            End::Item { slot, child, .. } => {
                let rect = nodes[slot];
                let contact = if let Some(child) = child {
                    Contact {
                        at: translated(
                            child.at,
                            Point {
                                x: rect.x,
                                y: rect.y,
                            },
                        ),
                        ..child
                    }
                } else {
                    let inner = wire.contacts[end].expect("resolved item");
                    let at = cross(inner.side, translated(inner.at, offset));
                    Contact {
                        at: face(rect, inner.side, at),
                        ..inner
                    }
                };
                if let Some(route) = wire.route.as_mut() {
                    snap(route, end, contact);
                }
                contact
            }
        };
        wire.contacts[end] = Some(contact);
    }
    Ok(())
}

fn marker_rect(port: Contact, marker: MarkerMetrics) -> Rect {
    let tip = towards(port.at, port.side, marker.length);
    if port.side.axis() == Axis::Vertical {
        Rect {
            x: port.at.x - marker.width / 2.0,
            y: port.at.y.min(tip.y),
            width: marker.width,
            height: marker.length,
        }
    } else {
        Rect {
            x: port.at.x.min(tip.x),
            y: port.at.y - marker.width / 2.0,
            width: marker.length,
            height: marker.width,
        }
    }
}

struct Walls {
    boxes: Vec<Rect>,
    markers: Vec<(usize, Contact, Rect)>,
    title: Option<Rect>,
}

impl Walls {
    fn new(
        nodes: &[Rect],
        wires: &[Wire],
        title: Option<Rect>,
        work: &mut Work<'_>,
    ) -> LayoutResult<Self> {
        work.charge(nodes.len())?;
        let mut boxes = reserved(nodes.len())?;
        boxes.extend(
            nodes
                .iter()
                .copied()
                .filter(|rect| rect.width > 0.0 && rect.height > 0.0),
        );
        let mut markers = Vec::new();
        for (index, wire) in wires.iter().enumerate() {
            work.visit(1)?;
            if let Some(route) = &wire.route {
                for (center, size) in [
                    (route.label_center, wire.spec.label),
                    (route.start_label_center, wire.spec.start_label),
                    (route.end_label_center, wire.spec.end_label),
                ] {
                    if let (Some(center), Some(size)) = (center, size) {
                        push(&mut boxes, centered(center, size))?;
                    }
                }
            }
            for (end, marker) in [wire.spec.start_marker, wire.spec.end_marker]
                .into_iter()
                .enumerate()
            {
                if marker.width > 0.0 && marker.length > 0.0 {
                    let port = travel(wire, end);
                    push(&mut markers, (index, port, marker_rect(port, marker)))?;
                }
            }
        }
        Ok(Self {
            boxes,
            markers,
            title,
        })
    }

    fn route(
        &self,
        index: usize,
        pair: [Contact; 2],
        work: &mut Work<'_>,
    ) -> LayoutResult<Vec<Rect>> {
        let count = self
            .boxes
            .len()
            .checked_add(self.markers.len())
            .and_then(|n| n.checked_add(1))
            .ok_or(WorkError::ArithmeticOverflow)?;
        work.charge(count)?;
        let mut result = reserved(count)?;
        result.extend_from_slice(&self.boxes);
        for &(owner, marker, rect) in &self.markers {
            work.visit(1)?;
            if index != owner
                && !pair.iter().any(|port| {
                    port.side == marker.side
                        && match marker.side.axis() {
                            Axis::Vertical => {
                                port.at.y == marker.at.y
                                    && port.at.x >= rect.x
                                    && port.at.x <= rect.x + rect.width
                            }
                            Axis::Horizontal => {
                                port.at.x == marker.at.x
                                    && port.at.y >= rect.y
                                    && port.at.y <= rect.y + rect.height
                            }
                        }
                })
            {
                result.push(rect);
            }
        }
        if let Some(title) = self.title {
            let on_title = |port: &Contact| {
                let point = port.at;
                point.x >= title.x
                    && point.x <= title.x + title.width
                    && point.y >= title.y
                    && point.y <= title.y + title.height
            };
            if !pair.iter().any(on_title) {
                result.push(title);
            }
        }
        Ok(result)
    }
}

fn overlaps(a: Rect, b: Rect) -> bool {
    a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y
}

fn labels(
    fit: &fit::Fit,
    wires: &[Wire],
    walls: &mut Walls,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<[Option<Point>; 3]>> {
    let mut placed = filled(wires.len(), [None; 3])?;
    for (index, seats) in fit.seats.iter().enumerate() {
        work.visit(1)?;
        for (label, &seat) in seats.iter().enumerate() {
            let Some(mut rect) = seat else { continue };
            if label > 0 {
                let end = label - 1;
                let wire = &wires[index];
                let port = travel(wire, end);
                let marker = [wire.spec.start_marker, wire.spec.end_marker][end];
                let size = Size {
                    width: rect.width,
                    height: rect.height,
                };
                let base = towards(
                    port.at,
                    port.side,
                    marker.length + options.spacing.end_label_along + depth(port.side, size) / 2.0,
                );
                let positive = if port.side.axis() == Axis::Vertical {
                    Side::Right
                } else {
                    Side::Bottom
                };
                for side in [positive, opposite(positive)] {
                    let center = towards(
                        base,
                        side,
                        across(port.side, size) / 2.0 + options.spacing.end_label_across,
                    );
                    let candidate = centered(center, size);
                    if candidate.x < 0.0
                        || candidate.y < 0.0
                        || candidate.x + candidate.width > fit.size.width
                        || candidate.y + candidate.height > fit.size.height
                    {
                        continue;
                    }
                    let mut clear = true;
                    for wall in walls
                        .boxes
                        .iter()
                        .chain(&walls.title)
                        .chain(walls.markers.iter().map(|entry| &entry.2))
                        .chain(fit.seats.iter().flatten().flatten())
                    {
                        work.visit(1)?;
                        if overlaps(*wall, candidate) {
                            clear = false;
                            break;
                        }
                    }
                    if !clear {
                        continue;
                    }
                    for wire in wires {
                        if let Some(route) = &wire.route {
                            for segment in route.points.windows(2) {
                                work.visit(1)?;
                                if blocks(candidate, point(segment[0]), point(segment[1])) {
                                    clear = false;
                                    break;
                                }
                            }
                        }
                        if !clear {
                            break;
                        }
                    }
                    if clear {
                        rect = candidate;
                        break;
                    }
                }
            }
            placed[index][label] = Some(rect.center());
            push(&mut walls.boxes, rect)?;
        }
    }
    Ok(placed)
}

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}

fn clear(path: &[Point], walls: &[Rect], work: &mut Work<'_>) -> LayoutResult<bool> {
    for segment in path.windows(2) {
        for &wall in walls {
            work.visit(1)?;
            if blocks(wall, segment[0], segment[1]) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn stub(
    port: Contact,
    marker: f64,
    gap: f64,
    walls: &[Rect],
    bounds: Size,
    work: &mut Work<'_>,
) -> LayoutResult<Point> {
    let mut tip = towards(port.at, port.side, finite(marker + gap)?);
    tip.x = tip.x.clamp(0.0, bounds.width);
    tip.y = tip.y.clamp(0.0, bounds.height);
    for &wall in walls {
        work.visit(1)?;
        if blocks(wall, port.at, tip) {
            match port.side {
                Side::Top => tip.y = (wall.y + wall.height).min(port.at.y),
                Side::Right => tip.x = wall.x.max(port.at.x),
                Side::Bottom => tip.y = wall.y.max(port.at.y),
                Side::Left => tip.x = (wall.x + wall.width).min(port.at.x),
            }
        }
    }
    let room = distance(port.at, tip);
    if room < marker
        && marker - room > f64::EPSILON * (1.0 + port.at.x.abs() + port.at.y.abs()) * 4.0
    {
        return Err(LayoutError::InvalidInput(
            "compound port has no room for its marker shaft",
        ));
    }
    Ok(tip)
}

fn knot(points: &mut Vec<Point>, point: Point) -> LayoutResult<()> {
    if points.last() == Some(&point) {
        return Ok(());
    }
    if let [.., before, last] = points.as_mut_slice()
        && ((before.x == last.x
            && last.x == point.x
            && last.y >= before.y.min(point.y)
            && last.y <= before.y.max(point.y))
            || (before.y == last.y
                && last.y == point.y
                && last.x >= before.x.min(point.x)
                && last.x <= before.x.max(point.x)))
    {
        *last = point;
        return Ok(());
    }
    push(points, point)
}

struct Connection<'a> {
    fit: &'a fit::Fit,
    walls: &'a [Rect],
    gap: f64,
}

impl Connection<'_> {
    fn path(
        &self,
        inner: Contact,
        outer: Contact,
        markers: [f64; 2],
        item: Option<Rect>,
        work: &mut Work<'_>,
    ) -> LayoutResult<Vec<Point>> {
        let mut points = reserved(6)?;
        points.push(inner.at);
        let length = distance(inner.at, outer.at);
        if inner.side == opposite(outer.side)
            && cross(inner.side, inner.at) == cross(inner.side, outer.at)
            && towards(inner.at, inner.side, length) == outer.at
            && length >= markers[0] + markers[1]
            && clear(&[inner.at, outer.at], self.walls, work)?
        {
            points.push(outer.at);
            return Ok(points);
        }
        let rim = self.fit.corridor[opposite(outer.side) as usize];
        let start = stub(inner, markers[0], self.gap, self.walls, self.fit.size, work)?;
        let end = stub(
            outer,
            markers[1],
            self.gap.max(rim),
            self.walls,
            self.fit.size,
            work,
        )?;
        knot(&mut points, start)?;
        if let Some(item) = item
            && rim > 0.0
            && inner.side == opposite(outer.side)
        {
            let vertical = outer.side.axis() == Axis::Vertical;
            let near = if vertical { item.x } else { item.y };
            let far = near + if vertical { item.width } else { item.height };
            let extent = across(outer.side, self.fit.size);
            let mut channels = [(near - self.gap).max(0.0), (far + self.gap).min(extent)];
            let cost = |value: f64| {
                (value - cross(outer.side, start)).abs() + (value - cross(outer.side, end)).abs()
            };
            if cost(channels[1]) < cost(channels[0]) {
                channels.swap(0, 1);
            }
            for line in channels {
                let via = if vertical {
                    [
                        start,
                        Point { x: line, ..start },
                        Point { x: line, ..end },
                        end,
                    ]
                } else {
                    [
                        start,
                        Point { y: line, ..start },
                        Point { y: line, ..end },
                        end,
                    ]
                };
                if clear(&via, self.walls, work)? {
                    for p in via {
                        knot(&mut points, p)?;
                    }
                    knot(&mut points, outer.at)?;
                    return Ok(points);
                }
            }
        }
        let path = flat::corridor(start, end, &[], self.walls, self.gap, work)?;
        for point in path {
            work.visit(1)?;
            knot(
                &mut points,
                Point {
                    x: point.x.clamp(0.0, self.fit.size.width),
                    y: point.y.clamp(0.0, self.fit.size.height),
                },
            )?;
        }
        knot(&mut points, outer.at)?;
        if points.len() == 1 {
            points.push(outer.at);
        }
        Ok(points)
    }
}

fn knots(
    mut path: Vec<Point>,
    reverse: bool,
    first: Axis,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<RoutePoint>> {
    work.charge(path.len())?;
    if reverse {
        path.reverse();
    }
    let mut points = reserved(path.len())?;
    let mut previous = path[0];
    for (index, at) in path.into_iter().enumerate() {
        let axis = if index == 0 || at == previous {
            first
        } else if at.y == previous.y {
            Axis::Horizontal
        } else {
            Axis::Vertical
        };
        points.push(RoutePoint {
            x: at.x,
            y: at.y,
            axis,
        });
        previous = at;
    }
    Ok(points)
}

struct TitleSweep {
    points: [Point; 3],
    remaining: usize,
    depth: f64,
}

fn title_sweep(
    edge: &EdgeGeometry,
    border: usize,
    outer: Contact,
    title: Rect,
    shaft: f64,
    work: &mut Work<'_>,
) -> LayoutResult<Option<TitleSweep>> {
    let clearance = fit::TITLE_CLEAR;
    if outer.side.axis() != Axis::Vertical
        || (outer.at.x > title.x - clearance && outer.at.x < title.x + title.width + clearance)
    {
        return Ok(None);
    }
    let len = edge.points.len();
    let index = |step: usize| if border == 0 { step } else { len - 1 - step };
    let dummy = point(edge.points[index(0)]);
    if dummy.x == outer.at.x {
        return Ok(None);
    }
    let inward = if outer.side == Side::Bottom {
        1.0
    } else {
        -1.0
    };
    let exit_y = if inward > 0.0 {
        title.y + title.height + clearance
    } else {
        title.y - clearance
    };
    let exit = Point {
        x: outer.at.x,
        y: exit_y,
    };
    let mut run = 0;
    let mut last = dummy;
    while run + 1 < len {
        work.visit(1)?;
        let next = point(edge.points[index(run + 1)]);
        if next.x != dummy.x || (next.y - last.y) * inward < 0.0 {
            break;
        }
        last = next;
        run += 1;
    }
    let join = if run + 1 == len {
        Point {
            y: last.y - inward * shaft,
            ..last
        }
    } else {
        last
    };
    if (exit.y - outer.at.y) * inward <= 0.0 {
        return Ok(None);
    }
    Ok(Some(TitleSweep {
        points: [outer.at, exit, join],
        remaining: run + usize::from(join == last),
        depth: (join.y - exit.y) * inward,
    }))
}

pub(super) fn title_room(
    wires: &[Wire],
    ports: &[Point],
    side: Side,
    fit: &fit::Fit,
    mut title: Rect,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<f64> {
    if options.routing != crate::RoutingStyle::Curved {
        return Ok(0.0);
    }
    title.x -= fit.offset.x;
    title.y -= fit.offset.y;
    let mut extra = 0.0_f64;
    for wire in wires {
        work.visit(1)?;
        let Some((end, slot, physical)) = wire.border() else {
            continue;
        };
        if physical != side || wire.own {
            continue;
        }
        let Some(edge) = &wire.route else { continue };
        let outer = Contact {
            at: Point {
                x: ports[slot].x - fit.offset.x,
                y: ports[slot].y - fit.offset.y,
            },
            side: opposite(side),
        };
        let marker = [wire.spec.end_marker, wire.spec.start_marker][end];
        if let Some(sweep) = title_sweep(
            edge,
            end,
            outer,
            title,
            marker.length + options.spacing.edge,
            work,
        )? {
            let across = (sweep.points[1].x - sweep.points[2].x).abs();
            extra = extra.max((across / 2.0).min(options.spacing.rank) - sweep.depth);
        }
    }
    finite(extra)
}

fn through_title(
    edge: &mut EdgeGeometry,
    border: usize,
    sweep: TitleSweep,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let len = edge.points.len();
    let remaining = sweep.remaining;
    let capacity = (len - remaining)
        .checked_add(3)
        .ok_or(WorkError::ArithmeticOverflow)?;
    work.charge(capacity)?;
    let mut points = reserved(capacity)?;
    let sweep = sweep.points.map(|at| RoutePoint {
        x: at.x,
        y: at.y,
        axis: Axis::Vertical,
    });
    if border == 0 {
        points.extend(sweep);
        points.extend_from_slice(&edge.points[remaining..]);
    } else {
        points.extend_from_slice(&edge.points[..len - remaining]);
        points.extend(sweep.into_iter().rev());
    }
    edge.points = points;
    Ok(())
}

fn boundary_leg(
    index: usize,
    wire: &mut Wire,
    nodes: &[Rect],
    walls: &Walls,
    fit: &fit::Fit,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<EdgeGeometry> {
    let (border, _, side) = wire.border().expect("leg has a border");
    let member = 1 - border;
    let outer = travel(wire, border);
    let inner = travel(wire, member);
    let markers = [wire.spec.start_marker.length, wire.spec.end_marker.length];
    let ports = (
        wire.contacts[0].unwrap().side,
        wire.contacts[1].unwrap().side,
    );
    let direction = if border == 0 {
        side.axis()
    } else {
        inner.side.axis()
    };
    if wire.flat() && !wire.own {
        let mut edge = wire.route.take().expect("flat border route");
        let index_in_route = if border == 0 {
            0
        } else {
            edge.points.len() - 1
        };
        let at = point(edge.points[index_in_route]);
        if at != outer.at {
            let dummy = Contact { at, side };
            let blockers = walls.route(index, [dummy, outer], work)?;
            if options.routing == crate::RoutingStyle::Curved
                && let Some(title) = walls.title
                && let Some(sweep) = title_sweep(
                    &edge,
                    border,
                    outer,
                    title,
                    markers[member] + options.spacing.edge,
                    work,
                )?
                && sweep.depth > 0.0
                && clear(&sweep.points, &blockers, work)?
            {
                through_title(&mut edge, border, sweep, work)?;
                edge.ports = ports;
                return Ok(edge);
            }
            let connection = Connection {
                fit,
                walls: &blockers,
                gap: options.spacing.edge,
            };
            let path = connection.path(dummy, outer, [0.0, markers[border]], None, work)?;
            let mut extension = knots(path, border == 0, side.axis(), work)?;
            if border == 0 {
                extension.pop();
                extension
                    .try_reserve(edge.points.len())
                    .map_err(|_| LayoutError::AllocationLimit)?;
                extension.append(&mut edge.points);
                edge.points = extension;
            } else {
                edge.points
                    .try_reserve(extension.len())
                    .map_err(|_| LayoutError::AllocationLimit)?;
                edge.points.extend(extension.into_iter().skip(1));
            }
        }
        edge.ports = ports;
        return Ok(edge);
    }
    let End::Item { slot, .. } = wire.ends[member] else {
        unreachable!()
    };
    let blockers = walls.route(index, [inner, outer], work)?;
    let connection = Connection {
        fit,
        walls: &blockers,
        gap: options.spacing.edge,
    };
    let path = connection.path(
        inner,
        outer,
        [markers[member], markers[border]],
        Some(nodes[slot]),
        work,
    )?;
    Ok(EdgeGeometry {
        points: knots(path, border == 0, direction, work)?,
        ports,
        label_center: None,
        start_label_center: None,
        end_label_center: None,
        reversed_for_ranking: wire
            .route
            .as_ref()
            .is_some_and(|edge| edge.reversed_for_ranking),
    })
}

pub(super) fn finish(
    fit: &fit::Fit,
    title: Option<Rect>,
    nodes: &[Rect],
    wires: &mut [Wire],
    ports: &[Point],
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<EdgeGeometry>> {
    work.charge(wires.len())?;
    let mut legs = reserved(wires.len())?;
    for wire in &mut *wires {
        position(wire, nodes, ports, fit.offset, work)?;
    }
    if ports.is_empty() {
        legs.extend(
            wires
                .iter_mut()
                .map(|wire| wire.route.take().expect("flat item route")),
        );
        return Ok(legs);
    }
    let mut walls = Walls::new(nodes, wires, title, work)?;
    let placed = labels(fit, wires, &mut walls, options, work)?;
    for (index, (wire, seats)) in wires.iter_mut().zip(placed).enumerate() {
        work.visit(1)?;
        let mut edge = if wire.border().is_some() {
            boundary_leg(index, wire, nodes, &walls, fit, options, work)?
        } else {
            wire.route.take().expect("flat item route")
        };
        edge.label_center = seats[0].or(edge.label_center);
        edge.start_label_center = seats[1].or(edge.start_label_center);
        edge.end_label_center = seats[2].or(edge.end_label_center);
        legs.push(edge);
    }
    Ok(legs)
}
