//! Relieve crowded automatic port rows while the drawing is still in canonical coordinates.

use super::{Component, Drawing, canonical_size};
use crate::geom::{Bounds, Bvh, BvhEntry};
use crate::work::{LayoutResult, Work, push, reserved};
use crate::{
    Axis, EdgeGeometry, EdgeSpec, Graph, KnotKind, LayoutError, MarkerMetrics, Options, Point,
    PortSideHint, Rect, RoutePoint, RoutingStyle, Side, WorkError,
};
use std::ops::ControlFlow::{Break, Continue};

const MARKER_PITCH: f64 = 1.15;
// A component may contain many otherwise independent crowded hubs. Bound the total clearance
// traversal even when the same obstacles are in range of all of them.
const CLEARANCE_PASSES: usize = 64;
const DIRECT_PROPOSAL_LIMIT: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq)]
enum End {
    Source,
    Target,
}

impl End {
    const BOTH: [Self; 2] = [Self::Source, Self::Target];

    fn face(self) -> Side {
        match self {
            Self::Source => Side::Bottom,
            Self::Target => Side::Top,
        }
    }

    fn outer_y(self) -> f64 {
        match self {
            Self::Source => 1.0,
            Self::Target => -1.0,
        }
    }

    fn peer(self) -> Self {
        match self {
            Self::Source => Self::Target,
            Self::Target => Self::Source,
        }
    }

    fn index(self, len: usize, distance: usize) -> usize {
        match self {
            Self::Source => distance,
            Self::Target => len - 1 - distance,
        }
    }

    fn point(self, points: &[RoutePoint], distance: usize) -> RoutePoint {
        points[self.index(points.len(), distance)]
    }

    fn side(self, edge: &EdgeGeometry) -> Side {
        match self {
            Self::Source => edge.ports.0,
            Self::Target => edge.ports.1,
        }
    }

    fn marker(self, edge: &EdgeSpec) -> MarkerMetrics {
        match self {
            Self::Source => edge.start_marker,
            Self::Target => edge.end_marker,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Face {
    count: usize,
    marked: usize,
    width: f64,
    depth: f64,
    left: f64,
    right: f64,
    held: bool,
    run: Option<usize>,
}

impl Face {
    fn add(&mut self, x: f64, marker: MarkerMetrics, movable: bool) {
        if self.count == 0 {
            (self.left, self.right) = (x, x);
        } else {
            self.left = self.left.min(x);
            self.right = self.right.max(x);
        }
        self.count += 1;
        if marker.width > 0.0 && marker.length > 0.0 {
            self.marked += 1;
            self.width = self.width.max(marker.width);
        }
        self.depth = self.depth.max(marker.length);
        self.held |= !movable;
    }
}

struct Run {
    hub: usize,
    end: End,
    main_span: f64,
    side_span: f64,
    pitch: f64,
    main_count: usize,
    main_capacity: usize,
    side_capacity: usize,
    runway: f64,
    edges: Vec<usize>,
}

fn capacity(span: f64, pitch: f64, count: usize) -> usize {
    1 + ((span / pitch).floor() as usize).min(count - 1)
}

impl Run {
    fn new(
        hub: usize,
        end: End,
        face: Face,
        drawing: &Drawing,
        c: &Component,
        options: &Options,
    ) -> Option<Self> {
        let spacing = options.spacing;
        let pitch = spacing.min_port_pitch.max(MARKER_PITCH * face.width);
        if face.held
            || face.marked < 2
            || !pitch.is_finite()
            || (face.count == 2 && face.right - face.left >= pitch)
        {
            return None;
        }
        let rect = drawing.nodes[hub];
        let node = &c.nodes[hub];
        let usable_span = |size: f64, measured: Option<f64>| {
            measured
                .unwrap_or((size - 2.0 * spacing.corner_clearance).max(0.0))
                .min((size - face.width).max(0.0))
        };
        let main_span = usable_span(rect.width, node.port_span);
        let side_span = usable_span(rect.height, node.perpendicular_port_span.or(node.port_span));
        let runway = face.depth + spacing.edge;
        if main_span <= 0.0 || side_span <= 0.0 || !runway.is_finite() || runway <= 0.0 {
            return None;
        }
        let main_count = capacity(main_span, pitch, face.count);
        // A measured marker can reserve its entire viewport. Aim for the comfortable primary
        // pitch, then use the remaining primary space if the sides cannot take every overflow.
        // Even the tighter pitch separates the full reserved footprints.
        let footprint_pitch = spacing.min_port_pitch.max(face.width);
        let main_capacity = capacity(main_span, footprint_pitch, face.count);
        let side_capacity = capacity(side_span, footprint_pitch, face.count);
        if (face.count - main_capacity).div_ceil(2) > side_capacity {
            return None;
        }
        Some(Self {
            hub,
            end,
            main_span,
            side_span,
            pitch,
            main_count,
            main_capacity,
            side_capacity,
            runway,
            edges: Vec::new(),
        })
    }
}

impl Bounds {
    fn path(points: &[RoutePoint]) -> Self {
        let first = point(points[0]);
        let mut bounds = Self::between(first, first);
        for &p in &points[1..] {
            bounds.include(point(p));
        }
        bounds
    }
}

fn point(p: RoutePoint) -> Point {
    Point { x: p.x, y: p.y }
}

fn marker(p: Point, side: Side, mark: MarkerMetrics) -> Option<Bounds> {
    if mark.width == 0.0 || mark.length == 0.0 {
        return None;
    }
    let half = mark.width * 0.5;
    let (dx, dy) = match side {
        Side::Left => (-mark.length, 0.0),
        Side::Right => (mark.length, 0.0),
        Side::Top => (0.0, -mark.length),
        Side::Bottom => (0.0, mark.length),
    };
    let (wx, wy) = if side.axis() == Axis::Vertical {
        (half, 0.0)
    } else {
        (0.0, half)
    };
    Some(Bounds::between(
        Point {
            x: p.x - wx,
            y: p.y - wy,
        },
        Point {
            x: p.x + dx + wx,
            y: p.y + dy + wy,
        },
    ))
}

#[derive(Debug)]
struct Tail {
    /// Points from the contact through the beginning of the old fan-in turn, inclusive.
    cut: usize,
    fixed: RoutePoint,
    shelf: Option<f64>,
    guard: f64,
    straight: bool,
}

impl Tail {
    fn read(
        end: End,
        edge: &EdgeGeometry,
        spec: &EdgeSpec,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Self>> {
        let points = &edge.points;
        let contact = end.point(points, 0);
        let outward = end.outer_y();
        let progress = |p: RoutePoint| outward * (p.y - contact.y);
        let mut previous = contact;
        let mut cut = points.len() - 1;
        let mut straight = true;
        let mut shelf = None;
        let mut guard = 0.0_f64;
        for distance in 1..points.len() {
            work.visit(1)?;
            let p = end.point(points, distance);
            if progress(p) < progress(previous) {
                return Ok(None);
            }
            if p.x != contact.x {
                straight = false;
                cut = distance;
                if p.y == previous.y && distance + 1 < points.len() {
                    let before = end.point(points, distance + 1);
                    if before.x != p.x || progress(before) <= progress(p) {
                        return Ok(None);
                    }
                    shelf = Some(p.y);
                    cut += 1;
                }
                break;
            }
            if p.kind == KnotKind::GapBoundary && guard == 0.0 {
                guard = progress(p);
            }
            previous = p;
        }
        let mut fixed = end.point(points, cut);
        if cut + 1 < points.len() {
            let before = end.point(points, cut + 1);
            if before.x != fixed.x || progress(before) <= progress(fixed) {
                return Ok(None);
            }
        } else {
            let peer = end.peer();
            if peer.side(edge).axis() != Axis::Vertical {
                return Ok(None);
            }
            // Keep the opposite contact and its marker lead in the retained part.
            fixed.y -= outward * peer.marker(spec).length;
        }
        fixed.kind = KnotKind::GapBoundary;
        Ok((progress(fixed) > 0.0).then_some(Self {
            cut,
            fixed,
            shelf,
            guard,
            straight,
        }))
    }
}

#[derive(Debug)]
struct Edit {
    points: Vec<RoutePoint>,
    side: Side,
    body: [Bounds; 2],
    retained: Bounds,
    envelope: Bounds,
    mark: Option<Bounds>,
}

impl Edit {
    fn blocked_by(&self, wall: Bounds) -> bool {
        self.body.iter().any(|&body| body.overlaps_strictly(wall))
            || self.mark.is_some_and(|mark| mark.overlaps_strictly(wall))
    }

    fn blocks_marker(&self, marker: Bounds) -> bool {
        // The retained envelope also covers corners when the painter coalesces free knots.
        self.retained.overlaps_strictly(marker)
            || self.body.iter().any(|part| part.overlaps_strictly(marker))
    }

    fn new(
        run: &Run,
        tail: &Tail,
        (side, at): (Side, Point),
        edge: &EdgeGeometry,
        spec: &EdgeSpec,
        options: &Options,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Self>> {
        let outward = run.end.outer_y();
        let depth = |y: f64| outward * (y - at.y);
        let fixed = point(tail.fixed);
        let (guard, shelf) = if side == run.end.face() {
            let guard = Point {
                y: at.y + outward * run.runway.max(tail.guard),
                ..at
            };
            if depth(fixed.y) <= depth(guard.y) {
                return Ok(None);
            }
            let shelf = if at.x == fixed.x {
                None
            } else if let Some(y) = tail.shelf {
                if !(depth(guard.y) < depth(y) && depth(y) < depth(fixed.y)) {
                    return Ok(None);
                }
                Some(y)
            } else {
                (options.routing == RoutingStyle::Orthogonal).then(|| guard.y.midpoint(fixed.y))
            };
            (guard, shelf)
        } else {
            let sign = if side == Side::Left { -1.0 } else { 1.0 };
            let guard = Point {
                x: at.x + sign * run.runway,
                ..at
            };
            if sign * (fixed.x - guard.x) <= 0.0 || depth(fixed.y) <= 0.0 {
                return Ok(None);
            }
            (guard, None)
        };
        if at == guard
            || ![at.x, at.y, guard.x, guard.y, fixed.y]
                .into_iter()
                .all(f64::is_finite)
        {
            return Ok(None);
        }
        let axis = side.axis();
        let mut knots = [RoutePoint::new(at.x, at.y, axis); 5];
        knots[1] = RoutePoint::gap_boundary(guard.x, guard.y, axis);
        let mut len = 2;
        if axis == Axis::Horizontal {
            knots[len] = RoutePoint::new(fixed.x, at.y, Axis::Horizontal);
            len += 1;
        } else if let Some(y) = shelf {
            knots[len] = RoutePoint::new(at.x, y, Axis::Vertical);
            knots[len + 1] = RoutePoint::new(fixed.x, y, Axis::Horizontal);
            len += 2;
        }
        knots[len] = RoutePoint {
            axis: Axis::Vertical,
            ..tail.fixed
        };
        let knots = &knots[..=len];
        let count = edge
            .points
            .len()
            .checked_add(knots.len())
            .ok_or(WorkError::ArithmeticOverflow)?;
        work.charge(count)?;
        let mut points = reserved(count)?;
        let cut = run.end.index(edge.points.len(), tail.cut);
        let mut retained = match run.end {
            End::Source => Bounds::path(&edge.points[cut..]),
            End::Target => Bounds::path(&edge.points[..=cut]),
        };
        retained.include(fixed);
        match run.end {
            End::Source => {
                points.extend_from_slice(knots);
                let keep = cut + usize::from(fixed == point(edge.points[cut]));
                points.extend_from_slice(&edge.points[keep..]);
            }
            End::Target => {
                let keep = cut + usize::from(fixed != point(edge.points[cut]));
                points.extend_from_slice(&edge.points[..keep]);
                let axis = if keep == cut {
                    edge.points[cut].axis
                } else {
                    Axis::Vertical
                };
                points.push(RoutePoint { axis, ..knots[len] });
                for k in (0..len).rev() {
                    points.push(RoutePoint {
                        axis: knots[k + 1].axis,
                        ..knots[k]
                    });
                }
            }
        }
        work.visit(points.len())?;
        let envelope = Bounds::path(&points);
        Ok(Some(Self {
            points,
            side,
            body: [Bounds::between(at, guard), Bounds::between(guard, fixed)],
            retained,
            envelope,
            mark: marker(at, side, run.end.marker(spec)),
        }))
    }
}

#[derive(Clone, Copy)]
struct RouteBounds {
    stroke: Bounds,
    markers: [Option<Bounds>; 2],
    changed: bool,
}

struct IndexedRoute {
    edge: usize,
    stroke: Bounds,
    markers: [Option<Bounds>; 2],
}

struct RouteIndex {
    routes: Bvh<IndexedRoute>,
}

impl RouteIndex {
    fn new(
        routes: impl ExactSizeIterator<Item = RouteBounds>,
        work: &mut Work<'_>,
    ) -> LayoutResult<Self> {
        work.charge(routes.len())?;
        let mut items = reserved(routes.len())?;
        for (edge, route) in routes.enumerate() {
            items.push(IndexedRoute {
                edge,
                stroke: route.stroke,
                markers: route.markers,
            });
        }
        Ok(Self {
            routes: Bvh::build_with_work(
                items,
                |route| {
                    let mut bounds = route.stroke;
                    for marker in route.markers.into_iter().flatten() {
                        bounds = bounds.union(marker);
                    }
                    bounds
                },
                work,
            )?,
        })
    }

    fn allows(
        &self,
        edit: &Edit,
        remaining: &mut usize,
        work: &mut Work<'_>,
        check_marker: impl Fn(usize, End) -> bool,
        blocks_stroke: impl Fn(usize, Bounds) -> bool,
    ) -> LayoutResult<bool> {
        clear_index(
            &self.routes,
            remaining,
            work,
            |bounds| {
                edit.blocked_by(bounds)
                    || edit.mark.is_some_and(|mark| mark.overlaps_strictly(bounds))
            },
            |item| {
                item.markers[End::Source as usize].is_some_and(|bounds| {
                    edit.blocked_by(bounds) && check_marker(item.edge, End::Source)
                }) || item.markers[End::Target as usize].is_some_and(|bounds| {
                    edit.blocked_by(bounds) && check_marker(item.edge, End::Target)
                }) || edit.mark.is_some_and(|mark| {
                    mark.overlaps_strictly(item.stroke) && blocks_stroke(item.edge, mark)
                })
            },
        )
    }
}

struct Clearance {
    walls: Bvh<Bounds>,
    routes: Vec<RouteBounds>,
    index: RouteIndex,
    changed: Vec<usize>,
    remaining: usize,
}

impl Clearance {
    fn new(drawing: &Drawing, graph: &Graph, work: &mut Work<'_>) -> LayoutResult<Self> {
        work.charge(drawing.nodes.len())?;
        let mut walls = reserved(drawing.nodes.len())?;
        walls.extend(
            drawing
                .nodes
                .iter()
                .copied()
                .map(|rect| Bounds::of(rect).ordered())
                .filter(|b| b.has_area()),
        );
        work.charge(drawing.edges.len())?;
        let mut routes = reserved(drawing.edges.len())?;
        for (id, edge) in &drawing.edges {
            let Some((first, rest)) = edge.points.split_first() else {
                return Err(LayoutError::InvalidInput("missing hub route contacts"));
            };
            let last = rest.last().unwrap_or(first);
            work.visit(edge.points.len())?;
            let spec = &graph.edges[*id as usize];
            routes.push(RouteBounds {
                stroke: Bounds::path(&edge.points),
                markers: [
                    marker(point(*first), edge.ports.0, spec.start_marker),
                    marker(point(*last), edge.ports.1, spec.end_marker),
                ],
                changed: false,
            });
            for (center, size) in [
                (edge.label_center, spec.label),
                (edge.start_label_center, spec.start_label),
                (edge.end_label_center, spec.end_label),
            ] {
                if let (Some(p), Some(size)) = (center, size) {
                    let bounds =
                        Bounds::centered(p, canonical_size(graph.direction, size)).ordered();
                    if bounds.has_area() {
                        push(&mut walls, bounds)?;
                    }
                }
            }
        }
        let remaining = drawing
            .edges
            .len()
            .saturating_add(walls.len())
            .saturating_mul(CLEARANCE_PASSES);
        Ok(Self {
            walls: Bvh::from_bounds(walls, work)?,
            index: RouteIndex::new(routes.iter().copied(), work)?,
            routes,
            changed: Vec::new(),
            remaining,
        })
    }

    fn accept(
        &mut self,
        end: End,
        proposed: &mut Vec<(usize, Edit)>,
        drawing: &mut Drawing,
        saved: &mut Vec<SavedRoute>,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        work.charge(proposed.len())?;
        saved
            .try_reserve(proposed.len())
            .map_err(|_| LayoutError::AllocationLimit)?;
        self.changed
            .try_reserve(proposed.len())
            .map_err(|_| LayoutError::AllocationLimit)?;
        for (i, edit) in proposed.drain(..) {
            let route = &mut self.routes[i];
            if !route.changed {
                self.changed.push(i);
                route.changed = true;
            }
            route.stroke = edit.envelope;
            route.markers[end as usize] = edit.mark;

            let edge = &mut drawing.edges[i].1;
            saved.push(SavedRoute {
                edge: i,
                points: std::mem::replace(&mut edge.points, edit.points),
                ports: edge.ports,
                corridor_clearance: edge.corridor_clearance,
            });
            edge.corridor_clearance = 0.0;
            match end {
                End::Source => edge.ports.0 = edit.side,
                End::Target => edge.ports.1 = edit.side,
            }
        }
        Ok(())
    }

    fn refresh_index(&mut self, work: &mut Work<'_>) -> LayoutResult<()> {
        // Keep recent edits out of the snapshot until rebuilding can be amortized over
        // a square root of the edge count. Each route can be changed at both ends.
        if self.changed.len() > self.routes.len().isqrt() {
            self.index = RouteIndex::new(self.routes.iter().copied(), work)?;
            for edge in self.changed.drain(..) {
                self.routes[edge].changed = false;
            }
        }
        Ok(())
    }

    fn allows_walls(&mut self, edit: &Edit, work: &mut Work<'_>) -> LayoutResult<bool> {
        if self.walls.root_bounds().is_none() {
            return visit_clearance(&mut self.remaining, 1, work);
        }
        clear_index(
            &self.walls,
            &mut self.remaining,
            work,
            |bounds| edit.blocked_by(bounds),
            |&wall| edit.blocked_by(wall),
        )
    }

    fn allows(
        &mut self,
        end_to_move: End,
        proposed: &[(usize, Edit)],
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        let Some((last, _)) = proposed.last() else {
            return Ok(true);
        };
        if !visit_clearance(&mut self.remaining, proposed.len(), work)? {
            return Ok(false);
        }
        if *last >= self.routes.len() || !proposed.is_sorted_by(|a, b| a.0 < b.0) {
            return Err(LayoutError::InvalidInput("invalid hub proposal edges"));
        }
        self.refresh_index(work)?;
        let patch_index = if proposed.len() > DIRECT_PROPOSAL_LIMIT {
            Some(RouteIndex::new(
                proposed.iter().map(|(_, edit)| {
                    let mut markers = [None; 2];
                    markers[end_to_move as usize] = edit.mark;
                    RouteBounds {
                        stroke: edit.retained.union(edit.body[0]).union(edit.body[1]),
                        markers,
                        changed: false,
                    }
                }),
                work,
            )?)
        } else {
            None
        };
        // The proposal replaces the moved marker and stroke. The peer marker still comes
        // from the current route even when this edit is for that same edge.
        let replaced = |edge| proposed.binary_search_by_key(&edge, |&(i, _)| i).is_ok();
        for (position, &(edge, ref edit)) in proposed.iter().enumerate() {
            if !self.allows_walls(edit, work)? {
                return Ok(false);
            }
            if !self.index.allows(
                edit,
                &mut self.remaining,
                work,
                |i, end| !self.routes[i].changed && (end != end_to_move || !replaced(i)),
                |i, _| !self.routes[i].changed && i != edge && !replaced(i),
            )? {
                return Ok(false);
            }
            for &i in &self.changed {
                if !visit_clearance(&mut self.remaining, 1, work)? {
                    return Ok(false);
                }
                let other = &self.routes[i];
                let replaced = replaced(i);
                for end in End::BOTH {
                    if (end != end_to_move || !replaced)
                        && other.markers[end as usize].is_some_and(|mark| edit.blocked_by(mark))
                    {
                        return Ok(false);
                    }
                }
                if i != edge
                    && !replaced
                    && edit
                        .mark
                        .is_some_and(|mark| other.stroke.overlaps_strictly(mark))
                {
                    return Ok(false);
                }
            }
            if let Some(index) = &patch_index {
                if !index.allows(
                    edit,
                    &mut self.remaining,
                    work,
                    |i, _| i != position,
                    |i, marker| i != position && proposed[i].1.blocks_marker(marker),
                )? {
                    return Ok(false);
                }
            } else {
                for (i, other) in proposed {
                    if !visit_clearance(&mut self.remaining, 1, work)?
                        || (*i != edge
                            && (other.mark.is_some_and(|mark| edit.blocked_by(mark))
                                || edit.mark.is_some_and(|mark| other.blocks_marker(mark))))
                    {
                        return Ok(false);
                    }
                }
            }
        }
        Ok(true)
    }
}

fn clear_index<T>(
    index: &Bvh<T>,
    remaining: &mut usize,
    work: &mut Work<'_>,
    may_block: impl Fn(Bounds) -> bool,
    blocks: impl Fn(&T) -> bool,
) -> LayoutResult<bool> {
    let result = index.visit(|entry| -> LayoutResult<_> {
        if !visit_clearance(remaining, 1, work)? {
            return Ok(Break(()));
        }
        Ok(match entry {
            BvhEntry::Node(bounds) => Continue(may_block(bounds)),
            BvhEntry::Item(item) if blocks(item) => Break(()),
            BvhEntry::Item(_) => Continue(true),
        })
    })?;
    Ok(result.is_continue())
}

fn visit_clearance(remaining: &mut usize, units: usize, work: &mut Work<'_>) -> LayoutResult<bool> {
    let Some(left) = remaining.checked_sub(units) else {
        *remaining = 0;
        return Ok(false);
    };
    *remaining = left;
    work.visit(units)?;
    Ok(true)
}

fn seats(
    run: &Run,
    tails: &[Tail],
    rect: Rect,
    left: usize,
    options: &Options,
) -> impl Iterator<Item = (Side, Point)> {
    let center = rect.center();
    let far = rect.x + rect.width;
    let (primary_y, sign) = match run.end {
        End::Source => (rect.y + rect.height, -1.0),
        End::Target => (rect.y, 1.0),
    };
    let count = run.edges.len();
    let right = count - run.main_count - left;
    let pitch = |span: f64, count: usize| {
        options
            .spacing
            .port_pitch
            .max(run.pitch)
            .min(span / count.saturating_sub(1).max(1) as f64)
    };
    let primary_pitch = pitch(run.main_span, run.main_count);
    let middle = (run.main_count - 1) as f64 * 0.5;
    let slack = ((run.main_span - 2.0 * middle * primary_pitch) * 0.5).max(0.0);
    let straight = tails[left..left + run.main_count]
        .iter()
        .enumerate()
        .filter_map(|(i, tail)| {
            let delta = tail.fixed.x - center.x - (i as f64 - middle) * primary_pitch;
            (tail.straight && delta.abs() <= slack).then_some((left + i, delta))
        })
        .min_by(|a, b| a.1.abs().total_cmp(&b.1.abs()));
    let shift = straight.map_or(0.0, |(_, delta)| delta);
    (0..count).map(move |i| {
        if (left..left + run.main_count).contains(&i) {
            let x = if straight.is_some_and(|(index, _)| i == index) {
                tails[i].fixed.x
            } else {
                center.x + shift + ((i - left) as f64 - middle) * primary_pitch
            };
            (run.end.face(), Point { x, y: primary_y })
        } else {
            let (side, x, slot, count) = if i < left {
                (Side::Left, rect.x, left - 1 - i, left)
            } else {
                (Side::Right, far, i - left - run.main_count, right)
            };
            let y = center.y
                + sign * (slot as f64 - (count - 1) as f64 * 0.5) * pitch(run.side_span, count);
            (side, Point { x, y })
        }
    })
}

#[derive(Debug)]
struct SavedRoute {
    edge: usize,
    points: Vec<RoutePoint>,
    ports: (Side, Side),
    corridor_clearance: f64,
}

#[derive(Debug, Default)]
pub(super) struct HubScratch {
    faces: Vec<[Face; 2]>,
    tails: Vec<Tail>,
    proposed: Vec<(usize, Edit)>,
    saved: Vec<SavedRoute>,
}

impl HubScratch {
    pub(super) fn retained_bytes(&self) -> usize {
        use crate::scratch::capacity_bytes;
        capacity_bytes(&self.faces)
            + capacity_bytes(&self.tails)
            + capacity_bytes(&self.proposed)
            + capacity_bytes(&self.saved)
    }
}

#[cfg(test)]
fn spread(
    drawing: &mut Drawing,
    c: &Component,
    graph: &Graph,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    spread_with_scratch(drawing, c, graph, options, &mut HubScratch::default(), work)
}

pub(super) fn spread_with_scratch(
    drawing: &mut Drawing,
    c: &Component,
    graph: &Graph,
    options: &Options,
    scratch: &mut HubScratch,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let result = spread_faces(drawing, c, graph, options, scratch, work);
    scratch.proposed.clear();
    if result.is_err() {
        // Both ends of an edge can move in separate runs, so undo them in reverse order.
        for saved in scratch.saved.drain(..).rev() {
            let route = &mut drawing.edges[saved.edge].1;
            route.points = saved.points;
            route.ports = saved.ports;
            route.corridor_clearance = saved.corridor_clearance;
        }
    } else {
        scratch.saved.clear();
    }
    result
}

fn spread_faces(
    drawing: &mut Drawing,
    c: &Component,
    graph: &Graph,
    options: &Options,
    scratch: &mut HubScratch,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    use crate::scratch::clear_reserve;
    if c.links.len() < 2 || c.boundaries || c.pinned || c.group_proxies {
        return Ok(());
    }
    let HubScratch {
        faces,
        tails,
        proposed,
        saved,
    } = scratch;
    work.charge(c.nodes.len())?;
    clear_reserve(faces, c.nodes.len())?;
    faces.resize(c.nodes.len(), [Face::default(); 2]);
    for (link, (id, edge)) in c.links.iter().zip(&drawing.edges) {
        work.visit(1)?;
        let spec = &graph.edges[*id as usize];
        let movable = !edge.reversed_for_ranking
            && edge.ports == (Side::Bottom, Side::Top)
            && spec.source_port == PortSideHint::Auto
            && spec.target_port == PortSideHint::Auto
            && c.nodes[link.from as usize].anchor_y.is_none()
            && c.nodes[link.to as usize].anchor_y.is_none()
            && spec.label.is_none()
            && spec.start_label.is_none()
            && spec.end_label.is_none();
        for (node, end) in [(link.from, End::Source), (link.to, End::Target)] {
            let face = match end.side(edge) {
                Side::Bottom => &mut faces[node as usize][End::Source as usize],
                Side::Top => &mut faces[node as usize][End::Target as usize],
                _ => continue,
            };
            face.add(end.point(&edge.points, 0).x, end.marker(spec), movable);
        }
    }
    for loop_edge in &c.loops {
        work.visit(1)?;
        for face in &mut faces[loop_edge.node as usize] {
            face.held = true;
        }
    }
    let mut runs = Vec::new();
    for (hub, faces) in faces.iter_mut().enumerate() {
        work.visit(1)?;
        for end in End::BOTH {
            let face = &mut faces[end as usize];
            if let Some(mut run) = Run::new(hub, end, *face, drawing, c, options) {
                work.charge(face.count)?;
                run.edges = reserved(face.count)?;
                face.run = Some(runs.len());
                push(&mut runs, run)?;
            }
        }
    }
    if runs.is_empty() {
        return Ok(());
    }
    for (i, link) in c.links.iter().enumerate() {
        work.visit(1)?;
        for (node, end) in [(link.from, End::Source), (link.to, End::Target)] {
            if let Some(run) = faces[node as usize][end as usize].run
                && end.side(&drawing.edges[i].1) == end.face()
            {
                runs[run].edges.push(i);
            }
        }
    }
    let mut clearance: Option<Clearance> = None;
    for mut run in runs {
        if clearance.as_ref().is_some_and(|proof| proof.remaining == 0) {
            break;
        }
        let across = |i: usize| run.end.point(&drawing.edges[i].1.points, 0).x;
        work.sort(&mut run.edges, |&a, &b| {
            across(a).total_cmp(&across(b)).then(a.cmp(&b))
        })?;
        work.visit(run.edges.len())?;
        if !run
            .edges
            .windows(2)
            .any(|pair| across(pair[1]) - across(pair[0]) < run.pitch)
        {
            continue;
        }
        clear_reserve(tails, run.edges.len())?;
        for &i in &run.edges {
            let (id, edge) = &drawing.edges[i];
            if run.end.side(edge) != run.end.face()
                || run.end.point(&edge.points, 0).axis != Axis::Vertical
            {
                break;
            }
            let Some(tail) = Tail::read(run.end, edge, &graph.edges[*id as usize], work)? else {
                break;
            };
            tails.push(tail);
        }
        if tails.len() != run.edges.len() {
            continue;
        }
        let rect = drawing.nodes[run.hub];
        work.visit(run.edges.len())?;
        let left = tails
            .iter()
            .take(run.side_capacity)
            .take_while(|tail| tail.fixed.x < rect.x - run.runway)
            .count();
        let right = tails
            .iter()
            .rev()
            .take(run.side_capacity)
            .take_while(|tail| tail.fixed.x > rect.x + rect.width + run.runway)
            .count();
        run.main_count = run
            .main_count
            .max(run.edges.len().saturating_sub(left + right));
        if run.main_count > run.main_capacity {
            continue;
        }
        let spill = run.edges.len() - run.main_count;
        let least = spill.saturating_sub(right);
        let most = left.min(spill);
        if least > most {
            continue;
        }
        if clearance.is_none() {
            clearance = Some(Clearance::new(drawing, graph, work)?);
        }
        let proof = clearance.as_mut().unwrap();
        let preferred = (spill / 2).clamp(least, most);
        // An obstructed side can give its overflow to the other side when there is room.
        let splits =
            std::iter::once(preferred).chain((least..=most).filter(|&left| left != preferred));
        clear_reserve(proposed, run.edges.len())?;
        'split: for left in splits {
            if !visit_clearance(&mut proof.remaining, run.edges.len(), work)? {
                break;
            }
            let right = left + run.main_count;
            let ordered = |tails: &[Tail]| {
                tails
                    .windows(2)
                    .all(|pair| pair[0].fixed.x < pair[1].fixed.x)
            };
            if !ordered(&tails[..=left]) || !ordered(&tails[right - 1..]) {
                continue;
            }
            proposed.clear();
            for ((&i, tail), seat) in run
                .edges
                .iter()
                .zip(tails.iter())
                .zip(seats(&run, tails, rect, left, options))
            {
                let (id, edge) = &drawing.edges[i];
                if seat == (run.end.side(edge), point(run.end.point(&edge.points, 0))) {
                    continue;
                }
                let Some(edit) = Edit::new(
                    &run,
                    tail,
                    seat,
                    edge,
                    &graph.edges[*id as usize],
                    options,
                    work,
                )?
                else {
                    continue 'split;
                };
                proposed.push((i, edit));
            }
            work.charge_sort(proposed.len())?;
            proposed.sort_unstable_by_key(|(i, _)| *i);
            work.check()?;
            if !proof.allows(run.end, proposed, work)? {
                continue;
            }
            proof.accept(run.end, proposed, drawing, saved, work)?;
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
