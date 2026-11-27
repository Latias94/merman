use super::*;
use crate::work::push;
use crate::{MarkerMetrics, Options};
use fit::blocks;

pub(super) fn opposite(side: Side) -> Side {
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

pub(super) fn snap(edge: &mut EdgeGeometry, end: usize, contact: Contact) {
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
    edge.points[index] = RoutePoint::new(contact.at.x, contact.at.y, contact.side.axis());
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
            End::Lane { .. } => unreachable!("lane contacts are resolved in the root frame"),
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

pub(super) fn marker_rect(port: Contact, marker: MarkerMetrics) -> Rect {
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

pub(super) fn item_boxes(
    nodes: &[Rect],
    wires: &[Wire],
    work: &mut Work<'_>,
) -> LayoutResult<Vec<Rect>> {
    work.charge(nodes.len())?;
    let mut boxes = reserved(nodes.len())?;
    boxes.extend(
        nodes
            .iter()
            .copied()
            .filter(|rect| rect.width > 0.0 && rect.height > 0.0),
    );
    for wire in wires {
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
    }
    Ok(boxes)
}

impl Walls {
    fn new(
        nodes: &[Rect],
        wires: &[Wire],
        title: Option<Rect>,
        work: &mut Work<'_>,
    ) -> LayoutResult<Self> {
        let boxes = item_boxes(nodes, wires, work)?;
        let mut markers = Vec::new();
        for (index, wire) in wires.iter().enumerate() {
            work.visit(1)?;
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

    fn into_route_walls(self) -> RouteWalls {
        RouteWalls {
            fixed_count: self.boxes.len(),
            walls: self,
        }
    }
}

struct RouteWalls {
    walls: Walls,
    fixed_count: usize,
}

impl RouteWalls {
    fn for_leg(
        &mut self,
        index: usize,
        pair: [Contact; 2],
        title: Option<Rect>,
        title_margin: f64,
        work: &mut Work<'_>,
    ) -> LayoutResult<(&[Rect], Option<Rect>)> {
        let Walls { boxes, markers, .. } = &mut self.walls;
        boxes.truncate(self.fixed_count);
        let extra = markers
            .len()
            .checked_add(1)
            .ok_or(WorkError::ArithmeticOverflow)?;
        work.charge(
            self.fixed_count
                .checked_add(extra)
                .ok_or(WorkError::ArithmeticOverflow)?,
        )?;
        boxes
            .try_reserve_exact(extra)
            .map_err(|_| LayoutError::AllocationLimit)?;
        for &(owner, marker, rect) in markers.iter() {
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
                boxes.push(rect);
            }
        }
        let mut padded = None;
        if let Some(title) = title.filter(|title| title.width > 0.0 && title.height > 0.0) {
            let on_title = |port: &Contact| {
                let point = port.at;
                point.x >= title.x
                    && point.x <= title.x + title.width
                    && point.y >= title.y
                    && point.y <= title.y + title.height
            };
            if !pair.iter().any(on_title) {
                boxes.push(title);
                padded = Some(Rect {
                    x: finite(title.x - title_margin)?,
                    y: finite(title.y - title_margin)?,
                    width: finite(title.width + 2.0 * title_margin)?,
                    height: finite(title.height + 2.0 * title_margin)?,
                });
            }
        }
        Ok((boxes, padded))
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
    protected_title: Option<Rect>,
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
        let base = self.connect(inner, outer, markers, item, work)?;
        let Some(title) = self.protected_title else {
            return Ok(base);
        };
        if clear(&base, &[title], work)?
            || blocks(title, inner.at, inner.at)
            || blocks(title, outer.at, outer.at)
        {
            return Ok(base);
        }

        let count = self
            .walls
            .len()
            .checked_add(1)
            .ok_or(WorkError::ArithmeticOverflow)?;
        work.charge(count)?;
        let mut walls = reserved(count)?;
        walls.extend_from_slice(self.walls);
        walls.push(title);
        let candidate = Connection {
            walls: &walls,
            ..*self
        }
        .connect(inner, outer, markers, item, work);
        // Clamping an exterior detour to the group may reintroduce a collision.
        match candidate {
            Ok(path) if clear(&path, &walls, work)? => Ok(path),
            Ok(_) | Err(LayoutError::InvalidInput(_)) => Ok(base),
            Err(error) => Err(error),
        }
    }

    fn connect(
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
        if let Some(title) = self.protected_title
            && inner.side == opposite(outer.side)
        {
            let vertical = inner.side.axis() == Axis::Vertical;
            let along = |p: Point| if vertical { p.y } else { p.x };
            let sign = match inner.side {
                Side::Top | Side::Left => -1.0,
                Side::Bottom | Side::Right => 1.0,
            };
            let (near, far) = (along(start), along(end));
            if (far - near) * sign >= 0.0 {
                // The title apron already has a hard clearance. Use a monotone crossbar
                // there before asking the general corridor to take an exterior detour.
                for level in [near, near.midpoint(far), far] {
                    let on_plane = |point: Point| {
                        if vertical {
                            Point { y: level, ..point }
                        } else {
                            Point { x: level, ..point }
                        }
                    };
                    let via = [inner.at, on_plane(start), on_plane(end), outer.at];
                    if clear(&via, &[title], work)? && clear(&via, self.walls, work)? {
                        for p in via {
                            knot(&mut points, p)?;
                        }
                        return Ok(points);
                    }
                }
            }
        }
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
        points.push(RoutePoint::new(at.x, at.y, axis));
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
    clearance: f64,
    work: &mut Work<'_>,
) -> LayoutResult<Option<TitleSweep>> {
    let vertical = outer.side.axis() == Axis::Vertical;
    let low = Point {
        x: title.x - clearance,
        y: title.y - clearance,
    };
    let high = Point {
        x: title.x + title.width + clearance,
        y: title.y + title.height + clearance,
    };
    let across = |at: Point| cross(outer.side, at);
    let along = |at: Point| if vertical { at.y } else { at.x };
    let with_along = |at: Point, value| {
        if vertical {
            Point { y: value, ..at }
        } else {
            Point { x: value, ..at }
        }
    };
    let lane = across(outer.at);
    if across(low) < lane && lane < across(high) {
        return Ok(None);
    }
    let len = edge.points.len();
    let index = |step: usize| if border == 0 { step } else { len - 1 - step };
    let dummy = point(edge.points[index(0)]);
    let inner_lane = across(dummy);
    if inner_lane == lane
        || inner_lane.max(lane) <= across(low)
        || inner_lane.min(lane) >= across(high)
    {
        return Ok(None);
    }
    let (inward, far) = match outer.side {
        Side::Bottom | Side::Right => (1.0, high),
        Side::Top | Side::Left => (-1.0, low),
    };
    let exit = with_along(outer.at, along(far));
    if (along(exit) - along(outer.at)) * inward <= 0.0 {
        return Ok(None);
    }
    let mut run = 0;
    let mut last = dummy;
    while run + 1 < len {
        work.visit(1)?;
        let next = point(edge.points[index(run + 1)]);
        if across(next) != inner_lane || (along(next) - along(last)) * inward < 0.0 {
            break;
        }
        last = next;
        run += 1;
    }
    let join = if run + 1 == len {
        with_along(last, along(last) - inward * shaft)
    } else {
        last
    };
    Ok(Some(TitleSweep {
        points: [outer.at, exit, join],
        remaining: run + usize::from(join == last),
        depth: (along(join) - along(exit)) * inward,
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
        if physical != side || wire.uses_caption_band() {
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
            fit.title_margin(),
            work,
        )? {
            let across = (cross(side, sweep.points[1]) - cross(side, sweep.points[2])).abs();
            let room = (across * 0.75).max(options.spacing.edge.max(16.0));
            extra = extra.max(room - sweep.depth);
        }
    }
    finite(extra)
}

fn through_title(
    edge: &mut EdgeGeometry,
    border: usize,
    sweep: TitleSweep,
    axis: Axis,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let len = edge.points.len();
    let remaining = sweep.remaining;
    let capacity = (len - remaining)
        .checked_add(3)
        .ok_or(WorkError::ArithmeticOverflow)?;
    work.charge(capacity)?;
    let mut points = reserved(capacity)?;
    let sweep = sweep.points.map(|at| RoutePoint::new(at.x, at.y, axis));
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
    walls: &mut RouteWalls,
    fit: &fit::Fit,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<EdgeGeometry> {
    let (border, _, side) = wire.border().expect("leg has a border");
    let member = 1 - border;
    let outer = travel(wire, border);
    let inner = travel(wire, member);
    let measured_title = walls.walls.title;
    let title = measured_title.filter(|title| {
        side != Side::Top
            || !fit.adaptive_top_title
            || (outer.at.x - title.center().x).abs()
                >= title.width / 2.0 + wire.title_clearance(border, fit.title_margin())
    });
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
    if wire.flat() && !wire.uses_caption_band() {
        let mut edge = wire.route.take().expect("flat border route");
        let index_in_route = if border == 0 {
            0
        } else {
            edge.points.len() - 1
        };
        let at = point(edge.points[index_in_route]);
        if at != outer.at {
            let dummy = Contact { at, side };
            let (blockers, protected_title) =
                walls.for_leg(index, [dummy, outer], title, fit.title_margin(), work)?;
            if options.routing == crate::RoutingStyle::Curved
                && let Some(title) = title
                && let Some(sweep) = title_sweep(
                    &edge,
                    border,
                    outer,
                    title,
                    markers[member] + options.spacing.edge,
                    fit.title_margin(),
                    work,
                )?
                && sweep.depth > 0.0
                && clear(&sweep.points, blockers, work)?
                && clear(&sweep.points, protected_title.as_slice(), work)?
            {
                through_title(&mut edge, border, sweep, side.axis(), work)?;
                edge.ports = ports;
                return Ok(edge);
            }
            let mut connection = Connection {
                fit,
                walls: blockers,
                gap: options.spacing.edge,
                protected_title,
            };
            // Both contacts lie on parallel planes. A short seam needs just enough
            // runway to turn between the planes without doubling back through either.
            let depth = match side.axis() {
                Axis::Horizontal => (at.x - outer.at.x).abs(),
                Axis::Vertical => (at.y - outer.at.y).abs(),
            };
            connection.gap = connection.gap.min((depth - markers[border]).max(0.0) * 0.5);
            let path = connection.path(dummy, outer, [0.0, markers[border]], None, work)?;
            let mut extension = knots(path, border == 0, side.axis(), work)?;
            if border == 0 {
                if let Some(last) = extension.pop() {
                    edge.points[0].axis = last.axis;
                }
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
    let (blockers, protected_title) =
        walls.for_leg(index, [inner, outer], title, fit.title_margin(), work)?;
    let connection = Connection {
        fit,
        walls: blockers,
        gap: options.spacing.edge,
        protected_title,
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
        shared_bus: false,
        corridor_clearance: 0.0,
        reversed_for_ranking: wire
            .route
            .as_ref()
            .is_some_and(|edge| edge.reversed_for_ranking),
    })
}

pub(super) fn title_struck(
    title: Rect,
    legs: &[EdgeGeometry],
    routing: crate::RoutingStyle,
    work: &mut Work<'_>,
) -> LayoutResult<bool> {
    if title.width == 0.0 || title.height == 0.0 {
        return Ok(false);
    }
    for leg in legs {
        work.visit(1)?;
        for (index, pair) in leg.points.windows(2).enumerate() {
            work.visit(1)?;
            let [from, to] = [point(pair[0]), point(pair[1])];
            // A rounded corner can enter the rectangle between two clear straight segments.
            if blocks(title, from, to)
                || (routing != crate::RoutingStyle::Polyline
                    && leg.points.get(index + 2).is_some_and(|&next| {
                        let next = point(next);
                        ((from.x == to.x && to.y == next.y) || (from.y == to.y && to.x == next.x))
                            && blocks(title, from, next)
                    }))
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
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
    let base_box_count = walls.boxes.len();
    let placed = labels(fit, wires, &mut walls, options, work)?;
    let mut label_boxes = reserved(wires.len())?;
    let mut seat_box_indices = filled(wires.len(), None::<usize>)?;
    {
        let mut cursor = base_box_count;
        for (index, seats) in placed.iter().enumerate() {
            for (slot, seat) in seats.iter().enumerate() {
                if seat.is_some() {
                    if slot == 0 {
                        seat_box_indices[index] = Some(label_boxes.len());
                    }
                    push(&mut label_boxes, walls.boxes[cursor])?;
                    cursor += 1;
                }
            }
        }
    }
    let mut walls = walls.into_route_walls();
    for (index, (wire, seats)) in wires.iter_mut().zip(placed).enumerate() {
        work.visit(1)?;
        let mut edge = if wire.border().is_some() {
            boundary_leg(index, wire, nodes, &mut walls, fit, options, work)?
        } else {
            wire.route.take().expect("flat item route")
        };
        edge.label_center = seats[0].or(edge.label_center);
        edge.start_label_center = seats[1].or(edge.start_label_center);
        edge.end_label_center = seats[2].or(edge.end_label_center);
        legs.push(edge);
    }
    rehome_detached_border_captions(
        fit,
        &walls.walls.boxes[..base_box_count],
        walls.walls.title,
        &walls.walls.markers,
        &mut label_boxes,
        &seat_box_indices,
        wires,
        &mut legs,
        options,
        work,
    )?;
    Ok(legs)
}

fn rect_to_segment_distance(rect: Rect, a: Point, b: Point) -> f64 {
    if blocks(rect, a, b) {
        return 0.0;
    }
    let rx0 = rect.x;
    let rx1 = rect.x + rect.width;
    let ry0 = rect.y;
    let ry1 = rect.y + rect.height;
    let point_to_rect = |p: Point| {
        let dx = if p.x < rx0 {
            rx0 - p.x
        } else if p.x > rx1 {
            p.x - rx1
        } else {
            0.0
        };
        let dy = if p.y < ry0 {
            ry0 - p.y
        } else if p.y > ry1 {
            p.y - ry1
        } else {
            0.0
        };
        dx.hypot(dy)
    };
    let mut best = point_to_rect(a).min(point_to_rect(b));
    for corner in [
        Point { x: rx0, y: ry0 },
        Point { x: rx1, y: ry0 },
        Point { x: rx0, y: ry1 },
        Point { x: rx1, y: ry1 },
    ] {
        best = best.min(point_to_segment_distance(corner, a, b));
    }
    // Also check orthogonal projection of axis-aligned segment onto the rectangle's interval.
    if (a.x - b.x).abs() <= 1e-9 {
        let y_lo = a.y.min(b.y);
        let y_hi = a.y.max(b.y);
        if y_hi >= ry0 && y_lo <= ry1 {
            let dx = if a.x < rx0 {
                rx0 - a.x
            } else if a.x > rx1 {
                a.x - rx1
            } else {
                0.0
            };
            best = best.min(dx);
        }
    } else if (a.y - b.y).abs() <= 1e-9 {
        let x_lo = a.x.min(b.x);
        let x_hi = a.x.max(b.x);
        if x_hi >= rx0 && x_lo <= rx1 {
            let dy = if a.y < ry0 {
                ry0 - a.y
            } else if a.y > ry1 {
                a.y - ry1
            } else {
                0.0
            };
            best = best.min(dy);
        }
    }
    best
}

fn point_to_segment_distance(p: Point, a: Point, b: Point) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len_sq = dx * dx + dy * dy;
    let t = if len_sq > 0.0 {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / len_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.x - (a.x + t * dx)).hypot(p.y - (a.y + t * dy))
}

#[allow(clippy::too_many_arguments)]
fn rehome_detached_border_captions(
    fit: &fit::Fit,
    base_boxes: &[Rect],
    title: Option<Rect>,
    markers: &[(usize, Contact, Rect)],
    label_boxes: &mut [Rect],
    seat_box_indices: &[Option<usize>],
    wires: &[Wire],
    legs: &mut [EdgeGeometry],
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let gap = options.spacing.edge.max(4.0);
    for index in 0..wires.len() {
        work.visit(1)?;
        if !wires[index].uses_caption_band() {
            continue;
        }
        let (Some(size), Some(current_center), Some(box_slot)) = (
            wires[index].spec.label,
            legs[index].label_center,
            seat_box_indices[index],
        ) else {
            continue;
        };
        if legs[index].points.len() < 2 {
            continue;
        }
        let current_rect = centered(current_center, size);
        let mut min_rect_dist = f64::INFINITY;
        for seg in legs[index].points.windows(2) {
            work.visit(1)?;
            min_rect_dist = min_rect_dist.min(rect_to_segment_distance(
                current_rect,
                point(seg[0]),
                point(seg[1]),
            ));
        }
        if min_rect_dist <= gap + 6.0 {
            continue;
        }
        // Collect route segments sorted longest-first (with index tie-breaker).
        let mut segments = reserved(legs[index].points.len() - 1)?;
        for (segment, seg) in legs[index].points.windows(2).enumerate() {
            work.visit(1)?;
            let a = point(seg[0]);
            let b = point(seg[1]);
            let len = (b.x - a.x).hypot(b.y - a.y);
            if len > 1e-6 && len.is_finite() {
                segments.push((len, segment, a, b));
            }
        }
        let mut order = reserved(segments.len())?;
        order.extend(0..segments.len());
        work.sort(&mut order, |&a, &b| {
            segments[b]
                .0
                .total_cmp(&segments[a].0)
                .then_with(|| a.cmp(&b))
        })?;
        let mut chosen = None::<Point>;
        // Phase A: centered directly on a clear long leg of this edge.
        // Phase B: offset immediately beside a clear leg of this edge.
        'search: for beside in [false, true] {
            for &segment_index in &order {
                work.visit(1)?;
                let (len, host, a, b) = segments[segment_index];
                let dx = b.x - a.x;
                let dy = b.y - a.y;
                let horizontal = dy.abs() <= dx.abs();
                let inset = if beside {
                    0.0
                } else {
                    // Keep the projected caption and its clearance between both turns.
                    let half =
                        dx.abs() / len * (size.width / 2.0) + dy.abs() / len * (size.height / 2.0);
                    (half + gap) / len
                };
                if inset > 0.5 {
                    continue;
                }
                let across_offset = if horizontal { size.height } else { size.width } * 0.5 + gap;
                let offsets: &[f64] = if beside {
                    &[across_offset, -across_offset]
                } else {
                    &[0.0]
                };
                for fraction in [0.5_f64, 0.35, 0.65] {
                    let t = fraction.clamp(inset, 1.0 - inset);
                    let base = Point {
                        x: a.x + dx * t,
                        y: a.y + dy * t,
                    };
                    for &off in offsets {
                        work.visit(1)?;
                        // The padded box abuts the host. Round outward to keep that boundary
                        // clear when forming the box loses an ULP to cancellation.
                        let shift = |along: f64| {
                            let shifted = along + off;
                            if off > 0.0 {
                                shifted.next_up()
                            } else if off < 0.0 {
                                shifted.next_down()
                            } else {
                                shifted
                            }
                        };
                        let center = if horizontal {
                            Point {
                                y: shift(base.y),
                                ..base
                            }
                        } else {
                            Point {
                                x: shift(base.x),
                                ..base
                            }
                        };
                        let footprint = centered(
                            center,
                            Size {
                                width: size.width + 2.0 * gap,
                                height: size.height + 2.0 * gap,
                            },
                        );
                        if footprint.x < 0.0
                            || footprint.y < 0.0
                            || footprint.x + footprint.width > fit.size.width
                            || footprint.y + footprint.height > fit.size.height
                        {
                            continue;
                        }
                        let mut clear_ok = true;
                        for &wall in base_boxes
                            .iter()
                            .chain(title.iter())
                            .chain(markers.iter().map(|entry| &entry.2))
                        {
                            work.visit(1)?;
                            if overlaps(wall, footprint) {
                                clear_ok = false;
                                break;
                            }
                        }
                        if !clear_ok {
                            continue;
                        }
                        for (other_slot, &other_label) in label_boxes.iter().enumerate() {
                            if other_slot == box_slot {
                                continue;
                            }
                            work.visit(1)?;
                            if overlaps(other_label, footprint) {
                                clear_ok = false;
                                break;
                            }
                        }
                        if !clear_ok {
                            continue;
                        }
                        for (other_idx, other_leg) in legs.iter().enumerate() {
                            for (segment, seg) in other_leg.points.windows(2).enumerate() {
                                work.visit(1)?;
                                if !beside && other_idx == index && segment == host {
                                    continue;
                                }
                                if blocks(footprint, point(seg[0]), point(seg[1])) {
                                    clear_ok = false;
                                    break;
                                }
                            }
                            if !clear_ok {
                                break;
                            }
                        }
                        if clear_ok {
                            chosen = Some(center);
                            break 'search;
                        }
                    }
                }
            }
        }
        if let Some(center) = chosen {
            legs[index].label_center = Some(center);
            label_boxes[box_slot] = centered(center, size);
        }
    }
    Ok(())
}

#[cfg(test)]
mod route_unit_tests {
    use super::*;
    use crate::{
        Direction, Endpoint, GroupId, GroupSpec, Insets, NodeId, NodeSpec, NoopWorkControl,
        RoutingStyle, layout,
    };

    #[test]
    fn title_apron_crosses_once_below_the_title_instead_of_wrapping_around_it() {
        let options = Options::default();
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        let fit = fit::Fit::new(
            None,
            Size {
                width: 280.,
                height: 160.,
            },
            &[],
            &[],
            0.,
            &options,
            &mut work,
        )
        .unwrap();
        let title = Rect {
            x: 2.,
            y: 8.,
            width: 192.,
            height: 36.,
        };
        let inner = Contact {
            at: Point { x: 100., y: 72. },
            side: Side::Top,
        };
        let outer = Contact {
            at: Point { x: 195., y: 0. },
            side: Side::Bottom,
        };
        let path = Connection {
            fit: &fit,
            walls: &[title],
            gap: TITLE_CLEAR,
            protected_title: Some(title),
        }
        .path(inner, outer, [16., 0.], None, &mut work)
        .unwrap();
        assert_eq!(path.len(), 4, "{path:?}");
        assert!(
            path.windows(2)
                .all(|pair| pair[0].x <= pair[1].x && pair[0].y >= pair[1].y)
        );
        assert!(clear(&path, &[title], &mut work).unwrap());
    }

    fn rehome_label(points: &[(f64, f64)], walls: &[Rect]) -> Rect {
        let size = Size {
            width: 60.0,
            height: 14.0,
        };
        let mut spec = EdgeSpec::new(GroupId(0).into(), NodeId(0).into());
        spec.label = Some(size);
        let wires = [Wire {
            ends: [
                End::Border {
                    slot: 0,
                    side: Side::Left,
                    flat: None,
                    hint: PortSideHint::Auto,
                },
                End::Item {
                    slot: 0,
                    hint: PortSideHint::Auto,
                    child: None,
                },
            ],
            spec,
            route: None,
            contacts: [None; 2],
        }];
        let seat = Point { x: 175.0, y: 170.0 };
        let mut legs = [EdgeGeometry {
            points: points
                .iter()
                .map(|&(x, y)| RoutePoint::new(x, y, Axis::Horizontal))
                .collect(),
            ports: (Side::Left, Side::Left),
            label_center: Some(seat),
            start_label_center: None,
            end_label_center: None,
            shared_bus: false,
            corridor_clearance: 0.0,
            reversed_for_ranking: false,
        }];
        let mut boxes = [centered(seat, size)];
        let mut options = Options::default();
        options.spacing.edge = 7.0;
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        let fit = fit::Fit::new(
            None,
            Size {
                width: 220.0,
                height: 200.0,
            },
            &[],
            &[],
            0.0,
            &options,
            &mut work,
        )
        .unwrap();
        rehome_detached_border_captions(
            &fit,
            walls,
            None,
            &[],
            &mut boxes,
            &[Some(0)],
            &wires,
            &mut legs,
            &options,
            &mut work,
        )
        .unwrap();
        assert_eq!(legs[0].label_center, Some(boxes[0].center()));
        assert_ne!(boxes[0].center(), seat);
        boxes[0]
    }

    #[test]
    fn rehomed_inline_caption_keeps_both_ends_inside_its_host_segment() {
        let points = [(40.0, 34.0), (40.0, 60.0), (116.0, 60.0)];
        assert_eq!(
            rehome_label(&points, &[]).center(),
            Point { x: 78.0, y: 60.0 }
        );
        let wall = Rect {
            x: 107.0,
            y: 65.0,
            width: 2.0,
            height: 5.0,
        };
        let seat = rehome_label(&points, &[wall]);
        assert!(seat.center().y != 60.0 || (seat.x >= 47.0 && seat.x + seat.width <= 109.0));
    }

    #[test]
    fn rehomed_inline_caption_clears_the_return_leg_of_its_own_edge() {
        let points = [(30.0, 80.0), (150.0, 80.0), (150.0, 84.0), (35.0, 84.0)];
        let seat = rehome_label(&points, &[]);
        assert!(
            seat.center().y != 80.0 && seat.center().y != 84.0,
            "{seat:?}"
        );
    }

    #[test]
    fn rehomed_inline_caption_reserves_a_gap_to_nearby_obstacles() {
        let wall = Rect {
            x: 15.0,
            y: 88.0,
            width: 170.0,
            height: 55.0,
        };
        let seat = rehome_label(&[(30.0, 80.0), (150.0, 80.0)], &[wall]);
        assert!(seat.y + seat.height + 7.0 <= wall.y, "{seat:?}");
    }

    #[test]
    fn border_entry_preserves_incoming_axis_and_rehomes_detached_parent_label() {
        let mut graph = Graph {
            direction: Direction::Down,
            ..Default::default()
        };
        let outer = GroupSpec {
            padding: Insets {
                top: 20.0,
                right: 20.0,
                bottom: 20.0,
                left: 20.0,
            },
            title: Some(Size {
                width: 120.0,
                height: 20.0,
            }),
            ..Default::default()
        };
        let inner = GroupSpec {
            parent: Some(GroupId(0)),
            padding: Insets {
                top: 16.0,
                right: 16.0,
                bottom: 16.0,
                left: 16.0,
            },
            ..Default::default()
        };
        graph.groups = vec![outer, inner];
        let mut n0 = NodeSpec::new(
            Size {
                width: 100.0,
                height: 40.0,
            },
            0,
        );
        n0.parent = Some(GroupId(1));
        let mut n1 = NodeSpec::new(
            Size {
                width: 100.0,
                height: 40.0,
            },
            1,
        );
        n1.parent = Some(GroupId(0));
        let n2 = NodeSpec::new(
            Size {
                width: 80.0,
                height: 40.0,
            },
            2,
        );
        graph.nodes = vec![n0, n1, n2];
        // Child-to-parent-subgraph labeled edge (like `a2 -->|Parent Escalation| outer_sg`).
        let mut to_parent = EdgeSpec::new(Endpoint::Node(NodeId(0)), Endpoint::Group(GroupId(0)));
        to_parent.label = Some(Size {
            width: 120.0,
            height: 22.0,
        });
        // Cross-boundary entry into `outer` to exercise `border == 0` extension axis.
        let entry = EdgeSpec::new(Endpoint::Node(NodeId(2)), Endpoint::Node(NodeId(1)));
        graph.edges = vec![to_parent, entry];

        let options = Options {
            routing: RoutingStyle::Orthogonal,
            ..Default::default()
        };
        let out = layout(&graph, &options).expect("orthogonal compound layout");
        let label = out.edges[0]
            .label_center
            .expect("child-to-parent edge must place its label");
        let label_rect = centered(
            label,
            Size {
                width: 120.0,
                height: 22.0,
            },
        );
        let min_rect_dist = out.edges[0]
            .points
            .windows(2)
            .map(|seg| rect_to_segment_distance(label_rect, point(seg[0]), point(seg[1])))
            .fold(f64::INFINITY, f64::min);
        assert!(
            min_rect_dist <= 20.0,
            "border caption box should stay adjacent to its own edge instead of floating detached (rect_dist={min_rect_dist:.1})"
        );
        // Every interior knot of the orthogonal entry edge must carry the incoming segment's travel axis.
        for pair in out.edges[1].points.windows(2) {
            let dx = (pair[1].x - pair[0].x).abs();
            let dy = (pair[1].y - pair[0].y).abs();
            if dx > 1e-6 && dy <= 1e-6 {
                assert_eq!(pair[1].axis, Axis::Horizontal);
            } else if dy > 1e-6 && dx <= 1e-6 {
                assert_eq!(pair[1].axis, Axis::Vertical);
            }
        }
    }
}
