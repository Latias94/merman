mod grid;
mod sweep;

pub(super) use crate::geom::Bounds;
pub(super) use grid::{Grid, GridQuery, Node};

use super::{OrthoSegment, project, zero};
use crate::geom::{Bvh, BvhEntry};
use crate::model::{Axis, Point, Rect};
use crate::work::{LayoutError, LayoutResult, Work, reserved};
use std::ops::ControlFlow::{Break, Continue};

impl Bounds {
    pub(super) fn new(rect: Rect) -> LayoutResult<Self> {
        let bounds = Self::of(rect);
        let bounds = Self {
            x: bounds.x.map(zero),
            y: bounds.y.map(zero),
        };
        if rect.width < 0.0 || rect.height < 0.0 || !bounds.finite() {
            return Err(LayoutError::InvalidInput(
                "invalid orthogonal routing rectangle",
            ));
        }
        Ok(bounds)
    }

    pub(super) fn contains(self, point: Point) -> bool {
        self.x[0] <= point.x && point.x <= self.x[1] && self.y[0] <= point.y && point.y <= self.y[1]
    }

    pub(super) fn project(self, axis: Axis) -> ([f64; 2], [f64; 2]) {
        match axis {
            Axis::Horizontal => (self.x, self.y),
            Axis::Vertical => (self.y, self.x),
        }
    }

    fn pad(self, margin: f64) -> Self {
        Self {
            x: [zero(self.x[0] - margin), zero(self.x[1] + margin)],
            y: [zero(self.y[0] - margin), zero(self.y[1] + margin)],
        }
    }
}

/// A static balanced box index for repeated visibility and channel queries.
pub(super) struct Obstacles {
    rects: Bvh<Bounds>,
}

impl Obstacles {
    pub(super) fn new(obstacles: &[Rect], margin: f64, work: &mut Work<'_>) -> LayoutResult<Self> {
        work.charge(obstacles.len())?;
        let mut rects = reserved(obstacles.len())?;
        for &r in obstacles {
            work.visit(1)?;
            let r = Bounds::new(r)?.pad(margin);
            if !r.finite() {
                return Err(LayoutError::InvalidInput(
                    "orthogonal obstacle halo overflow",
                ));
            }
            if r.has_area() {
                rects.push(r);
            }
        }
        Ok(Self {
            rects: Bvh::from_bounds(rects, work)?,
        })
    }

    pub(super) fn window(&self, start: Point, end: Point, margin: f64) -> Option<Bounds> {
        let mut b = Bounds::between(start, end);
        if let Some(root) = self.rects.root_bounds() {
            b = b.union(root);
        }
        let outer = b.pad(margin);
        // Even a margin smaller than an ulp should make room on the outside.
        let x = [
            outer.x[0].min(b.x[0].next_down()),
            outer.x[1].max(b.x[1].next_up()),
        ];
        let y = [
            outer.y[0].min(b.y[0].next_down()),
            outer.y[1].max(b.y[1].next_up()),
        ];
        let bounds = Bounds { x, y };
        bounds.finite().then_some(bounds)
    }

    pub(super) fn rays(
        &self,
        point: Point,
        bounds: Bounds,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<[OrthoSegment; 2]>> {
        let Some(h) = self.ray(Axis::Horizontal, point, bounds, work)? else {
            return Ok(None);
        };
        let Some(v) = self.ray(Axis::Vertical, point, bounds, work)? else {
            return Ok(None);
        };
        Ok(Some([h, v]))
    }

    pub(super) fn ray(
        &self,
        axis: Axis,
        point: Point,
        bounds: Bounds,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<OrthoSegment>> {
        let (along, fixed) = project(axis, point);
        Ok(self
            .free_span(axis, along, [fixed; 2], bounds.project(axis).0, work)?
            .map(|[min, max]| OrthoSegment {
                axis,
                fixed,
                min,
                max,
            }))
    }

    pub(super) fn free_span(
        &self,
        axis: Axis,
        at: f64,
        across: [f64; 2],
        span: [f64; 2],
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<[f64; 2]>> {
        let mut run = FreeSpan { axis, across, span };
        let result = self.rects.visit(|entry| -> LayoutResult<_> {
            work.visit(1)?;
            Ok(match entry {
                BvhEntry::Node(bounds) => Continue(run.touches(bounds)),
                BvhEntry::Item(&rect) if run.touches(rect) => {
                    let ([lo, hi], _) = rect.project(axis);
                    if hi <= at {
                        run.span[0] = run.span[0].max(hi);
                    } else if at <= lo {
                        run.span[1] = run.span[1].min(lo);
                    } else {
                        return Ok(Break(()));
                    }
                    Continue(true)
                }
                BvhEntry::Item(_) => Continue(true),
            })
        })?;
        Ok(result.is_continue().then_some(run.span))
    }

    pub(super) fn corridors(&self, work: &mut Work<'_>) -> LayoutResult<Vec<OrthoSegment>> {
        sweep::corridors(self.rects.items(), work)
    }
}

struct FreeSpan {
    axis: Axis,
    across: [f64; 2],
    span: [f64; 2],
}

impl FreeSpan {
    fn touches(&self, bounds: Bounds) -> bool {
        let ([lo, hi], [a, b]) = bounds.project(self.axis);
        a < self.across[1] && self.across[0] < b && lo < self.span[1] && self.span[0] < hi
    }
}
