//! Terminal spreading, border port alignment, and bend adjustment.

use super::{
    Along, End, GeoResult, Geometry, GeometryError, IN_LINE, Kind, Options, TWIN, Work,
    compare_number, js_max, js_min,
};

const PORT_PITCH: f64 = 16.0;
const MIN_PITCH: f64 = 8.0;
const SNAP: f64 = 12.0;
const BEND_SNAP: f64 = 24.0;
const LEAN_CLEAR: f64 = 40.0;

pub(super) struct SideFit {
    node: usize,
    along: Along,
    span: f64,
    free: Vec<usize>,
    gaps: Vec<f64>,
}

pub(super) fn spread<E>(
    graph: &mut Geometry<'_>,
    work: &mut Work<'_, E>,
) -> GeoResult<Vec<SideFit>, E> {
    let mut sides = Vec::new();
    work.price(graph.real())?;
    for node in 0..graph.real() {
        work.poll(node)?;
        let span = graph.span(node);
        if span <= 0.0 || graph.order.vertices[node].kind != Kind::Node {
            continue;
        }
        for along in [Along::Up, Along::Down] {
            let all = graph.adjacent(node, along);
            let mut free = Vec::new();
            // Adjacency already uses the opposite position, then the segment ordinal.
            work.price(all.len())?;
            for (i, &edge) in all.iter().enumerate() {
                work.poll(i)?;
                if graph.end(edge, along) != End::Held {
                    work.push(&mut free, edge)?;
                }
            }
            if free.is_empty() {
                continue;
            }
            let mut gaps = work.filled(free.len(), 0.0)?;
            let mut total = fill_gaps(graph, &free, along, &mut gaps, 0, work)?;
            if span / total < MIN_PITCH {
                total = fill_gaps(graph, &free, along, &mut gaps, 1, work)?;
            }
            if span / total < MIN_PITCH {
                total = fill_gaps(graph, &free, along, &mut gaps, 2, work)?;
            }
            if total != 0.0 {
                let pitch = js_min(PORT_PITCH, span / total);
                let mut at = -(total * pitch) / 2.0;
                work.price(gaps.len())?;
                for (index, gap) in gaps.iter_mut().enumerate() {
                    work.poll(index)?;
                    if index > 0 {
                        *gap *= pitch;
                    }
                    at += *gap;
                    graph.set_offset(free[index], along, at);
                }
            }
            work.push(
                &mut sides,
                SideFit {
                    node,
                    along,
                    span,
                    free,
                    gaps,
                },
            )?;
        }
    }
    Ok(sides)
}

fn fill_gaps<E>(
    graph: &Geometry<'_>,
    edges: &[usize],
    along: Along,
    gaps: &mut [f64],
    reduced: usize,
    work: &mut Work<'_, E>,
) -> GeoResult<f64, E> {
    let mut total = 0.0;
    work.price(edges.len().saturating_sub(1))?;
    for (i, pair) in edges.windows(2).enumerate() {
        work.poll(i)?;
        let prior = graph.end(pair[0], along);
        let next = graph.end(pair[1], along);
        let gap = if reduced == 2 && prior == next {
            0.0
        } else if reduced == 1 && prior == End::Leaves && next == End::Leaves {
            0.5
        } else {
            1.0
        };
        gaps[i + 1] = gap;
        total += gap;
    }
    Ok(total)
}

pub(super) fn empty_ranks<E>(
    graph: &Geometry<'_>,
    work: &mut Work<'_, E>,
) -> GeoResult<Vec<bool>, E> {
    let mut empty = work.filled(graph.rows(), false)?;
    let mut any = false;
    let max_rank = graph.rows() - 1;
    work.price(max_rank.saturating_sub(1))?;
    for (r, at) in empty.iter_mut().enumerate().take(max_rank).skip(1) {
        work.poll(r - 1)?;
        let row = graph.row(r);
        let mut only = !row.is_empty();
        work.price(row.len())?;
        for (i, &node) in row.iter().enumerate() {
            work.poll(i)?;
            if graph.order.vertices[node].kind != Kind::Bend {
                only = false;
                break;
            }
        }
        *at = only;
        any |= only;
    }
    if any { Ok(empty) } else { Ok(Vec::new()) }
}

fn empty_at(empty: &[bool], rank: usize) -> bool {
    empty.get(rank).copied().unwrap_or(false)
}

fn follow<E>(
    graph: &Geometry<'_>,
    empty: &[bool],
    mut edge: usize,
    along: Along,
    work: &mut Work<'_, E>,
) -> GeoResult<usize, E> {
    if empty.is_empty() {
        return Ok(edge);
    }
    for _ in 0..100_000 {
        work.price(1)?;
        let u = graph.other(edge, along);
        let vertex = graph.order.vertices[u];
        if vertex.kind != Kind::Bend || !empty[vertex.rank] {
            break;
        }
        edge = graph.adjacent(u, along)[0];
    }
    Ok(edge)
}

pub(super) fn align_ports<E>(
    graph: &Geometry<'_>,
    xs: &mut [f64],
    empty: &[bool],
    work: &mut Work<'_, E>,
) -> GeoResult<(), E> {
    let mut positions = Vec::new();
    for (rank, kind, along) in [
        (0, Kind::StartPort, Along::Down),
        (graph.rows() - 1, Kind::EndPort, Along::Up),
    ] {
        let row = graph.row(rank);
        positions.clear();
        work.price(row.len())?;
        for (i, &node) in row.iter().enumerate() {
            work.poll(i)?;
            let [edge] = graph.adjacent(node, along) else {
                break;
            };
            if graph.order.vertices[node].kind != kind {
                break;
            }
            let e = follow(graph, empty, *edge, along, work)?;
            let x = graph.other_point(xs, e, along);
            if positions.last().is_some_and(|&prior| x < prior + TWIN) {
                break;
            }
            work.push(&mut positions, x)?;
        }
        if positions.len() == row.len() {
            work.price(row.len())?;
            for (i, (&node, &x)) in row.iter().zip(&positions).enumerate() {
                work.poll(i)?;
                xs[node] = x;
            }
        }
    }
    Ok(())
}

pub(super) fn align_labels<E>(
    graph: &Geometry<'_>,
    xs: &mut [f64],
    empty: &[bool],
    options: Options,
    work: &mut Work<'_, E>,
) -> GeoResult<(), E> {
    let mut done = work.filled(xs.len(), false)?;
    let mut chain = Vec::new();
    let placed = |v: usize| {
        let node = graph.order.vertices[v];
        node.kind == Kind::Label || (node.kind == Kind::Bend && !empty_at(empty, node.rank))
    };
    let max_rank = graph.rows() - 1;
    work.price(max_rank.saturating_sub(1))?;
    for r in 1..max_rank {
        work.poll(r - 1)?;
        let row = graph.row(r);
        work.price(row.len())?;
        for (i, &v) in row.iter().enumerate() {
            work.poll(i)?;
            if !placed(v) || done[v] {
                continue;
            }
            let mut top = v;
            loop {
                work.price(1)?;
                let u = graph.other(graph.adjacent(top, Along::Up)[0], Along::Up);
                if !graph.order.vertices[u].kind.is_dummy() {
                    break;
                }
                top = u;
            }
            let mut bottom = top;
            let (mut lo, mut hi) = (f64::NEG_INFINITY, f64::INFINITY);
            chain.clear();
            loop {
                work.price(1)?;
                let d = bottom;
                if placed(d) {
                    done[d] = true;
                    work.push(&mut chain, d)?;
                    let (least, most) = label_room(graph, xs, options, d);
                    if least > lo {
                        lo = least;
                    }
                    if most < hi {
                        hi = most;
                    }
                }
                let u = graph.other(graph.adjacent(d, Along::Down)[0], Along::Down);
                if !graph.order.vertices[u].kind.is_dummy() {
                    break;
                }
                bottom = u;
            }
            if lo <= hi {
                let up = graph.adjacent(top, Along::Up)[0];
                let down = graph.adjacent(bottom, Along::Down)[0];
                let x = choose_label(graph, xs, xs[chain[0]], up, down, (lo, hi));
                work.price(chain.len())?;
                for (j, &d) in chain.iter().enumerate() {
                    work.poll(j)?;
                    xs[d] = x;
                }
            } else {
                work.price(chain.len())?;
                for (j, &d) in chain.iter().enumerate() {
                    work.poll(j)?;
                    let room = label_room(graph, xs, options, d);
                    if room.0 > room.1 {
                        continue;
                    }
                    let up = follow(
                        graph,
                        empty,
                        graph.adjacent(d, Along::Up)[0],
                        Along::Up,
                        work,
                    )?;
                    let down = follow(
                        graph,
                        empty,
                        graph.adjacent(d, Along::Down)[0],
                        Along::Down,
                        work,
                    )?;
                    xs[d] = choose_label(graph, xs, xs[d], up, down, room);
                }
            }
        }
    }
    Ok(())
}

fn label_room(graph: &Geometry<'_>, xs: &[f64], options: Options, node: usize) -> (f64, f64) {
    let v = graph.order.vertices[node];
    let row = graph.row(v.rank);
    let i = graph.order.positions[node];
    let clear = |other: usize| {
        let u = graph.order.vertices[other];
        let gap = if u.kind == Kind::Node {
            options.node_sep / 2.0 + options.edge_sep / 2.0
        } else if u.kind == v.kind {
            options.edge_sep
        } else {
            options.edge_sep / 2.0
        };
        u.size.width / 2.0 + v.size.width / 2.0 + gap
    };
    let lo = if i > 0 {
        xs[row[i - 1]] + clear(row[i - 1])
    } else {
        f64::NEG_INFINITY
    };
    let hi = if i + 1 < row.len() {
        xs[row[i + 1]] - clear(row[i + 1])
    } else {
        f64::INFINITY
    };
    (lo, hi)
}

fn choose_label(
    graph: &Geometry<'_>,
    xs: &[f64],
    x: f64,
    up: usize,
    down: usize,
    (lo, hi): (f64, f64),
) -> f64 {
    let a = graph.other_point(xs, up, Along::Up);
    let b = graph.other_point(xs, down, Along::Down);
    let da = graph.free(graph.other(up, Along::Up), Along::Down);
    let db = graph.free(graph.other(down, Along::Down), Along::Up);
    let up_first = if da != db {
        da < db
    } else {
        (a - x).abs() <= (b - x).abs()
    };
    let (first, second) = if up_first { (a, b) } else { (b, a) };
    if first >= lo && first <= hi {
        first
    } else if second >= lo && second <= hi {
        second
    } else {
        js_max(lo, js_min(hi, first))
    }
}

fn wanted<E>(
    graph: &Geometry<'_>,
    xs: &[f64],
    empty: &[bool],
    side: &SideFit,
    edge: usize,
    again: bool,
    work: &mut Work<'_, E>,
) -> GeoResult<f64, E> {
    let SideFit {
        node, along, span, ..
    } = *side;
    if side.free.len() < 2 {
        return Ok(0.0);
    }
    let edge = follow(graph, empty, edge, along, work)?;
    let u = graph.other(edge, along);
    let point = graph.other_point(xs, edge, along);
    let x = xs[node];
    if again && (point - x).abs() <= span / 2.0 + IN_LINE {
        return Ok(js_max(-span / 2.0, js_min(span / 2.0, point - x)));
    }
    let mut middle = point;
    let mut width = 0.0;
    if graph.end(edge, along.other()) != End::Held && graph.free(u, along.other()) >= 2 {
        width = js_max(0.0, graph.span(u));
        if width > 0.0 {
            middle = xs[u];
        }
    }
    let lo = js_max(x - span / 2.0, middle - width / 2.0);
    let hi = js_min(x + span / 2.0, middle + width / 2.0);
    if lo > hi + IN_LINE {
        let lean = js_max(0.0, span / 2.0 - LEAN_CLEAR);
        return Ok(js_max(-lean, js_min(lean, point - x)));
    }
    let place = (x * width + middle * span) / (width + span);
    Ok(js_max(lo, js_min(hi, place)) - x)
}

pub(super) fn settle<E>(
    graph: &mut Geometry<'_>,
    xs: &[f64],
    empty: &[bool],
    sides: &[SideFit],
    work: &mut Work<'_, E>,
) -> GeoResult<(), E> {
    let mut pools: Vec<(f64, usize)> = Vec::new();
    for pass in 0..6 {
        let mut moved = false;
        work.price(sides.len())?;
        for (slot, side) in sides.iter().enumerate() {
            work.poll(slot)?;
            moved |= fit_side(graph, xs, empty, side, pass > 0, &mut pools, work)?;
        }
        if !moved {
            break;
        }
    }
    Ok(())
}

fn fit_side<E>(
    graph: &mut Geometry<'_>,
    xs: &[f64],
    empty: &[bool],
    side: &SideFit,
    again: bool,
    pools: &mut Vec<(f64, usize)>,
    work: &mut Work<'_, E>,
) -> GeoResult<bool, E> {
    let len = graph.adjacent(side.node, side.along).len();
    let mut moved = false;
    let mut lo = -side.span / 2.0;
    let mut first = 0;
    let mut next = 0;
    work.price(len + 1)?;
    for i in 0..=len {
        work.poll(i)?;
        let held = if i < len {
            let edge = graph.adjacent(side.node, side.along)[i];
            if graph.end(edge, side.along) != End::Held {
                next += 1;
                continue;
            }
            Some(graph.offset(edge, side.along))
        } else {
            None
        };
        if next > first {
            let hi = held.map_or(side.span / 2.0, |at| at - PORT_PITCH);
            let mut before = 0.0;
            pools.clear();
            work.price(next - first)?;
            for j in first..next {
                work.poll(j - first)?;
                let gap = side.gaps[j];
                if j > first {
                    before += gap;
                }
                let desired = wanted(graph, xs, empty, side, side.free[j], again, work)?;
                work.push(pools, (desired - before, 1))?;
                let mut together = j > first && gap == 0.0;
                while pools.len() > 1 {
                    let len = pools.len();
                    let (a, ac) = pools[len - 2];
                    let (b, bc) = pools[len - 1];
                    let overlaps = a / ac as f64 > b / bc as f64;
                    if !together && !overlaps {
                        break;
                    }
                    work.price(1)?;
                    together = false;
                    pools.pop();
                    pools[len - 2] = (a + b, ac + bc);
                }
            }
            let total = before;
            let mut j = first;
            before = 0.0;
            work.price(next - first)?;
            for &(sum, count) in pools.iter() {
                let level = if hi - total < lo {
                    (lo + hi - total) / 2.0
                } else {
                    js_max(lo, js_min(hi - total, sum / count as f64))
                };
                for _ in 0..count {
                    work.poll(j - first)?;
                    if j > first {
                        before += side.gaps[j];
                    }
                    let edge = side.free[j];
                    let dx = graph.offset(edge, side.along);
                    moved |= (dx - level - before).abs() > 0.01;
                    let new_dx = level + before;
                    if !new_dx.is_finite() {
                        return Err(GeometryError::NonFiniteGeometry);
                    }
                    graph.set_offset(edge, side.along, new_dx);
                    j += 1;
                }
            }
            first = next;
        }
        if let Some(held) = held {
            lo = held + PORT_PITCH;
        }
    }
    Ok(moved)
}

struct SnapRun {
    from: usize,
    to: usize,
    from_dx: f64,
    to_dx: f64,
}

impl SnapRun {
    fn offset(&self, xs: &[f64]) -> f64 {
        xs[self.to] + self.to_dx - xs[self.from] - self.from_dx
    }
}

pub(super) fn snap<E>(
    graph: &Geometry<'_>,
    xs: &mut [f64],
    empty: &[bool],
    options: Options,
    work: &mut Work<'_, E>,
) -> GeoResult<(), E> {
    let mut runs = work.reserve(graph.segments().len())?;
    work.price(graph.segments().len())?;
    for (edge, segment) in graph.segments().iter().enumerate() {
        work.poll(edge)?;
        let v = graph.order.vertices[segment.from];
        if v.kind == Kind::Bend && empty_at(empty, v.rank) {
            continue;
        }
        let last = follow(graph, empty, edge, Along::Down, work)?;
        runs.push(SnapRun {
            from: segment.from,
            to: graph.segments()[last].to,
            from_dx: graph.offset(edge, Along::Down),
            to_dx: graph.offset(last, Along::Up),
        });
    }
    let mut blocks = SnapBlocks::new(xs.len(), work)?;
    let mut near = work.reserve(runs.len())?;
    work.price(runs.len())?;
    for (k, run) in runs.iter().enumerate() {
        work.poll(k)?;
        let d = run.offset(xs).abs();
        if d < 0.01 {
            blocks.join(run.from, run.to, work)?;
        } else if d <= BEND_SNAP {
            near.push(k);
        }
    }
    work.sort(&mut near, |a, b| {
        compare_number(runs[a].offset(xs).abs(), runs[b].offset(xs).abs()).then(a.cmp(&b))
    })?;
    work.price(near.len())?;
    for (i, k) in near.into_iter().enumerate() {
        work.poll(i)?;
        let run = &runs[k];
        let d = run.offset(xs);
        if d.abs() < 0.01 {
            blocks.join(run.from, run.to, work)?;
            continue;
        }
        let upper = blocks.root[run.from];
        let lower = blocks.root[run.to];
        if d.abs() > BEND_SNAP || upper == lower {
            continue;
        }
        let ids = if blocks.count[upper] <= blocks.count[lower] {
            [upper, lower]
        } else {
            [lower, upper]
        };
        for id in ids {
            let shift = if id == upper { d } else { -d };
            if !blocks.can_move(graph, xs, options, id, shift, work)? {
                continue;
            }
            work.price(blocks.count[id])?;
            for (index, v) in blocks.members(id).enumerate() {
                work.poll(index)?;
                xs[v] += shift;
            }
            blocks.join(run.from, run.to, work)?;
            break;
        }
    }
    Ok(())
}

struct SnapBlocks {
    root: Vec<usize>,
    next: Vec<Option<usize>>,
    tail: Vec<usize>,
    count: Vec<usize>,
}

impl SnapBlocks {
    fn new<E>(n: usize, work: &mut Work<'_, E>) -> GeoResult<Self, E> {
        let mut roots = work.reserve(n)?;
        work.price(n)?;
        for i in 0..n {
            work.poll(i)?;
            roots.push(i);
        }
        Ok(Self {
            tail: work.copy(&roots)?,
            root: roots,
            next: work.filled(n, None)?,
            count: work.filled(n, 1)?,
        })
    }
    fn members(&self, id: usize) -> impl Iterator<Item = usize> + '_ {
        std::iter::successors(Some(id), |&v| self.next[v])
    }
    fn join<E>(&mut self, a: usize, b: usize, work: &mut Work<'_, E>) -> GeoResult<(), E> {
        let (mut keep, mut drop) = (self.root[a], self.root[b]);
        if keep == drop {
            return Ok(());
        }
        if self.count[keep] < self.count[drop] {
            std::mem::swap(&mut keep, &mut drop);
        }
        work.price(self.count[drop])?;
        let mut next = Some(drop);
        let mut i = 0;
        while let Some(v) = next {
            work.poll(i)?;
            self.root[v] = keep;
            next = self.next[v];
            i += 1;
        }
        self.next[self.tail[keep]] = Some(drop);
        self.tail[keep] = self.tail[drop];
        self.count[keep] += self.count[drop];
        self.count[drop] = 0;
        Ok(())
    }
    fn can_move<E>(
        &self,
        graph: &Geometry<'_>,
        xs: &[f64],
        options: Options,
        id: usize,
        shift: f64,
        work: &mut Work<'_, E>,
    ) -> GeoResult<bool, E> {
        if shift.abs() > SNAP {
            work.price(self.count[id])?;
            for (i, v) in self.members(id).enumerate() {
                work.poll(i)?;
                if graph.order.vertices[v].kind != Kind::Bend {
                    return Ok(false);
                }
            }
        }
        work.price(self.count[id])?;
        for (i, v) in self.members(id).enumerate() {
            work.poll(i)?;
            let row = graph.row(graph.order.vertices[v].rank);
            let at = graph.order.positions[v];
            let gap = |u| graph.half_room(options, v) + graph.half_room(options, u) - 0.01;
            if at > 0 {
                let u = row[at - 1];
                if self.root[u] != id && xs[v] + shift - xs[u] < gap(u) {
                    return Ok(false);
                }
            }
            if at + 1 < row.len() {
                let u = row[at + 1];
                if self.root[u] != id && xs[u] - xs[v] - shift < gap(u) {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
}

#[derive(Clone, Copy, Default)]
struct BendLine {
    x: f64,
    from: f64,
    to: f64,
}

pub(super) fn straighten<E>(
    graph: &Geometry<'_>,
    xs: &mut [f64],
    empty: &[bool],
    sep: f64,
    on_line: &mut [bool],
    work: &mut Work<'_, E>,
) -> GeoResult<(), E> {
    let mut lines = work.filled(xs.len(), BendLine::default())?;
    work.price(graph.plan.edges.len())?;
    for edge in 0..graph.plan.edges.len() {
        work.poll(edge)?;
        let range = graph.dummy_range(edge);
        work.price(range.len())?;
        let mut d = range.start;
        while d < range.end {
            work.poll(d - range.start)?;
            if !empty[graph.order.vertices[d].rank] {
                d += 1;
                continue;
            }
            let mut last = d;
            while last + 1 < range.end && empty[graph.order.vertices[last + 1].rank] {
                work.poll(last - d)?;
                last += 1;
            }
            let above = graph.adjacent(d, Along::Up)[0];
            let below = graph.adjacent(last, Along::Down)[0];
            let from = graph.other_point(xs, above, Along::Up);
            let to = graph.other_point(xs, below, Along::Down);
            if !from.is_finite() || !to.is_finite() {
                return Err(GeometryError::NonFiniteGeometry);
            }
            let steps = (last - d + 2) as f64;
            work.price(last - d + 1)?;
            for (offset, line) in lines[d..=last].iter_mut().enumerate() {
                work.poll(offset)?;
                *line = BendLine {
                    x: from + (to - from) * (offset + 1) as f64 / steps,
                    from,
                    to,
                };
            }
            d = last + 1;
        }
    }
    let max_rank = graph.rows() - 1;
    work.price(max_rank.saturating_sub(1))?;
    for (r, &only) in empty.iter().enumerate().take(max_rank).skip(1) {
        work.poll(r - 1)?;
        if !only {
            continue;
        }
        let mut row = work.copy(graph.row(r))?;
        work.sort(&mut row, |a, b| {
            compare_number(lines[a].from, lines[b].from)
                .then_with(|| compare_number(lines[a].to, lines[b].to))
                .then(a.cmp(&b))
        })?;
        work.price(row.len())?;
        let mut i = 0;
        while i < row.len() {
            work.poll(i)?;
            let mut j = i + 1;
            let first = lines[row[i]];
            while j < row.len()
                && (lines[row[j]].from - first.from).abs() < TWIN
                && (lines[row[j]].to - first.to).abs() < TWIN
            {
                work.poll(j - i)?;
                j += 1;
            }
            let twins = &mut row[i..j];
            work.sort(twins, |a, b| {
                graph.order.positions[a].cmp(&graph.order.positions[b])
            })?;
            let count = twins.len();
            for (k, &bend) in twins.iter().enumerate() {
                work.poll(k)?;
                xs[bend] = lines[bend].x + (k as f64 - (count - 1) as f64 / 2.0) * sep;
                if count == 1 || sep == 0.0 {
                    on_line[bend] = true;
                }
            }
            i = j;
        }
    }
    Ok(())
}

pub(super) fn past_labels<E>(
    graph: &Geometry<'_>,
    xs: &mut [f64],
    clear: f64,
    on_line: &mut [bool],
    work: &mut Work<'_, E>,
) -> GeoResult<(), E> {
    let mut labelled = work.filled(graph.rows(), false)?;
    let mut any = false;
    let max_rank = graph.rows() - 1;
    work.price(max_rank.saturating_sub(1))?;
    for (r, at) in labelled.iter_mut().enumerate().take(max_rank).skip(1) {
        work.poll(r - 1)?;
        let row = graph.row(r);
        let mut labels = false;
        let mut others = false;
        work.price(row.len())?;
        for (i, &v) in row.iter().enumerate() {
            work.poll(i)?;
            labels |= graph.order.vertices[v].kind == Kind::Label;
            others |= !graph.order.vertices[v].kind.is_dummy();
        }
        *at = labels && !others;
        any |= *at;
    }
    if !any {
        return Ok(());
    }
    work.price(graph.plan.edges.len())?;
    for edge in 0..graph.plan.edges.len() {
        work.poll(edge)?;
        let range = graph.dummy_range(edge);
        work.price(range.len())?;
        for d in range.clone() {
            work.poll(d - range.start)?;
            let v = graph.order.vertices[d];
            if v.kind != Kind::Bend || !labelled[v.rank] {
                continue;
            }
            let (mut first, mut last) = (d, d);
            while first > range.start && on_line[first - 1] {
                work.price(1)?;
                first -= 1;
            }
            while last + 1 < range.end && on_line[last + 1] {
                work.price(1)?;
                last += 1;
            }
            let above = graph.adjacent(first, Along::Up)[0];
            let below = graph.adjacent(last, Along::Down)[0];
            let from = graph.other_point(xs, above, Along::Up);
            let to = graph.other_point(xs, below, Along::Down);
            let lo = js_min(from, to) - clear;
            let hi = js_max(from, to) + clear;
            let row = graph.row(v.rank);
            let mut free = true;
            work.price(row.len())?;
            for (i, &label) in row.iter().enumerate() {
                work.poll(i)?;
                if graph.order.vertices[label].kind == Kind::Label {
                    let half = graph.order.vertices[label].size.width / 2.0;
                    if xs[label] + half > lo && xs[label] - half < hi {
                        free = false;
                        break;
                    }
                }
            }
            if !free {
                continue;
            }
            on_line[d] = true;
            let steps = (last - first + 2) as f64;
            work.price(last - first + 1)?;
            for (j, at) in xs[first..=last].iter_mut().enumerate() {
                work.poll(j)?;
                *at = from + (to - from) * (j + 1) as f64 / steps;
            }
        }
    }
    Ok(())
}
