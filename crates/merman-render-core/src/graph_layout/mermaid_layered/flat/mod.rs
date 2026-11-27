//! Measured, single-level Mermaid 12.4.0 Layered. All coordinates are in solver axes.

use std::ops::Range;

use super::{Point, SizeExt};

use super::level::{
    self, Charge, CompoundError, LevelPlan, LevelResult, Mode, NodeKind, OrderedLevel, PlacedLevel,
    RankPoint, Result,
};

mod crossings;
mod order;
mod placement;
mod ranking;

use super::work::{self, Work, add, check_slots};

/// Solve a borrowed level in source slot order, charging the caller's cumulative meter.
pub fn solve<E>(
    plan: LevelPlan<'_>,
    mode: Mode,
    charge: &mut Charge<'_, E>,
) -> Result<LevelResult, E> {
    let mut work = Work::new(charge);
    work.price(0)?;
    validate(plan, mode, &mut work)?;
    let result = if mode == Mode::Orient {
        let ranked = ranking::rank(plan, 1, &mut work)?;
        let reversed = work.map(&ranked.edges, |edge| edge.reversed)?;
        LevelResult::Oriented(reversed)
    } else if plan.nodes.is_empty()
        || (plan.nodes.len() == 1 && plan.edges.is_empty() && plan.nodes[0].kind == NodeKind::Node)
    {
        // The source exits before ordering/placement for an empty or ordinary singleton level.
        let point = |n: &super::level::LevelNode| Point {
            x: n.size.width / 2.0,
            y: n.size.height / 2.0,
        };
        if mode == Mode::OrderOnly {
            LevelResult::Ordered(OrderedLevel {
                nodes: work.map(plan.nodes, |n| RankPoint {
                    rank: 0,
                    cross: point(n).x,
                })?,
                edges: Vec::new(),
                crossings: 0.0,
                declared_crossings: 0.0,
            })
        } else {
            LevelResult::Placed(PlacedLevel {
                size: plan.nodes.first().map_or_else(Default::default, |n| n.size),
                centers: work.map(plan.nodes, point)?,
                edges: Vec::new(),
            })
        }
    } else {
        let step = ranking::step(plan, &mut work)?;
        let mut ranked = ranking::rank(plan, step, &mut work)?;
        ranked.plan_expansion(&mut work)?;
        let mut ordered = order::expand(plan, &ranked, &mut work)?;
        crossings::reduce(&mut ordered, &mut work)?;
        placement::finish(plan, &ranked, &mut ordered, mode, &mut work)?
    };
    work.price(0)?;
    Ok(result)
}

fn validate<E>(plan: LevelPlan<'_>, mode: Mode, work: &mut Work<'_, E>) -> Result<(), E> {
    let real = plan.nodes.len();
    check_slots(real)?;
    check_slots(plan.edges.len())?;
    if let Some(groups) = plan.groups {
        check_slots(groups.parent.len())?;
        if mode != Mode::OrderOnly || groups.of.len() != real {
            return Err(inconsistent("invalid ordering membership"));
        }
    }
    work.price(real)?;
    for (i, node) in plan.nodes.iter().enumerate() {
        work.poll(i)?;
        if !node.size.valid()
            || !node.span.is_finite()
            || !(-1..=1).contains(&node.pin)
            || node.key.is_some_and(|key| !key.is_finite())
            || node.hint.is_some_and(|hint| !hint.is_finite())
        {
            return Err(inconsistent("invalid level node"));
        }
    }
    work.price(plan.edges.len())?;
    for (i, e) in plan.edges.iter().enumerate() {
        work.poll(i)?;
        if e.tail >= real || e.head >= real || e.tail == e.head {
            return Err(inconsistent("invalid local edge endpoints"));
        }
        if !e.label.valid()
            || !e.tail_attachment.dx.is_finite()
            || !e.head_attachment.dx.is_finite()
        {
            return Err(inconsistent("invalid local edge geometry"));
        }
    }
    let options = plan.options;
    if [
        options.spacing.node,
        options.spacing.rank,
        options.spacing.edge,
        options.port_spacing,
        options.head_room,
    ]
    .into_iter()
    .any(|n| !n.is_finite() || n < 0.0)
    {
        return Err(inconsistent("invalid level spacing"));
    }
    Ok(())
}

fn inconsistent<E>(message: &'static str) -> CompoundError<E> {
    CompoundError::InconsistentLevel(message)
}

struct Adjacency {
    starts: Vec<usize>,
    slots: Vec<usize>,
}

impl Adjacency {
    fn range(&self, node: usize) -> Range<usize> {
        self.starts[node]..self.starts[node + 1]
    }

    fn at(&self, node: usize) -> &[usize] {
        &self.slots[self.range(node)]
    }

    fn build<E>(
        nodes: usize,
        edges: usize,
        endpoint: impl Fn(usize) -> Option<usize>,
        work: &mut Work<'_, E>,
    ) -> Result<Self, E> {
        let mut starts = work.filled(add(nodes, 1)?, 0usize)?;
        work.price(edges)?;
        for edge in 0..edges {
            work.poll(edge)?;
            if let Some(node) = endpoint(edge) {
                starts[node + 1] += 1;
            }
        }
        prefix(&mut starts, work)?;
        let mut slots = work.filled(starts[nodes], 0usize)?;
        let mut fill = work.copy(&starts[..nodes])?;
        work.price(edges)?;
        for edge in 0..edges {
            work.poll(edge)?;
            if let Some(node) = endpoint(edge) {
                slots[fill[node]] = edge;
                fill[node] += 1;
            }
        }
        Ok(Self { starts, slots })
    }
}

fn prefix<E>(starts: &mut [usize], work: &mut Work<'_, E>) -> Result<(), E> {
    work.price(starts.len())?;
    let mut total = 0;
    for (i, start) in starts.iter_mut().enumerate() {
        work.poll(i)?;
        // Each source slot contributed at most once to this preadmitted table.
        total += *start;
        *start = total;
    }
    Ok(())
}
