//! Score compound placements with the path approximation in Mermaid's compound.ts.

use super::compound::{CEdge, CNode};
use super::work::{Work, add, check_slots, cmp, geometry, js_max, js_min, mul};
use super::{Axis, Point, Result, RoutePoint, Routing};
use super::{PointExt, RoutePointExt};

const FAN: f64 = 24.0;
const IN_LINE: f64 = 0.5;
const ALIGNED: f64 = 0.01;
const HANDLE: f64 = 0.62;
const CURVE_PIECES: u32 = 3;
const CORNER_PIECES: u32 = 2;
const MAX_PIECES: usize = 3;
const START: u8 = 1;
const END: u8 = 2;

struct Segment {
    from: Point,
    to: Point,
    low: f64,
    high: f64,
    owner: usize,
    ends: u8,
}

struct Segments {
    pieces: Vec<Segment>,
    // Original traversal direction of the last piece, before ordering by x.
    last: Option<(Point, Point)>,
}

impl Segments {
    fn add<E>(
        &mut self,
        edge: usize,
        ends: u8,
        mut from: Point,
        to: Point,
        work: &mut Work<'_, E>,
    ) -> Result<(), E> {
        work.price(1)?;
        geometry(from.is_finite() && to.is_finite())?;
        if let (Some((a, b)), Some(last)) = (self.last, self.pieces.last()) {
            let collinear =
                from == b && (to.y == from.y && a.y == from.y || to.x == from.x && a.x == from.x);
            if collinear
                && last.owner == edge
                && last.ends == ends
                && (from.x - a.x) * (to.x - from.x) + (from.y - a.y) * (to.y - from.y) > 0.0
            {
                self.pieces.pop();
                from = a;
            }
        }
        self.last = Some((from, to));
        let (ay, by) = if from.x <= to.x {
            (from.y, to.y)
        } else {
            (to.y, from.y)
        };
        self.pieces.push(Segment {
            from: Point {
                x: js_min(from.x, to.x),
                y: ay,
            },
            to: Point {
                x: js_max(from.x, to.x),
                y: by,
            },
            low: js_min(from.y, to.y),
            high: js_max(from.y, to.y),
            owner: edge,
            ends,
        });
        Ok(())
    }

    fn add_route<E>(
        &mut self,
        owner: usize,
        route: &[RoutePoint],
        routing: Routing,
        work: &mut Work<'_, E>,
    ) -> Result<(), E> {
        let Some(first) = route.first() else {
            return Ok(());
        };
        let end = route.len() - 1;
        let mut at = first.position();
        for k in 1..route.len() {
            work.price(1)?;
            let ends = (if k == 1 { START } else { 0 }) | (if k == end { END } else { 0 });
            let from = at;
            let point = route[k].position();
            at = point;
            if aligned(route, k) || routing == Routing::Polyline {
                if turns(route, k, routing) {
                    let before_len = length(route, k);
                    let after_len = length(route, k + 1);
                    let before = if turns(route, k - 1, routing) {
                        before_len / 2.0
                    } else {
                        before_len
                    };
                    let after = if turns(route, k + 1, routing) {
                        after_len / 2.0
                    } else {
                        after_len
                    };
                    let u = unit(route[k - 1].position(), point, before_len);
                    let v = unit(point, route[k + 1].position(), after_len);
                    let start = Point {
                        x: point.x - u.x * before,
                        y: point.y - u.y * before,
                    };
                    self.add(owner, ends, from, start, work)?;
                    let c1 = Point {
                        x: point.x - u.x * before * (1.0 - HANDLE),
                        y: point.y - u.y * before * (1.0 - HANDLE),
                    };
                    let c2 = Point {
                        x: point.x + v.x * after * (1.0 - HANDLE),
                        y: point.y + v.y * after * (1.0 - HANDLE),
                    };
                    at = Point {
                        x: point.x + v.x * after,
                        y: point.y + v.y * after,
                    };
                    let flags = ends | if k + 1 == end { END } else { 0 };
                    let mut previous = start;
                    for i in 1..=CORNER_PIECES {
                        let t = f64::from(i) / f64::from(CORNER_PIECES);
                        let p = Point {
                            x: cubic(start.x, c1.x, c2.x, at.x, t),
                            y: cubic(start.y, c1.y, c2.y, at.y, t),
                        };
                        self.add(owner, flags, previous, p, work)?;
                        previous = p;
                    }
                } else {
                    self.add(owner, ends, from, point, work)?;
                }
            } else if routing == Routing::Orthogonal {
                let (a, b) = if route[k].axis == Axis::Vertical {
                    let y = (from.y + point.y) / 2.0;
                    (Point { x: from.x, y }, Point { x: point.x, y })
                } else {
                    let x = (from.x + point.x) / 2.0;
                    (Point { x, y: from.y }, Point { x, y: point.y })
                };
                for (a, b) in [(from, a), (a, b), (b, point)] {
                    self.add(owner, ends, a, b, work)?;
                }
            } else {
                let (dx, dy) = (point.x - from.x, point.y - from.y);
                let down = route[k].axis == Axis::Vertical;
                let c = if down { ease(dx, dy) } else { ease(dy, dx) };
                let mut previous = from;
                for i in 1..=CURVE_PIECES {
                    let t = f64::from(i) / f64::from(CURVE_PIECES);
                    let u = 1.0 - t;
                    let across = t * t * (3.0 - 2.0 * t);
                    let along = 3.0 * c * t * u * u + 3.0 * (1.0 - c) * t * t * u + t * t * t;
                    let p = Point {
                        x: from.x + dx * if down { across } else { along },
                        y: from.y + dy * if down { along } else { across },
                    };
                    self.add(owner, ends, previous, p, work)?;
                    previous = p;
                }
            }
        }
        Ok(())
    }
}

fn aligned(route: &[RoutePoint], k: usize) -> bool {
    let (a, b) = (route[k - 1].position(), route[k].position());
    (b.x - a.x).abs() < ALIGNED || (b.y - a.y).abs() < ALIGNED
}

fn length(route: &[RoutePoint], k: usize) -> f64 {
    let (a, b) = (route[k - 1].position(), route[k].position());
    (b.x - a.x).abs() + (b.y - a.y).abs()
}

fn turns(route: &[RoutePoint], k: usize, routing: Routing) -> bool {
    routing == Routing::Curved
        && k > 0
        && k < route.len() - 1
        && aligned(route, k)
        && aligned(route, k + 1)
        && length(route, k) >= ALIGNED
        && length(route, k + 1) >= ALIGNED
        && ((route[k].position().x - route[k - 1].position().x).abs() < ALIGNED)
            != ((route[k + 1].position().x - route[k].position().x).abs() < ALIGNED)
}

fn unit(a: Point, b: Point, len: f64) -> Point {
    Point {
        x: (b.x - a.x) / len,
        y: (b.y - a.y) / len,
    }
}

fn cubic(a: f64, b: f64, c: f64, d: f64, t: f64) -> f64 {
    let u = 1.0 - t;
    u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
}

fn ease(across: f64, along: f64) -> f64 {
    let across = if across == 0.0 { 1.0 } else { across.abs() };
    js_max(0.25, js_min(0.5, (4.0 * along.abs()) / across))
}

pub(crate) fn route_crossings<E>(
    nodes: &[CNode],
    edges: &[CEdge],
    routing: Routing,
    work: &mut Work<'_, E>,
) -> Result<usize, E> {
    let mut capacity = 0;
    for edge in edges {
        work.price(1)?;
        if edge.ends[0] != edge.ends[1] {
            capacity = add(
                capacity,
                mul(edge.route.len().saturating_sub(1), MAX_PIECES)?,
            )?;
        }
    }
    check_slots(capacity)?;
    let mut segments = Segments {
        pieces: work.reserve(capacity)?,
        last: None,
    };
    for (i, edge) in edges.iter().enumerate() {
        work.price(1)?;
        if edge.ends[0] != edge.ends[1] {
            segments.add_route(i, &edge.route, routing, work)?;
        }
    }
    let pieces = &segments.pieces;
    if pieces.len() < 2 {
        return Ok(0);
    }

    let Bands {
        starts: band_starts,
        first,
        pieces: mut slots,
    } = bands(pieces, work)?;
    let mut runs = work.reserve(edges.len())?;
    for edge in edges {
        work.price(1)?;
        runs.push(end_runs(&edge.route, work)?);
    }
    let mut total = 0;
    for (band, range) in band_starts.windows(2).enumerate() {
        work.price(1)?;
        let cells = &mut slots[range[0]..range[1]];
        work.sort_rows(cells, |&p, &q| {
            cmp(pieces[p].from.x, pieces[q].from.x).then(p.cmp(&q))
        })?;
        for (i, &pi) in cells.iter().enumerate() {
            work.price(1)?;
            let p = &pieces[pi];
            for &qi in &cells[i + 1..] {
                work.price(1)?;
                let q = &pieces[qi];
                if q.from.x > p.to.x {
                    break;
                }
                if q.high < p.low
                    || q.low > p.high
                    || p.owner == q.owner
                    || (first[pi] != band && first[qi] != band)
                {
                    continue;
                }
                let Some(at) = intersection(p, q)? else {
                    continue;
                };
                let ep = &edges[p.owner];
                let eq = &edges[q.owner];
                let mut shared = false;
                for (a, &node) in ep.ends.iter().enumerate() {
                    for (b, &other) in eq.ends.iter().enumerate() {
                        if node == other && fan(&nodes[node], p, q, a, b, at, &runs) {
                            shared = true;
                        }
                    }
                }
                if !shared {
                    total = add(total, 1)?;
                }
            }
        }
    }
    Ok(total)
}

struct Bands {
    starts: Vec<usize>,
    first: Vec<usize>,
    pieces: Vec<usize>,
}

fn bands<E>(pieces: &[Segment], work: &mut Work<'_, E>) -> Result<Bands, E> {
    let mut top = f64::INFINITY;
    let mut bottom = f64::NEG_INFINITY;
    work.price(pieces.len())?;
    for (i, p) in pieces.iter().enumerate() {
        work.poll(i)?;
        top = js_min(top, p.low);
        bottom = js_max(bottom, p.high);
    }
    let height = bottom - top;
    geometry(height.is_finite())?;
    let count = if height > 0.0 {
        (pieces.len() as f64).sqrt().round() as usize
    } else {
        1
    };
    let scale = if height > 0.0 {
        count as f64 / height
    } else {
        0.0
    };
    // A subnormal range can overflow the scale. It has no resolvable bands then.
    geometry(scale.is_finite())?;
    let band_of = |y: f64| (((y - top) * scale).floor() as usize).min(count - 1);
    let mut starts = work.fill(add(count, 1)?, 0)?;
    let mut first = work.reserve(pieces.len())?;
    for p in pieces {
        work.price(1)?;
        let (a, b) = (band_of(p.low), band_of(p.high));
        first.push(a);
        work.price(b - a + 1)?;
        for (i, slots) in starts[a + 1..=b + 1].iter_mut().enumerate() {
            work.poll(i)?;
            *slots += 1;
        }
    }
    work.price(count)?;
    for i in 1..=count {
        work.poll(i)?;
        starts[i] = add(starts[i], starts[i - 1])?;
    }
    check_slots(starts[count])?;
    let mut slots = work.fill(starts[count], 0)?;
    let mut fill = work.reserve(count)?;
    work.price(count)?;
    for (i, &start) in starts[..count].iter().enumerate() {
        work.poll(i)?;
        fill.push(start);
    }
    for (i, p) in pieces.iter().enumerate() {
        work.price(1)?;
        let (a, b) = (first[i], band_of(p.high));
        work.price(b - a + 1)?;
        for (j, at) in fill[a..=b].iter_mut().enumerate() {
            work.poll(j)?;
            slots[*at] = i;
            *at += 1;
        }
    }
    Ok(Bands {
        starts,
        first,
        pieces: slots,
    })
}

fn intersection<E>(p: &Segment, q: &Segment) -> Result<Option<Point>, E> {
    let (dx, dy) = (p.to.x - p.from.x, p.to.y - p.from.y);
    let d1 = dx * (q.from.y - p.from.y) - dy * (q.from.x - p.from.x);
    let d2 = dx * (q.to.y - p.from.y) - dy * (q.to.x - p.from.x);
    geometry(d1.is_finite() && d2.is_finite())?;
    // Points on the line belong to one side, counting a segment-boundary crossing once.
    if (d1 > 0.0) == (d2 > 0.0) {
        return Ok(None);
    }
    let (ex, ey) = (q.to.x - q.from.x, q.to.y - q.from.y);
    let d3 = ex * (p.from.y - q.from.y) - ey * (p.from.x - q.from.x);
    let d4 = ex * (p.to.y - q.from.y) - ey * (p.to.x - q.from.x);
    geometry(d3.is_finite() && d4.is_finite())?;
    if (d3 > 0.0) == (d4 > 0.0) {
        return Ok(None);
    }
    let at = d3 / (d3 - d4);
    let point = Point {
        x: p.from.x + at * dx,
        y: p.from.y + at * dy,
    };
    geometry(point.is_finite())?;
    Ok(Some(point))
}

type EndRuns = [[Point; 2]; 2];

fn end_runs<E>(route: &[RoutePoint], work: &mut Work<'_, E>) -> Result<EndRuns, E> {
    if route.len() < 2 {
        return Ok([[Point::default(); 2]; 2]);
    }
    let last = route.len() - 1;
    let start = route[0].position();
    let end = route[last].position();
    let mut k = 1;
    let mut j = last - 1;
    while k < last && in_line(start, route[k].position(), route[k + 1].position()) {
        work.price(1)?;
        k += 1;
    }
    while j >= 1 && in_line(end, route[j].position(), route[j - 1].position()) {
        work.price(1)?;
        j -= 1;
    }
    Ok([[start, route[k].position()], [route[j].position(), end]])
}

fn in_line(a: Point, b: Point, c: Point) -> bool {
    (a.x - b.x).abs() < IN_LINE && (a.x - c.x).abs() < IN_LINE
        || (a.y - b.y).abs() < IN_LINE && (a.y - c.y).abs() < IN_LINE
}

fn fan(
    node: &CNode,
    p: &Segment,
    q: &Segment,
    a: usize,
    b: usize,
    at: Point,
    runs: &[EndRuns],
) -> bool {
    p.ends & (1 << a) != 0 && q.ends & (1 << b) != 0
        || ((at.x - node.center.x).abs() < node.size.width / 2.0 + FAN
            && (at.y - node.center.y).abs() < node.size.height / 2.0 + FAN)
        || (on_run(runs[p.owner][a], at) && on_run(runs[q.owner][b], at))
}

fn on_run([a, b]: [Point; 2], at: Point) -> bool {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length = dx * dx + dy * dy;
    let t = if length > 0.0 {
        ((at.x - a.x) * dx + (at.y - a.y) * dy) / length
    } else {
        0.0
    };
    let t = t.clamp(0.0, 1.0);
    (at.x - a.x - t * dx).hypot(at.y - a.y - t * dy) < IN_LINE
}
