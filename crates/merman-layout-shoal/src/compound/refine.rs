//! Compare stitched routing centerlines in drawing coordinates. The linear interpolation error
//! of a cubic on an interval of width `h` is at most `max(abs(B'')) * h² / 8` per coordinate.

use super::{Pass, sides::flow_sides};
use crate::geom::Bounds;
use crate::work::{LayoutResult, Work, filled, push, reserved};
use crate::{
    Axis, Endpoint, Graph, KnotKind, LayoutError, LayoutOutput, Point, Rect, RoutePoint,
    RoutingStyle, WorkError, sweep_handle_fraction,
};
use std::ops::{ControlFlow, Range};

mod sweep;
pub(crate) use sweep::PairSet;

const FAN: f64 = 24.0;
const TURN_HANDLE: f64 = 0.62;
const MAX_CORNER_ARM: f64 = 24.0;
const MAX_CORNER_ASPECT: f64 = 3.0;
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

fn along(at: Point, axis: Axis) -> f64 {
    match axis {
        Axis::Vertical => at.y,
        Axis::Horizontal => at.x,
    }
}

fn extent(bounds: Bounds, axis: Axis) -> [f64; 2] {
    match axis {
        Axis::Vertical => bounds.y,
        Axis::Horizontal => bounds.x,
    }
}

fn turn(route: &[RoutePoint], index: usize) -> bool {
    if index == 0 || index + 1 >= route.len() {
        return false;
    }
    let [before, at, after] = [route[index - 1], route[index], route[index + 1]];
    (before.x == at.x && at.y == after.y) || (before.y == at.y && at.x == after.x)
}

struct Primitive {
    edge: usize,
    ends: [Point; 2],
    bounds: Bounds,
    cubic: Option<Cubic>,
}

struct Cubic {
    handles: [Point; 2],
    minimum: usize,
    points: Range<usize>,
    ordered: [bool; 2],
}

impl Cubic {
    fn chord_count(&self, [from, to]: [Point; 2]) -> usize {
        let [first, second] = self.handles;
        let bend = (first.x - from.x.midpoint(second.x))
            .abs()
            .max((first.y - from.y.midpoint(second.y)).abs())
            .max((second.x - first.x.midpoint(to.x)).abs())
            .max((second.y - first.y.midpoint(to.y)).abs());
        ((bend.sqrt() * (1.5 / CURVE_TOLERANCE).sqrt()).ceil() as usize)
            .clamp(self.minimum, MAX_CURVE_CHORDS)
    }

    fn sample(&self, [from, to]: [Point; 2], at: f64) -> Point {
        let [first, second] = self.handles;
        let middle = lerp(first, second, at);
        lerp(
            lerp(lerp(from, first, at), middle, at),
            lerp(middle, lerp(second, to, at), at),
            at,
        )
    }
}

#[derive(Default)]
struct Centerlines {
    primitives: Vec<Primitive>,
    samples: Vec<Point>,
}

impl Centerlines {
    fn expand(&mut self, index: usize, work: &mut Work<'_>) -> LayoutResult<()> {
        let primitive = &mut self.primitives[index];
        let Some(cubic) = primitive.cubic.as_mut().filter(|c| c.points.is_empty()) else {
            return Ok(());
        };
        let [from, to] = primitive.ends;
        let chords = cubic.chord_count(primitive.ends);
        work.visit(chords)?;
        self.samples
            .try_reserve(chords + 1)
            .map_err(|_| LayoutError::AllocationLimit)?;
        let start = self.samples.len();
        self.samples.push(from);
        for index in 1..chords {
            let at = index as f64 / chords as f64;
            let sample = cubic.sample(primitive.ends, at);
            if self.samples.last() != Some(&sample) {
                self.samples.push(sample);
            }
        }
        if self.samples.last() != Some(&to) {
            self.samples.push(to);
        }
        cubic.points = start..self.samples.len();
        let samples = &self.samples[cubic.points.clone()];
        // Rounding near a precision limit can disturb the control polygon's monotone order.
        cubic.ordered = [Axis::Vertical, Axis::Horizontal].map(|axis| {
            if along(from, axis) <= along(to, axis) {
                samples.is_sorted_by(|a, b| along(*a, axis) <= along(*b, axis))
            } else {
                samples.is_sorted_by(|a, b| along(*a, axis) >= along(*b, axis))
            }
        });
        Ok(())
    }

    #[inline]
    fn points(&self, index: usize) -> &[Point] {
        let primitive = &self.primitives[index];
        primitive
            .cubic
            .as_ref()
            .map_or(&primitive.ends, |c| &self.samples[c.points.clone()])
    }
}

struct Pen<'a> {
    edge: usize,
    at: Point,
    primitives: &'a mut Vec<Primitive>,
}

impl Pen<'_> {
    fn add(&mut self, to: Point, bounds: Bounds, cubic: Option<Cubic>) -> LayoutResult<()> {
        if self.at != to {
            push(
                self.primitives,
                Primitive {
                    edge: self.edge,
                    ends: [self.at, to],
                    bounds,
                    cubic,
                },
            )?;
            self.at = to;
        }
        Ok(())
    }

    fn line(&mut self, to: Point) -> LayoutResult<()> {
        self.add(to, Bounds::between(self.at, to), None)
    }

    fn cubic(
        &mut self,
        first: Point,
        second: Point,
        to: Point,
        minimum: usize,
    ) -> LayoutResult<()> {
        let mut bounds = Bounds::between(self.at, to);
        bounds.include(first);
        bounds.include(second);
        // Sweeps and rounded corners with coincident endpoints collapse to a point.
        self.add(
            to,
            bounds,
            Some(Cubic {
                handles: [first, second],
                minimum,
                points: 0..0,
                ordered: [false; 2],
            }),
        )
    }

    fn sweep(&mut self, to: Point, axis: Axis) -> LayoutResult<()> {
        let (dx, dy) = (to.x - self.at.x, to.y - self.at.y);
        if !dx.is_finite() || !dy.is_finite() {
            return Err(WorkError::ArithmeticOverflow.into());
        }
        let (along, across) = match axis {
            Axis::Vertical => (dy, dx),
            Axis::Horizontal => (dx, dy),
        };
        let ease = sweep_handle_fraction(across, along);
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
        self.cubic(first, second, to, 3)
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

fn trace_section(
    edge: usize,
    route: &[RoutePoint],
    style: RoutingStyle,
    knots: &mut Vec<RoutePoint>,
    primitives: &mut Vec<Primitive>,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    knots.clear();
    for &knot in route {
        work.visit(1)?;
        if !knot.x.is_finite() || !knot.y.is_finite() {
            return Err(WorkError::ArithmeticOverflow.into());
        }
        if let Some(last) = knots.last_mut()
            && point(*last) == point(knot)
        {
            if knot.kind == KnotKind::GapBoundary {
                last.kind = knot.kind;
            }
            continue;
        }
        if let [.., before, last] = knots.as_slice()
            && last.kind == KnotKind::Free
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
        primitives,
    };
    for index in 1..knots.len() {
        work.visit(1)?;
        let knot = knots[index];
        let to = point(knot);
        if style == RoutingStyle::Curved && turn(knots, index) {
            let before = point(knots[index - 1]);
            let after = point(knots[index + 1]);
            let incoming = (to.x - before.x).hypot(to.y - before.y);
            let outgoing = (after.x - to.x).hypot(after.y - to.y);
            let before_arm = if turn(knots, index - 1) {
                incoming * 0.5
            } else {
                incoming
            };
            let after_arm = if turn(knots, index + 1) {
                outgoing * 0.5
            } else {
                outgoing
            };
            let track_shelf = knots[index - 1].kind == KnotKind::GapBoundary
                || knots[index + 1].kind == KnotKind::GapBoundary;
            let cap = before_arm.min(after_arm) * MAX_CORNER_ASPECT;
            let (before_arm, after_arm) = if track_shelf {
                (before_arm.min(cap), after_arm.min(cap))
            } else {
                (
                    before_arm.min(cap).min(MAX_CORNER_ARM),
                    after_arm.min(cap).min(MAX_CORNER_ARM),
                )
            };
            let start = if incoming > 0.0 {
                lerp(to, before, before_arm / incoming)
            } else {
                to
            };
            let end = if outgoing > 0.0 {
                lerp(to, after, after_arm / outgoing)
            } else {
                to
            };
            pen.line(start)?;
            pen.cubic(
                lerp(start, to, TURN_HANDLE),
                lerp(end, to, TURN_HANDLE),
                end,
                2,
            )?;
        } else if aligned(pen.at, to) || style == RoutingStyle::Polyline {
            pen.line(to)?;
        } else if style == RoutingStyle::Orthogonal {
            pen.step(to, knot.axis)?;
        } else {
            pen.sweep(to, knot.axis)?;
        }
    }
    Ok(())
}

fn trace(
    output: &LayoutOutput,
    style: RoutingStyle,
    work: &mut Work<'_>,
) -> LayoutResult<Centerlines> {
    let mut centerlines = Centerlines::default();
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
            trace_section(
                edge,
                section,
                style,
                &mut knots,
                &mut centerlines.primitives,
                work,
            )?;
            begin = stitch.point;
        }
        trace_section(
            edge,
            &geometry.points[begin..],
            style,
            &mut knots,
            &mut centerlines.primitives,
            work,
        )?;
    }
    if stitches.next().is_some() {
        return Err(LayoutError::InvalidInput(
            "unordered compound border stitches",
        ));
    }
    Ok(centerlines)
}

#[derive(Clone, Copy)]
struct BandEntry {
    primitive: usize,
    edge: usize,
    bounds: Bounds,
    subtree_right: f64,
    subtree_y: [f64; 2],
    first_edge: usize,
}

fn index_band(entries: &mut [BandEntry]) -> Option<&BandEntry> {
    let middle = entries.len() / 2;
    let (lower, upper) = entries.split_at_mut(middle);
    let (entry, upper) = upper.split_first_mut()?;
    for child in [index_band(lower), index_band(upper)].into_iter().flatten() {
        entry.subtree_right = entry.subtree_right.max(child.subtree_right);
        entry.subtree_y[0] = entry.subtree_y[0].min(child.subtree_y[0]);
        entry.subtree_y[1] = entry.subtree_y[1].max(child.subtree_y[1]);
        entry.first_edge = entry.first_edge.min(child.first_edge);
    }
    Some(entry)
}

fn overlaps(
    entries: &[BandEntry],
    bounds: Bounds,
    edge: usize,
    all_edges: bool,
    work: &mut Work<'_>,
    emit: &mut impl FnMut(&BandEntry, &mut Work<'_>) -> LayoutResult<ControlFlow<()>>,
) -> LayoutResult<ControlFlow<()>> {
    let (lower, upper) = entries.split_at(entries.len() / 2);
    let Some((entry, upper)) = upper.split_first() else {
        return Ok(ControlFlow::Continue(()));
    };
    work.visit(1)?;
    if (!all_edges && entry.first_edge >= edge)
        || entry.subtree_right < bounds.x[0]
        || entries[0].bounds.x[0] > bounds.x[1]
        || entry.subtree_y[1] < bounds.y[0]
        || bounds.y[1] < entry.subtree_y[0]
    {
        return Ok(ControlFlow::Continue(()));
    }
    if entry.bounds.x[0] > bounds.x[1] {
        return overlaps(lower, bounds, edge, all_edges, work, emit);
    }
    // A pair confined to the same axis-aligned line has no transverse crossing.
    let parallel = |[a, b]: [f64; 2], [c, d]: [f64; 2]| a == b && c == d;
    let eligible = if all_edges {
        entry.edge != edge
    } else {
        entry.edge < edge
    };
    if eligible
        && entry.bounds.overlaps(bounds)
        && !parallel(entry.bounds.x, bounds.x)
        && !parallel(entry.bounds.y, bounds.y)
        && emit(entry, work)?.is_break()
    {
        return Ok(ControlFlow::Break(()));
    }
    if overlaps(lower, bounds, edge, all_edges, work, emit)?.is_break() {
        return Ok(ControlFlow::Break(()));
    }
    overlaps(upper, bounds, edge, all_edges, work, emit)
}

struct Bands {
    bins: Vec<Vec<BandEntry>>,
    spans: Vec<(usize, usize)>,
}

impl Bands {
    fn build(primitives: &[Primitive], work: &mut Work<'_>) -> LayoutResult<Option<Self>> {
        let mut extent = Bounds {
            x: [f64::INFINITY, f64::NEG_INFINITY],
            y: [f64::INFINITY, f64::NEG_INFINITY],
        };
        for primitive in primitives {
            work.visit(1)?;
            extent = extent.union(primitive.bounds);
        }
        if !extent.has_area() {
            return Ok(None);
        }
        let [top, bottom] = extent.y;
        let height = bottom - top;
        if !height.is_finite() || !(extent.x[1] - extent.x[0]).is_finite() {
            return Err(WorkError::ArithmeticOverflow.into());
        }
        let mut covered = 0.0;
        for primitive in primitives {
            work.visit(1)?;
            let [top, bottom] = primitive.bounds.y;
            covered += (bottom - top) / height;
        }
        let count = primitives
            .len()
            .isqrt()
            .min((primitives.len() as f64 / covered.max(1.0)) as usize)
            .max(1);
        let band = |y: f64| ((((y - top) / height) * count as f64) as usize).min(count - 1);
        work.charge(primitives.len())?;
        let mut spans = reserved(primitives.len())?;
        spans.extend(
            primitives
                .iter()
                .map(|p| (band(p.bounds.y[0]), band(p.bounds.y[1]))),
        );
        let mut sorted = reserved(primitives.len())?;
        sorted.extend(0..primitives.len());
        work.sort(&mut sorted, |&a, &b| {
            primitives[a].bounds.x[0]
                .total_cmp(&primitives[b].bounds.x[0])
                .then(a.cmp(&b))
        })?;
        let mut bins = filled(count, Vec::new())?;
        for primitive in sorted {
            let (first, last) = spans[primitive];
            let bounds = primitives[primitive].bounds;
            let edge = primitives[primitive].edge;
            let entry = BandEntry {
                primitive,
                edge,
                bounds,
                subtree_right: bounds.x[1],
                subtree_y: bounds.y,
                first_edge: edge,
            };
            work.visit(last - first + 1)?;
            for bin in &mut bins[first..=last] {
                push(bin, entry)?;
            }
        }
        for bin in &mut bins {
            work.charge(bin.len())?;
            index_band(bin);
        }
        Ok(Some(Self { bins, spans }))
    }
}

fn signed_offset([from, to]: [Point; 2], at: Point) -> f64 {
    if from.x == to.x {
        return at.x - from.x;
    }
    if from.y == to.y {
        return at.y - from.y;
    }
    let transpose = |p: Point| Point { x: p.y, y: p.x };
    let (from, to, at) = if (to.x - from.x).abs() >= (to.y - from.y).abs() {
        (from, to, at)
    } else {
        (transpose(from), transpose(to), transpose(at))
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

#[derive(Clone, Copy)]
struct Chord {
    primitive: usize,
    index: usize,
    ends: [Point; 2],
}

impl Chord {
    fn bounds(self) -> Bounds {
        Bounds::between(self.ends[0], self.ends[1])
    }

    fn span(self) -> f64 {
        let [from, to] = self.ends;
        (to.x - from.x).abs().max((to.y - from.y).abs())
    }
}

impl Primitive {
    fn ordered(&self, axis: Axis) -> bool {
        self.cubic
            .as_ref()
            .is_none_or(|cubic| cubic.ordered[axis as usize])
    }

    fn chord_range(
        &self,
        points: &[Point],
        against: Bounds,
        work: &mut Work<'_>,
    ) -> LayoutResult<Range<usize>> {
        if points.len() == 2 {
            return Ok(0..1);
        }
        let mut offset = 0;
        let mut active = points;
        for axis in [Axis::Horizontal, Axis::Vertical] {
            if !self.ordered(axis) {
                continue;
            }
            let [low, high] = extent(against, axis);
            let first = along(active[0], axis);
            let last = along(active[active.len() - 1], axis);
            if first.max(last) < low || first.min(last) > high {
                return Ok(0..0);
            }
            let ascends = first <= last;
            let cost = active.len().ilog2() as usize + 1;
            let mut start = 0;
            let mut end = active.len();
            // Keep the chords on both sides of any vertex on a closed box face.
            if first < low || first > high {
                work.visit(cost)?;
                start = active
                    .partition_point(|&p| {
                        if ascends {
                            along(p, axis) < low
                        } else {
                            along(p, axis) > high
                        }
                    })
                    .saturating_sub(1);
            }
            if last < low || last > high {
                work.visit(cost)?;
                end = active.partition_point(|&p| {
                    if ascends {
                        along(p, axis) <= high
                    } else {
                        along(p, axis) >= low
                    }
                }) + 1;
            }
            active = &active[start..end];
            offset += start;
        }
        Ok(offset..offset + active.len() - 1)
    }
}

struct FineRun<'a> {
    points: &'a [Point],
    indices: Range<usize>,
    primitive: usize,
}

impl FineRun<'_> {
    fn start(&self) -> Point {
        self.points[self.indices.start]
    }
    fn end(&self) -> Point {
        self.points[self.indices.end]
    }
    #[inline]
    fn chord(&self, position: usize, reverse: bool) -> Chord {
        let index = if reverse {
            self.indices.end - 1 - position
        } else {
            self.indices.start + position
        };
        Chord {
            primitive: self.primitive,
            index,
            ends: [self.points[index], self.points[index + 1]],
        }
    }
}

impl Centerlines {
    pub(super) fn chord(&self, primitive: usize, index: usize) -> Chord {
        let points = self.points(primitive);
        Chord {
            primitive,
            index,
            ends: [points[index], points[index + 1]],
        }
    }

    fn ray_ends(
        &self,
        chord: Chord,
        at: Point,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<[Point; 2]>> {
        let mut ends = chord.ends;
        let (side, nearby, neighbor) = if at == ends[0] {
            (
                0,
                chord.index.checked_sub(1),
                chord.primitive.checked_sub(1),
            )
        } else if at == ends[1] {
            (1, Some(chord.index + 2), Some(chord.primitive + 1))
        } else {
            return Ok(Some(ends));
        };
        if let Some(&point) = nearby.and_then(|i| self.points(chord.primitive).get(i)) {
            ends[side] = point;
        } else {
            // The adjacent ray can belong to the next primitive, even across a border stitch.
            let edge = self.primitives[chord.primitive].edge;
            let Some(neighbor) = neighbor.filter(|&id| {
                self.primitives
                    .get(id)
                    .is_some_and(|primitive| primitive.edge == edge)
            }) else {
                return Ok(None);
            };
            ends[side] = self.near_end(neighbor, side == 0, work)?;
        }
        Ok(Some(ends))
    }

    fn near_end(&self, index: usize, at_end: bool, work: &mut Work<'_>) -> LayoutResult<Point> {
        let primitive = &self.primitives[index];
        let points = self.points(index);
        if !points.is_empty() {
            return Ok(if at_end {
                points[points.len() - 2]
            } else {
                points[1]
            });
        }
        let cubic = primitive.cubic.as_ref().unwrap();
        let [from, to] = primitive.ends;
        let (at, far) = if at_end { (to, from) } else { (from, to) };
        let chords = cubic.chord_count(primitive.ends);
        // Find the adjacent distinct sample without expanding a curve outside the query.
        for index in 1..chords {
            work.visit(1)?;
            let index = if at_end { chords - index } else { index };
            let sample = cubic.sample(primitive.ends, index as f64 / chords as f64);
            if sample != at {
                return Ok(sample);
            }
        }
        Ok(far)
    }
}

#[inline]
fn strictly_same_side([from, to]: [Point; 2], p0: Point, p1: Point) -> bool {
    let dx = to.x - from.x;
    let dy = to.y - from.y;
    let side = |p: Point| {
        let a = dx * (p.y - from.y);
        let b = dy * (p.x - from.x);
        let det = a - b;
        let bound = (16.0 * f64::EPSILON) * (a.abs() + b.abs());
        if det > bound {
            1i8
        } else if det < -bound {
            -1i8
        } else {
            0i8
        }
    };
    let s0 = side(p0);
    s0 != 0 && s0 == side(p1)
}

impl Centerlines {
    fn crossing(
        &self,
        a: Chord,
        b: Chord,
        in_shared_fan: &dyn Fn(Point) -> bool,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        let [a_from, a_to] = a.ends;
        let [b_from, b_to] = b.ends;
        if (a_from.x == a_to.x && b_from.x == b_to.x) || (a_from.y == a_to.y && b_from.y == b_to.y)
        {
            return Ok(false);
        }
        // Division-free Shewchuk orientation filter rejects clearly separated chord pairs
        // without paying `signed_offset`'s division, falling back to exact `signed_offset`
        // whenever either endpoint is near-collinear or straddling.
        if strictly_same_side(b.ends, a_from, a_to) || strictly_same_side(a.ends, b_from, b_to) {
            return Ok(false);
        }
        let misses = |from: f64, to: f64| {
            (from == 0.0 && to == 0.0) || (from > 0.0 && to > 0.0) || (from < 0.0 && to < 0.0)
        };
        let a0 = signed_offset(b.ends, a_from);
        let a1 = signed_offset(b.ends, a_to);
        if misses(a0, a1) {
            return Ok(false);
        }
        let b0 = signed_offset(a.ends, b_from);
        let b1 = signed_offset(a.ends, b_to);
        if misses(b0, b1) {
            return Ok(false);
        }
        let contact = [(a0, a_from), (a1, a_to), (b0, b_from), (b1, b_to)]
            .into_iter()
            .find(|&(side, _)| side == 0.0);
        let Some((_, at)) = contact else {
            let at = if a.span() <= b.span() {
                lerp(a_from, a_to, zero_fraction(a0, a1))
            } else {
                lerp(b_from, b_to, zero_fraction(b0, b1))
            };
            return Ok(!in_shared_fan(at));
        };
        if [a.bounds(), b.bounds()].iter().any(|bounds| {
            at.x < bounds.x[0] || at.x > bounds.x[1] || at.y < bounds.y[0] || at.y > bounds.y[1]
        }) || in_shared_fan(at)
        {
            return Ok(false);
        }
        let Some(a_ends) = self.ray_ends(a, at, work)? else {
            return Ok(false);
        };
        let Some(b_ends) = self.ray_ends(b, at, work)? else {
            return Ok(false);
        };
        let angles = |ends: [Point; 2]| {
            ends.map(|end| {
                let dy = end.y - at.y;
                (if dy == 0.0 { 0.0 } else { dy }).atan2(end.x - at.x)
            })
        };
        let [start, end] = angles(a_ends);
        let [one, two] = angles(b_ends);
        if [one, two]
            .iter()
            .any(|&angle| angle == start || angle == end)
        {
            return Ok(false);
        }
        let inside = |angle| start.min(end) < angle && angle < start.max(end);
        Ok(inside(one) != inside(two))
    }

    #[inline(never)]
    fn crosses(
        &mut self,
        first: usize,
        second: usize,
        in_shared_fan: &dyn Fn(Point) -> bool,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        let (a, b) = (&self.primitives[first], &self.primitives[second]);
        if a.cubic.is_none() && b.cubic.is_none() {
            return self.crossing(
                self.chord(first, 0),
                self.chord(second, 0),
                in_shared_fan,
                work,
            );
        }
        self.expand(first, work)?;
        self.expand(second, work)?;
        self.crosses_expanded(first, second, in_shared_fan, work)
    }

    #[inline(never)]
    pub(super) fn crosses_expanded(
        &self,
        first: usize,
        second: usize,
        in_shared_fan: &dyn Fn(Point) -> bool,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        let (a, b) = (&self.primitives[first], &self.primitives[second]);
        if a.cubic.is_none() && b.cubic.is_none() {
            return self.crossing(
                self.chord(first, 0),
                self.chord(second, 0),
                in_shared_fan,
                work,
            );
        }
        let a_points = self.points(first);
        let b_points = self.points(second);
        let a_range = a.chord_range(a_points, b.bounds, work)?;
        let b_range = b.chord_range(b_points, a.bounds, work)?;
        if a_range.is_empty() || b_range.is_empty() {
            return Ok(false);
        }
        let a = FineRun {
            points: a_points,
            indices: a_range,
            primitive: first,
        };
        let b = FineRun {
            points: b_points,
            indices: b_range,
            primitive: second,
        };
        if a.indices.len() == 1 && b.indices.len() == 1 {
            return self.test_chords(a.chord(0, false), b.chord(0, false), in_shared_fan, work);
        }
        let (first, second) = (&self.primitives[first], &self.primitives[second]);
        let ordered = |axis| first.ordered(axis) && second.ordered(axis);
        let same_extent = |axis| {
            let span = extent(first.bounds, axis);
            span[0] < span[1] && span == extent(second.bounds, axis)
        };
        let shorter_span = |axis| {
            let span = |p: &Primitive| {
                let [start, end] = extent(p.bounds, axis);
                end - start
            };
            span(first).min(span(second))
        };
        let horizontal = Axis::Horizontal;
        let vertical = Axis::Vertical;
        let axis = match (ordered(horizontal), ordered(vertical)) {
            (true, true) => {
                // A common extent can be the tighter sweep axis even for very wide curves.
                match (same_extent(horizontal), same_extent(vertical)) {
                    (true, false) => Some(horizontal),
                    (false, true) => Some(vertical),
                    _ if shorter_span(vertical) > shorter_span(horizontal) => Some(vertical),
                    _ => Some(horizontal),
                }
            }
            (true, false) => Some(horizontal),
            (false, true) => Some(vertical),
            (false, false) => None,
        };
        match axis {
            Some(Axis::Vertical) => self.scan::<true, true>(&a, &b, in_shared_fan, work),
            Some(Axis::Horizontal) => self.scan::<false, true>(&a, &b, in_shared_fan, work),
            None => self.scan::<true, false>(&a, &b, in_shared_fan, work),
        }
    }

    fn test_chords(
        &self,
        a: Chord,
        b: Chord,
        fan: &dyn Fn(Point) -> bool,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        work.visit(1)?;
        Ok(a.bounds().overlaps(b.bounds()) && self.crossing(a, b, fan, work)?)
    }

    fn scan<const VERTICAL: bool, const ORDERED: bool>(
        &self,
        a: &FineRun<'_>,
        b: &FineRun<'_>,
        fan: &dyn Fn(Point) -> bool,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        let along = |p: Point| if VERTICAL { p.y } else { p.x };
        let across = |p: Point| if VERTICAL { p.x } else { p.y };
        let (a_rev, b_rev) = (
            ORDERED && along(a.start()) > along(a.end()),
            ORDERED && along(b.start()) > along(b.end()),
        );
        let across_axis = if VERTICAL {
            Axis::Horizontal
        } else {
            Axis::Vertical
        };
        let across_rev = (across(a.start()) > across(a.end())) != a_rev;
        let across_ordered = ORDERED
            && self.primitives[a.primitive].ordered(across_axis)
            && self.primitives[b.primitive].ordered(across_axis)
            && across_rev == ((across(b.start()) > across(b.end())) != b_rev);
        // Advance on the second axis as well when both runs traverse it in the same direction.
        let mut first_b = 0;
        for i in 0..a.indices.len() {
            let a = a.chord(i, a_rev);
            let [a0, a1] = if a_rev {
                [a.ends[1], a.ends[0]]
            } else {
                a.ends
            };
            let a_low = across(a0).min(across(a1));
            let a_high = across(a0).max(across(a1));
            let mut j = first_b;
            // One row visits at most the chord cap before the next accounting checkpoint.
            let mut visits = 1;
            while j < b.indices.len() {
                let b = b.chord(j, b_rev);
                visits += 1;
                let [b0, b1] = if b_rev {
                    [b.ends[1], b.ends[0]]
                } else {
                    b.ends
                };
                let b_low = across(b0).min(across(b1));
                let b_high = across(b0).max(across(b1));
                if ORDERED {
                    if along(b0) > along(a1)
                        || (across_ordered
                            && if across_rev {
                                b_high < a_low
                            } else {
                                b_low > a_high
                            })
                    {
                        break;
                    }
                    if along(b1) < along(a0)
                        || (across_ordered
                            && if across_rev {
                                b_low > a_high
                            } else {
                                b_high < a_low
                            })
                    {
                        first_b = j + 1;
                        j += 1;
                        continue;
                    }
                }
                if a_low <= b_high
                    && b_low <= a_high
                    && (ORDERED || a.bounds().overlaps(b.bounds()))
                    && self.crossing(a, b, fan, work)?
                {
                    work.visit(visits)?;
                    return Ok(true);
                }
                j += 1;
            }
            work.visit(visits)?;
            if first_b == b.indices.len() {
                break;
            }
        }
        Ok(false)
    }
}

fn in_fan(at: Point, rect: Rect) -> bool {
    rect.x - at.x < FAN
        && at.x - (rect.x + rect.width) < FAN
        && rect.y - at.y < FAN
        && at.y - (rect.y + rect.height) < FAN
}

pub(super) fn bounds_in_shared_fan(
    a: Bounds,
    b: Bounds,
    first: [Endpoint; 2],
    second: [Endpoint; 2],
    out: &LayoutOutput,
) -> bool {
    let inter = Bounds {
        x: [a.x[0].max(b.x[0]), a.x[1].min(b.x[1])],
        y: [a.y[0].max(b.y[0]), a.y[1].min(b.y[1])],
    };
    first.into_iter().any(|end| {
        if !second.contains(&end) {
            return false;
        }
        let rect = match end {
            Endpoint::Node(id) => out.nodes[id.0 as usize],
            Endpoint::Group(id) => out.groups[id.0 as usize].rect,
        };
        rect.x - inter.x[0] < FAN
            && inter.x[1] - (rect.x + rect.width) < FAN
            && rect.y - inter.y[0] < FAN
            && inter.y[1] - (rect.y + rect.height) < FAN
    })
}

pub(super) fn endpoint_in_fan(at: Point, end: Endpoint, out: &LayoutOutput) -> bool {
    let rect = match end {
        Endpoint::Node(id) => out.nodes[id.0 as usize],
        Endpoint::Group(id) => out.groups[id.0 as usize].rect,
    };
    in_fan(at, rect)
}

pub(super) fn endpoint_in_fan_interior(
    at: Point,
    end: Endpoint,
    margin: f64,
    out: &LayoutOutput,
) -> bool {
    let safe_fan = FAN - margin;
    if !safe_fan.is_finite() {
        return false;
    }
    let rect = match end {
        Endpoint::Node(id) => out.nodes[id.0 as usize],
        Endpoint::Group(id) => out.groups[id.0 as usize].rect,
    };
    rect.x - at.x < safe_fan
        && at.x - (rect.x + rect.width) < safe_fan
        && rect.y - at.y < safe_fan
        && at.y - (rect.y + rect.height) < safe_fan
}

pub(super) fn shared_fan(
    at: Point,
    first: [Endpoint; 2],
    second: [Endpoint; 2],
    out: &LayoutOutput,
) -> bool {
    first
        .into_iter()
        .any(|end| second.contains(&end) && endpoint_in_fan(at, end, out))
}

pub(crate) fn crossings(
    graph: &Graph,
    out: &LayoutOutput,
    style: RoutingStyle,
    work: &mut Work<'_>,
) -> LayoutResult<u64> {
    count_crossings(graph, out, style, u64::MAX, work)
}

pub(super) fn count_crossings(
    graph: &Graph,
    out: &LayoutOutput,
    style: RoutingStyle,
    limit: u64,
    work: &mut Work<'_>,
) -> LayoutResult<u64> {
    count_crossings_impl(graph, out, style, limit, true, work)
}

fn count_crossings_impl(
    graph: &Graph,
    out: &LayoutOutput,
    style: RoutingStyle,
    limit: u64,
    allow_sweep: bool,
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
    let mut centerlines = trace(out, style, work)?;
    work.charge(graph.edges.len())?;
    let mut ends = reserved(graph.edges.len())?;
    ends.extend(graph.edges.iter().map(|edge| [edge.source, edge.target]));
    for route in &out.truncated_routes {
        work.visit(1)?;
        ends[route.edge.0 as usize] = [route.source, route.target];
    }

    if allow_sweep && centerlines.primitives.len() >= sweep::MIN_SWEEP_PRIMITIVES {
        let (x_overlaps, overlapping_count) = sweep::x_overlap_mask(&centerlines.primitives, work)?;
        if overlapping_count == 0 {
            return Ok(0);
        }
        if let Some(mut partition) =
            sweep::MonotonePartition::build(&mut centerlines, &x_overlaps, work)?
            && let Some(mut crossed) =
                sweep::run_monotone_sweep(&centerlines, &mut partition, &ends, out, limit, work)?
        {
            let mut count = crossed.len() as u64;
            if count >= limit || partition.other.is_empty() {
                return Ok(count);
            }
            let Some(bands) = Bands::build(&centerlines.primitives, work)? else {
                return Ok(count);
            };
            for &index in &partition.other {
                let query = &centerlines.primitives[index];
                let (edge, bounds) = (query.edge, query.bounds);
                let (first, last) = bands.spans[index];
                for band in first..=last {
                    let flow = overlaps(
                        &bands.bins[band],
                        bounds,
                        edge,
                        true,
                        work,
                        &mut |candidate, work| {
                            let other = candidate.edge;
                            let other_in_sweep = partition.in_sweep[candidate.primitive];
                            if !other_in_sweep && other >= edge {
                                return Ok(ControlFlow::Continue(()));
                            }
                            if band != first.max(bands.spans[candidate.primitive].0) {
                                return Ok(ControlFlow::Continue(()));
                            }
                            let pair = if edge < other {
                                (edge as u32, other as u32)
                            } else {
                                (other as u32, edge as u32)
                            };
                            if !crossed.contains(pair)
                                && !bounds_in_shared_fan(
                                    bounds,
                                    candidate.bounds,
                                    ends[edge],
                                    ends[other],
                                    out,
                                )
                                && centerlines.crosses(
                                    index,
                                    candidate.primitive,
                                    &|at| shared_fan(at, ends[edge], ends[other], out),
                                    work,
                                )?
                            {
                                crossed.insert(pair, work)?;
                                count =
                                    count.checked_add(1).ok_or(WorkError::ArithmeticOverflow)?;
                                if count >= limit {
                                    return Ok(ControlFlow::Break(()));
                                }
                            }
                            Ok(ControlFlow::Continue(()))
                        },
                    )?;
                    if flow.is_break() || count >= limit {
                        return Ok(count);
                    }
                }
            }
            return Ok(count);
        }
    }

    let Some(bands) = Bands::build(&centerlines.primitives, work)? else {
        return Ok(0);
    };
    let mut seen = filled(graph.edges.len(), usize::MAX)?;
    let mut count = 0_u64;
    for index in 0..centerlines.primitives.len() {
        let query = &centerlines.primitives[index];
        let (edge, bounds) = (query.edge, query.bounds);
        let (first, last) = bands.spans[index];
        for band in first..=last {
            let flow = overlaps(
                &bands.bins[band],
                bounds,
                edge,
                false,
                work,
                &mut |candidate, work| {
                    let other = candidate.edge;
                    if seen[other] != edge
                        && band == first.max(bands.spans[candidate.primitive].0)
                        && !bounds_in_shared_fan(
                            bounds,
                            candidate.bounds,
                            ends[edge],
                            ends[other],
                            out,
                        )
                        && centerlines.crosses(
                            index,
                            candidate.primitive,
                            &|at| shared_fan(at, ends[edge], ends[other], out),
                            work,
                        )?
                    {
                        seen[other] = edge;
                        count = count.checked_add(1).ok_or(WorkError::ArithmeticOverflow)?;
                        if count >= limit {
                            return Ok(ControlFlow::Break(()));
                        }
                    }
                    Ok(ControlFlow::Continue(()))
                },
            )?;
            // A trial needs its precise score only if it improves on the current drawing.
            if flow.is_break() || count >= limit {
                return Ok(count);
            }
        }
    }
    Ok(count)
}

fn across(at: RoutePoint, axis: Axis) -> f64 {
    match axis {
        Axis::Vertical => at.x,
        Axis::Horizontal => at.y,
    }
}

fn outer_cross(
    seam: f64,
    route: impl Iterator<Item = RoutePoint>,
    axis: Axis,
    work: &mut Work<'_>,
) -> LayoutResult<f64> {
    for knot in route {
        work.visit(1)?;
        let cross = across(knot, axis);
        if (cross - seam).abs() >= IN_LINE {
            return Ok(cross);
        }
    }
    Ok(seam)
}

pub(super) fn port_hints(
    ctx: &Pass<'_>,
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
    'face: for level_id in 0..output.groups.len() {
        work.visit(1)?;
        let level = &ctx.hierarchy.levels[level_id];
        if level.ports.len() < 2 {
            continue;
        }
        let side = flow_sides(level.direction).1;
        let axis = side.axis();
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
                // Group endpoints can have only an inner leg. Keep that face's original keys.
                continue 'face;
            };
            let seam = stitches[by_border[index]].point;
            let route = &output.edges[border.edge].points;
            let Some(&port) = route.get(seam) else {
                return Err(LayoutError::InvalidInput("missing compound port seam"));
            };
            let at = across(port, axis);
            let hint = if border.outgoing {
                outer_cross(at, route[seam + 1..].iter().copied(), axis, work)?
            } else {
                outer_cross(at, route[..seam].iter().rev().copied(), axis, work)?
            };
            if !at.is_finite() || !hint.is_finite() {
                return Err(WorkError::ArithmeticOverflow.into());
            }
            push(&mut sorted, positions.len())?;
            push(&mut positions, (at, hint, id))?;
        }
        // At each boundary between distinct seams, compare the largest and smallest outer keys.
        work.sort(&mut sorted, |&a, &b| {
            let (left, right) = (positions[a], positions[b]);
            left.0
                .total_cmp(&right.0)
                .then_with(|| left.1.total_cmp(&right.1))
                .then(left.2.cmp(&right.2))
        })?;
        work.visit(sorted.len().saturating_sub(1))?;
        let inverted = sorted.windows(2).any(|pair| {
            let (left, right) = (positions[pair[0]], positions[pair[1]]);
            left.0 < right.0 && left.1 > right.1
        });
        if inverted {
            for &(_, hint, id) in &positions {
                work.visit(1)?;
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
