//! The `outline-orthogonal` paint projection in Mermaid's orthogonalEdgeClipping.js.

use super::{POINT_EPSILON, finite, invalid_geometry, near, reserve_vec};
use crate::Result;
use crate::graph_layout::Size;
use crate::resources::OperationWorkMeter;
use kurbo::Point;
use merman_model::OperationPhase;
use std::borrow::Cow;

#[derive(Clone, Copy, Debug)]
pub struct GridOutlineQuery {
    pub adjacent: Point,
    /// Origin on the router's terminal axis, on the far side of the outline from `adjacent`.
    pub line_origin: Option<Point>,
}

#[derive(Clone, Copy)]
pub struct GridOutline<'a> {
    pub center: Option<Point>,
    pub size: Option<Size>,
    pub intersect: &'a dyn Fn(GridOutlineQuery) -> Result<Option<Point>>,
}

#[derive(Clone, Copy)]
struct ClippedEnd {
    outline: Point,
    elbow: Point,
}

/// Query both endpoint outlines independently. Borrow the route if both ports stay unchanged.
/// Return the points for SVG projection before marker offset. Labels keep the layout route.
pub fn clip_grid_endpoints<'a>(
    points: &'a [Point],
    source: Option<GridOutline<'_>>,
    target: Option<GridOutline<'_>>,
    work: &OperationWorkMeter,
) -> Result<Cow<'a, [Point]>> {
    work.checkpoint(OperationPhase::Emit)?;
    let [first, second, ..] = points else {
        return Ok(Cow::Borrowed(points));
    };
    let penultimate = points[points.len() - 2];
    let last = points[points.len() - 1];
    let source = clip_end(source, *first, *second, work)?;
    let target = clip_end(target, last, penultimate, work)?;
    if source.is_none() && target.is_none() {
        return Ok(Cow::Borrowed(points));
    }

    work.charge_emit_work(points.len())?;
    let capacity = points
        .len()
        .checked_add(2)
        .ok_or_else(|| work.arithmetic_overflow())?;
    let mut clipped = reserve_vec(capacity)?;
    if points.len() == 2 {
        let start = source.map_or(*first, |end| end.outline);
        let end = target.map_or(last, |end| end.outline);
        append_distinct(&mut clipped, start);
        // Preserve the midpoint subdivision used by the shared edge painter.
        let (before, after) = if (first.x - last.x).abs() <= POINT_EPSILON {
            let middle = (start.y + end.y) / 2.0;
            (Point::new(start.x, middle), Point::new(end.x, middle))
        } else {
            let middle = (start.x + end.x) / 2.0;
            (Point::new(middle, start.y), Point::new(middle, end.y))
        };
        if !finite(before) || !finite(after) {
            return Err(invalid_geometry());
        }
        append_distinct(&mut clipped, before);
        append_distinct(&mut clipped, after);
        append_distinct(&mut clipped, end);
    } else {
        if let Some(source) = source {
            append_distinct(&mut clipped, source.outline);
            append_distinct(&mut clipped, source.elbow);
        } else {
            append_distinct(&mut clipped, *first);
        }
        for &point in &points[1..points.len() - 1] {
            work.checkpoint(OperationPhase::Emit)?;
            append_distinct(&mut clipped, point);
        }
        if let Some(target) = target {
            append_distinct(&mut clipped, target.elbow);
            append_distinct(&mut clipped, target.outline);
        } else {
            append_distinct(&mut clipped, last);
        }
    }
    work.checkpoint(OperationPhase::Emit)?;
    Ok(Cow::Owned(clipped))
}

fn clip_end(
    shape: Option<GridOutline<'_>>,
    port: Point,
    adjacent: Point,
    work: &OperationWorkMeter,
) -> Result<Option<ClippedEnd>> {
    let Some(shape) = shape else { return Ok(None) };
    let vertical = (port.x - adjacent.x).abs() <= POINT_EPSILON;
    let horizontal = (port.y - adjacent.y).abs() <= POINT_EPSILON;
    if !finite(port) || !finite(adjacent) || (!vertical && !horizontal) {
        return Ok(None);
    }
    let delta = if vertical {
        port.y - adjacent.y
    } else {
        port.x - adjacent.x
    };
    if delta == 0.0 {
        return Ok(None);
    }
    let inward = delta.signum();
    let line_origin = shape
        .size
        .filter(|size| {
            size.width.is_finite()
                && size.height.is_finite()
                && size.width > 0.0
                && size.height > 0.0
        })
        .map(|size| {
            let extend = (if vertical { size.height } else { size.width }) + delta.abs() + 1.0;
            if vertical {
                Point::new(port.x, port.y + inward * extend)
            } else {
                Point::new(port.x + inward * extend, port.y)
            }
        })
        .filter(|&origin| finite(origin));
    work.charge_emit_work(1)?;
    let outline = (shape.intersect)(GridOutlineQuery {
        adjacent,
        line_origin,
    })?;
    work.checkpoint(OperationPhase::Emit)?;
    let Some(outline) = outline else {
        return Ok(None);
    };
    if !finite(outline)
        || shape
            .center
            .is_some_and(|center| near(center, outline, POINT_EPSILON))
        || (vertical && (outline.x - port.x).abs() > POINT_EPSILON)
        || (horizontal && (outline.y - port.y).abs() > POINT_EPSILON)
    {
        return Ok(None);
    }
    let (depth, elbow) = if vertical {
        (outline.y - port.y, Point::new(outline.x, adjacent.y))
    } else {
        (outline.x - port.x, Point::new(adjacent.x, outline.y))
    };
    Ok((depth * inward > POINT_EPSILON).then_some(ClippedEnd { outline, elbow }))
}

fn append_distinct(points: &mut Vec<Point>, point: Point) {
    if points
        .last()
        .is_none_or(|&previous| !near(previous, point, POINT_EPSILON))
    {
        points.push(point);
    }
}
