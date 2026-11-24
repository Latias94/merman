mod trails;

pub(super) use trails::Trails;

use super::visibility::Node;
use super::{OrthoRequest, OrthoSegment, at, axis_id, other_axis, project, reset};
use crate::model::{Axis, Point, RoutePoint, Side};
use crate::work::{LayoutError, LayoutResult, Work, WorkError, push, reserved};
use std::{cmp::Reverse, collections::BinaryHeap};

const BEND_COST: f64 = 40.0;
const AXES: [Axis; 2] = [Axis::Horizontal, Axis::Vertical];

pub(super) fn distance(a: Point, b: Point) -> f64 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}

fn turn(from: Option<Axis>, to: Option<Axis>) -> f64 {
    if matches!((from, to), (Some(a), Some(b)) if a != b) {
        BEND_COST
    } else {
        0.0
    }
}

fn lower_bound(from: Point, to: Point, incoming: Option<Axis>, outgoing: Option<Axis>) -> f64 {
    // Cover every required displacement axis with the fewest possible switches.
    let bends = |first: Axis, last: Axis| {
        turn(incoming, Some(first)) + turn(Some(first), Some(last)) + turn(Some(last), outgoing)
    };
    let bend_cost = match (from.x == to.x, from.y == to.y) {
        (false, false) => {
            bends(Axis::Horizontal, Axis::Vertical).min(bends(Axis::Vertical, Axis::Horizontal))
        }
        (true, false) => bends(Axis::Vertical, Axis::Vertical),
        (false, true) => bends(Axis::Horizontal, Axis::Horizontal),
        (true, true) => 0.0,
    };
    distance(from, to) + bend_cost
}

fn leaves(side: Option<Side>, from: Point, toward: Point) -> bool {
    match side {
        Some(Side::Top) => from.x != toward.x || toward.y <= from.y,
        Some(Side::Right) => from.y != toward.y || toward.x >= from.x,
        Some(Side::Bottom) => from.x != toward.x || toward.y >= from.y,
        Some(Side::Left) => from.y != toward.y || toward.x <= from.x,
        None => true,
    }
}

pub(super) struct Quick {
    via: Option<RoutePoint>,
    cost: f64,
}

impl Quick {
    pub(super) fn reaches_lower_bound(&self, request: OrthoRequest) -> bool {
        self.cost
            <= lower_bound(
                at(request.start),
                at(request.end),
                request.start_side.map(Side::axis),
                request.end_side.map(Side::axis),
            )
    }

    pub(super) fn emit(&self, request: OrthoRequest) -> LayoutResult<Vec<RoutePoint>> {
        let mut points = reserved(2 + usize::from(self.via.is_some()))?;
        points.push(request.start);
        points.extend(self.via);
        points.push(request.end);
        Ok(points)
    }
}

pub(super) fn quick(
    request: OrthoRequest,
    rays: [[OrthoSegment; 2]; 2],
    trails: &Trails,
    work: &mut Work<'_>,
) -> LayoutResult<Option<Quick>> {
    let (a, b) = (at(request.start), at(request.end));
    let mut best: Option<Quick> = None;
    let mut candidate = |via: Option<Point>, first_axis: Axis| -> LayoutResult<()> {
        let first_end = via.unwrap_or(b);
        let last_start = via.unwrap_or(a);
        if !leaves(request.start_side, a, first_end) || !leaves(request.end_side, b, last_start) {
            return Ok(());
        }
        let first = rays[0][axis_id(first_axis)];
        let (along, _) = project(first_axis, first_end);
        if along < first.min || along > first.max {
            return Ok(());
        }
        let last_axis = if via.is_some() {
            other_axis(first_axis)
        } else {
            first_axis
        };
        let last = rays[1][axis_id(last_axis)];
        let (along, _) = project(last_axis, last_start);
        if along < last.min || along > last.max {
            return Ok(());
        }
        let mut cost = distance(a, b)
            + turn(request.start_side.map(Side::axis), Some(first_axis))
            + turn(Some(first_axis), Some(last_axis))
            + turn(Some(last_axis), request.end_side.map(Side::axis));
        for (p, q) in [(a, first_end), (last_start, b)]
            .into_iter()
            .take(1 + usize::from(via.is_some()))
        {
            let leg = OrthoSegment::from_points(p, q).expect("nonzero orthogonal shortcut");
            cost += trails.cost(leg, None, work)?;
        }
        if cost.is_finite() && best.as_ref().is_none_or(|old| cost < old.cost) {
            best = Some(Quick {
                cost,
                via: via.map(|p| RoutePoint::new(p.x, p.y, first_axis)),
            });
        }
        Ok(())
    };
    if let Some(leg) = OrthoSegment::from_points(a, b) {
        candidate(None, leg.axis)?;
    } else {
        candidate(Some(Point { x: a.x, y: b.y }), Axis::Vertical)?;
        candidate(Some(Point { x: b.x, y: a.y }), Axis::Horizontal)?;
    }
    Ok(best)
}

#[derive(Clone, Copy)]
struct Best {
    cost: f64,
    parent: Option<usize>,
    settled: bool,
}

const UNREACHED: Best = Best {
    cost: f64::INFINITY,
    parent: None,
    settled: false,
};

#[derive(Default)]
pub(super) struct Search {
    best: Vec<Best>,
    queue: BinaryHeap<Reverse<(u64, u64, usize)>>,
    chain: Vec<Point>,
}

impl Search {
    pub(super) fn route(
        &mut self,
        nodes: &[Node],
        [source, target]: [usize; 2],
        request: OrthoRequest,
        trails: &Trails,
        quick: Option<&Quick>,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Vec<RoutePoint>>> {
        let states = nodes
            .len()
            .checked_mul(2)
            .ok_or(WorkError::ArithmeticOverflow)?;
        reset(&mut self.best, states, UNREACHED, work)?;
        self.queue.clear();
        let end = nodes[target].at;
        let first_axis = request.start_side.map(Side::axis);
        let final_axis = request.end_side.map(Side::axis);
        let cutoff = quick.map_or(f64::INFINITY, |path| path.cost);
        for axis in AXES {
            if first_axis.is_none_or(|expected| axis == expected) {
                let state = source * 2 + axis_id(axis);
                self.best[state].cost = 0.0;
                self.enqueue(
                    state,
                    0.0,
                    lower_bound(nodes[source].at, end, Some(axis), final_axis),
                    work,
                )?;
            }
        }
        while let Some(Reverse((estimate, cost, state))) = self.queue.pop() {
            work.visit(self.queue.len().checked_ilog2().unwrap_or(0) as usize + 1)?;
            if f64::from_bits(estimate) >= cutoff {
                break;
            }
            let best = self.best[state];
            if best.settled || best.cost.to_bits() != cost {
                continue;
            }
            self.best[state].settled = true;
            let node = state / 2;
            if node == target {
                return self.emit(state, nodes, request, work).map(Some);
            }
            let incoming = AXES[state % 2];
            let from = nodes[node].at;
            for (direction, next) in nodes[node].adjacent.into_iter().enumerate() {
                let Some(next) = next.filter(|&n| n != source) else {
                    continue;
                };
                let axis = AXES[direction / 2];
                let next_state = next * 2 + axis_id(axis);
                let to = nodes[next].at;
                if self.best[next_state].settled
                    || (node == source && !leaves(request.start_side, from, to))
                    || (next == target && !leaves(request.end_side, to, from))
                {
                    continue;
                }
                let leg = OrthoSegment::from_points(from, to)
                    .expect("adjacent visibility vertices are orthogonal");
                let mut price = best.cost + (leg.max - leg.min) + turn(Some(incoming), Some(axis));
                if next == target {
                    price += turn(Some(axis), final_axis);
                }
                if price >= self.best[next_state].cost || price >= cutoff {
                    continue;
                }
                // A straight leg split by a lattice vertex still crosses exactly once.
                let include = (node != source && incoming == axis).then(|| project(axis, from).0);
                price += trails.cost(leg, include, work)?;
                if price >= self.best[next_state].cost {
                    continue;
                }
                let estimate = price + lower_bound(to, end, Some(axis), final_axis);
                if estimate < cutoff {
                    self.best[next_state] = Best {
                        cost: price,
                        parent: Some(state),
                        settled: false,
                    };
                    self.enqueue(next_state, price, estimate, work)?;
                }
            }
        }
        Ok(None)
    }

    fn enqueue(
        &mut self,
        state: usize,
        price: f64,
        estimate: f64,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        work.visit(self.queue.len().checked_ilog2().unwrap_or(0) as usize + 1)?;
        self.queue
            .try_reserve(1)
            .map_err(|_| LayoutError::AllocationLimit)?;
        self.queue
            .push(Reverse((estimate.to_bits(), price.to_bits(), state)));
        Ok(())
    }

    fn emit(
        &mut self,
        mut state: usize,
        nodes: &[Node],
        request: OrthoRequest,
        work: &mut Work<'_>,
    ) -> LayoutResult<Vec<RoutePoint>> {
        self.chain.clear();
        loop {
            work.visit(1)?;
            push(&mut self.chain, nodes[state / 2].at)?;
            let Some(parent) = self.best[state].parent else {
                break;
            };
            state = parent;
        }
        let mut points = reserved(2)?;
        points.push(request.start);
        for triple in self.chain.windows(3).rev() {
            work.visit(1)?;
            let (previous, p, next) = (triple[2], triple[1], triple[0]);
            let incoming =
                OrthoSegment::from_points(previous, p).expect("path uses adjacent vertices");
            let outgoing = OrthoSegment::from_points(p, next).expect("path uses adjacent vertices");
            let reversal = incoming.min.max(outgoing.min) < incoming.max.min(outgoing.max);
            if incoming.axis != outgoing.axis || reversal {
                push(&mut points, RoutePoint::new(p.x, p.y, incoming.axis))?;
            }
        }
        push(&mut points, request.end)?;
        Ok(points)
    }
}
