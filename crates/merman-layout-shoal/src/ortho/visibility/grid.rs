//! Intersect only the free runs that actually meet. Adjacent intersections on
//! a run share a link. Storage is linear in the runs and capped vertices.

use super::{Bounds, Obstacles};
use crate::model::{Axis, Point};
use crate::ortho::{OrthoSegment, axis_id, reset, sort};
use crate::work::{LayoutError, LayoutResult, Work, WorkError, push};

pub(in crate::ortho) struct Node {
    pub at: Point,
    // Left, right, top, bottom, in drawing coordinates.
    pub adjacent: [Option<usize>; 4],
}

pub(in crate::ortho) struct GridQuery {
    pub bounds: Bounds,
    pub guarded: bool,
    pub anchors: [Point; 2],
    pub rays: [[OrthoSegment; 2]; 2],
    pub cap: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum EventKind {
    Start,
    Vertical,
    End,
}

struct Event {
    x: f64,
    kind: EventKind,
    line: usize,
}

#[derive(Default)]
pub(in crate::ortho) struct Grid {
    pub nodes: Vec<Node>,
    scope: Option<(Bounds, bool)>,
    scoped_lines: Vec<OrthoSegment>,
    lines: Vec<OrthoSegment>,
    blocked: Vec<[f64; 2]>,
    events: Vec<Event>,
    tails: Vec<Option<usize>>,
    active: Vec<bool>,
}

impl Grid {
    pub(in crate::ortho) fn build(
        &mut self,
        corridors: &[OrthoSegment],
        obstacles: &Obstacles,
        query: GridQuery,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<[usize; 2]>> {
        self.nodes.clear();
        self.lines.clear();
        self.events.clear();
        let scope = (query.bounds, query.guarded);
        if self.scope != Some(scope) {
            self.prepare_scope(corridors, obstacles, scope, work)?;
        }
        work.charge(self.scoped_lines.len())?;
        self.lines
            .try_reserve(self.scoped_lines.len())
            .map_err(|_| LayoutError::AllocationLimit)?;
        self.lines.extend_from_slice(&self.scoped_lines);
        for line in query.rays.into_iter().flatten() {
            push(&mut self.lines, line)?;
        }
        merge(&mut self.lines, work)?;
        let horizontal = self
            .lines
            .partition_point(|line| line.axis == Axis::Horizontal);
        let base = horizontal
            .max(1)
            .checked_next_power_of_two()
            .ok_or(WorkError::ArithmeticOverflow)?;
        reset(
            &mut self.active,
            base.checked_mul(2).ok_or(WorkError::ArithmeticOverflow)?,
            false,
            work,
        )?;
        reset(&mut self.tails, horizontal, None, work)?;
        for (line, segment) in self.lines.iter().enumerate() {
            work.visit(1)?;
            if segment.axis == Axis::Horizontal {
                push(
                    &mut self.events,
                    Event {
                        x: segment.min,
                        kind: EventKind::Start,
                        line,
                    },
                )?;
                push(
                    &mut self.events,
                    Event {
                        x: segment.max,
                        kind: EventKind::End,
                        line,
                    },
                )?;
            } else {
                push(
                    &mut self.events,
                    Event {
                        x: segment.fixed,
                        kind: EventKind::Vertical,
                        line,
                    },
                )?;
            }
        }
        sort(
            &mut self.events,
            |a, b| {
                a.x.total_cmp(&b.x)
                    .then_with(|| a.kind.cmp(&b.kind))
                    .then_with(|| a.line.cmp(&b.line))
            },
            work,
        )?;
        let mut terminals = [None; 2];
        let horizontals = &self.lines[..horizontal];
        for event in &self.events {
            work.visit(1)?;
            if event.kind != EventKind::Vertical {
                set(
                    &mut self.active,
                    base + event.line,
                    event.kind == EventKind::Start,
                    work,
                )?;
                continue;
            }
            let line = self.lines[event.line];
            work.visit(horizontal.checked_ilog2().unwrap_or(0) as usize + 1)?;
            let mut cursor = horizontals.partition_point(|h| h.fixed < line.min);
            let mut above = None;
            while let Some(h) = next_active(&self.active, base, cursor, work)? {
                let y = horizontals[h].fixed;
                if y > line.max {
                    break;
                }
                if self.nodes.len() == query.cap {
                    return Ok(None);
                }
                let at = Point { x: event.x, y };
                let id = self.nodes.len();
                let left = self.tails[h].replace(id);
                push(
                    &mut self.nodes,
                    Node {
                        at,
                        adjacent: [left, None, above, None],
                    },
                )?;
                if let Some(left) = left {
                    self.nodes[left].adjacent[1] = Some(id);
                }
                if let Some(top) = above.replace(id) {
                    self.nodes[top].adjacent[3] = Some(id);
                }
                for (i, anchor) in query.anchors.into_iter().enumerate() {
                    if at == anchor {
                        terminals[i] = Some(id);
                    }
                }
                cursor = h + 1;
            }
        }
        Ok(terminals[0].zip(terminals[1]).map(|(a, b)| [a, b]))
    }

    fn prepare_scope(
        &mut self,
        corridors: &[OrthoSegment],
        obstacles: &Obstacles,
        (bounds, guarded): (Bounds, bool),
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        self.scope = None;
        self.scoped_lines.clear();
        for &line in corridors {
            work.visit(1)?;
            let (along, across) = bounds.project(line.axis);
            let min = line.min.max(along[0]);
            let max = line.max.min(along[1]);
            if across[0] <= line.fixed && line.fixed <= across[1] && min < max {
                push(&mut self.scoped_lines, OrthoSegment { min, max, ..line })?;
            }
        }
        self.frame(obstacles, bounds, guarded, work)?;
        merge(&mut self.scoped_lines, work)?;
        self.scope = Some((bounds, guarded));
        Ok(())
    }

    fn frame(
        &mut self,
        obstacles: &Obstacles,
        bounds: Bounds,
        guarded: bool,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        for axis in [Axis::Horizontal, Axis::Vertical] {
            let ([min, max], across) = bounds.project(axis);
            for fixed in across {
                self.blocked.clear();
                if guarded {
                    for &r in &obstacles.rects {
                        work.visit(1)?;
                        let ([a, b], [lo, hi]) = r.project(axis);
                        if lo < fixed && fixed < hi && a < max && min < b {
                            push(&mut self.blocked, [a, b])?;
                        }
                    }
                    sort(&mut self.blocked, |a, b| a[0].total_cmp(&b[0]), work)?;
                }
                let mut start = min;
                for [a, b] in self.blocked.iter().copied().chain([[max, max]]) {
                    work.visit(1)?;
                    if start < a {
                        push(
                            &mut self.scoped_lines,
                            OrthoSegment {
                                axis,
                                fixed,
                                min: start,
                                max: a,
                            },
                        )?;
                    }
                    start = start.max(b);
                }
            }
        }
        Ok(())
    }
}

fn merge(lines: &mut Vec<OrthoSegment>, work: &mut Work<'_>) -> LayoutResult<()> {
    sort(
        lines,
        |a, b| {
            axis_id(a.axis)
                .cmp(&axis_id(b.axis))
                .then_with(|| a.fixed.total_cmp(&b.fixed))
                .then_with(|| a.min.total_cmp(&b.min))
                .then_with(|| a.max.total_cmp(&b.max))
        },
        work,
    )?;
    let mut written = 0usize;
    for read in 0..lines.len() {
        work.visit(1)?;
        let next = lines[read];
        if let Some(i) = written.checked_sub(1) {
            let last = &mut lines[i];
            if last.axis == next.axis && last.fixed == next.fixed && next.min <= last.max {
                last.max = last.max.max(next.max);
                continue;
            }
        }
        lines[written] = next;
        written += 1;
    }
    lines.truncate(written);
    Ok(())
}

fn set(
    active: &mut [bool],
    mut index: usize,
    value: bool,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    active[index] = value;
    while index > 1 {
        work.visit(1)?;
        index /= 2;
        let any = active[index * 2] || active[index * 2 + 1];
        if active[index] == any {
            break;
        }
        active[index] = any;
    }
    Ok(())
}

fn next_active(
    active: &[bool],
    base: usize,
    from: usize,
    work: &mut Work<'_>,
) -> LayoutResult<Option<usize>> {
    if from >= base {
        return Ok(None);
    }
    let mut index = base + from;
    if active[index] {
        return Ok(Some(from));
    }
    while index > 1 {
        work.visit(1)?;
        if index.is_multiple_of(2) && active[index + 1] {
            index += 1;
            while index < base {
                work.visit(1)?;
                index *= 2;
                if !active[index] {
                    index += 1;
                }
            }
            return Ok(Some(index - base));
        }
        index /= 2;
    }
    Ok(None)
}
