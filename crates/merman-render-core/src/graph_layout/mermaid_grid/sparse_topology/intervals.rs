use super::{
    CLEARANCE, GridTopologyError, GridTopologyInput, GridTopologyObstacle, allocation_failed,
    charge_lookup, check_estimate, checked_add, checked_mul, code_units, coordinate_order,
    estimate, invalid, normalize_rect, reserved, sort_metered,
};
use crate::graph_layout::Axis;
use crate::resources::OperationWorkMeter;
use kurbo::Rect;
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug)]
struct Span {
    low: f64,
    high: f64,
}

impl Span {
    fn compare(self, other: Self) -> Ordering {
        coordinate_order(self.low, other.low).then_with(|| coordinate_order(self.high, other.high))
    }
}

#[derive(Clone, Copy)]
struct TaggedSpan<'a> {
    span: Span,
    owner: &'a str,
}

#[derive(Debug)]
struct Boundary {
    coordinate: f64,
    strict: Vec<Span>,
    after: Vec<Span>,
}

/// Sparse interval bands for either axis. An exact-coordinate point and a segment on that
/// coordinate use different parts of the obstacle union, as in the reference.
#[derive(Debug)]
pub struct GridIntervalIndex {
    boundaries: Vec<Boundary>,
}

impl GridIntervalIndex {
    pub(super) fn build(
        obstacles: &[GridTopologyObstacle<'_>],
        axis: Axis,
        budget: &mut IntervalBudget,
        work: &OperationWorkMeter,
    ) -> Result<Self, GridTopologyError> {
        let mut sweep = Sweep::new(obstacles, axis, work)?;
        let mut intervals = reserved(obstacles.len(), work)?;
        let mut boundaries = reserved(sweep.events.len(), work)?;
        while let Some(coordinate) = sweep.leave(work)? {
            sweep.merge_spans(obstacles, axis, &mut intervals, work)?;
            let strict = budget.freeze(&intervals, work)?;
            sweep.enter(coordinate, work)?;
            let after = if sweep.next_coordinate().is_some() {
                sweep.merge_spans(obstacles, axis, &mut intervals, work)?;
                budget.freeze(&intervals, work)?
            } else {
                Vec::new()
            };
            boundaries.push(Boundary {
                coordinate,
                strict,
                after,
            });
        }
        Ok(Self { boundaries })
    }

    pub fn contains(
        &self,
        fixed: f64,
        varying: f64,
        work: &OperationWorkMeter,
    ) -> crate::Result<bool> {
        check_coordinates(&[varying])?;
        let intervals = self.strict_at(fixed, work)?;
        charge_lookup(intervals.len(), work)?;
        let index = intervals.partition_point(|span| span.low < varying);
        Ok(index != 0 && varying < intervals[index - 1].high)
    }

    pub fn nearest_boundary(
        &self,
        fixed: f64,
        origin: f64,
        toward_start: bool,
        work: &OperationWorkMeter,
    ) -> crate::Result<Option<f64>> {
        check_coordinates(&[origin])?;
        let intervals = self.strict_at(fixed, work)?;
        charge_lookup(intervals.len(), work)?;
        if toward_start {
            let index = intervals.partition_point(|span| span.high <= origin);
            Ok(index.checked_sub(1).map(|index| intervals[index].high))
        } else {
            let index = intervals.partition_point(|span| span.low < origin);
            Ok(intervals.get(index).map(|span| span.low))
        }
    }

    pub fn intersects(
        &self,
        fixed: f64,
        start: f64,
        end: f64,
        work: &OperationWorkMeter,
    ) -> crate::Result<bool> {
        check_coordinates(&[start, end])?;
        let low = start.min(end);
        let high = start.max(end);
        match self.find_boundary(fixed, work)? {
            Err(after) => {
                let intervals = self.band_before(after);
                charge_lookup(intervals.len(), work)?;
                let index = intervals.partition_point(|span| span.high <= low);
                Ok(intervals.get(index).is_some_and(|span| span.low < high))
            }
            Ok(boundary) => {
                // A boundary segment is obstructed only where both incident open bands have
                // interior. Point and ray queries instead use rectangles crossing the boundary.
                let mut before = self.band_before(boundary);
                let mut after = self.boundaries[boundary].after.as_slice();
                while let (Some(a), Some(b)) = (before.first(), after.first()) {
                    work.charge(1)?;
                    if low.max(a.low).max(b.low) < high.min(a.high).min(b.high) {
                        return Ok(true);
                    }
                    if a.high < b.high {
                        before = &before[1..];
                    } else {
                        after = &after[1..];
                    }
                }
                Ok(false)
            }
        }
    }

    fn strict_at(&self, fixed: f64, work: &OperationWorkMeter) -> crate::Result<&[Span]> {
        Ok(match self.find_boundary(fixed, work)? {
            Ok(boundary) => &self.boundaries[boundary].strict,
            Err(after) => self.band_before(after),
        })
    }

    fn find_boundary(
        &self,
        fixed: f64,
        work: &OperationWorkMeter,
    ) -> crate::Result<Result<usize, usize>> {
        check_coordinates(&[fixed])?;
        charge_lookup(self.boundaries.len(), work)?;
        Ok(self
            .boundaries
            .binary_search_by(|boundary| coordinate_order(boundary.coordinate, fixed)))
    }

    fn band_before(&self, after: usize) -> &[Span] {
        after
            .checked_sub(1)
            .map_or(&[], |index| self.boundaries[index].after.as_slice())
    }
}

fn check_coordinates(values: &[f64]) -> crate::Result<()> {
    if values.iter().any(|value| !value.is_finite()) {
        Err(invalid(
            "Grid interval queries must contain only finite coordinates",
        ))
    } else {
        Ok(())
    }
}

pub(super) struct IntervalBudget {
    pub(super) count: usize,
    obstacle_count: usize,
    cap: usize,
}

impl IntervalBudget {
    pub(super) fn new(obstacle_count: usize, cap: usize) -> Self {
        Self {
            count: 0,
            obstacle_count,
            cap,
        }
    }

    fn freeze(
        &mut self,
        intervals: &[TaggedSpan<'_>],
        work: &OperationWorkMeter,
    ) -> Result<Vec<Span>, GridTopologyError> {
        let count = checked_add(self.count, intervals.len(), work)?;
        check_estimate(
            estimate(0, 0, self.obstacle_count, count, work)?,
            self.cap,
            work,
        )?;
        let mut spans = reserved(intervals.len(), work)?;
        spans.extend(intervals.iter().map(|interval| interval.span));
        self.count = count;
        Ok(spans)
    }
}

pub(super) fn obstacle_union<'a>(
    input: GridTopologyInput<'_, 'a>,
    domain: Rect,
    max_bytes: usize,
    work: &OperationWorkMeter,
) -> Result<Vec<GridTopologyObstacle<'a>>, GridTopologyError> {
    let mut inflated = reserved(
        checked_add(input.obstacles.len(), input.title_exclusions.len(), work)?,
        work,
    )?;
    for obstacle in input.obstacles.iter().chain(input.title_exclusions) {
        work.charge(1)?;
        let bounds = normalize_rect(obstacle.bounds)?;
        let bounds = Rect::new(
            domain.x0.max(bounds.x0 - CLEARANCE),
            domain.y0.max(bounds.y0 - CLEARANCE),
            domain.x1.min(bounds.x1 + CLEARANCE),
            domain.y1.min(bounds.y1 + CLEARANCE),
        );
        if bounds.x0 < bounds.x1 && bounds.y0 < bounds.y1 {
            inflated.push(GridTopologyObstacle {
                bounds,
                ..*obstacle
            });
        }
    }

    let mut sweep = Sweep::new(&inflated, Axis::Vertical, work)?;
    let mut spans = reserved(inflated.len(), work)?;
    let mut previous: Vec<usize> = reserved(inflated.len(), work)?;
    let mut current = reserved(inflated.len(), work)?;
    let mut slabs: Vec<GridTopologyObstacle<'a>> = Vec::new();
    while let Some(left) = sweep.leave(work)? {
        sweep.enter(left, work)?;
        let Some(right) = sweep.next_coordinate() else {
            break;
        };
        sweep.merge_spans(&inflated, Axis::Vertical, &mut spans, work)?;
        let mut previous_slots = previous.iter().copied().peekable();
        current.clear();
        for interval in &spans {
            work.charge(1)?;
            let old_slot = loop {
                let Some(slot) = previous_slots.peek().copied() else {
                    break None;
                };
                work.charge(1)?;
                match varying_span(slabs[slot].bounds, Axis::Vertical).compare(interval.span) {
                    Ordering::Less => {
                        previous_slots.next();
                    }
                    Ordering::Equal => break previous_slots.next(),
                    Ordering::Greater => break None,
                }
            };
            if let Some(slot) = old_slot {
                let slab = &mut slabs[slot];
                slab.bounds.x1 = right;
                work.charge(checked_add(
                    slab.id.len().min(interval.owner.len()),
                    1,
                    work,
                )?)?;
                if code_units(interval.owner, slab.id).is_lt() {
                    slab.id = interval.owner;
                }
                current.push(slot);
            } else {
                let count = checked_add(slabs.len(), 1, work)?;
                // A slab may be extended later. Once created, it is never removed from the union.
                check_estimate(estimate(0, 0, count, 0, work)?, max_bytes, work)?;
                slabs.try_reserve(1).map_err(|_| allocation_failed())?;
                current.push(slabs.len());
                slabs.push(GridTopologyObstacle {
                    id: interval.owner,
                    bounds: Rect::new(left, interval.span.low, right, interval.span.high),
                });
            }
        }
        std::mem::swap(&mut previous, &mut current);
    }

    sort_metered(
        &mut slabs,
        checked_add(sweep.max_id_len, 1, work)?,
        |a, b| {
            coordinate_order(a.bounds.y0, b.bounds.y0)
                .then_with(|| coordinate_order(a.bounds.x0, b.bounds.x0))
                .then_with(|| coordinate_order(a.bounds.y1, b.bounds.y1))
                .then_with(|| coordinate_order(a.bounds.x1, b.bounds.x1))
                .then_with(|| code_units(a.id, b.id))
        },
        work,
    )?;
    Ok(slabs)
}

#[derive(Clone, Copy)]
struct Event {
    coordinate: f64,
    starts: bool,
    index: usize,
}

/// Events are sorted by coordinate, with exits before entries. A dense obstacle-slot index
/// lets the active set use fallible vectors and constant-time membership updates.
struct Sweep {
    events: Vec<Event>,
    after: usize,
    active: Vec<usize>,
    positions: Vec<usize>,
    max_id_len: usize,
}

impl Sweep {
    fn new(
        obstacles: &[GridTopologyObstacle<'_>],
        axis: Axis,
        work: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let mut events = reserved(checked_mul(obstacles.len(), 2, work)?, work)?;
        let mut max_id_len = 0;
        for (index, obstacle) in obstacles.iter().enumerate() {
            work.charge(1)?;
            let fixed = varying_span(obstacle.bounds, other_axis(axis));
            events.extend([
                Event {
                    coordinate: fixed.low,
                    starts: true,
                    index,
                },
                Event {
                    coordinate: fixed.high,
                    starts: false,
                    index,
                },
            ]);
            max_id_len = max_id_len.max(obstacle.id.len());
        }
        sort_metered(
            &mut events,
            1,
            |a, b| {
                coordinate_order(a.coordinate, b.coordinate)
                    .then_with(|| a.starts.cmp(&b.starts))
                    .then_with(|| a.index.cmp(&b.index))
            },
            work,
        )?;
        let mut positions = reserved(obstacles.len(), work)?;
        positions.resize(obstacles.len(), 0);
        Ok(Self {
            events,
            after: 0,
            active: reserved(obstacles.len(), work)?,
            positions,
            max_id_len,
        })
    }

    fn next_coordinate(&self) -> Option<f64> {
        self.events.get(self.after).map(|event| event.coordinate)
    }

    fn leave(&mut self, work: &OperationWorkMeter) -> crate::Result<Option<f64>> {
        let Some(coordinate) = self.next_coordinate() else {
            return Ok(None);
        };
        while let Some(event) = self.events.get(self.after)
            && event.coordinate == coordinate
            && !event.starts
        {
            work.charge(1)?;
            let position = self.positions[event.index];
            self.active.swap_remove(position);
            if let Some(&replacement) = self.active.get(position) {
                self.positions[replacement] = position;
            }
            self.after += 1;
        }
        Ok(Some(coordinate))
    }

    fn enter(&mut self, coordinate: f64, work: &OperationWorkMeter) -> crate::Result<()> {
        while let Some(event) = self.events.get(self.after)
            && event.coordinate == coordinate
        {
            work.charge(1)?;
            self.positions[event.index] = self.active.len();
            self.active.push(event.index);
            self.after += 1;
        }
        Ok(())
    }

    fn merge_spans<'a>(
        &self,
        obstacles: &[GridTopologyObstacle<'a>],
        axis: Axis,
        buffer: &mut Vec<TaggedSpan<'a>>,
        work: &OperationWorkMeter,
    ) -> crate::Result<()> {
        work.charge(self.active.len())?;
        buffer.clear();
        buffer.extend(self.active.iter().map(|&index| TaggedSpan {
            span: varying_span(obstacles[index].bounds, axis),
            owner: obstacles[index].id,
        }));
        sort_metered(
            buffer,
            checked_add(self.max_id_len, 1, work)?,
            |a, b| {
                a.span
                    .compare(b.span)
                    .then_with(|| code_units(a.owner, b.owner))
            },
            work,
        )?;
        let mut written = 0;
        for read in 0..buffer.len() {
            work.charge(1)?;
            let next = buffer[read];
            if written > 0 && next.span.low <= buffer[written - 1].span.high {
                let previous = &mut buffer[written - 1];
                previous.span.high = previous.span.high.max(next.span.high);
                work.charge(checked_add(
                    next.owner.len().min(previous.owner.len()),
                    1,
                    work,
                )?)?;
                if code_units(next.owner, previous.owner).is_lt() {
                    previous.owner = next.owner;
                }
            } else {
                buffer[written] = next;
                written += 1;
            }
        }
        buffer.truncate(written);
        Ok(())
    }
}

fn varying_span(bounds: Rect, axis: Axis) -> Span {
    match axis {
        Axis::Horizontal => Span {
            low: bounds.x0,
            high: bounds.x1,
        },
        Axis::Vertical => Span {
            low: bounds.y0,
            high: bounds.y1,
        },
    }
}

fn other_axis(axis: Axis) -> Axis {
    match axis {
        Axis::Horizontal => Axis::Vertical,
        Axis::Vertical => Axis::Horizontal,
    }
}
