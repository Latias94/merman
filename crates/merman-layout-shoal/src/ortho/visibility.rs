mod grid;
mod sweep;

pub(super) use grid::{Grid, GridQuery, Node};

use super::{OrthoSegment, project, zero};
use crate::model::{Axis, Point, Rect};
use crate::work::{LayoutError, LayoutResult, Work, push, reserved};

#[derive(Clone, Copy, PartialEq)]
pub(super) struct Bounds {
    pub(super) x: [f64; 2],
    pub(super) y: [f64; 2],
}

impl Bounds {
    pub(super) fn new(rect: Rect) -> LayoutResult<Self> {
        let bounds = Self {
            x: [zero(rect.x), zero(rect.x + rect.width)],
            y: [zero(rect.y), zero(rect.y + rect.height)],
        };
        if rect.width < 0.0 || rect.height < 0.0 || !bounds.is_finite() {
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

    fn is_finite(self) -> bool {
        self.x.into_iter().chain(self.y).all(f64::is_finite)
    }

    fn union(self, other: Self) -> Self {
        Self {
            x: [self.x[0].min(other.x[0]), self.x[1].max(other.x[1])],
            y: [self.y[0].min(other.y[0]), self.y[1].max(other.y[1])],
        }
    }

    fn pad(self, margin: f64) -> Self {
        Self {
            x: [zero(self.x[0] - margin), zero(self.x[1] + margin)],
            y: [zero(self.y[0] - margin), zero(self.y[1] + margin)],
        }
    }
}

const LEAF_SIZE: usize = 8;

enum TreeKind {
    Leaf { first: usize, len: usize },
    Branch([usize; 2]),
}

struct TreeNode {
    bounds: Bounds,
    kind: TreeKind,
}

/// A static balanced box index for repeated visibility and channel queries.
pub(super) struct Obstacles {
    rects: Vec<Bounds>,
    tree: Vec<TreeNode>,
}

impl Obstacles {
    pub(super) fn new(obstacles: &[Rect], margin: f64, work: &mut Work<'_>) -> LayoutResult<Self> {
        work.charge(obstacles.len())?;
        let mut rects = reserved(obstacles.len())?;
        for &r in obstacles {
            work.visit(1)?;
            let r = Bounds::new(r)?.pad(margin);
            if !r.is_finite() {
                return Err(LayoutError::InvalidInput(
                    "orthogonal obstacle halo overflow",
                ));
            }
            if r.x[0] < r.x[1] && r.y[0] < r.y[1] {
                rects.push(r);
            }
        }
        let mut tree = Vec::new();
        if !rects.is_empty() {
            build_tree(&mut rects, 0, &mut tree, work)?;
        }
        Ok(Self { rects, tree })
    }

    pub(super) fn window(&self, start: Point, end: Point, margin: f64) -> Option<Bounds> {
        let mut b = Bounds {
            x: [start.x.min(end.x), start.x.max(end.x)],
            y: [start.y.min(end.y), start.y.max(end.y)],
        };
        if let Some(root) = self.tree.last() {
            b = b.union(root.bounds);
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
        bounds.is_finite().then_some(bounds)
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
        mut span: [f64; 2],
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<[f64; 2]>> {
        if let Some(root) = self.tree.len().checked_sub(1) {
            let mut run = FreeSpan {
                axis,
                at,
                across,
                span,
            };
            if !self.cut_span(root, &mut run, work)? {
                return Ok(None);
            }
            span = run.span;
        }
        Ok(Some(span))
    }

    fn cut_span(&self, id: usize, run: &mut FreeSpan, work: &mut Work<'_>) -> LayoutResult<bool> {
        work.visit(1)?;
        let node = &self.tree[id];
        if !run.touches(node.bounds) {
            return Ok(true);
        }
        match node.kind {
            TreeKind::Branch([left, right]) => {
                Ok(self.cut_span(left, run, work)? && self.cut_span(right, run, work)?)
            }
            TreeKind::Leaf { first, len } => {
                for &rect in &self.rects[first..first + len] {
                    work.visit(1)?;
                    if run.touches(rect) {
                        let ([lo, hi], _) = rect.project(run.axis);
                        if hi <= run.at {
                            run.span[0] = run.span[0].max(hi);
                        } else if run.at <= lo {
                            run.span[1] = run.span[1].min(lo);
                        } else {
                            return Ok(false);
                        }
                    }
                }
                Ok(true)
            }
        }
    }

    pub(super) fn corridors(&self, work: &mut Work<'_>) -> LayoutResult<Vec<OrthoSegment>> {
        sweep::corridors(&self.rects, work)
    }
}

struct FreeSpan {
    axis: Axis,
    at: f64,
    across: [f64; 2],
    span: [f64; 2],
}

impl FreeSpan {
    fn touches(&self, bounds: Bounds) -> bool {
        let ([lo, hi], [a, b]) = bounds.project(self.axis);
        a < self.across[1] && self.across[0] < b && lo < self.span[1] && self.span[0] < hi
    }
}

fn build_tree(
    rects: &mut [Bounds],
    first: usize,
    tree: &mut Vec<TreeNode>,
    work: &mut Work<'_>,
) -> LayoutResult<usize> {
    let mut bounds = rects[0];
    for &r in &rects[1..] {
        work.visit(1)?;
        bounds = bounds.union(r);
    }
    let kind = if rects.len() <= LEAF_SIZE {
        TreeKind::Leaf {
            first,
            len: rects.len(),
        }
    } else {
        let axis = if bounds.x[1] - bounds.x[0] >= bounds.y[1] - bounds.y[0] {
            Axis::Horizontal
        } else {
            Axis::Vertical
        };
        let midpoint = rects.len() / 2;
        work.charge(rects.len())?;
        rects.select_nth_unstable_by(midpoint, |a, b| {
            let (a, _) = a.project(axis);
            let (b, _) = b.project(axis);
            a[0].midpoint(a[1]).total_cmp(&b[0].midpoint(b[1]))
        });
        work.check()?;
        let (left, right) = rects.split_at_mut(midpoint);
        TreeKind::Branch([
            build_tree(left, first, tree, work)?,
            build_tree(right, first + midpoint, tree, work)?,
        ])
    };
    let id = tree.len();
    push(tree, TreeNode { bounds, kind })?;
    Ok(id)
}
