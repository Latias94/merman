pub(super) use crate::geom::Bounds;
use crate::geom::{Bvh, BvhEntry};
use crate::work::{LayoutResult, Work, push};
use crate::{MarkerMetrics, Point, RoutePoint, RoutingStyle, Side, Size, WorkError};
use std::num::NonZeroU32;
use std::ops::ControlFlow::{Break, Continue};

const MAX_WALLS: usize = 16 * 1024;
const MAX_KNOTS: usize = 2 * MAX_WALLS;
const MAX_CHECKS: usize = 128 * 1024;

impl Bounds {
    pub(super) fn label(at: Point, size: Size, pad: f64) -> Self {
        let (w, h) = (size.width * 0.5 + pad, size.height * 0.5 + pad);
        Self::from_half_extents(at, w, h)
    }

    pub(super) fn contains_center(self, center: Point) -> bool {
        self.x[0] < center.x && center.x < self.x[1] && self.y[0] < center.y && center.y < self.y[1]
    }

    fn hits_segment(self, bounds: Self, [reverse_x, reverse_y]: [bool; 2]) -> bool {
        let (mut enter, mut leave) = (0.0_f64, 1.0_f64);
        for ([low, high], [a, b], reverse) in
            [(self.x, bounds.x, reverse_x), (self.y, bounds.y, reverse_y)]
        {
            if a == b {
                // Equal endpoints, including opposite signed zeros, only need comparisons.
                if a < low || a > high {
                    return false;
                }
            } else {
                // Reversing a segment can change the rounding at a clipped boundary.
                let (a, b) = if reverse { (b, a) } else { (a, b) };
                let (p, q) = ((low - a) / (b - a), (high - a) / (b - a));
                enter = enter.max(p.min(q));
                leave = leave.min(p.max(q));
            }
        }
        enter <= leave
    }

    fn hits_corner(self, bounds: Self, [high_x, high_y]: [bool; 2]) -> bool {
        // The rectangle point closest to the corner also minimizes distance in the two
        // normalized axes of this right triangle. The triangle contains every rounded turn.
        let x = bounds.x[usize::from(high_x)];
        let y = bounds.y[usize::from(high_y)];
        let dx = (x - x.clamp(self.x[0], self.x[1])).abs();
        let dy = (y - y.clamp(self.y[0], self.y[1])).abs();
        dx / (bounds.x[1] - bounds.x[0]) + dy / (bounds.y[1] - bounds.y[0]) <= 1.0
    }

    fn hits_diamond(self, diamond: Self) -> bool {
        let distance = |[a, b]: [f64; 2], [low, high]: [f64; 2]| {
            let half = (b - a) * 0.5;
            if half <= 0.0 {
                return 0.0;
            }
            let center = a.midpoint(b);
            (center - center.clamp(low, high)).abs() / half
        };
        distance(diamond.x, self.x) + distance(diamond.y, self.y) <= 1.0
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

fn stroke_id(edge: usize) -> Option<NonZeroU32> {
    NonZeroU32::new(u32::try_from(edge.checked_add(1)?).ok()?)
}

#[derive(Clone, Copy)]
enum Shape {
    Area,
    Diamond,
    Segment { reverse: [bool; 2] },
    Corner { high: [bool; 2] },
}

struct Wall {
    bounds: Bounds,
    stroke: Option<NonZeroU32>,
    shape: Shape,
}

impl Wall {
    fn blocks(&self, label: Bounds, own_stroke: Option<NonZeroU32>) -> bool {
        if own_stroke.is_some() && self.stroke == own_stroke || !self.bounds.overlaps(label) {
            return false;
        }
        match self.shape {
            Shape::Area => true,
            Shape::Diamond => label.hits_diamond(self.bounds),
            Shape::Segment { reverse } => label.hits_segment(self.bounds, reverse),
            Shape::Corner { high } => label.hits_corner(self.bounds, high),
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
        stroke: Option<NonZeroU32>,
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

    pub(super) fn diamond(&mut self, bounds: Bounds, work: &mut Work<'_>) -> LayoutResult<bool> {
        self.add(bounds, None, Shape::Diamond, work)
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
        let stroke = Some(stroke_id(edge).ok_or(WorkError::ArithmeticOverflow)?);
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
            let bounds = Bounds::between(a, b);
            let shape = if bends && !aligned(a, b) {
                // The solver supplies both ends of the monotone sweep.
                Shape::Area
            } else {
                Shape::Segment {
                    reverse: [a.x > b.x, a.y > b.y],
                }
            };
            if !self.add(bounds, stroke, shape, work)? {
                return Ok(false);
            }
            if bends
                && let Some(&end) = self.knots.get(i + 1)
                && aligned(a, b)
                && aligned(b, end)
                && !aligned(a, end)
            {
                let bounds = Bounds::between(a, end);
                let shape = Shape::Corner {
                    high: [b.x > bounds.x[0], b.y > bounds.y[0]],
                };
                if !self.add(bounds, stroke, shape, work)? {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    pub(super) fn index(self, work: &mut Work<'_>) -> LayoutResult<Clearance> {
        Ok(Clearance {
            walls: Bvh::build_with_work(self.walls, |wall| wall.bounds, work)?,
            remaining: MAX_CHECKS,
        })
    }
}

pub(super) struct Clearance {
    walls: Bvh<Wall>,
    remaining: usize,
}

impl Clearance {
    pub(super) fn exhausted(&self) -> bool {
        self.remaining == 0
    }

    pub(super) fn visit(&mut self, work: &mut Work<'_>) -> LayoutResult<bool> {
        visit_check(&mut self.remaining, work)
    }

    pub(super) fn clear(
        &mut self,
        label: Bounds,
        own_stroke: usize,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        let remaining = &mut self.remaining;
        let own_stroke = stroke_id(own_stroke);
        let result = self.walls.visit(|entry| -> LayoutResult<_> {
            if !visit_check(remaining, work)? {
                return Ok(Break(()));
            }
            Ok(match entry {
                BvhEntry::Node(bounds) => Continue(label.overlaps(bounds)),
                BvhEntry::Item(wall) if wall.blocks(label, own_stroke) => Break(()),
                BvhEntry::Item(_) => Continue(true),
            })
        })?;
        Ok(result.is_continue())
    }
}

fn visit_check(remaining: &mut usize, work: &mut Work<'_>) -> LayoutResult<bool> {
    if *remaining == 0 {
        return Ok(false);
    }
    *remaining -= 1;
    work.visit(1)?;
    Ok(true)
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
    fn compact_shaft_clearance_preserves_direction_at_a_floating_point_contact() {
        let (x, y) = (2.0_f64.powi(512), 2.0_f64.powi(-512));
        let start = RoutePoint::new(-0.0, 0.0, Axis::Vertical);
        let end = RoutePoint::new(10.0 * x, 3.0 * y, Axis::Vertical);
        let label = Bounds {
            x: [0.0, x],
            y: [0.30000000000000004 * y, y],
        };
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        let edge = u32::MAX as usize - 1;
        for (points, clear) in [([start, end], true), ([end, start], false)] {
            let mut scene = Scene::default();
            assert!(
                scene
                    .stroke(edge, &points, RoutingStyle::Polyline, &mut work)
                    .unwrap()
            );
            let mut index = scene.index(&mut work).unwrap();
            assert_eq!(index.clear(label, 0, &mut work).unwrap(), clear);
            assert!(index.clear(label, edge, &mut work).unwrap());
        }
        assert!(std::mem::size_of::<Wall>() <= 48);
    }

    #[test]
    fn an_offset_diamond_blocks_its_sloping_boundary_and_leaves_empty_corners_clear() {
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        let mut scene = Scene::default();
        assert!(
            scene
                .diamond(
                    Bounds {
                        x: [13., 33.],
                        y: [-9., 3.]
                    },
                    &mut work
                )
                .unwrap()
        );
        let mut index = scene.index(&mut work).unwrap();
        for (label, clear) in [
            (
                Bounds {
                    x: [28., 29.],
                    y: [0., 1.],
                },
                false,
            ),
            (
                Bounds {
                    x: [30., 31.],
                    y: [0., 1.],
                },
                true,
            ),
            (
                Bounds {
                    x: [9., 11.],
                    y: [0., 2.],
                },
                true,
            ),
        ] {
            assert_eq!(index.clear(label, 0, &mut work).unwrap(), clear);
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
