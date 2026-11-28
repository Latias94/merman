//! Mermaid 12.4's minimal corridor route for an ordinary edge within one Grid container.

use crate::graph_layout::{Axis, Side};
use crate::resources::OperationWorkMeter;
use crate::{Error, Result};
use kurbo::{Point, Rect};
use merman_model::OperationPhase;

pub mod labels;
pub mod outline;
#[cfg(feature = "svg-edge-parity")]
pub mod paint;
#[cfg(test)]
mod tests;

// Grid routerTopology.ts, routerPlanning.ts and layout-utils/geometry.ts.
const ROUTE_CLEARANCE: f64 = 6.0;
const EDGE_CLEARANCE: f64 = 24.0;
const TERMINAL_APPROACH: f64 = 20.0;
const PIXEL_EPSILON: f64 = 1.0;
const POINT_EPSILON: f64 = 1e-6;
const OVERLAP_PLAN_LIMIT: usize = 64;
const LANE_SEPARATION: f64 = 8.0;

#[derive(Clone, Copy, Debug)]
pub struct GridRoutingChild {
    pub bounds: Rect,
    /// Absolute left/top/right/bottom corridor coordinates of this child.
    pub corridors: Rect,
}

#[derive(Clone, Copy, Debug)]
pub struct GridRoutingView<'a> {
    /// Direct children in container-local slot order, including group shells. All coordinates
    /// are absolute. Edge-label helpers are kept for the separate label pass.
    pub children: &'a [GridRoutingChild],
    pub horizontal_corridors: &'a [f64],
    pub vertical_corridors: &'a [f64],
}

#[derive(Clone, Copy, Debug)]
pub struct GridPort<'a> {
    /// Index in the container's direct-child slice.
    pub child: usize,
    pub side: Side,
    /// The assigned tangential coordinate, in absolute drawing units.
    pub coordinate: f64,
    /// Legal ports from endpoint planning for this edge end.
    pub candidates: &'a [Point],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridSegment {
    start: Point,
    end: Point,
    axis: Axis,
}

impl GridSegment {
    pub fn new(start: Point, end: Point) -> Option<Self> {
        let axis = if start.y == end.y && start.x != end.x {
            Axis::Horizontal
        } else if start.x == end.x && start.y != end.y {
            Axis::Vertical
        } else {
            return None;
        };
        (finite(start) && finite(end)).then_some(Self { start, end, axis })
    }

    fn fixed(self) -> f64 {
        match self.axis {
            Axis::Horizontal => self.start.y,
            Axis::Vertical => self.start.x,
        }
    }

    fn span(self) -> (f64, f64) {
        let along = |p: Point| match self.axis {
            Axis::Horizontal => p.x,
            Axis::Vertical => p.y,
        };
        let (start, end) = (along(self.start), along(self.end));
        (start.min(end), start.max(end))
    }

    fn end_fixed(self) -> f64 {
        match self.axis {
            Axis::Horizontal => self.end.y,
            Axis::Vertical => self.end.x,
        }
    }
}

#[derive(Debug)]
pub struct GridCorridorRoute {
    pub points: Vec<Point>,
    /// Corridor-conflict segments with the 20px terminal approaches removed.
    pub occupancy: Vec<GridSegment>,
}

/// Tries the production compatibility path for an unbundled, uncoordinated sibling edge.
/// `occupied` contains earlier fast routes from this container and other endpoint pairs.
/// `total_plans` is the count for the diagram, including non-sibling edges.
pub fn route_grid_corridor(
    view: GridRoutingView<'_>,
    source: GridPort<'_>,
    target: GridPort<'_>,
    occupied: &[GridSegment],
    total_plans: usize,
    work: &OperationWorkMeter,
) -> Result<Option<GridCorridorRoute>> {
    work.checkpoint(OperationPhase::Layout)?;
    if source.child == target.child {
        return Ok(None);
    }
    let start = grid_item_attachment(view, source)?;
    let end = grid_item_attachment(view, target)?;
    let mut points = map_grid_corridor(view, start, end, 0, work)?;
    let [first, .., last] = points.as_slice() else {
        return Ok(None);
    };
    if !contains_port(source.candidates, *first, work)?
        || !contains_port(target.candidates, *last, work)?
    {
        return Ok(None);
    }
    let lower_bound = distance(*first, *last);
    let Some(mut occupancy) =
        validated_occupancy(&points, view, source.child, target.child, lower_bound, work)?
    else {
        return Ok(None);
    };

    if conflicts(&occupancy, occupied, work)? {
        if total_plans <= OVERLAP_PLAN_LIMIT {
            return Ok(None);
        }
        // routerCompatibility.ts separates large-graph fast routes with bounded interior nudges.
        let mut candidate = reserve_vec(points.len())?;
        'separate: for step in 1..=4 {
            for sign in [1.0, -1.0] {
                let delta = sign * f64::from(step) * LANE_SEPARATION;
                for index in 1..points.len().saturating_sub(2) {
                    work.charge(points.len())?;
                    candidate.clear();
                    candidate.extend_from_slice(&points);
                    for point in &mut candidate[index..=index + 1] {
                        if points[index].x == points[index + 1].x {
                            point.x += delta;
                        } else {
                            point.y += delta;
                        }
                    }
                    if let Some(separated) = validated_occupancy(
                        &candidate,
                        view,
                        source.child,
                        target.child,
                        lower_bound,
                        work,
                    )? && !conflicts(&separated, occupied, work)?
                    {
                        std::mem::swap(&mut points, &mut candidate);
                        occupancy = separated;
                        break 'separate;
                    }
                }
            }
        }
    }
    work.checkpoint(OperationPhase::Layout)?;
    Ok(Some(GridCorridorRoute { points, occupancy }))
}

#[derive(Clone, Copy)]
pub(super) struct GridAttachment {
    pub port: Point,
    pub connect: Point,
    pub corridor_axis: Axis,
}

pub(super) fn grid_item_attachment(
    view: GridRoutingView<'_>,
    port: GridPort<'_>,
) -> Result<GridAttachment> {
    let child = view.children.get(port.child).ok_or_else(invalid_geometry)?;
    let (face, corridor, corridor_axis) = match port.side {
        Side::Left => (child.bounds.x0, child.corridors.x0, Axis::Vertical),
        Side::Right => (child.bounds.x1, child.corridors.x1, Axis::Vertical),
        Side::Top => (child.bounds.y0, child.corridors.y0, Axis::Horizontal),
        Side::Bottom => (child.bounds.y1, child.corridors.y1, Axis::Horizontal),
    };
    let point = |coordinate| match corridor_axis {
        Axis::Vertical => Point::new(coordinate, port.coordinate),
        Axis::Horizontal => Point::new(port.coordinate, coordinate),
    };
    let attachment = GridAttachment {
        port: point(face),
        connect: point(corridor),
        corridor_axis,
    };
    if !finite(attachment.port) || !finite(attachment.connect) {
        return Err(invalid_geometry());
    }
    Ok(attachment)
}

/// Builds one compatibility corridor. The caller admits geometry before committing it.
pub(super) fn map_grid_corridor(
    view: GridRoutingView<'_>,
    start: GridAttachment,
    end: GridAttachment,
    variant: usize,
    work: &OperationWorkMeter,
) -> Result<Vec<Point>> {
    let mut points = reserve_vec(6)?;
    let (a, b) = (start.connect, end.connect);
    points.extend([start.port, a]);
    match (start.corridor_axis, end.corridor_axis) {
        (Axis::Vertical, Axis::Vertical) if (a.x - b.x).abs() > POINT_EPSILON => {
            let y = intermediate(view.horizontal_corridors, a.y, b.y, variant, None, work)?;
            points.extend([Point::new(a.x, y), Point::new(b.x, y)]);
        }
        (Axis::Vertical, Axis::Vertical) if (a.y - b.y).abs() > POINT_EPSILON && variant > 0 => {
            let x = intermediate(
                view.vertical_corridors,
                a.x,
                b.x,
                variant - 1,
                Some(a.x),
                work,
            )?;
            points.extend([Point::new(x, a.y), Point::new(x, b.y)]);
        }
        (Axis::Horizontal, Axis::Horizontal) if (a.y - b.y).abs() > POINT_EPSILON => {
            let x = intermediate(view.vertical_corridors, a.x, b.x, variant, None, work)?;
            points.extend([Point::new(x, a.y), Point::new(x, b.y)]);
        }
        (Axis::Vertical, Axis::Horizontal) => points.push(Point::new(a.x, b.y)),
        (Axis::Horizontal, Axis::Vertical) => points.push(Point::new(b.x, a.y)),
        _ => {}
    }
    points.extend([b, end.port]);
    points.dedup_by(|a, b| near(*a, *b, POINT_EPSILON));
    let segments = normalize(&points, PIXEL_EPSILON, work)?;
    if let Some(first) = segments.first() {
        points.clear();
        points.push(first.start);
        points.extend(segments.iter().map(|segment| segment.end));
    }
    Ok(points)
}

fn intermediate(
    corridors: &[f64],
    start: f64,
    end: f64,
    variant: usize,
    exclude: Option<f64>,
    work: &OperationWorkMeter,
) -> Result<f64> {
    if variant != 0 || exclude.is_some() {
        let mut candidates = reserve_vec(corridors.len())?;
        for &coordinate in corridors {
            work.charge(1)?;
            let distance = (coordinate - start).abs() + (coordinate - end).abs();
            if !distance.is_finite() {
                return Err(invalid_geometry());
            }
            if exclude.is_none_or(|excluded| (coordinate - excluded).abs() > POINT_EPSILON) {
                candidates.push((distance, coordinate));
            }
        }
        if candidates.is_empty() && exclude.is_some() {
            return intermediate(corridors, start, end, variant, None, work);
        }
        let sort_work = candidates
            .len()
            .checked_mul(candidates.len().checked_ilog2().unwrap_or_default() as usize + 1)
            .ok_or_else(|| work.arithmetic_overflow())?;
        work.charge(sort_work)?;
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.total_cmp(&b.1)));
        return Ok(candidates
            .get(variant.min(candidates.len().saturating_sub(1)))
            .map_or_else(|| (start + end) / 2.0, |(_, coordinate)| *coordinate));
    }
    // The compatibility fast path only selects the first coordinate.
    let mut best: Option<(f64, f64)> = None;
    for &value in corridors {
        work.charge(1)?;
        let cost = (value - start).abs() + (value - end).abs();
        if !cost.is_finite() {
            return Err(invalid_geometry());
        }
        if best.is_none_or(|(best_cost, best_value)| {
            cost < best_cost || (cost == best_cost && value < best_value)
        }) {
            best = Some((cost, value));
        }
    }
    Ok(best.map_or_else(|| (start + end) / 2.0, |(_, value)| value))
}

fn contains_port(candidates: &[Point], point: Point, work: &OperationWorkMeter) -> Result<bool> {
    for &candidate in candidates {
        work.charge(1)?;
        if candidate == point {
            return Ok(true);
        }
    }
    Ok(false)
}

fn validated_occupancy(
    points: &[Point],
    view: GridRoutingView<'_>,
    source: usize,
    target: usize,
    lower_bound: f64,
    work: &OperationWorkMeter,
) -> Result<Option<Vec<GridSegment>>> {
    let mut segments = reserve_vec(points.len().saturating_sub(1))?;
    let mut length = 0.0;
    for pair in points.windows(2) {
        work.charge(1)?;
        let Some(segment) = GridSegment::new(pair[0], pair[1]) else {
            return Ok(None);
        };
        let span = distance(segment.start, segment.end);
        // The fast-path output must already survive the one-pixel canonicalizer unchanged.
        if span <= PIXEL_EPSILON
            || segments
                .last()
                .is_some_and(|previous: &GridSegment| previous.axis == segment.axis)
        {
            return Ok(None);
        }
        length += span;
        segments.push(segment);
    }
    if !length.is_finite() || length != lower_bound || segments.is_empty() {
        return Ok(None);
    }
    let last = segments.len() - 1;
    if last > 0
        && [segments[0], segments[last]]
            .iter()
            .any(|s| distance(s.start, s.end) < TERMINAL_APPROACH)
    {
        return Ok(None);
    }
    for (child_index, child) in view.children.iter().enumerate() {
        work.charge(segments.len())?;
        let bounds = child.bounds;
        if !finite_rect(bounds) {
            return Err(invalid_geometry());
        }
        for (index, &segment) in segments.iter().enumerate() {
            let terminal =
                (child_index == source && index == 0) || (child_index == target && index == last);
            let rect = if terminal {
                bounds
            } else {
                bounds.inflate(ROUTE_CLEARANCE, ROUTE_CLEARANCE)
            };
            if crosses_rect(segment, rect, false) {
                return Ok(None);
            }
        }
    }
    if last == 0 && length <= TERMINAL_APPROACH * 2.0 {
        segments.clear();
        return Ok(Some(segments));
    }
    trim_approach(&mut segments[0], true);
    trim_approach(&mut segments[last], false);
    segments.retain(|segment| segment.start != segment.end);
    Ok(Some(segments))
}

fn trim_approach(segment: &mut GridSegment, start: bool) {
    let (port, other) = if start {
        (&mut segment.start, segment.end)
    } else {
        (&mut segment.end, segment.start)
    };
    let (coordinate, toward) = match segment.axis {
        Axis::Horizontal => (&mut port.x, other.x),
        Axis::Vertical => (&mut port.y, other.y),
    };
    *coordinate += (toward - *coordinate).signum() * TERMINAL_APPROACH;
}

fn conflicts(
    segments: &[GridSegment],
    occupied: &[GridSegment],
    work: &OperationWorkMeter,
) -> Result<bool> {
    for &segment in segments {
        let (low, high) = segment.span();
        for &other in occupied {
            work.charge(1)?;
            if segment.axis == other.axis
                && (segment.fixed() - other.fixed()).abs() < EDGE_CLEARANCE
            {
                let (other_low, other_high) = other.span();
                if high.min(other_high) > low.max(other_low) {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

fn normalize(
    points: &[Point],
    tolerance: f64,
    work: &OperationWorkMeter,
) -> Result<Vec<GridSegment>> {
    work.charge(points.len())?;
    let mut segments: Vec<GridSegment> = reserve_vec(points.len().saturating_sub(1))?;
    for pair in points.windows(2) {
        work.checkpoint(OperationPhase::Layout)?;
        let (a, b) = (pair[0], pair[1]);
        if !finite(a) || !finite(b) {
            return Err(invalid_geometry());
        }
        if near(a, b, tolerance) {
            continue;
        }
        let axis = if (a.y - b.y).abs() <= tolerance {
            Axis::Horizontal
        } else {
            Axis::Vertical
        };
        let current = GridSegment {
            start: a,
            end: b,
            axis,
        };
        if let Some(previous) = segments.last_mut()
            && previous.axis == axis
            && (previous.end_fixed() - current.fixed()).abs() <= tolerance
            && (previous.fixed() - current.fixed()).abs() <= tolerance
        {
            previous.end = b;
        } else {
            segments.push(current);
        }
    }
    Ok(segments)
}

fn crosses_rect(segment: GridSegment, rect: Rect, include_boundary: bool) -> bool {
    let (start, end) = segment.span();
    let (low, high, near, far) = match segment.axis {
        Axis::Horizontal => (rect.x0, rect.x1, rect.y0, rect.y1),
        Axis::Vertical => (rect.y0, rect.y1, rect.x0, rect.x1),
    };
    let fixed = segment.fixed();
    let within = if include_boundary {
        fixed >= near && fixed <= far
    } else {
        fixed > near && fixed < far
    };
    within && start.max(low) < end.min(high)
}

fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}

fn near(a: Point, b: Point, tolerance: f64) -> bool {
    (a.x - b.x).abs() <= tolerance && (a.y - b.y).abs() <= tolerance
}

fn finite(point: Point) -> bool {
    point.x.is_finite() && point.y.is_finite()
}

fn finite_rect(rect: Rect) -> bool {
    finite(Point::new(rect.x0, rect.y0))
        && finite(Point::new(rect.x1, rect.y1))
        && rect.x0 <= rect.x1
        && rect.y0 <= rect.y1
}

fn invalid_geometry() -> Error {
    Error::InvalidModel {
        message: "Invalid measured grid routing geometry".into(),
    }
}

fn allocation_failed() -> Error {
    Error::LayoutAllocationFailed { backend: "grid" }
}

fn reserve_vec<T>(capacity: usize) -> Result<Vec<T>> {
    let mut items = Vec::new();
    items
        .try_reserve_exact(capacity)
        .map_err(|_| allocation_failed())?;
    Ok(items)
}
