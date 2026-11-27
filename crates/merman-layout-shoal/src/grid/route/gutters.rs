//! Connect fixed lead ends through the measured placement grid when visibility is exhausted.

use super::*;
use crate::geom::BoundsIndex;

#[derive(Default)]
pub(super) struct LaneUse(BoundsIndex);

impl LaneUse {
    pub(super) fn record(
        &mut self,
        points: &[RoutePoint],
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        let mut previous: Option<OrthoSegment> = None;
        for leg in points.windows(2) {
            work.visit(1)?;
            let segment = OrthoSegment::between(leg[0], leg[1]);
            if let (Some(previous), Some(segment)) = (&mut previous, segment)
                && previous.axis == segment.axis
                && previous.fixed == segment.fixed
            {
                previous.min = previous.min.min(segment.min);
                previous.max = previous.max.max(segment.max);
            } else if let Some(segment) = std::mem::replace(&mut previous, segment) {
                self.0.insert_with_work(lane_bounds(segment), work)?;
            }
        }
        if let Some(segment) = previous {
            self.0.insert_with_work(lane_bounds(segment), work)?;
        }
        Ok(())
    }

    pub(super) fn vacant(&self, path: &[RoutePoint], work: &mut Work<'_>) -> LayoutResult<bool> {
        for leg in path.windows(2) {
            work.visit(1)?;
            let Some(segment) = OrthoSegment::between(leg[0], leg[1]) else {
                continue;
            };
            let query = lane_bounds(segment);
            if self
                .0
                .visit_overlaps_with_work(query, work, |occupied, _| {
                    Ok(match segment.axis {
                        Axis::Horizontal => {
                            occupied.y == query.y
                                && query.x[0] < occupied.x[1]
                                && occupied.x[0] < query.x[1]
                        }
                        Axis::Vertical => {
                            occupied.x == query.x
                                && query.y[0] < occupied.y[1]
                                && occupied.y[0] < query.y[1]
                        }
                    })
                })?
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn lane_bounds(segment: OrthoSegment) -> Bounds {
    let (along, cross) = ([segment.min, segment.max], [segment.fixed; 2]);
    match segment.axis {
        Axis::Horizontal => Bounds { x: along, y: cross },
        Axis::Vertical => Bounds { x: cross, y: along },
    }
}

/// Card placement already supplies clear lanes between its rows and columns. Try a straight
/// approach or an elbow matching the endpoint normals before using those measured lanes.
pub(super) fn card_channel(
    router: &OrthoRouter,
    [start, end]: [(RoutePoint, Side); 2],
    bounds: Rect,
    gutters: [&[f64]; 2],
    used: &LaneUse,
    pitch: f64,
    work: &mut Work<'_>,
) -> LayoutResult<Option<Vec<RoutePoint>>> {
    let [a, b] = [start.0, end.0];
    let sides = [start.1, end.1];
    if position(a) == position(b) {
        return Ok(None);
    }
    let vacant = |path: &[RoutePoint], work: &mut Work<'_>| -> LayoutResult<bool> {
        Ok(used.vacant(path, work)? && clear(router, path, bounds, sides, work)?)
    };
    let axis = start.1.axis();
    if a.x == b.x || a.y == b.y {
        let path = [a, b];
        if vacant(&path, work)? {
            return Ok(Some(copy_points(&path)?));
        }
    } else if axis != end.1.axis() {
        let turn = match axis {
            Axis::Horizontal => Point { x: b.x, y: a.y },
            Axis::Vertical => Point { x: a.x, y: b.y },
        };
        let path = [a, knot(turn, axis), b];
        if vacant(&path, work)? {
            return Ok(Some(copy_points(&path)?));
        }
    } else if start.1 == opposite(end.1) {
        let track = match axis {
            Axis::Horizontal => a.x.midpoint(b.x),
            Axis::Vertical => a.y.midpoint(b.y),
        };
        let path = dogleg(a, b, axis, track);
        if vacant(&path, work)? {
            return Ok(Some(copy_points(&path)?));
        }
    }
    let lines = Lines::new(gutters, bounds);
    // A shared gutter can accommodate parallel routes on nearby tracks. Check the shifted
    // geometry against the same walls and record actual shaft intervals, not lane IDs.
    for offset in [0.0, 0.5, -0.5, 1.0, -1.0, 1.5, -1.5, 2.0, -2.0] {
        work.visit(1)?;
        if let Some(path) =
            trace_gutter(router, [start, end], bounds, &lines, offset * pitch, work)?
            && used.vacant(&path, work)?
        {
            return Ok(Some(path));
        }
        if pitch == 0.0 {
            break;
        }
    }
    Ok(None)
}

struct Lines<'a> {
    axes: [&'a [f64]; 2],
    bounds: [[f64; 2]; 2],
}

impl Lines<'_> {
    fn new(axes: [&[f64]; 2], bounds: Rect) -> Lines<'_> {
        let bounds = Bounds::of(bounds);
        Lines {
            axes,
            bounds: [bounds.x, bounds.y],
        }
    }

    fn search(&self, axis: usize, at: f64, work: &mut Work<'_>) -> LayoutResult<usize> {
        let lines = self.axes[axis];
        work.visit(lines.len().checked_ilog2().unwrap_or_default() as usize + 1)?;
        Ok(lines.partition_point(|&line| line < at))
    }

    fn outward(
        &self,
        at: Point,
        side: Side,
        shift: f64,
        work: &mut Work<'_>,
    ) -> LayoutResult<[Option<f64>; 4]> {
        let (axis, sign) = normal(side);
        let v = [at.x, at.y][axis];
        let list = self.axes[axis];
        let i = self.search(axis, v, work)?;
        let candidate = |offset| {
            let slot = if sign > 0 {
                i.checked_add(offset)
            } else {
                i.checked_sub(offset + 1)
            }?;
            list.get(slot).map(|line| line + shift).filter(|line| {
                let [lo, hi] = self.bounds[axis];
                lo <= *line && *line <= hi
            })
        };
        Ok([
            candidate(0),
            candidate(1),
            Some(v),
            Some(self.bounds[axis][usize::from(sign > 0)]),
        ])
    }

    fn across(
        &self,
        axis: usize,
        at: f64,
        shift: f64,
        work: &mut Work<'_>,
    ) -> LayoutResult<[Option<f64>; 4]> {
        let i = self.search(axis, at, work)?;
        let [lo, hi] = self.bounds[axis];
        let list = self.axes[axis];
        let get = |k| {
            list.get(k)
                .map(|line| line + shift)
                .filter(|line| lo <= *line && *line <= hi)
        };
        let before = i.checked_sub(1).and_then(get);
        let after = get(i);
        let [a, b] = match (before, after) {
            (Some(a), Some(b)) if b - at < at - a => [Some(b), Some(a)],
            _ => [before, after],
        };
        Ok([a, b, Some(lo), Some(hi)])
    }
}

/// Tries the neighboring free tracks and scope perimeter in deterministic order. The input
/// gutters are sorted drawing coordinates on x and y. Every emitted leg is checked in the
/// same obstacle index used by visibility routing.
pub(crate) fn gutter_route(
    router: &OrthoRouter,
    [start, end]: [(RoutePoint, Side); 2],
    bounds: Rect,
    gutters: [&[f64]; 2],
    work: &mut Work<'_>,
) -> LayoutResult<Option<Vec<RoutePoint>>> {
    trace_gutter(
        router,
        [start, end],
        bounds,
        &Lines::new(gutters, bounds),
        0.0,
        work,
    )
}

fn trace_gutter(
    router: &OrthoRouter,
    [start, end]: [(RoutePoint, Side); 2],
    bounds: Rect,
    lines: &Lines<'_>,
    shift: f64,
    work: &mut Work<'_>,
) -> LayoutResult<Option<Vec<RoutePoint>>> {
    let a = [start.0.x, start.0.y];
    let b = [end.0.x, end.0.y];
    let (axis_a, _) = normal(start.1);
    let (axis_b, _) = normal(end.1);
    let first = lines.outward(position(start.0), start.1, shift, work)?;
    let last = lines.outward(position(end.0), end.1, shift, work)?;
    let cross = 1 - axis_a;
    let across = if axis_a == axis_b {
        lines.across(cross, a[cross].midpoint(b[cross]), shift, work)?
    } else {
        [None; 4]
    };
    let sides = [start.1, end.1];
    let knot = |coords| super::knot(point(coords), Axis::Horizontal);
    for from in first.into_iter().flatten() {
        let mut p = a;
        p[axis_a] = from;
        for to in last.into_iter().flatten() {
            work.visit(1)?;
            let mut q = b;
            q[axis_b] = to;
            if axis_a != axis_b {
                let mut turn = p;
                turn[axis_b] = to;
                let candidate = [start.0, knot(p), knot(turn), knot(q), end.0];
                if clear(router, &candidate, bounds, sides, work)? {
                    return Ok(Some(super::copy_points(&candidate)?));
                }
                continue;
            }
            for track in across.into_iter().flatten() {
                let (mut u, mut v) = (p, q);
                u[cross] = track;
                v[cross] = track;
                let candidate = [start.0, knot(p), knot(u), knot(v), knot(q), end.0];
                if clear(router, &candidate, bounds, sides, work)? {
                    return Ok(Some(super::copy_points(&candidate)?));
                }
            }
        }
    }
    Ok(None)
}
