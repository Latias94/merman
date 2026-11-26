//! Route each scope against its immediate walls, then stitch contacts at group faces.

mod captions;
mod comb;
mod gutters;

use gutters::gutter_route;

use super::*;
use crate::geom::Bounds;
use crate::ortho::{OrthoRequest, OrthoRouter, OrthoSegment};
use crate::{
    BorderStitch, EdgeGeometry, EdgeId, MarkerMetrics, Rect, RoutePoint, TitlePosition,
    TruncatedRoute,
};

#[derive(Clone, Copy)]
struct Site {
    owner: usize,
    side: Side,
    preferred: f64,
    tie_preferred: f64,
    offset: Option<f64>,
    inner: bool,
    marker: MarkerMetrics,
    reach: f64,
    at: Point,
    lane: FaceLane,
    aligned_dist: Option<f64>,
    scope: usize,
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
    edge: usize,
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

fn anchor(graph: &Graph, drawing: &LayoutOutput, item: usize) -> Point {
    let body = rect(drawing, item);
    let y = graph
        .nodes
        .get(item)
        .and_then(|node| node.anchor_y)
        .map_or(body.center().y, |offset| body.y + offset);
    Point {
        x: body.center().x,
        y,
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
            let other = anchor(context.graph, context.drawing, ends[1 - end]);
            let from_rect = rect(context.drawing, item);
            let from = anchor(context.graph, context.drawing, item);
            let inner = item == scope_item;
            let offset = match hints[end] {
                PortSideHint::FixedOffset { offset, .. } if item == projection.ends[end] => {
                    Some(offset)
                }
                _ => None,
            };
            let aligned_dist = if !inner && item < h.node_count && sides[1 - end] == opposite(side)
            {
                let (axis, sign) = normal(side);
                let delta = [other.x - from.x, other.y - from.y][axis] * sign as f64;
                ((cross(side, other) - cross(side, from)).abs() <= 1.0 && delta > 0.0)
                    .then_some(delta)
            } else {
                None
            };
            let terminal_side = side;
            let mut terminal = self.site(
                Site {
                    owner: item,
                    side,
                    preferred: cross(side, other),
                    tie_preferred: cross(side, other),
                    offset,
                    inner,
                    marker: markers[end],
                    reach: 0.0,
                    at: Point::default(),
                    lane: FaceLane::default(),
                    aligned_dist,
                    scope: if inner { scope } else { h.parent[item] },
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
                // When the terminal face is perpendicular to the group border it exits through
                // (e.g., entering `orders:L` via `core:Top`), align the border crossing with the
                // terminal's outward lead corridor instead of overshooting to its center.
                let preferred = if terminal_side.axis() != side.axis() {
                    let (t_axis, t_sign) = normal(terminal_side);
                    let b = Bounds::of(from_rect);
                    let edge_coord = [b.x, b.y][t_axis][usize::from(t_sign > 0)];
                    edge_coord + t_sign as f64 * (markers[end].length + context.metrics.separation)
                } else {
                    cross(side, from)
                };
                let outer = self.site(
                    Site {
                        owner: group,
                        side,
                        preferred,
                        tie_preferred: cross(side, other),
                        offset: None,
                        inner: true,
                        marker: MarkerMetrics::default(),
                        reach: 0.0,
                        at: Point::default(),
                        lane: FaceLane::default(),
                        aligned_dist: None,
                        scope: h.parent[group],
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
                        edge,
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
                edge,
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
        if inner
            && matches!(side, Side::Left | Side::Right)
            && let Some(title) = self.title(owner - self.hierarchy.node_count)
        {
            let group = &self.graph.groups[owner - self.hierarchy.node_count];
            let title_is_top = place::is_top(group.title_position);
            let clear = self.metrics.separation;
            if title_is_top {
                range[0] = range[0].max(title.y + title.height + clear);
            } else {
                range[1] = range[1].min(title.y - clear);
            }
        }
        if range[0] > range[1] {
            range = [lo.midpoint(hi); 2];
        }
        [range[0], lo.midpoint(hi), range[1]]
    }

    fn border_title_gap(&self, owner: usize, side: Side, inner: bool) -> Option<(f64, f64)> {
        if !inner
            || owner < self.hierarchy.node_count
            || !matches!(side, Side::Top | Side::Bottom)
        {
            return None;
        }
        let group_id = owner - self.hierarchy.node_count;
        let title = self.title(group_id)?;
        let title_is_top = place::is_top(self.graph.groups[group_id].title_position);
        if title_is_top != (side == Side::Top) {
            return None;
        }
        let clear = self.metrics.separation;
        Some((title.x - clear, title.x + title.width + clear))
    }

    fn ports(&self, sites: &mut [Site], work: &mut Work<'_>) -> LayoutResult<()> {
        let mut indices = reserved(sites.len())?;
        work.charge(sites.len())?;
        indices.extend(0..sites.len());
        work.charge_sort(indices.len())?;
        indices.sort_unstable_by(|&a, &b| {
            let u = &sites[a];
            let v = &sites[b];
            (u.owner, u.side as u8)
                .cmp(&(v.owner, v.side as u8))
                .then_with(|| u.preferred.total_cmp(&v.preferred))
                .then_with(|| u.tie_preferred.total_cmp(&v.tie_preferred))
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
            // When multiple collinear edges share a node face (e.g., Admin and User both
            // targeting Inventory:L), the farther edge must detour around the closer one.
            // Nudge the farther edge toward the open scope interior so it takes the side port
            // matching its detour gutter instead of crossing the direct neighbor's straight line.
            if first.owner < self.hierarchy.node_count && count >= 2 {
                let min_dist = indices[cursor..next]
                    .iter()
                    .filter_map(|&i| {
                        sites[i]
                            .offset
                            .is_none()
                            .then_some(sites[i].aligned_dist)
                            .flatten()
                    })
                    .fold(f64::INFINITY, f64::min);
                if min_dist.is_finite() {
                    let mut nudged = false;
                    for &i in &indices[cursor..next] {
                        let site = &mut sites[i];
                        if site.offset.is_none()
                            && let Some(dist) = site.aligned_dist
                            && dist > min_dist + 1.0
                        {
                            let scope_center = cross(first.side, self.bounds(site.scope).center());
                            let dir = if center <= scope_center { 1.0 } else { -1.0 };
                            site.preferred = center + dir * 2.0;
                            nudged = true;
                        }
                    }
                    if nudged {
                        indices[cursor..next].sort_unstable_by(|&a, &b| {
                            sites[a]
                                .preferred
                                .total_cmp(&sites[b].preferred)
                                .then(a.cmp(&b))
                        });
                    }
                }
            }
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
            // If a single unpinned site on a node face is aligned with the face center,
            // anchor it at `center` so straight direct connections avoid an unnecessary jog.
            let mut anchored_start = None;
            if first.owner < self.hierarchy.node_count && count >= 2 {
                let mut aligned_slot = None;
                let mut aligned_count = 0usize;
                let mut unpinned_rank = 0usize;
                for &i in &indices[cursor..next] {
                    if sites[i].offset.is_some() {
                        continue;
                    }
                    if (sites[i].preferred - center).abs() <= 1.0 {
                        aligned_slot = Some(unpinned_rank);
                        aligned_count += 1;
                    }
                    unpinned_rank += 1;
                }
                if aligned_count == 1
                    && let Some(slot) = aligned_slot
                {
                    let candidate = center - slot as f64 * pitch;
                    if candidate >= lo && candidate + span * pitch <= hi {
                        anchored_start = Some(candidate);
                    }
                }
            }
            let start = anchored_start.unwrap_or_else(|| {
                (center - span * pitch / 2.0).clamp(lo, (hi - span * pitch).max(lo))
            });
            let border_split = self
                .border_title_gap(first.owner, first.side, inner)
                .map(|(gap_left, gap_right)| {
                    let left_hi = gap_left.min(hi);
                    let right_lo = gap_right.max(lo);
                    let port_pitch = self.options.spacing.port_pitch;
                    let capacity = |available: f64| -> usize {
                        if available < 0.0 {
                            0
                        } else if port_pitch > 0.0 {
                            ((available / port_pitch).floor() as usize)
                                .saturating_add(1)
                                .min(count)
                        } else {
                            count
                        }
                    };
                    let left_cap = capacity(left_hi - lo);
                    let right_cap = capacity(hi - right_lo);
                    let middle = gap_left.midpoint(gap_right);
                    let mut left_want = 0usize;
                    for &i in &indices[cursor..next] {
                        if sites[i].offset.is_none() && sites[i].preferred < middle {
                            left_want += 1;
                        }
                    }
                    let split = if left_cap == 0 && right_cap == 0 {
                        // Fallback when title spans the full face: pack onto the wider side.
                        if left_hi - lo >= hi - right_lo {
                            count
                        } else {
                            0
                        }
                    } else {
                        left_want.max(count.saturating_sub(right_cap)).min(left_cap)
                    };
                    (split, left_hi, right_lo)
                });
            let mut rank = 0usize;
            let mut batch_rank = 0.0;
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
                        let (batch_lo, batch_hi, batch_len, local_rank) =
                            if let Some((split, left_hi, right_lo)) = border_split {
                                if rank < split {
                                    (lo, left_hi.max(lo), split, rank)
                                } else {
                                    if rank == split {
                                        last = right_lo.min(hi) - self.options.spacing.port_pitch;
                                    }
                                    (right_lo.min(hi), hi, count - split, rank - split)
                                }
                            } else {
                                (lo, hi, count, rank)
                            };
                        let batch_span = batch_len.saturating_sub(1) as f64;
                        let batch_pitch = self
                            .options
                            .spacing
                            .port_pitch
                            .min((batch_hi - batch_lo).max(0.0) / batch_span.max(1.0));
                        let high = batch_hi - (batch_span - local_rank as f64) * batch_pitch;
                        let low = if local_rank == 0 {
                            batch_lo
                        } else {
                            (last + batch_pitch).max(batch_lo)
                        };
                        site.preferred.clamp(low.min(high), high)
                    } else {
                        start + batch_rank * pitch
                    };
                    rank += 1;
                    batch_rank += 1.0;
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
    if lane.degree < crate::ortho::comb::MIN_DEGREE || start.1 != opposite(end.1) {
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

/// Align transverse site coordinates that differ only by floating-point ULP noise
/// (e.g., two nodes of different widths centered on the same grid track).
fn snap_collinear_sites(sites: &mut [Site], work: &mut Work<'_>) -> LayoutResult<()> {
    const SNAP_EPS: f64 = 1e-9;
    for axis in [Axis::Vertical, Axis::Horizontal] {
        let mut indices = reserved(sites.len())?;
        work.charge(sites.len())?;
        for (i, site) in sites.iter().enumerate() {
            work.visit(1)?;
            if site.side.axis() == axis {
                indices.push(i);
            }
        }
        if indices.len() < 2 {
            continue;
        }
        work.charge_sort(indices.len())?;
        indices.sort_unstable_by(|&a, &b| {
            let ca = cross(sites[a].side, sites[a].at);
            let cb = cross(sites[b].side, sites[b].at);
            ca.total_cmp(&cb).then(a.cmp(&b))
        });
        let mut anchor = cross(sites[indices[0]].side, sites[indices[0]].at);
        for &i in &indices[1..] {
            work.visit(1)?;
            let value = cross(sites[i].side, sites[i].at);
            if (value - anchor).abs() <= SNAP_EPS {
                match axis {
                    Axis::Vertical => sites[i].at.x = anchor,
                    Axis::Horizontal => sites[i].at.y = anchor,
                }
            } else {
                anchor = value;
            }
        }
    }
    Ok(())
}

pub(crate) enum Gutters<'a> {
    Architecture(&'a [Vec<f64>; 2]),
    CardGrid(&'a [[Vec<f64>; 2]]),
}

impl Gutters<'_> {
    fn tracks(&self, scope: usize) -> [&[f64]; 2] {
        let axes = match *self {
            Self::Architecture(axes) => axes,
            Self::CardGrid(scopes) => &scopes[scope],
        };
        axes.each_ref().map(Vec::as_slice)
    }
}

fn adapt_auto_titles(
    graph: &Graph,
    hierarchy: &Hierarchy,
    sites: &[Site],
    metrics: Metrics,
    options: &Options,
    drawing: &mut LayoutOutput,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    if !options.group_policy.adaptive_title_clearance {
        return Ok(());
    }
    let margin = metrics.margin();
    let clear = metrics.separation;
    for (group_id, (spec, geom)) in graph.groups.iter().zip(&mut drawing.groups).enumerate() {
        work.visit(1)?;
        if spec.title_position != TitlePosition::TopAuto {
            continue;
        }
        let Some(title) = spec.title else { continue };
        let Some(current) = geom.title_center else {
            continue;
        };
        let left = geom.rect.x + margin + title.width / 2.0;
        let right = (geom.rect.x + geom.rect.width - margin - title.width / 2.0).max(left);
        let center = geom.rect.center().x.clamp(left, right);
        let candidates = [left, right, center];
        let mut scores = [(0usize, 0.0_f64); 3];
        let owner = hierarchy.node_count + group_id;
        let mut top_prefs = Vec::new();
        for site in sites {
            work.visit(1)?;
            if site.owner != owner || site.side != Side::Top {
                continue;
            }
            push(&mut top_prefs, (site.preferred, site.tie_preferred))?;
        }
        // Account for port_pitch spreading when multiple edges share the same preferred x
        // (e.g., two parallel entries to the same service), so a title candidate does not pick
        // a corner that squeezes the spread port into a border S-jog.
        if !top_prefs.is_empty() {
            work.charge_sort(top_prefs.len())?;
            top_prefs.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.total_cmp(&b.1)));
            let border = options.spacing.corner_clearance.max(clear);
            let lo = geom.rect.x + border.min(geom.rect.width / 2.0);
            let hi = geom.rect.x + geom.rect.width - border.min(geom.rect.width / 2.0);
            let span = top_prefs.len().saturating_sub(1) as f64;
            let pitch = options
                .spacing
                .port_pitch
                .min((hi - lo).max(0.0) / span.max(1.0));
            let mut last = lo - pitch;
            let half = title.width / 2.0 + clear;
            for (rank, &(pref, _)) in top_prefs.iter().enumerate() {
                work.visit(1)?;
                let high = hi - (span - rank as f64) * pitch;
                let at = pref.clamp((last + pitch).min(high), high);
                last = at;
                for (candidate, (hits, shift)) in candidates.into_iter().zip(&mut scores) {
                    let shortfall = half - (at - candidate).abs();
                    if shortfall > 0.0 {
                        *hits += 1;
                        *shift += shortfall;
                    }
                }
            }
        }
        let best = candidates
            .into_iter()
            .zip(scores)
            .min_by(|(_, a), (_, b)| a.0.cmp(&b.0).then_with(|| a.1.total_cmp(&b.1)))
            .expect("three title anchors")
            .0;
        geom.title_center = Some(Point { x: best, ..current });
    }
    Ok(())
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
    let mut plan = {
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
        plan
    };
    adapt_auto_titles(graph, hierarchy, &plan.sites, metrics, options, drawing, work)?;
    let context = Routing {
        graph,
        hierarchy,
        drawing,
        metrics,
        options,
    };
    context.ports(&mut plan.sites, work)?;
    snap_collinear_sites(&mut plan.sites, work)?;
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
    let origin = normalize(graph, drawing, work)?;
    if matches!(gutters, Gutters::Architecture(_))
        && options.bus_routing
        && options.routing == crate::RoutingStyle::Orthogonal
        && edges.len() >= crate::ortho::comb::MIN_DEGREE
    {
        // Finish the required routes before spending surplus work on shared buses.
        let context = Routing {
            graph,
            hierarchy,
            drawing,
            metrics,
            options,
        };
        for (edge, points) in comb::plan(&context, &plan, origin, work)? {
            let route = &mut drawing.edges[edge];
            route.points = points;
            route.shared_bus = true;
        }
    }
    drawing.truncated_routes = plan.truncated;
    Ok(())
}

fn normalize(
    graph: &Graph,
    drawing: &mut LayoutOutput,
    work: &mut Work<'_>,
) -> LayoutResult<Point> {
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
        return Ok(Point::default());
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
    Ok(point(origin))
}
