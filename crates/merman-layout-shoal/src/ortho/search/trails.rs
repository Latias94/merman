use crate::model::RoutePoint;
use crate::ortho::{OrthoSegment, at, axis_id};
use crate::work::{LayoutResult, Work, push, reserved};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::ops::Bound::{Excluded, Included};

const CROSS_COST: f64 = 80.0;
const OVERLAP_COST: f64 = 4.0;

#[derive(Clone, Copy)]
struct Coordinate(f64);

impl PartialEq for Coordinate {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Coordinate {}
impl PartialOrd for Coordinate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Coordinate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

#[derive(Default)]
pub(in crate::ortho) struct Trails {
    lines: [BTreeMap<Coordinate, Vec<[f64; 2]>>; 2],
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
                let key = Coordinate(leg.fixed);
                if let Some(row) = lines.get_mut(&key) {
                    push(row, [leg.min, leg.max])?;
                } else {
                    let mut row = reserved(1)?;
                    row.push([leg.min, leg.max]);
                    lines.insert(key, row);
                }
                inserted += 1;
            }
            work.finish()
        })();
        if result.is_err() {
            for leg in self.pending[..inserted].iter().rev() {
                let lines = &mut self.lines[axis_id(leg.axis)];
                let key = Coordinate(leg.fixed);
                let row = lines.get_mut(&key).expect("staged trail exists");
                row.pop();
                if row.is_empty() {
                    lines.remove(&key);
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
        if let Some(row) = parallel.get(&Coordinate(leg.fixed)) {
            for &[a, b] in row {
                work.visit(1)?;
                cost += (b.min(leg.max) - a.max(leg.min)).max(0.0) * OVERLAP_COST;
            }
        }
        let crossing = &self.lines[1 - axis];
        let bound = |value| {
            if Some(value) == include_start {
                Included(Coordinate(value))
            } else {
                Excluded(Coordinate(value))
            }
        };
        work.visit(crossing.len().checked_ilog2().unwrap_or(0) as usize + 1)?;
        for (_, row) in crossing.range((bound(leg.min), bound(leg.max))) {
            work.visit(1)?;
            for &[a, b] in row {
                work.visit(1)?;
                if a < leg.fixed && leg.fixed < b {
                    cost += CROSS_COST;
                }
            }
        }
        Ok(cost)
    }
}
