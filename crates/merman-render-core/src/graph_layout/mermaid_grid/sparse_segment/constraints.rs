use super::super::routing::GridRoutingChild;
use super::{APPROACH, GridSiblingSegment, finite, invalid, reserved};
use crate::Result;
use crate::graph_layout::Axis;
use crate::resources::OperationWorkMeter;
use kurbo::{Point, Rect};
use merman_model::OperationPhase;

const EPSILON: f64 = 1.0;
const CLEARANCE: f64 = 6.0;
const PAIR_SPACING: f64 = 8.0;
const FOREIGN_SPACING: f64 = 24.0;

#[derive(Clone, Copy)]
struct Segment {
    start: Point,
    end: Point,
    axis: Axis,
}

impl Segment {
    fn fixed(self) -> f64 {
        fixed(self.axis, self.start)
    }

    fn span(self) -> (f64, f64) {
        let (a, b) = (varying(self.axis, self.start), varying(self.axis, self.end));
        (a.min(b), a.max(b))
    }

    fn overlap(self, other: Self) -> (f64, f64) {
        if self.axis != other.axis {
            return (0.0, 0.0);
        }
        let (a0, a1) = self.span();
        let (b0, b1) = other.span();
        (
            a1.min(b1) - a0.max(b0),
            (self.fixed() - other.fixed()).abs(),
        )
    }

    fn pair_conflict(self, other: Self) -> bool {
        let (overlap, distance) = self.overlap(other);
        overlap > 0.0
            && ((distance == 0.0 && overlap >= PAIR_SPACING)
                || (distance > 0.0 && distance < PAIR_SPACING))
    }

    fn foreign_conflict(self, other: Self) -> bool {
        let (overlap, distance) = self.overlap(other);
        overlap > 0.0 && distance < FOREIGN_SPACING
    }
}

struct PairRoute {
    ports: [Point; 2],
    arc_conflicts: Vec<Segment>,
    route_conflicts: Vec<Segment>,
}

pub(super) struct RouteGeometry<'a> {
    pub children: &'a [GridRoutingChild],
    pub terminals: [Option<usize>; 2],
    pub require_approaches: bool,
}

pub(super) struct Constraints<'a> {
    geometry: RouteGeometry<'a>,
    pair_routes: Vec<PairRoute>,
    foreign_routes: Vec<Vec<Segment>>,
}

impl<'a> Constraints<'a> {
    pub(super) fn new(
        input: GridSiblingSegment<'a>,
        include_foreign: bool,
        work: &OperationWorkMeter,
    ) -> Result<Self> {
        Self::from_routes(
            RouteGeometry {
                children: input.children,
                terminals: [Some(input.source_child), Some(input.target_child)],
                require_approaches: true,
            },
            (input.bundle_size > 1).then_some(input.committed_pair_routes),
            include_foreign.then_some(input.foreign_routes),
            work,
        )
    }

    pub(super) fn from_routes(
        geometry: RouteGeometry<'a>,
        committed_pairs: Option<&[&[Point]]>,
        foreign: Option<&[&[Point]]>,
        work: &OperationWorkMeter,
    ) -> Result<Self> {
        for child in geometry.children {
            work.charge(1)?;
            let bounds = child.bounds;
            if ![bounds.x0, bounds.y0, bounds.x1, bounds.y1]
                .into_iter()
                .all(f64::is_finite)
                || bounds.x0 >= bounds.x1
                || bounds.y0 >= bounds.y1
            {
                return Err(invalid("Invalid Grid child bounds"));
            }
        }
        let mut pair_routes = Vec::new();
        if let Some(committed) = committed_pairs {
            pair_routes = reserved(committed.len(), work)?;
            for route in committed {
                let raw = normalize(route, work)?;
                let points = raw.points(route, work)?;
                let [first, .., last] = points.as_slice() else {
                    return Err(invalid("Missing committed Grid pair ports"));
                };
                pair_routes.push(PairRoute {
                    ports: [*first, *last],
                    arc_conflicts: raw.nonterminal(work)?,
                    route_conflicts: normalize(&points, work)?.nonterminal(work)?,
                });
            }
        }
        let mut foreign_routes = Vec::new();
        if let Some(foreign) = foreign {
            foreign_routes = reserved(foreign.len(), work)?;
            for route in foreign {
                foreign_routes.push(normalize(route, work)?.nonterminal(work)?);
            }
        }
        work.checkpoint(OperationPhase::Layout)?;
        Ok(Self {
            geometry,
            pair_routes,
            foreign_routes,
        })
    }

    pub(super) fn has_occupancy(&self) -> bool {
        self.foreign_routes
            .iter()
            .any(|segments| !segments.is_empty())
            || self
                .pair_routes
                .iter()
                .any(|route| !route.arc_conflicts.is_empty())
    }

    pub(super) fn allows_arc(
        &self,
        start: Point,
        end: Point,
        avoid_occupied: bool,
        work: &OperationWorkMeter,
    ) -> Result<bool> {
        let axis = if start.y == end.y {
            Axis::Horizontal
        } else {
            Axis::Vertical
        };
        let arc = Segment { start, end, axis };
        for route in &self.pair_routes {
            work.charge(1)?;
            for &other in &route.arc_conflicts {
                work.charge(1)?;
                if arc.pair_conflict(other) {
                    return Ok(false);
                }
            }
        }
        if avoid_occupied && self.foreign_conflict(&[arc], work)? {
            return Ok(false);
        }
        Ok(true)
    }

    pub(super) fn validates(
        &self,
        points: &[Point],
        avoid_occupied: bool,
        work: &OperationWorkMeter,
    ) -> Result<bool> {
        if !self.clears_geometry(points, work)? || !self.respects_pair(points, false, work)? {
            return Ok(false);
        }
        if avoid_occupied
            && self.foreign_conflict(&normalize(points, work)?.nonterminal(work)?, work)?
        {
            return Ok(false);
        }
        Ok(true)
    }

    pub(super) fn clears_geometry(
        &self,
        points: &[Point],
        work: &OperationWorkMeter,
    ) -> Result<bool> {
        let Some(canonical) = canonicalize(points, work)? else {
            return Ok(false);
        };
        let normalized = normalize(&canonical, work)?;
        let route = normalized.points(&canonical, work)?;
        if route.len() < 2 {
            return Ok(false);
        }
        let segments = &normalized.0;
        if self.geometry.require_approaches
            && let [first, .., last] = segments.as_slice()
            && [first, last]
                .iter()
                .any(|segment| distance(segment.start, segment.end) < APPROACH)
        {
            return Ok(false);
        }
        for (child, item) in self.geometry.children.iter().enumerate() {
            let inflated = item.bounds.inflate(CLEARANCE, CLEARANCE);
            for (index, segment) in route.windows(2).enumerate() {
                work.charge(1)?;
                let terminal = (Some(child) == self.geometry.terminals[0] && index == 0)
                    || (Some(child) == self.geometry.terminals[1] && index == route.len() - 2);
                let bounds = if terminal { item.bounds } else { inflated };
                if enters_rect(segment[0], segment[1], bounds) {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    pub(super) fn respects_pair(
        &self,
        points: &[Point],
        distinct_only: bool,
        work: &OperationWorkMeter,
    ) -> Result<bool> {
        if !self.pair_routes.is_empty() {
            // The reference pair check normalizes the original route again, independently
            // of the canonical points used for rectangle clearance.
            let normalized_points = normalize(points, work)?.points(points, work)?;
            let [first, .., last] = normalized_points.as_slice() else {
                return Ok(false);
            };
            let ports = [*first, *last];
            let candidate = if distinct_only {
                Vec::new()
            } else {
                normalize(&normalized_points, work)?.nonterminal(work)?
            };
            for route in &self.pair_routes {
                work.charge(1)?;
                if ports.iter().any(|port| route.ports.contains(port)) {
                    return Ok(false);
                }
                for &segment in &candidate {
                    work.charge(1)?;
                    for &other in &route.route_conflicts {
                        work.charge(1)?;
                        if segment.pair_conflict(other) {
                            return Ok(false);
                        }
                    }
                }
            }
        }
        Ok(true)
    }

    fn foreign_conflict(&self, candidate: &[Segment], work: &OperationWorkMeter) -> Result<bool> {
        for &segment in candidate {
            for route in &self.foreign_routes {
                work.charge(1)?;
                for &other in route {
                    work.charge(1)?;
                    if segment.foreign_conflict(other) {
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }
}

pub(super) fn attach_ports(
    source: Point,
    search: &[Point],
    target: Point,
    work: &OperationWorkMeter,
) -> Result<Vec<Point>> {
    let mut points = reserved(
        search
            .len()
            .checked_add(2)
            .ok_or_else(|| work.arithmetic_overflow())?,
        work,
    )?;
    points.push(source);
    points.extend_from_slice(search);
    points.push(target);
    normalize_points(&points, work)
}

pub(super) fn normalize_points(points: &[Point], work: &OperationWorkMeter) -> Result<Vec<Point>> {
    normalize(points, work)?.points(points, work)
}

struct Normalized(Vec<Segment>);

impl Normalized {
    fn points(&self, original: &[Point], work: &OperationWorkMeter) -> Result<Vec<Point>> {
        if let Some(first) = self.0.first() {
            let mut points = reserved(
                self.0
                    .len()
                    .checked_add(1)
                    .ok_or_else(|| work.arithmetic_overflow())?,
                work,
            )?;
            points.push(first.start);
            points.extend(self.0.iter().map(|segment| segment.end));
            Ok(points)
        } else {
            let mut points = reserved(original.len(), work)?;
            points.extend_from_slice(original);
            Ok(points)
        }
    }

    fn nonterminal(self, work: &OperationWorkMeter) -> Result<Vec<Segment>> {
        let mut segments = self.0;
        work.charge(segments.len())?;
        if let [only] = segments.as_slice()
            && distance(only.start, only.end) <= 2.0 * APPROACH
        {
            segments.clear();
        }
        if let Some(first) = segments.first_mut() {
            trim(first, true);
        }
        if let Some(last) = segments.last_mut() {
            trim(last, false);
        }
        segments.retain(|segment| segment.start != segment.end);
        Ok(segments)
    }
}

fn normalize(points: &[Point], work: &OperationWorkMeter) -> Result<Normalized> {
    let mut segments: Vec<Segment> = reserved(points.len().saturating_sub(1), work)?;
    for pair in points.windows(2) {
        work.charge(1)?;
        let (start, end) = (pair[0], pair[1]);
        if !finite(start) || !finite(end) {
            return Err(invalid("Non-finite Grid route point"));
        }
        if near(start, end) {
            continue;
        }
        let axis = if (start.y - end.y).abs() <= EPSILON {
            Axis::Horizontal
        } else {
            Axis::Vertical
        };
        if let Some(previous) = segments.last_mut()
            && previous.axis == axis
            && (fixed(axis, previous.end) - fixed(axis, start)).abs() <= EPSILON
            && (previous.fixed() - fixed(axis, start)).abs() <= EPSILON
        {
            previous.end = end;
        } else {
            segments.push(Segment { start, end, axis });
        }
    }
    Ok(Normalized(segments))
}

fn canonicalize(points: &[Point], work: &OperationWorkMeter) -> Result<Option<Vec<Point>>> {
    let [initial, rest @ ..] = points else {
        return Ok(None);
    };
    if rest.is_empty() || !finite(*initial) {
        return Ok(None);
    }
    let mut collapsed = reserved(points.len(), work)?;
    collapsed.push(*initial);
    for (offset, &point) in rest.iter().enumerate() {
        work.charge(1)?;
        if !finite(point) {
            return Ok(None);
        }
        if collapsed.last().is_some_and(|&prior| near(prior, point)) {
            if offset + 1 != rest.len() {
                continue;
            }
            while let [.., before, _] = collapsed.as_slice() {
                work.charge(1)?;
                if !near(*before, point) {
                    break;
                }
                collapsed.pop();
            }
            if collapsed.len() == 1 {
                return Ok(None);
            }
            if let Some(previous) = collapsed.last_mut() {
                *previous = point;
            }
        } else {
            collapsed.push(point);
        }
    }
    if collapsed.len() < 2 {
        return Ok(None);
    }
    let mut axes = reserved(collapsed.len() - 1, work)?;
    for pair in collapsed.windows(2) {
        work.charge(1)?;
        let (a, b) = (pair[0], pair[1]);
        if (a.y - b.y).abs() <= EPSILON {
            axes.push(Axis::Horizontal);
        } else if (a.x - b.x).abs() <= EPSILON {
            axes.push(Axis::Vertical);
        } else {
            return Ok(None);
        }
    }
    let mut canonical = reserved(collapsed.len(), work)?;
    canonical.extend_from_slice(&collapsed);
    let mut start = 0;
    while start < axes.len() {
        let axis = axes[start];
        let mut end = start + 1;
        while end < axes.len() && axes[end] == axis {
            work.charge(1)?;
            end += 1;
        }
        let from = fixed(axis, collapsed[start]);
        let to = fixed(axis, collapsed[end]);
        if start == 0 && end == axes.len() && from != to {
            return Ok(None);
        }
        let coordinate = if end == axes.len() { to } else { from };
        for index in start..=end {
            work.charge(1)?;
            if (fixed(axis, collapsed[index]) - coordinate).abs() > EPSILON {
                return Ok(None);
            }
            match axis {
                Axis::Horizontal => canonical[index].y = coordinate,
                Axis::Vertical => canonical[index].x = coordinate,
            }
        }
        start = end;
    }
    Ok(Some(canonical))
}

fn enters_rect(a: Point, b: Point, rect: Rect) -> bool {
    if a.y == b.y {
        a.y > rect.y0 && a.y < rect.y1 && a.x.max(b.x) > rect.x0 && a.x.min(b.x) < rect.x1
    } else if a.x == b.x {
        a.x > rect.x0 && a.x < rect.x1 && a.y.max(b.y) > rect.y0 && a.y.min(b.y) < rect.y1
    } else {
        true
    }
}

fn trim(segment: &mut Segment, at_start: bool) {
    let difference = varying(segment.axis, segment.end) - varying(segment.axis, segment.start);
    let delta = if difference == 0.0 {
        0.0
    } else {
        difference.signum() * APPROACH
    };
    match (segment.axis, at_start) {
        (Axis::Horizontal, true) => segment.start.x += delta,
        (Axis::Horizontal, false) => segment.end.x -= delta,
        (Axis::Vertical, true) => segment.start.y += delta,
        (Axis::Vertical, false) => segment.end.y -= delta,
    }
}

fn fixed(axis: Axis, point: Point) -> f64 {
    match axis {
        Axis::Horizontal => point.y,
        Axis::Vertical => point.x,
    }
}

fn varying(axis: Axis, point: Point) -> f64 {
    match axis {
        Axis::Horizontal => point.x,
        Axis::Vertical => point.y,
    }
}

fn near(a: Point, b: Point) -> bool {
    (a.x - b.x).abs() <= EPSILON && (a.y - b.y).abs() <= EPSILON
}

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}
