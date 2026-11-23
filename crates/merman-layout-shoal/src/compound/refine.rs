//! Compare stitched routing centerlines in drawing coordinates. The linear interpolation error
//! of a cubic on an interval of width `h` is at most `max(abs(B'')) * h² / 8` per coordinate.

use super::{Pass, sides::flow_sides, solve::Frame};
use crate::work::{LayoutResult, Work, filled, push, reserved};
use crate::{
    Axis, Endpoint, Graph, LayoutError, LayoutOutput, Point, Rect, RoutePoint, RoutingStyle,
    WorkError,
};

const FAN: f64 = 24.0;
const TURN_HANDLE: f64 = 0.62;
const IN_LINE: f64 = 0.5;
const CURVE_TOLERANCE: f64 = 0.25;
const MAX_CURVE_CHORDS: usize = 64;

fn point(knot: RoutePoint) -> Point {
    Point {
        x: knot.x,
        y: knot.y,
    }
}

fn blend(from: f64, to: f64, fraction: f64) -> f64 {
    if from.is_sign_negative() == to.is_sign_negative() {
        (to - from).mul_add(fraction, from)
    } else {
        from * (1.0 - fraction) + to * fraction
    }
}

fn lerp(from: Point, to: Point, fraction: f64) -> Point {
    Point {
        x: blend(from.x, to.x, fraction),
        y: blend(from.y, to.y, fraction),
    }
}

fn aligned(from: Point, to: Point) -> bool {
    from.x == to.x || from.y == to.y
}

fn turn(route: &[RoutePoint], index: usize) -> bool {
    if index == 0 || index + 1 >= route.len() {
        return false;
    }
    let [before, at, after] = [route[index - 1], route[index], route[index + 1]];
    (before.x == at.x && at.y == after.y) || (before.y == at.y && at.x == after.x)
}

#[derive(Clone, Copy)]
struct Piece {
    edge: usize,
    from: Point,
    to: Point,
}

impl Piece {
    fn left(self) -> f64 {
        self.from.x.min(self.to.x)
    }
    fn right(self) -> f64 {
        self.from.x.max(self.to.x)
    }
    fn top(self) -> f64 {
        self.from.y.min(self.to.y)
    }
    fn bottom(self) -> f64 {
        self.from.y.max(self.to.y)
    }
}

struct Pen<'a> {
    edge: usize,
    at: Point,
    pieces: &'a mut Vec<Piece>,
}

impl Pen<'_> {
    fn line(&mut self, to: Point) -> LayoutResult<()> {
        if self.at != to {
            push(
                self.pieces,
                Piece {
                    edge: self.edge,
                    from: self.at,
                    to,
                },
            )?;
            self.at = to;
        }
        Ok(())
    }

    fn cubic(
        &mut self,
        first: Point,
        second: Point,
        to: Point,
        minimum: usize,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        let from = self.at;
        let bend = (first.x - from.x.midpoint(second.x))
            .abs()
            .max((first.y - from.y.midpoint(second.y)).abs())
            .max((second.x - first.x.midpoint(to.x)).abs())
            .max((second.y - first.y.midpoint(to.y)).abs());
        let chords = ((bend.sqrt() * (1.5 / CURVE_TOLERANCE).sqrt()).ceil() as usize)
            .clamp(minimum, MAX_CURVE_CHORDS);
        work.visit(chords)?;
        for index in 1..chords {
            let at = index as f64 / chords as f64;
            let middle = lerp(first, second, at);
            self.line(lerp(
                lerp(lerp(from, first, at), middle, at),
                lerp(middle, lerp(second, to, at), at),
                at,
            ))?;
        }
        self.line(to)
    }

    fn sweep(&mut self, to: Point, axis: Axis, work: &mut Work<'_>) -> LayoutResult<()> {
        let (dx, dy) = (to.x - self.at.x, to.y - self.at.y);
        if !dx.is_finite() || !dy.is_finite() {
            return Err(WorkError::ArithmeticOverflow.into());
        }
        let (along, across) = match axis {
            Axis::Vertical => (dy, dx),
            Axis::Horizontal => (dx, dy),
        };
        let ease = (4.0 * (along.abs() / across.abs().max(1.0))).clamp(0.25, 0.5);
        let (first, second) = match axis {
            Axis::Vertical => (
                Point {
                    y: blend(self.at.y, to.y, ease),
                    ..self.at
                },
                Point {
                    y: blend(to.y, self.at.y, ease),
                    ..to
                },
            ),
            Axis::Horizontal => (
                Point {
                    x: blend(self.at.x, to.x, ease),
                    ..self.at
                },
                Point {
                    x: blend(to.x, self.at.x, ease),
                    ..to
                },
            ),
        };
        self.cubic(first, second, to, 3, work)
    }

    fn step(&mut self, to: Point, axis: Axis) -> LayoutResult<()> {
        let (first, second) = match axis {
            Axis::Vertical => {
                let y = self.at.y.midpoint(to.y);
                (Point { y, ..self.at }, Point { y, ..to })
            }
            Axis::Horizontal => {
                let x = self.at.x.midpoint(to.x);
                (Point { x, ..self.at }, Point { x, ..to })
            }
        };
        self.line(first)?;
        self.line(second)?;
        self.line(to)
    }
}

fn flatten_section(
    edge: usize,
    route: &[RoutePoint],
    style: RoutingStyle,
    knots: &mut Vec<RoutePoint>,
    pieces: &mut Vec<Piece>,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    knots.clear();
    for &knot in route {
        work.visit(1)?;
        if !knot.x.is_finite() || !knot.y.is_finite() {
            return Err(WorkError::ArithmeticOverflow.into());
        }
        if knots.last().is_some_and(|&last| point(last) == point(knot)) {
            continue;
        }
        if let [.., before, last] = knots.as_slice()
            && ((before.x == last.x
                && last.x == knot.x
                && (before.y.min(knot.y)..=before.y.max(knot.y)).contains(&last.y))
                || (before.y == last.y
                    && last.y == knot.y
                    && (before.x.min(knot.x)..=before.x.max(knot.x)).contains(&last.x)))
        {
            knots.pop();
        }
        push(knots, knot)?;
    }
    let Some(first) = knots.first() else {
        return Ok(());
    };
    let mut pen = Pen {
        edge,
        at: point(*first),
        pieces,
    };
    for index in 1..knots.len() {
        work.visit(1)?;
        let knot = knots[index];
        let to = point(knot);
        if style == RoutingStyle::Curved && turn(knots, index) {
            let before = point(knots[index - 1]);
            let after = point(knots[index + 1]);
            let start = if turn(knots, index - 1) {
                lerp(before, to, 0.5)
            } else {
                before
            };
            let end = if turn(knots, index + 1) {
                lerp(after, to, 0.5)
            } else {
                after
            };
            pen.line(start)?;
            pen.cubic(
                lerp(start, to, TURN_HANDLE),
                lerp(end, to, TURN_HANDLE),
                end,
                2,
                work,
            )?;
        } else if aligned(pen.at, to) || style == RoutingStyle::Polyline {
            pen.line(to)?;
        } else if style == RoutingStyle::Orthogonal {
            pen.step(to, knot.axis)?;
        } else {
            pen.sweep(to, knot.axis, work)?;
        }
    }
    Ok(())
}

fn flatten(
    output: &LayoutOutput,
    style: RoutingStyle,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<Piece>> {
    let mut pieces = Vec::new();
    let mut knots = Vec::new();
    let mut stitches = output.border_stitches.iter().peekable();
    for (edge, geometry) in output.edges.iter().enumerate() {
        work.visit(1)?;
        let mut begin = 0;
        while let Some(stitch) = stitches.next_if(|s| s.edge.0 as usize == edge) {
            let section = geometry
                .points
                .get(begin..=stitch.point)
                .ok_or(LayoutError::InvalidInput("invalid compound border stitch"))?;
            flatten_section(edge, section, style, &mut knots, &mut pieces, work)?;
            begin = stitch.point;
        }
        flatten_section(
            edge,
            &geometry.points[begin..],
            style,
            &mut knots,
            &mut pieces,
            work,
        )?;
    }
    if stitches.next().is_some() {
        return Err(LayoutError::InvalidInput(
            "unordered compound border stitches",
        ));
    }
    Ok(pieces)
}

#[derive(Clone, Copy)]
struct BandEntry {
    piece: usize,
    subtree_right: f64,
}

fn index_band(entries: &mut [BandEntry], pieces: &[Piece]) -> f64 {
    let middle = entries.len() / 2;
    let (lower, upper) = entries.split_at_mut(middle);
    let Some((entry, upper)) = upper.split_first_mut() else {
        return f64::NEG_INFINITY;
    };
    entry.subtree_right = pieces[entry.piece]
        .right()
        .max(index_band(lower, pieces))
        .max(index_band(upper, pieces));
    entry.subtree_right
}

fn overlaps(
    entries: &[BandEntry],
    pieces: &[Piece],
    query: Piece,
    work: &mut Work<'_>,
    emit: &mut impl FnMut(usize) -> LayoutResult<()>,
) -> LayoutResult<()> {
    let (lower, upper) = entries.split_at(entries.len() / 2);
    let Some((entry, upper)) = upper.split_first() else {
        return Ok(());
    };
    work.visit(1)?;
    if entry.subtree_right < query.left() || pieces[entries[0].piece].left() > query.right() {
        return Ok(());
    }
    let other = pieces[entry.piece];
    if other.edge < query.edge
        && other.left() <= query.right()
        && query.left() <= other.right()
        && other.top() <= query.bottom()
        && query.top() <= other.bottom()
    {
        emit(entry.piece)?;
    }
    overlaps(lower, pieces, query, work, emit)?;
    overlaps(upper, pieces, query, work, emit)
}

struct Bands {
    bins: Vec<Vec<BandEntry>>,
    spans: Vec<(usize, usize)>,
}

impl Bands {
    fn build(pieces: &[Piece], work: &mut Work<'_>) -> LayoutResult<Option<Self>> {
        let mut left = f64::INFINITY;
        let mut top = f64::INFINITY;
        let mut right = f64::NEG_INFINITY;
        let mut bottom = f64::NEG_INFINITY;
        for &piece in pieces {
            work.visit(1)?;
            left = left.min(piece.left());
            right = right.max(piece.right());
            top = top.min(piece.top());
            bottom = bottom.max(piece.bottom());
        }
        if left >= right || top >= bottom {
            return Ok(None);
        }
        let height = bottom - top;
        if !height.is_finite() || !(right - left).is_finite() {
            return Err(WorkError::ArithmeticOverflow.into());
        }
        let mut covered = 0.0;
        for &piece in pieces {
            work.visit(1)?;
            covered += (piece.bottom() - piece.top()) / height;
        }
        let count = pieces
            .len()
            .isqrt()
            .min((pieces.len() as f64 / covered.max(1.0)) as usize)
            .max(1);
        let band = |y: f64| ((((y - top) / height) * count as f64) as usize).min(count - 1);
        work.charge(pieces.len())?;
        let mut spans = reserved(pieces.len())?;
        spans.extend(pieces.iter().map(|p| (band(p.top()), band(p.bottom()))));
        let mut sorted = reserved(pieces.len())?;
        sorted.extend(0..pieces.len());
        work.sort(&mut sorted, |&a, &b| {
            pieces[a]
                .left()
                .total_cmp(&pieces[b].left())
                .then(a.cmp(&b))
        })?;
        let mut bins = filled(count, Vec::new())?;
        for piece in sorted {
            let (first, last) = spans[piece];
            work.visit(last - first + 1)?;
            for bin in &mut bins[first..=last] {
                push(
                    bin,
                    BandEntry {
                        piece,
                        subtree_right: 0.0,
                    },
                )?;
            }
        }
        for bin in &mut bins {
            work.charge(bin.len())?;
            index_band(bin, pieces);
        }
        Ok(Some(Self { bins, spans }))
    }
}

fn signed_offset(line: Piece, at: Point) -> f64 {
    let transpose = |p: Point| Point { x: p.y, y: p.x };
    let (from, to, at) = if (line.to.x - line.from.x).abs() >= (line.to.y - line.from.y).abs() {
        (line.from, line.to, at)
    } else {
        (transpose(line.from), transpose(line.to), transpose(at))
    };
    let middle = Point {
        x: from.x.midpoint(to.x),
        y: from.y.midpoint(to.y),
    };
    let mut anchor = if (at.x - from.x).abs() <= (at.x - to.x).abs() {
        from
    } else {
        to
    };
    if (at.x - middle.x).abs() < (at.x - anchor.x).abs()
        && middle.x - from.x == to.x - middle.x
        && middle.y - from.y == to.y - middle.y
    {
        anchor = middle;
    }
    let run = at.x - anchor.x;
    let height = at.y - anchor.y;
    let width = to.x - from.x;
    let rise = to.y - from.y;
    let slope = rise / width;
    let projected = if slope.is_normal() || rise == 0.0 {
        slope * run
    } else if (run / width).is_finite() {
        (run / width) * rise
    } else {
        (run * rise) / width
    };
    let offset = height - projected;
    if offset.abs() <= (4.0 * f64::EPSILON) * height.abs().max(projected.abs()) {
        0.0
    } else {
        offset.clamp(-f64::MAX, f64::MAX)
    }
}

fn zero_fraction(from: f64, to: f64) -> f64 {
    let scale = from.abs().max(to.abs());
    (from.abs() / scale) / (from.abs() / scale + to.abs() / scale)
}

fn rays(pieces: &[Piece], index: usize, at: Point) -> Option<[f64; 2]> {
    let piece = pieces[index];
    let mut ends = [piece.from, piece.to];
    if at == piece.from {
        let previous = pieces.get(index.checked_sub(1)?)?;
        if previous.edge != piece.edge {
            return None;
        }
        ends[0] = previous.from;
    } else if at == piece.to {
        let next = pieces.get(index + 1)?;
        if next.edge != piece.edge {
            return None;
        }
        ends[1] = next.to;
    }
    Some(ends.map(|end| {
        let dy = end.y - at.y;
        (if dy == 0.0 { 0.0 } else { dy }).atan2(end.x - at.x)
    }))
}

fn crossing(pieces: &[Piece], first: usize, second: usize) -> Option<Point> {
    let (a, b) = (pieces[first], pieces[second]);
    if (a.from.x == a.to.x && b.from.x == b.to.x) || (a.from.y == a.to.y && b.from.y == b.to.y) {
        return None;
    }
    let a0 = signed_offset(b, a.from);
    let a1 = signed_offset(b, a.to);
    let b0 = signed_offset(a, b.from);
    let b1 = signed_offset(a, b.to);
    let misses = |from: f64, to: f64| {
        (from == 0.0 && to == 0.0) || (from > 0.0 && to > 0.0) || (from < 0.0 && to < 0.0)
    };
    if misses(a0, a1) || misses(b0, b1) {
        return None;
    }
    let contact = [(a0, a.from), (a1, a.to), (b0, b.from), (b1, b.to)]
        .into_iter()
        .find(|&(side, _)| side == 0.0);
    let Some((_, at)) = contact else {
        return Some(
            if (a.right() - a.left()).max(a.bottom() - a.top())
                <= (b.right() - b.left()).max(b.bottom() - b.top())
            {
                lerp(a.from, a.to, zero_fraction(a0, a1))
            } else {
                lerp(b.from, b.to, zero_fraction(b0, b1))
            },
        );
    };
    if [a, b]
        .iter()
        .any(|p| at.x < p.left() || at.x > p.right() || at.y < p.top() || at.y > p.bottom())
    {
        return None;
    }
    let [start, end] = rays(pieces, first, at)?;
    let [one, two] = rays(pieces, second, at)?;
    if [one, two]
        .iter()
        .any(|&angle| angle == start || angle == end)
    {
        return None;
    }
    let inside = |angle| start.min(end) < angle && angle < start.max(end);
    (inside(one) != inside(two)).then_some(at)
}

fn in_fan(at: Point, rect: Rect) -> bool {
    rect.x - at.x < FAN
        && at.x - (rect.x + rect.width) < FAN
        && rect.y - at.y < FAN
        && at.y - (rect.y + rect.height) < FAN
}

fn shared_fan(at: Point, first: [Endpoint; 2], second: [Endpoint; 2], out: &LayoutOutput) -> bool {
    first.into_iter().any(|end| {
        if !second.contains(&end) {
            return false;
        }
        let rect = match end {
            Endpoint::Node(id) => out.nodes[id.0 as usize],
            Endpoint::Group(id) => out.groups[id.0 as usize].rect,
        };
        in_fan(at, rect)
    })
}

pub(super) fn crossings(
    graph: &Graph,
    out: &LayoutOutput,
    style: RoutingStyle,
    work: &mut Work<'_>,
) -> LayoutResult<u64> {
    work.check()?;
    if out.edges.len() != graph.edges.len()
        || out.nodes.len() != graph.nodes.len()
        || out.groups.len() != graph.groups.len()
    {
        return Err(LayoutError::InvalidInput("incomplete compound drawing"));
    }
    if graph.edges.len() < 2 {
        return Ok(0);
    }
    let pieces = flatten(out, style, work)?;
    let Some(bands) = Bands::build(&pieces, work)? else {
        return Ok(0);
    };
    work.charge(graph.edges.len())?;
    let mut ends = reserved(graph.edges.len())?;
    ends.extend(graph.edges.iter().map(|edge| [edge.source, edge.target]));
    for route in &out.truncated_routes {
        work.visit(1)?;
        ends[route.edge.0 as usize] = [route.source, route.target];
    }
    let mut seen = filled(graph.edges.len(), usize::MAX)?;
    let mut count = 0_u64;
    for (index, &query) in pieces.iter().enumerate() {
        let (first, last) = bands.spans[index];
        for band in first..=last {
            overlaps(&bands.bins[band], &pieces, query, work, &mut |candidate| {
                let other = pieces[candidate].edge;
                if seen[other] != query.edge
                    && band == first.max(bands.spans[candidate].0)
                    && let Some(at) = crossing(&pieces, index, candidate)
                    && !shared_fan(at, ends[query.edge], ends[other], out)
                {
                    seen[other] = query.edge;
                    count = count.checked_add(1).ok_or(WorkError::ArithmeticOverflow)?;
                }
                Ok(())
            })?;
        }
    }
    Ok(count)
}

fn across(at: Point, axis: Axis) -> f64 {
    match axis {
        Axis::Vertical => at.x,
        Axis::Horizontal => at.y,
    }
}

fn first_outer_turn(
    mut route: impl Iterator<Item = RoutePoint>,
    axis: Axis,
    work: &mut Work<'_>,
) -> LayoutResult<Option<f64>> {
    let first = route
        .next()
        .ok_or(LayoutError::InvalidInput("missing compound port seam"))?;
    let start = across(point(first), axis);
    for knot in route {
        work.visit(1)?;
        let cross = across(point(knot), axis);
        if (cross - start).abs() >= IN_LINE {
            return Ok(Some(cross));
        }
    }
    Ok(None)
}

pub(super) fn port_hints(
    ctx: &Pass<'_>,
    frames: &[Option<Frame>],
    output: &LayoutOutput,
    work: &mut Work<'_>,
) -> LayoutResult<Option<Vec<Option<f64>>>> {
    work.check()?;
    let stitches = &output.border_stitches;
    if stitches.len() < 2 {
        return Ok(None);
    }
    work.charge(stitches.len())?;
    let mut by_border = reserved(stitches.len())?;
    by_border.extend(0..stitches.len());
    let key = |&index: &usize| {
        let stitch = stitches[index];
        (stitch.group.0 as usize, stitch.edge.0 as usize)
    };
    work.sort(&mut by_border, |a, b| key(a).cmp(&key(b)))?;
    let lookup_cost = 1 + by_border.len().ilog2() as usize;
    let mut result = None;
    let mut positions = Vec::new();
    let mut sorted = Vec::new();
    for (level_id, group) in output.groups.iter().enumerate() {
        work.visit(1)?;
        let level = &ctx.hierarchy.levels[level_id];
        if level.ports.len() < 2 {
            continue;
        }
        let frame = frames[level_id].as_ref().expect("solved compound frame");
        let side = flow_sides(level.direction).1;
        let axis = side.axis();
        let origin = Point {
            x: group.rect.x,
            y: group.rect.y,
        };
        positions.clear();
        sorted.clear();
        for &id in &level.ports {
            work.visit(1)?;
            if ctx.sides[id] != side {
                continue;
            }
            let border = &ctx.hierarchy.borders[id];
            work.visit(lookup_cost)?;
            let Ok(index) = by_border.binary_search_by_key(&(level_id, border.edge), key) else {
                continue;
            };
            let seam = stitches[by_border[index]].point;
            let local = frame.ports[border.slot];
            let route = &output.edges[border.edge].points;
            if seam >= route.len() {
                return Err(LayoutError::InvalidInput("missing compound port seam"));
            }
            let turn = if border.outgoing {
                first_outer_turn(route[seam..].iter().copied(), axis, work)?
            } else {
                first_outer_turn(route[..=seam].iter().rev().copied(), axis, work)?
            };
            if let Some(turn) = turn {
                let hint = turn - across(origin, axis);
                if !hint.is_finite() || !across(local, axis).is_finite() {
                    return Err(WorkError::ArithmeticOverflow.into());
                }
                push(&mut sorted, positions.len())?;
                push(&mut positions, (across(local, axis), hint, id))?;
            }
        }
        work.sort(&mut sorted, |&a, &b| {
            let (left, right) = (positions[a], positions[b]);
            left.0
                .partial_cmp(&right.0)
                .unwrap()
                .then_with(|| left.1.total_cmp(&right.1))
                .then(left.2.cmp(&right.2))
        })?;
        let inverted = sorted.windows(2).any(|pair| {
            let (left, right) = (positions[pair[0]], positions[pair[1]]);
            left.0 < right.0 && left.1 > right.1
        });
        if inverted {
            for &(_, hint, id) in &positions {
                if ctx.ports[id] == Some(hint) {
                    continue;
                }
                if result.is_none() {
                    work.charge(ctx.ports.len())?;
                    let mut hints = reserved(ctx.ports.len())?;
                    hints.extend_from_slice(ctx.ports);
                    result = Some(hints);
                }
                result.as_mut().unwrap()[id] = Some(hint);
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
