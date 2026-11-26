//! Intersect only the free runs that actually meet. Adjacent intersections on
//! a run share a link. Storage is linear in the runs and capped vertices.

use super::{Bounds, Obstacles};
use crate::model::{Axis, Point};
use crate::ortho::{OrthoSegment, axis_id, reset, sort};
use crate::work::{LayoutError, LayoutResult, Work, WorkError, push};
use std::cmp::Ordering;

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

#[derive(Clone, Copy)]
struct Event {
    x: f64,
    kind: EventKind,
    line: usize,
}

impl Event {
    fn order(&self, other: &Self) -> Ordering {
        self.x
            .total_cmp(&other.x)
            .then_with(|| self.kind.cmp(&other.kind))
            .then_with(|| self.line.cmp(&other.line))
    }

    fn append(events: &mut Vec<Self>, line: usize, segment: OrthoSegment) -> LayoutResult<()> {
        if segment.axis == Axis::Horizontal {
            for (x, kind) in [
                (segment.min, EventKind::Start),
                (segment.max, EventKind::End),
            ] {
                push(events, Self { x, kind, line })?;
            }
            Ok(())
        } else {
            push(
                events,
                Self {
                    x: segment.fixed,
                    kind: EventKind::Vertical,
                    line,
                },
            )
        }
    }
}

#[derive(Default)]
pub(in crate::ortho) struct Grid {
    pub nodes: Vec<Node>,
    scope: Option<(Bounds, bool)>,
    scoped_lines: Vec<OrthoSegment>,
    scoped_events: Vec<Event>,
    // Maps a cached run to its current sweep index. A ray can replace the run.
    scoped_ids: Vec<Option<usize>>,
    lines: Vec<OrthoSegment>,
    blocked: Vec<[f64; 2]>,
    ray_events: Vec<Event>,
    tails: Vec<Option<usize>>,
    active: ActiveRuns,
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
        let scope = (query.bounds, query.guarded);
        if self.scope != Some(scope) {
            self.prepare_scope(corridors, obstacles, scope, work)?;
        }
        let [[start_h, start_v], [end_h, end_v]] = query.rays;
        self.merge_rays([start_h, start_v, end_h, end_v], work)?;
        let horizontal = self
            .lines
            .partition_point(|line| line.axis == Axis::Horizontal);
        self.active.reset(horizontal, work)?;
        reset(&mut self.tails, horizontal, None, work)?;
        let mut terminals = [None; 2];
        let horizontals = &self.lines[..horizontal];
        let mut scoped = self.scoped_events.iter().copied();
        let mut rays = self.ray_events.iter().copied().peekable();
        let mut next_scoped = None;
        loop {
            while next_scoped.is_none() {
                let Some(event) = scoped.next() else {
                    break;
                };
                if let Some(line) = self.scoped_ids[event.line] {
                    next_scoped = Some(Event { line, ..event });
                } else {
                    work.visit(1)?;
                }
            }
            let next = if rays
                .peek()
                .is_some_and(|ray| next_scoped.is_none_or(|event| ray.order(&event).is_lt()))
            {
                rays.next()
            } else {
                next_scoped.take()
            };
            let Some(event) = next else {
                break;
            };
            work.visit(1)?;
            if event.kind != EventKind::Vertical {
                self.active
                    .set(event.line, event.kind == EventKind::Start, work)?;
                continue;
            }
            let line = self.lines[event.line];
            work.visit(horizontal.checked_ilog2().unwrap_or(0) as usize + 1)?;
            let mut cursor = horizontals.partition_point(|h| h.fixed < line.min);
            let mut above = None;
            while let Some(h) = self.active.next(cursor, work)? {
                let y = horizontals[h].fixed;
                if y > line.max {
                    break;
                }
                if self.nodes.len() == query.cap {
                    return Ok(None);
                }
                let at = Point { x: event.x, y };
                let id = self.nodes.len();
                if id == self.nodes.capacity() {
                    self.nodes
                        .try_reserve(1)
                        .map_err(|_| LayoutError::AllocationLimit)?;
                }
                let left = self.tails[h].replace(id);
                self.nodes.push(Node {
                    at,
                    adjacent: [left, None, above, None],
                });
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

    fn merge_rays(&mut self, mut rays: [OrthoSegment; 4], work: &mut Work<'_>) -> LayoutResult<()> {
        sort(&mut rays, line_order, work)?;
        self.lines.clear();
        self.ray_events.clear();
        let max_lines = self
            .scoped_lines
            .len()
            .checked_add(rays.len())
            .ok_or(WorkError::ArithmeticOverflow)?;
        reset(&mut self.scoped_ids, self.scoped_lines.len(), None, work)?;
        self.lines
            .try_reserve(max_lines)
            .map_err(|_| LayoutError::AllocationLimit)?;

        let mut scoped = self.scoped_lines.iter().copied().enumerate().peekable();
        let mut rays = rays.into_iter().peekable();
        let mut runs = std::iter::from_fn(|| {
            if rays.peek().is_some_and(|ray| {
                scoped
                    .peek()
                    .is_none_or(|(_, line)| line_order(ray, line).is_lt())
            }) {
                rays.next().map(|line| (line, None))
            } else {
                scoped.next().map(|(id, line)| (line, Some(id)))
            }
        })
        .peekable();
        while let Some((mut line, mut scoped)) = runs.next() {
            work.visit(1)?;
            while let Some(&(next, _)) = runs.peek() {
                if !join(&mut line, next) {
                    break;
                }
                runs.next();
                scoped = None;
                work.visit(1)?;
            }
            let id = self.lines.len();
            self.lines.push(line);
            // Unchanged runs retain their relative order in the cached events. A run
            // joined to any ray replaces all of its old events with at most two new ones.
            if let Some(scoped) = scoped {
                self.scoped_ids[scoped] = Some(id);
            } else {
                Event::append(&mut self.ray_events, id, line)?;
            }
        }
        sort(&mut self.ray_events, Event::order, work)
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
        self.scoped_events.clear();
        for (id, &line) in self.scoped_lines.iter().enumerate() {
            work.visit(1)?;
            Event::append(&mut self.scoped_events, id, line)?;
        }
        sort(&mut self.scoped_events, Event::order, work)?;
        work.charge(0)?;
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

fn line_order(a: &OrthoSegment, b: &OrthoSegment) -> Ordering {
    axis_id(a.axis)
        .cmp(&axis_id(b.axis))
        .then_with(|| a.fixed.total_cmp(&b.fixed))
        .then_with(|| a.min.total_cmp(&b.min))
        .then_with(|| a.max.total_cmp(&b.max))
}

fn join(line: &mut OrthoSegment, next: OrthoSegment) -> bool {
    if line.axis == next.axis && line.fixed == next.fixed && next.min <= line.max {
        line.max = line.max.max(next.max);
        true
    } else {
        false
    }
}

fn merge(lines: &mut Vec<OrthoSegment>, work: &mut Work<'_>) -> LayoutResult<()> {
    sort(lines, line_order, work)?;
    let mut written = 0usize;
    for read in 0..lines.len() {
        work.visit(1)?;
        let next = lines[read];
        if let Some(i) = written.checked_sub(1)
            && join(&mut lines[i], next)
        {
            continue;
        }
        lines[written] = next;
        written += 1;
    }
    lines.truncate(written);
    Ok(())
}

#[derive(Default)]
struct ActiveRuns {
    small: u128,
    tree: Vec<bool>,
    base: usize,
}

impl ActiveRuns {
    fn reset(&mut self, runs: usize, work: &mut Work<'_>) -> LayoutResult<()> {
        self.small = 0;
        self.base = 0;
        // A fixed-width set handles the common small scope. The tree keeps the same
        // logarithmic bound for sparse queries in wider scopes.
        if runs > u128::BITS as usize {
            let base = runs
                .checked_next_power_of_two()
                .ok_or(WorkError::ArithmeticOverflow)?;
            let cells = base.checked_mul(2).ok_or(WorkError::ArithmeticOverflow)?;
            reset(&mut self.tree, cells, false, work)?;
            self.base = base;
        }
        Ok(())
    }

    fn set(&mut self, index: usize, value: bool, work: &mut Work<'_>) -> LayoutResult<()> {
        if self.base == 0 {
            work.visit(1)?;
            let bit = 1_u128 << index;
            self.small = (self.small & !bit) | (u128::from(value) << index);
            Ok(())
        } else {
            set(&mut self.tree, self.base + index, value, work)
        }
    }

    fn next(&self, from: usize, work: &mut Work<'_>) -> LayoutResult<Option<usize>> {
        if self.base == 0 {
            work.visit(1)?;
            let tail = self.small.checked_shr(from as u32).unwrap_or(0);
            Ok((tail != 0).then(|| from + tail.trailing_zeros() as usize))
        } else {
            next_active(&self.tree, self.base, from, work)
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NoopWorkControl;

    #[test]
    fn both_active_sets_find_the_same_next_run_across_word_boundaries() {
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        let mut small = ActiveRuns::default();
        let mut tree = ActiveRuns::default();
        small.reset(128, &mut work).unwrap();
        tree.reset(129, &mut work).unwrap();
        for (index, value) in [(0, true), (63, true), (64, true), (127, true), (64, false)] {
            small.set(index, value, &mut work).unwrap();
            tree.set(index, value, &mut work).unwrap();
            for from in 0..=129 {
                assert_eq!(small.next(from, &mut work), tree.next(from, &mut work));
            }
        }
        tree.reset(128, &mut work).unwrap();
        assert_eq!(tree.next(0, &mut work), Ok(None));
    }
}
