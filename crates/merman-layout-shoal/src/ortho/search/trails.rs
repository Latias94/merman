use crate::model::RoutePoint;
use crate::ortho::{OrthoSegment, at, axis_id};
use crate::work::{LayoutError, LayoutResult, Work, WorkError, push};

const CROSS_COST: f64 = 80.0;
const OVERLAP_COST: f64 = 4.0;

struct Line {
    fixed: f64,
    first: [f64; 2],
    rest: Vec<[f64; 2]>,
}

impl Line {
    fn spans(&self) -> impl Iterator<Item = [f64; 2]> + '_ {
        std::iter::once(self.first).chain(self.rest.iter().copied())
    }
}

fn find(lines: &[Line], fixed: f64) -> Result<usize, usize> {
    lines.binary_search_by(|line| line.fixed.total_cmp(&fixed))
}

#[derive(Default)]
pub(in crate::ortho) struct Trails {
    lines: [Vec<Line>; 2],
    pending: Vec<OrthoSegment>,
}

impl Trails {
    pub(in crate::ortho) fn record(
        &mut self,
        points: &[RoutePoint],
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        work.check()?;
        self.pending.clear();
        let mut continuous = false;
        for pair in points.windows(2) {
            work.visit(1)?;
            if at(pair[0]) == at(pair[1]) {
                continue;
            }
            let Some(leg) = OrthoSegment::between(pair[0], pair[1]) else {
                continuous = false;
                continue;
            };
            if continuous
                && let Some(last) = self.pending.last_mut()
                && leg.axis == last.axis
                && leg.fixed == last.fixed
            {
                last.min = last.min.min(leg.min);
                last.max = last.max.max(leg.max);
            } else {
                push(&mut self.pending, leg)?;
            }
            continuous = true;
        }
        let mut inserted = 0;
        let result = (|| {
            for leg in &self.pending {
                let lines = &mut self.lines[axis_id(leg.axis)];
                work.visit(lines.len().checked_ilog2().unwrap_or(0) as usize + 1)?;
                let span = [leg.min, leg.max];
                match find(lines, leg.fixed) {
                    Ok(row) => push(&mut lines[row].rest, span)?,
                    Err(row) => {
                        // A failed transaction can shift the same suffix back in reverse order.
                        let shifts = (lines.len() - row)
                            .checked_mul(2)
                            .ok_or(WorkError::ArithmeticOverflow)?;
                        work.visit(shifts)?;
                        lines
                            .try_reserve(1)
                            .map_err(|_| LayoutError::AllocationLimit)?;
                        lines.insert(
                            row,
                            Line {
                                fixed: leg.fixed,
                                first: span,
                                rest: Vec::new(),
                            },
                        );
                    }
                }
                inserted += 1;
            }
            work.finish()
        })();
        if result.is_err() {
            for leg in self.pending[..inserted].iter().rev() {
                let lines = &mut self.lines[axis_id(leg.axis)];
                let row = find(lines, leg.fixed).expect("staged trail exists");
                if lines[row].rest.pop().is_none() {
                    lines.remove(row);
                }
            }
        }
        result
    }

    pub(super) fn cost(
        &self,
        leg: OrthoSegment,
        include_start: Option<f64>,
        work: &mut Work<'_>,
    ) -> LayoutResult<f64> {
        let axis = axis_id(leg.axis);
        let mut cost = 0.0;
        let parallel = &self.lines[axis];
        work.visit(parallel.len().checked_ilog2().unwrap_or(0) as usize + 1)?;
        if let Ok(row) = find(parallel, leg.fixed) {
            for [a, b] in parallel[row].spans() {
                work.visit(1)?;
                cost += (b.min(leg.max) - a.max(leg.min)).max(0.0) * OVERLAP_COST;
            }
        }
        let crossing = &self.lines[1 - axis];
        work.visit(crossing.len().checked_ilog2().unwrap_or(0) as usize + 1)?;
        let include_min = include_start == Some(leg.min);
        let include_max = include_start == Some(leg.max);
        let first = crossing.partition_point(|line| {
            let order = line.fixed.total_cmp(&leg.min);
            order.is_lt() || (order.is_eq() && !include_min)
        });
        for line in &crossing[first..] {
            let order = line.fixed.total_cmp(&leg.max);
            if order.is_gt() || (order.is_eq() && !include_max) {
                break;
            }
            work.visit(1)?;
            for [a, b] in line.spans() {
                work.visit(1)?;
                if a < leg.fixed && leg.fixed < b {
                    cost += CROSS_COST;
                }
            }
        }
        Ok(cost)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Axis;
    use crate::work::{NoopWorkControl, WorkControl};

    fn point(x: f64, y: f64) -> RoutePoint {
        RoutePoint::new(x, y, Axis::Horizontal)
    }

    #[test]
    fn trail_cost_keeps_overlap_and_open_crossing_endpoints() {
        let mut trails = Trails::default();
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        for points in [
            &[
                point(-3.5, 0.0),
                point(2.0, 0.0),
                point(1.0, -0.0),
                point(1.0, 0.0),
                point(5.25, 0.0),
            ][..],
            &[point(-8.0, 0.0), point(-3.5, 0.0)],
            &[point(1.125, 0.0), point(2.375, 0.0)],
            &[point(5.25, -1.0), point(5.25, 1.0)],
            &[point(0.0, -1.0), point(0.0, 1.0)],
            &[point(-3.5, -1.0), point(-3.5, 1.0)],
            &[point(2.0, 0.0), point(2.0, 1.0)],
        ] {
            trails.record(points, &mut work).unwrap();
        }
        let mut cost = |min, max, include| {
            let leg = OrthoSegment::between(point(min, 0.0), point(max, 0.0)).unwrap();
            trails.cost(leg, include, &mut work).unwrap()
        };
        assert_eq!(cost(-3.5, 5.25, None), 120.0);
        assert_eq!(cost(-3.5, 5.25, Some(-3.5)), 200.0);
        assert_eq!(cost(-3.5, 5.25, Some(5.25)), 200.0);
        assert_eq!(cost(-3.5, 0.0, None) + cost(0.0, 5.25, Some(-0.0)), 120.0);
        assert_eq!(cost(0.0, 5.25, None) + cost(-3.5, 0.0, Some(0.0)), 120.0);
    }

    #[derive(Default)]
    struct Meter {
        units: usize,
        checks: usize,
        stop_at: Option<usize>,
    }

    impl WorkControl for Meter {
        fn check(&mut self) -> Result<(), WorkError> {
            if self.stop_at == Some(self.checks) {
                return Err(WorkError::Interrupted);
            }
            self.checks += 1;
            Ok(())
        }

        fn charge(&mut self, units: usize) -> Result<(), WorkError> {
            self.units += units;
            Ok(())
        }
    }

    #[test]
    fn shifted_rows_are_metered_and_restored_on_interruption() {
        const ROWS: i32 = 640;
        let seed = || {
            let mut trails = Trails::default();
            let mut control = NoopWorkControl;
            let mut work = Work::new(&mut control);
            for row in 0..ROWS {
                let y = f64::from(row);
                trails
                    .record(&[point(0.0, y), point(10.0, y)], &mut work)
                    .unwrap();
            }
            trails
        };
        let units = |y| {
            let mut meter = Meter::default();
            seed()
                .record(&[point(0.0, y), point(10.0, y)], &mut Work::new(&mut meter))
                .unwrap();
            meter.units
        };
        assert_eq!(units(-1.0) - units(f64::from(ROWS)), 2 * ROWS as usize);

        let mut trails = seed();
        let points = [
            point(0.0, 10.0),
            point(8.0, 10.0),
            point(8.0, -1.0),
            point(0.0, -1.0),
        ];
        let mut meter = Meter::default();
        seed().record(&points, &mut Work::new(&mut meter)).unwrap();
        let mut meter = Meter {
            stop_at: Some(meter.checks - 1),
            ..Default::default()
        };
        assert_eq!(
            trails.record(&points, &mut Work::new(&mut meter)),
            Err(LayoutError::Work(WorkError::Interrupted)),
        );
        assert!(trails.lines[1].is_empty());
        assert_eq!(trails.lines[0].len(), ROWS as usize);
        for (index, row) in (0..ROWS).zip(&trails.lines[0]) {
            assert_eq!(row.fixed, f64::from(index));
            assert!(row.spans().eq([[0.0, 10.0]]));
        }
        trails
            .record(&points, &mut Work::new(&mut NoopWorkControl))
            .unwrap();
        assert_eq!(trails.lines[0][0].fixed, -1.0);
        assert!(trails.lines[0][11].spans().eq([[0.0, 10.0], [0.0, 8.0]]));
        assert!(trails.lines[1][0].spans().eq([[-1.0, 10.0]]));
    }
}
