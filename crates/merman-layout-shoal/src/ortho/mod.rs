//! Bounded orthogonal routing between fixed lead ends in drawing coordinates.

mod search;
mod visibility;

use crate::model::{Axis, Point, Rect, RoutePoint, Side};
use crate::work::{LayoutError, LayoutResult, Work, WorkControl, WorkError, reserved};
use search::{Search, Trails};
use std::cmp::Ordering;
use visibility::{Bounds, Grid, GridQuery, Obstacles};

pub(crate) const DEFAULT_MAX_ORTHO_GRID_NODES: usize = 4_096;

/// One span of an already placed drawing. The caller reserves the terminal
/// leads. Each side faces away from its reserved lead, toward the inner span.
#[derive(Clone, Copy, Debug)]
pub struct OrthogonalSpan {
    /// Fixed inner knot and outward side of the source lead.
    pub start: (RoutePoint, Side),
    /// Fixed inner knot and outward side of the target lead.
    pub end: (RoutePoint, Side),
}

/// Routes fixed spans in drawing coordinates with one obstacle index.
///
/// Obstacles receive half the edge separation on each side. `bounds`, when
/// supplied, is an inclusive limit for all spans. Each search admits at most
/// 4,096 visibility vertices, and returns `None` in place if it exceeds that
/// cap or finds no route. Successful paths include both fixed knots and affect
/// later spans' crossing and overlap cost. Cancellation and work exhaustion
/// abort the batch with an error.
pub fn route_orthogonal_spans(
    obstacles: &[Rect],
    bounds: Option<Rect>,
    edge_sep: f64,
    spans: &[OrthogonalSpan],
    control: &mut impl WorkControl,
) -> Result<Vec<Option<Vec<RoutePoint>>>, LayoutError> {
    let mut work = Work::new(control);
    work.check()?;
    if spans.is_empty() {
        return Ok(Vec::new());
    }
    work.charge(spans.len())?;
    let mut routes = reserved(spans.len())?;
    let mut router =
        OrthoRouter::new(obstacles, edge_sep, DEFAULT_MAX_ORTHO_GRID_NODES, &mut work)?;
    for &span in spans {
        let path = router.route(
            OrthoRequest {
                start: span.start.0,
                end: span.end.0,
                start_side: Some(span.start.1),
                end_side: Some(span.end.1),
                bounds,
            },
            &mut work,
        )?;
        if let Some(points) = &path {
            router.record_route(points, &mut work)?;
        }
        routes.push(path);
    }
    work.finish()?;
    Ok(routes)
}

/// A single span between two fixed knots. A supplied side points from its knot
/// away from the reserved lead. The span can continue outward or turn at the knot.
/// Bounds, when provided, include their boundary and constrain the whole span.
#[derive(Clone, Copy)]
pub(crate) struct OrthoRequest {
    pub start: RoutePoint,
    pub end: RoutePoint,
    pub start_side: Option<Side>,
    pub end_side: Option<Side>,
    pub bounds: Option<Rect>,
}

/// An occupied or candidate centerline interval. `fixed` is y for a horizontal
/// interval and x for a vertical one. `min` and `max` are along the travel axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct OrthoSegment {
    pub axis: Axis,
    pub fixed: f64,
    pub min: f64,
    pub max: f64,
}

impl OrthoSegment {
    pub(crate) fn between(a: RoutePoint, b: RoutePoint) -> Option<Self> {
        Self::from_points(at(a), at(b))
    }

    fn from_points(a: Point, b: Point) -> Option<Self> {
        let axis = match (a.x == b.x, a.y == b.y) {
            (false, true) => Axis::Horizontal,
            (true, false) => Axis::Vertical,
            _ => return None,
        };
        let (a, fixed) = project(axis, a);
        let (b, _) = project(axis, b);
        let (min, max) = (a.min(b), a.max(b));
        (min < max && [min, max, fixed].into_iter().all(f64::is_finite)).then_some(Self {
            axis,
            fixed,
            min,
            max,
        })
    }
}

/// Reuses the obstacle index, visibility corridors, and query buffers within one drawing.
pub(crate) struct OrthoRouter {
    obstacles: Obstacles,
    exterior_pad: f64,
    max_ortho_grid_nodes: usize,
    corridors: Option<Vec<OrthoSegment>>,
    grid: Grid,
    search: Search,
    trails: Trails,
}

impl OrthoRouter {
    /// Obstacles are inflated by `edge_sep / 2`. A route may touch the inflated
    /// boundary and must stay outside the open interior.
    pub(crate) fn new(
        obstacles: &[Rect],
        edge_sep: f64,
        max_ortho_grid_nodes: usize,
        work: &mut Work<'_>,
    ) -> LayoutResult<Self> {
        work.check()?;
        if !edge_sep.is_finite() || edge_sep < 0.0 {
            return Err(LayoutError::InvalidInput(
                "invalid orthogonal edge separation",
            ));
        }
        let margin = edge_sep * 0.5;
        let obstacles = Obstacles::new(obstacles, margin, work)?;
        work.finish()?;
        Ok(Self {
            obstacles,
            exterior_pad: margin.max(1.0),
            max_ortho_grid_nodes,
            corridors: None,
            grid: Grid::default(),
            search: Search::default(),
            trails: Trails::default(),
        })
    }

    /// Returns the complete inner span, including both unchanged endpoint knots.
    /// The caller can retain its previous route if no span fits the walls or vertex cap.
    pub(crate) fn route(
        &mut self,
        request: OrthoRequest,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Vec<RoutePoint>>> {
        work.check()?;
        let result = self.route_inner(request, work)?;
        work.finish()?;
        Ok(result)
    }

    fn route_inner(
        &mut self,
        request: OrthoRequest,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Vec<RoutePoint>>> {
        let cap = self.max_ortho_grid_nodes;
        let start = at(request.start);
        let end = at(request.end);
        let min_vertices = 1 + usize::from(start.x != end.x) + usize::from(start.y != end.y);
        if cap < min_vertices || !search::distance(start, end).is_finite() {
            return Ok(None);
        }
        let explicit = request.bounds.map(Bounds::new).transpose()?;
        let bounds = explicit.or_else(|| self.obstacles.window(start, end, self.exterior_pad));
        let Some(bounds) = bounds.filter(|b| b.contains(start) && b.contains(end)) else {
            return Ok(None);
        };
        let Some(start_rays) = self.obstacles.rays(start, bounds, work)? else {
            return Ok(None);
        };
        if start == end {
            let mut path = reserved(2)?;
            path.push(request.start);
            if request.start != request.end {
                path.push(request.end);
            }
            return Ok(Some(path));
        }
        let Some(end_rays) = self.obstacles.rays(end, bounds, work)? else {
            return Ok(None);
        };
        let quick = search::quick(request, [start_rays, end_rays], &self.trails, work)?;
        if let Some(path) = quick
            .as_ref()
            .filter(|path| path.reaches_lower_bound(request))
        {
            return Ok(Some(path.emit(request)?));
        }
        if self.corridors.is_none() {
            self.corridors = Some(self.obstacles.corridors(work)?);
        }
        let Some(terminals) = self.grid.build(
            self.corridors.as_deref().unwrap_or_default(),
            &self.obstacles,
            GridQuery {
                bounds,
                guarded: explicit.is_some(),
                anchors: [start, end],
                rays: [start_rays, end_rays],
                cap,
            },
            work,
        )?
        else {
            return Ok(None);
        };
        if let Some(path) = self.search.route(
            &self.grid.nodes,
            terminals,
            request,
            &self.trails,
            quick.as_ref(),
            work,
        )? {
            return Ok(Some(path));
        }
        quick.map(|path| path.emit(request)).transpose()
    }

    /// Records the orthogonal legs of one accepted edge. Call after stitching
    /// its spans to account for the whole edge together.
    pub(crate) fn record_route(
        &mut self,
        points: &[RoutePoint],
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        self.trails.record(points, work)
    }

    /// Returns the inclusive cross-axis range through which the entire segment
    /// can slide without entering a halo. Unbounded sides are infinite if no
    /// drawing bounds constrain them. The caller also checks the adjoining legs.
    pub(crate) fn channel(
        &self,
        segment: OrthoSegment,
        bounds: Option<Rect>,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<[f64; 2]>> {
        work.check()?;
        let motion = other_axis(segment.axis);
        let across = [segment.min, segment.max];
        let span = [f64::NEG_INFINITY, f64::INFINITY];
        let result = if ![segment.min, segment.max, segment.fixed]
            .into_iter()
            .all(f64::is_finite)
            || segment.min >= segment.max
        {
            None
        } else if let Some(bounds) = bounds.map(Bounds::new).transpose()? {
            let (slide, extent) = bounds.project(motion);
            if across[0] < extent[0]
                || across[1] > extent[1]
                || segment.fixed < slide[0]
                || segment.fixed > slide[1]
            {
                None
            } else {
                self.obstacles
                    .free_span(motion, segment.fixed, across, slide, work)?
            }
        } else {
            self.obstacles
                .free_span(motion, segment.fixed, across, span, work)?
        };
        work.finish()?;
        Ok(result)
    }
}

fn at(knot: RoutePoint) -> Point {
    Point {
        x: zero(knot.x),
        y: zero(knot.y),
    }
}

fn zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn project(axis: Axis, point: Point) -> (f64, f64) {
    match axis {
        Axis::Horizontal => (point.x, point.y),
        Axis::Vertical => (point.y, point.x),
    }
}

fn axis_id(axis: Axis) -> usize {
    usize::from(axis == Axis::Vertical)
}

fn other_axis(axis: Axis) -> Axis {
    match axis {
        Axis::Horizontal => Axis::Vertical,
        Axis::Vertical => Axis::Horizontal,
    }
}

fn sort<T>(
    values: &mut [T],
    cmp: impl Fn(&T, &T) -> Ordering,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    if values.len() > 1 {
        let units = values
            .len()
            .checked_mul(values.len().ilog2() as usize + 1)
            .ok_or(WorkError::ArithmeticOverflow)?;
        work.charge(units)?;
        values.sort_unstable_by(cmp);
        work.check()?;
    }
    Ok(())
}

fn reset<T: Clone>(
    values: &mut Vec<T>,
    len: usize,
    value: T,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    work.charge(len)?;
    values.clear();
    values
        .try_reserve(len)
        .map_err(|_| LayoutError::AllocationLimit)?;
    values.resize(len, value);
    Ok(())
}

#[cfg(test)]
mod tests;
