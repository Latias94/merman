use crate::work::{LayoutResult, Work, push, reserved};
use crate::{MarkerMetrics, Point, Rect, RoutePoint, RoutingStyle, Side, Size};

const LEAF_SIZE: usize = 8;
const MAX_WALLS: usize = 16 * 1024;
const MAX_KNOTS: usize = 2 * MAX_WALLS;
const MAX_CHECKS: usize = 128 * 1024;

#[derive(Clone, Copy)]
pub(super) struct Bounds {
    pub(super) x: [f64; 2],
    pub(super) y: [f64; 2],
}

impl Bounds {
    pub(super) fn of(rect: Rect) -> Self {
        Self {
            x: [rect.x, rect.x + rect.width],
            y: [rect.y, rect.y + rect.height],
        }
    }

    pub(super) fn label(at: Point, size: Size, pad: f64) -> Self {
        let (w, h) = (size.width * 0.5 + pad, size.height * 0.5 + pad);
        Self {
            x: [at.x - w, at.x + w],
            y: [at.y - h, at.y + h],
        }
    }

    fn marker(at: Point, side: Side, marker: MarkerMetrics) -> Self {
        let Point { x, y } = at;
        let half = marker.width * 0.5;
        let length = marker.length;
        let (x, y) = match side {
            Side::Top => ([x - half, x + half], [y - length, y]),
            Side::Bottom => ([x - half, x + half], [y, y + length]),
            Side::Left => ([x - length, x], [y - half, y + half]),
            Side::Right => ([x, x + length], [y - half, y + half]),
        };
        Self { x, y }
    }

    fn span(from: Point, to: Point) -> Self {
        Self {
            x: [from.x.min(to.x), from.x.max(to.x)],
            y: [from.y.min(to.y), from.y.max(to.y)],
        }
    }

    pub(super) fn finite(self) -> bool {
        self.x.into_iter().chain(self.y).all(f64::is_finite)
    }

    pub(super) fn contains_center(self, center: Point) -> bool {
        self.x[0] < center.x && center.x < self.x[1] && self.y[0] < center.y && center.y < self.y[1]
    }

    fn union(self, other: Self) -> Self {
        let union = |a: [f64; 2], b: [f64; 2]| [a[0].min(b[0]), a[1].max(b[1])];
        Self {
            x: union(self.x, other.x),
            y: union(self.y, other.y),
        }
    }

    pub(super) fn overlaps(self, other: Self) -> bool {
        self.x[0] <= other.x[1]
            && other.x[0] <= self.x[1]
            && self.y[0] <= other.y[1]
            && other.y[0] <= self.y[1]
    }

    fn hits_segment(self, from: Point, to: Point) -> bool {
        let (mut enter, mut leave) = (0.0_f64, 1.0_f64);
        for ([low, high], a, b) in [(self.x, from.x, to.x), (self.y, from.y, to.y)] {
            if a == b {
                if a < low || a > high {
                    return false;
                }
            } else {
                let (p, q) = ((low - a) / (b - a), (high - a) / (b - a));
                enter = enter.max(p.min(q));
                leave = leave.min(p.max(q));
            }
        }
        enter <= leave
    }

    fn hits_corner(self, bounds: Self, corner: Point) -> bool {
        // The rectangle point closest to the corner also minimizes distance in the two
        // normalized axes of this right triangle. The triangle contains every rounded turn.
        let dx = (corner.x - corner.x.clamp(self.x[0], self.x[1])).abs();
        let dy = (corner.y - corner.y.clamp(self.y[0], self.y[1])).abs();
        dx / (bounds.x[1] - bounds.x[0]) + dy / (bounds.y[1] - bounds.y[0]) <= 1.0
    }
}

pub(super) fn point(knot: RoutePoint) -> Point {
    Point {
        x: knot.x,
        y: knot.y,
    }
}

fn aligned(a: Point, b: Point) -> bool {
    a.x == b.x || a.y == b.y
}

fn between(a: f64, b: f64, c: f64) -> bool {
    a.min(c) <= b && b <= a.max(c)
}

#[derive(Clone, Copy)]
enum Shape {
    Area,
    Segment(Point, Point),
    Corner(Point),
}

struct Wall {
    bounds: Bounds,
    stroke: Option<usize>,
    shape: Shape,
}

impl Wall {
    fn blocks(&self, label: Bounds, own_stroke: usize) -> bool {
        if self.stroke == Some(own_stroke) || !self.bounds.overlaps(label) {
            return false;
        }
        match self.shape {
            Shape::Area => true,
            Shape::Segment(from, to) => label.hits_segment(from, to),
            Shape::Corner(at) => label.hits_corner(self.bounds, at),
        }
    }
}

#[derive(Default)]
pub(super) struct Scene {
    walls: Vec<Wall>,
    knots: Vec<Point>,
    total_knots: usize,
}

impl Scene {
    fn add(
        &mut self,
        bounds: Bounds,
        stroke: Option<usize>,
        shape: Shape,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        if self.walls.len() == MAX_WALLS || !bounds.finite() {
            return Ok(false);
        }
        work.visit(1)?;
        push(
            &mut self.walls,
            Wall {
                bounds,
                stroke,
                shape,
            },
        )?;
        Ok(true)
    }

    pub(super) fn area(&mut self, bounds: Bounds, work: &mut Work<'_>) -> LayoutResult<bool> {
        self.add(bounds, None, Shape::Area, work)
    }

    pub(super) fn marker(
        &mut self,
        tip: RoutePoint,
        side: Side,
        marker: MarkerMetrics,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        if marker.length == 0.0 || marker.width == 0.0 {
            return Ok(true);
        }
        self.area(Bounds::marker(point(tip), side, marker), work)
    }

    pub(super) fn stroke(
        &mut self,
        edge: usize,
        points: &[RoutePoint],
        style: RoutingStyle,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        if points.len() > MAX_KNOTS - self.total_knots {
            return Ok(false);
        }
        self.total_knots += points.len();
        self.knots.clear();
        for &p in points {
            work.visit(1)?;
            let p = point(p);
            if self.knots.last() == Some(&p) {
                continue;
            }
            if let &[.., a, b] = self.knots.as_slice()
                && (a.x == b.x && b.x == p.x && between(a.y, b.y, p.y)
                    || a.y == b.y && b.y == p.y && between(a.x, b.x, p.x))
            {
                self.knots.pop();
            }
            push(&mut self.knots, p)?;
        }
        let bends = style != RoutingStyle::Polyline;
        for i in 1..self.knots.len() {
            let (a, b) = (self.knots[i - 1], self.knots[i]);
            let bounds = Bounds::span(a, b);
            if bends && !aligned(a, b) {
                // Sweeps and steps are coordinate-monotone. Terminal smoothing may absorb
                // the preceding/following collinear shaft, extending the same envelope.
                let last = self.knots.len() - 1;
                let mut sweep = bounds;
                if i == 2 && aligned(self.knots[0], a) {
                    sweep = sweep.union(Bounds::span(self.knots[0], b));
                }
                if i + 1 == last && aligned(b, self.knots[last]) {
                    sweep = sweep.union(Bounds::span(a, self.knots[last]));
                }
                if !self.add(sweep, Some(edge), Shape::Area, work)? {
                    return Ok(false);
                }
            } else if !self.add(bounds, Some(edge), Shape::Segment(a, b), work)? {
                return Ok(false);
            }
            if bends
                && let Some(&end) = self.knots.get(i + 1)
                && aligned(a, b)
                && aligned(b, end)
                && !aligned(a, end)
                && !self.add(Bounds::span(a, end), Some(edge), Shape::Corner(b), work)?
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    pub(super) fn index(self, work: &mut Work<'_>) -> LayoutResult<Clearance> {
        let mut walls = self.walls;
        let capacity = walls.len().div_ceil(LEAF_SIZE) * 4;
        work.charge(capacity)?;
        let mut branches = reserved(capacity)?;
        let root = if walls.is_empty() {
            None
        } else {
            Some(Branch::build(&mut walls, 0, &mut branches, work)?)
        };
        Ok(Clearance {
            walls,
            branches,
            root,
            remaining: MAX_CHECKS,
        })
    }
}

#[derive(Clone, Copy)]
enum Child {
    Range { start: usize, end: usize },
    Pair(usize, usize),
}

#[derive(Clone, Copy)]
struct Branch {
    bounds: Bounds,
    child: Child,
}

impl Branch {
    fn build(
        walls: &mut [Wall],
        offset: usize,
        branches: &mut Vec<Self>,
        work: &mut Work<'_>,
    ) -> LayoutResult<usize> {
        work.charge(walls.len())?;
        let bounds = walls
            .iter()
            .skip(1)
            .fold(walls[0].bounds, |bounds, wall| bounds.union(wall.bounds));
        let child = if walls.len() <= LEAF_SIZE {
            Child::Range {
                start: offset,
                end: offset + walls.len(),
            }
        } else {
            let horizontal = bounds.x[1] - bounds.x[0] >= bounds.y[1] - bounds.y[0];
            let center = |wall: &Wall| {
                let axis = if horizontal {
                    wall.bounds.x
                } else {
                    wall.bounds.y
                };
                axis[0].midpoint(axis[1])
            };
            let middle = walls.len() / 2;
            work.charge(walls.len())?;
            walls.select_nth_unstable_by(middle, |a, b| center(a).total_cmp(&center(b)));
            let (left, right) = walls.split_at_mut(middle);
            Child::Pair(
                Self::build(left, offset, branches, work)?,
                Self::build(right, offset + middle, branches, work)?,
            )
        };
        let index = branches.len();
        push(branches, Self { bounds, child })?;
        Ok(index)
    }
}

pub(super) struct Clearance {
    walls: Vec<Wall>,
    branches: Vec<Branch>,
    root: Option<usize>,
    remaining: usize,
}

impl Clearance {
    pub(super) fn exhausted(&self) -> bool {
        self.remaining == 0
    }

    pub(super) fn visit(&mut self, work: &mut Work<'_>) -> LayoutResult<bool> {
        if self.exhausted() {
            return Ok(false);
        }
        self.remaining -= 1;
        work.visit(1)?;
        Ok(true)
    }

    pub(super) fn clear(
        &mut self,
        label: Bounds,
        own_stroke: usize,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        let Some(root) = self.root else {
            return Ok(true);
        };
        self.clear_branch(root, label, own_stroke, work)
    }

    fn clear_branch(
        &mut self,
        index: usize,
        label: Bounds,
        own_stroke: usize,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        if !self.visit(work)? {
            return Ok(false);
        }
        let branch = self.branches[index];
        if !label.overlaps(branch.bounds) {
            return Ok(true);
        }
        match branch.child {
            Child::Range { start, end } => {
                for index in start..end {
                    if !self.visit(work)? || self.walls[index].blocks(label, own_stroke) {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Child::Pair(left, right) => Ok(self.clear_branch(left, label, own_stroke, work)?
                && self.clear_branch(right, label, own_stroke, work)?),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Axis, NoopWorkControl};

    #[test]
    fn a_linear_shaft_uses_segment_intersection_and_a_curved_shaft_reserves_its_sweep() {
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        let points = [
            RoutePoint::new(0.0, 0.0, Axis::Vertical),
            RoutePoint::new(40.0, 40.0, Axis::Vertical),
        ];
        let hit = Bounds {
            x: [19.0, 21.0],
            y: [19.0, 21.0],
        };
        let corner = Bounds {
            x: [10.0, 12.0],
            y: [25.0, 27.0],
        };
        for (style, fits_corner) in [
            (RoutingStyle::Polyline, true),
            (RoutingStyle::Curved, false),
        ] {
            let mut scene = Scene::default();
            assert!(scene.stroke(1, &points, style, &mut work).unwrap());
            assert!(
                scene
                    .marker(
                        points[1],
                        Side::Top,
                        MarkerMetrics {
                            length: 9.0,
                            width: 8.0
                        },
                        &mut work
                    )
                    .unwrap()
            );
            let mut clearance = scene.index(&mut work).unwrap();
            assert!(!clearance.clear(hit, 0, &mut work).unwrap());
            assert!(clearance.clear(hit, 1, &mut work).unwrap());
            assert_eq!(clearance.clear(corner, 0, &mut work).unwrap(), fits_corner);
            let marker = Bounds {
                x: [41.0, 42.0],
                y: [34.0, 35.0],
            };
            assert!(!clearance.clear(marker, 1, &mut work).unwrap());
        }
    }

    #[test]
    fn exhausting_the_search_budget_does_not_approve_an_unexamined_candidate() {
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        let mut scene = Scene::default();
        for i in 0..20 {
            let x = f64::from(i) * 10.0;
            assert!(
                scene
                    .area(
                        Bounds {
                            x: [x, x + 5.0],
                            y: [0.0, 5.0]
                        },
                        &mut work
                    )
                    .unwrap()
            );
        }
        let mut index = scene.index(&mut work).unwrap();
        let between = Bounds {
            x: [6.0, 8.0],
            y: [1.0, 2.0],
        };
        assert!(index.clear(between, 0, &mut work).unwrap());
        index.remaining = 1;
        assert!(!index.clear(between, 0, &mut work).unwrap());
        assert!(index.exhausted());
    }
}
