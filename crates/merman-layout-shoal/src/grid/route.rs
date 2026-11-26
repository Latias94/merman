//! Route each scope against its immediate walls, then stitch contacts at group faces.

mod captions;
mod gutters;

use gutters::gutter_route;

use super::*;
use crate::geom::Bounds;
use crate::ortho::{OrthoRequest, OrthoRouter, OrthoSegment};
use crate::{BorderStitch, EdgeGeometry, EdgeId, MarkerMetrics, Rect, RoutePoint, TruncatedRoute};

#[derive(Clone, Copy)]
struct Site {
    owner: usize,
    side: Side,
    preferred: f64,
    offset: Option<f64>,
    inner: bool,
    marker: MarkerMetrics,
    reach: f64,
    at: Point,
    lane: FaceLane,
}

#[derive(Clone, Copy, Default)]
struct FaceLane {
    ordinal: usize,
    degree: usize,
}

#[derive(Clone, Copy)]
struct End {
    site: usize,
    inner: bool,
}

#[derive(Clone, Copy)]
struct Part {
    from: End,
    to: End,
    labels: [Option<Size>; 3],
}

#[derive(Default)]
struct Routed {
    points: Vec<RoutePoint>,
    labels: [Option<Point>; 3],
}

#[derive(Clone, Copy)]
struct Slot {
    scope: usize,
    index: usize,
}

struct Plan {
    sites: Vec<Site>,
    scopes: Vec<Vec<Part>>,
    paths: Vec<Vec<Slot>>,
    truncated: Vec<TruncatedRoute>,
    border_ports_left: usize,
}

fn opposite(side: Side) -> Side {
    match side {
        Side::Top => Side::Bottom,
        Side::Right => Side::Left,
        Side::Bottom => Side::Top,
        Side::Left => Side::Right,
    }
}

fn side_toward(from: Rect, to: Point, preferred: Side) -> Side {
    let [dx, dy] = [to.x - from.center().x, to.y - from.center().y];
    let clearance = [from.width * 0.5, from.height * 0.5];
    let (axis, sign) = normal(preferred);
    if [dx, dy][axis] * sign as f64 >= clearance[axis] {
        return preferred;
    }
    if dx.abs() - clearance[0] >= dy.abs() - clearance[1] {
        if dx >= 0.0 { Side::Right } else { Side::Left }
    } else if dy >= 0.0 {
        Side::Bottom
    } else {
        Side::Top
    }
}

fn cross(side: Side, p: Point) -> f64 {
    if side.axis() == Axis::Vertical {
        p.x
    } else {
        p.y
    }
}

impl Plan {
    fn new(context: &Routing<'_>, work: &mut Work<'_>) -> LayoutResult<Self> {
        let scopes = context.hierarchy.scopes.len();
        let edges = context.graph.edges.len();
        work.charge(scopes + edges)?;
        Ok(Self {
            sites: Vec::new(),
            scopes: filled(scopes, Vec::new())?,
            paths: reserved(edges)?,
            truncated: Vec::new(),
            border_ports_left: context.options.caps.max_border_ports,
        })
    }

    fn site(&mut self, site: Site, inner: bool) -> LayoutResult<End> {
        let id = self.sites.len();
        push(&mut self.sites, site)?;
        Ok(End { site: id, inner })
    }

    fn leg(&mut self, scope: usize, part: Part) -> LayoutResult<Slot> {
        let index = self.scopes[scope].len();
        push(&mut self.scopes[scope], part)?;
        Ok(Slot { scope, index })
    }

    fn edge(
        &mut self,
        context: &Routing<'_>,
        projection: Projection,
        sides: [Side; 2],
        edge: usize,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        let spec = &context.graph.edges[edge];
        let h = context.hierarchy;
        let scope = projection.scope;
        let scope_item = h.node_count + scope;
        let transit = |item: usize| {
            if item == scope_item {
                0
            } else {
                h.scopes[h.parent[item]].depth - h.scopes[scope].depth
            }
        };
        let needed = transit(projection.ends[0])
            .checked_add(transit(projection.ends[1]))
            .ok_or(WorkError::ArithmeticOverflow)?;
        let mut ends = projection.ends;
        let kept = if needed <= self.border_ports_left {
            self.border_ports_left -= needed;
            needed
        } else {
            for (end, item) in ends.iter_mut().zip(projection.items) {
                *end = item.unwrap_or(*end);
            }
            push(
                &mut self.truncated,
                TruncatedRoute {
                    edge: EdgeId(edge as u32),
                    source: h.endpoint(ends[0]),
                    target: h.endpoint(ends[1]),
                },
            )?;
            0
        };
        let mut steps = reserved(kept.checked_add(1).ok_or(WorkError::ArithmeticOverflow)?)?;
        let hints = [spec.source_port, spec.target_port];
        let markers = [spec.start_marker, spec.end_marker];
        let mut terminals = [End {
            site: 0,
            inner: false,
        }; 2];
        let mut incoming = 0;
        for (end, (&item, &side)) in ends.iter().zip(&sides).enumerate() {
            work.visit(1)?;
            let other = rect(context.drawing, ends[1 - end]).center();
            let from = rect(context.drawing, item).center();
            let inner = item == scope_item;
            let offset = match hints[end] {
                PortSideHint::FixedOffset { offset, .. } if item == projection.ends[end] => {
                    Some(offset)
                }
                _ => None,
            };
            let mut terminal = self.site(
                Site {
                    owner: item,
                    side,
                    preferred: cross(side, other),
                    offset,
                    inner,
                    marker: markers[end],
                    reach: 0.0,
                    at: Point::default(),
                    lane: FaceLane::default(),
                },
                inner,
            )?;
            let mut owner = if inner { scope } else { h.parent[item] };
            if end == 1 {
                incoming = steps.len();
            }
            while owner != scope {
                work.visit(1)?;
                let group = h.node_count + owner;
                let side = side_toward(rect(context.drawing, group), other, side);
                let outer = self.site(
                    Site {
                        owner: group,
                        side,
                        preferred: cross(side, from),
                        offset: None,
                        inner: true,
                        marker: MarkerMetrics::default(),
                        reach: 0.0,
                        at: Point::default(),
                        lane: FaceLane::default(),
                    },
                    true,
                )?;
                let (from, to) = if end == 0 {
                    (terminal, outer)
                } else {
                    (outer, terminal)
                };
                steps.push(self.leg(
                    owner,
                    Part {
                        from,
                        to,
                        labels: [None; 3],
                    },
                )?);
                terminal = End {
                    inner: false,
                    ..outer
                };
                owner = h.parent[group];
            }
            terminals[end] = terminal;
        }
        let [from, to] = terminals;
        steps.push(self.leg(
            scope,
            Part {
                from,
                to,
                labels: [spec.label, spec.start_label, spec.end_label],
            },
        )?);
        steps[incoming..].reverse();
        self.paths.push(steps);
        Ok(())
    }
}

struct Routing<'a> {
    graph: &'a Graph,
    hierarchy: &'a Hierarchy,
    drawing: &'a LayoutOutput,
    metrics: Metrics,
    options: &'a Options,
}

impl Routing<'_> {
    fn bounds(&self, scope: usize) -> Rect {
        self.drawing.groups.get(scope).map_or(
            Rect {
                width: self.drawing.bounds.width,
                height: self.drawing.bounds.height,
                ..Rect::default()
            },
            |group| group.rect,
        )
    }

    fn title(&self, group: usize) -> Option<Rect> {
        let at = self.drawing.groups.get(group)?.title_center?;
        let size = self.graph.groups[group].title?;
        Some(Rect {
            x: at.x - size.width / 2.0,
            y: at.y - size.height / 2.0,
            width: size.width,
            height: size.height,
        })
    }

    fn seat(&self, owner: usize, side: Side, inner: bool) -> [f64; 3] {
        let body = rect(self.drawing, owner);
        let transverse = 1 - normal(side).0;
        let [lo, hi] = [Bounds::of(body).x, Bounds::of(body).y][transverse];
        if let Some(node) = self.graph.nodes.get(owner) {
            let center = if transverse == 1 {
                body.y + node.anchor_y.unwrap_or(body.height / 2.0)
            } else {
                body.center().x
            };
            let span = if side.axis() == self.graph.direction.axis() {
                node.port_span
            } else {
                node.perpendicular_port_span.or(node.port_span)
            };
            let half = span.unwrap_or(hi - lo) / 2.0;
            return [(center - half).max(lo), center, (center + half).min(hi)];
        }
        let border = self
            .options
            .spacing
            .corner_clearance
            .max(self.metrics.separation);
        let inset = border.min((hi - lo) / 2.0);
        let mut range = [lo + inset, hi - inset];
        if inner && let Some(title) = self.title(owner - self.hierarchy.node_count) {
            let group = &self.graph.groups[owner - self.hierarchy.node_count];
            let title_is_top = place::is_top(group.title_position);
            let clear = self.metrics.separation;
            match side {
                Side::Top | Side::Bottom if title_is_top == (side == Side::Top) => {
                    let left = title.x - clear;
                    let right = title.x + title.width + clear;
                    if left - range[0] > range[1] - right {
                        range[1] = range[1].min(left);
                    } else {
                        range[0] = range[0].max(right);
                    }
                }
                Side::Left | Side::Right => {
                    if title_is_top {
                        range[0] = range[0].max(title.y + title.height + clear);
                    } else {
                        range[1] = range[1].min(title.y - clear);
                    }
                }
                _ => {}
            }
        }
        if range[0] > range[1] {
            range = [lo.midpoint(hi); 2];
        }
        [range[0], lo.midpoint(hi), range[1]]
    }

    fn ports(&self, sites: &mut [Site], work: &mut Work<'_>) -> LayoutResult<()> {
        let mut indices = reserved(sites.len())?;
        work.charge(sites.len())?;
        indices.extend(0..sites.len());
        charge_sort(indices.len(), work)?;
        indices.sort_unstable_by(|&a, &b| {
            let u = &sites[a];
            let v = &sites[b];
            (u.owner, u.side as u8)
                .cmp(&(v.owner, v.side as u8))
                .then_with(|| u.preferred.total_cmp(&v.preferred))
                .then(a.cmp(&b))
        });
        let mut cursor = 0;
        while cursor < indices.len() {
            let first = sites[indices[cursor]];
            let mut next = cursor + 1;
            let mut inner = first.inner;
            let mut reach = first.marker.length;
            let mut count = usize::from(first.offset.is_none());
            while let Some(&i) = indices.get(next) {
                let site = sites[i];
                work.visit(1)?;
                if site.owner != first.owner || site.side != first.side {
                    break;
                }
                inner |= site.inner;
                reach = reach.max(site.marker.length);
                count += usize::from(site.offset.is_none());
                next += 1;
            }
            let body = rect(self.drawing, first.owner);
            let [lo, center, hi] = self.seat(first.owner, first.side, inner);
            let (axis, sign) = normal(first.side);
            let bounds = Bounds::of(body);
            let [face_lo, face_hi] = [bounds.x, bounds.y][1 - axis];
            let face_center = face_lo.midpoint(face_hi);
            let span = count.saturating_sub(1) as f64;
            let pitch = self
                .options
                .spacing
                .port_pitch
                .min((hi - lo) / span.max(1.0));
            let start = (center - span * pitch / 2.0).clamp(lo, (hi - span * pitch).max(lo));
            let mut rank = 0.0;
            let mut last = lo - pitch;
            for (ordinal, &i) in indices[cursor..next].iter().enumerate() {
                work.visit(1)?;
                let site = &mut sites[i];
                let cross = if let Some(offset) = site.offset {
                    let at = face_center + offset;
                    if at < face_lo || at > face_hi {
                        return Err(LayoutError::InvalidInput(
                            "fixed grid port lies outside its face",
                        ));
                    }
                    at
                } else {
                    let at = if first.owner >= self.hierarchy.node_count {
                        let high = hi - (span - rank) * pitch;
                        site.preferred.clamp((last + pitch).min(high), high)
                    } else {
                        start + rank * pitch
                    };
                    rank += 1.0;
                    last = at;
                    at
                };
                let mut along = [bounds.x, bounds.y][axis][usize::from(sign > 0)];
                if let Some(node) = self.graph.nodes.get(first.owner) {
                    along -= sign as f64
                        * node
                            .contour
                            .inset_at(node.size, first.side, cross - face_center);
                }
                site.at = if axis == 0 {
                    Point { x: along, y: cross }
                } else {
                    Point { x: cross, y: along }
                };
                site.reach = reach;
                site.lane = FaceLane {
                    ordinal,
                    degree: next - cursor,
                };
            }
            cursor = next;
        }
        Ok(())
    }

    fn lead(&self, site: Site, inside: bool) -> (RoutePoint, Side) {
        let side = if inside {
            opposite(site.side)
        } else {
            site.side
        };
        let body = rect(self.drawing, site.owner);
        let bounds = Bounds::of(body);
        let (axis, sign) = normal(side);
        let (_, face_sign) = normal(site.side);
        let face = [bounds.x, bounds.y][axis][usize::from(face_sign > 0)];
        let mut at = [site.at.x, site.at.y];
        let d = site.reach + self.metrics.separation;
        at[axis] = if sign > 0 {
            (face + self.metrics.separation).max(at[axis] + d)
        } else {
            (face - self.metrics.separation).min(at[axis] - d)
        };
        (knot(point(at), side.axis()), side)
    }

    fn walls(&self, scope: usize, work: &mut Work<'_>) -> LayoutResult<Vec<Rect>> {
        let items = &self.hierarchy.scopes[scope].items;
        work.charge(items.len() + 1)?;
        let mut walls = reserved(items.len() + 1)?;
        for &item in items {
            work.visit(1)?;
            walls.push(rect(self.drawing, item));
        }
        walls.extend(self.title(scope));
        Ok(walls)
    }
}

fn knot(at: Point, axis: Axis) -> RoutePoint {
    RoutePoint::new(at.x, at.y, axis)
}

fn position(at: RoutePoint) -> Point {
    Point { x: at.x, y: at.y }
}

fn leaves(side: Side, from: RoutePoint, toward: RoutePoint) -> bool {
    let (axis, sign) = normal(side);
    [toward.x - from.x, toward.y - from.y][axis] * sign as f64 >= 0.0
}

pub(crate) fn clear(
    router: &OrthoRouter,
    path: &[RoutePoint],
    bounds: Rect,
    sides: [Side; 2],
    work: &mut Work<'_>,
) -> LayoutResult<bool> {
    work.visit(path.len())?;
    if path.len() < 2 {
        return Ok(false);
    }
    let last = path.len() - 1;
    for (start, side) in [(0, sides[0]), (last, sides[1])] {
        let next = if start == 0 {
            path.iter().find(|&&p| position(p) != position(path[start]))
        } else {
            path.iter()
                .rev()
                .find(|&&p| position(p) != position(path[start]))
        };
        if next.is_none_or(|&next| !leaves(side, path[start], next)) {
            return Ok(false);
        }
    }
    for leg in path.windows(2) {
        if position(leg[0]) != position(leg[1]) {
            let Some(seg) = OrthoSegment::between(leg[0], leg[1]) else {
                return Ok(false);
            };
            if router.channel(seg, Some(bounds), work)?.is_none() {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn dogleg(a: RoutePoint, b: RoutePoint, axis: Axis, value: f64) -> [RoutePoint; 4] {
    let (p, q) = match axis {
        Axis::Horizontal => (
            RoutePoint::new(value, a.y, axis),
            RoutePoint::new(value, b.y, Axis::Vertical),
        ),
        Axis::Vertical => (
            RoutePoint::new(a.x, value, axis),
            RoutePoint::new(b.x, value, Axis::Horizontal),
        ),
    };
    [a, p, q, b]
}

fn fan_channel(
    router: &OrthoRouter,
    [start, end]: [(RoutePoint, Side); 2],
    lanes: [FaceLane; 2],
    bounds: Rect,
    work: &mut Work<'_>,
) -> LayoutResult<Option<[RoutePoint; 4]>> {
    let hub = usize::from(lanes[1].degree > lanes[0].degree);
    let lane = lanes[hub];
    if lane.degree < 6 || start.1 != opposite(end.1) {
        return Ok(None);
    }
    let [(from, side), (to, _)] = if hub == 0 { [start, end] } else { [end, start] };
    if !leaves(side, from, to) || cross(side, position(from)) == cross(side, position(to)) {
        return Ok(None);
    }
    let (a, b) = match side.axis() {
        Axis::Horizontal => (from.x, to.x),
        Axis::Vertical => (from.y, to.y),
    };
    if a == b {
        return Ok(None);
    }
    // Keep the outside branches' turns nearer the fan face. Consecutive branches can
    // then occupy distinct tracks without crossing each other's approach legs.
    let ordinal = if cross(side, position(to)) < cross(side, position(from)) {
        lane.ordinal + 1
    } else {
        lane.degree - lane.ordinal
    };
    let fraction = ordinal as f64 / (lane.degree as f64 + 1.0);
    let track = (b - a).mul_add(fraction, a);
    let path = dogleg(start.0, end.0, side.axis(), track);
    Ok(clear(router, &path, bounds, [start.1, end.1], work)?.then_some(path))
}

// The visibility graph may reach its cap on a dense grid. Try the shared channel query
// on the facing midline and the four open perimeter tracks before declining the route.
fn span(
    router: &mut OrthoRouter,
    start: (RoutePoint, Side),
    end: (RoutePoint, Side),
    bounds: Rect,
    separation: f64,
    work: &mut Work<'_>,
) -> LayoutResult<Option<Vec<RoutePoint>>> {
    let [a, b] = [start.0, end.0];
    let sides = [start.1, end.1];
    if (position(a) != position(b) || start.1 == opposite(end.1))
        && let Some(path) = router.route(
            OrthoRequest {
                start: a,
                end: b,
                start_side: Some(start.1),
                end_side: Some(end.1),
                bounds: Some(bounds),
            },
            work,
        )?
    {
        return Ok(Some(path));
    }
    if position(a) == position(b) {
        // Opposed leads already meet. The visibility query tested their shared knot.
        if start.1 == opposite(end.1) {
            return Ok(None);
        }
        let (axis, sign) = normal(start.1);
        let cross = 1 - axis;
        for bend in [separation, -separation] {
            let mut coords = [a.x, a.y];
            let mut path = [a; 5];
            coords[cross] = [a.x, a.y][cross] + bend;
            path[1] = knot(point(coords), Axis::Horizontal);
            coords[axis] += sign as f64 * separation;
            path[2] = knot(point(coords), Axis::Horizontal);
            coords[cross] -= bend;
            path[3] = knot(point(coords), Axis::Horizontal);
            if clear(router, &path, bounds, sides, work)? {
                return Ok(Some(path.to_vec()));
            }
        }
        return Ok(None);
    }
    let [x, y] = [Bounds::of(bounds).x, Bounds::of(bounds).y];
    for (axis, values) in [
        (Axis::Horizontal, [a.x.midpoint(b.x), a.x, b.x, x[0], x[1]]),
        (Axis::Vertical, [a.y.midpoint(b.y), a.y, b.y, y[0], y[1]]),
    ] {
        for value in values {
            let path = dogleg(a, b, axis, value);
            if clear(router, &path, bounds, sides, work)? {
                return Ok(Some(path.to_vec()));
            }
        }
    }
    Ok(None)
}

fn add_point(points: &mut Vec<RoutePoint>, mut next: RoutePoint) {
    if let Some(last) = points.last_mut() {
        if position(*last) == position(next) {
            return;
        }
        next.axis = if last.x == next.x {
            Axis::Vertical
        } else {
            Axis::Horizontal
        };
        if points.len() == 1 {
            points[0].axis = next.axis;
        }
    }
    points.push(next);
}

pub(crate) enum Gutters<'a> {
    Drawing(&'a [Vec<f64>; 2]),
    Scopes(&'a [[Vec<f64>; 2]]),
}

impl Gutters<'_> {
    fn tracks(&self, scope: usize) -> [&[f64]; 2] {
        let axes = match *self {
            Self::Drawing(axes) => axes,
            Self::Scopes(scopes) => &scopes[scope],
        };
        axes.each_ref().map(Vec::as_slice)
    }
}

/// Route against a scope's walls and use its placement gutters when visibility is exhausted.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(
    graph: &Graph,
    hierarchy: &Hierarchy,
    edges: &[(Projection, [Side; 2])],
    metrics: Metrics,
    options: &Options,
    drawing: &mut LayoutOutput,
    gutters: Gutters<'_>,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let context = Routing {
        graph,
        hierarchy,
        drawing,
        metrics,
        options,
    };
    let mut plan = Plan::new(&context, work)?;
    for (index, &(projection, sides)) in edges.iter().enumerate() {
        work.visit(1)?;
        plan.edge(&context, projection, sides, index, work)?;
    }
    context.ports(&mut plan.sites, work)?;
    let mut routed = reserved(plan.scopes.len())?;
    let mut bounds = drawing.bounds;
    for (scope, parts) in plan.scopes.iter().enumerate() {
        work.visit(1)?;
        let mut routes = reserved(parts.len())?;
        if !parts.is_empty() {
            work.charge(parts.len())?;
            let walls = context.walls(scope, work)?;
            let scope_bounds = context.bounds(scope);
            let mut router = OrthoRouter::new(
                &walls,
                metrics.separation,
                options.caps.max_ortho_grid_nodes,
                work,
            )?;
            for part in parts {
                work.visit(1)?;
                let from = plan.sites[part.from.site];
                let to = plan.sites[part.to.site];
                let start = context.lead(from, part.from.inner);
                let end = context.lead(to, part.to.inner);
                let fan = if part.labels.iter().all(Option::is_none) {
                    fan_channel(
                        &router,
                        [start, end],
                        [from.lane, to.lane],
                        scope_bounds,
                        work,
                    )?
                } else {
                    None
                };
                let ordinary = if let Some(fan) = fan {
                    Some(fan.to_vec())
                } else {
                    span(
                        &mut router,
                        start,
                        end,
                        scope_bounds,
                        metrics.separation,
                        work,
                    )?
                };
                let middle = if let Some(points) = ordinary {
                    points
                } else {
                    gutter_route(
                        &router,
                        [start, end],
                        scope_bounds,
                        gutters.tracks(scope),
                        work,
                    )?
                    .ok_or(LayoutError::InvalidInput(
                        "no clear orthogonal channel in grid scope",
                    ))?
                };
                work.charge(middle.len() + 2)?;
                let mut points = reserved(middle.len() + 2)?;
                add_point(&mut points, knot(from.at, start.1.axis()));
                for p in middle {
                    add_point(&mut points, p);
                }
                add_point(&mut points, knot(to.at, end.1.axis()));
                router.record_route(&points, work)?;
                routes.push(Routed {
                    points,
                    ..Default::default()
                });
            }
            captions::place(
                &context,
                &plan,
                scope,
                &walls,
                &mut routes,
                &mut bounds,
                work,
            )?;
        }
        routed.push(routes);
    }
    drawing.edges = reserved(edges.len())?;
    drawing.truncated_routes = plan.truncated;
    for (index, (steps, &(_, sides))) in plan.paths.iter().zip(edges).enumerate() {
        work.visit(1)?;
        let mut points = Vec::<RoutePoint>::new();
        let mut labels = [None; 3];
        for step in steps {
            let part = plan.scopes[step.scope][step.index];
            let route = &routed[step.scope][step.index];
            let mut first = 0;
            work.charge(route.points.len())?;
            points
                .try_reserve(route.points.len())
                .map_err(|_| LayoutError::AllocationLimit)?;
            if let Some(previous) = points.last() {
                if Some(position(*previous)) != route.points.first().copied().map(position) {
                    return Err(LayoutError::InvalidInput("discontinuous grid group route"));
                }
                let site = plan.sites[part.from.site];
                push(
                    &mut drawing.border_stitches,
                    BorderStitch {
                        edge: EdgeId(index as u32),
                        point: points.len() - 1,
                        group: GroupId((site.owner - hierarchy.node_count) as u32),
                        side: site.side,
                    },
                )?;
                first = 1;
            }
            points.extend_from_slice(&route.points[first..]);
            for (label, center) in labels.iter_mut().zip(route.labels) {
                if center.is_some() {
                    *label = center;
                }
            }
        }
        drawing.edges.push(EdgeGeometry {
            points,
            ports: (sides[0], sides[1]),
            label_center: labels[0],
            start_label_center: labels[1],
            end_label_center: labels[2],
            shared_bus: false,
            corridor_clearance: 0.0,
            reversed_for_ranking: false,
        });
    }
    normalize(graph, drawing, work)
}

fn normalize(graph: &Graph, drawing: &mut LayoutOutput, work: &mut Work<'_>) -> LayoutResult<()> {
    let mut extent: Option<Bounds> = None;
    let mut include = |bounds| {
        extent = Some(extent.map_or(bounds, |extent| extent.union(bounds)));
    };
    for rect in drawing
        .nodes
        .iter()
        .copied()
        .chain(drawing.groups.iter().map(|group| group.rect))
    {
        work.visit(1)?;
        include(Bounds::of(rect));
    }
    for (edge, spec) in drawing.edges.iter().zip(&graph.edges) {
        work.visit(1)?;
        for &knot in &edge.points {
            work.visit(1)?;
            let at = position(knot);
            include(Bounds::between(at, at));
        }
        for (center, size) in [
            (edge.label_center, spec.label),
            (edge.start_label_center, spec.start_label),
            (edge.end_label_center, spec.end_label),
        ] {
            if let (Some(center), Some(size)) = (center, size) {
                include(Bounds::centered(center, size));
            }
        }
    }
    let Some(extent) = extent else {
        return Ok(());
    };
    let origin = [extent.x[0], extent.y[0]];
    for node in &mut drawing.nodes {
        work.visit(1)?;
        node.x -= origin[0];
        node.y -= origin[1];
    }
    let translate = |p: &mut Point| {
        p.x -= origin[0];
        p.y -= origin[1];
    };
    for group in &mut drawing.groups {
        work.visit(1)?;
        group.rect.x -= origin[0];
        group.rect.y -= origin[1];
        if let Some(title) = &mut group.title_center {
            translate(title);
        }
    }
    for edge in &mut drawing.edges {
        work.visit(1)?;
        for knot in &mut edge.points {
            work.visit(1)?;
            knot.x -= origin[0];
            knot.y -= origin[1];
        }
        for center in [
            &mut edge.label_center,
            &mut edge.start_label_center,
            &mut edge.end_label_center,
        ]
        .into_iter()
        .flatten()
        {
            translate(center);
        }
    }
    drawing.bounds = Size {
        width: extent.x[1] - origin[0],
        height: extent.y[1] - origin[1],
    };
    Ok(())
}
