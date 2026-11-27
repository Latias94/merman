//! Emit branches of an orthogonal comb after the caller proves its clearance.

use crate::work::{LayoutResult, Work, reserved};
use crate::{Axis, Point, RoutePoint};

pub(crate) const MIN_DEGREE: usize = 6;

pub(crate) struct Comb {
    pub(crate) travel: Axis,
    pub(crate) shelf: f64,
    pub(crate) corner: f64,
}

impl Comb {
    fn on_shelf(&self, point: Point) -> Point {
        match self.travel {
            Axis::Vertical => Point {
                y: self.shelf,
                ..point
            },
            Axis::Horizontal => Point {
                x: self.shelf,
                ..point
            },
        }
    }

    pub(crate) fn branch(
        &self,
        hub: Point,
        knee: Point,
        leaf: Point,
        reversed: bool,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Vec<RoutePoint>>> {
        let from = self.on_shelf(knee);
        let to = self.on_shelf(leaf);
        if from == knee || to == leaf {
            return Ok(None);
        }
        let mut turns = [Point::default(); 5];
        let mut count = 0;
        for at in [hub, knee, from, to, leaf] {
            if count > 0 && turns[count - 1] == at {
                continue;
            }
            if count > 1 {
                let [a, b] = [turns[count - 2], turns[count - 1]];
                let incoming = axis(a, b);
                if incoming == axis(b, at) {
                    let [a, b, c] = [a, b, at].map(|p| match incoming {
                        Axis::Horizontal => p.x,
                        Axis::Vertical => p.y,
                    });
                    if !(a.min(c) < b && b < a.max(c)) {
                        return Ok(None);
                    }
                    count -= 1;
                }
            }
            turns[count] = at;
            count += 1;
        }
        let turns = &mut turns[..count];
        if reversed {
            turns.reverse();
        }
        let [first, second, ..] = &*turns else {
            return Ok(None);
        };
        let capacity = 1 + 3 * (turns.len() - 1);
        work.charge(capacity)?;
        let mut points = reserved(capacity)?;
        points.push(RoutePoint::new(first.x, first.y, axis(*first, *second)));
        for (index, leg) in turns.windows(2).enumerate() {
            let [a, b] = [leg[0], leg[1]];
            let axis = axis(a, b);
            // Guard knots bound rounded corners to the region checked by the caller.
            for guard in [
                (index > 0).then(|| corner_guard(a, b, axis, self.corner)),
                (index + 2 < turns.len()).then(|| corner_guard(b, a, axis, self.corner)),
            ]
            .into_iter()
            .flatten()
            {
                if guard == a || guard == b {
                    return Ok(None);
                }
                let knot = RoutePoint::gap_boundary(guard.x, guard.y, axis);
                if points.last() != Some(&knot) {
                    points.push(knot);
                }
            }
            points.push(RoutePoint::new(b.x, b.y, axis));
        }
        Ok(Some(points))
    }
}

fn axis(a: Point, b: Point) -> Axis {
    if a.x == b.x {
        Axis::Vertical
    } else {
        Axis::Horizontal
    }
}

pub(crate) fn corner_guard(a: Point, b: Point, axis: Axis, corner: f64) -> Point {
    let toward = |a: f64, b: f64| {
        if (b - a).abs() <= 2.0 * corner {
            a.midpoint(b)
        } else {
            a + (b - a).signum() * corner
        }
    };
    match axis {
        Axis::Vertical => Point {
            y: toward(a.y, b.y),
            ..a
        },
        Axis::Horizontal => Point {
            x: toward(a.x, b.x),
            ..a
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NoopWorkControl;

    #[test]
    fn comb_declines_a_collinear_reversal_in_either_direction() {
        let comb = Comb {
            travel: Axis::Vertical,
            shelf: 5.0,
            corner: 1.0,
        };
        for reversed in [false, true] {
            let branch = comb
                .branch(
                    Point { x: 0.0, y: 0.0 },
                    Point { x: 0.0, y: 10.0 },
                    Point { x: 5.0, y: 15.0 },
                    reversed,
                    &mut Work::new(&mut NoopWorkControl),
                )
                .unwrap();
            assert!(branch.is_none());
        }
    }
}
