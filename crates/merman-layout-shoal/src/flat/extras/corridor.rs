//! Orthogonal connections for exceptional edges only. Try the two elbows, then outside
//! corridors. An interlocking set of obstacles can require more turns than either admits.
use super::*;
use std::{cmp::Reverse, collections::BinaryHeap};

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
                    (r.x0 - p.x)
                        .max(p.x - r.x1)
                        .max(r.y0 - p.y)
                        .max(p.y - r.y1)
                        .max(0.0),
                );
            }
            halos.push(Bounds {
                x0: finite(r.x0 - margin)?,
                x1: finite(r.x1 + margin)?,
                y0: finite(r.y0 - margin)?,
                y1: finite(r.y1 + margin)?,
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
    let (mut left, mut right, mut top, mut bottom) =
        (a.x.min(b.x), a.x.max(b.x), a.y.min(b.y), a.y.max(b.y));
    for r in &walls.0 {
        work.visit(1)?;
        left = left.min(r.x0);
        right = right.max(r.x1);
        top = top.min(r.y0);
        bottom = bottom.max(r.y1);
    }
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
struct Cell {
    down: i64,
    right: i64,
}

/// Add a half-open rectangle to a 2D difference array. An occupied grid link has an
/// interior intersection with a measured obstacle. Exact obstacle boundaries are legal.
fn block(
    mask: &mut [Cell],
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
        let cell = &mut mask[y * stride + x];
        if horizontal {
            cell.right += delta;
        } else {
            cell.down += delta;
        }
    }
}

/// For B obstacles the coordinate arrangement has at most (2B+2)^2 intersections. Admit
/// the actual distinct product before allocation. Rasterization costs O(B log B + cells),
/// shortest paths O(cells log cells) and storage O(cells + B). Distances use drawing units.
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
        xs.extend([r.x0, r.x1]);
        ys.extend([r.y0, r.y1]);
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
    let mut mask = filled(padded, Cell::default())?;
    for r in &walls.0 {
        work.visit(4 * (width.max(height).ilog2() as usize + 1))?;
        let (x0, x1, y0, y1) = (
            index(&xs, r.x0),
            index(&xs, r.x1),
            index(&ys, r.y0),
            index(&ys, r.y1),
        );
        block(&mut mask, stride, x0..x1, y0 + 1..y1, true);
        block(&mut mask, stride, x0 + 1..x1, y0..y1, false);
    }
    for y in 0..height {
        let mut row = Cell::default();
        for x in 0..width {
            work.visit(1)?;
            let i = y * stride + x;
            row.right += mask[i].right;
            row.down += mask[i].down;
            let above = if y == 0 {
                Cell::default()
            } else {
                mask[i - stride]
            };
            mask[i] = Cell {
                right: row.right + above.right,
                down: row.down + above.down,
            };
        }
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
        let i = y * stride + x;
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
        if x > 0 && mask[i - 1].right == 0 {
            relax(v - 1, xs[x] - xs[x - 1])?;
        }
        if x + 1 < width && mask[i].right == 0 {
            relax(v + 1, xs[x + 1] - xs[x])?;
        }
        if y > 0 && mask[i - stride].down == 0 {
            relax(v - width, ys[y] - ys[y - 1])?;
        }
        if y + 1 < height && mask[i].down == 0 {
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
