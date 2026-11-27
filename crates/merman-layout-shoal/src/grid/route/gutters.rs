//! Connect fixed lead ends through the measured placement grid when visibility is exhausted.

use super::*;

struct Lines<'a> {
    axes: [&'a [f64]; 2],
    bounds: [[f64; 2]; 2],
}

impl Lines<'_> {
    fn search(&self, axis: usize, at: f64, work: &mut Work<'_>) -> LayoutResult<usize> {
        let lines = self.axes[axis];
        work.visit(lines.len().checked_ilog2().unwrap_or_default() as usize + 1)?;
        Ok(lines.partition_point(|&line| line < at))
    }

    fn outward(
        &self,
        at: Point,
        side: Side,
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
            list.get(slot).copied().filter(|line| {
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

    fn across(&self, axis: usize, at: f64, work: &mut Work<'_>) -> LayoutResult<[Option<f64>; 4]> {
        let i = self.search(axis, at, work)?;
        let [lo, hi] = self.bounds[axis];
        let list = self.axes[axis];
        let get = |k| {
            list.get(k)
                .copied()
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
    let bounds_at = Bounds::of(bounds);
    let lines = Lines {
        axes: gutters,
        bounds: [bounds_at.x, bounds_at.y],
    };
    let a = [start.0.x, start.0.y];
    let b = [end.0.x, end.0.y];
    let (axis_a, _) = normal(start.1);
    let (axis_b, _) = normal(end.1);
    let first = lines.outward(position(start.0), start.1, work)?;
    let last = lines.outward(position(end.0), end.1, work)?;
    let cross = 1 - axis_a;
    let across = if axis_a == axis_b {
        lines.across(cross, a[cross].midpoint(b[cross]), work)?
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
