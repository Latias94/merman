//! Orthogonal connections for exceptional edges only. Try the two elbows, then outside
//! corridors. An interlocking set of obstacles can require more turns than either admits.
use super::*;
use std::{cmp::Reverse, collections::BinaryHeap, mem::size_of};

pub(super) const NO_CORRIDOR: LayoutError =
    LayoutError::InvalidInput("no open orthogonal corridor between ports");

pub(super) fn path(
    start: Point,
    end: Point,
    via: &[Point],
    walls: &RouteWalls,
    gap: f64,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<Point>> {
    let mut path = Vec::new();
    let mut build = |walls: &RouteWalls, work: &mut Work<'_>| -> LayoutResult<()> {
        path.clear();
        let mut previous = start;
        for &next in via.iter().chain(std::iter::once(&end)) {
            connect(&mut path, previous, next, walls, gap, work)?;
            previous = next;
        }
        Ok(())
    };
    if gap > 0.0 {
        work.charge(walls.0.len())?;
        let mut halos = reserved(walls.0.len())?;
        for &r in &walls.0 {
            let mut margin = gap;
            for &p in [start, end].iter().chain(via) {
                work.visit(1)?;
                margin = margin.min(
                    (r.x[0] - p.x)
                        .max(p.x - r.x[1])
                        .max(r.y[0] - p.y)
                        .max(p.y - r.y[1])
                        .max(0.0),
                );
            }
            halos.push(Bounds {
                x: [finite(r.x[0] - margin)?, finite(r.x[1] + margin)?],
                y: [finite(r.y[0] - margin)?, finite(r.y[1] + margin)?],
            });
        }
        match build(&RouteWalls(halos), work) {
            Ok(()) => return Ok(path),
            Err(NO_CORRIDOR) => {}
            Err(err) => return Err(err),
        }
    }
    build(walls, work)?;
    Ok(path)
}

fn clear(points: &[Point], walls: &RouteWalls, work: &mut Work<'_>) -> LayoutResult<bool> {
    for p in points.windows(2) {
        if !walls.clear_leg(p[0], p[1], work)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn emit(out: &mut Vec<Point>, points: &[Point]) -> LayoutResult<()> {
    for &p in points {
        if out.last() != Some(&p) {
            push(out, p)?;
        }
    }
    Ok(())
}

pub(super) fn connect(
    out: &mut Vec<Point>,
    a: Point,
    b: Point,
    walls: &RouteWalls,
    gap: f64,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    if a == b {
        return emit(out, &[a]);
    }
    for turn in [Point { x: a.x, y: b.y }, Point { x: b.x, y: a.y }] {
        let path = [a, turn, b];
        if clear(&path, walls, work)? {
            return emit(out, &path);
        }
    }
    let mut outer = Bounds::between(a, b);
    for &r in &walls.0 {
        work.visit(1)?;
        outer = outer.union(r);
    }
    let [left, right] = outer.x;
    let [top, bottom] = outer.y;
    let mut best = None;
    for (axis, line) in [
        (Axis::Vertical, finite(left - gap)?),
        (Axis::Vertical, finite(right + gap)?),
        (Axis::Horizontal, finite(top - gap)?),
        (Axis::Horizontal, finite(bottom + gap)?),
    ] {
        let (p, q) = match axis {
            Axis::Vertical => (Point { x: line, ..a }, Point { x: line, ..b }),
            Axis::Horizontal => (Point { y: line, ..a }, Point { y: line, ..b }),
        };
        let candidate = [a, p, q, b];
        let cost = finite(
            candidate
                .windows(2)
                .map(|s| (s[0].x - s[1].x).abs() + (s[0].y - s[1].y).abs())
                .sum(),
        )?;
        if best.as_ref().is_none_or(|(old, _)| cost < *old) && clear(&candidate, walls, work)? {
            best = Some((cost, candidate));
        }
    }
    if let Some((_, path)) = best {
        return emit(out, &path);
    }
    grid(out, a, b, walls, work)
}

fn index(lines: &[f64], value: f64) -> usize {
    lines
        .binary_search_by(|&v| {
            if v == value {
                std::cmp::Ordering::Equal
            } else {
                v.total_cmp(&value)
            }
        })
        .expect("grid includes all obstacle boundaries and terminals")
}

#[derive(Clone, Copy, Default)]
struct LinkCounts {
    down: i64,
    right: i64,
}

const RIGHT: u8 = 1;
const DOWN: u8 = 2;

/// Four vertices per byte, with one bit for each outgoing grid link.
struct BlockedLinks(Vec<u8>);

impl BlockedLinks {
    fn set(&mut self, vertex: usize, counts: LinkCounts) {
        let flags = u8::from(counts.right != 0) | (u8::from(counts.down != 0) << 1);
        self.0[vertex / 4] |= flags << (vertex % 4 * 2);
    }

    fn blocked(&self, vertex: usize, link: u8) -> bool {
        self.0[vertex / 4] & (link << (vertex % 4 * 2)) != 0
    }
}

const NO_EVENT: usize = usize::MAX;
const SWEEP_CHECK_INTERVAL: usize = 128;

#[derive(Clone, Copy)]
struct RowEvents {
    start: usize,
    end: usize,
}

// One wall belongs to its start row and its end row. Links into both row lists
// avoid an event sort and keep the wall's column interval in one place.
struct WallSpan {
    left: usize,
    right: usize,
    next_start: usize,
    next_end: usize,
}

fn visit_walls(
    xs: &[f64],
    ys: &[f64],
    walls: &RouteWalls,
    work: &mut Work<'_>,
    mut block: impl FnMut([usize; 2], [usize; 2]),
) -> LayoutResult<()> {
    let lookup_work = 4 * (xs.len().max(ys.len()).ilog2() as usize + 1);
    for r in &walls.0 {
        work.visit(lookup_work)?;
        block(
            [index(xs, r.x[0]), index(xs, r.x[1])],
            [index(ys, r.y[0]), index(ys, r.y[1])],
        );
    }
    Ok(())
}

fn rasterize_sweep(
    xs: &[f64],
    ys: &[f64],
    walls: &RouteWalls,
    mask: &mut BlockedLinks,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let width = xs.len();
    let height = ys.len();
    let mut rows = filled(
        height,
        RowEvents {
            start: NO_EVENT,
            end: NO_EVENT,
        },
    )?;
    let mut columns = filled(width, LinkCounts::default())?;
    let mut spans = reserved(walls.0.len())?;
    visit_walls(xs, ys, walls, work, |[x0, x1], [y0, y1]| {
        if x0 >= x1 || y0 >= y1 || (x0 + 1 == x1 && y0 + 1 == y1) {
            return;
        }
        let event = spans.len();
        spans.push(WallSpan {
            left: x0,
            right: x1,
            next_start: rows[y0].start,
            next_end: rows[y1].end,
        });
        rows[y0].start = event;
        rows[y1].end = event;
    })?;

    // Wall indexing and the grid plan already paid for these updates. A single row can
    // still contain many events, so poll across all three lists without charging again.
    let mut events_since_check = 0;
    let mut poll_after_event = |work: &mut Work<'_>| -> LayoutResult<()> {
        events_since_check += 1;
        if events_since_check == SWEEP_CHECK_INTERVAL {
            events_since_check = 0;
            work.check()?;
        }
        Ok(())
    };
    let mut previous_start = NO_EVENT;
    for (y, row) in rows.iter().enumerate() {
        // Right links use [x0, x1) × [y0 + 1, y1); down links use
        // [x0 + 1, x1) × [y0, y1). Both leave at y1. Starting right links
        // before removing expired spans also handles a one-row span (it cancels).
        let mut event = previous_start;
        while let Some(span) = spans.get(event) {
            columns[span.left].right += 1;
            columns[span.right].right -= 1;
            event = span.next_start;
            poll_after_event(work)?;
        }
        let mut event = row.end;
        while let Some(span) = spans.get(event) {
            columns[span.left].right -= 1;
            columns[span.right].right += 1;
            columns[span.left + 1].down -= 1;
            columns[span.right].down += 1;
            event = span.next_end;
            poll_after_event(work)?;
        }
        let mut event = row.start;
        while let Some(span) = spans.get(event) {
            columns[span.left + 1].down += 1;
            columns[span.right].down -= 1;
            event = span.next_start;
            poll_after_event(work)?;
        }
        previous_start = row.start;

        let mut counts = LinkCounts::default();
        for (x, delta) in columns.iter().enumerate() {
            work.visit(1)?;
            counts.right += delta.right;
            counts.down += delta.down;
            mask.set(y * width + x, counts);
        }
    }
    Ok(())
}

/// Mark a half-open rectangle of links with a corner difference at each endpoint.
fn block(
    deltas: &mut [LinkCounts],
    stride: usize,
    xs: std::ops::Range<usize>,
    ys: std::ops::Range<usize>,
    horizontal: bool,
) {
    if xs.is_empty() || ys.is_empty() {
        return;
    }
    for (x, y, delta) in [
        (xs.start, ys.start, 1),
        (xs.end, ys.start, -1),
        (xs.start, ys.end, -1),
        (xs.end, ys.end, 1),
    ] {
        let cell = &mut deltas[y * stride + x];
        if horizontal {
            cell.right += delta;
        } else {
            cell.down += delta;
        }
    }
}

/// Many walls can share a small coordinate grid. Use its smaller difference buffer as
/// scratch in that case, releasing it before shortest-path state is allocated.
fn rasterize_dense(
    xs: &[f64],
    ys: &[f64],
    walls: &RouteWalls,
    padded: usize,
    mask: &mut BlockedLinks,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let width = xs.len();
    let stride = width + 1;
    let mut deltas = filled(padded, LinkCounts::default())?;
    visit_walls(xs, ys, walls, work, |[x0, x1], [y0, y1]| {
        block(&mut deltas, stride, x0..x1, y0 + 1..y1, true);
        block(&mut deltas, stride, x0 + 1..x1, y0..y1, false);
    })?;
    for y in 0..ys.len() {
        let mut row = LinkCounts::default();
        for x in 0..width {
            work.visit(1)?;
            let i = y * stride + x;
            row.right += deltas[i].right;
            row.down += deltas[i].down;
            let above = if y == 0 {
                LinkCounts::default()
            } else {
                deltas[i - stride]
            };
            deltas[i] = LinkCounts {
                right: row.right + above.right,
                down: row.down + above.down,
            };
            mask.set(y * width + x, deltas[i]);
        }
    }
    Ok(())
}

/// For B obstacles the coordinate arrangement has at most (2B+2)^2 intersections. Admit
/// the actual distinct product before allocation. Rasterization takes O(B log B + cells)
/// time with the smaller of row events or a difference grid as scratch. Shortest paths take
/// O(cells log cells) time and O(cells) storage. Distances use drawing units.
fn grid(
    out: &mut Vec<Point>,
    a: Point,
    b: Point,
    walls: &RouteWalls,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let count = walls
        .0
        .len()
        .checked_mul(2)
        .and_then(|n| n.checked_add(2))
        .ok_or(WorkError::ArithmeticOverflow)?;
    work.charge(count.checked_mul(2).ok_or(WorkError::ArithmeticOverflow)?)?;
    let mut xs = reserved(count)?;
    let mut ys = reserved(count)?;
    xs.extend([a.x, b.x]);
    ys.extend([a.y, b.y]);
    for r in &walls.0 {
        work.visit(1)?;
        xs.extend(r.x);
        ys.extend(r.y);
    }
    for lines in [&mut xs, &mut ys] {
        super::super::validate::charge_sort(lines.len(), work)?;
        lines.sort_unstable_by(f64::total_cmp);
        lines.dedup();
    }
    let width = xs.len();
    let height = ys.len();
    let cells = width
        .checked_mul(height)
        .ok_or(WorkError::ArithmeticOverflow)?;
    let stride = width.checked_add(1).ok_or(WorkError::ArithmeticOverflow)?;
    let padded = stride
        .checked_mul(height.checked_add(1).ok_or(WorkError::ArithmeticOverflow)?)
        .ok_or(WorkError::ArithmeticOverflow)?;
    work.charge(
        padded
            .checked_mul(2)
            .and_then(|v| cells.checked_mul(2).and_then(|n| n.checked_add(v)))
            .ok_or(WorkError::ArithmeticOverflow)?,
    )?;
    // Preserve allocation-size rejection after admission, even with packed links.
    let dense_bytes = std::alloc::Layout::array::<LinkCounts>(padded)
        .map_err(|_| LayoutError::AllocationLimit)?
        .size();
    let sweep_bytes = walls
        .0
        .len()
        .checked_mul(size_of::<WallSpan>())
        .and_then(|n| n.checked_add(height.checked_mul(size_of::<RowEvents>())?))
        .and_then(|n| n.checked_add(width.checked_mul(size_of::<LinkCounts>())?));
    let mut mask = BlockedLinks(filled(cells.div_ceil(4), 0)?);
    if sweep_bytes.is_some_and(|n| n <= dense_bytes) {
        rasterize_sweep(&xs, &ys, walls, &mut mask, work)?;
    } else {
        rasterize_dense(&xs, &ys, walls, padded, &mut mask, work)?;
    }
    let source = index(&ys, a.y) * width + index(&xs, a.x);
    let target = index(&ys, b.y) * width + index(&xs, b.x);
    let mut parents = filled(cells, usize::MAX)?;
    let mut dist = filled(cells, f64::INFINITY)?;
    let mut queue = BinaryHeap::new();
    queue
        .try_reserve(1)
        .map_err(|_| LayoutError::AllocationLimit)?;
    queue.push(Reverse((0u64, source)));
    dist[source] = 0.0;
    while let Some(Reverse((bits, v))) = queue.pop() {
        work.visit(queue.len().checked_ilog2().unwrap_or(0) as usize + 1)?;
        if dist[v].to_bits() != bits {
            continue;
        }
        if v == target {
            break;
        }
        let x = v % width;
        let y = v / width;
        let mut relax = |next: usize, length: f64| -> LayoutResult<()> {
            let d = finite(dist[v] + length)?;
            if d < dist[next] {
                work.visit(queue.len().checked_ilog2().unwrap_or(0) as usize + 1)?;
                queue
                    .try_reserve(1)
                    .map_err(|_| LayoutError::AllocationLimit)?;
                queue.push(Reverse((d.to_bits(), next)));
                dist[next] = d;
                parents[next] = v;
            }
            Ok(())
        };
        if x > 0 && !mask.blocked(v - 1, RIGHT) {
            relax(v - 1, xs[x] - xs[x - 1])?;
        }
        if x + 1 < width && !mask.blocked(v, RIGHT) {
            relax(v + 1, xs[x + 1] - xs[x])?;
        }
        if y > 0 && !mask.blocked(v - width, DOWN) {
            relax(v - width, ys[y] - ys[y - 1])?;
        }
        if y + 1 < height && !mask.blocked(v, DOWN) {
            relax(v + width, ys[y + 1] - ys[y])?;
        }
    }
    if !dist[target].is_finite() {
        return Err(NO_CORRIDOR);
    }
    let mut back = Vec::new();
    let mut v = target;
    loop {
        work.visit(1)?;
        push(
            &mut back,
            Point {
                x: xs[v % width],
                y: ys[v / width],
            },
        )?;
        if v == source {
            break;
        }
        v = parents[v];
    }
    for p in back.into_iter().rev() {
        work.visit(1)?;
        emit(out, &[p])?;
    }
    Ok(())
}
